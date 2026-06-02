use serde_json::{json, Value};

use super::super::AgentContext;
use super::ToolResult;

const CAP: usize = 100;

pub fn definition() -> Value {
  super::def(
    "Glob",
    "Find files by name pattern. Supports `**` for recursive matching \
    (e.g. `**/*.rs`, `src/**/*.ts`, `*.{json,yaml}`). Results are sorted \
    by mtime descending and capped at 100 — narrow the pattern if \
    `truncated: true`.",
    json!({
      "type": "object",
      "properties": {
        "pattern": {
          "type": "string",
          "description": "Glob pattern, evaluated against the working directory."
        }
      },
      "required": ["pattern"]
    }),
  )
}

pub async fn execute(input: &Value, ctx: &mut AgentContext) -> ToolResult {
  let pattern = input["pattern"]
    .as_str()
    .ok_or("Missing 'pattern' parameter")?;
  let mut builder = globset::GlobSetBuilder::new();
  builder
    .add(globset::Glob::new(pattern).map_err(|e| format!("Bad glob: {e}"))?);
  let set = builder.build().map_err(|e| format!("Bad glob: {e}"))?;

  let mut hits: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();
  let mut total = 0usize;
  let walker = ignore::WalkBuilder::new(&ctx.cwd).build();
  for entry in walker.flatten() {
    let path = entry.path();
    if !path.is_file() {
      continue;
    }
    let rel = path.strip_prefix(&ctx.cwd).unwrap_or(path);
    if set.is_match(rel) {
      let mtime = entry
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .unwrap_or(std::time::UNIX_EPOCH);
      hits.push((path.to_path_buf(), mtime));
      total += 1;
    }
  }
  hits.sort_by_key(|b| std::cmp::Reverse(b.1));
  let truncated = total > CAP;
  hits.truncate(CAP);

  let paths: Vec<String> = hits
    .into_iter()
    .map(|(p, _)| p.strip_prefix(&ctx.cwd).unwrap_or(&p).display().to_string())
    .collect();

  let out = json!({
    "matches": paths,
    "truncated": truncated,
    "total": total,
  });
  Ok(serde_json::to_string_pretty(&out).unwrap())
}
