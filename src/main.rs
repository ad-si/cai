use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::str;

use config::Config;
use serde_derive::{Deserialize, Serialize};
use serde_json::{Map, Value};
use textwrap::termwidth;
use xdg::BaseDirectories;

#[derive(Serialize, Debug, PartialEq)]
enum Provider {
  OpenAI,
  Groq,
}

#[derive(Serialize, Debug)]
struct AiRequest {
  provider: Provider,
  url: String,
  model: String,
  prompt: String,
  max_tokens: u32,
  stop: String,
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

async fn exec_tool() -> Result<(), Box<dyn Error + Send + Sync>> {
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

  let default_request_groq = AiRequest {
    provider: Provider::Groq,
    url: "https://api.groq.com/openai/v1/chat/completions".to_string(),
    model: "mixtral-8x7b-32768".to_string(),
    prompt: "".to_string(),
    max_tokens: 64,
    stop: "Text".to_string(),
    api_key: "".to_string(),
  };

  let default_request_openai = AiRequest {
    provider: Provider::OpenAI,
    url: "https://api.openai.com/v1/chat/completions".to_string(),
    model: "gpt-4-turbo-preview".to_string(),
    prompt: "".to_string(),
    max_tokens: 64,
    stop: "Text".to_string(),
    api_key: "".to_string(),
  };

  let full_config = config //
    .try_deserialize::<HashMap<String, String>>()
    .unwrap();

  let http_req = match full_config //
    .get("groq_api_key")
    .map(|k| k.as_str())
  {
    None | Some("") => full_config
      .get("openai_api_key")
      .and_then(|api_key| {
        if api_key == "" {
          None
        } else {
          Some(AiRequest {
            api_key: api_key.to_string(),
            ..default_request_openai
          })
        }
      })
      .ok_or(format!(
        "An API key must be provided. Use one of the following options:\n\
        \n\
        1. Set `groq_api_key` or `openai_api_key` in {secrets_path_str}\n\
        2. Set the env variable CAI_GROQ_API_KEY or GROQ_API_KEY\n\
        3. Set the env variable CAI_OPENAI_API_KEY or OPENAI_API_KEY\n\
        ",
      )),
    Some(api_key) => Ok(AiRequest {
      api_key: api_key.to_string(),
      ..default_request_groq
    }),
  }?;

  let user_input = env::args() //
    .skip(1)
    .collect::<Vec<String>>()
    .join(" ");

  if user_input.is_empty() {
    eprintln!("Usage: cai <prompt>\n");
    std::process::exit(1);
  };

  if http_req.provider != Provider::Groq {
    println!("ℹ️ Using {:#?} ({})\n", http_req.provider, http_req.model);
  }

  let req_body_obj = {
    let mut map = Map::new();
    map.insert("model".to_string(), Value::String(http_req.model));
    map.insert(
      "messages".to_string(),
      Value::Array(vec![Value::Object(Map::from_iter([
        ("role".to_string(), "user".into()),
        ("content".to_string(), Value::String(user_input)),
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
    eprintln!("ERROR: {}", resp.text().await?);
    std::process::exit(1);
  }

  let res_json = &resp.json::<AiResponse>().await?;
  let msg = &res_json.choices[0].message.content;

  if termwidth() < 100 {
    println!("{}", msg);
  } else {
    let msg_lines = textwrap::wrap(msg, 100);
    for line in msg_lines {
      println!("{}", line);
    }
  }
  Ok(())
}

#[tokio::main]
async fn main() {
  // Necessary to wrap the execution function,
  // because a `main` function that returns a `Result` quotes any errors.
  match exec_tool().await {
    Ok(_) => (),
    Err(err) => {
      eprintln!("ERROR:\n{}", err);
      std::process::exit(1);
    }
  }
}
