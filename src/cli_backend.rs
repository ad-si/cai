//! Route text prompts through the official Codex and Claude Code CLIs,
//! so that they're covered by a ChatGPT or Claude subscription
//! instead of being billed to an API key.
//! Prompts for Apple's on-device model are routed through `apfel`
//! (https://github.com/Arthur-Ficial/apfel).
//!
//! Codex and Claude Code are coding agents. To make them behave like a plain model call,
//! their system prompts are replaced, all tools are disabled,
//! and neither user configuration nor project instructions are loaded.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};

use base64::Engine;
use serde_derive::Serialize;
use serde_json::Value;
use tokio::io::AsyncWriteExt;

use crate::Provider;

/// How requests to a provider are delivered
#[derive(Serialize, Debug, PartialEq, Default, Clone, Copy)]
pub enum Backend {
  /// The provider's HTTP API, billed to the API key
  #[default]
  Api,
  /// `codex exec`, billed to the ChatGPT subscription
  Codex,
  /// `claude -p`, billed to the Claude subscription
  ClaudeCode,
  /// `apfel`, Apple's on-device model
  Apfel,
}

impl std::fmt::Display for Backend {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Backend::Api => write!(f, "API"),
      Backend::Codex => write!(f, "Codex"),
      Backend::ClaudeCode => write!(f, "Claude Code"),
      Backend::Apfel => write!(f, "apfel"),
    }
  }
}

impl Backend {
  /// The config key that selects the backend, e.g. `openai_api_key`
  fn api_key_name(provider: Provider) -> &'static str {
    match provider {
      Provider::Anthropic => "anthropic_api_key",
      _ => "openai_api_key",
    }
  }

  /// Message for a request the CLI can't serve and no API key is set
  pub fn unsupported_msg(&self, provider: Provider, feature: &str) -> String {
    if *self == Backend::Apfel {
      return format!(
        "{feature} isn't supported by {provider}'s on-device model."
      );
    }
    format!(
      "{feature} isn't supported via {self}. \
      Set `{}` to use the {provider} API for it.",
      Self::api_key_name(provider),
    )
  }
}

/// The backend configured for a provider via `<provider>_via`
/// (e.g. `openai_via: codex` or `anthropic_via: claude-code`).
/// Apple's on-device model is only available via `apfel`.
pub fn configured_backend(
  full_config: &HashMap<String, String>,
  provider: Provider,
) -> Result<Backend, String> {
  let (key, cli_name, cli_backend) = match provider {
    Provider::OpenAI => ("openai_via", "codex", Backend::Codex),
    Provider::Anthropic => {
      ("anthropic_via", "claude-code", Backend::ClaudeCode) //
    }
    Provider::Apple => return Ok(Backend::Apfel),
    _ => return Ok(Backend::Api),
  };

  match full_config
    .get(key)
    .map(|value| value.trim().to_lowercase())
  {
    None => Ok(Backend::Api),
    Some(value) if value.is_empty() || value == "api" => Ok(Backend::Api),
    Some(value) if value == cli_name => Ok(cli_backend),
    Some(value) => Err(format!(
      "Invalid value '{value}' for `{key}`. Use `api` or `{cli_name}`."
    )),
  }
}

/// Why a CLI call failed
#[derive(Debug)]
pub enum CliError {
  /// The subscription doesn't offer the requested model
  UnsupportedModel(String),
  /// Any other failure (not installed, not logged in, usage limit, …)
  Failed(String),
}

impl std::fmt::Display for CliError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      CliError::UnsupportedModel(msg) | CliError::Failed(msg) => {
        write!(f, "{msg}")
      }
    }
  }
}

/// Replaces the coding agent prompts of both CLIs
const SYSTEM_PROMPT: &str = "You are a helpful assistant. \
  Answer the user's request directly. \
  You have no tools, so don't offer to run commands, \
  read files, or browse the web.";

/// An image file attached to a prompt
#[derive(Debug, Clone, Copy)]
pub struct Image<'a> {
  pub path: &'a str,
  pub mime_type: &'a str,
}

/// Send `prompt` (and `images`) to `model_id` via the CLI of `backend`
/// and return the model's answer.
///
/// `json_schema` is the bare JSON schema the answer must conform to.
/// The answer is then the serialized JSON object.
pub async fn complete(
  backend: Backend,
  model_id: &str,
  prompt: &str,
  images: &[Image<'_>],
  json_schema: Option<&Value>,
) -> Result<String, CliError> {
  match backend {
    Backend::Codex => {
      complete_via_codex(model_id, prompt, images, json_schema).await
    }
    Backend::ClaudeCode => {
      complete_via_claude_code(model_id, prompt, images, json_schema).await
    }
    Backend::Apfel => complete_via_apfel(prompt, images, json_schema).await,
    Backend::Api => Err(CliError::Failed(
      "The API backend has no CLI".to_string(), //
    )),
  }
}

/// A file in the temp directory that is deleted when dropped
struct TempFile(std::path::PathBuf);

impl TempFile {
  fn new(name: &str, content: &str) -> Result<Self, CliError> {
    // Distinguishes concurrent calls of the same process (e.g. `cai all`)
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
      "cai-{}-{}-{name}",
      std::process::id(),
      COUNTER.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::write(&path, content).map_err(|err| {
      CliError::Failed(format!("Failed to write {}: {err}", path.display()))
    })?;
    Ok(TempFile(path))
  }

  /// The path as a TOML string literal, as expected by `codex -c`
  fn toml_str(&self) -> String {
    // JSON strings are valid TOML basic strings
    serde_json::to_string(&self.0.to_string_lossy()).unwrap_or_default()
  }
}

impl Drop for TempFile {
  fn drop(&mut self) {
    let _ = std::fs::remove_file(&self.0);
  }
}

/// Run `program` with `args` in the temp directory,
/// pass `prompt` via stdin, and return its stdout, stderr, and success.
/// `not_found_hint` explains what to do if `program` isn't installed.
async fn run_cli(
  program: &str,
  not_found_hint: &str,
  args: &[String],
  removed_env_vars: &[&str],
  prompt: &str,
) -> Result<(String, String, bool), CliError> {
  let mut command = tokio::process::Command::new(program);
  command
    .args(args)
    // Keeps the CLI from picking up instructions of the current project
    .current_dir(std::env::temp_dir())
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true);
  // API keys in the environment would take precedence over the subscription
  for var in removed_env_vars {
    command.env_remove(var);
  }

  let mut child = command.spawn().map_err(|err| {
    CliError::Failed(if err.kind() == std::io::ErrorKind::NotFound {
      format!("`{program}` was not found. {not_found_hint}")
    } else {
      format!("Failed to start `{program}`: {err}")
    })
  })?;

  if let Some(mut stdin) = child.stdin.take() {
    stdin.write_all(prompt.as_bytes()).await.map_err(|err| {
      CliError::Failed(format!(
        "Failed to pass the prompt to `{program}`: {err}"
      ))
    })?;
    // Dropping stdin closes it, so the CLI stops waiting for more input
  }

  let output = child.wait_with_output().await.map_err(|err| {
    CliError::Failed(format!("Failed to run `{program}`: {err}"))
  })?;

  Ok((
    String::from_utf8_lossy(&output.stdout).into_owned(),
    String::from_utf8_lossy(&output.stderr).trim().to_string(),
    output.status.success(),
  ))
}

/// Extract the readable message from an error,
/// which Codex reports as the raw JSON of the API's error response.
fn codex_error_message(message: &str) -> String {
  serde_json::from_str::<Value>(message)
    .ok()
    .and_then(|json| json["error"]["message"].as_str().map(str::to_string))
    .unwrap_or_else(|| message.to_string())
}

/// Parse the JSONL event stream of `codex exec --json` into the final answer
fn parse_codex_events(
  stdout: &str,
  stderr: &str,
  is_success: bool,
) -> Result<String, CliError> {
  let mut answer = None;
  let mut failure = None;

  for event in stdout
    .lines()
    .filter_map(|line| serde_json::from_str::<Value>(line).ok())
  {
    match event["type"].as_str() {
      Some("item.completed")
        if event["item"]["type"].as_str() == Some("agent_message") =>
      {
        answer = event["item"]["text"].as_str().map(str::to_string);
      }
      Some("turn.failed") => {
        failure = event["error"]["message"].as_str().map(codex_error_message);
      }
      Some("error") if failure.is_none() => {
        failure = event["message"].as_str().map(codex_error_message);
      }
      _ => {}
    }
  }

  match (answer, failure) {
    (Some(answer), None) if is_success => Ok(answer),
    (_, Some(msg)) if msg.contains("not supported when using Codex") => {
      Err(CliError::UnsupportedModel(msg))
    }
    (_, Some(msg)) => Err(CliError::Failed(msg)),
    _ if !stderr.is_empty() => Err(CliError::Failed(stderr.to_string())),
    _ => Err(CliError::Failed("Codex returned no answer".to_string())),
  }
}

async fn complete_via_codex(
  model_id: &str,
  prompt: &str,
  images: &[Image<'_>],
  json_schema: Option<&Value>,
) -> Result<String, CliError> {
  let instructions = TempFile::new("instructions.md", SYSTEM_PROMPT)?;
  let schema_file = json_schema
    .map(|schema| TempFile::new("schema.json", &schema.to_string()))
    .transpose()?;
  let temp_dir = std::env::temp_dir().to_string_lossy().into_owned();

  let mut args: Vec<String> = [
    "exec",
    "--ephemeral",
    "--skip-git-repo-check",
    "--sandbox",
    "read-only",
    // The user's config could add MCP servers, hooks, or another model
    "--ignore-user-config",
    "--ignore-rules",
    "--cd",
    &temp_dir,
    "--model",
    model_id,
    "--config",
    &format!("model_instructions_file={}", instructions.toml_str()),
    "--config",
    "project_doc_max_bytes=0",
    "--config",
    "web_search=\"disabled\"",
    "--config",
    "include_permissions_instructions=false",
    "--config",
    "include_apps_instructions=false",
    "--config",
    "include_collaboration_mode_instructions=false",
    "--config",
    "include_environment_context=false",
    "--disable",
    "shell_tool",
    "--disable",
    "unified_exec",
    "--disable",
    "view_image",
    "--disable",
    "apply_patch_freeform",
    "--disable",
    "multi_agent",
    "--json",
  ]
  .iter()
  .map(|arg| arg.to_string())
  .collect();

  if let Some(schema_file) = &schema_file {
    args.push("--output-schema".to_string());
    args.push(schema_file.0.to_string_lossy().into_owned());
  }
  for image in images {
    // Codex runs in the temp directory and silently skips missing images
    let path = std::fs::canonicalize(image.path).map_err(|err| {
      CliError::Failed(format!("Failed to read {}: {err}", image.path))
    })?;
    args.push("--image".to_string());
    args.push(path.to_string_lossy().into_owned());
  }
  // Read the prompt from stdin, which has no length limit
  args.push("-".to_string());

  let (stdout, stderr, is_success) = run_cli(
    "codex",
    "Install it, or remove `openai_via` from the cai config \
    to use the API instead.",
    &args,
    &["OPENAI_API_KEY", "CODEX_API_KEY"],
    prompt,
  )
  .await?;

  parse_codex_events(&stdout, &stderr, is_success)
}

/// Parse the event stream of `claude -p --output-format stream-json`
/// into the answer, which is part of the final `result` event.
fn parse_claude_code_result(
  stdout: &str,
  stderr: &str,
  is_structured: bool,
) -> Result<String, CliError> {
  let Some(result) = stdout
    .lines()
    .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    .rfind(|event| event["type"].as_str() == Some("result"))
  else {
    let msg = if stderr.is_empty() {
      stdout.trim()
    } else {
      stderr
    };
    return Err(CliError::Failed(format!("Claude Code failed: {msg}")));
  };

  let text = result["result"].as_str().unwrap_or_default().to_string();

  if result["is_error"].as_bool().unwrap_or(false) {
    return Err(if result["api_error_status"].as_u64() == Some(404) {
      CliError::UnsupportedModel(text)
    } else {
      CliError::Failed(text)
    });
  }

  match &result["structured_output"] {
    Value::Null if is_structured => Err(CliError::Failed(format!(
      "Claude Code returned no structured output: {text}"
    ))),
    Value::Null => Ok(text),
    structured => Ok(structured.to_string()),
  }
}

/// The user message for `claude -p --input-format stream-json`,
/// which is the only way to pass images without enabling tools.
fn claude_code_message(
  prompt: &str,
  images: &[Image<'_>],
) -> Result<String, CliError> {
  let mut content = vec![serde_json::json!({ "type": "text", "text": prompt })];

  for image in images {
    let bytes = std::fs::read(image.path).map_err(|err| {
      CliError::Failed(format!("Failed to read {}: {err}", image.path))
    })?;
    content.push(serde_json::json!({
      "type": "image",
      "source": {
        "type": "base64",
        "media_type": image.mime_type,
        "data": base64::engine::general_purpose::STANDARD.encode(bytes),
      },
    }));
  }

  let message = serde_json::json!({
    "type": "user",
    "message": { "role": "user", "content": content },
  });
  Ok(format!("{message}\n"))
}

async fn complete_via_claude_code(
  model_id: &str,
  prompt: &str,
  images: &[Image<'_>],
  json_schema: Option<&Value>,
) -> Result<String, CliError> {
  let mut args: Vec<String> = [
    "--print",
    "--model",
    model_id,
    "--system-prompt",
    SYSTEM_PROMPT,
    "--tools",
    "",
    // Skip the user's settings, hooks, MCP servers, and skills
    "--setting-sources",
    "",
    "--strict-mcp-config",
    "--disable-slash-commands",
    "--no-session-persistence",
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    // Required by `--output-format stream-json`
    "--verbose",
  ]
  .iter()
  .map(|arg| arg.to_string())
  .collect();

  if let Some(schema) = json_schema {
    args.push("--json-schema".to_string());
    args.push(schema.to_string());
  }

  let (stdout, stderr, _is_success) = run_cli(
    "claude",
    "Install it, or remove `anthropic_via` from the cai config \
    to use the API instead.",
    &args,
    &["ANTHROPIC_API_KEY"],
    &claude_code_message(prompt, images)?,
  )
  .await?;

  parse_claude_code_result(&stdout, &stderr, json_schema.is_some())
}

async fn complete_via_apfel(
  prompt: &str,
  images: &[Image<'_>],
  json_schema: Option<&Value>,
) -> Result<String, CliError> {
  if !images.is_empty() {
    return Err(CliError::Failed(
      "Images aren't supported by Apple's on-device model".to_string(),
    ));
  }
  let schema_file = json_schema
    .map(|schema| TempFile::new("schema.json", &schema.to_string()))
    .transpose()?;

  let mut args: Vec<String> = vec!["--quiet".into(), "--no-color".into()];
  if let Some(schema_file) = &schema_file {
    args.push("--schema".to_string());
    args.push(schema_file.0.to_string_lossy().into_owned());
  }

  // Without a prompt argument, apfel reads the prompt from stdin
  let (stdout, stderr, is_success) = run_cli(
    "apfel",
    "Install it with `brew install apfel` (requires macOS 26+).",
    &args,
    &[],
    prompt,
  )
  .await?;

  match (is_success, stderr.is_empty()) {
    (true, _) => Ok(stdout.trim_end().to_string()),
    (false, false) => Err(CliError::Failed(stderr)),
    (false, true) => {
      Err(CliError::Failed(format!("apfel failed: {}", stdout.trim())))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn config(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
      .iter()
      .map(|(key, value)| (key.to_string(), value.to_string()))
      .collect()
  }

  #[test]
  fn test_configured_backend() {
    let cfg =
      config(&[("openai_via", "Codex"), ("anthropic_via", "claude-code")]);
    assert_eq!(
      configured_backend(&cfg, Provider::OpenAI),
      Ok(Backend::Codex)
    );
    assert_eq!(
      configured_backend(&cfg, Provider::Anthropic),
      Ok(Backend::ClaudeCode)
    );
    assert_eq!(configured_backend(&cfg, Provider::Groq), Ok(Backend::Api));
    assert_eq!(
      configured_backend(&config(&[]), Provider::Apple),
      Ok(Backend::Apfel)
    );
    assert_eq!(
      configured_backend(&config(&[]), Provider::OpenAI),
      Ok(Backend::Api)
    );
    assert_eq!(
      configured_backend(&config(&[("openai_via", "api")]), Provider::OpenAI),
      Ok(Backend::Api)
    );
    assert!(configured_backend(
      &config(&[("openai_via", "claude-code")]),
      Provider::OpenAI
    )
    .is_err());
  }

  #[test]
  fn test_parse_codex_events_answer() {
    let stdout = r#"{"type":"thread.started","thread_id":"1"}
{"type":"item.completed","item":{"id":"item_0","type":"error","message":"Model metadata not found"}}
{"type":"turn.started"}
{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"13.8 billion years"}}
{"type":"turn.completed","usage":{"input_tokens":1}}"#;
    assert_eq!(
      parse_codex_events(stdout, "", true).unwrap(),
      "13.8 billion years"
    );
  }

  #[test]
  fn test_parse_codex_events_unsupported_model() {
    let stdout = r#"{"type":"turn.started"}
{"type":"error","message":"{\"type\":\"error\",\"status\":400,\"error\":{\"type\":\"invalid_request_error\",\"message\":\"The 'gpt-5-mini' model is not supported when using Codex with a ChatGPT account.\"}}"}
{"type":"turn.failed","error":{"message":"{\"type\":\"error\",\"status\":400,\"error\":{\"type\":\"invalid_request_error\",\"message\":\"The 'gpt-5-mini' model is not supported when using Codex with a ChatGPT account.\"}}"}}"#;
    match parse_codex_events(stdout, "", false) {
      Err(CliError::UnsupportedModel(msg)) => assert_eq!(
        msg,
        "The 'gpt-5-mini' model is not supported \
        when using Codex with a ChatGPT account."
      ),
      other => panic!("Unexpected result: {other:?}"),
    }
  }

  #[test]
  fn test_parse_codex_events_failure_without_events() {
    match parse_codex_events("", "Not logged in", false) {
      Err(CliError::Failed(msg)) => assert_eq!(msg, "Not logged in"),
      other => panic!("Unexpected result: {other:?}"),
    }
  }

  #[test]
  fn test_parse_claude_code_result() {
    let stdout = r#"{"type":"system","subtype":"init"}
{"type":"assistant","message":{"content":[{"type":"text","text":"13.8 billion years"}]}}
{"type":"result","is_error":false,"result":"13.8 billion years"}"#;
    assert_eq!(
      parse_claude_code_result(stdout, "", false).unwrap(),
      "13.8 billion years"
    );

    let structured = r#"{"type":"result","is_error":false,"result":"{\"years\": \"13.8\"}","structured_output":{"years":"13.8"}}"#;
    assert_eq!(
      parse_claude_code_result(structured, "", true).unwrap(),
      r#"{"years":"13.8"}"#
    );

    let unknown_model = r#"{"type":"result","is_error":true,"api_error_status":404,"result":"There's an issue with the selected model"}"#;
    assert!(matches!(
      parse_claude_code_result(unknown_model, "", false),
      Err(CliError::UnsupportedModel(_))
    ));

    assert!(matches!(
      parse_claude_code_result("", "Invalid API key", false),
      Err(CliError::Failed(_))
    ));
  }

  #[test]
  fn test_claude_code_message() {
    let message = claude_code_message("Hi", &[]).unwrap();
    assert!(message.ends_with('\n'));
    assert_eq!(
      serde_json::from_str::<Value>(&message).unwrap(),
      serde_json::json!({
        "type": "user",
        "message": {
          "role": "user",
          "content": [{ "type": "text", "text": "Hi" }],
        },
      })
    );
  }
}
