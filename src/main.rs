use std::io::stdin;
use std::io::{read_to_string, IsTerminal};

use cai::{
  exec_tool, groq_models_pretty, ollama_models_pretty, openai_models_pretty,
  submit_prompt, Model, Provider, CLAUDE_HAIKU, CLAUDE_OPUS, CLAUDE_SONNET,
  GROQ_LLAMA, GROQ_MIXTRAL, OPENAI_GPT, OPENAI_GPT_TURBO,
};
use clap::{builder::styling, crate_version, Parser, Subcommand};
use color_print::cformat;
use futures::future::join_all;

const CRATE_VERSION: &str = crate_version!();

#[derive(Subcommand, Debug, PartialEq)]
#[clap(args_conflicts_with_subcommands = true, arg_required_else_help(true))]
enum Commands {
  #[clap(visible_alias = "gr")]
  Groq {
    #[clap(help = groq_models_pretty!("Following aliases are available:"))]
    model: String,
    /// The prompt to send to the AI model
    #[clap(required(true))]
    prompt: Vec<String>,
  },
  /// - Mixtral shortcut
  #[clap(name = "mi")]
  Mixtral {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - Llama 3 shortcut (🏆 Default)
  #[clap(name = "ll")]
  Llama3 {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// OpenAI
  #[clap(visible_alias = "op")]
  Openai {
    #[clap(help = openai_models_pretty!(
      "Following aliases are available
(Check out https://platform.openai.com/docs/models for all supported model ids):"
    ))]
    model: String,
    /// The prompt to send to the AI model
    #[clap(required(true))]
    prompt: Vec<String>,
  },
  /// - GPT 4 shortcut
  #[clap(name = "gp")]
  Gpt {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - GPT 4 Turbo shortcut
  #[clap(name = "gt")]
  GptTurbo {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Anthropic
  #[clap(visible_alias = "an")]
  Anthropic {
    /// The model to use
    /// - opus
    /// - sonnet
    /// - haiku
    /// - <model-id> from https://docs.anthropic.com/claude/docs/models-overview
    #[clap(verbatim_doc_comment)] // Include linebreaks
    model: String,
    /// The prompt to send to the AI model
    #[clap(required(true))]
    prompt: Vec<String>,
  },
  /// - Claude Opus
  #[clap(name = "cl")]
  ClaudeOpus {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - Claude Sonnet
  #[clap(name = "so")]
  ClaudeSonnet {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - Claude Haiku
  #[clap(name = "ha")]
  ClaudeHaiku {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Llamafile server hosted at http://localhost:8080
  #[clap(visible_alias = "lf")]
  Llamafile {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Ollama server hosted at http://localhost:11434
  #[clap(visible_alias = "ol")]
  Ollama {
    #[clap(help = ollama_models_pretty!(
      "The model to use from the locally installed ones.\n\
      Get new ones from https://ollama.com/library.\n\
      Following aliases are available:"
    ))]
    model: String,
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Send prompt to each provider's default model simultaneously
  /// - Groq Llama3
  /// - Antropic Claude Haiku
  /// - OpenAI GPT 4 Turbo
  /// - Ollama Phi3
  /// - Llamafile
  #[clap(verbatim_doc_comment)] // Include linebreaks
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
  ), /**/
  after_help = color_print::cformat!(
"
<bold,underline>Examples:</bold,underline>
  <dim># Send a prompt to the default model</dim>
  <b>cai</b> Which year did the Titanic sink

  <dim># Send a prompt to each provider's default model</dim>
  <b>cai all</b> Which year did the Titanic sink

  <dim># Send a prompt to Anthropic's Claude Opus</dim>
  <b>cai anthropic claude-opus</b> Which year did the Titanic sink
  <b>cai an claude-opus</b> Which year did the Titanic sink
  <b>cai cl</b> Which year did the Titanic sink
  <b>cai anthropic claude-3-opus-20240229</b> Which year did the Titanic sink

  <dim># Send a prompt to locally running Ollama server</dim>
  <b>cai ollama llama3</b> Which year did the Titanic sink
  <b>cai ol ll</b> Which year did the Titanic sink

  <dim># Add data via stdin</dim>
  cat main.rs | <b>cai</b> Explain this code
"
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

async fn exec_with_args(args: Args, stdin: &str) {
  let stdin = if stdin.is_empty() {
    "".into()
  } else {
    format!("{}\n", stdin)
  };

  match args.command {
    None => {
      // No subcommand provided -> Use input as prompt for the default model
      submit_prompt(
        &None,
        &format!("{stdin}{}", &args.prompt.join(" ")), //
      )
      .await
    }
    Some(cmd) => match cmd {
      Commands::Groq { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Groq, model)),
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Mixtral { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Groq, GROQ_MIXTRAL.to_string())),
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Llama3 { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Groq, GROQ_LLAMA.to_string())),
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Openai { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, model)),
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Gpt { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, OPENAI_GPT.to_string())),
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::GptTurbo { prompt } => {
        submit_prompt(
          &Some(&Model::Model(
            Provider::OpenAI,
            OPENAI_GPT_TURBO.to_string(),
          )),
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Anthropic { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Anthropic, model)),
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::ClaudeOpus { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Anthropic, CLAUDE_OPUS.to_string())),
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::ClaudeSonnet { prompt } => {
        submit_prompt(
          &Some(&Model::Model(
            Provider::Anthropic,
            CLAUDE_SONNET.to_string(),
          )),
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::ClaudeHaiku { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Anthropic, CLAUDE_HAIKU.to_string())),
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Llamafile { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Llamafile, "".to_string())),
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await //
      }
      Commands::Ollama { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Ollama, model)),
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await //
      }
      Commands::All { prompt } => {
        let models = vec![
          Model::Model(Provider::Anthropic, CLAUDE_HAIKU.to_string()),
          Model::Model(Provider::Groq, GROQ_LLAMA.to_string()),
          Model::Model(Provider::OpenAI, OPENAI_GPT_TURBO.to_string()),
          Model::Model(Provider::Ollama, "phi3".to_string()),
          Model::Model(Provider::Llamafile, "".to_string()),
        ];

        let mut handles = vec![];

        for model in models.into_iter() {
          let prompt_str = format!("{}\n{}", stdin, prompt.join(" "));
          let model_fmt = model.to_string();

          handles.push(tokio::spawn(async move {
            match exec_tool(&Some(&model), &prompt_str).await {
              Ok(_) => {}
              Err(err) => {
                let err_fmt = capitalize_str(&err.to_string());
                eprintln!(
                  "{}",
                  cformat!(
                    "<bold>⏱️    0 ms</bold> | \
                    <bold>🧠 {}</bold><red>\nERROR:\n{}</red>\n",
                    model_fmt,
                    err_fmt
                  )
                );
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
  let stdin = stdin();
  let mut args_vector = std::env::args().collect::<Vec<_>>();

  if stdin.is_terminal() {
    exec_with_args(Args::parse_from(args_vector), "").await;
  } else {
    let input = read_to_string(stdin).unwrap();
    let only_stdin = !input.is_empty() && args_vector.len() <= 1;

    if only_stdin {
      args_vector.push("".to_string());
    }

    let mut args = Args::parse_from(args_vector);

    if only_stdin {
      args.prompt = vec![input];
      exec_with_args(args, "").await;
    } else {
      exec_with_args(args, input.trim()).await;
    }
  }
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
