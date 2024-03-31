use cai::{submit_prompt, Provider};
use clap::{builder::styling, crate_version, Parser, Subcommand};

const CRATE_VERSION: &str = crate_version!();

#[derive(Subcommand, Debug, PartialEq)]
#[clap(args_conflicts_with_subcommands = true, arg_required_else_help(true))]
enum Commands {
  /// Use OpenAI's ChatGPT
  #[clap(visible_alias = "g")]
  Gpt {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use a local model hosted at http://localhost:8080 (e.g. Llamafile)
  #[clap(visible_alias = "l")]
  Local {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
}

#[derive(Parser, Debug)]
// #[command(version, about, long_about = None)]
#[clap(
  trailing_var_arg = true,
  about = color_print::cformat!(
    "<bold,underline>Cai {}</bold,underline>\n\n\
      <black,bold>The fastest CLI tool for prompting LLMs</black,bold>",
    CRATE_VERSION,
  ),
  styles = styling::Styles::styled()
    .literal(styling::AnsiColor::Blue.on_default() | styling::Effects::BOLD)
    .placeholder(styling::AnsiColor::Yellow.on_default())
)]
struct Args {
  #[command(subcommand)]
  command: Option<Commands>,

  /// The prompt to send to the AI model
  #[clap(allow_hyphen_values = true)]
  prompt: Vec<String>,
}

fn exec_with_args(args: Args) {
  match &args.command {
    Some(Commands::Gpt { prompt }) => {
      submit_prompt(&Some(Provider::OpenAI), &prompt.join(" "))
    }
    Some(Commands::Local { prompt }) => {
      submit_prompt(&Some(Provider::Local), &prompt.join(" "))
    }
    _ => submit_prompt(&None, &args.prompt.join(" ")),
  }
}

fn main() {
  let args = Args::parse();
  exec_with_args(args);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_args() {
    let parse_res = Args::try_parse_from(&["gpt"]);
    assert!(parse_res.is_err());
    assert!(&parse_res.unwrap_err().to_string().contains("Usage: gpt"));
  }
}
