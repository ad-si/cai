use std::io::stdin;
use std::io::{read_to_string, IsTerminal};

use cai::{
  analyze_file_content, create_commits, exec_tool, extract_text_from_file,
  generate_changelog, google_ocr_file, prompt_with_lang_cntxt, submit_prompt,
  transcribe_audio_file, Commands, ExecOptions, Model, Provider,
};
use chrono::NaiveDateTime;
use clap::crate_description;
use clap::{builder::styling, crate_version, Parser};
use color_print::cformat;
use futures::future::join_all;
use serde_json::{json, Value};
use std::error::Error;

// Rename a single file
async fn process_rename(
  opts: &ExecOptions,
  file: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  match analyze_file_content(opts, file).await {
    Ok(analysis) => {
      let timestamp_str = analysis.timestamp.unwrap_or_default();
      let timestamp_norm = timestamp_str.trim().to_lowercase();
      let valid_timestamp =
        NaiveDateTime::parse_from_str(&timestamp_norm, "%Y-%m-%dt%H:%Mz")
          .or_else(|_| {
            NaiveDateTime::parse_from_str(
              &(timestamp_norm.clone() + "t00:00z"),
              "%Y-%m-%dt%H:%Mz",
            )
          })
          .is_ok();
      let timestamp = if valid_timestamp {
        timestamp_norm.replace([':', 'z'], "").replace("t0000", "")
      } else {
        chrono::Local::now().format("%Y-%m-%dt%H%M").to_string()
      };
      let description = analysis //
        .description
        .trim()
        .to_lowercase()
        .replace(' ', "_")
        // Remove any non-alphanumeric characters
        .replace(
          |c: char| {
            !c.is_ascii_alphanumeric()
              && c != '_'
              && c != '-'
              && c != 'ä'
              && c != 'ö'
              && c != 'ü'
              && c != 'ß'
          },
          "",
        );
      rename_file(file.to_string(), timestamp, description);
      Ok(())
    }
    Err(error) => match error.downcast_ref::<std::io::Error>() {
      Some(err) if err.kind() == std::io::ErrorKind::InvalidData => {
        // If it's not a text file, use the creation time
        let timestamp = std::fs::metadata(file)
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
          })
          .unwrap_or_else(|_| {
            chrono::Local::now().format("%Y-%m-%dt%H%M").to_string()
          });

        std::path::Path::new(file)
          .file_stem()
          .and_then(|file_name_no_ext| {
            file_name_no_ext.to_str().map(|s| s.to_string())
          })
          .map(|file_name| rename_file(file.to_string(), timestamp, file_name))
          .ok_or_else(|| {
            // Could not rename -> propagate error
            Box::<dyn Error + Send + Sync>::from("Failed to rename file")
          })?;
        Ok(())
      }
      _ => Err(error),
    },
  }
}

const CRATE_VERSION: &str = crate_version!();

#[derive(Parser, Debug)]
// #[command(version, about, long_about = None)]
#[clap(
  trailing_var_arg = true,
  about = color_print::cformat!(
    "<bold,underline>Cai {}</bold,underline>\n\n\
      <black,bold>{}</black,bold>",
    CRATE_VERSION,
    crate_description!(),
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
  <b>cai anthropic claude-opus-4-1</b> Which year did the Titanic sink

  <dim># Send a prompt to locally running Ollama server</dim>
  <b>cai ollama llama3</b> Which year did the Titanic sink
  <b>cai ol ll</b> Which year did the Titanic sink

  <dim># Use the `local` shortcut for using Ollama's default model</dim>
  <b>cai local</b> Which year did the Titanic sink

  <dim># Add data via stdin</dim>
  cat main.rs | <b>cai</b> Explain this code

  <dim># Get raw output without any metadata</dim>
  <b>cai --raw capital of Germany</b>

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

fn shell_single_quote(arg: &str) -> String {
  // Wrap an argument for POSIX shells using single quotes,
  // escaping any embedded single quotes.
  if arg.is_empty() {
    "''".to_string()
  } else {
    format!("'{}'", arg.replace('\'', "'\\''"))
  }
}

async fn exec_with_args(args: Args, stdin: &str) {
  let stdin = if stdin.is_empty() {
    "".into()
  } else {
    format!("{stdin}\n")
  };
  let opts = ExecOptions {
    is_raw: args.raw,
    is_json: args.json,
    json_schema: args
      .json_schema
      .and_then(|schema_str| {
        serde_json::from_str(&schema_str).expect("Invalid JSON schema")
      })
      .map(|schema: Value| {
        let mut schema_obj = schema.as_object().unwrap().clone();
        schema_obj.insert("additionalProperties".to_string(), false.into());
        if !schema_obj.contains_key("type") {
          schema_obj.insert("type".to_string(), "object".into());
        }
        json!({
          "name": "requested_json_schema",
          "strict": true,
          "schema": schema_obj,
        })
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
      Commands::Fast { prompt } => {
        submit_prompt(
          &Some(&Model::Model(
            Provider::Groq,
            "openai/gpt-oss-20b".to_string(),
          )),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Local { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Ollama, "llama3.2".to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Value { prompt } => {
        let value_prompt = format!(
          "I want you to return only a plain value without explanation or additional text. \
          Respond with the answer and nothing else. Do not include any explanation, \
          reasoning, or additional information. Just give me the answer value.\n\n{}",
          prompt.join(" ")
        );
        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, "gpt-4.1".to_string())),
          &opts,
          &format!("{stdin}{value_prompt}"),
        )
        .await
      }
      Commands::Short { prompt } => {
        let short_prompt = format!(
          "Please provide a short, compact, and focused answer to the following question. \
          Be concise and to the point while still being accurate and complete. \
          Avoid unnecessary elaboration or tangential information.\n\n{}",
          prompt.join(" ")
        );
        submit_prompt(&None, &opts, &format!("{stdin}{short_prompt}")).await
      }
      Commands::Svg { prompt } => {
        // Force raw so only the SVG markup is printed
        let mut opts_svg = opts.clone();
        opts_svg.is_raw = true;

        let svg_prompt = format!(
          "Generate an SVG image according to the following description. \
           Respond ONLY with valid SVG markup – no explanations, no code fences.\n\n{}",
          prompt.join(" ")
        );

        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, "gpt-4o-mini".to_string())),
          &opts_svg,
          &format!("{stdin}{svg_prompt}"),
        )
        .await
      }
      Commands::Edit {} => {
        // Create a temp file and open it in the user's editor
        let mut tmp_path = std::env::temp_dir();
        let ts = chrono::Local::now().format("%Y%m%d%H%M%S");
        tmp_path.push(format!("cai_prompt_{ts}.txt"));

        // Ensure the file exists
        if let Err(err) = std::fs::File::create(&tmp_path) {
          eprintln!("Failed to create temp file: {err}");
          std::process::exit(1);
        }

        let tmp_str = tmp_path.to_string_lossy().to_string();

        // Prefer VISUAL, then EDITOR, else sensible OS-specific fallbacks
        let editor_var = std::env::var("VISUAL")
          .ok()
          .or_else(|| std::env::var("EDITOR").ok());

        let status = if let Some(editor_cmd) = editor_var {
          if editor_cmd.contains(' ') {
            // Complex editor command with args, run via shell
            let cmdline =
              format!("{} {}", editor_cmd, shell_single_quote(&tmp_str));
            std::process::Command::new("sh")
              .arg("-c")
              .arg(cmdline)
              .status()
          } else {
            std::process::Command::new(editor_cmd)
              .arg(&tmp_str)
              .status()
          }
        } else if cfg!(target_os = "macos") {
          std::process::Command::new("open")
            .args(["-t", "-W", &tmp_str])
            .status()
        } else if cfg!(target_os = "windows") {
          std::process::Command::new("cmd")
            .args(["/C", "start", "/WAIT", "notepad", &tmp_str])
            .status()
        } else {
          // Generic UNIX fallback
          std::process::Command::new("nano").arg(&tmp_str).status()
        };

        match status {
          Ok(st) if st.success() => {}
          _ => {
            eprintln!("Failed to launch editor or editor exited with error");
            std::process::exit(1);
          }
        }

        let content = std::fs::read_to_string(&tmp_path)
          .unwrap_or_default()
          .trim()
          .to_string();

        if content.is_empty() {
          eprintln!("No prompt written (file empty). Aborting.");
          std::process::exit(1);
        }

        // Echo the prompt back to the user unless raw mode is requested
        if !opts.is_raw {
          let echoed = content.replace('\n', "\n> ");
          println!("> {echoed}\n");
        }

        submit_prompt(&None, &opts, &content).await
      }
      Commands::Config {} => {
        use cai::get_full_config;
        let xdg_dirs = xdg::BaseDirectories::with_prefix("cai").unwrap();
        let secrets_path = xdg_dirs
          .place_config_file("secrets.yaml")
          .expect("Couldn't create configuration directory");
        let secrets_path_str = secrets_path.to_str().unwrap();

        match get_full_config(secrets_path_str) {
          Ok(config) => {
            println!("Configuration loaded from: {secrets_path_str}\n");
            println!("Settings:");

            // Sort keys for consistent output
            let mut keys: Vec<_> = config.keys().collect();
            keys.sort();

            for key in keys {
              let value = config.get(key).unwrap();
              // Mask sensitive API keys - show first 4 and last 4 characters
              if key.contains("api_key") && value.len() > 12 {
                let masked =
                  format!("{}...{}", &value[..4], &value[value.len() - 4..]);
                println!("  {}: {}", key, masked);
              } else if key.contains("api_key") && !value.is_empty() {
                println!("  {}: ****", key);
              } else if !value.is_empty() {
                println!("  {}: {}", key, value);
              }
            }
          }
          Err(err) => {
            eprintln!("Failed to load configuration: {err}");
            std::process::exit(1);
          }
        }
      }
      Commands::Ocr { file } => {
        if let Err(err) = extract_text_from_file(&opts, file).await {
          eprintln!("Error extracting text: {err}");
          std::process::exit(1);
        }
      }
      Commands::GoogleOcr { file } => {
        if let Err(err) = google_ocr_file(&opts, file).await {
          eprintln!("Error extracting text with Google OCR: {err}");
          std::process::exit(1);
        }
      }
      Commands::Rename { files } => {
        for file in files {
          if let Err(e) = process_rename(&opts, file).await {
            eprintln!("{e}");
            std::process::exit(1);
          }
        }
      }
      Commands::Changelog { commit_hash } => {
        if let Err(err) = generate_changelog(&opts, commit_hash).await {
          eprintln!("Error generating changelog: {err}");
          std::process::exit(1);
        }
      }
      Commands::Commit {} => {
        if let Err(err) = create_commits(&opts).await {
          eprintln!("Error creating commits: {err}");
          std::process::exit(1);
        }
      }
      Commands::Reply { prompt } => {
        if stdin.is_empty() {
          eprintln!("Please pipe the conversation into cai via stdin.");
          std::process::exit(1);
        }
        let username = whoami::username();
        let reply_prompt = format!(
          "Given the following conversation, write the best possible reply. \
            Do not print a timestamp or a name at the beginning of your reply. \
            You are {username} and you reply to the other person/persons.\n\
            Conversation:\n{stdin}\n\n\
            Reply guidance: {}\n",
          prompt.join(" ")
        );

        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, "gpt-4.1".to_string())),
          &opts,
          &reply_prompt,
        )
        .await
      }
      Commands::Rewrite { prompt } => {
        if stdin.is_empty() {
          eprintln!("Please pipe the text to be rewritten into cai via stdin.");
          std::process::exit(1);
        }
        let base_rewrite_prompt = "\
          Fix any spelling mistakes, grammatical errors, \
            and wording issues in the following text. \
          Maintain the original meaning and tone \
            while improving clarity and correctness. \
          Return only the corrected text \
            without any explanations or additional commentary.";

        let rewrite_prompt = if prompt.is_empty() {
          format!(
            "{base_rewrite_prompt}\n\n\
            Text to rewrite:\n{stdin}"
          )
        } else {
          format!(
            "{base_rewrite_prompt}\n\n
            Additional instructions: {}\n\n\
            Text to correct:\n\
            {stdin}",
            prompt.join(" ")
          )
        };

        let mut rewrite_opts = opts.clone();
        rewrite_opts.is_raw = true;

        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, "gpt-4.1".to_string())),
          &rewrite_opts,
          &rewrite_prompt,
        )
        .await
      }
      Commands::Transcribe { file } => {
        if let Err(_err) = transcribe_audio_file(&opts, file).await {
          eprintln!("Error transcribing file: {{_err}}");
          std::process::exit(1);
        }
      }
      Commands::Say { prompt } => {
        submit_prompt(
          &Some(&Model::Model(
            Provider::OpenAI,
            "gpt-4o-mini-tts".to_string(),
          )),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Image { prompt } => {
        let image_prompt = prompt.join(" ").to_string();
        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, "gpt-5".to_string())),
          &opts,
          &format!("{stdin}{image_prompt}"),
        )
        .await
      }

      //////////////////////////////////////////////////////////////////////////
      //=============================== MODELS =================================
      //////////////////////////////////////////////////////////////////////////
      Commands::SectionModels {} => {}
      Commands::Google { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Google, model.to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Gemini { prompt } => {
        submit_prompt(
          &Some(&Model::Model(
            Provider::Google,
            "gemini-2.5-flash".to_string(),
          )),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::GeminiFlash { prompt } => {
        submit_prompt(
          &Some(&Model::Model(
            Provider::Google,
            "gemini-2.5-flash".to_string(),
          )),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::GoogleImage { prompt } => {
        submit_prompt(
          &Some(&Model::Model(
            Provider::Google,
            "gemini-2.5-flash-image".to_string(),
          )),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Groq { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Groq, model.to_string())),
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
      Commands::Gpt5 { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, "gpt-5".to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Gpt5Mini { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, "gpt-5-mini".to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Gpt5Nano { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, "gpt-5-nano".to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Gpt41 { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, "gpt-4.1".to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Gpt41Mini { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, "gpt-4.1-mini".to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Gpt41Nano { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, "gpt-4.1-nano".to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::O1Pro { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::OpenAI, "o1-pro".to_string())),
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
            "claude-opus-4-1".to_string(),
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
            "claude-sonnet-4-5".to_string(),
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
          &Some(&Model::Model(Provider::XAI, "grok-4-latest".to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Perplexity { model, prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Perplexity, model.to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::Sonar { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Perplexity, "sonar".to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::SonarPro { prompt } => {
        submit_prompt(
          &Some(&Model::Model(Provider::Perplexity, "sonar-pro".to_string())),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::SonarReasoning { prompt } => {
        submit_prompt(
          &Some(&Model::Model(
            Provider::Perplexity,
            "sonar-reasoning".to_string(),
          )),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::SonarReasoningPro { prompt } => {
        submit_prompt(
          &Some(&Model::Model(
            Provider::Perplexity,
            "sonar-reasoning-pro".to_string(),
          )),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::SonarDeepResearch { prompt } => {
        submit_prompt(
          &Some(&Model::Model(
            Provider::Perplexity,
            "sonar-deep-research".to_string(),
          )),
          &opts,
          &format!("{stdin}{}", prompt.join(" ")),
        )
        .await
      }
      Commands::All { prompt } => {
        let models = vec![
          Model::Model(Provider::Anthropic, "claude-sonnet-4-5".to_string()),
          Model::Model(Provider::Cerebras, "gpt-oss-120b".to_string()),
          Model::Model(Provider::Google, "gemini-2.5-flash".to_string()),
          Model::Model(Provider::Groq, "openai/gpt-oss-20b".to_string()),
          Model::Model(Provider::Llamafile, "".to_string()),
          Model::Model(Provider::Ollama, "llama3".to_string()),
          Model::Model(Provider::OpenAI, "gpt-5-mini".to_string()),
          Model::Model(Provider::XAI, "grok-3-mini-latest".to_string()),
          Model::Model(Provider::Perplexity, "sonar".to_string()),
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

      //////////////////////////////////////////////////////////////////////////
      //================================ CODING ================================
      //////////////////////////////////////////////////////////////////////////
      Commands::SectionCoding {} => {}
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
      Commands::Golang { prompt } => {
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
      Commands::Nix { prompt } => {
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
      Commands::Docker { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Git { prompt } => {
        prompt_with_lang_cntxt(&opts, &cmd, prompt).await
      }
      Commands::Jq { prompt } => {
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
    .join(format!("{timestamp}_{description}.{ext}"))
    .to_str()
    .unwrap()
    .to_string();

  let mut counter = 0;
  loop {
    if std::path::Path::new(&new_name).exists() {
      counter += 1;
      new_name = format!("{timestamp}_{description}_{counter}.{ext}")
    } else {
      break;
    }
  }

  if let Err(err) = std::fs::rename(&file, &new_name) {
    eprintln!("Error renaming file: {err}");
    std::process::exit(1);
  }
  println!("Renamed {file} to {new_name}");
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
    let parse_res = Args::try_parse_from(["gpt"]);
    assert!(parse_res.is_err());
    assert!(&parse_res.unwrap_err().to_string().contains("Usage: gpt"));
  }
}
