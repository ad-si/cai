mod highlight;
mod types;

use base64::Engine;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::str;
use std::time::Instant;

use color_print::{cformat, cprintln};
use config::Config;
use reqwest::Response;
use serde_derive::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
pub use types::Commands;
use xdg::BaseDirectories;

#[derive(Serialize, Debug, PartialEq, Default, Clone)]
pub struct ExecOptions {
  pub is_raw: bool, // Raw output mode (no metadata and no syntax highlighting)
  pub is_json: bool, // JSON output mode
  pub json_schema: Option<Value>, // JSON schema of expected output
  pub subcommand: Option<Commands>, // Optional subcommand that was executed
}

#[derive(Serialize, Debug, PartialEq, Default, Clone, Copy)]
pub enum Provider {
  #[default]
  Anthropic,
  Cerebras,
  DeepSeek,
  Google,
  Groq,
  OpenAI,
  Llamafile,
  Ollama,
  XAI,
}

impl std::fmt::Display for Provider {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Provider::Anthropic => write!(f, "Anthropic"),
      Provider::Cerebras => write!(f, "Cerebras"),
      Provider::DeepSeek => write!(f, "DeepSeek"),
      Provider::Google => write!(f, "Google"),
      Provider::Groq => write!(f, "Groq"),
      Provider::Llamafile => write!(f, "Llamafile"),
      Provider::Ollama => write!(f, "Ollama"),
      Provider::OpenAI => write!(f, "OpenAI"),
      Provider::XAI => write!(f, "xAI"),
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
          write!(f, "{provider}")
        } else {
          write!(f, "{provider} {model_id}")
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
    Provider::Anthropic => AiRequest {
      provider: *provider,
      url: "https://api.anthropic.com/v1/messages".to_string(),
      model: types::get_anthropic_model(model_id).to_string(),
      ..Default::default()
    },
    Provider::Cerebras => AiRequest {
      provider: *provider,
      url: "https://api.cerebras.ai/v1/chat/completions".to_string(),
      model: types::get_cerebras_model(model_id).to_string(),
      ..Default::default()
    },
    Provider::DeepSeek => AiRequest {
      provider: *provider,
      url: "https://api.deepseek.com/chat/completions".to_string(),
      model: types::get_cerebras_model(model_id).to_string(),
      ..Default::default()
    },
    Provider::Google => AiRequest {
      provider: *provider,
      url: "https://generativelanguage.googleapis.com/v1beta/models"
        .to_string(),
      model: types::get_google_model(model_id).to_string(),
      ..Default::default()
    },
    Provider::Groq => AiRequest {
      provider: *provider,
      url: "https://api.groq.com/openai/v1/chat/completions".to_string(),
      model: types::get_groq_model(model_id).to_string(),
      ..Default::default()
    },
    Provider::Llamafile => AiRequest {
      provider: *provider,
      url: "http://localhost:8080/v1/chat/completions".to_string(),
      ..Default::default()
    },
    Provider::Ollama => AiRequest {
      provider: *provider,
      url: "http://localhost:11434/v1/chat/completions".to_string(),
      model: types::get_ollama_model(model_id).to_string(),
      ..Default::default()
    },
    Provider::OpenAI => {
      let resolved_model = types::get_openai_model(model_id);
      let url = if resolved_model.contains("-tts") {
        "https://api.openai.com/v1/audio/speech".to_string()
      } else if resolved_model == "gpt-image-1" {
        "https://api.openai.com/v1/responses".to_string()
      } else if resolved_model.starts_with("dall") {
        "https://api.openai.com/v1/images/generations".to_string()
      } else {
        "https://api.openai.com/v1/chat/completions".to_string()
      };
      AiRequest {
        provider: *provider,
        url,
        model: resolved_model.to_string(),
        ..Default::default()
      }
    }
    Provider::XAI => AiRequest {
      provider: *provider,
      url: "https://api.x.ai/v1/chat/completions".to_string(),
      model: types::get_xai_model(model_id).to_string(),
      ..Default::default()
    },
  }
}

fn get_key_setup_msg(secrets_path_str: &str) -> String {
  format!(
    "An API key must be provided. Use one of the following options:\n\
        \n\
        1. Set one or more API keys in {secrets_path_str}\n\
           (`anthropic_api_key`, `google_api_key`, `groq_api_key`, `openai_api_key`)\n\
        2. Set one or more cai specific env variables\n\
            (CAI_ANTHROPIC_API_KEY, CAI_GOOGLE_API_KEY, CAI_GROQ_API_KEY, CAI_OPENAI_API_KEY)\n\
        3. Set one or more generic env variables\n\
            (ANTHROPIC_API_KEY, GOOGLE_API_KEY, GROQ_API_KEY, OPENAI_API_KEY)\n\
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
      Provider::Anthropic => full_config.get("anthropic_api_key"),
      Provider::Cerebras => full_config.get("cerebras_api_key"),
      Provider::DeepSeek => full_config.get("deepseek_api_key"),
      Provider::Google => full_config.get("google_api_key"),
      Provider::Groq => full_config.get("groq_api_key"),
      Provider::Llamafile => Some(&dummy_key),
      Provider::Ollama => Some(&dummy_key),
      Provider::OpenAI => full_config.get("openai_api_key"),
      Provider::XAI => full_config.get("xai_api_key"),
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
      Provider::Anthropic => types::get_anthropic_model(model_id),
      Provider::Cerebras => types::get_cerebras_model(model_id),
      Provider::DeepSeek => types::get_deepseek_model(model_id),
      Provider::Google => types::get_google_model(model_id),
      Provider::Groq => types::get_groq_model(model_id),
      Provider::Llamafile => model_id,
      Provider::Ollama => types::get_ollama_model(model_id),
      Provider::OpenAI => types::get_openai_model(model_id),
      Provider::XAI => types::get_xai_model(model_id),
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
      "google_api_key",
      env::var("GOOGLE_API_KEY").unwrap_or_default(),
    )?
    .set_default(
      "groq_api_key", //
      env::var("GROQ_API_KEY").unwrap_or_default(),
    )?
    .add_source(config::File::with_name(secrets_path_str))
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
      let used_model = get_used_model(model);
      get_api_request(full_config, secrets_path_str, model)
        .map(|req| (used_model, req))
    }
    // Use the first provider that has an API key
    None => {
      let req =
        get_api_request(full_config, secrets_path_str, &Default::default())
          .or(get_api_request(
            full_config,
            secrets_path_str,
            &Model::Model(Provider::Groq, "llama-3.1-8b-instant".to_owned()),
          ))
          .or(get_api_request(
            full_config,
            secrets_path_str,
            &Model::Model(Provider::OpenAI, "gpt-4o-mini".to_string()),
          ))
          .or(get_api_request(
            full_config,
            secrets_path_str,
            &Model::Model(
              Provider::Anthropic,
              "claude-3-7-sonnet-latest".to_string(),
            ),
          ))?;
      let used_model = get_used_model(
        &Model::Model(req.provider, req.model.clone()), //
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
  if let Ok(json) = serde_json::from_str(user_input) {
    return json;
  }

  // Special handling for Google's Gemini API
  if http_req.provider == Provider::Google {
    let mut contents = Map::new();
    contents.insert("role".to_string(), "user".into());
    contents.insert(
      "parts".to_string(),
      Value::Array(vec![Value::Object(Map::from_iter([(
        "text".to_string(),
        Value::String(user_input.to_string()),
      )]))]),
    );

    let mut generation_config = Map::new();
    generation_config.insert(
      "maxOutputTokens".to_string(),
      Value::Number(http_req.max_tokens.into()),
    );

    let mut map = Map::new();
    map.insert(
      "contents".to_string(),
      Value::Array(vec![Value::Object(contents)]),
    );
    map.insert(
      "generationConfig".to_string(),
      Value::Object(generation_config),
    );

    return Value::Object(map);
  }

  // Special handling for OpenAI TTS models
  if http_req.provider == Provider::OpenAI && http_req.model.contains("-tts") {
    let mut map = Map::new();
    map.insert("model".to_string(), Value::String(http_req.model.clone()));
    map.insert("input".to_string(), Value::String(user_input.to_string()));
    map.insert("voice".to_string(), Value::String("alloy".to_string()));
    return Value::Object(map);
  }

  // Special handling for OpenAI image generation (detect when "image" alias was used)
  let is_image_generation = matches!(&opts.subcommand, Some(Commands::Openai { model, .. }) if model == "image")
    || matches!(&opts.subcommand, Some(Commands::Image { .. }));

  if http_req.provider == Provider::OpenAI
    && (is_image_generation || http_req.model == "gpt-image-1")
  {
    let mut map = Map::new();
    map.insert("model".to_string(), Value::String(http_req.model.clone()));
    map.insert("input".to_string(), Value::String(user_input.to_string()));

    let mut tools = Vec::new();
    let mut tool = Map::new();
    tool.insert(
      "type".to_string(),
      Value::String("image_generation".to_string()),
    );
    tools.push(Value::Object(tool));

    map.insert("tools".to_string(), Value::Array(tools));

    return Value::Object(map);
  }

  // Special handling for OpenAI DALL-E models (use images API)
  if http_req.provider == Provider::OpenAI
    && http_req.model.starts_with("dall-e")
  {
    let mut map = Map::new();
    map.insert("model".to_string(), Value::String(http_req.model.clone()));
    map.insert("prompt".to_string(), Value::String(user_input.to_string()));
    map.insert("n".to_string(), Value::Number(1.into()));
    map.insert("size".to_string(), Value::String("1024x1024".to_string()));

    return Value::Object(map);
  }

  // For all other providers
  let mut map = Map::new();
  map.insert("model".to_string(), Value::String(http_req.model.clone()));
  // OpenAI o1, o3, o4, and gpt-5 models require max_completion_tokens instead of max_tokens
  if http_req.provider == Provider::OpenAI
    && (http_req.model.starts_with("o1")
      || http_req.model.starts_with("o3")
      || http_req.model.starts_with("o4")
      || http_req.model.starts_with("gpt-5"))
  {
    map.insert(
      "max_completion_tokens".to_string(),
      Value::Number(http_req.max_tokens.into()),
    );
  } else {
    map.insert(
      "max_tokens".to_string(),
      Value::Number(http_req.max_tokens.into()),
    );
  }

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
      Provider::OpenAI | Provider::Ollama => {
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
    Provider::Google => {
      // For Google's Gemini API we need to append the model name and ":generateContent"
      // to the URL along with the API key as a query parameter
      let model = &http_req.model;
      let url = format!(
        "{}/{model}:generateContent?key={}",
        http_req.url, http_req.api_key
      );
      client.post(url).json(&req_body_obj)
    }
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
  let (used_model, mut http_req) =
    get_http_req(optional_model, &secrets_path_str, &full_config)?;

  // Check if this is an image generation request and update URL accordingly
  let is_image_generation = matches!(&opts.subcommand, Some(Commands::Openai { model, .. }) if model == "image")
    || matches!(&opts.subcommand, Some(Commands::Image { .. }));

  if http_req.provider == Provider::OpenAI && is_image_generation {
    http_req.url = "https://api.openai.com/v1/responses".to_string();
  }

  // This is checked here, so that the missing API key message comes first
  if user_input.is_empty() {
    Err("No prompt was provided")?;
  }

  let req_body_obj = get_req_body_obj(opts, &http_req, user_input);

  let resp = exec_request(&http_req, &req_body_obj).await?;
  let elapsed_time: String = start.elapsed().as_millis().to_string();
  let subcommand = opts
    .subcommand
    .as_ref()
    .and_then(|x| x.to_string_pretty())
    .map(|subcom| format!("| ➡️ {subcom}"))
    .unwrap_or_default();

  if !&resp.status().is_success() {
    let resp_json = resp.json::<Value>().await?;
    let resp_formatted = serde_json::to_string_pretty(&resp_json).unwrap();
    Err(cformat!(
      "<bold>⏱️ {: >5} ms</bold> | {used_model} {subcommand}\n\
      \n{resp_formatted}",
      elapsed_time,
    ))?;
  } else {
    // Special handling for OpenAI TTS models - they return audio data
    if http_req.provider == Provider::OpenAI && http_req.model.contains("-tts")
    {
      let audio_data = resp.bytes().await?;

      // Find a unique filename
      let mut filename = "output.mp3".to_string();
      let mut counter = 1;
      while std::path::Path::new(&filename).exists() {
        filename = format!("output_{counter}.mp3");
        counter += 1;
      }

      std::fs::write(&filename, &audio_data)?;

      cprintln!(
        "<bold>⏱️{: >5} ms</bold> | {used_model} {subcommand}\n",
        elapsed_time,
      );
      println!("Audio generated and saved to: {filename}");
      return Ok(());
    }

    // Check if this is an image generation request
    let is_image_generation = matches!(&opts.subcommand, Some(Commands::Openai { model, .. }) if model == "image")
      || matches!(&opts.subcommand, Some(Commands::Image { .. }));

    // Special handling for OpenAI image generation models
    if http_req.provider == Provider::OpenAI
      && (http_req.model.starts_with("dall-e")
        || http_req.model == "gpt-image-1"
        || is_image_generation)
    {
      let response_json = resp.json::<Value>().await?;

      cprintln!(
        "<bold>⏱️{: >5} ms</bold> | {used_model} {subcommand}\n",
        elapsed_time,
      );

      // Handle Responses API format (gpt-5, gpt-4.1, gpt-4o when used for image generation)
      if is_image_generation || http_req.model == "gpt-image-1" {
        if let Some(outputs) = response_json["output"].as_array() {
          let mut image_count = 0;
          for output in outputs {
            if let Some(output_type) = output["type"].as_str() {
              if output_type == "image_generation_call" {
                image_count += 1;
                if let Some(image_base64) = output["result"].as_str() {
                  use base64::{engine::general_purpose, Engine as _};
                  match general_purpose::STANDARD.decode(image_base64) {
                    Ok(image_bytes) => {
                      // Find the next available filename
                      let mut counter = 1;
                      let mut filename =
                        format!("generated_image_{counter}.png");
                      while std::path::Path::new(&filename).exists() {
                        counter += 1;
                        filename = format!("generated_image_{counter}.png");
                      }

                      match std::fs::write(&filename, image_bytes) {
                        Ok(_) => {
                          println!("Generated image saved to: {filename}")
                        }
                        Err(e) => {
                          println!("Failed to save image {image_count}: {e}")
                        }
                      }
                    }
                    Err(e) => println!(
                      "Failed to decode base64 for image {image_count}: {e}"
                    ),
                  }
                }
              }
            }
          }
        }
      } else {
        // Handle dall-e models (legacy format)
        if let Some(data) = response_json["data"].as_array() {
          for (i, image) in data.iter().enumerate() {
            if let Some(url) = image["url"].as_str() {
              println!("Generated image {}: {}", i + 1, url);
            }
          }
        }
      }

      return Ok(());
    }

    let msg = match http_req.provider {
      Provider::Anthropic => {
        let anth_response = resp.json::<AnthropicAiResponse>().await?;
        anth_response.content[0].text.clone()
      }
      Provider::Google => {
        // Handle Google's unique response format
        let response_text = resp.text().await?;
        let response_json: Value = serde_json::from_str(&response_text)?;

        // Extract the text from the Gemini response format
        response_json["candidates"][0]["content"]["parts"][0]["text"]
          .as_str()
          .unwrap_or_default()
          .to_string()
      }
      _ => {
        let ai_response = resp.json::<AiResponse>().await?;
        ai_response.choices[0].message.content.clone()
      }
    };

    if opts.is_raw {
      println!("{msg}");
    } else {
      cprintln!(
        "<bold>⏱️{: >5} ms</bold> | {used_model} {subcommand}\n",
        elapsed_time,
      );
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
  match exec_tool(optional_model, opts, user_input).await {
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
    .args([
      "log",
      "--date=short",
      "--pretty=format:%cd - %s%d", // date - subject (refs)
      &format!("{commit_hash}..HEAD"),
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

  exec_tool(&Some(&model), opts, &prompt).await
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
      .map_err(|e| format!("Failed to extract PDF text: {e}"))?
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
            "A short (1-4 words) description that captures its main purpose. \
            If it's a receipt or an invoice, \
            start with the name of the company or person that created it. \
            Do not use overly generic terms like \
            analysis, summary, transaction, document, etc.",
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

  exec_tool(&Some(model), opts, &prompt).await
}

pub async fn prompt_with_lang_cntxt(
  opts: &ExecOptions,
  cmd: &Commands,
  prompt: &[String],
) {
  let prog_lang = cmd.to_string_pretty().unwrap_or_default();
  let system_prompt = format!(
    "You're a professional {prog_lang} developer.\n
    Answer the following question in the context of {prog_lang}.\n
    Keep your answer concise and to the point.\n"
  );

  let model = Model::Model(
    Provider::Anthropic,
    "claude-3-7-sonnet-latest".to_string(), //
  );

  if let Err(err) = exec_tool(
    &Some(&model),
    opts,
    &(system_prompt.to_owned() + &prompt.join(" ")), //
  )
  .await
  {
    eprintln!("Error prompting with OCaml context: {err}");
    std::process::exit(1);
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
        json_schema: None,
        subcommand: None,
      },
      prompt,
    )
    .await;
    assert!(result.is_err());
  }

  #[test]
  fn test_o_models_use_max_completion_tokens() {
    let test_cases = vec![
      ("o1-pro", true),
      ("o3", true),
      ("o4-mini", true),
      ("gpt-5", true),
      ("gpt-5-mini", true),
      ("gpt-5-nano", true),
      ("gpt-4o", false),
      ("gpt-4.1", false),
    ];

    for (model, should_use_max_completion) in test_cases {
      let http_req = AiRequest {
        provider: Provider::OpenAI,
        model: model.to_string(),
        max_tokens: 100,
        ..Default::default()
      };

      let opts = ExecOptions {
        is_raw: false,
        is_json: false,
        json_schema: None,
        subcommand: None,
      };

      let body = get_req_body_obj(&opts, &http_req, "test");
      let has_max_completion = body
        .as_object()
        .unwrap()
        .contains_key("max_completion_tokens");
      let has_max_tokens = body.as_object().unwrap().contains_key("max_tokens");

      assert_eq!(
        has_max_completion, should_use_max_completion,
        "Failed for model {model}"
      );
      assert_eq!(
        has_max_tokens, !should_use_max_completion,
        "Failed for model {model}"
      );
    }
  }
}
