use serde_json::{json, Value};

use super::super::AgentContext;
use super::ToolResult;

pub fn definition() -> Value {
  super::def(
    "Edit",
    "Replace `old_string` with `new_string` in an existing file. \
    Requirements: the file must have been Read in this conversation and \
    unchanged on disk since; `old_string` must appear verbatim and \
    uniquely (or pass `replace_all: true`). Cannot create new files — \
    use UserInstruct if a new file is required.",
    json!({
      "type": "object",
      "properties": {
        "path":        { "type": "string" },
        "old_string":  { "type": "string" },
        "new_string":  { "type": "string" },
        "replace_all": { "type": "boolean" }
      },
      "required": ["path", "old_string", "new_string"]
    }),
  )
}

pub async fn execute(input: &Value, ctx: &mut AgentContext) -> ToolResult {
  let raw = input["path"].as_str().ok_or("Missing 'path'")?;
  let old_string =
    input["old_string"].as_str().ok_or("Missing 'old_string'")?;
  let new_string =
    input["new_string"].as_str().ok_or("Missing 'new_string'")?;
  let replace_all = input["replace_all"].as_bool().unwrap_or(false);

  let path = super::resolve_path(ctx, raw)?;

  if !path.exists() {
    return Err(
      format!(
        "File does not exist. Edit cannot create files — use UserInstruct: {}",
        path.display()
      )
      .into(),
    );
  }

  // Read-before-edit + freshness check.
  let read_state = ctx.read_files.get(&path).ok_or_else(|| {
    format!("File must be Read before Edit: {}", path.display())
  })?;
  let meta = std::fs::metadata(&path)?;
  let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
  if mtime != read_state.mtime || meta.len() != read_state.size {
    return Err(
      format!(
        "File changed on disk since last Read: {}. Re-Read and try again.",
        path.display()
      )
      .into(),
    );
  }

  let original = std::fs::read_to_string(&path)?;
  let count = original.matches(old_string).count();
  if count == 0 {
    return Err(
      format!("`old_string` not found in {}.", path.display()).into(),
    );
  }
  if count > 1 && !replace_all {
    return Err(
      format!(
        "`old_string` matches {count} times in {}. Use `replace_all: true` \
      or supply more surrounding context for a unique match.",
        path.display()
      )
      .into(),
    );
  }

  let updated = if replace_all {
    original.replace(old_string, new_string)
  } else {
    original.replacen(old_string, new_string, 1)
  };
  std::fs::write(&path, &updated)?;

  // Refresh the tracked state so a subsequent Edit on the same file
  // doesn't trip the freshness check.
  let new_meta = std::fs::metadata(&path)?;
  if let Some(state) = ctx.read_files.get_mut(&path) {
    state.mtime = new_meta.modified().unwrap_or(std::time::UNIX_EPOCH);
    state.size = new_meta.len();
  }

  Ok(format!(
    "Edited {} ({} replacement{}).",
    path.display(),
    count,
    if count == 1 { "" } else { "s" }
  ))
}
