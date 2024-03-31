use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::str;

use config::Config;
use serde_derive::{Deserialize, Serialize};
use serde_json::{Map, Value};
use textwrap::termwidth;
use xdg::BaseDirectories;

#[derive(Serialize, Debug, PartialEq, Default, Clone, Copy)]
pub enum Provider {
  #[default]
  Groq,
  OpenAI,
  Local,
}

#[derive(Serialize, Debug, Default, Clone)]
struct AiRequest {
  provider: Provider,
  url: String,
  model: String,
  prompt: String,
  api_key: String,
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

fn default_req_for_provider(provider: &Provider) -> AiRequest {
  match provider {
    Provider::Groq => AiRequest {
      provider: Provider::Groq,
      url: "https://api.groq.com/openai/v1/chat/completions".to_string(),
      model: "mixtral-8x7b-32768".to_string(),
      ..Default::default()
    },
    Provider::OpenAI => AiRequest {
      provider: Provider::OpenAI,
      url: "https://api.openai.com/v1/chat/completions".to_string(),
      model: "gpt-4-turbo-preview".to_string(),
      ..Default::default()
    },
    Provider::Local => AiRequest {
      provider: Provider::Local,
      url: "http://localhost:8080/v1/chat/completions".to_string(),
      ..Default::default()
    },
  }
}

fn get_key_setup_msg(secrets_path_str: &str) -> String {
  format!(
    "An API key must be provided. Use one of the following options:\n\
        \n\
        1. Set `groq_api_key` or `openai_api_key` in {secrets_path_str}\n\
        2. Set the env variable CAI_GROQ_API_KEY or GROQ_API_KEY\n\
        3. Set the env variable CAI_OPENAI_API_KEY or OPENAI_API_KEY\n\
        ",
  )
}

fn get_api_request(
  full_config: &HashMap<String, String>,
  secrets_path_str: &str,
  provider: &Provider,
) -> Result<AiRequest, String> {
  let dummy_key = "DUMMY_KEY".to_string();

  {
    match provider {
      Provider::Groq => full_config.get("groq_api_key"),
      Provider::OpenAI => full_config.get("openai_api_key"),
      Provider::Local => Some(&dummy_key),
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
    ..(default_req_for_provider(provider)).clone()
  })
}

async fn exec_tool(
  optional_provider: &Option<Provider>,
  user_input: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let xdg_dirs = BaseDirectories::with_prefix("cai").unwrap();
  let secrets_path = xdg_dirs
    .place_config_file("secrets.yaml")
    .expect("Couldn't create configuration directory");

  let _ = std::fs::File::create_new(&secrets_path);

  let secrets_path_str = secrets_path.to_str().unwrap();

  let config = Config::builder()
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

  let http_req = match optional_provider {
    Some(Provider::OpenAI) => {
      get_api_request(&full_config, secrets_path_str, &Provider::OpenAI)?
    }
    Some(Provider::Local) => {
      get_api_request(&full_config, secrets_path_str, &Provider::Local)?
    }
    _ => get_api_request(&full_config, secrets_path_str, &Provider::Groq)
      .unwrap_or(get_api_request(
        &full_config,
        secrets_path_str,
        &Provider::OpenAI,
      )?),
  };

  // This is checked here, so that the missing API key message comes first
  if user_input.is_empty() {
    Err("No prompt was provided")?;
  }

  if http_req.provider != Provider::Groq {
    println!(
      "ℹ️ Using {:#?} {}\n",
      http_req.provider,
      if http_req.model != "" {
        format!("({})", http_req.model)
      } else {
        "".to_string()
      }
    )
  }

  let req_body_obj = {
    let mut map = Map::new();
    map.insert("model".to_string(), Value::String(http_req.model));
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
  let req = client
    .post(http_req.url.clone())
    .json(&req_body_obj)
    .bearer_auth(http_req.api_key);

  let resp = req.send().await?;

  if !&resp.status().is_success() {
    Err(resp.text().await?)?;
  } else {
    let res_json = &resp.json::<AiResponse>().await?;
    let msg = &res_json.choices[0].message.content;

    if termwidth() < 100 {
      println!("{}", msg);
    } else {
      let msg_lines = textwrap::wrap(msg, 60);
      for line in msg_lines {
        println!("{}", line);
      }
    }
  }
  Ok(())
}

#[tokio::main]
pub async fn submit_prompt(
  optional_provider: &Option<Provider>,
  user_input: &str,
) {
  // Necessary to wrap the execution function,
  // because a `main` function that returns a `Result` quotes any errors.
  match exec_tool(optional_provider, user_input).await {
    Ok(_) => (),
    Err(err) => {
      eprintln!("ERROR:\n{}", err);
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
    let result = exec_tool(&Some(Provider::OpenAI), &prompt).await;
    assert!(result.is_err());
  }
}
