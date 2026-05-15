use serde_json::{json, Value};

use super::super::AgentContext;
use super::ToolResult;

pub fn definition() -> Value {
  super::def(
    "WebSearch",
    "Search the web (via Perplexity Sonar). Returns a list of result \
    titles, URLs, and (when available) dates. Use WebFetch on a result \
    URL to read the page.",
    json!({
      "type": "object",
      "properties": {
        "query": { "type": "string" }
      },
      "required": ["query"]
    }),
  )
}

pub async fn execute(input: &Value, ctx: &mut AgentContext) -> ToolResult {
  let query = input["query"].as_str().ok_or("Missing 'query'")?;
  let req = ctx
    .perplexity_req
    .as_ref()
    .ok_or("WebSearch unavailable: no Perplexity API key configured")?;

  let body = json!({
    "model": req.model,
    "messages": [{
      "role": "user",
      "content": format!(
        "Search query: {query}\n\nReturn a brief paragraph answering the \
        query, then list the most relevant sources."
      ),
    }],
  });

  let resp = reqwest::Client::new()
    .post(&req.url)
    .bearer_auth(&req.api_key)
    .json(&body)
    .send()
    .await?;
  if !resp.status().is_success() {
    let err = resp.text().await.unwrap_or_default();
    return Err(format!("Perplexity error: {err}").into());
  }
  let v: Value = resp.json().await?;

  let answer = v["choices"][0]["message"]["content"]
    .as_str()
    .unwrap_or("")
    .to_string();
  let results: Vec<Value> =
    v["search_results"].as_array().cloned().unwrap_or_default();

  let out = json!({
    "summary": answer,
    "results": results,
  });
  Ok(serde_json::to_string_pretty(&out).unwrap())
}
