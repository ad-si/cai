//! Anthropic native tool-use agent loop.
//!
//! Sends `messages` + `tools` to `/v1/messages`, streams text back through
//! `streamdown`, accumulates `tool_use` blocks, executes tools (with
//! permission), appends `tool_result`s, and loops until the model returns
//! `stop_reason == "end_turn"`.

use std::error::Error;
use std::io::Write;

use color_print::cprintln;
use futures::StreamExt;
use serde_json::{json, Value};

use super::tools;
use super::AgentContext;

const MAX_ITERATIONS: usize = 50;

pub async fn run(
  ctx: &mut AgentContext,
  user_prompt: &str,
  is_raw: bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let tool_defs = tools::definitions(ctx);
  let system_prompt = build_system_prompt(ctx);

  let mut messages: Vec<Value> = vec![json!({
    "role": "user",
    "content": user_prompt,
  })];

  for iteration in 0..MAX_ITERATIONS {
    let body = json!({
      "model": ctx.anthropic_req.model,
      "max_tokens": ctx.anthropic_req.max_tokens,
      "system": system_prompt,
      "tools": tool_defs,
      "messages": messages,
      "stream": true,
    });

    let resp = reqwest::Client::new()
      .post(&ctx.anthropic_req.url)
      .header("anthropic-version", "2023-06-01")
      .header("x-api-key", &ctx.anthropic_req.api_key)
      .json(&body)
      .send()
      .await?;

    if !resp.status().is_success() {
      let status = resp.status();
      let text = resp.text().await.unwrap_or_default();
      return Err(format!("Anthropic API error ({status}): {text}").into());
    }

    let outcome = read_stream(resp, is_raw).await?;
    messages.push(json!({
      "role": "assistant",
      "content": outcome.content_blocks,
    }));

    match outcome.stop_reason.as_deref() {
      Some("end_turn") | Some("stop_sequence") | None => {
        if !is_raw {
          println!();
        }
        return Ok(());
      }
      Some("max_tokens") => {
        return Err("Hit max_tokens — increase the limit and retry.".into());
      }
      Some("tool_use") => {
        if iteration == MAX_ITERATIONS - 1 {
          return Err(
            format!(
              "Agent exceeded {MAX_ITERATIONS} iterations without producing \
              a final answer."
            )
            .into(),
          );
        }
        let tool_results =
          execute_tool_calls(ctx, &outcome.tool_calls, is_raw).await?;
        if tool_results.is_empty() {
          // User aborted at the prompt.
          return Ok(());
        }
        messages.push(json!({
          "role": "user",
          "content": tool_results,
        }));
      }
      Some(other) => {
        return Err(format!("Unexpected stop_reason: {other}").into());
      }
    }
  }
  Ok(())
}

#[derive(Default)]
struct StreamOutcome {
  /// Assistant message content blocks, in the order Anthropic produced them.
  /// Mix of `{type: "text", text}` and `{type: "tool_use", id, name, input}`.
  content_blocks: Vec<Value>,
  /// Convenience copy of the tool_use blocks for execution.
  tool_calls: Vec<ToolCall>,
  stop_reason: Option<String>,
}

#[derive(Clone)]
struct ToolCall {
  id: String,
  name: String,
  input: Value,
}

/// Per-content-block accumulator while streaming.
enum BlockBuf {
  Text(String),
  ToolUse {
    id: String,
    name: String,
    json_buf: String,
  },
}

async fn read_stream(
  resp: reqwest::Response,
  is_raw: bool,
) -> Result<StreamOutcome, Box<dyn Error + Send + Sync>> {
  let mut byte_stream = resp.bytes_stream();
  let mut buffer: Vec<u8> = Vec::new();
  let mut blocks: Vec<BlockBuf> = Vec::new();
  let mut stop_reason: Option<String> = None;

  // Reuse the existing markdown-streaming infrastructure for assistant text.
  let width = textwrap::termwidth();
  let light = crate::is_light_terminal();
  let mut md_parser = streamdown_parser::Parser::new();
  let mut md_renderer = (!is_raw).then(|| {
    let mut r = streamdown_render::Renderer::new(
      crate::AutoFlush(std::io::stdout()),
      width,
    );
    r.set_style(crate::transparent_render_style(light));
    if light {
      r.set_theme("InspiredGitHub");
    }
    r
  });
  let mut line_buf = String::new();

  while let Some(chunk_result) = byte_stream.next().await {
    let chunk = chunk_result?;
    buffer.extend_from_slice(&chunk);

    loop {
      let lf = buffer.windows(2).position(|w| w == b"\n\n").map(|p| (p, 2));
      let crlf = buffer
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| (p, 4));
      let (pos, sep_len) = match (lf, crlf) {
        (Some(a), Some(b)) if a.0 <= b.0 => a,
        (Some(_), Some(b)) => b,
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => break,
      };
      let event_str = match std::str::from_utf8(&buffer[..pos]) {
        Ok(s) => s.to_string(),
        Err(_) => {
          buffer.drain(..pos + sep_len);
          continue;
        }
      };
      buffer.drain(..pos + sep_len);

      for line in event_str.lines() {
        let data = match line.strip_prefix("data:") {
          Some(d) => d.strip_prefix(' ').unwrap_or(d),
          None => continue,
        };
        let json: Value = match serde_json::from_str(data) {
          Ok(v) => v,
          Err(_) => continue,
        };

        match json["type"].as_str() {
          Some("content_block_start") => {
            let idx = json["index"].as_u64().unwrap_or(0) as usize;
            let block = &json["content_block"];
            let new_block = match block["type"].as_str() {
              Some("text") => BlockBuf::Text(String::new()),
              Some("tool_use") => BlockBuf::ToolUse {
                id: block["id"].as_str().unwrap_or_default().to_string(),
                name: block["name"].as_str().unwrap_or_default().to_string(),
                json_buf: String::new(),
              },
              _ => continue,
            };
            while blocks.len() <= idx {
              blocks.push(BlockBuf::Text(String::new()));
            }
            blocks[idx] = new_block;
          }
          Some("content_block_delta") => {
            let idx = json["index"].as_u64().unwrap_or(0) as usize;
            let delta = &json["delta"];
            match delta["type"].as_str() {
              Some("text_delta") => {
                if let Some(text) = delta["text"].as_str() {
                  if let Some(BlockBuf::Text(buf)) = blocks.get_mut(idx) {
                    buf.push_str(text);
                  }
                  if let Some(renderer) = md_renderer.as_mut() {
                    line_buf.push_str(text);
                    crate::render_through_streamdown(
                      &mut line_buf,
                      &mut md_parser,
                      renderer,
                      false,
                    )?;
                  } else {
                    let mut stdout = std::io::stdout();
                    stdout.write_all(text.as_bytes())?;
                    stdout.flush()?;
                  }
                }
              }
              Some("input_json_delta") => {
                if let Some(partial) = delta["partial_json"].as_str() {
                  if let Some(BlockBuf::ToolUse { json_buf, .. }) =
                    blocks.get_mut(idx)
                  {
                    json_buf.push_str(partial);
                  }
                }
              }
              _ => {}
            }
          }
          Some("message_delta") => {
            if let Some(reason) = json["delta"]["stop_reason"].as_str() {
              stop_reason = Some(reason.to_string());
            }
          }
          Some("message_stop")
          | Some("content_block_stop")
          | Some("message_start")
          | Some("ping") => {}
          _ => {}
        }
      }
    }
  }

  if let Some(renderer) = md_renderer.as_mut() {
    crate::render_through_streamdown(
      &mut line_buf,
      &mut md_parser,
      renderer,
      true,
    )?;
    for event in md_parser.finalize() {
      renderer.render_event(&event)?;
    }
  }
  if !is_raw {
    println!();
  }

  let mut content_blocks = Vec::with_capacity(blocks.len());
  let mut tool_calls = Vec::new();
  for block in blocks {
    match block {
      BlockBuf::Text(text) => {
        content_blocks.push(json!({ "type": "text", "text": text }));
      }
      BlockBuf::ToolUse { id, name, json_buf } => {
        let input: Value = if json_buf.trim().is_empty() {
          json!({})
        } else {
          serde_json::from_str(&json_buf).unwrap_or(json!({}))
        };
        content_blocks.push(json!({
          "type": "tool_use",
          "id": id,
          "name": name,
          "input": input,
        }));
        tool_calls.push(ToolCall { id, name, input });
      }
    }
  }

  Ok(StreamOutcome {
    content_blocks,
    tool_calls,
    stop_reason,
  })
}

async fn execute_tool_calls(
  ctx: &mut AgentContext,
  calls: &[ToolCall],
  is_raw: bool,
) -> Result<Vec<Value>, Box<dyn Error + Send + Sync>> {
  let mut results: Vec<Value> = Vec::new();

  for call in calls {
    let pretty_input =
      serde_json::to_string_pretty(&call.input).unwrap_or_default();

    let decision =
      super::permissions::decide(ctx.permission, &call.name, &pretty_input)?;

    let result_block = match decision {
      super::permissions::Decision::Abort => {
        if !is_raw {
          cprintln!("<yellow>Aborted by user.</yellow>");
        }
        return Ok(Vec::new());
      }
      super::permissions::Decision::Skip => json!({
        "type": "tool_result",
        "tool_use_id": call.id,
        "content": "User declined to run this tool.",
        "is_error": true,
      }),
      super::permissions::Decision::Run => {
        match tools::dispatch(&call.name, &call.input, ctx).await {
          Ok(output) => {
            let truncated = truncate(&output, 60_000);
            json!({
              "type": "tool_result",
              "tool_use_id": call.id,
              "content": truncated,
            })
          }
          Err(err) => json!({
            "type": "tool_result",
            "tool_use_id": call.id,
            "content": format!("Error: {err}"),
            "is_error": true,
          }),
        }
      }
    };

    if !is_raw {
      let summary = match result_block.get("content").and_then(|v| v.as_str()) {
        Some(s) => first_line(s, 100),
        None => String::new(),
      };
      let marker = if result_block
        .get("is_error")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
      {
        "✗"
      } else {
        "✓"
      };
      cprintln!("<dim>  {marker} {} → {summary}</dim>", call.name);
    }

    results.push(result_block);
  }

  Ok(results)
}

fn build_system_prompt(ctx: &AgentContext) -> String {
  let cwd = ctx.cwd.display();
  let os = std::env::consts::OS;
  let date = chrono::Local::now().format("%Y-%m-%d");
  format!(
    "You are `cai agent`, an autonomous coding assistant invoked from the \
    command line. You have file-system, search, and web tools.\n\
    \n\
    - Working directory: {cwd}\n\
    - Operating system: {os}\n\
    - Current date: {date}\n\
    \n\
    Use tools to gather context before answering. Prefer Glob/Grep over \
    listing huge directories. Use Read before Edit; Edit cannot create \
    files (use UserInstruct if a new file is needed). Be concise. When \
    you are done, reply with the final answer in plain text — no further \
    tool calls."
  )
}

fn truncate(s: &str, max: usize) -> String {
  if s.len() <= max {
    return s.to_string();
  }
  let mut end = max;
  while end > 0 && !s.is_char_boundary(end) {
    end -= 1;
  }
  let mut out = s[..end].to_string();
  out.push_str(&format!("\n\n[…truncated {} more bytes]", s.len() - end));
  out
}

fn first_line(s: &str, max: usize) -> String {
  let line = s.lines().next().unwrap_or("");
  if line.chars().count() <= max {
    line.to_string()
  } else {
    let truncated: String = line.chars().take(max).collect();
    format!("{truncated}…")
  }
}
