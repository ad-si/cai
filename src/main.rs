use std::io::stdin;
use std::io::{read_to_string, IsTerminal};

use cai::{
  analyze_file_content, exec_tool, generate_changelog, groq_models_pretty,
  ollama_models_pretty, openai_models_pretty, prompt_with_lang_cntxt,
  submit_prompt, ExecOptions, Model, Provider,
};
use clap::{builder::styling, crate_version, Parser, Subcommand};
use color_print::cformat;
use futures::future::join_all;

const CRATE_VERSION: &str = crate_version!();

#[derive(Subcommand, Debug, PartialEq)]
#[clap(args_conflicts_with_subcommands = false, arg_required_else_help(true))]
enum Commands {
  /// Groq
  #[clap(visible_alias = "gr")]
  Groq {
    #[clap(help = groq_models_pretty!("Following aliases are available:"))]
    model: String,
    /// The prompt to send to the AI model
    #[clap(required(true))]
    prompt: Vec<String>,
  },
  /// - Llama 3 shortcut (🏆 Default)
  #[clap(name = "ll")]
  Llama3 {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - Mixtral shortcut
  #[clap(name = "mi")]
  Mixtral {
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
  /// - GPT-4o shortcut
  #[clap(name = "gp")]
  Gpt {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - GPT-4o mini shortcut
  #[clap(name = "gm")]
  GptMini {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Anthropic
  #[clap(visible_alias = "an")]
  Anthropic {
    #[clap(help = openai_models_pretty!(
      "Following aliases are available
(Check out https://docs.anthropic.com/claude/docs/models-overview \
for all supported model ids):"
    ))]
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
  /// Simultaneously send prompt to each provider's default model:
  /// - Groq Llama 3.1
  /// - Antropic Claude Sonnet 3.5
  /// - OpenAI GPT-4o mini
  /// - Ollama Llama 3
  /// - Llamafile
  #[clap(verbatim_doc_comment)] // Include linebreaks
  All {
    /// The prompt to send to the AI models simultaneously
    prompt: Vec<String>,
  },
  /// Generate a changelog starting from a given commit
  /// using OpenAI's GPT-4o
  #[clap()]
  Changelog {
    /// The commit hash to start the changelog from
    commit_hash: String,
  },

  /// Analyze and rename a file with timestamp and description
  #[clap()]
  Rename {
    /// The file to analyze and rename
    file: String,
  },

  /////////////////////////////////////////
  //========== LANGUAGE CONTEXTS ==========
  /////////////////////////////////////////
  /// Use Bash development as the prompt context
  #[clap()]
  Bash {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use C development as the prompt context
  #[clap()]
  C {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use C++ development as the prompt context
  #[clap()]
  Cpp {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use C# development as the prompt context
  #[clap()]
  Cs {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Elm development as the prompt context
  #[clap()]
  Elm {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use F# development as the prompt context
  #[clap()]
  Fs {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Godot and GDScript development as the prompt context
  #[clap()]
  Gd {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Gleam development as the prompt context
  #[clap()]
  Gl {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Go development as the prompt context
  #[clap()]
  Go {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Haskell development as the prompt context
  #[clap()]
  Hs {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Java development as the prompt context
  #[clap()]
  Java {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use JavaScript development as the prompt context
  #[clap()]
  Js {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Kotlin development as the prompt context
  #[clap()]
  Kt {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Lua development as the prompt context
  #[clap()]
  Lua {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use OCaml development as the prompt context
  #[clap()]
  Oc {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use PHP development as the prompt context
  #[clap()]
  Php {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Postgres development as the prompt context
  #[clap()]
  Po {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use PureScript development as the prompt context
  #[clap()]
  Ps {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Python development as the prompt context
  #[clap()]
  Py {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Ruby development as the prompt context
  #[clap()]
  Rb {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Rust development as the prompt context
  #[clap()]
  Rs {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use SQLite development as the prompt context
  #[clap()]
  Sql {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Swift development as the prompt context
  #[clap()]
  Sw {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use TypeScript development as the prompt context
  #[clap()]
  Ts {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Wolfram Language and Mathematica development as the prompt context
  #[clap()]
  Wl {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Zig development as the prompt context
  #[clap()]
  Zig {
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
  <b>cai anthropic claude-3-opus-latest</b> Which year did the Titanic sink

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
  #[arg(long, short, action, help = "Print raw response without any metadata")]
  raw: bool,

  #[arg(long, short, action, help = "Prompt LLM in JSON output mode")]
  json: bool,

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
  let opts = ExecOptions {
    is_raw: args.raw,
    is_json: args.json,
  };

  match args.command {
    None => {
      // No subcommand provided -> Use input as prompt for the default model
      submit_prompt(
        &None,
        &opts,
        &format!("{stdin}{}", &args.prompt.join(" ")), //
      )
      .await
    }
    Some(cmd) => match cmd {
      Commands::Groq { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Groq, model)),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Mixtral { prompt } => {
        submit_prompt(
          &Some(&Model::Model(
            Provider::Groq,
            "mixtral-8x7b-32768".to_string(),
          )),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Llama3 { prompt } => {
        submit_prompt(
          &Some(&Model::Model(
            Provider::Groq,
            "llama-3.1-8b-instant".to_string(),
          )),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Openai { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, model)),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Gpt { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, "gpt-4o".to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::GptMini { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, "gpt-4o-mini".to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Anthropic { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Anthropic, model)),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::ClaudeOpus { prompt } => {
        submit_prompt(
          &Some(&Model::Model(
            Provider::Anthropic,
            "claude-3-opus-latest".to_string(),
          )),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::ClaudeSonnet { prompt } => {
        submit_prompt(
          &Some(&Model::Model(
            Provider::Anthropic,
            "claude-3-5-sonnet-latest".to_string(),
          )),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::ClaudeHaiku { prompt } => {
        submit_prompt(
          &Some(&Model::Model(
            Provider::Anthropic,
            "claude-3-5-haiku-latest".to_string(),
          )),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Llamafile { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Llamafile, "".to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await //
      }
      Commands::Ollama { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Ollama, model)),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await //
      }
      Commands::All { prompt } => {
        let models = vec![
          Model::Model(
            Provider::Anthropic,
            "claude-3-5-sonnet-latest".to_string(),
          ),
          Model::Model(Provider::Groq, "llama-3.1-8b-instant".to_string()),
          Model::Model(Provider::OpenAI, "gpt-4o-mini".to_string()),
          Model::Model(Provider::Ollama, "llama3".to_string()),
          Model::Model(Provider::Llamafile, "".to_string()),
        ];

        let mut handles = vec![];

        for model in models.into_iter() {
          let prompt_str = format!("{}\n{}", stdin, prompt.join(" "));
          let model_fmt = model.to_string();

          handles.push(tokio::spawn(async move {
            match exec_tool(&Some(&model), &opts, &prompt_str).await {
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
      Commands::Changelog { commit_hash } => {
        if let Err(err) = generate_changelog(&opts, &commit_hash).await {
          eprintln!("Error generating changelog: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Rename { file } => {
        match analyze_file_content(&opts, &file).await {
          Ok(description) => {
            let timestamp = chrono::Local::now().format("%Y-%m-%dT%H%M");
            let description =
              description.trim().to_lowercase().replace(' ', "_");
            let path = std::path::Path::new(&file);
            let extension =
              path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
            let new_name =
              format!("{}_{}.{}", timestamp, description, extension);

            if let Err(err) = std::fs::rename(&file, &new_name) {
              eprintln!("Error renaming file: {}", err);
              std::process::exit(1);
            }
            println!("Renamed {} to {}", file, new_name);
          }
          Err(err) => {
            eprintln!("Error analyzing file: {}", err);
            std::process::exit(1);
          }
        }
      }
      /////////////////////////////////////////
      //========== LANGUAGE CONTEXTS ==========
      /////////////////////////////////////////
      Commands::Bash { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "Bash", prompt).await {
          eprintln!("Error prompting with Bash context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::C { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "C", prompt).await {
          eprintln!("Error prompting with C context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Cpp { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "C++", prompt).await {
          eprintln!("Error prompting with C++ context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Cs { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "C#", prompt).await {
          eprintln!("Error prompting with C# context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Elm { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "Elm", prompt).await {
          eprintln!("Error prompting with Elm context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Fs { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "F#", prompt).await {
          eprintln!("Error prompting with F# context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Gd { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "Godot", prompt).await {
          eprintln!("Error prompting with Godot context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Gl { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "Gleam", prompt).await {
          eprintln!("Error prompting with Gleam context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Go { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "Go", prompt).await {
          eprintln!("Error prompting with Go context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Hs { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "Haskell", prompt).await
        {
          eprintln!("Error prompting with Haskell context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Java { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "Java", prompt).await {
          eprintln!("Error prompting with Java context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Js { prompt } => {
        if let Err(err) =
          prompt_with_lang_cntxt(&opts, "JavaScript", prompt).await
        {
          eprintln!("Error prompting with JavaScript context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Kt { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "Kotlin", prompt).await
        {
          eprintln!("Error prompting with Kotlin context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Lua { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "Lua", prompt).await {
          eprintln!("Error prompting with Lua context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Oc { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "OCaml", prompt).await {
          eprintln!("Error prompting with OCaml context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Php { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "PHP", prompt).await {
          eprintln!("Error prompting with PHP context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Po { prompt } => {
        if let Err(err) =
          prompt_with_lang_cntxt(&opts, "Postgres", prompt).await
        {
          eprintln!("Error prompting with Postgres context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Ps { prompt } => {
        if let Err(err) =
          prompt_with_lang_cntxt(&opts, "PureScript", prompt).await
        {
          eprintln!("Error prompting with PureScript context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Py { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "Python", prompt).await
        {
          eprintln!("Error prompting with Python context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Rb { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "Ruby", prompt).await {
          eprintln!("Error prompting with Ruby context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Rs { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "Rust", prompt).await {
          eprintln!("Error prompting with Rust context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Sql { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "SQLite", prompt).await
        {
          eprintln!("Error prompting with SQLite context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Sw { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "Swift", prompt).await {
          eprintln!("Error prompting with Swift context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Ts { prompt } => {
        if let Err(err) =
          prompt_with_lang_cntxt(&opts, "TypeScript", prompt).await
        {
          eprintln!("Error prompting with TypeScript context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Wl { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "Wolfram", prompt).await
        {
          eprintln!("Error prompting with Wolfram context: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Zig { prompt } => {
        if let Err(err) = prompt_with_lang_cntxt(&opts, "Zig", prompt).await {
          eprintln!("Error prompting with Zig context: {}", err);
          std::process::exit(1);
        }
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
