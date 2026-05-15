use std::time::{Duration, Instant};

use color_print::cprintln;
use serde_json::{json, Value};

use super::super::AgentContext;
use super::ToolResult;

const PREVIEW_LINES: usize = 20;

const CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const MAX_CHARS: usize = 100_000;

pub fn definition() -> Value {
  super::def(
    "WebFetch",
    "Fetch a URL, convert HTML→Markdown, then run the supplied prompt \
    against the page using a small fast model and return its answer. \
    HTTP is upgraded to HTTPS. Cross-host redirects are NOT followed — \
    a notice is returned naming the new URL so you can fetch it \
    explicitly. Responses are cached for 15 minutes.",
    json!({
      "type": "object",
      "properties": {
        "url":    { "type": "string" },
        "prompt": { "type": "string", "description": "What to extract from the page." }
      },
      "required": ["url", "prompt"]
    }),
  )
}

pub async fn execute(input: &Value, ctx: &mut AgentContext) -> ToolResult {
  let url_in = input["url"].as_str().ok_or("Missing 'url'")?;
  let prompt = input["prompt"].as_str().ok_or("Missing 'prompt'")?;

  let url = if let Some(rest) = url_in.strip_prefix("http://") {
    format!("https://{rest}")
  } else {
    url_in.to_string()
  };

  let cache_key = format!("{url}\n{prompt}");
  if let Some((when, cached)) = ctx.web_cache.get(&cache_key) {
    if when.elapsed() < CACHE_TTL {
      return Ok(cached.clone());
    }
  }

  // No-redirect client so we can intercept cross-host hops.
  let client = reqwest::Client::builder()
    .redirect(reqwest::redirect::Policy::none())
    .user_agent("cai-agent/0.1")
    .build()?;
  let resp = client
    .get(&url)
    .header("Accept", "text/markdown, text/html;q=0.9")
    .send()
    .await?;

  if resp.status().is_redirection() {
    if let Some(location) = resp.headers().get("location") {
      let loc = location.to_str().unwrap_or("").to_string();
      let target = if loc.starts_with("http") {
        loc.clone()
      } else {
        // relative redirect — same host, fine to follow up.
        match reqwest::Url::parse(&url).and_then(|u| u.join(&loc)) {
          Ok(u) => u.to_string(),
          Err(_) => loc.clone(),
        }
      };
      let host_orig = reqwest::Url::parse(&url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
      let host_new = reqwest::Url::parse(&target)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
      if host_orig != host_new {
        return Ok(format!(
          "Redirect to a different host detected.\n\
          Original: {url}\n\
          Redirect: {target}\n\
          Call WebFetch again with the new URL to follow it."
        ));
      }
      // Same-host redirect: re-fetch (one hop).
      let resp2 = client
        .get(&target)
        .header("Accept", "text/markdown, text/html;q=0.9")
        .send()
        .await?;
      return process_response(ctx, resp2, &cache_key, prompt).await;
    }
  }

  process_response(ctx, resp, &cache_key, prompt).await
}

async fn process_response(
  ctx: &mut AgentContext,
  resp: reqwest::Response,
  cache_key: &str,
  prompt: &str,
) -> ToolResult {
  let status = resp.status();
  let final_url = resp.url().to_string();
  if !status.is_success() {
    return Err(format!("HTTP {status} from server").into());
  }
  let content_type = resp
    .headers()
    .get("content-type")
    .and_then(|v| v.to_str().ok())
    .unwrap_or("")
    .to_string();
  let body = resp.text().await?;

  let markdown = if content_type.contains("html") {
    html2md::parse_html(&body)
  } else {
    body
  };
  let truncated: String = markdown.chars().take(MAX_CHARS).collect();

  log_response(&final_url, status, &content_type, &truncated);

  let answer = match &ctx.fetch_helper_req {
    Some(helper) => {
      let helper_prompt = format!("{prompt}\n\n---\n{truncated}");
      let body = json!({
        "model": helper.model,
        "max_tokens": helper.max_tokens,
        "messages": [{"role": "user", "content": helper_prompt}],
      });
      let resp = reqwest::Client::new()
        .post(&helper.url)
        .header("anthropic-version", "2023-06-01")
        .header("x-api-key", &helper.api_key)
        .json(&body)
        .send()
        .await?;
      if !resp.status().is_success() {
        let err = resp.text().await.unwrap_or_default();
        return Err(format!("WebFetch helper error: {err}").into());
      }
      let v: Value = resp.json().await?;
      v["content"][0]["text"].as_str().unwrap_or("").to_string()
    }
    None => truncated,
  };

  ctx
    .web_cache
    .insert(cache_key.to_string(), (Instant::now(), answer.clone()));
  Ok(answer)
}

/// Print the fetched response to the terminal so the user can see what
/// the agent actually got back. Goes to stderr to keep stdout clean for
/// piping. Shows status, final URL, content type, total size, and a
/// short preview of the converted markdown.
fn log_response(
  url: &str,
  status: reqwest::StatusCode,
  content_type: &str,
  markdown: &str,
) {
  eprintln!();
  cprintln!("<dim>  ┌─ WebFetch response ──────────────────────────</dim>");
  cprintln!("<dim>  │ {} {}</dim>", status.as_u16(), url);
  cprintln!(
    "<dim>  │ {} · {} chars</dim>",
    content_type,
    markdown.chars().count()
  );
  cprintln!("<dim>  ├─ preview ────────────────────────────────────</dim>");
  for line in markdown
    .lines()
    .filter(|l| !l.trim().is_empty())
    .take(PREVIEW_LINES)
  {
    let trimmed: String = line.chars().take(120).collect();
    cprintln!("<dim>  │ {}</dim>", trimmed);
  }
  cprintln!("<dim>  └──────────────────────────────────────────────</dim>");
  eprintln!();
}
