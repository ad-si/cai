pub mod agent;
mod cli_backend;
mod highlight;
mod types;
mod typesafe;

use base64::Engine;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::io::Write;
use std::str;
use std::time::Instant;

use chrono::Utc;
use cli_backend::{Backend, CliError, Image};
use color_print::{cformat, cprintln};
use config::Config;
use futures::future::join_all;
use futures::StreamExt;
use reqwest::Response;
use serde_derive::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
pub use types::is_deepseek_model;
pub use types::Commands;
pub use typesafe::{
  ask_jev, ask_jev_many, JevQuestion, JevQuestions, DEFAULT_JEV_MODEL,
};
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

/// Detect an image's file extension from its magic bytes
/// (defaults to `png`, which is what OpenAI and Google return)
fn image_extension(image_bytes: &[u8]) -> &'static str {
  match image_bytes {
    [0xff, 0xd8, 0xff, ..] => "jpg",
    [b'G', b'I', b'F', b'8', ..] => "gif",
    [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "webp",
    _ => "png",
  }
}

/// Write media bytes to a uniquely named file
/// derived from the current timestamp and the prompt.
/// Returns the name of the written file.
fn save_media_bytes(
  bytes: &[u8],
  prompt: &str,
  extension: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
  // Generate timestamp prefix in format: 2025-08-17t1943
  let timestamp_prefix = Utc::now().format("%Y-%m-%dt%H%M").to_string();
  let short_name = prompt_to_short_name(prompt);

  // Find the next available filename
  let mut counter = 1;
  let mut filename = format!("{timestamp_prefix}_{short_name}.{extension}");
  while std::path::Path::new(&filename).exists() {
    counter += 1;
    filename = format!("{timestamp_prefix}_{short_name}_{counter}.{extension}");
  }

  std::fs::write(&filename, bytes)?;

  Ok(filename)
}

/// Decode a base64 encoded image and write it to a uniquely named file
/// derived from the current timestamp and the prompt.
/// Returns the name of the written file.
fn save_base64_image(
  image_base64: &str,
  prompt: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
  use base64::{engine::general_purpose, Engine as _};
  let image_bytes = general_purpose::STANDARD.decode(image_base64)?;
  let extension = image_extension(&image_bytes);

  save_media_bytes(&image_bytes, prompt, extension)
}

/// File extension for a media MIME type as returned by Gemini's `inlineData`
/// (e.g. `audio/mpeg` → `mp3`). Raw PCM (`audio/L16`) is converted to WAV
/// before being written, hence the `wav` extension.
fn media_extension_for_mime(mime_type: &str) -> &'static str {
  match mime_type.split(';').next().unwrap_or_default().trim() {
    "image/jpeg" => "jpg",
    "image/gif" => "gif",
    "image/webp" => "webp",
    "audio/mpeg" | "audio/mp3" => "mp3",
    "audio/ogg" => "ogg",
    "audio/flac" => "flac",
    "audio/wav" | "audio/x-wav" => "wav",
    mime if mime.eq_ignore_ascii_case("audio/l16") => "wav",
    "video/mp4" => "mp4",
    _ => "png",
  }
}

/// Read a numeric MIME parameter, e.g. `24000` from
/// `audio/L16; rate=24000; channels=1`.
fn mime_param(mime_type: &str, key: &str) -> Option<u32> {
  mime_type
    .split(';')
    .skip(1)
    .filter_map(|param| param.split_once('='))
    .find(|(name, _)| name.trim().eq_ignore_ascii_case(key))
    .and_then(|(_, value)| value.trim().parse().ok())
}

/// Wrap raw little-endian 16 bit PCM samples in a RIFF/WAVE container,
/// so that the audio Gemini's TTS models return is playable as a file.
fn pcm_to_wav(pcm: &[u8], sample_rate: u32, channels: u16) -> Vec<u8> {
  let bits_per_sample: u16 = 16;
  let block_align = channels * bits_per_sample / 8;
  let byte_rate = sample_rate * block_align as u32;

  let mut wav = Vec::with_capacity(44 + pcm.len());
  wav.extend_from_slice(b"RIFF");
  wav.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
  wav.extend_from_slice(b"WAVEfmt ");
  wav.extend_from_slice(&16u32.to_le_bytes()); // PCM header size
  wav.extend_from_slice(&1u16.to_le_bytes()); // Format: uncompressed PCM
  wav.extend_from_slice(&channels.to_le_bytes());
  wav.extend_from_slice(&sample_rate.to_le_bytes());
  wav.extend_from_slice(&byte_rate.to_le_bytes());
  wav.extend_from_slice(&block_align.to_le_bytes());
  wav.extend_from_slice(&bits_per_sample.to_le_bytes());
  wav.extend_from_slice(b"data");
  wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
  wav.extend_from_slice(pcm);
  wav
}

/// Decode one `inlineData` part of a Gemini response and write it to disk.
/// Returns the name of the written file.
fn save_gemini_inline_data(
  mime_type: &str,
  data_base64: &str,
  prompt: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
  use base64::{engine::general_purpose, Engine as _};
  let bytes = general_purpose::STANDARD.decode(data_base64)?;
  let extension = media_extension_for_mime(mime_type);

  // The TTS models return headerless PCM, which no player recognizes
  if mime_type.to_ascii_lowercase().starts_with("audio/l16") {
    let sample_rate = mime_param(mime_type, "rate").unwrap_or(24_000);
    let channels = mime_param(mime_type, "channels").unwrap_or(1) as u16;
    let wav = pcm_to_wav(&bytes, sample_rate, channels);
    return save_media_bytes(&wav, prompt, extension);
  }

  save_media_bytes(&bytes, prompt, extension)
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

#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct ExecOptions {
  pub is_raw: bool, // Raw output mode (no metadata and no syntax highlighting)
  pub is_json: bool, // JSON output mode
  pub json_schema: Option<Value>, // JSON schema of expected output
  pub subcommand: Option<Commands>, // Optional subcommand that was executed
  pub is_streaming: bool, // Stream tokens as they arrive
}

impl Default for ExecOptions {
  fn default() -> Self {
    Self {
      is_raw: false,
      is_json: false,
      json_schema: None,
      subcommand: None,
      is_streaming: true,
    }
  }
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
    Model::Model(Provider::Cerebras, "gpt-oss-120b".to_owned())
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
  backend: Backend,
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
      backend: Default::default(),
    }
  }
}

impl AiRequest {
  /// Send the request to the provider's API instead of a subscription CLI,
  /// because the CLI can't serve `feature`.
  /// Fails if no API key is configured.
  fn require_api(mut self, feature: &str) -> Result<AiRequest, String> {
    if self.backend == Backend::Api {
      return Ok(self);
    }
    if self.api_key.is_empty() {
      return Err(self.backend.unsupported_msg(self.provider, feature));
    }
    // To stderr, so that piping the answer stays unaffected.
    eprintln!(
      "{}",
      cformat!(
        "<dim>{feature} isn't supported via {}. \
        Using the {} API instead …</dim>",
        self.backend,
        self.provider,
      )
    );
    self.backend = Backend::Api;
    Ok(self)
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
      let url = if is_xai_image_model(resolved_model) {
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
  let backend = cli_backend::configured_backend(full_config, *provider)?;

  let api_key = {
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
  .filter(|api_key| !api_key.is_empty())
  .cloned();

  // A subscription CLI needs no API key.
  // If one is set anyway, it serves the requests the CLI can't handle.
  let api_key = match (api_key, backend) {
    (Some(api_key), _) => api_key,
    (None, Backend::Api) => return Err(get_key_setup_msg(secrets_path_str)),
    (None, _) => String::new(),
  };

  Ok(AiRequest {
    api_key,
    backend,
    ..default_req_for_model(model, full_config)
  })
}

/// Bold label of the model a request is sent to
/// (e.g. `🧠 OpenAI gpt-5.6-sol via Codex`)
fn model_label(req: &AiRequest) -> String {
  let label = get_used_model(&Model::Model(req.provider, req.model.clone()));
  match req.backend {
    Backend::Api => label,
    backend => label + &cformat!("<bold> via {backend}</bold>"),
  }
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

/// Parse a provider name (case-insensitive) as used in config model overrides.
fn provider_from_name(name: &str) -> Option<Provider> {
  match name.trim().to_lowercase().as_str() {
    "anthropic" => Some(Provider::Anthropic),
    "cerebras" => Some(Provider::Cerebras),
    "deepseek" => Some(Provider::DeepSeek),
    "google" => Some(Provider::Google),
    "groq" => Some(Provider::Groq),
    "openai" => Some(Provider::OpenAI),
    "llamafile" => Some(Provider::Llamafile),
    "ollama" => Some(Provider::Ollama),
    "xai" => Some(Provider::XAI),
    "perplexity" => Some(Provider::Perplexity),
    _ => None,
  }
}

/// Parse a model override string of the form `<provider> <model_id>`
/// (e.g. `anthropic claude-opus-5-5`). A bare provider name with no model id
/// is also accepted (e.g. `llamafile`).
fn parse_model_override(value: &str) -> Option<Model> {
  let value = value.trim();
  match value.split_once(char::is_whitespace) {
    Some((provider, model_id)) => provider_from_name(provider)
      .map(|p| Model::Model(p, model_id.trim().to_string())),
    None => provider_from_name(value).map(|p| Model::Model(p, String::new())),
  }
}

/// Lazily loaded, process-wide configuration used to resolve shortcut model
/// overrides. Falls back to an empty map if the config can't be read.
static CONFIG_CACHE: std::sync::OnceLock<HashMap<String, String>> =
  std::sync::OnceLock::new();

fn cached_config() -> &'static HashMap<String, String> {
  CONFIG_CACHE.get_or_init(|| {
    let secrets_path_str = get_secrets_path_str();
    get_full_config(&secrets_path_str).unwrap_or_default()
  })
}

/// Look up a per-shortcut model override from `~/.config/cai/config.yaml`.
///
/// Overrides live under a `shortcut_models:` table keyed by shortcut name,
/// e.g.:
/// ```yaml
/// shortcut_models:
///   fast: groq llama-3.1-8b-instant
///   opus: openai gpt-4.1
/// ```
/// Returns `None` when the shortcut isn't overridable, no override is set, or
/// the override value is malformed (a warning is printed in the latter case).
pub fn shortcut_model_override(cmd: &Commands) -> Option<Model> {
  configured_shortcut_model(cached_config(), cmd)
}

/// Look up a per-shortcut model override in `full_config`
/// (see [`shortcut_model_override`]).
fn configured_shortcut_model(
  full_config: &HashMap<String, String>,
  cmd: &Commands,
) -> Option<Model> {
  let key = cmd.config_key()?;
  let raw = full_config.get(&format!("shortcut_models.{key}"))?;
  if raw.trim().is_empty() {
    return None;
  }
  match parse_model_override(raw) {
    Some(model) => Some(model),
    None => {
      eprintln!(
        "⚠️  Invalid model override for `shortcut_models.{key}`: '{raw}'. \
        Expected format '<provider> <model>', \
        e.g. 'anthropic claude-opus-5-5'. Using default."
      );
      None
    }
  }
}

/// Resolve the model for a shortcut, preferring a config override over the
/// built-in `default`.
pub fn shortcut_model(cmd: &Commands, default: Model) -> Model {
  shortcut_model_override(cmd).unwrap_or(default)
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
    .set_default(
      "typesafe_api_key", //
      env::var("TYPESAFE_API_KEY").unwrap_or_default(),
    )?
    .add_source(config::File::with_name(secrets_path_str))
    .add_source(config::File::with_name(&config_path_str).required(false))
    .add_source(config::Environment::with_prefix("CAI"))
    .build()
    .unwrap();

  // Deserialize into the config crate's own value type first so that nested
  // tables (e.g. the `models:` section) don't break a flat string mapping.
  // Nested tables are then flattened into dotted keys (e.g. `models.fast`).
  let raw = config.try_deserialize::<HashMap<String, config::Value>>()?;
  let mut flat = HashMap::new();
  for (key, value) in raw {
    flatten_config_value(&key, value, &mut flat);
  }
  Ok(flat)
}

/// Flatten a (possibly nested) config value into dotted string keys.
/// Tables recurse with a `parent.child` prefix; scalars are stringified;
/// arrays and nulls are dropped.
fn flatten_config_value(
  prefix: &str,
  value: config::Value,
  out: &mut HashMap<String, String>,
) {
  match value.kind {
    config::ValueKind::Table(table) => {
      for (key, val) in table {
        let full_key = if prefix.is_empty() {
          key
        } else {
          format!("{prefix}.{key}")
        };
        flatten_config_value(&full_key, val, out);
      }
    }
    config::ValueKind::Nil | config::ValueKind::Array(_) => {}
    _ => {
      if let Ok(string) = value.into_string() {
        out.insert(prefix.to_string(), string);
      }
    }
  }
}

/// Models tried, in order, when the user didn't specify one.
/// Starts with the `fast` shortcut's model if it's overridden via
/// `shortcut_models.fast`, then the configured subscriptions
/// (see `<provider>_via`), which come at no extra cost,
/// and finally the built-in API defaults.
fn default_model_chain(full_config: &HashMap<String, String>) -> Vec<Model> {
  let is_subscription = |provider| {
    cli_backend::configured_backend(full_config, provider)
      .is_ok_and(|backend| backend != Backend::Api)
  };
  let sonnet =
    Model::Model(Provider::Anthropic, "claude-sonnet-5-5".to_string());
  let mut candidates: Vec<Model> =
    configured_shortcut_model(full_config, &Commands::Fast { prompt: vec![] })
      .into_iter()
      .collect();

  if is_subscription(Provider::Anthropic) {
    candidates.push(sonnet.clone());
  }
  if is_subscription(Provider::OpenAI) {
    candidates.push(Model::Model(Provider::OpenAI, "gpt-5.6-luna".to_string()));
  }

  candidates.push(Model::Model(Provider::Cerebras, "gpt-oss-120b".to_owned()));
  // Codex doesn't offer gpt-5-mini, so it would be billed to the API key
  if !is_subscription(Provider::OpenAI) {
    candidates.push(Model::Model(Provider::OpenAI, "gpt-5-mini".to_string()));
  }
  candidates.push(sonnet);

  let mut chain = vec![];
  for model in candidates {
    if !chain.contains(&model) {
      chain.push(model);
    }
  }
  chain
}

/// All requests that could serve the given model selection, in priority order.
/// An explicit model yields exactly one candidate; the default selection
/// yields every chain entry that has an API key configured, so that a
/// provider failing at request time (see [`is_provider_unavailable`]) can
/// fall through to the next one.
fn get_http_req_chain(
  optional_model: &Option<&Model>,
  secrets_path_str: &str,
  full_config: &HashMap<String, String>,
) -> Result<Vec<(String, AiRequest)>, std::string::String> {
  match optional_model {
    Some(model) => get_api_request(full_config, secrets_path_str, model)
      .map(|req| vec![(model_label(&req), req)]),
    None => {
      let candidates: Vec<(String, AiRequest)> =
        default_model_chain(full_config)
          .iter()
          .filter_map(|model| {
            get_api_request(full_config, secrets_path_str, model).ok()
          })
          .map(|req| (model_label(&req), req))
          .collect();

      if candidates.is_empty() {
        Err(get_key_setup_msg(secrets_path_str))
      } else {
        Ok(candidates)
      }
    }
  }
}

/// Whether a response status means the provider itself can't serve us
/// (missing/invalid key, exhausted quota, rate limit) as opposed to the
/// request being malformed. Only these are worth retrying elsewhere.
fn is_provider_unavailable(status: reqwest::StatusCode) -> bool {
  matches!(status.as_u16(), 401 | 402 | 403 | 429)
}

fn get_http_req(
  optional_model: &Option<&Model>,
  secrets_path_str: &str,
  full_config: &HashMap<String, String>,
) -> Result<(String, AiRequest), std::string::String> {
  get_http_req_chain(optional_model, secrets_path_str, full_config).map(
    |mut chain| chain.remove(0), //
  )
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
    let model = &http_req.model;

    // The Interactions API takes the prompt as a plain string
    if is_google_interactions_model(model) {
      return json!({ "model": model, "input": user_input });
    }

    // Veo's video models use the prediction API instead of `generateContent`
    if is_google_video_model(model) {
      return json!({
        "instances": [{ "prompt": user_input }],
        "parameters": { "aspectRatio": "16:9" },
      });
    }

    // Embedding models expect a single `content` object
    if is_google_embedding_model(model) {
      return json!({
        "content": { "parts": [{ "text": user_input }] },
      });
    }

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

    if is_google_image_model(model) {
      generation_config.insert(
        "maxOutputTokens".to_string(),
        Value::Number(http_req.max_tokens.into()),
      );
      generation_config.insert(
        "responseModalities".to_string(),
        Value::Array(vec![Value::String("IMAGE".to_string())]),
      );
    } else if is_google_audio_model(model) {
      // No token limit, as it would cut the generated audio short
      generation_config.insert(
        "responseModalities".to_string(),
        Value::Array(vec![Value::String("AUDIO".to_string())]),
      );
      if is_google_tts_model(model) {
        generation_config.insert(
          "speechConfig".to_string(),
          json!({
            "voiceConfig": { "prebuiltVoiceConfig": { "voiceName": "Kore" } },
          }),
        );
      }
    } else {
      generation_config.insert(
        "maxOutputTokens".to_string(),
        Value::Number(http_req.max_tokens.into()),
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

    if let Some(Commands::Image {
      background: Some(bg),
      ..
    }) = &opts.subcommand
    {
      map.insert("background".to_string(), Value::String(bg.clone()));
    }

    return Value::Object(map);
  }

  // Special handling for xAI image models (use images API)
  if http_req.provider == Provider::XAI && is_xai_image_model(&http_req.model) {
    let mut map = Map::new();
    map.insert("model".to_string(), Value::String(http_req.model.clone()));
    map.insert("prompt".to_string(), Value::String(user_input.to_string()));
    map.insert("n".to_string(), Value::Number(1.into()));
    // Request the image data itself, as the returned URLs expire
    map.insert(
      "response_format".to_string(),
      Value::String("b64_json".to_string()),
    );

    return Value::Object(map);
  }

  // For all other providers
  let mut map = Map::new();
  map.insert("model".to_string(), Value::String(http_req.model.clone()));
  // OpenAI o1, o3, o4, gpt-5, and gpt-6 models
  // require max_completion_tokens instead of max_tokens
  if http_req.provider == Provider::OpenAI
    && (http_req.model.starts_with("o1")
      || http_req.model.starts_with("o3")
      || http_req.model.starts_with("o4")
      || http_req.model.starts_with("gpt-5")
      || http_req.model.starts_with("gpt-6"))
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
  is_streaming: bool,
) -> Result<Response, reqwest::Error> {
  let client = reqwest::Client::new();
  let req_base = client.post(http_req.url.clone()).json(&req_body_obj);
  let req = match http_req.provider {
    Provider::Anthropic => req_base
      .header("anthropic-version", "2023-06-01")
      .header("x-api-key", &http_req.api_key),
    Provider::Google => {
      // For Google's Gemini API we need to append the model name and the
      // generation action to the URL along with the API key as a query
      // parameter. When streaming, use ":streamGenerateContent" with SSE.
      let model = &http_req.model;

      // The Interactions API has its own endpoint
      // and names the model in the request body instead of the URL
      if is_google_interactions_model(model) {
        let url = format!(
          "{}/interactions?key={}",
          google_base_url(http_req),
          http_req.api_key,
        );
        return client.post(url).json(&req_body_obj).send().await;
      }

      let (action, extra_query) = if is_google_video_model(model) {
        ("predictLongRunning", "")
      } else if is_google_embedding_model(model) {
        ("embedContent", "")
      } else if is_streaming {
        ("streamGenerateContent", "alt=sse&")
      } else {
        ("generateContent", "")
      };
      let url = format!(
        "{}/{model}:{action}?{extra_query}key={}",
        http_req.url, http_req.api_key
      );
      client.post(url).json(&req_body_obj)
    }
    _ => req_base.bearer_auth(&http_req.api_key),
  };
  req.send().await
}

/// The Gemini API's base URL, derived from the request's `<base>/models` URL
fn google_base_url(http_req: &AiRequest) -> &str {
  http_req.url.trim_end_matches("/models")
}

/// xAI image models (Grok Imagine) are served by the images API,
/// not the chat completions API
fn is_xai_image_model(model: &str) -> bool {
  model.starts_with("grok-imagine-image") || model == "grok-2-image"
}

/// Gemini models that return images (`gemini-*-image`, Nano Banana)
fn is_google_image_model(model: &str) -> bool {
  model.contains("-image") || model.starts_with("nano-banana")
}

/// Veo models generate videos via a long running operation
fn is_google_video_model(model: &str) -> bool {
  model.starts_with("veo-")
}

/// Lyria models generate music
fn is_google_music_model(model: &str) -> bool {
  model.starts_with("lyria-")
}

/// Gemini models that synthesize speech
fn is_google_tts_model(model: &str) -> bool {
  model.contains("-tts")
}

/// Gemini models that are served by the `:embedContent` endpoint
fn is_google_embedding_model(model: &str) -> bool {
  model.contains("embedding")
}

/// The any-to-any Omni models are only served by the Interactions API,
/// which takes a plain prompt and can answer with any modality
fn is_google_interactions_model(model: &str) -> bool {
  model.starts_with("gemini-omni")
}

/// Whether a Google model returns audio instead of text
fn is_google_audio_model(model: &str) -> bool {
  is_google_tts_model(model) || is_google_music_model(model)
}

/// Whether a request returns text (vs binary like images or audio)
fn is_text_response(http_req: &AiRequest, opts: &ExecOptions) -> bool {
  // OpenAI TTS
  if http_req.provider == Provider::OpenAI && http_req.model.contains("-tts") {
    return false;
  }

  // OpenAI image generation (gpt-image, DALL-E, or `cai openai image …`)
  let is_image_generation =
    matches!(
      &opts.subcommand,
      Some(Commands::Openai { model, .. }) if model == "image"
    ) || matches!(&opts.subcommand, Some(Commands::Image { .. }));
  if http_req.provider == Provider::OpenAI
    && (is_image_generation
      || http_req.model.starts_with("gpt-image")
      || http_req.model.starts_with("dall-e"))
  {
    return false;
  }

  // xAI image generation (Grok Imagine)
  if http_req.provider == Provider::XAI && is_xai_image_model(&http_req.model) {
    return false;
  }

  // Google media generation (images, video, music, speech), embeddings,
  // and the any-to-any Omni models all need their own response handling
  if http_req.provider == Provider::Google
    && (is_google_image_model(&http_req.model)
      || is_google_video_model(&http_req.model)
      || is_google_audio_model(&http_req.model)
      || is_google_embedding_model(&http_req.model)
      || is_google_interactions_model(&http_req.model))
  {
    return false;
  }

  true
}

/// Detect whether the terminal has a light background by querying it via
/// OSC 11. Falls back to dark on any error (e.g. no TTY, unsupported term).
fn is_light_terminal() -> bool {
  matches!(terminal_light::luma(), Ok(luma) if luma > 0.5)
}

/// Render style tuned to the detected terminal background.
///
/// Code blocks get a subtle background tint via `dark` so they stand out
/// from prose; everything else (table headers, surrounding fill) stays
/// transparent so the terminal's own background shows through.
fn transparent_render_style(
  light_terminal: bool,
) -> streamdown_render::RenderStyle {
  let (bright, head, symbol, grey, code_bg) = if light_terminal {
    ("#0050a0", "#005500", "#7d2eb6", "#606060", "#e8e8e8")
  } else {
    ("#87ceeb", "#98fb98", "#dda0dd", "#808080", "#1a1a2e")
  };
  streamdown_render::RenderStyle {
    bright: bright.to_string(),
    head: head.to_string(),
    symbol: symbol.to_string(),
    grey: grey.to_string(),
    dark: code_bg.to_string(),
    // Empty hex strings make `bg_color` emit nothing for these slots,
    // keeping table headers and other fills transparent.
    mid: String::new(),
    light: String::new(),
  }
}

/// Wraps a writer so every `write` is followed by a flush, ensuring streamed
/// content reaches the terminal as soon as the renderer emits it.
struct AutoFlush<W: Write>(W);

impl<W: Write> Write for AutoFlush<W> {
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    let n = self.0.write(buf)?;
    self.0.flush()?;
    Ok(n)
  }

  fn flush(&mut self) -> std::io::Result<()> {
    self.0.flush()
  }
}

/// Render one or more newline-separated lines through the streamdown
/// markdown renderer, emitting events for any complete lines and keeping
/// any trailing partial line in `line_buf` for the next chunk.
fn render_through_streamdown(
  line_buf: &mut String,
  parser: &mut streamdown_parser::Parser,
  renderer: &mut streamdown_render::Renderer<AutoFlush<std::io::Stdout>>,
  flush_partial: bool,
) -> std::io::Result<()> {
  while let Some(pos) = line_buf.find('\n') {
    let line: String = line_buf.drain(..pos + 1).collect();
    let line_no_nl = &line[..line.len() - 1];
    for event in parser.parse_line(line_no_nl) {
      renderer.render_event(&event)?;
    }
  }
  if flush_partial && !line_buf.is_empty() {
    let remaining = std::mem::take(line_buf);
    for event in parser.parse_line(&remaining) {
      renderer.render_event(&event)?;
    }
  }
  Ok(())
}

/// Read an SSE response stream and emit text deltas as they arrive.
/// In non-raw mode, output is rendered as markdown via `streamdown` so
/// headings, code blocks, lists, etc. are styled even while streaming.
/// In raw mode, deltas go to stdout unmodified for downstream piping.
/// Returns the accumulated full text and any search results encountered.
async fn stream_text_response(
  resp: Response,
  provider: Provider,
  is_raw: bool,
) -> Result<(String, Option<Vec<SearchResult>>), Box<dyn Error + Send + Sync>> {
  let mut full_text = String::new();
  let mut search_results: Option<Vec<SearchResult>> = None;
  let mut buffer: Vec<u8> = Vec::new();
  let mut byte_stream = resp.bytes_stream();

  let width = textwrap::termwidth();
  let light = is_light_terminal();
  let mut md_parser = streamdown_parser::Parser::new();
  let mut md_renderer = (!is_raw).then(|| {
    let mut r =
      streamdown_render::Renderer::new(AutoFlush(std::io::stdout()), width);
    r.set_style(transparent_render_style(light));
    if light {
      r.set_theme("InspiredGitHub");
    }
    r
  });
  let mut line_buf = String::new();

  while let Some(chunk_result) = byte_stream.next().await {
    let chunk = chunk_result?;
    buffer.extend_from_slice(&chunk);

    // Parse complete SSE events (separated by blank lines).
    loop {
      let boundary = buffer
        .windows(2)
        .position(|w| w == b"\n\n")
        .map(|p| (p, 2))
        .or_else(|| {
          buffer
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|p| (p, 4))
        });
      let (pos, sep_len) = match boundary {
        Some(b) => b,
        None => break,
      };

      let event_bytes: Vec<u8> = buffer.drain(..pos + sep_len).collect();
      let event_str = match std::str::from_utf8(&event_bytes) {
        Ok(s) => s,
        Err(_) => continue,
      };

      for line in event_str.lines() {
        let data = match line.strip_prefix("data:") {
          Some(d) => d.strip_prefix(' ').unwrap_or(d),
          None => continue,
        };
        if data == "[DONE]" {
          continue;
        }
        let json: Value = match serde_json::from_str(data) {
          Ok(v) => v,
          Err(_) => continue,
        };

        let text_delta: Option<String> = match provider {
          Provider::Anthropic => {
            if json["type"].as_str() == Some("content_block_delta") {
              json["delta"]["text"].as_str().map(str::to_string)
            } else {
              None
            }
          }
          Provider::Google => json["candidates"][0]["content"]["parts"][0]
            ["text"]
            .as_str()
            .map(str::to_string),
          _ => json["choices"][0]["delta"]["content"]
            .as_str()
            .map(str::to_string),
        };

        if let Some(results) = json["search_results"].as_array() {
          let parsed: Vec<SearchResult> = results
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();
          if !parsed.is_empty() {
            search_results = Some(parsed);
          }
        }

        if let Some(text) = text_delta {
          full_text.push_str(&text);
          if let Some(renderer) = md_renderer.as_mut() {
            line_buf.push_str(&text);
            render_through_streamdown(
              &mut line_buf,
              &mut md_parser,
              renderer,
              false,
            )?;
          } else {
            let mut stdout = std::io::stdout();
            stdout.write_all(text.as_bytes())?;
            stdout.flush()?;
          }
        }
      }
    }
  }

  // End of stream: render any trailing partial line, then close any
  // still-open blocks (lists, code fences, …) via `finalize()`.
  if let Some(renderer) = md_renderer.as_mut() {
    render_through_streamdown(&mut line_buf, &mut md_parser, renderer, true)?;
    for event in md_parser.finalize() {
      renderer.render_event(&event)?;
    }
  }

  Ok((full_text, search_results))
}

/// Extract the assistant's text (and any search results) from a
/// non-streaming response, accounting for each provider's response shape.
async fn parse_text_response(
  resp: Response,
  provider: Provider,
) -> Result<(String, Option<Vec<SearchResult>>), Box<dyn Error + Send + Sync>> {
  Ok(match provider {
    Provider::Anthropic => {
      let anth_response = resp.json::<AnthropicAiResponse>().await?;
      (anth_response.content[0].text.clone(), None)
    }
    Provider::Google => {
      // Handle Google's unique response format
      let response_text = resp.text().await?;
      let response_json: Value = serde_json::from_str(&response_text)?;

      // Extract the text from the Gemini response format
      let text = response_json["candidates"][0]["content"]["parts"][0]["text"]
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
  })
}

/// Write one base64 encoded media blob of a Gemini response to disk
/// and report where it landed.
fn report_saved_media(
  mime_type: &str,
  data_base64: &str,
  prompt: &str,
  index: usize,
) {
  let media_kind = match mime_type.split('/').next().unwrap_or_default() {
    "image" => "image",
    "audio" => "audio",
    "video" => "video",
    _ => "file",
  };

  match save_gemini_inline_data(mime_type, data_base64, prompt) {
    Ok(filename) => println!("Generated {media_kind} saved to: {filename}"),
    Err(err) => println!("Failed to save {media_kind} {index}: {err}"),
  }
}

/// Print the text parts of a Gemini response (e.g. the lyrics Lyria returns
/// alongside a song) and write its inline media parts to disk.
fn save_gemini_media_parts(response_json: &Value, prompt: &str) {
  let mut media_count = 0;

  for candidate in response_json["candidates"].as_array().into_iter().flatten()
  {
    for part in candidate["content"]["parts"]
      .as_array()
      .into_iter()
      .flatten()
    {
      if let Some(text) = part["text"].as_str() {
        println!("{text}\n");
      }

      if let Some(data_base64) = part["inlineData"]["data"].as_str() {
        media_count += 1;
        report_saved_media(
          part["inlineData"]["mimeType"].as_str().unwrap_or_default(),
          data_base64,
          prompt,
          media_count,
        );
      }
    }
  }
}

/// Print the text blocks of an Interactions API response
/// and write its media blocks to disk.
fn save_interaction_content(response_json: &Value, prompt: &str) {
  let mut media_count = 0;

  for step in response_json["steps"].as_array().into_iter().flatten() {
    // Skip the model's thoughts and tool calls
    if step["type"].as_str() != Some("model_output") {
      continue;
    }

    for block in step["content"].as_array().into_iter().flatten() {
      match block["text"].as_str() {
        Some(text) if !text.is_empty() => println!("{text}\n"),
        _ => {}
      }

      if let Some(data_base64) = block["data"].as_str() {
        media_count += 1;
        report_saved_media(
          block["mime_type"].as_str().unwrap_or_default(),
          data_base64,
          prompt,
          media_count,
        );
      }
    }
  }
}

/// How long to wait for a Veo video generation operation before giving up
const VIDEO_GENERATION_TIMEOUT: std::time::Duration =
  std::time::Duration::from_secs(10 * 60);

/// Poll a Veo video generation operation until it's done,
/// then download every generated sample.
/// Returns the names of the written files.
async fn await_video_operation(
  http_req: &AiRequest,
  operation_name: &str,
  prompt: &str,
) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
  // Operation names are relative to the API's base URL
  // (`models/veo-…/operations/…`)
  let base_url = google_base_url(http_req);
  let poll_url =
    format!("{base_url}/{operation_name}?key={}", http_req.api_key);
  let client = reqwest::Client::new();
  let deadline = Instant::now() + VIDEO_GENERATION_TIMEOUT;

  eprintln!("{}", cformat!("<dim>Generating video …</dim>"));

  let operation = loop {
    let operation = client.get(&poll_url).send().await?.json::<Value>().await?;

    if let Some(error) = operation.get("error") {
      Err(serde_json::to_string_pretty(error)?)?;
    }
    if operation["done"].as_bool().unwrap_or(false) {
      break operation;
    }
    if Instant::now() >= deadline {
      Err(format!(
        "Video generation timed out after {} minutes. \
        Check the operation at {base_url}/{operation_name}",
        VIDEO_GENERATION_TIMEOUT.as_secs() / 60,
      ))?;
    }
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
  };

  let mut filenames = vec![];

  for sample in operation["response"]["generateVideoResponse"]
    ["generatedSamples"]
    .as_array()
    .into_iter()
    .flatten()
  {
    // The video is served from the Files API and needs the API key as well
    if let Some(uri) = sample["video"]["uri"].as_str() {
      let separator = if uri.contains('?') { "&" } else { "?" };
      let video_bytes = client
        .get(format!("{uri}{separator}key={}", http_req.api_key))
        .send()
        .await?
        .bytes()
        .await?;
      filenames.push(save_media_bytes(&video_bytes, prompt, "mp4")?);
    }
    // Smaller videos can be returned inline instead
    else if let Some(data_base64) =
      sample["video"]["bytesBase64Encoded"].as_str()
    {
      let video_bytes =
        base64::engine::general_purpose::STANDARD.decode(data_base64)?;
      filenames.push(save_media_bytes(&video_bytes, prompt, "mp4")?);
    }
  }

  if filenames.is_empty() {
    Err(serde_json::to_string_pretty(&operation)?)?;
  }

  Ok(filenames)
}

pub async fn exec_tool(
  optional_model: &Option<&Model>,
  opts: &ExecOptions,
  user_input: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let start = Instant::now();
  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;
  let req_chain =
    get_http_req_chain(optional_model, &secrets_path_str, &full_config)?;

  // This is checked here, so that the missing API key message comes first
  if user_input.is_empty() {
    Err("No prompt was provided")?;
  }

  // Walk the candidates until one answers. Only a provider-level failure
  // (no quota, bad key, rate limit) falls through to the next entry;
  // any other response — including errors — is reported as is.
  // With an explicit model the chain holds one entry and this is a no-op.
  let chain_labels: Vec<String> = req_chain
    .iter()
    .map(|(_, req)| match req.backend {
      Backend::Api => format!("{} {}", req.provider, req.model),
      backend => format!("{} {} via {backend}", req.provider, req.model),
    })
    .collect();
  let last_index = req_chain.len() - 1;
  let mut attempt = None;

  let subcommand = subcommand_prefix(opts);

  for (index, (mut used_model, mut http_req)) in
    req_chain.into_iter().enumerate()
  {
    // Requests the subscription CLI can't serve go to the API, if possible
    if http_req.backend != Backend::Api {
      let api_feature =
        match cli_unsupported_feature(&http_req, opts, user_input) {
          Some(feature) => feature.to_string(),
          None => match cli_backend::complete(
            http_req.backend,
            &http_req.model,
            user_input,
            &[],
            cli_json_schema(opts),
          )
          .await
          {
            Ok(answer) => {
              print_text_answer(
                opts,
                &format!("{subcommand}{used_model}"),
                start,
                &answer,
                None,
              );
              return Ok(());
            }
            Err(CliError::UnsupportedModel(_)) => {
              format!("`{}`", http_req.model)
            }
            Err(CliError::Failed(msg)) if index < last_index => {
              warn_fallback(
                &chain_labels[index],
                &msg,
                &chain_labels[index + 1],
              );
              continue;
            }
            Err(CliError::Failed(msg)) => Err(msg)?,
          },
        };

      match http_req.require_api(&api_feature) {
        Ok(api_req) => {
          used_model = model_label(&api_req);
          http_req = api_req;
        }
        Err(msg) if index < last_index => {
          warn_fallback(&chain_labels[index], &msg, &chain_labels[index + 1]);
          continue;
        }
        Err(msg) => return Err(msg.into()),
      }
    }

    let should_stream = opts.is_streaming && is_text_response(&http_req, opts);

    let mut req_body_obj = get_req_body_obj(opts, &http_req, user_input);
    // Google controls streaming via the URL (:streamGenerateContent), not a
    // body field — adding `stream: true` there is rejected as an unknown field.
    if should_stream && http_req.provider != Provider::Google {
      if let Some(obj) = req_body_obj.as_object_mut() {
        obj.insert("stream".to_string(), Value::Bool(true));
      }
    }

    let resp = exec_request(&http_req, &req_body_obj, should_stream).await?;

    if index < last_index && is_provider_unavailable(resp.status()) {
      let status = resp.status();
      let reason = status.canonical_reason().unwrap_or("Error");
      warn_fallback(
        &chain_labels[index],
        &format!("{} {reason}", status.as_u16()),
        &chain_labels[index + 1],
      );
      continue;
    }

    attempt = Some((used_model, http_req, should_stream, resp));
    break;
  }

  let (used_model, http_req, should_stream, resp) =
    attempt.expect("request chain is never empty");

  if !&resp.status().is_success() {
    let elapsed_millis = start.elapsed().as_millis();
    let (elapsed_time, time_unit) = format_elapsed_time(elapsed_millis);
    let resp_json = resp.json::<Value>().await?;
    let resp_formatted = serde_json::to_string_pretty(&resp_json).unwrap();
    Err(cformat!(
      "<bold>{subcommand}{used_model} | ⏱️ {} {}</bold>\n\
      \n{resp_formatted}",
      elapsed_time,
      time_unit,
    ))?;
  } else if should_stream {
    if !opts.is_raw {
      cprintln!("<bold>{subcommand}{used_model}</bold>\n");
    }

    let (_full_text, search_results) =
      stream_text_response(resp, http_req.provider, opts.is_raw).await?;

    let elapsed_millis = start.elapsed().as_millis();
    let (elapsed_time, time_unit) = format_elapsed_time(elapsed_millis);

    if opts.is_raw {
      println!();
    } else {
      cprintln!("\n<bold>⏱️ {} {}</bold>", elapsed_time, time_unit);

      if let Some(results) = search_results {
        if !results.is_empty() {
          println!("\n## Search Results\n");
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
      println!();
    }
    return Ok(());
  } else {
    let elapsed_millis = start.elapsed().as_millis();
    let (elapsed_time, time_unit) = format_elapsed_time(elapsed_millis);
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
            // Extract original user prompt from subcommand if available
            // (for Photo/Image commands, user_input contains system instructions)
            let original_prompt = match &opts.subcommand {
              Some(Commands::Photo { prompt }) => prompt.join(" "),
              Some(Commands::Image { prompt, .. }) => prompt.join(" "),
              _ => user_input.to_string(),
            };

            match save_base64_image(image_base64, &original_prompt) {
              Ok(filename) => println!("Generated image saved to: {filename}"),
              Err(err) => println!("Failed to save image {image_count}: {err}"),
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

    // Special handling for xAI image models (Grok Imagine)
    if http_req.provider == Provider::XAI && is_xai_image_model(&http_req.model)
    {
      let response_json = resp.json::<Value>().await?;

      cprintln!(
        "<bold>{subcommand}{used_model} | ⏱️ {} {}</bold>\n",
        elapsed_time,
        time_unit,
      );

      // xAI uses the same format as the OpenAI images API:
      // a data array with either a URL or base64 encoded image data
      if let Some(data) = response_json["data"].as_array() {
        for (i, image) in data.iter().enumerate() {
          let image_count = i + 1;

          if let Some(image_base64) = image["b64_json"].as_str() {
            match save_base64_image(image_base64, user_input) {
              Ok(filename) => {
                println!("Generated image saved to: {filename}")
              }
              Err(err) => println!("Failed to save image {image_count}: {err}"),
            }
          } else if let Some(url) = image["url"].as_str() {
            println!("Generated image {image_count}: {url}");
          }
        }
      }

      return Ok(());
    }

    // Special handling for Google's media and embedding models
    if http_req.provider == Provider::Google
      && !is_text_response(&http_req, opts)
    {
      let response_json = resp.json::<Value>().await?;

      // Name generated files after the prompt the user actually typed
      // (`user_input` can carry additional instructions)
      let media_prompt = match &opts.subcommand {
        Some(
          Commands::Music { prompt }
          | Commands::GoogleImage { prompt }
          | Commands::GoogleVideo { prompt }
          | Commands::GoogleMusic { prompt }
          | Commands::GoogleSay { prompt },
        ) => prompt.join(" "),
        _ => user_input.to_string(),
      };

      if is_google_embedding_model(&http_req.model) {
        let embedding = &response_json["embedding"]["values"];
        if opts.is_raw {
          println!("{embedding}");
        } else {
          let dimensions = embedding.as_array().map_or(0, |vec| vec.len());
          cprintln!(
            "<bold>{subcommand}{used_model} | \
            {dimensions} dimensions | ⏱️ {} {}</bold>\n",
            elapsed_time,
            time_unit,
          );
          println!("{embedding}");
        }
        return Ok(());
      }

      cprintln!(
        "<bold>{subcommand}{used_model} | ⏱️ {} {}</bold>\n",
        elapsed_time,
        time_unit,
      );

      // Veo only returns the name of a long running operation
      if is_google_video_model(&http_req.model) {
        let operation_name =
          response_json["name"].as_str().ok_or_else(|| {
            serde_json::to_string_pretty(&response_json).unwrap_or_default()
          })?;
        for filename in
          await_video_operation(&http_req, operation_name, &media_prompt)
            .await?
        {
          println!("Generated video saved to: {filename}");
        }
        return Ok(());
      }

      // The Omni models answer with their own response shape
      if is_google_interactions_model(&http_req.model) {
        save_interaction_content(&response_json, &media_prompt);
        return Ok(());
      }

      // Images, music, and speech are returned inline
      save_gemini_media_parts(&response_json, &media_prompt);

      return Ok(());
    }

    let (msg, search_results) =
      parse_text_response(resp, http_req.provider).await?;

    print_text_answer(
      opts,
      &format!("{subcommand}{used_model}"),
      start,
      &msg,
      search_results,
    );
  }
  Ok(())
}

/// Warn that a model of the default chain is unavailable
/// and that the next one is tried instead.
fn warn_fallback(label: &str, reason: &str, next_label: &str) {
  // To stderr, so that piping the answer stays unaffected.
  eprintln!(
    "{}",
    cformat!(
      "<yellow>⚠️  {} is unavailable ({}). Falling back to {} …</yellow>",
      label,
      reason,
      next_label,
    )
  );
}

/// Print a complete (i.e. not streamed) text answer with its metadata
fn print_text_answer(
  opts: &ExecOptions,
  header: &str,
  start: Instant,
  msg: &str,
  search_results: Option<Vec<SearchResult>>,
) {
  if opts.is_raw {
    println!("{msg}");
    return;
  }

  let (elapsed_time, time_unit) =
    format_elapsed_time(start.elapsed().as_millis());
  cprintln!(
    "<bold>{header} | ⏱️ {} {}</bold>\n",
    elapsed_time,
    time_unit
  );
  highlight::text_via_bat(msg);

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

/// Why a request can't be sent via a subscription CLI, if it can't
fn cli_unsupported_feature(
  http_req: &AiRequest,
  opts: &ExecOptions,
  user_input: &str,
) -> Option<&'static str> {
  if !is_text_response(http_req, opts) {
    Some("Media generation")
  }
  // A JSON object is sent as the raw request body (see `get_req_body_obj`)
  else if serde_json::from_str::<Value>(user_input)
    .is_ok_and(|v| v.is_object())
  {
    Some("A raw request body")
  } else if opts.is_json && opts.json_schema.is_none() {
    Some("JSON mode without a JSON schema")
  } else {
    None
  }
}

/// The bare JSON schema of the requested output for a subscription CLI
/// (`opts.json_schema` wraps it in OpenAI's `json_schema` response format)
fn cli_json_schema(opts: &ExecOptions) -> Option<&Value> {
  opts
    .json_schema
    .as_ref()
    .map(|wrapper| wrapper.get("schema").unwrap_or(wrapper))
}

/// Header prefix naming the executed subcommand (e.g. `➡️ OCR | `)
fn subcommand_prefix(opts: &ExecOptions) -> String {
  opts
    .subcommand
    .as_ref()
    .and_then(|x| x.to_string_pretty())
    .map(|subcom| format!("➡️ {subcom} | "))
    .unwrap_or_default()
}

/// Send `prompt` to the model of `http_req` and return its whole answer
/// without printing it.
async fn complete_text(
  http_req: &AiRequest,
  opts: &ExecOptions,
  prompt: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
  let (_model_label, answer) =
    complete_prompt(http_req, opts, prompt, &[]).await?;
  Ok(answer)
}

/// Send `prompt` and `images` to the model of `http_req`
/// and return the label of the model that answered along with its answer.
async fn complete_prompt(
  http_req: &AiRequest,
  opts: &ExecOptions,
  prompt: &str,
  images: &[Image<'_>],
) -> Result<(String, String), Box<dyn Error + Send + Sync>> {
  let mut http_req = match cli_unsupported_feature(http_req, opts, prompt) {
    Some(feature) => http_req.clone().require_api(feature)?,
    None => http_req.clone(),
  };

  if http_req.backend != Backend::Api {
    match cli_backend::complete(
      http_req.backend,
      &http_req.model,
      prompt,
      images,
      cli_json_schema(opts),
    )
    .await
    {
      Ok(answer) => return Ok((model_label(&http_req), answer)),
      Err(CliError::UnsupportedModel(_)) => {
        let model = format!("`{}`", http_req.model);
        http_req = http_req.require_api(&model)?;
      }
      Err(CliError::Failed(msg)) => Err(msg)?,
    }
  }

  let mut req_body_obj = get_req_body_obj(opts, &http_req, prompt);
  attach_images(&mut req_body_obj, http_req.provider, images)?;
  let resp = exec_request(&http_req, &req_body_obj, false).await?;

  if !resp.status().is_success() {
    let resp_json = resp.json::<Value>().await?;
    return Err(serde_json::to_string_pretty(&resp_json)?.into());
  }

  let (msg, _search_results) =
    parse_text_response(resp, http_req.provider).await?;
  Ok((model_label(&http_req), msg))
}

/// Attach `images` to the user message of a chat request body
fn attach_images(
  req_body_obj: &mut Value,
  provider: Provider,
  images: &[Image<'_>],
) -> Result<(), Box<dyn Error + Send + Sync>> {
  if images.is_empty() {
    return Ok(());
  }
  let Some(message) = req_body_obj.pointer_mut("/messages/0") else {
    Err(format!("Attaching images isn't supported for {provider}"))?
  };

  let mut content = vec![json!({ "type": "text", "text": message["content"] })];
  for image in images {
    let data = base64::engine::general_purpose::STANDARD
      .encode(std::fs::read(image.path)?);
    content.push(match provider {
      Provider::Anthropic => json!({
        "type": "image",
        "source": {
          "type": "base64",
          "media_type": image.mime_type,
          "data": data,
        },
      }),
      _ => json!({
        "type": "image_url",
        "image_url": { "url": format!("data:{};base64,{data}", image.mime_type) },
      }),
    });
  }
  message["content"] = Value::Array(content);

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

  let model = Model::Model(Provider::OpenAI, "gpt-5".to_string());

  exec_tool(&Some(&model), opts, &prompt).await
}

#[derive(Deserialize)]
pub struct FileAnalysis {
  pub description: String,
  pub timestamp: Option<String>,
}

const OCR_PROMPT: &str = "Extract and return all text from this image. \
  Just the text and no explanation!";

/// Gemini model used to extract text from images at high media resolution
const GOOGLE_OCR_MODEL: &str = "gemini-3.1-pro-preview";

/// Extract all text from an image via OCR with a vision model
/// and return the label of the used model along with the text.
async fn ocr_image_to_text(
  file_path: &str,
) -> Result<(String, String), Box<dyn Error + Send + Sync>> {
  let mime_type = get_image_mime_type(file_path);

  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;

  // OpenAI's vision API doesn't accept HEIC/HEIF, but Google Gemini does
  if mime_type == "image/heic" || mime_type == "image/heif" {
    eprintln!(
      "{}",
      cformat!(
        "<dim>{file_path}: \
        Extracting text from image with Google Gemini …</dim>"
      )
    );
    let model = Model::Model(Provider::Google, GOOGLE_OCR_MODEL.to_string());
    let (used_model, http_req) =
      get_http_req(&Some(&model), &secrets_path_str, &full_config)?;
    let base64_content = base64::engine::general_purpose::STANDARD
      .encode(std::fs::read(file_path)?);

    let req_body_obj = json!({
      "contents": [{
        "parts": [
          { "text": OCR_PROMPT },
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
    });

    let resp = exec_request(&http_req, &req_body_obj, false).await?;

    if resp.status().is_success() {
      let json_val = resp.json::<Value>().await?;
      let text = json_val["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string();
      Ok((used_model, text))
    } else {
      let json_val = resp.json::<Value>().await?;
      let json_str = serde_json::to_string_pretty(&json_val).unwrap();
      Err(json_str.into())
    }
  } else {
    eprintln!(
      "{}",
      cformat!(
        "<dim>{file_path}: Extracting text from image with OpenAI …</dim>"
      )
    );
    let model = Model::Model(Provider::OpenAI, "gpt-5.6-terra".to_string());
    let (_used_model, http_req) =
      get_http_req(&Some(&model), &secrets_path_str, &full_config)?;

    complete_prompt(
      &http_req,
      &ExecOptions::default(),
      OCR_PROMPT,
      &[Image {
        path: file_path,
        mime_type,
      }],
    )
    .await
  }
}

fn is_image_file(file_path: &str) -> bool {
  let lower = file_path.to_lowercase();
  [".png", ".jpg", ".jpeg", ".gif", ".webp", ".heic", ".heif"]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

pub async fn analyze_file_content(
  opts: &ExecOptions,
  file_path: &str,
) -> Result<FileAnalysis, Box<dyn Error + Send + Sync>> {
  let content = if file_path.to_lowercase().ends_with(".pdf") {
    eprintln!(
      "{}",
      cformat!("<dim>{file_path}: Extracting text from PDF …</dim>")
    );
    pdf_extract::extract_text(file_path)
      .map_err(|e| format!("Failed to extract PDF text: {e}"))?
  } else if is_image_file(file_path) {
    let (_used_model, ocr_text) = ocr_image_to_text(file_path).await?;
    if ocr_text.trim().is_empty() {
      // No text in the image -> let callers use their non-text fallback
      return Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "Image contains no extractable text",
      )));
    }
    ocr_text
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
    &Some(&Model::Model(Provider::OpenAI, "gpt-5.6-terra".to_string())),
    &secrets_path_str,
    &full_config,
  )?;
  eprintln!(
    "{}",
    cformat!("<dim>{file_path}: Generating description and timestamp …</dim>")
  );
  let content = complete_text(&http_req, &opts, &prompt).await?;
  let analysis: FileAnalysis = serde_json::from_str(&content).map_err(|e| {
    format!(
      "Failed to parse LLM response as JSON\n
        Response: {content}\n
        Error: {e}\n",
    )
  })?;
  Ok(analysis)
}

pub async fn extract_text_from_file(
  opts: &ExecOptions,
  file_path: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let start = Instant::now();
  let (used_model, text) = ocr_image_to_text(file_path).await?;
  let subcommand = subcommand_prefix(opts);
  print_text_answer(
    opts,
    &format!("{subcommand}{used_model}"),
    start,
    &text,
    None,
  );
  Ok(())
}

pub async fn google_ocr_file(
  opts: &ExecOptions,
  file_path: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let file_content = std::fs::read(file_path)?;
  let base64_content =
    base64::engine::general_purpose::STANDARD.encode(&file_content);

  let mime_type = get_image_mime_type(file_path);

  let model = &Model::Model(Provider::Google, GOOGLE_OCR_MODEL.to_string());

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

/// Guess the MIME type of an audio file from its file extension.
/// Falls back to `audio/mpeg` for unknown extensions.
fn get_audio_mime_type(file_path: &str) -> &'static str {
  let lower = file_path.to_lowercase();
  let extension = lower.rsplit('.').next().unwrap_or_default();
  match extension {
    "wav" => "audio/wav",
    "flac" => "audio/flac",
    "ogg" | "oga" => "audio/ogg",
    "webm" => "audio/webm",
    "m4a" | "mp4" => "audio/mp4",
    _ => "audio/mpeg",
  }
}

/// Resolve the model of the `transcribe` command.
/// Gemini models (and anything prefixed with `google/`) are served by Google's
/// API, every other transcription model by OpenAI's.
pub fn transcription_model(model_id: &str) -> Model {
  match model_id.strip_prefix("google/") {
    Some(google_model_id) => {
      Model::Model(Provider::Google, google_model_id.to_string())
    }
    None if model_id.starts_with("gemini") => {
      Model::Model(Provider::Google, model_id.to_string())
    }
    None => Model::Model(Provider::OpenAI, model_id.to_string()),
  }
}

/// Transcribe an audio file with a Gemini model, which takes the audio inline
/// in a `generateContent` request instead of a multipart file upload.
///
/// Gemini has no dedicated language or keyword parameters,
/// so those hints become part of the prompt.
async fn transcribe_via_gemini(
  opts: &ExecOptions,
  http_req: &AiRequest,
  languages: &[String],
  keywords: &[String],
  file_path: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  if http_req.model.ends_with("-live") {
    Err(format!(
      "`{}` is only available in realtime sessions. \
      Use `gemini-3.5-transcribe` to transcribe audio files.",
      http_req.model,
    ))?
  }

  let file_content = std::fs::read(file_path)?;
  let base64_content =
    base64::engine::general_purpose::STANDARD.encode(&file_content);

  let mut prompt = "Transcribe this audio. \
    Just the transcript and no explanation!"
    .to_string();
  if !languages.is_empty() {
    prompt += &format!("\nThe audio is spoken in: {}.", languages.join(", "));
  }
  if !keywords.is_empty() {
    prompt += &format!("\nExpect these terms: {}.", keywords.join(", "));
  }

  let req_body_obj = json!({
    "contents": [{
      "parts": [
        { "text": prompt },
        {
          "inlineData": {
            "mimeType": get_audio_mime_type(file_path),
            "data": base64_content,
          }
        }
      ]
    }]
  });

  let resp = exec_request(http_req, &req_body_obj, false).await?;
  let status = resp.status();
  let resp_json = resp.json::<Value>().await?;

  if !status.is_success() {
    Err(serde_json::to_string_pretty(&resp_json).unwrap())?;
  }

  // The dedicated transcription models return `audioTranscription` parts,
  // the general purpose ones plain text parts
  let text = resp_json["candidates"][0]["content"]["parts"]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(|part| {
      part["audioTranscription"]["text"]
        .as_str()
        .or_else(|| part["text"].as_str())
    })
    .collect::<Vec<&str>>()
    .join("");

  if opts.is_raw {
    println!("{text}");
  } else {
    highlight::text_via_bat(&format!("{text}\n"));
  }

  Ok(())
}

/// Transcribe an audio file via OpenAI's `/audio/transcriptions` endpoint
/// or, for Gemini models, via Google's `generateContent` endpoint.
///
/// `languages` and `keywords` are only supported by `gpt-transcribe`.
/// For the other OpenAI models the first language is sent as the legacy
/// `language` parameter and the keywords are ignored.
pub async fn transcribe_audio_file(
  opts: &ExecOptions,
  model: &Model,
  languages: &[String],
  keywords: &[String],
  file_path: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;
  let (_used_model, http_req) =
    get_http_req(&Some(model), &secrets_path_str, &full_config)?;
  let http_req = http_req.require_api("Transcription")?;

  if http_req.provider == Provider::Google {
    return transcribe_via_gemini(
      opts, &http_req, languages, keywords, file_path,
    )
    .await;
  }

  let model_id = http_req.model.clone();
  if model_id == "gpt-live-transcribe" {
    Err(
      "`gpt-live-transcribe` is only available in realtime sessions. \
      Use `gpt-transcribe` to transcribe audio files.",
    )?
  }
  // Only `gpt-transcribe` accepts multiple languages and keyword hints
  let is_gpt_transcribe = model_id == "gpt-transcribe";
  // Speaker annotations require the diarizing model and its own response format
  let is_diarizing = model_id.ends_with("-diarize");

  let file = std::fs::read(file_path)?;
  let file_name = std::path::Path::new(file_path)
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or(file_path)
    .to_string();
  let part = reqwest::multipart::Part::bytes(file)
    .file_name(file_name)
    .mime_str(get_audio_mime_type(file_path))?;

  let mut form = reqwest::multipart::Form::new()
    .text("model", model_id.clone())
    .part("file", part);

  if is_diarizing {
    form = form
      .text("response_format", "diarized_json")
      // Required for inputs longer than 30 seconds
      .text("chunking_strategy", "auto");
  } else {
    form = form.text("response_format", "json");
  }

  if is_gpt_transcribe {
    for language in languages {
      form = form.text("languages[]", language.clone());
    }
    for keyword in keywords {
      form = form.text("keywords[]", keyword.clone());
    }
  } else {
    if let Some(language) = languages.first() {
      if languages.len() > 1 {
        eprintln!(
          "⚠️  `{model_id}` supports only one language. \
          Using '{language}' and ignoring the remaining ones."
        );
      }
      form = form.text("language", language.clone());
    }
    if !keywords.is_empty() {
      eprintln!("⚠️  `{model_id}` doesn't support keywords. Ignoring them.");
    }
  }

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
    let text = match resp_json["segments"].as_array() {
      // Prefix each segment of a diarized transcript with its speaker label
      Some(segments) if is_diarizing => segments
        .iter()
        .map(|segment| {
          format!(
            "{}: {}\n",
            segment["speaker"].as_str().unwrap_or("Unknown"),
            segment["text"].as_str().unwrap_or_default().trim(),
          )
        })
        .collect::<String>(),
      _ => format!("{}\n", resp_json["text"].as_str().unwrap_or_default()),
    };
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

fn get_image_mime_type(file_path: &str) -> &'static str {
  let lower = file_path.to_lowercase();
  if lower.ends_with(".png") {
    "image/png"
  } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
    "image/jpeg"
  } else if lower.ends_with(".webp") {
    "image/webp"
  } else if lower.ends_with(".gif") {
    "image/gif"
  } else if lower.ends_with(".heic") {
    "image/heic"
  } else if lower.ends_with(".heif") {
    "image/heif"
  } else {
    "image/png"
  }
}

pub async fn edit_images(
  opts: &ExecOptions,
  image_files: &[String],
  prompt: &str,
  background: Option<&str>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let start = Instant::now();
  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;
  let model =
    &Model::Model(Provider::OpenAI, "gpt-image-2.5-sunburst".to_string());
  let (_used_model, http_req) =
    get_http_req(&Some(model), &secrets_path_str, &full_config)?;
  let http_req = http_req.require_api("Image editing")?;
  let used_model = model_label(&http_req);

  let mut form = reqwest::multipart::Form::new()
    .text("model", http_req.model.clone())
    .text("prompt", prompt.to_string());

  if let Some(bg) = background {
    form = form.text("background", bg.to_string());
  }

  for file_path in image_files {
    let file_bytes = std::fs::read(file_path)
      .map_err(|e| format!("Failed to read image file '{file_path}': {e}"))?;
    let mime_type = get_image_mime_type(file_path);
    let file_name = std::path::Path::new(file_path)
      .file_name()
      .and_then(|n| n.to_str())
      .unwrap_or(file_path)
      .to_string();
    let part = reqwest::multipart::Part::bytes(file_bytes)
      .file_name(file_name)
      .mime_str(mime_type)?;
    form = form.part("image[]", part);
  }

  let client = reqwest::Client::new();
  let base_url =
    get_base_url(&full_config, "openai_base_url", "https://api.openai.com/v1");
  let url = format!("{base_url}/images/edits");

  let resp = client
    .post(&url)
    .bearer_auth(&http_req.api_key)
    .multipart(form)
    .send()
    .await?;

  let elapsed_millis = start.elapsed().as_millis();
  let (elapsed_time, time_unit) = format_elapsed_time(elapsed_millis);
  let subcommand = subcommand_prefix(opts);

  let status = resp.status();
  let response_json = resp.json::<Value>().await?;

  if !status.is_success() {
    let resp_formatted = serde_json::to_string_pretty(&response_json).unwrap();
    Err(cformat!(
      "<bold>{subcommand}{used_model} | ⏱️ {} {}</bold>\n\
      \n{resp_formatted}",
      elapsed_time,
      time_unit,
    ))?;
  }

  cprintln!(
    "<bold>{subcommand}{used_model} | ⏱️ {} {}</bold>\n",
    elapsed_time,
    time_unit,
  );

  if let Some(data) = response_json["data"].as_array() {
    let mut image_count = 0;
    for image_data in data {
      image_count += 1;
      if let Some(image_base64) = image_data["b64_json"].as_str() {
        match save_base64_image(image_base64, prompt) {
          Ok(filename) => println!("Edited image saved to: {filename}"),
          Err(err) => println!("Failed to save image {image_count}: {err}"),
        }
      } else if let Some(url) = image_data["url"].as_str() {
        println!("Edited image {}: {}", image_count, url);
      }
    }
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

  let model = shortcut_model(
    cmd,
    Model::Model(Provider::Anthropic, "claude-sonnet-5-5".to_string()),
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

/// Instructions shared by all `rewrite` invocations.
///
/// The emphasis on matching the input's level of markup is deliberate:
/// `cai rewrite` is meant for round-tripping text
/// (e.g. `pbpaste | cai rewrite | pbcopy`),
/// so models must neither "helpfully" add backticks, asterisks or bullet
/// points that were never in the input, nor strip the ones that were.
const REWRITE_PROMPT: &str = "\
  Fix any spelling mistakes, grammatical errors, \
    and wording issues in the following text. \
  Maintain the original meaning and tone \
    while improving clarity and correctness.\n\
  \n\
  Output rules:\n\
  - Return only the corrected text, \
    without explanations or additional commentary.\n\
  - Do not wrap the text in quotation marks or a code fence.\n\
  - Match the input's formatting exactly. \
    Reproduce its line breaks, blank lines, indentation, list markers, \
    and any markup it already uses.\n\
  - Keep every piece of existing markup. \
    If the input is Markdown, return Markdown \
    with the same headings, emphasis, links, lists, and code spans.\n\
  - Add no markup that the input does not already have. \
    Do not introduce backticks, asterisks, underscores, headings, \
    bullet points, or code fences \
    that are absent from the text you are given.";

/// Undo a code fence that the model wrapped around the whole rewritten text.
///
/// Only strips when the fence encloses *all* of `rewritten` and `original`
/// wasn't fenced itself, so a fenced input still round-trips unchanged.
fn strip_wrapping_code_fence(rewritten: &str, original: &str) -> String {
  let trimmed = rewritten.trim();

  if original.trim_start().starts_with("```") {
    return trimmed.to_string();
  }

  let Some(after_open) = trimmed.strip_prefix("```") else {
    return trimmed.to_string();
  };
  // Drop an optional language tag on the opening fence's line.
  let Some((_lang, rest)) = after_open.split_once('\n') else {
    return trimmed.to_string();
  };
  match rest.trim_end().strip_suffix("```") {
    Some(inner) => inner.trim_matches('\n').to_string(),
    None => trimmed.to_string(),
  }
}

/// Rewrite `text` with the given model and print the result verbatim,
/// so it can be piped into other commands.
pub async fn rewrite_text(
  optional_model: &Option<&Model>,
  opts: &ExecOptions,
  text: &str,
  instructions: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;
  let (_used_model, http_req) =
    get_http_req(optional_model, &secrets_path_str, &full_config)?;

  let prompt = if instructions.is_empty() {
    format!("{REWRITE_PROMPT}\n\nText to rewrite:\n{text}")
  } else {
    format!(
      "{REWRITE_PROMPT}\n\n\
      Additional instructions: {instructions}\n\n\
      Text to rewrite:\n{text}"
    )
  };

  // Non-streaming, so the response can be checked for stray code fences
  // before anything is written to stdout.
  let mut raw_opts = opts.clone();
  raw_opts.is_raw = true;
  raw_opts.is_streaming = false;

  let msg = complete_text(&http_req, &raw_opts, &prompt)
    .await
    .map_err(|err| format!("Failed to rewrite the text: {err}"))?;

  println!("{}", strip_wrapping_code_fence(&msg, text));

  Ok(())
}

/// Strip surrounding markdown code fences from a generated command.
fn strip_code_fences(text: &str) -> String {
  let trimmed = text.trim();
  if let Some(after) = trimmed.strip_prefix("```") {
    // Drop an optional language tag on the opening fence's first line.
    let after = after
      .split_once('\n')
      .map(|(_, rest)| rest)
      .unwrap_or(after);
    let inner = after.trim_end().trim_end_matches("```");
    return inner.trim().to_string();
  }
  trimmed.to_string()
}

async fn generate_command(
  http_req: &AiRequest,
  raw_opts: &ExecOptions,
  prompt: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
  let answer = complete_text(http_req, raw_opts, prompt)
    .await
    .map_err(|err| format!("Failed to generate command: {err}"))?;
  let command = strip_code_fences(&answer);

  if command.is_empty() {
    return Err("LLM returned an empty command.".into());
  }

  Ok(command)
}

/// Run `command` via `bash -c`, streaming its stdout/stderr to the terminal
/// live while also capturing them so the output can be inspected afterwards.
/// stdin is inherited so interactive commands keep working.
fn exec_and_capture(
  command: &str,
) -> std::io::Result<(std::process::ExitStatus, String, String)> {
  use std::io::Read;
  use std::process::Stdio;

  let mut child = std::process::Command::new("bash")
    .arg("-c")
    .arg(command)
    .stdin(Stdio::inherit())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;

  // Tee a child pipe to the given terminal stream while capturing the bytes.
  fn tee<R: Read + Send + 'static, W: Write + Send + 'static>(
    mut reader: R,
    mut writer: W,
  ) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
      let mut captured = Vec::new();
      let mut chunk = [0u8; 4096];
      loop {
        match reader.read(&mut chunk) {
          Ok(0) | Err(_) => break,
          Ok(n) => {
            let _ = writer.write_all(&chunk[..n]);
            let _ = writer.flush();
            captured.extend_from_slice(&chunk[..n]);
          }
        }
      }
      String::from_utf8_lossy(&captured).into_owned()
    })
  }

  let child_stdout = child.stdout.take().expect("piped stdout");
  let child_stderr = child.stderr.take().expect("piped stderr");
  let stdout_handle = tee(child_stdout, std::io::stdout());
  let stderr_handle = tee(child_stderr, std::io::stderr());

  let status = child.wait()?;
  let captured_stdout = stdout_handle.join().unwrap_or_default();
  let captured_stderr = stderr_handle.join().unwrap_or_default();
  Ok((status, captured_stdout, captured_stderr))
}

/// Keep only the last `max_chars` characters, prefixing with a marker when
/// truncated, so a large error log doesn't blow up the follow-up prompt.
fn tail_chars(text: &str, max_chars: usize) -> String {
  let chars: Vec<char> = text.chars().collect();
  if chars.len() <= max_chars {
    text.to_string()
  } else {
    let tail: String = chars[chars.len() - max_chars..].iter().collect();
    format!("[... truncated ...]\n{tail}")
  }
}

/// Resolve the on-disk paths of common shell tools so the model can tell
/// which implementation it is dealing with (GNU coreutils vs BSD, or the exact
/// package on Nix systems, where the store path encodes it). Returns a
/// formatted, newline-separated `tool: /path` list, or an empty string if
/// detection fails.
fn detect_tool_locations() -> String {
  const TOOLS: &[&str] = &[
    "bash", "sed", "awk", "grep", "find", "stat", "date", "xargs", "sort",
    "ls", "readlink", "head", "tail", "cut",
  ];
  // Only report absolute paths: a bare name means a builtin/alias/function,
  // which carries no GNU-vs-BSD signal and would just be noise.
  let script = TOOLS
    .iter()
    .map(|t| {
      format!(
        "p=$(command -v {t} 2>/dev/null); \
        case \"$p\" in /*) echo \"{t}: $p\";; esac"
      )
    })
    .collect::<Vec<_>>()
    .join("; ");

  match std::process::Command::new("bash")
    .arg("-c")
    .arg(&script)
    .output()
  {
    Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
    Err(_) => String::new(),
  }
}

pub async fn run_shell_command(
  opts: &ExecOptions,
  prompt_text: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let os = std::env::consts::OS;
  let arch = std::env::consts::ARCH;

  let tool_locations = detect_tool_locations();
  let tools_section = if tool_locations.is_empty() {
    String::new()
  } else {
    format!(
      "\n\nResolved paths of common tools on this system, provided ONLY so you \
      can pick the correct flag syntax — e.g. a `coreutils`/`findutils` (GNU) \
      path means GNU flags, a macOS `/usr/bin` path or a `*-bsd-*` path means \
      BSD flags. Do NOT put these absolute paths in the command: always invoke \
      tools by their bare name (`find`, `sort`, …) and let `PATH` resolve them:\n\
      {tool_locations}"
    )
  };

  let rules = format!(
    "Rules:\n\
    - Respond with ONLY the command. No explanations, no comments, \
      no markdown code fences.\n\
    - The command will be executed via `bash -c` on `{os}` ({arch}), \
      so use bash/POSIX syntax.\n\
    - It is fine to span multiple lines if needed for a heredoc \
      or multi-statement pipeline.{tools_section}"
  );

  let initial_prompt = format!(
    "You are a shell command generator. \
    Given the user's request, output a single shell command \
    (or pipeline) that fulfills it.\n\n\
    {rules}\n\n\
    User request: {prompt_text}"
  );

  let model = Model::Model(Provider::OpenAI, "gpt-5".to_string());
  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;
  let (_used_model, http_req) =
    get_http_req(&Some(&model), &secrets_path_str, &full_config)?;

  let mut raw_opts = opts.clone();
  raw_opts.is_raw = true;
  raw_opts.is_streaming = false;

  let mut command =
    generate_command(&http_req, &raw_opts, &initial_prompt).await?;

  loop {
    println!();
    bat::PrettyPrinter::new()
      .input_from_bytes(command.as_bytes())
      .language("bash")
      .print()
      .ok();
    println!("\n");
    print!("Execute command? [y]es, [n]o, [c]hange: ");
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let choice = input.trim().to_lowercase();

    match choice.as_str() {
      "" | "y" | "yes" | "e" | "execute" => {
        let (status, _stdout, stderr) = exec_and_capture(&command)?;
        if status.success() {
          return Ok(());
        }

        let code_str = status
          .code()
          .map(|c| c.to_string())
          .unwrap_or_else(|| "signal".to_string());
        eprintln!("\nCommand failed (exit {code_str}).");
        print!("Ask AI to fix it from the error output? [y]es, [n]o: ");
        std::io::stdout().flush()?;

        let mut fix_input = String::new();
        std::io::stdin().read_line(&mut fix_input)?;
        match fix_input.trim().to_lowercase().as_str() {
          "" | "y" | "yes" => {
            let captured_stderr = if stderr.trim().is_empty() {
              "(no stderr output)".to_string()
            } else {
              tail_chars(stderr.trim(), 4000)
            };
            let fix_prompt = format!(
              "You are a shell command generator. \
              The user originally requested: {prompt_text}\n\n\
              You generated this command:\n{command}\n\n\
              It failed with exit code {code_str} \
              and produced this stderr:\n{captured_stderr}\n\n\
              Generate a corrected shell command that fixes the error.\n\n\
              {rules}"
            );
            command =
              generate_command(&http_req, &raw_opts, &fix_prompt).await?;
          }
          _ => {
            if let Some(code) = status.code() {
              std::process::exit(code);
            }
            return Err("Command terminated by signal".into());
          }
        }
      }
      "c" | "change" => {
        print!("Describe the change: ");
        std::io::stdout().flush()?;
        let mut suggestion = String::new();
        std::io::stdin().read_line(&mut suggestion)?;
        let suggestion = suggestion.trim();
        if suggestion.is_empty() {
          eprintln!("No change suggestion provided. Keeping previous command.");
          continue;
        }
        let refine_prompt = format!(
          "You are a shell command generator. \
          The user originally requested: {prompt_text}\n\n\
          The previous command you generated was:\n{command}\n\n\
          The user wants the following change: {suggestion}\n\n\
          Generate an updated shell command.\n\n\
          {rules}"
        );
        command =
          generate_command(&http_req, &raw_opts, &refine_prompt).await?;
      }
      "a" | "n" | "no" | "abort" => {
        println!("Aborted.");
        return Ok(());
      }
      other => {
        eprintln!(
          "Unrecognized choice '{other}'. \
          Please answer with 'y', 'n', or 'c'."
        );
      }
    }
  }
}

fn select_commit_files(
  status: &str,
  staged_files: &str,
) -> (Vec<String>, bool) {
  let staged_files: Vec<String> = staged_files
    .lines()
    .filter(|file| !file.is_empty())
    .map(ToOwned::to_owned)
    .collect();

  if !staged_files.is_empty() {
    return (staged_files, true);
  }

  // Include modified files when nothing has been staged. Exclude untracked
  // files, which preserves the command's existing behavior.
  let modified_files = status
    .lines()
    .filter(|line| {
      let trimmed = line.trim();
      trimmed.starts_with("M ")
        || trimmed.starts_with("MM")
        || trimmed.starts_with("AM")
        || trimmed.starts_with(" M")
    })
    .map(|line| line[3..].to_string()) // Skip the status prefix
    .collect();

  (modified_files, false)
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

  let staged_files_output = std::process::Command::new("git")
    .args(["diff", "--cached", "--name-only"])
    .output()
    .expect("Failed to execute git diff");
  let staged_files = String::from_utf8_lossy(&staged_files_output.stdout);

  let (modified_files, has_staged_files) =
    select_commit_files(&status, &staged_files);

  if modified_files.is_empty() {
    println!("No modified files to commit.");
    return Ok(());
  }

  let file_description = if has_staged_files {
    "staged"
  } else {
    "modified"
  };
  println!(
    "Found {} {} file(s):\n",
    modified_files.len(),
    file_description
  );
  for file in &modified_files {
    println!("  - {}", file);
  }
  println!();

  // Respect an existing index selection. Otherwise, analyze all modified
  // files and stage each approved group as before.
  let diff_args = if has_staged_files {
    vec!["diff", "--cached"]
  } else {
    vec!["diff", "HEAD"]
  };
  let diff_output = std::process::Command::new("git")
    .args(diff_args)
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

  let model = Model::Model(Provider::OpenAI, "gpt-5".to_string());

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
  let content = &complete_text(&http_req, &analysis_opts, &analysis_prompt)
    .await
    .map_err(|err| format!("Failed to analyze the changes: {err}"))?;

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

    // An existing index is the user's explicit file selection. Do not expand
    // it with files from the working tree.
    if !has_staged_files {
      for file in &commit_group.files {
        let add_output = std::process::Command::new("git")
          .args(["add", file])
          .output()?;

        if !add_output.status.success() {
          eprintln!("Warning: Failed to stage {}", file);
        }
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
  let model = Model::Model(Provider::OpenAI, "gpt-5".to_string());
  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;
  let (_used_model, http_req) =
    get_http_req(&Some(&model), &secrets_path_str, &full_config)?;

  let mut raw_opts = opts.clone();
  raw_opts.is_raw = true;

  let answer = complete_text(&http_req, &raw_opts, &sql_prompt)
    .await
    .map_err(|err| format!("Failed to generate SQL: {err}"))?;
  let generated_sql = answer
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

/// How a provider authenticates requests to its `models` listing endpoint.
enum ModelsAuth {
  Bearer,
  AnthropicKey,
  GoogleQuery,
  None,
}

async fn fetch_provider_models(
  client: &reqwest::Client,
  url: &str,
  api_key: Option<String>,
  auth: ModelsAuth,
) -> Result<Vec<String>, String> {
  let resp = match auth {
    ModelsAuth::Bearer => {
      let key = api_key.ok_or("No API key configured")?;
      client.get(url).bearer_auth(&key).send().await
    }
    ModelsAuth::AnthropicKey => {
      let key = api_key.ok_or("No API key configured")?;
      client
        .get(url)
        .header("x-api-key", &key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
    }
    ModelsAuth::GoogleQuery => {
      let key = api_key.ok_or("No API key configured")?;
      client.get(format!("{url}?key={key}")).send().await
    }
    ModelsAuth::None => client.get(url).send().await,
  };

  let resp = resp.map_err(|e| format!("Request failed: {e}"))?;
  let status = resp.status();
  if !status.is_success() {
    let body = resp.text().await.unwrap_or_default();
    let snippet: String = body.chars().take(200).collect();
    return Err(format!("HTTP {status}: {snippet}"));
  }

  let json: Value = resp
    .json()
    .await
    .map_err(|e| format!("Failed to parse JSON: {e}"))?;

  // Common response shapes:
  //   {"data":   [{"id":   "..."}]}  OpenAI, Anthropic, Groq, Mistral
  //   {"models": [{"name": "..."}]}  Gemini, Ollama
  if let Some(arr) = json.get("data").and_then(|v| v.as_array()) {
    return Ok(
      arr
        .iter()
        .filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(String::from))
        .collect(),
    );
  }
  if let Some(arr) = json.get("models").and_then(|v| v.as_array()) {
    return Ok(
      arr
        .iter()
        .filter_map(|m| {
          m.get("name")
            .or_else(|| m.get("id"))
            .and_then(|n| n.as_str())
            // Gemini prefixes ids with "models/"; strip it for display.
            .map(|s| s.strip_prefix("models/").unwrap_or(s).to_string())
        })
        .collect(),
    );
  }

  Err(format!("Unexpected response shape: {json}"))
}

fn get_models_key(
  cfg: &HashMap<String, String>,
  cfg_key: &str,
  env_key: &str,
) -> Option<String> {
  cfg
    .get(cfg_key)
    .cloned()
    .or_else(|| env::var(env_key).ok())
    .filter(|s| !s.is_empty())
}

pub async fn list_models() -> Result<(), Box<dyn Error + Send + Sync>> {
  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;
  let client = reqwest::Client::new();

  let openai_url = format!(
    "{}/models",
    get_base_url(&full_config, "openai_base_url", "https://api.openai.com/v1")
  );
  let anthropic_url = format!(
    "{}/models",
    get_base_url(
      &full_config,
      "anthropic_base_url",
      "https://api.anthropic.com/v1"
    )
  );
  let groq_url = format!(
    "{}/models",
    get_base_url(
      &full_config,
      "groq_base_url",
      "https://api.groq.com/openai/v1"
    )
  );
  let gemini_url = format!(
    "{}/models",
    get_base_url(
      &full_config,
      "google_base_url",
      "https://generativelanguage.googleapis.com/v1beta"
    )
  );
  let cerebras_url = format!(
    "{}/models",
    get_base_url(
      &full_config,
      "cerebras_base_url",
      "https://api.cerebras.ai/v1"
    )
  );
  let deepseek_url = format!(
    "{}/models",
    get_base_url(
      &full_config,
      "deepseek_base_url",
      "https://api.deepseek.com"
    )
  );
  let xai_url = format!(
    "{}/models",
    get_base_url(&full_config, "xai_base_url", "https://api.x.ai/v1")
  );
  let typesafe_url = format!(
    "{}/models",
    get_base_url(
      &full_config,
      "typesafe_base_url",
      "https://api.typesafe.ai/v1"
    )
  );
  // Perplexity's chat completions live at `{base}/chat/completions` with
  // the default base lacking `/v1`, but the models endpoint sits under
  // `/v1/models`, so it's hardcoded here.
  let perplexity_url = "https://api.perplexity.ai/v1/models".to_string();
  // Ollama's models endpoint lives under `/api/tags`, not the
  // OpenAI-compatible `/v1` prefix used for chat completions.
  let ollama_base =
    get_base_url(&full_config, "ollama_base_url", "http://localhost:11434/v1");
  let ollama_host = ollama_base.trim_end_matches("/v1").to_string();
  let ollama_url = format!("{ollama_host}/api/tags");

  let providers: Vec<(&'static str, String, Option<String>, ModelsAuth)> = vec![
    (
      "OpenAI",
      openai_url,
      get_models_key(&full_config, "openai_api_key", "OPENAI_API_KEY"),
      ModelsAuth::Bearer,
    ),
    (
      "Anthropic",
      anthropic_url,
      get_models_key(&full_config, "anthropic_api_key", "ANTHROPIC_API_KEY"),
      ModelsAuth::AnthropicKey,
    ),
    (
      "Google Gemini",
      gemini_url,
      get_models_key(&full_config, "google_api_key", "GOOGLE_API_KEY"),
      ModelsAuth::GoogleQuery,
    ),
    (
      "Groq",
      groq_url,
      get_models_key(&full_config, "groq_api_key", "GROQ_API_KEY"),
      ModelsAuth::Bearer,
    ),
    (
      "Cerebras",
      cerebras_url,
      get_models_key(&full_config, "cerebras_api_key", "CEREBRAS_API_KEY"),
      ModelsAuth::Bearer,
    ),
    (
      "DeepSeek",
      deepseek_url,
      get_models_key(&full_config, "deepseek_api_key", "DEEPSEEK_API_KEY"),
      ModelsAuth::Bearer,
    ),
    (
      "xAI",
      xai_url,
      get_models_key(&full_config, "xai_api_key", "XAI_API_KEY"),
      ModelsAuth::Bearer,
    ),
    // Perplexity's models endpoint is public — no key required.
    ("Perplexity", perplexity_url, None, ModelsAuth::None),
    ("Ollama", ollama_url, None, ModelsAuth::None),
    (
      "Mistral",
      "https://api.mistral.ai/v1/models".to_string(),
      get_models_key(&full_config, "mistral_api_key", "MISTRAL_API_KEY"),
      ModelsAuth::Bearer,
    ),
    (
      "TypeSafe",
      typesafe_url,
      get_models_key(&full_config, "typesafe_api_key", "TYPESAFE_API_KEY"),
      ModelsAuth::Bearer,
    ),
  ];

  let mut handles = Vec::new();
  for (name, url, key, auth) in providers {
    let client = client.clone();
    handles.push(tokio::spawn(async move {
      (name, fetch_provider_models(&client, &url, key, auth).await)
    }));
  }

  let results = join_all(handles).await;

  for handle_result in results {
    let (provider, models_result) = handle_result?;
    cprintln!("<bold,underline>{}</bold,underline>", provider);
    match models_result {
      Ok(mut models) => {
        if models.is_empty() {
          println!("  (no models returned)");
        } else {
          models.sort();
          for m in &models {
            println!("  {m}");
          }
        }
      }
      Err(e) => cprintln!("  <red>{}</red>", e),
    }
    println!();
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn chain_for(entries: &[(&str, &str)]) -> Vec<String> {
    let full_config = entries
      .iter()
      .map(|(key, value)| (key.to_string(), value.to_string()))
      .collect();
    default_model_chain(&full_config)
      .iter()
      .map(|model| model.to_string())
      .collect()
  }

  #[test]
  fn test_default_model_chain() {
    assert_eq!(
      chain_for(&[]),
      [
        "Cerebras gpt-oss-120b",
        "OpenAI gpt-5-mini",
        "Anthropic claude-sonnet-5-5",
      ]
    );
    assert_eq!(
      chain_for(&[("openai_via", "codex"), ("anthropic_via", "claude-code")]),
      [
        "Anthropic claude-sonnet-5-5",
        "OpenAI gpt-5.6-luna",
        "Cerebras gpt-oss-120b",
      ]
    );
    assert_eq!(
      chain_for(&[
        ("openai_via", "codex"),
        ("shortcut_models.fast", "groq llama-3.1-8b-instant"),
      ]),
      [
        "Groq llama-3.1-8b-instant",
        "OpenAI gpt-5.6-luna",
        "Cerebras gpt-oss-120b",
        "Anthropic claude-sonnet-5-5",
      ]
    );
  }

  #[tokio::test]
  async fn test_submit_empty_prompt() {
    let prompt = "";
    let result = exec_tool(
      &Some(&Model::Model(Provider::OpenAI, "gpt-4o-mini".to_owned())),
      &ExecOptions::default(),
      prompt,
    )
    .await;
    assert!(result.is_err());
  }

  #[test]
  fn test_xai_image_models_use_the_images_api() {
    for model_id in ["image", "grok-image", "imagine", "image2", "quality"] {
      assert!(
        is_xai_image_model(types::get_xai_model(model_id)),
        "Alias `{model_id}` must resolve to an xAI image model"
      );
    }

    assert!(is_xai_image_model("grok-imagine-image-2.0"));
    assert!(!is_xai_image_model("grok-4"));
    assert!(!is_xai_image_model("grok-imagine-video"));
  }

  #[test]
  fn test_google_model_modalities() {
    let cases = [
      // (alias, image, video, music, tts, embedding, interactions)
      ("image", true, false, false, false, false, false),
      ("banana", true, false, false, false, false, false),
      ("pro-image", true, false, false, false, false, false),
      ("veo", false, true, false, false, false, false),
      ("video", false, true, false, false, false, false),
      ("music", false, false, true, false, false, false),
      ("lyria-clip", false, false, true, false, false, false),
      ("tts", false, false, false, true, false, false),
      ("embed", false, false, false, false, true, false),
      ("omni", false, false, false, false, false, true),
      ("gemini", false, false, false, false, false, false),
      ("gemma", false, false, false, false, false, false),
      ("transcribe", false, false, false, false, false, false),
    ];

    for (alias, image, video, music, tts, embedding, interactions) in cases {
      let model = types::get_google_model(alias);
      assert_eq!(is_google_image_model(model), image, "image: `{alias}`");
      assert_eq!(is_google_video_model(model), video, "video: `{alias}`");
      assert_eq!(is_google_music_model(model), music, "music: `{alias}`");
      assert_eq!(is_google_tts_model(model), tts, "tts: `{alias}`");
      assert_eq!(
        is_google_embedding_model(model),
        embedding,
        "embedding: `{alias}`"
      );
      assert_eq!(
        is_google_interactions_model(model),
        interactions,
        "interactions: `{alias}`"
      );
    }
  }

  #[test]
  fn test_google_media_models_bypass_text_handling() {
    let opts = ExecOptions::default();

    for alias in ["image", "banana", "veo", "music", "tts", "embed", "omni"] {
      let http_req = AiRequest {
        provider: Provider::Google,
        model: types::get_google_model(alias).to_string(),
        ..Default::default()
      };
      assert!(
        !is_text_response(&http_req, &opts),
        "Alias `{alias}` must not be handled as a text response"
      );
    }

    for alias in ["gemini", "pro", "lite", "gemma", "transcribe"] {
      let http_req = AiRequest {
        provider: Provider::Google,
        model: types::get_google_model(alias).to_string(),
        ..Default::default()
      };
      assert!(
        is_text_response(&http_req, &opts),
        "Alias `{alias}` must be handled as a text response"
      );
    }
  }

  #[test]
  fn test_google_req_bodies_per_modality() {
    let opts = ExecOptions::default();
    let body_for = |alias: &str| {
      let http_req = AiRequest {
        provider: Provider::Google,
        model: types::get_google_model(alias).to_string(),
        max_tokens: 100,
        ..Default::default()
      };
      get_req_body_obj(&opts, &http_req, "test")
    };

    // Veo uses the prediction API's instances/parameters shape
    assert_eq!(body_for("veo")["instances"][0]["prompt"], "test");

    // Embeddings take a single content object
    assert_eq!(body_for("embed")["content"]["parts"][0]["text"], "test");

    // Omni takes the prompt as a plain string
    assert_eq!(body_for("omni")["input"], "test");
    assert_eq!(body_for("omni")["model"], "gemini-omni-1.1-flash");

    // Image and audio models request their modality explicitly
    let image_config = &body_for("image")["generationConfig"];
    assert_eq!(image_config["responseModalities"][0], "IMAGE");
    assert_eq!(image_config["maxOutputTokens"], 100);

    let music_config = &body_for("music")["generationConfig"];
    assert_eq!(music_config["responseModalities"][0], "AUDIO");
    // A token limit would cut the generated audio short
    assert!(music_config["maxOutputTokens"].is_null());

    let tts_config = &body_for("tts")["generationConfig"];
    assert_eq!(tts_config["responseModalities"][0], "AUDIO");
    assert!(!tts_config["speechConfig"]["voiceConfig"].is_null());

    // Text models are unaffected
    let text_body = body_for("gemini");
    assert_eq!(text_body["contents"][0]["parts"][0]["text"], "test");
    assert_eq!(text_body["generationConfig"]["maxOutputTokens"], 100);
  }

  #[test]
  fn test_transcription_model_provider() {
    // Gemini models are served by Google
    assert_eq!(
      transcription_model("gemini-3.5-transcribe"),
      Model::Model(Provider::Google, "gemini-3.5-transcribe".to_string())
    );
    // The `google/` prefix also reaches Google's aliases
    assert_eq!(
      transcription_model("google/transcribe"),
      Model::Model(Provider::Google, "transcribe".to_string())
    );
    // Everything else stays with OpenAI
    assert_eq!(
      transcription_model("diarize"),
      Model::Model(Provider::OpenAI, "diarize".to_string())
    );
  }

  #[test]
  fn test_media_extension_for_mime() {
    assert_eq!(media_extension_for_mime("image/jpeg"), "jpg");
    assert_eq!(media_extension_for_mime("audio/mpeg"), "mp3");
    assert_eq!(media_extension_for_mime("video/mp4"), "mp4");
    // Raw PCM is wrapped in a WAV container before it's written
    assert_eq!(
      media_extension_for_mime("audio/L16; rate=24000; channels=1"),
      "wav"
    );
    assert_eq!(media_extension_for_mime("image/png"), "png");
  }

  #[test]
  fn test_mime_param() {
    let mime = "audio/L16; rate=24000; channels=1";
    assert_eq!(mime_param(mime, "rate"), Some(24_000));
    assert_eq!(mime_param(mime, "channels"), Some(1));
    assert_eq!(mime_param(mime, "codec"), None);
    assert_eq!(mime_param("audio/mpeg", "rate"), None);
  }

  #[test]
  fn test_pcm_to_wav() {
    let pcm = [0x01, 0x00, 0xff, 0x7f];
    let wav = pcm_to_wav(&pcm, 24_000, 1);

    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    // Byte rate: 24000 Hz × 1 channel × 2 bytes per sample
    assert_eq!(
      u32::from_le_bytes([wav[28], wav[29], wav[30], wav[31]]),
      48_000
    );
    assert_eq!(&wav[36..40], b"data");
    assert_eq!(&wav[44..], &pcm);
  }

  #[test]
  fn test_image_extension() {
    assert_eq!(image_extension(&[0xff, 0xd8, 0xff, 0xe0]), "jpg");
    assert_eq!(image_extension(b"GIF89a"), "gif");
    assert_eq!(image_extension(b"RIFF\0\0\0\0WEBPVP8 "), "webp");
    assert_eq!(image_extension(&[0x89, b'P', b'N', b'G']), "png");
  }

  #[test]
  fn test_strip_wrapping_code_fence() {
    // A fence around the whole response is an artifact and gets removed
    assert_eq!(
      strip_wrapping_code_fence("```\nHello world\n```", "Helo world"),
      "Hello world"
    );
    assert_eq!(
      strip_wrapping_code_fence("```text\nHello world\n```", "Helo world"),
      "Hello world"
    );

    // Plain text is passed through (modulo surrounding whitespace)
    assert_eq!(
      strip_wrapping_code_fence("  Hello world\n", "Helo world"),
      "Hello world"
    );

    // A fenced input keeps its fence
    assert_eq!(
      strip_wrapping_code_fence(
        "```\nHello world\n```",
        "```\nHelo world\n```"
      ),
      "```\nHello world\n```"
    );

    // A fence that doesn't enclose everything is left alone
    assert_eq!(
      strip_wrapping_code_fence("```\ncode\n```\nAnd prose.", "…"),
      "```\ncode\n```\nAnd prose."
    );
  }

  #[test]
  fn test_o_models_use_max_completion_tokens() {
    let test_cases = vec![
      ("o3", true),
      ("o4-mini", true),
      ("gpt-5", true),
      ("gpt-5-mini", true),
      ("gpt-5-nano", true),
      ("gpt-6-astra", true),
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

      let opts = ExecOptions::default();

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
  fn test_parse_model_override() {
    assert_eq!(
      parse_model_override("anthropic claude-opus-4-8"),
      Some(Model::Model(
        Provider::Anthropic,
        "claude-opus-4-8".to_string()
      ))
    );
    // Case-insensitive provider, surrounding whitespace trimmed
    assert_eq!(
      parse_model_override("  OpenAI   gpt-4.1  "),
      Some(Model::Model(Provider::OpenAI, "gpt-4.1".to_string()))
    );
    // Model id may itself contain a slash
    assert_eq!(
      parse_model_override("groq openai/gpt-oss-120b"),
      Some(Model::Model(
        Provider::Groq,
        "openai/gpt-oss-120b".to_string()
      ))
    );
    // Bare provider with no model id (e.g. llamafile)
    assert_eq!(
      parse_model_override("llamafile"),
      Some(Model::Model(Provider::Llamafile, String::new()))
    );
    // Unknown provider is rejected
    assert_eq!(parse_model_override("acme some-model"), None);
    assert_eq!(parse_model_override("not-a-provider"), None);
  }

  #[test]
  fn test_flatten_config_value() {
    // A nested `shortcut_models` table is flattened into dotted keys, scalars
    // are stringified, and arrays/nulls are dropped.
    let shortcut_models = config::Value::from(HashMap::from([
      ("fast".to_string(), "groq llama".to_string()),
      ("opus".to_string(), "openai gpt-4.1".to_string()),
    ]));
    let mut out = HashMap::new();
    flatten_config_value("openai_api_key", "secret".into(), &mut out);
    flatten_config_value("shortcut_models", shortcut_models, &mut out);

    assert_eq!(
      out.get("openai_api_key").map(String::as_str),
      Some("secret")
    );
    assert_eq!(
      out.get("shortcut_models.fast").map(String::as_str),
      Some("groq llama")
    );
    assert_eq!(
      out.get("shortcut_models.opus").map(String::as_str),
      Some("openai gpt-4.1")
    );
  }

  #[test]
  fn test_get_audio_mime_type() {
    assert_eq!(get_audio_mime_type("talk.mp3"), "audio/mpeg");
    assert_eq!(get_audio_mime_type("/tmp/Talk.WAV"), "audio/wav");
    assert_eq!(get_audio_mime_type("talk.flac"), "audio/flac");
    assert_eq!(get_audio_mime_type("talk.ogg"), "audio/ogg");
    assert_eq!(get_audio_mime_type("talk.webm"), "audio/webm");
    assert_eq!(get_audio_mime_type("talk.m4a"), "audio/mp4");
    // Unknown and missing extensions fall back to mp3
    assert_eq!(get_audio_mime_type("talk.xyz"), "audio/mpeg");
    assert_eq!(get_audio_mime_type("talk"), "audio/mpeg");
  }

  #[test]
  fn test_transcription_model_aliases() {
    assert_eq!(types::get_openai_model("transcribe"), "gpt-transcribe");
    assert_eq!(
      types::get_openai_model("diarize"),
      "gpt-4o-transcribe-diarize"
    );
    assert_eq!(types::get_openai_model("whisper"), "whisper-1");
  }

  #[test]
  fn test_config_key_overridable_and_not() {
    // Shortcut with a default model is overridable
    assert_eq!(
      Commands::ClaudeOpus { prompt: vec![] }.config_key(),
      Some("opus")
    );
    assert_eq!(Commands::Py { prompt: vec![] }.config_key(), Some("py"));
    assert_eq!(
      Commands::Transcribe {
        model: None,
        languages: vec![],
        keywords: vec![],
        file: "audio.mp3".to_string(),
      }
      .config_key(),
      Some("transcribe")
    );
    // Commands taking an explicit model arg are not overridable
    assert_eq!(
      Commands::Anthropic {
        model: "x".to_string(),
        prompt: vec![]
      }
      .config_key(),
      None
    );
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

  #[test]
  fn test_select_commit_files_prefers_staged_files() {
    let status = "M  staged.rs\n M unstaged.rs\n?? untracked.rs\n";
    let staged_files = "staged.rs\n";

    assert_eq!(
      select_commit_files(status, staged_files),
      (vec!["staged.rs".to_string()], true)
    );
  }
}
