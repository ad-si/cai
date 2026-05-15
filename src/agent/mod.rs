//! Agentic loop with tool use.
//!
//! `cai agent <prompt>` keeps a Claude conversation going, letting the
//! model call local tools (List, Glob, Grep, Read, Edit, WebFetch,
//! WebSearch, UserInstruct) until it produces a final answer.

mod agent_loop;
mod permissions;
mod tools;

use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::time::{Instant, SystemTime};

use crate::{get_full_config, AiRequest, Model, Provider};
use color_print::cprintln;

pub use permissions::Permission;

/// Per-invocation state shared across tool calls.
pub(crate) struct AgentContext {
  pub permission: Permission,
  pub cwd: PathBuf,
  /// Canonicalized `cwd`, cached so tool calls don't re-syscall.
  pub cwd_canon: PathBuf,
  /// Files the agent has Read this session (path → mtime+size when read).
  /// Edit consults this so the read-before-edit invariant holds.
  pub read_files: HashMap<PathBuf, ReadState>,
  /// 15-minute response cache for WebFetch.
  pub web_cache: HashMap<String, (Instant, String)>,
  /// Anthropic request template (URL, API key, model id).
  pub anthropic_req: AiRequest,
  /// Cheap model used by WebFetch's extraction step.
  pub fetch_helper_req: Option<AiRequest>,
  /// Perplexity request template, if WebSearch is configured.
  pub perplexity_req: Option<AiRequest>,
}

pub(crate) struct ReadState {
  pub mtime: SystemTime,
  pub size: u64,
}

const DEFAULT_AGENT_MODEL: &str = "claude-sonnet-4-6";
const FETCH_HELPER_MODEL: &str = "claude-haiku-4-5";

pub async fn run_agent(
  opts: &crate::ExecOptions,
  ask_for_permission_flag: Option<&str>,
  prompt: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let secrets_path_str = crate::get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;

  // Resolve mode: CLI flag > config file > default `always`.
  let permission = match ask_for_permission_flag
    .map(|s| s.to_string())
    .or_else(|| full_config.get("agent_ask_for_permission").cloned())
    .as_deref()
  {
    Some("never") => Permission::Never,
    Some("edit") => Permission::Edit,
    Some("always") | None => Permission::Always,
    Some(other) => {
      return Err(
        format!(
          "Invalid ask_for_permission value '{other}'. \
          Use always | edit | never."
        )
        .into(),
      );
    }
  };

  // Anthropic Sonnet for the main loop.
  let agent_model =
    Model::Model(Provider::Anthropic, DEFAULT_AGENT_MODEL.to_string());
  let (used_model, mut anthropic_req) =
    crate::get_http_req(&Some(&agent_model), &secrets_path_str, &full_config)?;
  anthropic_req.max_tokens = 8192;

  // Cheap helper for WebFetch extraction. If the user has no Anthropic key
  // here, WebFetch will still work via fall-through error inside the tool.
  let helper_model =
    Model::Model(Provider::Anthropic, FETCH_HELPER_MODEL.to_string());
  let fetch_helper_req =
    crate::get_http_req(&Some(&helper_model), &secrets_path_str, &full_config)
      .ok()
      .map(|(_, mut r)| {
        r.max_tokens = 4096;
        r
      });

  // Perplexity for WebSearch — optional.
  let perplexity_model =
    Model::Model(Provider::Perplexity, "sonar".to_string());
  let perplexity_req = crate::get_http_req(
    &Some(&perplexity_model),
    &secrets_path_str,
    &full_config,
  )
  .ok()
  .map(|(_, r)| r);

  let cwd = std::env::current_dir()?;
  let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
  let mut ctx = AgentContext {
    permission,
    cwd,
    cwd_canon,
    read_files: HashMap::new(),
    web_cache: HashMap::new(),
    anthropic_req,
    fetch_helper_req,
    perplexity_req,
  };

  if !opts.is_raw {
    cprintln!("<bold>🤖 cai agent | {used_model}</bold>\n");
  }

  agent_loop::run(&mut ctx, prompt, opts.is_raw).await
}
