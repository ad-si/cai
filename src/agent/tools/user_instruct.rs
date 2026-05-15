use std::io::Write;

use color_print::cprintln;
use serde_json::{json, Value};

use super::super::AgentContext;
use super::ToolResult;

pub fn definition() -> Value {
  super::def(
    "UserInstruct",
    "Ask the user to perform an action you can't do yourself \
    (e.g. create a new file, install a tool, log into a service). \
    The user does the action, then replies with a short status; \
    that reply is returned as the tool result.",
    json!({
      "type": "object",
      "properties": {
        "instruction": {
          "type": "string",
          "description": "What the user should do."
        }
      },
      "required": ["instruction"]
    }),
  )
}

pub async fn execute(input: &Value, _ctx: &mut AgentContext) -> ToolResult {
  let instruction = input["instruction"]
    .as_str()
    .ok_or("Missing 'instruction'")?;
  println!();
  cprintln!("<bold,cyan>📋 Action required:</bold,cyan>");
  println!("{instruction}");
  println!();
  print!("Reply when done (or 'abort'): ");
  std::io::stdout().flush()?;
  let mut reply = String::new();
  std::io::stdin().read_line(&mut reply)?;
  let reply = reply.trim().to_string();
  if reply.eq_ignore_ascii_case("abort") {
    return Err("User aborted the requested action.".into());
  }
  if reply.is_empty() {
    Ok("Done.".to_string())
  } else {
    Ok(reply)
  }
}
