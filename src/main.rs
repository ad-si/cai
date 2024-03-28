use serde_derive::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::env;
use std::error::Error;
use std::str;
use textwrap::termwidth;

#[derive(Serialize, Debug)]
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
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

  let user_input = env::args() //
    .skip(1)
    .collect::<Vec<String>>()
    .join(" ");

  if user_input.is_empty() {
    eprintln!("Usage: cai <prompt>\n");
    std::process::exit(1);
  };

  let http_req = match env::var("GROQ_API_KEY") {
    Ok(api_key) => Ok(AiRequest {
      api_key,
      ..default_request_groq
    }),
    Err(_) => {
      eprintln!("ℹ️ GROQ_API_KEY is not set, trying OPENAI_API_KEY …\n");
      env::var("OPENAI_API_KEY")
        .map(|api_key| AiRequest {
          api_key,
          ..default_request_openai
        })
        .map_err(|_| "GROQ_API_KEY or OPENAI_API_KEY must be set")
    }
  }?;

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
