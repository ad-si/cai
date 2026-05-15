//! Tool registry for the agent: definitions sent to the model and
//! dispatch from tool name → implementation.

mod edit;
mod glob;
mod grep;
mod list;
mod read;
mod user_instruct;
mod web_fetch;
mod web_search;

use std::error::Error;

use serde_json::{json, Value};

use super::AgentContext;

pub type ToolResult = Result<String, Box<dyn Error + Send + Sync>>;

/// Names of tools that mutate state outside the model — used by the
/// `edit` permission mode to decide whether to prompt.
pub fn is_mutating(name: &str) -> bool {
  matches!(name, "Edit" | "UserInstruct")
}

/// JSON Schemas sent to Anthropic. Tools that depend on optional config
/// (WebSearch needs a Perplexity key) are skipped when unavailable so the
/// model doesn't try to call something that can't run.
pub fn definitions(ctx: &AgentContext) -> Vec<Value> {
  let mut defs = vec![
    list::definition(),
    glob::definition(),
    grep::definition(),
    read::definition(),
    edit::definition(),
    web_fetch::definition(),
    user_instruct::definition(),
  ];
  if ctx.perplexity_req.is_some() {
    defs.push(web_search::definition());
  }
  defs
}

pub async fn dispatch(
  name: &str,
  input: &Value,
  ctx: &mut AgentContext,
) -> ToolResult {
  match name {
    "List" => list::execute(input, ctx).await,
    "Glob" => glob::execute(input, ctx).await,
    "Grep" => grep::execute(input, ctx).await,
    "Read" => read::execute(input, ctx).await,
    "Edit" => edit::execute(input, ctx).await,
    "WebFetch" => web_fetch::execute(input, ctx).await,
    "WebSearch" => web_search::execute(input, ctx).await,
    "UserInstruct" => user_instruct::execute(input, ctx).await,
    other => Err(format!("Unknown tool: {other}").into()),
  }
}

/// Helper: build a tool definition object in Anthropic's expected shape.
pub(super) fn def(name: &str, description: &str, schema: Value) -> Value {
  json!({
    "name": name,
    "description": description,
    "input_schema": schema,
  })
}

/// Resolve a path coming from the model relative to the agent's CWD and
/// reject anything that escapes the working tree.
pub(super) fn resolve_path(
  ctx: &AgentContext,
  raw: &str,
) -> Result<std::path::PathBuf, Box<dyn Error + Send + Sync>> {
  let p = std::path::PathBuf::from(raw);
  let candidate = if p.is_absolute() { p } else { ctx.cwd.join(&p) };
  // canonicalize requires the path to exist; for tools that may target a
  // missing path, callers fall back to lexical normalization.
  let resolved = candidate
    .canonicalize()
    .unwrap_or_else(|_| lexical_normalize(&candidate));
  if !resolved.starts_with(&ctx.cwd_canon) {
    return Err(
      format!("Path '{}' is outside the working directory.", raw).into(),
    );
  }
  Ok(resolved)
}

fn lexical_normalize(p: &std::path::Path) -> std::path::PathBuf {
  let mut out = std::path::PathBuf::new();
  for comp in p.components() {
    match comp {
      std::path::Component::ParentDir => {
        out.pop();
      }
      std::path::Component::CurDir => {}
      other => out.push(other.as_os_str()),
    }
  }
  out
}
