mod highlight;
mod types;

use base64::Engine;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::str;
use std::time::Instant;

use chrono::Utc;
use color_print::{cformat, cprintln};
use config::Config;
use reqwest::Response;
use serde_derive::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
pub use types::Commands;
use xdg::BaseDirectories;

/// Filler words to exclude from generated filenames
const FILLER_WORDS: &[&str] = &[
  "a", "an", "the", "of", "in", "on", "at", "to", "for", "with", "and", "or",
  "is", "it", "that", "this", "as", "by", "from",
];

/// Generate a short name from a prompt for use in filenames
/// (lowercase, underscores, filters filler words, max 30 chars)
fn prompt_to_short_name(prompt: &str) -> String {
  prompt
    .to_lowercase()
    .chars()
    .map(|c| if c.is_alphanumeric() { c } else { '_' })
    .collect::<String>()
    .split('_')
    .filter(|s| !s.is_empty() && !FILLER_WORDS.contains(s))
    .collect::<Vec<&str>>()
    .join("_")
    .chars()
    .take(30)
    .collect()
}

/// Format elapsed time for display - show in seconds if > 10 seconds, otherwise in milliseconds
fn format_elapsed_time(elapsed_millis: u128) -> (String, &'static str) {
  if elapsed_millis > 10_000 {
    let seconds = elapsed_millis as f64 / 1000.0;
    (format!("{:.1}", seconds), "s")
  } else {
    (elapsed_millis.to_string(), "ms")
  }
}

/// Get a provider base URL from config, with fallback to default
fn get_base_url(
  full_config: &HashMap<String, String>,
  key: &str,
  default: &str,
) -> String {
  full_config
    .get(key)
    .filter(|s| !s.is_empty())
    .map(|s| s.trim_end_matches('/').to_string())
    .unwrap_or_else(|| default.to_string())
}

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
  Perplexity,
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
      Provider::Perplexity => write!(f, "Perplexity"),
    }
  }
}

#[derive(Serialize, Debug, PartialEq, Clone)]
pub enum Model {
  Model(Provider, String),
}

impl Default for Model {
  fn default() -> Model {
    Model::Model(Provider::Groq, "openai/gpt-oss-20b".to_owned())
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
struct SearchResult {
  title: String,
  url: String,
  date: Option<String>,
  last_updated: Option<String>,
}

#[derive(Deserialize, Debug)]
struct AiResponse {
  choices: Vec<AiChoice>,
  search_results: Option<Vec<SearchResult>>,
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

fn default_req_for_model(
  model: &Model,
  full_config: &HashMap<String, String>,
) -> AiRequest {
  let Model::Model(provider, model_id) = model;

  match provider {
    Provider::Anthropic => {
      let base_url = get_base_url(
        full_config,
        "anthropic_base_url",
        "https://api.anthropic.com/v1",
      );
      AiRequest {
        provider: *provider,
        url: format!("{base_url}/messages"),
        model: types::get_anthropic_model(model_id).to_string(),
        ..Default::default()
      }
    }
    Provider::Cerebras => {
      let base_url = get_base_url(
        full_config,
        "cerebras_base_url",
        "https://api.cerebras.ai/v1",
      );
      AiRequest {
        provider: *provider,
        url: format!("{base_url}/chat/completions"),
        model: types::get_cerebras_model(model_id).to_string(),
        ..Default::default()
      }
    }
    Provider::DeepSeek => {
      let base_url = get_base_url(
        full_config,
        "deepseek_base_url",
        "https://api.deepseek.com",
      );
      AiRequest {
        provider: *provider,
        url: format!("{base_url}/chat/completions"),
        model: types::get_deepseek_model(model_id).to_string(),
        ..Default::default()
      }
    }
    Provider::Google => {
      let resolved_model = types::get_google_model(model_id);
      let base_url = get_base_url(
        full_config,
        "google_base_url",
        "https://generativelanguage.googleapis.com/v1beta",
      );
      AiRequest {
        provider: *provider,
        url: format!("{base_url}/models"),
        model: resolved_model.to_string(),
        ..Default::default()
      }
    }
    Provider::Groq => {
      let base_url = get_base_url(
        full_config,
        "groq_base_url",
        "https://api.groq.com/openai/v1",
      );
      AiRequest {
        provider: *provider,
        url: format!("{base_url}/chat/completions"),
        model: types::get_groq_model(model_id).to_string(),
        ..Default::default()
      }
    }
    Provider::Llamafile => {
      let base_url = get_base_url(
        full_config,
        "llamafile_base_url",
        "http://localhost:8080/v1",
      );
      AiRequest {
        provider: *provider,
        url: format!("{base_url}/chat/completions"),
        ..Default::default()
      }
    }
    Provider::Ollama => {
      let base_url = get_base_url(
        full_config,
        "ollama_base_url",
        "http://localhost:11434/v1",
      );
      AiRequest {
        provider: *provider,
        url: format!("{base_url}/chat/completions"),
        model: types::get_ollama_model(model_id).to_string(),
        ..Default::default()
      }
    }
    Provider::OpenAI => {
      let resolved_model = types::get_openai_model(model_id);
      let base_url = get_base_url(
        full_config,
        "openai_base_url",
        "https://api.openai.com/v1",
      );
      let url = if resolved_model.contains("-tts") {
        format!("{base_url}/audio/speech")
      } else if resolved_model.starts_with("gpt-image")
        || resolved_model.starts_with("dall")
      {
        format!("{base_url}/images/generations")
      } else {
        format!("{base_url}/chat/completions")
      };
      AiRequest {
        provider: *provider,
        url,
        model: resolved_model.to_string(),
        ..Default::default()
      }
    }
    Provider::XAI => {
      let resolved_model = types::get_xai_model(model_id);
      let base_url =
        get_base_url(full_config, "xai_base_url", "https://api.x.ai/v1");
      let url = if resolved_model == "grok-2-image" {
        format!("{base_url}/images/generations")
      } else {
        format!("{base_url}/chat/completions")
      };
      AiRequest {
        provider: *provider,
        url,
        model: resolved_model.to_string(),
        ..Default::default()
      }
    }
    Provider::Perplexity => {
      let base_url = get_base_url(
        full_config,
        "perplexity_base_url",
        "https://api.perplexity.ai",
      );
      AiRequest {
        provider: *provider,
        url: format!("{base_url}/chat/completions"),
        model: types::get_perplexity_model(model_id).to_string(),
        ..Default::default()
      }
    }
  }
}

fn get_key_setup_msg(secrets_path_str: &str) -> String {
  format!(
    "An API key must be provided. Use one of the following options:\n\
        \n\
        1. Set one or more API keys in {secrets_path_str}\n\
           (`anthropic_api_key`, `google_api_key`, `groq_api_key`, `openai_api_key`, `perplexity_api_key`)\n\
        2. Set one or more cai specific env variables\n\
            (CAI_ANTHROPIC_API_KEY, CAI_GOOGLE_API_KEY, CAI_GROQ_API_KEY, CAI_OPENAI_API_KEY, CAI_PERPLEXITY_API_KEY)\n\
        3. Set one or more generic env variables\n\
            (ANTHROPIC_API_KEY, GOOGLE_API_KEY, GROQ_API_KEY, OPENAI_API_KEY, PERPLEXITY_API_KEY)\n\
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
      Provider::Perplexity => full_config.get("perplexity_api_key"),
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
    ..(default_req_for_model(model, full_config)).clone()
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
      Provider::Perplexity => types::get_perplexity_model(model_id),
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

fn get_config_path_str() -> String {
  let xdg_dirs = BaseDirectories::with_prefix("cai").unwrap();
  let config_path = xdg_dirs
    .place_config_file("config.yaml")
    .expect("Couldn't create configuration directory");
  let _ = std::fs::File::create_new(&config_path);
  config_path.to_str().unwrap().to_string()
}

pub fn get_full_config(
  secrets_path_str: &str,
) -> Result<
  HashMap<std::string::String, std::string::String>,
  config::ConfigError,
> {
  let config_path_str = get_config_path_str();
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
    .set_default(
      "perplexity_api_key", //
      env::var("PERPLEXITY_API_KEY").unwrap_or_default(),
    )?
    .add_source(config::File::with_name(secrets_path_str))
    .add_source(config::File::with_name(&config_path_str).required(false))
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
            &Model::Model(Provider::Groq, "openai/gpt-oss-20b".to_owned()),
          ))
          .or(get_api_request(
            full_config,
            secrets_path_str,
            &Model::Model(Provider::OpenAI, "gpt-5-mini".to_string()),
          ))
          .or(get_api_request(
            full_config,
            secrets_path_str,
            &Model::Model(Provider::Anthropic, "claude-haiku-4-5".to_string()),
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

    // Add image generation specific config for image models
    if http_req.model.contains("-image") {
      generation_config.insert(
        "responseModalities".to_string(),
        Value::Array(vec![Value::String("IMAGE".to_string())]),
      );
    }

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

  // Special handling for OpenAI image generation models (gpt-image and DALL-E)
  let is_image_generation = matches!(&opts.subcommand, Some(Commands::Openai { model, .. }) if model == "image")
    || matches!(&opts.subcommand, Some(Commands::Image { .. }));

  if http_req.provider == Provider::OpenAI
    && (is_image_generation
      || http_req.model.starts_with("gpt-image")
      || http_req.model.starts_with("dall-e"))
  {
    let mut map = Map::new();
    map.insert("model".to_string(), Value::String(http_req.model.clone()));
    map.insert("prompt".to_string(), Value::String(user_input.to_string()));

    return Value::Object(map);
  }

  // Special handling for xAI grok-2-image model (use images API)
  if http_req.provider == Provider::XAI && http_req.model == "grok-2-image" {
    let mut map = Map::new();
    map.insert("model".to_string(), Value::String(http_req.model.clone()));
    map.insert("prompt".to_string(), Value::String(user_input.to_string()));
    map.insert("n".to_string(), Value::Number(1.into()));

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
  let (used_model, http_req) =
    get_http_req(optional_model, &secrets_path_str, &full_config)?;

  // This is checked here, so that the missing API key message comes first
  if user_input.is_empty() {
    Err("No prompt was provided")?;
  }

  let req_body_obj = get_req_body_obj(opts, &http_req, user_input);

  let resp = exec_request(&http_req, &req_body_obj).await?;
  let elapsed_millis = start.elapsed().as_millis();
  let (elapsed_time, time_unit) = format_elapsed_time(elapsed_millis);
  let subcommand = opts
    .subcommand
    .as_ref()
    .and_then(|x| x.to_string_pretty())
    .map(|subcom| format!("➡️ {subcom} | "))
    .unwrap_or_default();

  if !&resp.status().is_success() {
    let resp_json = resp.json::<Value>().await?;
    let resp_formatted = serde_json::to_string_pretty(&resp_json).unwrap();
    Err(cformat!(
      "<bold>{subcommand}{used_model} | ⏱️ {} {}</bold>\n\
      \n{resp_formatted}",
      elapsed_time,
      time_unit,
    ))?;
  } else {
    // Special handling for OpenAI TTS models - they return audio data
    if http_req.provider == Provider::OpenAI && http_req.model.contains("-tts")
    {
      let audio_data = resp.bytes().await?;

      // Generate timestamp prefix in format: 2025-08-17t1943
      let now = Utc::now();
      let timestamp_prefix = now.format("%Y-%m-%dt%H%M").to_string();

      // Find a unique filename with timestamp prefix
      let mut filename = format!("{timestamp_prefix}_output.mp3");
      let mut counter = 1;
      while std::path::Path::new(&filename).exists() {
        filename = format!("{timestamp_prefix}_output_{counter}.mp3");
        counter += 1;
      }

      std::fs::write(&filename, &audio_data)?;

      cprintln!(
        "<bold>{subcommand}{used_model} | ⏱️ {} {}</bold>\n",
        elapsed_time,
        time_unit,
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
        || http_req.model.starts_with("gpt-image")
        || is_image_generation)
    {
      let response_json = resp.json::<Value>().await?;

      cprintln!(
        "<bold>{subcommand}{used_model} | ⏱️ {} {}</bold>\n",
        elapsed_time,
        time_unit,
      );

      // Handle Images API format with base64 (gpt-image and dall-e models)
      if let Some(data) = response_json["data"].as_array() {
        let mut image_count = 0;
        for image_data in data {
          image_count += 1;

          // Check for base64 format first
          if let Some(image_base64) = image_data["b64_json"].as_str() {
            use base64::{engine::general_purpose, Engine as _};
            match general_purpose::STANDARD.decode(image_base64) {
              Ok(image_bytes) => {
                // Generate timestamp prefix in format: 2025-08-17t1943
                let now = Utc::now();
                let timestamp_prefix = now.format("%Y-%m-%dt%H%M").to_string();

                // Extract original user prompt from subcommand if available
                // (for Photo/Image commands, user_input contains system instructions)
                let original_prompt = match &opts.subcommand {
                  Some(Commands::Photo { prompt }) => prompt.join(" "),
                  Some(Commands::Image { prompt }) => prompt.join(" "),
                  _ => user_input.to_string(),
                };

                let short_name = prompt_to_short_name(&original_prompt);

                // Find the next available filename
                let mut counter = 1;
                let mut filename =
                  format!("{timestamp_prefix}_{short_name}.png");
                while std::path::Path::new(&filename).exists() {
                  counter += 1;
                  filename =
                    format!("{timestamp_prefix}_{short_name}_{counter}.png");
                }

                match std::fs::write(&filename, image_bytes) {
                  Ok(_) => println!("Generated image saved to: {filename}"),
                  Err(e) => println!("Failed to save image {image_count}: {e}"),
                }
              }
              Err(e) => {
                println!("Failed to decode base64 for image {image_count}: {e}")
              }
            }
          }
          // Fall back to URL format if base64 not present
          else if let Some(url) = image_data["url"].as_str() {
            println!("Generated image {}: {}", image_count, url);
          }
        }
      }

      return Ok(());
    }

    // Special handling for xAI grok-2-image model
    if http_req.provider == Provider::XAI && http_req.model == "grok-2-image" {
      let response_json = resp.json::<Value>().await?;

      cprintln!(
        "<bold>{subcommand}{used_model} | ⏱️ {} {}</bold>\n",
        elapsed_time,
        time_unit,
      );

      // xAI uses a similar format to DALL-E with data array containing URLs
      if let Some(data) = response_json["data"].as_array() {
        for (i, image) in data.iter().enumerate() {
          if let Some(url) = image["url"].as_str() {
            println!("Generated image {}: {}", i + 1, url);
          }
        }
      }

      return Ok(());
    }

    // Special handling for Google Gemini image generation
    if http_req.provider == Provider::Google
      && http_req.model.contains("-image")
    {
      let response_json = resp.json::<Value>().await?;

      cprintln!(
        "<bold>{subcommand}{used_model} | ⏱️ {} {}</bold>\n",
        elapsed_time,
        time_unit,
      );

      // Google Gemini returns images in the response as inline data
      if let Some(candidates) = response_json["candidates"].as_array() {
        for candidate in candidates {
          if let Some(parts) = candidate["content"]["parts"].as_array() {
            let mut image_count = 0;
            for part in parts {
              if let Some(inline_data) = part["inlineData"].as_object() {
                if let Some(data_base64) = inline_data["data"].as_str() {
                  image_count += 1;
                  use base64::{engine::general_purpose, Engine as _};
                  match general_purpose::STANDARD.decode(data_base64) {
                    Ok(image_bytes) => {
                      // Generate timestamp prefix in format: 2025-08-17t1943
                      let now = Utc::now();
                      let timestamp_prefix =
                        now.format("%Y-%m-%dt%H%M").to_string();

                      // Extract original user prompt from subcommand if available
                      let original_prompt = match &opts.subcommand {
                        Some(Commands::GoogleImage { prompt }) => {
                          prompt.join(" ")
                        }
                        _ => user_input.to_string(),
                      };

                      let short_name = prompt_to_short_name(&original_prompt);

                      // Find a unique filename with timestamp prefix
                      let mut filename =
                        format!("{timestamp_prefix}_{short_name}.png");
                      let mut counter = 1;
                      while std::path::Path::new(&filename).exists() {
                        filename = format!(
                          "{timestamp_prefix}_{short_name}_{counter}.png"
                        );
                        counter += 1;
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
      }

      return Ok(());
    }

    let (msg, search_results) = match http_req.provider {
      Provider::Anthropic => {
        let anth_response = resp.json::<AnthropicAiResponse>().await?;
        (anth_response.content[0].text.clone(), None)
      }
      Provider::Google => {
        // Handle Google's unique response format
        let response_text = resp.text().await?;
        let response_json: Value = serde_json::from_str(&response_text)?;

        // Extract the text from the Gemini response format
        let text = response_json["candidates"][0]["content"]["parts"][0]
          ["text"]
          .as_str()
          .unwrap_or_default()
          .to_string();
        (text, None)
      }
      _ => {
        let ai_response = resp.json::<AiResponse>().await?;
        let msg = ai_response.choices[0].message.content.clone();
        let search_results = ai_response.search_results;
        (msg, search_results)
      }
    };

    if opts.is_raw {
      println!("{msg}");
    } else {
      cprintln!(
        "<bold>{subcommand}{used_model} | ⏱️ {} {}</bold>\n",
        elapsed_time,
        time_unit,
      );
      highlight::text_via_bat(&msg);

      // Display search results for Perplexity models
      if let Some(results) = search_results {
        if !results.is_empty() {
          println!("\n\n## Search Results\n");
          for (i, result) in results.iter().enumerate() {
            let index = i + 1;
            println!(
              "[{index}] {title} ({url})",
              title = result.title,
              url = result.url
            );
            if let Some(date) = &result.date {
              println!("    Date: {date}");
            }
            if let Some(last_updated) = &result.last_updated {
              println!("    Updated: {last_updated}");
            }
          }
        }
      }

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
  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;
  let prompt = json!({
    "model": format!("{model_id}"),
    "max_tokens": default_req_for_model(model, &full_config).max_tokens,
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

pub async fn google_ocr_file(
  opts: &ExecOptions,
  file_path: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let file_content = std::fs::read(file_path)?;
  let base64_content =
    base64::engine::general_purpose::STANDARD.encode(&file_content);

  // Detect MIME type based on file extension
  let mime_type = if file_path.to_lowercase().ends_with(".png") {
    "image/png"
  } else if file_path.to_lowercase().ends_with(".jpg")
    || file_path.to_lowercase().ends_with(".jpeg")
  {
    "image/jpeg"
  } else if file_path.to_lowercase().ends_with(".gif") {
    "image/gif"
  } else if file_path.to_lowercase().ends_with(".webp") {
    "image/webp"
  } else {
    "image/jpeg" // default
  };

  let model_id = "gemini-3-pro-preview";
  let model = &Model::Model(Provider::Google, model_id.to_string());

  // Build the request JSON according to the Google Gemini API format
  // The mediaResolution should be specified at the generationConfig level
  let prompt = json!({
    "contents": [{
      "parts": [
        { "text": "Extract and return all text from this image.
            Just the text and no explanation!" },
        {
          "inlineData": {
            "mimeType": mime_type,
            "data": base64_content
          }
        }
      ]
    }],
    "generationConfig": {
      "mediaResolution": "media_resolution_high"
    }
  })
  .to_string();

  exec_tool(&Some(model), opts, &prompt).await
}

pub async fn transcribe_audio_file(
  opts: &ExecOptions,
  file_path: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;
  let model = &Model::Model(Provider::OpenAI, "gpt-4o-transcribe".to_string());
  let (_used_model, http_req) =
    get_http_req(&Some(model), &secrets_path_str, &full_config)?;

  let file = std::fs::read(file_path)?;
  let part = reqwest::multipart::Part::bytes(file)
    .file_name(file_path.to_string())
    .mime_str("audio/mpeg")?;

  let form = reqwest::multipart::Form::new()
    .text("model", http_req.model.clone())
    .part("file", part);

  let client = reqwest::Client::new();
  let base_url =
    get_base_url(&full_config, "openai_base_url", "https://api.openai.com/v1");
  let transcription_url = format!("{base_url}/audio/transcriptions");
  let resp = client
    .post(&transcription_url)
    .bearer_auth(&http_req.api_key)
    .multipart(form)
    .send()
    .await?;

  if resp.status().is_success() {
    let resp_json = resp.json::<Value>().await?;
    let text = format!("{}\n", resp_json["text"].as_str().unwrap_or_default());
    if opts.is_raw {
      println!("{text}");
    } else {
      highlight::text_via_bat(&text);
    }
  } else {
    let resp_json = resp.json::<Value>().await?;
    let resp_formatted = serde_json::to_string_pretty(&resp_json).unwrap();
    Err(resp_formatted)?;
  }

  Ok(())
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

  let model = Model::Model(Provider::Anthropic, "claude-haiku-4-5".to_string());

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
pub async fn create_commits(
  opts: &ExecOptions,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  // Get status of modified files (excluding untracked files)
  let status_output = std::process::Command::new("git")
    .args(["status", "--porcelain"])
    .output()
    .expect("Failed to execute git status");

  let status = String::from_utf8_lossy(&status_output.stdout);

  // Filter for modified files only (M, MM, or AM status)
  let modified_files: Vec<String> = status
    .lines()
    .filter(|line| {
      let trimmed = line.trim();
      // Include files that are modified (M), added and modified (AM), or modified in both index and working tree (MM)
      // Exclude untracked files (??)
      trimmed.starts_with("M ")
        || trimmed.starts_with("MM")
        || trimmed.starts_with("AM")
        || trimmed.starts_with(" M")
    })
    .map(|line| line[3..].to_string()) // Skip the status prefix
    .collect();

  if modified_files.is_empty() {
    println!("No modified files to commit.");
    return Ok(());
  }

  println!("Found {} modified file(s):\n", modified_files.len());
  for file in &modified_files {
    println!("  - {}", file);
  }
  println!();

  // Get the diff for all modified files
  let diff_output = std::process::Command::new("git")
    .args(["diff", "HEAD"])
    .output()
    .expect("Failed to execute git diff");

  let diff = String::from_utf8_lossy(&diff_output.stdout);

  // Ask AI to analyze the diff and suggest commit groupings
  let analysis_prompt = format!(
    "Analyze the following git diff and determine if the changes should be split into multiple commits.\n\
    If the changes are related and form a coherent unit, suggest ONE commit.\n\
    If there are multiple unrelated changes, suggest how to group the files into separate commits.\n\
    \n\
    For each commit group, provide:\n\
    1. A list of file paths to include\n\
    2. A concise commit message (50 chars or less for the summary)\n\
    3. Optional: A longer description if needed\n\
    \n\
    Respond in JSON format:\n\
    {{\n\
      \"commits\": [\n\
        {{\n\
          \"files\": [\"path/to/file1.rs\", \"path/to/file2.rs\"],\n\
          \"message\": \"Brief summary of changes\",\n\
          \"description\": \"Optional longer description\"\n\
        }}\n\
      ]\n\
    }}\n\
    \n\
    Git diff:\n{diff}"
  );

  let model = Model::Model(Provider::OpenAI, "gpt-4o".to_string());

  // Get AI analysis of commit groupings
  let json_schema = json!({
    "name": "commit_analysis",
    "strict": true,
    "schema": {
      "type": "object",
      "properties": {
        "commits": {
          "type": "array",
          "items": {
            "type": "object",
            "properties": {
              "files": {
                "type": "array",
                "items": { "type": "string" }
              },
              "message": { "type": "string" },
              "description": { "type": "string" }
            },
            "required": ["files", "message", "description"],
            "additionalProperties": false
          }
        }
      },
      "required": ["commits"],
      "additionalProperties": false
    }
  });

  let mut analysis_opts = opts.clone();
  analysis_opts.is_json = true;
  analysis_opts.json_schema = Some(json_schema);

  // Capture the JSON response
  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;
  let (_used_model, http_req) =
    get_http_req(&Some(&model), &secrets_path_str, &full_config)?;
  let req_body_obj =
    get_req_body_obj(&analysis_opts, &http_req, &analysis_prompt);
  let response = exec_request(&http_req, &req_body_obj).await?;

  // Parse the response using the standard AiResponse struct
  let ai_response = response
    .json::<AiResponse>()
    .await
    .map_err(|e| format!("Failed to decode API response: {}", e))?;

  if ai_response.choices.is_empty() {
    return Err("API returned no choices".into());
  }

  let content = &ai_response.choices[0].message.content;

  #[derive(Deserialize)]
  struct CommitGroup {
    files: Vec<String>,
    message: String,
    description: Option<String>,
  }

  #[derive(Deserialize)]
  struct CommitAnalysis {
    commits: Vec<CommitGroup>,
  }

  let analysis: CommitAnalysis =
    serde_json::from_str(content).map_err(|e| {
      format!(
        "Failed to parse commit analysis: {}. Response: {}",
        e, content
      )
    })?;

  if analysis.commits.is_empty() {
    println!("No commits suggested by AI.");
    return Ok(());
  }

  println!("AI suggests {} commit(s):\n", analysis.commits.len());

  // Process each suggested commit
  for (idx, commit_group) in analysis.commits.iter().enumerate() {
    println!("─────────────────────────────────────────────");
    println!("Commit {}/{}", idx + 1, analysis.commits.len());
    println!("─────────────────────────────────────────────");
    println!("Files:");
    for file in &commit_group.files {
      println!("  - {}", file);
    }
    println!("\nCommit message:");
    println!("  {}", commit_group.message);
    if let Some(desc) = &commit_group.description {
      if !desc.is_empty() {
        println!("\n  {}", desc);
      }
    }
    println!();

    // Prompt user for approval
    print!("Proceed with this commit? [Y/n]: ");
    std::io::Write::flush(&mut std::io::stdout())?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input == "n" || input == "no" {
      println!("Skipped.\n");
      continue;
    }

    // Stage the files
    for file in &commit_group.files {
      let add_output = std::process::Command::new("git")
        .args(["add", file])
        .output()?;

      if !add_output.status.success() {
        eprintln!("Warning: Failed to stage {}", file);
      }
    }

    // Create the commit
    let full_message = if let Some(desc) = &commit_group.description {
      if !desc.is_empty() {
        format!("{}\n\n{}", commit_group.message, desc)
      } else {
        commit_group.message.clone()
      }
    } else {
      commit_group.message.clone()
    };

    let commit_output = std::process::Command::new("git")
      .args(["commit", "-m", &full_message])
      .output()?;

    if commit_output.status.success() {
      println!("✓ Commit created successfully.\n");
    } else {
      let error = String::from_utf8_lossy(&commit_output.stderr);
      eprintln!("✗ Failed to create commit: {}\n", error);
    }
  }

  println!("─────────────────────────────────────────────");
  println!("All commits processed.");

  Ok(())
}

pub async fn query_database(
  opts: &ExecOptions,
  database_path: &str,
  prompt_text: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  use rusqlite::Connection;

  // Open the database
  let conn = Connection::open(database_path).map_err(|e| {
    format!("Failed to open database '{}': {}", database_path, e)
  })?;

  // Get the database schema
  let mut schema_parts: Vec<String> = Vec::new();

  // Get all table names
  let mut stmt = conn
    .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
    .map_err(|e| format!("Failed to query schema: {}", e))?;

  let table_names: Vec<String> = stmt
    .query_map([], |row| row.get(0))
    .map_err(|e| format!("Failed to get table names: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

  // Get CREATE TABLE statement for each table
  for table_name in &table_names {
    let sql: String = conn
      .query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
        [table_name],
        |row| row.get(0),
      )
      .map_err(|e| {
        format!("Failed to get schema for table '{}': {}", table_name, e)
      })?;
    schema_parts.push(sql);
  }

  let schema = schema_parts.join("\n\n");

  // Build the prompt for the LLM to generate SQL
  let sql_prompt = format!(
    "You are a SQLite expert. Given the following database schema, \
    generate a SQL query to answer the user's question.\n\n\
    IMPORTANT: Respond with ONLY the SQL query, no explanations, \
    no markdown code blocks, no comments. Just the raw SQL.\n\n\
    Schema:\n{schema}\n\n\
    Question: {prompt_text}"
  );

  // Use OpenAI to generate the SQL query
  let model = Model::Model(Provider::OpenAI, "gpt-4.1".to_string());
  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;
  let (_used_model, http_req) =
    get_http_req(&Some(&model), &secrets_path_str, &full_config)?;

  let mut raw_opts = opts.clone();
  raw_opts.is_raw = true;

  let req_body_obj = get_req_body_obj(&raw_opts, &http_req, &sql_prompt);
  let resp = exec_request(&http_req, &req_body_obj).await?;

  if !resp.status().is_success() {
    let resp_json = resp.json::<Value>().await?;
    let resp_formatted = serde_json::to_string_pretty(&resp_json).unwrap();
    return Err(format!("Failed to generate SQL: {}", resp_formatted).into());
  }

  let ai_response = resp.json::<AiResponse>().await?;
  let generated_sql = ai_response.choices[0]
    .message
    .content
    .trim()
    .trim_start_matches("```sql")
    .trim_start_matches("```")
    .trim_end_matches("```")
    .trim()
    .to_string();

  if !opts.is_raw {
    cprintln!("<bold>Generated SQL:</bold>");
    println!("{}\n", generated_sql);
  }

  // Execute the generated SQL query
  let mut stmt = conn
    .prepare(&generated_sql)
    .map_err(|e| format!("SQL error: {}", e))?;

  let column_count = stmt.column_count();
  let column_names: Vec<String> =
    stmt.column_names().iter().map(|s| s.to_string()).collect();

  // Execute and collect results
  let rows_result = stmt.query_map([], |row| {
    let mut row_values: Vec<String> = Vec::new();
    for i in 0..column_count {
      let value: rusqlite::types::Value = row.get(i)?;
      let str_value = match value {
        rusqlite::types::Value::Null => "NULL".to_string(),
        rusqlite::types::Value::Integer(i) => i.to_string(),
        rusqlite::types::Value::Real(f) => f.to_string(),
        rusqlite::types::Value::Text(s) => s,
        rusqlite::types::Value::Blob(b) => format!("<blob {} bytes>", b.len()),
      };
      row_values.push(str_value);
    }
    Ok(row_values)
  });

  let rows: Vec<Vec<String>> = rows_result
    .map_err(|e| format!("Query execution error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

  if !opts.is_raw {
    cprintln!("<bold>Results ({} rows):</bold>", rows.len());
  }

  // Calculate column widths for pretty printing
  let mut col_widths: Vec<usize> =
    column_names.iter().map(|n| n.len()).collect();
  for row in &rows {
    for (i, val) in row.iter().enumerate() {
      if val.len() > col_widths[i] {
        col_widths[i] = val.len();
      }
    }
  }

  // Print header
  let header: Vec<String> = column_names
    .iter()
    .enumerate()
    .map(|(i, name)| format!("{:width$}", name, width = col_widths[i]))
    .collect();
  println!("{}", header.join(" | "));

  // Print separator
  let separator: Vec<String> =
    col_widths.iter().map(|w| "-".repeat(*w)).collect();
  println!("{}", separator.join("-+-"));

  // Print rows
  for row in &rows {
    let formatted: Vec<String> = row
      .iter()
      .enumerate()
      .map(|(i, val)| format!("{:width$}", val, width = col_widths[i]))
      .collect();
    println!("{}", formatted.join(" | "));
  }

  if !opts.is_raw {
    println!();
  }

  Ok(())
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

  #[test]
  fn test_format_elapsed_time() {
    // Test milliseconds (≤ 10 seconds)
    assert_eq!(format_elapsed_time(0), ("0".to_string(), "ms"));
    assert_eq!(format_elapsed_time(1), ("1".to_string(), "ms"));
    assert_eq!(format_elapsed_time(999), ("999".to_string(), "ms"));
    assert_eq!(format_elapsed_time(5000), ("5000".to_string(), "ms"));
    assert_eq!(format_elapsed_time(10000), ("10000".to_string(), "ms"));
    assert_eq!(format_elapsed_time(9999), ("9999".to_string(), "ms"));

    // Test seconds (> 10 seconds)
    assert_eq!(format_elapsed_time(10001), ("10.0".to_string(), "s"));
    assert_eq!(format_elapsed_time(15000), ("15.0".to_string(), "s"));
    assert_eq!(format_elapsed_time(15500), ("15.5".to_string(), "s"));
    assert_eq!(format_elapsed_time(15999), ("16.0".to_string(), "s"));
    assert_eq!(format_elapsed_time(60000), ("60.0".to_string(), "s"));
    assert_eq!(format_elapsed_time(60123), ("60.1".to_string(), "s"));
    assert_eq!(format_elapsed_time(65432), ("65.4".to_string(), "s"));
    assert_eq!(format_elapsed_time(120000), ("120.0".to_string(), "s"));
  }
}
