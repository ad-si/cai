use std::path::{Path, PathBuf};

use grep_searcher::sinks::UTF8;
use grep_searcher::{Searcher, SearcherBuilder};
use serde_json::{json, Value};

use super::super::AgentContext;
use super::ToolResult;

const FILE_CAP: usize = 200;
const LINE_CAP: usize = 1000;

pub fn definition() -> Value {
  super::def(
    "Grep",
    "Search file contents for a regex (ripgrep syntax). Respects \
    .gitignore by default; pass a single gitignored path explicitly to \
    search it anyway. Output modes: `files_with_matches` (default), \
    `content`, `count`. Optional `glob` filter (e.g. `**/*.rs`) and \
    `multiline: true` to match across line boundaries.",
    json!({
      "type": "object",
      "properties": {
        "pattern": { "type": "string", "description": "Regex pattern." },
        "path":    { "type": "string", "description": "File or directory to search. Defaults to CWD." },
        "glob":    { "type": "string", "description": "Optional glob filter, e.g. `**/*.rs`." },
        "output_mode": {
          "type": "string",
          "enum": ["files_with_matches", "content", "count"]
        },
        "multiline": { "type": "boolean" }
      },
      "required": ["pattern"]
    }),
  )
}

pub async fn execute(input: &Value, ctx: &mut AgentContext) -> ToolResult {
  let pattern = input["pattern"]
    .as_str()
    .ok_or("Missing 'pattern' parameter")?;
  let multiline = input["multiline"].as_bool().unwrap_or(false);
  let mode = input["output_mode"]
    .as_str()
    .unwrap_or("files_with_matches");

  let matcher = grep_regex::RegexMatcherBuilder::new()
    .multi_line(multiline)
    .build(pattern)
    .map_err(|e| format!("Bad regex: {e}"))?;

  let glob_filter: Option<globset::GlobSet> = match input["glob"].as_str() {
    Some(g) => {
      let mut b = globset::GlobSetBuilder::new();
      b.add(globset::Glob::new(g).map_err(|e| format!("Bad glob: {e}"))?);
      Some(b.build().map_err(|e| format!("Bad glob: {e}"))?)
    }
    None => None,
  };

  let search_root: PathBuf = match input["path"].as_str() {
    Some(p) => super::resolve_path(ctx, p)?,
    None => ctx.cwd.clone(),
  };

  // Explicit single-file paths bypass .gitignore, per spec.
  let single_file = search_root.is_file().then(|| search_root.clone());

  let candidates: Vec<PathBuf> = if let Some(f) = single_file {
    vec![f]
  } else {
    let mut out = Vec::new();
    let walker = ignore::WalkBuilder::new(&search_root).build();
    for entry in walker.flatten() {
      let path = entry.path();
      if !path.is_file() {
        continue;
      }
      if let Some(ref set) = glob_filter {
        let rel = path.strip_prefix(&ctx.cwd).unwrap_or(path);
        if !set.is_match(rel) {
          continue;
        }
      }
      out.push(path.to_path_buf());
    }
    out
  };

  let mut searcher: Searcher =
    SearcherBuilder::new().multi_line(multiline).build();

  let result = match mode {
    "files_with_matches" => {
      let mut hits: Vec<String> = Vec::new();
      let mut truncated = false;
      for file in &candidates {
        let mut found = false;
        let _ = searcher.search_path(
          &matcher,
          file,
          CallbackSink(|| {
            found = true;
            false
          }),
        );
        if found {
          hits.push(rel_display(file, &ctx.cwd));
          if hits.len() >= FILE_CAP {
            truncated = true;
            break;
          }
        }
      }
      json!({ "files": hits, "truncated": truncated })
    }
    "count" => {
      let mut counts: Vec<Value> = Vec::new();
      let mut truncated = false;
      for file in &candidates {
        let mut count: u64 = 0;
        let _ = searcher.search_path(
          &matcher,
          file,
          CallbackSink(|| {
            count += 1;
            true
          }),
        );
        if count > 0 {
          counts.push(json!({
            "path": rel_display(file, &ctx.cwd),
            "count": count,
          }));
          if counts.len() >= FILE_CAP {
            truncated = true;
            break;
          }
        }
      }
      json!({ "counts": counts, "truncated": truncated })
    }
    "content" => {
      let mut lines: Vec<Value> = Vec::new();
      let mut truncated = false;
      'outer: for file in &candidates {
        let display = rel_display(file, &ctx.cwd);
        let _ = searcher.search_path(
          &matcher,
          file,
          UTF8(|lnum, line| {
            lines.push(json!({
              "path": display,
              "line": lnum,
              "text": line.trim_end_matches('\n'),
            }));
            Ok(lines.len() < LINE_CAP)
          }),
        );
        if lines.len() >= LINE_CAP {
          truncated = true;
          break 'outer;
        }
      }
      json!({ "matches": lines, "truncated": truncated })
    }
    other => return Err(format!("Unknown output_mode: {other}").into()),
  };

  Ok(serde_json::to_string_pretty(&result).unwrap())
}

fn rel_display(path: &Path, cwd: &Path) -> String {
  path.strip_prefix(cwd).unwrap_or(path).display().to_string()
}

struct CallbackSink<F: FnMut() -> bool>(F);

impl<F: FnMut() -> bool> grep_searcher::Sink for CallbackSink<F> {
  type Error = std::io::Error;

  fn matched(
    &mut self,
    _: &Searcher,
    _: &grep_searcher::SinkMatch<'_>,
  ) -> Result<bool, Self::Error> {
    Ok((self.0)())
  }
}
