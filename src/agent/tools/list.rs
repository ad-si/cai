use serde_json::{json, Value};

use super::super::AgentContext;
use super::ToolResult;

const CAP: usize = 100;

pub fn definition() -> Value {
  super::def(
    "List",
    "List entries in a directory (one level deep) with size and \
    modification time, sorted by mtime descending. Capped at 100 entries; \
    a `truncated: true` flag indicates more were available — narrow the \
    target with the Glob tool in that case.",
    json!({
      "type": "object",
      "properties": {
        "path": {
          "type": "string",
          "description": "Absolute or CWD-relative directory path."
        }
      },
      "required": ["path"]
    }),
  )
}

pub async fn execute(input: &Value, ctx: &mut AgentContext) -> ToolResult {
  let raw = input["path"].as_str().ok_or("Missing 'path' parameter")?;
  let path = super::resolve_path(ctx, raw)?;
  let meta = std::fs::metadata(&path)
    .map_err(|e| format!("Cannot access {}: {e}", path.display()))?;
  if !meta.is_dir() {
    return Err(format!("Not a directory: {}", path.display()).into());
  }

  let mut entries: Vec<(std::path::PathBuf, u64, std::time::SystemTime, bool)> =
    Vec::new();
  let mut total = 0usize;
  for entry in std::fs::read_dir(&path)? {
    let entry = entry?;
    let m = match entry.metadata() {
      Ok(m) => m,
      Err(_) => continue,
    };
    let mtime = m.modified().unwrap_or(std::time::UNIX_EPOCH);
    entries.push((entry.path(), m.len(), mtime, m.is_dir()));
    total += 1;
  }
  entries.sort_by_key(|b| std::cmp::Reverse(b.2));
  let truncated = total > CAP;
  entries.truncate(CAP);

  let json_entries: Vec<Value> = entries
    .into_iter()
    .map(|(p, size, mtime, is_dir)| {
      let modified: chrono::DateTime<chrono::Local> = mtime.into();
      json!({
        "path": p.strip_prefix(&ctx.cwd).unwrap_or(&p).display().to_string(),
        "size": size,
        "modified": modified.format("%Y-%m-%d %H:%M:%S").to_string(),
        "is_dir": is_dir,
      })
    })
    .collect();

  let out = json!({
    "entries": json_entries,
    "truncated": truncated,
    "total_seen": total,
  });
  Ok(serde_json::to_string_pretty(&out).unwrap())
}
