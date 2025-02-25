use std::io::stdin;
use std::io::{read_to_string, IsTerminal};

use cai::{
  analyze_file_content, exec_tool, extract_text_from_file, generate_changelog,
  prompt_with_lang_cntxt, submit_prompt, Commands, ExecOptions, Model,
  Provider,
};
use clap::{builder::styling, crate_version, Parser};
use color_print::cformat;
use futures::future::join_all;
use serde_json::{json, Value};

const CRATE_VERSION: &str = crate_version!();

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

  <dim># Use a JSON schema to specify the output format</dim>
  <b>cai \
    --json-schema='{}' \
    gp Barack Obama
  </b>
",
"{\"properties\":{\"age\":{\"type\":\"number\"}},\"required\":[\"age\"]}"
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

  #[arg(long, action, help = "JSON schema to validate the output against")]
  json_schema: Option<String>,

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
    json_schema: args
      .json_schema
      .map(|schema_str| {
        serde_json::from_str(&schema_str).expect("Invalid JSON schema")
      })
      .map(|schema: Value| {
        let mut schema_obj = schema.as_object().unwrap().clone();
        schema_obj.insert("additionalProperties".to_string(), false.into());
        if !schema_obj.contains_key("type") {
          schema_obj.insert("type".to_string(), "object".into());
        }
        let api_object = json!({
          "name": "requested_json_schema",
          "strict": true,
          "schema": schema_obj,
        });
        api_object
      }),
    subcommand: args.command.clone(),
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
    Some(cmd) => match &cmd {
      Commands::Groq { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Groq, model.to_string())),
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
      Commands::Cerebras { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Cerebras, model.to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Deepseek { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::DeepSeek, model.to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Openai { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, model.to_string())),
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
          &Some(&Model::Model(Provider::Anthropic, model.to_string())),
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
            "claude-3-7-sonnet-latest".to_string(),
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
          &Some(&Model::Model(Provider::Ollama, model.to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await //
      }
      Commands::Xai { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::XAI, model.to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await //
      }
      Commands::Grok { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::XAI, "grok-2-latest".to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::All { prompt } => {
        let models = vec![
          Model::Model(
            Provider::Anthropic,
            "claude-3-7-sonnet-latest".to_string(),
          ),
          Model::Model(Provider::Cerebras, "llama-3.1-8b".to_string()),
          Model::Model(Provider::Groq, "llama-3.1-8b-instant".to_string()),
          Model::Model(Provider::Llamafile, "".to_string()),
          Model::Model(Provider::Ollama, "llama3".to_string()),
          Model::Model(Provider::OpenAI, "gpt-4o-mini".to_string()),
          Model::Model(Provider::XAI, "grok-2-latest".to_string()),
        ];

        let mut handles = vec![];

        for model in models.into_iter() {
          let prompt_str = format!("{}\n{}", stdin, prompt.join(" "));
          let model_fmt = model.to_string();
          let opts_clone = opts.clone();

          handles.push(tokio::spawn(async move {
            match exec_tool(&Some(&model), &opts_clone, &prompt_str).await {
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
          Ok(analysis) => {
            let timestamp_str = analysis.timestamp.unwrap_or_default();
            let timestamp_norm = timestamp_str.trim().to_lowercase();
            let valid_timestamp = chrono::NaiveDateTime::parse_from_str(
              &timestamp_norm,
              "%Y-%m-%dt%H:%Mz",
            )
            .or_else(|_| {
              chrono::NaiveDateTime::parse_from_str(&timestamp_norm, "%Y-%m-%d")
            })
            .is_ok();
            let timestamp = if valid_timestamp {
              timestamp_norm
                .replace(":", "")
                .replace("z", "")
                .replace("t0000", "")
            } else {
              chrono::Local::now().format("%Y-%m-%dt%H%M").to_string()
            };
            let description = analysis //
              .description
              .trim()
              .to_lowercase()
              .replace(' ', "_");
            rename_file(file.to_string(), timestamp, description);
          }
          Err(error) => match error.downcast_ref::<std::io::Error>() {
            Some(err) if err.kind() == std::io::ErrorKind::InvalidData => {
              // If it's not a text file, use the creation time
              let timestamp = std::fs::metadata(&file)
                .map(|meta| {
                  meta
                    .created()
                    .map(|created| {
                      chrono::DateTime::<chrono::Local>::from(created)
                        .format("%Y-%m-%dt%H%M")
                        .to_string()
                    })
                    .unwrap_or_else(|_| {
                      chrono::Local::now().format("%Y-%m-%dt%H%M").to_string()
                    })
                    .to_string()
                })
                .unwrap_or_else(|_| {
                  chrono::Local::now().format("%Y-%m-%dt%H%M").to_string()
                });

              std::path::Path::new(&file)
                .file_stem()
                .map(|file_name_no_ext| {
                  file_name_no_ext.to_str().unwrap_or_default().to_string()
                })
                .map(|file_name| {
                  rename_file(file.to_string(), timestamp, file_name)
                })
                .unwrap_or_else(|| {
                  dbg!(err);
                  std::process::exit(1);
                });
            }
            err => {
              err.map(|e| {
                eprintln!("{}", e);
              });
              std::process::exit(1);
            }
          },
        }
      }
      /////////////////////////////////////////
      //========== LANGUAGE CONTEXTS ==========
      /////////////////////////////////////////
      Commands::Ocr { file } => {
        if let Err(err) = extract_text_from_file(&opts, &file).await {
          eprintln!("Error extracting text: {}", err);
          std::process::exit(1);
        }
      }
      Commands::Bash { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::C { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Cpp { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Cs { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Elm { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Fish { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Fs { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Gd { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Gl { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Go { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Hs { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Java { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Js { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Kt { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Ly { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Lua { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Oc { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Php { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Pg { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Ps { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Py { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Rb { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Rs { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Sql { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Sw { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Ts { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Ty { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Wl { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Zig { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
    },
  };
}

fn rename_file(file: String, timestamp: String, description: String) {
  let path = std::path::Path::new(&file);
  let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
  let mut new_name = path
    .parent()
    .unwrap_or_else(|| std::path::Path::new(""))
    .join(format!("{}_{}.{}", timestamp, description, ext))
    .to_str()
    .unwrap()
    .to_string();

  let mut counter = 0;
  loop {
    if std::path::Path::new(&new_name).exists() {
      counter += 1;
      new_name = format!("{}_{}_{}.{}", timestamp, description, counter, ext)
    } else {
      break;
    }
  }

  if let Err(err) = std::fs::rename(&file, &new_name) {
    eprintln!("Error renaming file: {}", err);
    std::process::exit(1);
  }
  println!("Renamed {} to {}", file, new_name);
}

#[tokio::main]
async fn main() {
  let stdin = stdin();
  let mut args_vector = std::env::args().collect::<Vec<_>>();
  let args = Args::parse_from(&args_vector);

  match &args.command {
    Some(Commands::Rename { .. }) => {
      exec_with_args(args, "").await;
    }
    _ => {
      if stdin.is_terminal() {
        exec_with_args(args, "").await;
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
