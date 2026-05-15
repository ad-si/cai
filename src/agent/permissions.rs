use std::error::Error;
use std::io::Write;

use color_print::cprintln;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Permission {
  /// Prompt before every tool call.
  Always,
  /// Prompt before mutating tools (Edit, UserInstruct).
  Edit,
  /// Run all tools without prompting.
  Never,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Decision {
  Run,
  Skip,
  Abort,
}

/// Decide whether to run a tool, prompting the user if needed.
pub fn decide(
  permission: Permission,
  tool_name: &str,
  input_pretty: &str,
) -> Result<Decision, Box<dyn Error + Send + Sync>> {
  let needs_prompt = match permission {
    Permission::Never => false,
    Permission::Edit => super::tools::is_mutating(tool_name),
    Permission::Always => true,
  };

  if !needs_prompt {
    return Ok(Decision::Run);
  }

  println!();
  cprintln!("<bold,yellow>▶ Tool call: {tool_name}</bold,yellow>");
  bat::PrettyPrinter::new()
    .input_from_bytes(input_pretty.as_bytes())
    .language("json")
    .print()
    .ok();
  println!();

  loop {
    print!("Run this tool? [y]es, [s]kip, [a]bort: ");
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let choice = input.trim().to_lowercase();

    match choice.as_str() {
      "" | "y" | "yes" => return Ok(Decision::Run),
      "s" | "skip" | "n" | "no" => return Ok(Decision::Skip),
      "a" | "abort" | "q" | "quit" => return Ok(Decision::Abort),
      other => {
        eprintln!(
          "Unrecognized choice '{other}'. Answer with 'y', 's', or 'a'."
        );
      }
    }
  }
}
