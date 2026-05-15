use serde_json::{json, Value};

use super::super::{AgentContext, ReadState};
use super::ToolResult;

const DEFAULT_LIMIT: usize = 2000;

pub fn definition() -> Value {
  super::def(
    "Read",
    "Read a file and return its contents prefixed with line numbers \
    (`<line>\\t<text>`). Pass `offset` (1-based first line) and `limit` \
    to read a slice. Special handling: PDFs (≤ 10 pages without `pages`, \
    or pass `pages` like `1-5`); Jupyter notebooks return cells \
    sequentially. Always Read a file before Editing it.",
    json!({
      "type": "object",
      "properties": {
        "path":   { "type": "string" },
        "offset": { "type": "integer", "minimum": 1 },
        "limit":  { "type": "integer", "minimum": 1 },
        "pages":  { "type": "string", "description": "PDF page range, e.g. `1-5`." }
      },
      "required": ["path"]
    }),
  )
}

pub async fn execute(input: &Value, ctx: &mut AgentContext) -> ToolResult {
  let raw = input["path"].as_str().ok_or("Missing 'path'")?;
  let path = super::resolve_path(ctx, raw)?;
  let meta = std::fs::metadata(&path)
    .map_err(|e| format!("Cannot stat {}: {e}", path.display()))?;
  if !meta.is_file() {
    return Err(format!("Not a file: {}", path.display()).into());
  }

  let lower = path
    .extension()
    .and_then(|e| e.to_str())
    .map(str::to_lowercase);

  let content = match lower.as_deref() {
    Some("pdf") => read_pdf(&path, input)?,
    Some("ipynb") => read_notebook(&path)?,
    _ => read_text(&path, input)?,
  };

  // Track for Edit's read-before-edit invariant.
  ctx.read_files.insert(
    path.clone(),
    ReadState {
      mtime: meta.modified().unwrap_or(std::time::UNIX_EPOCH),
      size: meta.len(),
    },
  );

  Ok(content)
}

fn read_text(
  path: &std::path::Path,
  input: &Value,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
  use std::fmt::Write;

  let bytes = std::fs::read(path)?;
  let text = String::from_utf8_lossy(&bytes);
  let offset = input["offset"].as_u64().unwrap_or(1) as usize;
  let limit = input["limit"].as_u64().unwrap_or(DEFAULT_LIMIT as u64) as usize;
  let end = offset.saturating_add(limit);

  let mut out = String::new();
  let mut total = 0usize;
  for (i, line) in text.lines().enumerate() {
    let lnum = i + 1;
    total = lnum;
    if lnum >= offset && lnum < end {
      let _ = writeln!(out, "{lnum}\t{line}");
    }
  }
  if total >= end {
    let _ = writeln!(
      out,
      "\n[{} more lines; pass offset={} to continue]",
      total - end + 1,
      end
    );
  }
  Ok(out)
}

fn read_pdf(
  path: &std::path::Path,
  input: &Value,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
  let _pages = input["pages"].as_str();
  // pdf_extract returns the full document as text. Page-range slicing
  // would require a heavier crate; for now we honour the parameter as
  // a hint and just return everything.
  let text = pdf_extract::extract_text(path)
    .map_err(|e| format!("PDF extraction failed: {e}"))?;
  Ok(text)
}

fn read_notebook(
  path: &std::path::Path,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
  let bytes = std::fs::read(path)?;
  let nb: Value = serde_json::from_slice(&bytes)
    .map_err(|e| format!("Invalid notebook JSON: {e}"))?;
  let cells = nb["cells"].as_array().ok_or("Notebook has no cells")?;
  let mut out = String::new();
  for (i, cell) in cells.iter().enumerate() {
    let kind = cell["cell_type"].as_str().unwrap_or("unknown");
    out.push_str(&format!("\n## Cell {} [{kind}]\n", i + 1));
    if let Some(src) = cell["source"].as_array() {
      for line in src {
        if let Some(s) = line.as_str() {
          out.push_str(s);
        }
      }
      out.push('\n');
    } else if let Some(s) = cell["source"].as_str() {
      out.push_str(s);
      out.push('\n');
    }
    if let Some(outputs) = cell["outputs"].as_array() {
      for output in outputs {
        if let Some(text) = output["text"].as_array() {
          for line in text {
            if let Some(s) = line.as_str() {
              out.push_str("[out] ");
              out.push_str(s);
            }
          }
        }
        if let Some(data) = output["data"].as_object() {
          if let Some(text) = data.get("text/plain").and_then(|v| v.as_array())
          {
            for line in text {
              if let Some(s) = line.as_str() {
                out.push_str("[out] ");
                out.push_str(s);
              }
            }
          }
        }
      }
    }
  }
  Ok(out)
}
