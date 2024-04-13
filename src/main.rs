use cai::{
  exec_tool, submit_prompt, Model, Provider, CLAUDE_HAIKU, CLAUDE_OPUS,
  CLAUDE_SONNET, GROQ_MIXTRAL, OPENAI_GPT, OPENAI_GPT_TURBO,
};
use clap::{builder::styling, crate_version, Parser, Subcommand};
use color_print::cformat;
use futures::future::join_all;

const CRATE_VERSION: &str = crate_version!();

#[derive(Subcommand, Debug, PartialEq)]
#[clap(args_conflicts_with_subcommands = true, arg_required_else_help(true))]
enum Commands {
  /// Groq's Mixtral
  #[clap(visible_alias = "mi")]
  Mixtral {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// OpenAI's GPT 4 Turbo
  #[clap(visible_alias = "tu")]
  GptTurbo {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// OpenAI's GPT 4
  #[clap(visible_alias = "gp")]
  Gpt {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Anthropic's Claude Opus
  #[clap(visible_alias = "op")]
  ClaudeOpus {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Anthropic's Claude Sonnet
  #[clap(visible_alias = "so")]
  ClaudeSonnet {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// 🏆 Default | Anthropic's Claude Haiku
  #[clap(visible_alias = "ha")]
  ClaudeHaiku {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Local model hosted at http://localhost:8080 (e.g. Llamafile)
  #[clap(visible_alias = "lo")]
  Local {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Send the prompt to every provider's default model simultaneously
  /// (Claude Haiku, Groq Mixtral, GPT 4 Turbo, Local)
  All {
    /// The prompt to send to the AI models simultaneously
    prompt: Vec<String>,
  },
  // => https://stackoverflow.com/questions/51044467/how-can-i-perform-parallel-asynchronous-http-get-requests-with-reqwest
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

fn capitalize_str(str: &str) -> String {
  let mut chars = str.chars();
  match chars.next() {
    None => String::new(),
    Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
  }
}

async fn exec_with_args(args: Args) {
  async fn call_submit_fn(
    provider: Provider,
    model_id: &str,
    prompt: &Vec<String>,
  ) {
    submit_prompt(
      &Some(Model::Model(provider, model_id.to_string())),
      &prompt.join(" "),
    )
    .await
  }

  match args.command {
    None => submit_prompt(&None, &args.prompt.join(" ")).await,
    Some(cmd) => match cmd {
      Commands::Mixtral { prompt } => {
        call_submit_fn(Provider::Groq, GROQ_MIXTRAL, &prompt).await
      }
      Commands::GptTurbo { prompt } => {
        call_submit_fn(Provider::OpenAI, OPENAI_GPT_TURBO, &prompt).await
      }
      Commands::Gpt { prompt } => {
        call_submit_fn(Provider::OpenAI, OPENAI_GPT, &prompt).await
      }
      Commands::ClaudeOpus { prompt } => {
        call_submit_fn(Provider::Anthropic, CLAUDE_OPUS, &prompt).await
      }
      Commands::ClaudeSonnet { prompt } => {
        call_submit_fn(Provider::Anthropic, CLAUDE_SONNET, &prompt).await
      }
      Commands::ClaudeHaiku { prompt } => {
        call_submit_fn(Provider::Anthropic, CLAUDE_HAIKU, &prompt).await
      }
      Commands::Local { prompt } => {
        call_submit_fn(Provider::Local, "", &prompt).await //
      }
      Commands::All { prompt } => {
        let models = vec![
          Model::Model(Provider::Anthropic, CLAUDE_HAIKU.to_string()),
          Model::Model(Provider::Groq, GROQ_MIXTRAL.to_string()),
          Model::Model(Provider::OpenAI, OPENAI_GPT_TURBO.to_string()),
          Model::Model(Provider::Local, "".to_string()),
        ];

        let mut handles = vec![];

        for model in models.into_iter() {
          let prompt_str = prompt.join(" ");
          handles.push(tokio::spawn(async move {
            match exec_tool(&Some(model), &prompt_str).await {
              Ok(_) => {}
              Err(err) => {
                let err_fmt = capitalize_str(&err.to_string());
                eprintln!("{}", cformat!("<red>ERROR:\n{err_fmt}\n</red>"))
              }
            }
          }));
        }

        join_all(handles).await;
      }
    },
  };
}

#[tokio::main]
async fn main() {
  let args = Args::parse();
  exec_with_args(args).await
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
