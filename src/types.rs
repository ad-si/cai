// Includes `GROQ_MODEL_MAPPING` and `OLLAMA_MODEL_MAPPING` from `/build.rs`
include!(concat!(env!("OUT_DIR"), "/models.rs"));

use clap::Subcommand;
use serde_derive::Serialize;

#[derive(Subcommand, Debug, PartialEq, Clone, Serialize)]
#[clap(args_conflicts_with_subcommands = false, arg_required_else_help(true))]
pub enum Commands {
  #[allow(dead_code)]
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
  /// Cerebras
  #[clap(visible_alias = "ce")]
  Cerebras {
    #[clap(help = cerebras_models_pretty!("Following aliases are available:"))]
    model: String,
    /// The prompt to send to the AI model
    #[clap(required(true))]
    prompt: Vec<String>,
  },
  /// DeepSeek
  #[clap(visible_alias = "de")]
  Deepseek {
    #[clap(help = deepseek_models_pretty!("Following aliases are available:"))]
    model: String,
    /// The prompt to send to the AI model
    #[clap(required(true))]
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
    #[clap(help = anthropic_models_pretty!(
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
  /// xAI
  Xai {
    #[clap(help = xai_models_pretty!(
      "Following aliases are available
(Check out https://docs.x.ai/docs/models for all supported model ids):"
    ))]
    model: String,
    /// The prompt to send to the AI model
    #[clap(required(true))]
    prompt: Vec<String>,
  },
  /// - Grok
  #[clap(name = "grok")]
  Grok {
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
  /// - Antropic Claude Sonnet 3.7
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

  /// Extract text from an image
  #[clap()]
  Ocr {
    /// The file to extract text from
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
  /// Use Fish development as the prompt context
  #[clap()]
  Fish {
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
  /// Use LilyPond development as the prompt context
  #[clap()]
  Ly {
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
  Pg {
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
  /// Use Typst development as the prompt context
  #[clap()]
  Ty {
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

impl std::fmt::Display for Commands {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self)
  }
}

impl Commands {
  pub fn to_string_pretty(&self) -> Option<String> {
    match &self {
      Commands::Groq { .. } => None,
      Commands::Cerebras { .. } => None,
      Commands::Deepseek { .. } => None,
      Commands::Llama3 { .. } => None,
      Commands::Mixtral { .. } => None,
      Commands::Openai { .. } => None,
      Commands::Gpt { .. } => None,
      Commands::GptMini { .. } => None,
      Commands::Anthropic { .. } => None,
      Commands::ClaudeOpus { .. } => None,
      Commands::ClaudeSonnet { .. } => None,
      Commands::ClaudeHaiku { .. } => None,
      Commands::Xai { .. } => None,
      Commands::Grok { .. } => None,
      Commands::Llamafile { .. } => None,
      Commands::Ollama { .. } => None,
      Commands::All { .. } => None,

      Commands::Changelog { .. } => Some("Changelog"),
      Commands::Rename { .. } => Some("Rename"),
      Commands::Ocr { .. } => Some("OCR"),

      Commands::Bash { .. } => Some("Bash"),
      Commands::C { .. } => Some("C"),
      Commands::Cpp { .. } => Some("C++"),
      Commands::Cs { .. } => Some("C#"),
      Commands::Elm { .. } => Some("Elm"),
      Commands::Fish { .. } => Some("Fish"),
      Commands::Fs { .. } => Some("F#"),
      Commands::Gd { .. } => Some("GDScript"),
      Commands::Gl { .. } => Some("Gleam"),
      Commands::Go { .. } => Some("Go"),
      Commands::Hs { .. } => Some("Haskell"),
      Commands::Java { .. } => Some("Java"),
      Commands::Js { .. } => Some("JavaScript"),
      Commands::Kt { .. } => Some("Kotlin"),
      Commands::Lua { .. } => Some("Lua"),
      Commands::Ly { .. } => Some("LilyPond"),
      Commands::Oc { .. } => Some("OCaml"),
      Commands::Php { .. } => Some("PHP"),
      Commands::Pg { .. } => Some("Postgres"),
      Commands::Ps { .. } => Some("PureScript"),
      Commands::Py { .. } => Some("Python"),
      Commands::Rb { .. } => Some("Ruby"),
      Commands::Rs { .. } => Some("Rust"),
      Commands::Sql { .. } => Some("SQLite"),
      Commands::Sw { .. } => Some("Swift"),
      Commands::Ts { .. } => Some("TypeScript"),
      Commands::Ty { .. } => Some("Typst"),
      Commands::Wl { .. } => Some("Wolfram Language"),
      Commands::Zig { .. } => Some("Zig"),
    }
    .map(|s| s.to_string())
  }
}
