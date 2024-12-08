mod highlight;

use base64::Engine;
use std::env;
use std::error::Error;
use std::str;
use std::time::Instant;

use color_print::{cformat, cprintln};
use config::Config;
use reqwest::Response;
use serde_derive::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use xdg::BaseDirectories;

// Includes `GROQ_MODEL_MAPPING` and `OLLAMA_MODEL_MAPPING` from `/build.rs`
include!(concat!(env!("OUT_DIR"), "/models.rs"));

#[derive(Serialize, Debug, PartialEq, Default, Clone)]
pub struct ExecOptions {
  pub is_raw: bool, // Raw output mode (no metadata and no syntax highlighting)
  pub is_json: bool, // JSON output mode
  pub json_schema: Option<Value>, // JSON schema of expected output
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
    Model::Model(Provider::Groq, "llama-3.1-8b-instant".to_owned())
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

fn get_secrets_path_str() -> String {
  let xdg_dirs = BaseDirectories::with_prefix("cai").unwrap();
  let secrets_path = xdg_dirs
    .place_config_file("secrets.yaml")
    .expect("Couldn't create configuration directory");
  let _ = std::fs::File::create_new(&secrets_path);
  secrets_path.to_str().unwrap().to_string()
}

pub fn get_full_config(
  secrets_path_str: &str,
) -> Result<
  HashMap<std::string::String, std::string::String>,
  config::ConfigError,
> {
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
    .add_source(config::File::with_name(&secrets_path_str))
    .add_source(config::Environment::with_prefix("CAI"))
    .build()
    .unwrap();

  Ok(
    config //
      .try_deserialize::<HashMap<String, String>>()
      .unwrap(),
  )
}

fn get_http_req(
  optional_model: &Option<&Model>,
  secrets_path_str: &str,
  full_config: &HashMap<String, String>,
) -> Result<(String, AiRequest), std::string::String> {
  match optional_model {
    Some(model) => {
      let used_model = get_used_model(&model);
      get_api_request(&full_config, &secrets_path_str, model)
        .map(|req| (used_model, req))
    }
    // Use the first provider that has an API key
    None => {
      let req =
        get_api_request(&full_config, &secrets_path_str, &Default::default())
          .or(get_api_request(
            &full_config,
            &secrets_path_str,
            &Model::Model(Provider::Groq, "llama-3.1-8b-instant".to_owned()),
          ))
          .or(get_api_request(
            &full_config,
            &secrets_path_str,
            &Model::Model(Provider::OpenAI, "gpt-4o-mini".to_string()),
          ))?;
      let used_model = get_used_model(
        &Model::Model(req.provider.clone(), req.model.clone()), //
      );
      Ok((used_model, req))
    }
  }
}

fn get_req_body_obj(
  opts: &ExecOptions,
  http_req: &AiRequest,
  user_input: &str,
) -> Value {
  // Handle case where input is already a complete JSON string
  match serde_json::from_str(user_input) {
    Ok(json) => return json,
    Err(_) => (),
  }

  let mut map = Map::new();
  map.insert("model".to_string(), Value::String(http_req.model.clone()));
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
          cformat!("<red>ERROR: {provider} doesn't support a JSON mode</red>",)
        );
        std::process::exit(1);
      }
    }
  }

  if opts.json_schema.is_some() {
    match http_req.provider {
      Provider::OpenAI | Provider::Groq | Provider::Ollama => {
        let mut json_schema = Map::new();
        json_schema.insert("type".to_string(), "json_schema".into());
        json_schema.insert(
          "json_schema".to_string(),
          opts.json_schema.clone().unwrap(), //
        );

        map.insert("response_format".to_string(), Value::Object(json_schema));
      }
      provider => {
        eprintln!(
          "{}",
          cformat!(
            "<red>ERROR: {provider} doesn't support a JSON schema mode</red>",
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
}

async fn exec_request(
  http_req: &AiRequest,
  req_body_obj: &Value,
) -> Result<Response, reqwest::Error> {
  let client = reqwest::Client::new();
  let req_base = client.post(http_req.url.clone()).json(&req_body_obj);
  let req = match http_req.provider {
    Provider::Anthropic => req_base
      .header("anthropic-version", "2023-06-01")
      .header("x-api-key", &http_req.api_key),
    _ => req_base.bearer_auth(&http_req.api_key),
  };
  req.send().await
}

pub async fn exec_tool(
  optional_model: &Option<&Model>,
  opts: &ExecOptions,
  user_input: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let start = Instant::now();
  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;
  let (used_model, http_req) =
    get_http_req(optional_model, &secrets_path_str, &full_config)?;

  // This is checked here, so that the missing API key message comes first
  if user_input.is_empty() {
    Err("No prompt was provided")?;
  }

  let req_body_obj =
    get_req_body_obj(&opts, &http_req, &user_input.to_string());

  let resp = exec_request(&http_req, &req_body_obj).await?;
  let elapsed_time: String = start.elapsed().as_millis().to_string();

  if !&resp.status().is_success() {
    let resp_json = resp.json::<Value>().await?;
    let resp_formatted = serde_json::to_string_pretty(&resp_json).unwrap();
    Err(cformat!(
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

pub async fn generate_changelog(
  opts: &ExecOptions,
  commit_hash: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let output = std::process::Command::new("git")
    .args(&[
      "log",
      "--date=short",
      "--pretty=format:%cd - %s%d", // date - subject (refs)
      &format!("{}..HEAD", commit_hash),
    ])
    .output()
    .expect("Failed to execute git command");

  let changelog = String::from_utf8_lossy(&output.stdout);

  let prompt = format!(
    "Summarize the following git commit log into a concise markdown changelog.\n
    Only include user-facing changes (i.e. no code refactorings or similar).\n
    Use the tags to group the changes, and if there are no tags use the dates.\n
    Include the date and the tag in the header.\n
    Don't sub-categorize the changes, just list them.\n
    Insert a blank line after each header and sub-header.\n
    \n\n{changelog}"
  );

  let model = Model::Model(Provider::OpenAI, "gpt-4o".to_string());

  exec_tool(&Some(&model), &opts, &prompt).await
}

#[derive(Deserialize)]
pub struct FileAnalysis {
  pub description: String,
  pub timestamp: Option<String>,
}

pub async fn analyze_file_content(
  opts: &ExecOptions,
  file_path: &str,
) -> Result<FileAnalysis, Box<dyn Error + Send + Sync>> {
  let content = if file_path.to_lowercase().ends_with(".pdf") {
    pdf_extract::extract_text(file_path)
      .map_err(|e| format!("Failed to extract PDF text: {}", e))?
  } else {
    std::fs::read_to_string(file_path)?
  };

  let prompt = format!(
    "Analyze following file content and return a file analysis JSON object:\n\
    \n\
    {content}\n",
  );
  let mut opts = opts.clone();

  opts.json_schema = Some(json!({
    "name": "file_analysis",
    "strict": true,
    "schema": {
      "type": "object",
      "properties": {
        "description": {
          "type": "string",
          "description":
            "A short (1-4 words) description that captures its main purpose",
        },
        "timestamp": {
          "type": "string",
          "description": "Any timestamp/date found in the content. \
            If it includes only a date use the `YYYY-MM-DD` format. \
            If it includes date and time use the `YYYY-MM-DDThh:mmZ` format. \
            Note that in German dates are usually written as `DD.MM.YYYY`.",
        }
      },
      "required": [ "description", "timestamp" ],
      "additionalProperties": false,
    },
  }));
  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;
  let (_used_model, http_req) = get_http_req(
    &Some(&Model::Model(Provider::OpenAI, "gpt-4o-mini".to_string())),
    &secrets_path_str,
    &full_config,
  )?;
  let req_body_obj = get_req_body_obj(&opts, &http_req, &prompt);
  let resp = exec_request(&http_req, &req_body_obj).await?;

  if resp.status().is_success() {
    let ai_response = resp.json::<AiResponse>().await?;
    let content = ai_response.choices[0].message.content.clone();
    let analysis: FileAnalysis =
      serde_json::from_str(&content).map_err(|e| {
        format!(
          "Failed to parse LLM response as JSON\n
            Response: {content}\n
            Error: {e}\n",
        )
      })?;
    Ok(analysis)
  } else {
    let json_val = resp.json::<Value>().await?;
    let json_str = serde_json::to_string_pretty(&json_val).unwrap();
    Err(json_str.into())
  }
}

pub async fn extract_text_from_file(
  opts: &ExecOptions,
  file_path: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let file_content = std::fs::read(file_path)?;
  let base64_content =
    base64::engine::general_purpose::STANDARD.encode(&file_content);
  let model_id = "gpt-4o";
  let model = &Model::Model(Provider::OpenAI, model_id.to_string());
  let prompt = json!({
    "model": format!("{model_id}"),
    "max_tokens": default_req_for_model(model).max_tokens,
    "messages": [{
      "role": "user",
      "content": [
        {
          "type": "text",
          "text": "Extract and return all text from this image."
        },
        {
          "type": "image_url",
          "image_url": {
            "url": format!("data:image/jpeg;base64,{base64_content}")
          }
        }
      ]
    }]
  })
  .to_string();

  exec_tool(&Some(model), &opts, &prompt).await
}

pub async fn prompt_with_lang_cntxt(
  opts: &ExecOptions,
  prog_lang: &str,
  prompt: Vec<String>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let system_prompt = format!(
    "You're a professional {prog_lang} developer.\n
    Answer the following question in the context of {prog_lang}.\n
    Keep your answer concise and to the point.\n"
  );

  let model = Model::Model(
    Provider::Anthropic,
    "claude-3-5-sonnet-latest".to_string(), //
  );

  exec_tool(
    &Some(&model),
    &opts,
    &(system_prompt.to_owned() + &prompt.join(" ")), //
  )
  .await
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
        json_schema: None,
      },
      &prompt,
    )
    .await;
    assert!(result.is_err());
  }
}
