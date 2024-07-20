mod highlight;

use std::env;
use std::error::Error;
use std::str;
use std::time::Instant;

use color_print::{cformat, cprintln};
use config::Config;
use serde_derive::{Deserialize, Serialize};
use serde_json::{Map, Value};
use xdg::BaseDirectories;

// Includes `GROQ_MODEL_MAPPING` and `OLLAMA_MODEL_MAPPING` from `/build.rs`
include!(concat!(env!("OUT_DIR"), "/models.rs"));

#[derive(Serialize, Debug, PartialEq, Default, Clone, Copy)]
pub struct ExecOptions {
  pub is_raw: bool, // Raw output mode (no metadata and no syntax highlighting)
  pub is_json: bool, // JSON output mode
}

#[derive(Serialize, Debug, PartialEq, Default, Clone, Copy)]
pub enum Provider {
  #[default]
  Anthropic,
  Groq,
  OpenAI,
  Llamafile,
  Ollama,
}

impl std::fmt::Display for Provider {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Provider::Anthropic => write!(f, "Anthropic"),
      Provider::Groq => write!(f, "Groq"),
      Provider::OpenAI => write!(f, "OpenAI"),
      Provider::Llamafile => write!(f, "Llamafile"),
      Provider::Ollama => write!(f, "Ollama"),
    }
  }
}

#[derive(Serialize, Debug, PartialEq, Clone)]
pub enum Model {
  Model(Provider, String),
}

impl Default for Model {
  fn default() -> Model {
    Model::Model(Provider::Groq, "llama3-8b-8192".to_owned())
  }
}

impl std::fmt::Display for Model {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Model::Model(provider, model_id) => {
        if model_id.is_empty() {
          write!(f, "{}", provider)
        } else {
          write!(f, "{} {}", provider, model_id)
        }
      }
    }
  }
}

#[derive(Serialize, Debug, Clone)]
struct AiRequest {
  provider: Provider,
  url: String,
  model: String,
  prompt: String,
  max_tokens: u32,
  api_key: String,
}

impl Default for AiRequest {
  fn default() -> AiRequest {
    AiRequest {
      provider: Default::default(),
      url: Default::default(),
      model: Default::default(),
      prompt: Default::default(),
      max_tokens: 4096,
      api_key: Default::default(),
    }
  }
}

#[derive(Deserialize, Debug)]
struct AiMessage {
  // role: String,
  content: String,
}

#[derive(Deserialize, Debug)]
struct AiChoice {
  // index: u32,
  message: AiMessage,
  // logprobs: Option<Value>,
  // finish_reason: String,
}

#[derive(Deserialize, Debug)]
struct AiResponse {
  choices: Vec<AiChoice>,
}

/// For Anthropic's API
/// (https://docs.anthropic.com/claude/reference/messages_post)
#[derive(Deserialize, Debug)]
struct AnthropicAiContent {
  text: String,
}

#[derive(Deserialize, Debug)]
struct AnthropicAiResponse {
  content: Vec<AnthropicAiContent>,
}

fn default_req_for_model(model: &Model) -> AiRequest {
  let Model::Model(provider, model_id) = model;

  match provider {
    Provider::Groq => AiRequest {
      provider: Provider::Groq,
      url: "https://api.groq.com/openai/v1/chat/completions".to_string(),
      model: get_groq_model(model_id).to_string(),
      ..Default::default()
    },
    Provider::OpenAI => AiRequest {
      provider: Provider::OpenAI,
      url: "https://api.openai.com/v1/chat/completions".to_string(),
      model: get_openai_model(model_id).to_string(),
      ..Default::default()
    },
    Provider::Anthropic => AiRequest {
      provider: Provider::Anthropic,
      url: "https://api.anthropic.com/v1/messages".to_string(),
      model: get_anthropic_model(model_id).to_string(),
      max_tokens: 4096,
      ..Default::default()
    },
    Provider::Llamafile => AiRequest {
      provider: Provider::Llamafile,
      url: "http://localhost:8080/v1/chat/completions".to_string(),
      ..Default::default()
    },
    Provider::Ollama => AiRequest {
      provider: Provider::Ollama,
      url: "http://localhost:11434/v1/chat/completions".to_string(),
      model: get_ollama_model(model_id).to_string(),
      ..Default::default()
    },
  }
}

fn get_key_setup_msg(secrets_path_str: &str) -> String {
  format!(
    "An API key must be provided. Use one of the following options:\n\
        \n\
        1. Set one or more API keys in {secrets_path_str}\n\
           (`anthropic_api_key`, `groq_api_key`, `openai_api_key`)\n\
        2. Set one or more cai specific env variables\n\
            (CAI_ANTHROPIC_API_KEY, CAI_GROQ_API_KEY, CAI_OPENAI_API_KEY)\n\
        3. Set one or more generic env variables\n\
            (ANTHROPIC_API_KEY, GROQ_API_KEY, OPENAI_API_KEY)\n\
        ",
  )
}

fn get_api_request(
  full_config: &HashMap<String, String>,
  secrets_path_str: &str,
  model: &Model,
) -> Result<AiRequest, String> {
  let dummy_key = "DUMMY_KEY".to_string();
  let Model::Model(provider, _) = model;

  {
    match provider {
      Provider::Groq => full_config.get("groq_api_key"),
      Provider::OpenAI => full_config.get("openai_api_key"),
      Provider::Anthropic => full_config.get("anthropic_api_key"),
      Provider::Llamafile => Some(&dummy_key),
      Provider::Ollama => Some(&dummy_key),
    }
  }
  .and_then(|api_key| {
    if api_key.is_empty() {
      None
    } else {
      Some(api_key.to_string())
    }
  })
  .map(|api_key| api_key.to_string())
  .ok_or(get_key_setup_msg(secrets_path_str))
  .map(|api_key| AiRequest {
    api_key: api_key.clone(),
    ..(default_req_for_model(model)).clone()
  })
}

fn get_used_model(model: &Model) -> String {
  let Model::Model(provider, model_id) = model;

  if model_id.is_empty() {
    cformat!("<bold>🧠 {}</bold>", provider)
  } else {
    let full_model_id = match provider {
      Provider::Groq => get_groq_model(model_id),
      Provider::OpenAI => get_openai_model(model_id),
      Provider::Anthropic => get_anthropic_model(model_id),
      Provider::Llamafile => model_id,
      Provider::Ollama => get_ollama_model(model_id),
    };
    cformat!("<bold>🧠 {} {}</bold>", provider, full_model_id)
  }
}

pub async fn exec_tool(
  optional_model: &Option<&Model>,
  opts: &ExecOptions,
  user_input: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let start = Instant::now();
  let xdg_dirs = BaseDirectories::with_prefix("cai").unwrap();
  let secrets_path = xdg_dirs
    .place_config_file("secrets.yaml")
    .expect("Couldn't create configuration directory");

  let _ = std::fs::File::create_new(&secrets_path);

  let secrets_path_str = secrets_path.to_str().unwrap();

  let config = Config::builder()
    .set_default(
      "anthropic_api_key",
      env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
    )?
    .set_default(
      "openai_api_key",
      env::var("OPENAI_API_KEY").unwrap_or_default(),
    )?
    .set_default(
      "groq_api_key", //
      env::var("GROQ_API_KEY").unwrap_or_default(),
    )?
    .add_source(config::File::with_name(secrets_path_str))
    .add_source(config::Environment::with_prefix("CAI"))
    .build()
    .unwrap();

  let full_config = config //
    .try_deserialize::<HashMap<String, String>>()
    .unwrap();

  let used_model: String;
  let http_req = match optional_model {
    Some(model) => {
      used_model = get_used_model(&model);
      get_api_request(&full_config, secrets_path_str, model)?
    }
    // Use the first provider that has an API key
    None => {
      let req =
        get_api_request(&full_config, secrets_path_str, &Default::default())
          .or(get_api_request(
            &full_config,
            secrets_path_str,
            &Model::Model(Provider::Groq, "llama3-8b-8192".to_owned()),
          ))
          .or(get_api_request(
            &full_config,
            secrets_path_str,
            &Model::Model(Provider::OpenAI, "gpt-4o-mini".to_string()),
          ))?;
      used_model = get_used_model(
        &Model::Model(req.provider.clone(), req.model.clone()), //
      );
      req
    }
  };

  // This is checked here, so that the missing API key message comes first
  if user_input.is_empty() {
    Err("No prompt was provided")?;
  }

  let req_body_obj = {
    let mut map = Map::new();
    map.insert("model".to_string(), Value::String(http_req.model));
    map.insert(
      "max_tokens".to_string(),
      Value::Number(http_req.max_tokens.into()),
    );
    if opts.is_json {
      match http_req.provider {
        Provider::OpenAI | Provider::Groq | Provider::Ollama => {
          map.insert(
            "response_format".to_string(),
            Value::Object(Map::from_iter([(
              "type".to_string(),
              Value::String("json_object".to_string()),
            )])),
          );
        }
        provider => {
          eprintln!(
            "{}",
            cformat!(
              "<red>ERROR: {provider} doesn't support a JSON mode</red>",
            )
          );
          std::process::exit(1);
        }
      }
    }
    map.insert(
      "messages".to_string(),
      Value::Array(vec![Value::Object(Map::from_iter([
        ("role".to_string(), "user".into()),
        ("content".to_string(), Value::String(user_input.to_string())),
      ]))]),
    );
    Value::Object(map)
  };

  let client = reqwest::Client::new();
  let req_base = client.post(http_req.url.clone()).json(&req_body_obj);
  let req = match http_req.provider {
    Provider::Anthropic => req_base
      .header("anthropic-version", "2023-06-01")
      .header("x-api-key", http_req.api_key),
    _ => req_base.bearer_auth(http_req.api_key),
  };

  let resp = req.send().await?;
  let elapsed_time: String = start.elapsed().as_millis().to_string();

  if !&resp.status().is_success() {
    let resp_json = resp.json::<Value>().await?;
    let resp_formatted = serde_json::to_string_pretty(&resp_json).unwrap();
    Err(format!(
      "<bold>⏱️ {: >5} ms</bold> | {used_model}\n\
      \n{resp_formatted}",
      elapsed_time,
    ))?;
  } else {
    let msg = match http_req.provider {
      Provider::Anthropic => {
        let anth_response = resp.json::<AnthropicAiResponse>().await?;
        anth_response.content[0].text.clone()
      }
      _ => {
        let ai_response = resp.json::<AiResponse>().await?;
        ai_response.choices[0].message.content.clone()
      }
    };

    if opts.is_raw {
      println!("{}", msg);
    } else {
      cprintln!("<bold>⏱️{: >5} ms</bold> | {used_model}\n", elapsed_time,);
      highlight::text_via_bat(&msg);
      println!("\n");
    }
  }
  Ok(())
}

pub async fn submit_prompt(
  optional_model: &Option<&Model>,
  opts: &ExecOptions,
  user_input: &str,
) {
  // Necessary to wrap the execution function,
  // because a `main` function that returns a `Result` quotes any errors.
  match exec_tool(optional_model, &opts, user_input).await {
    Ok(_) => (),
    Err(err) => {
      let model_str = optional_model
        .as_ref()
        .map(|x| x.to_string())
        .unwrap_or("".to_string());
      eprintln!(
        "{}",
        cformat!("<bold>🧠 {model_str}</bold><red>\nERROR:\n{}</red>\n", err)
      );
      std::process::exit(1);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_submit_empty_prompt() {
    let prompt = "";
    let result = exec_tool(
      &Some(&Model::Model(Provider::OpenAI, "gpt-4o-mini".to_owned())),
      &ExecOptions {
        is_raw: false,
        is_json: false,
      },
      &prompt,
    )
    .await;
    assert!(result.is_err());
  }
}
