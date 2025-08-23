// Includes `GROQ_MODEL_MAPPING` and `OLLAMA_MODEL_MAPPING` from `/build.rs`
include!(concat!(env!("OUT_DIR"), "/models.rs"));

use clap::Subcommand;
use serde_derive::Serialize;

#[derive(Subcommand, Debug, PartialEq, Clone, Serialize)]
#[clap(args_conflicts_with_subcommands = false, arg_required_else_help(true))]
pub enum Commands {
  /// Shortcut for `groq openai/gpt-oss-20b`
  Fast {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },

  /// Shortcut for `ollama llama3.2`
  Local {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },

  /// Return only the value/answer without explanations
  Value {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },

  /// Fix spelling, grammar, and wording issues
  /// in text passed via standard input
  Rewrite {
    /// Additional instructions for how to improve the text
    prompt: Vec<String>,
  },

  /// Reply to a conversation passed via standard input.
  /// Add additional reply instructions as the prompt.
  Reply {
    /// How the AI should reply to the conversation
    prompt: Vec<String>,
  },

  /// Generate an image using GPT-5 image generation
  #[clap(visible_alias = "img")]
  Image {
    /// The prompt describing the image to generate
    prompt: Vec<String>,
  },

  /// Transcribe an audio file
  Transcribe {
    /// The audio file to transcribe
    file: String,
  },

  /// Extract text from an image
  Ocr {
    /// The file to extract text from
    file: String,
  },

  /// Analyze and rename files to timestamp + title
  /// (e.g. 2025-08-19t2041_invoice_car.pdf)
  Rename {
    /// One or more files to analyze and rename
    #[clap(required = true)]
    files: Vec<String>,
  },

  /// Generate a changelog starting from a given commit
  Changelog {
    /// The commit hash to start the changelog from
    commit_hash: String,
  },

  /// Generate an SVG graphic from a textual description
  Svg {
    /// The prompt that describes the SVG to create
    prompt: Vec<String>,
  },

  #[clap(
    about = color_print::cformat!(
      "\n<u><em><b!>{:<60}</b!></em></u>", "📚 MODELS"
    ),
    verbatim_doc_comment,
    name = "\u{00A0}" // Non-breaking space placeholder
  )]
  SectionModels {},

  /// Simultaneously send prompt to each provider's default model
  #[clap(
    verbatim_doc_comment,
    after_help = color_print::cformat!("
<bold>Used models:</bold>

- Groq GPT OSS 20B
- Cerebras GPT OSS 120B
- Anthropic Claude Sonnet 4.0
- Google Gemini 2.5 Flash
- OpenAI GPT-5 mini
- Ollama Llama 3
- Llamafile
"))]
  // Include linebreaks
  All {
    /// The prompt to send to the AI models simultaneously
    prompt: Vec<String>,
  },

  #[allow(dead_code)]
  /// Google
  #[clap(visible_alias = "go")]
  Google {
    #[clap(help = google_models_pretty!("Following aliases are available:"))]
    model: String,
    /// The prompt to send to the AI model
    #[clap(required(true))]
    prompt: Vec<String>,
  },
  /// - Gemini Pro shortcut
  #[clap(name = "gemini", visible_alias = "ge")]
  Gemini {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - Gemini Flash shortcut
  #[clap(name = "flash", visible_alias = "gf")]
  GeminiFlash {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Groq
  #[clap(visible_alias = "gr")]
  Groq {
    #[clap(help = groq_models_pretty!("Following aliases are available:"))]
    model: String,
    /// The prompt to send to the AI model
    #[clap(required(true))]
    prompt: Vec<String>,
  },
  /// - Llama 3 shortcut
  #[clap(name = "llama", visible_alias = "ll")]
  Llama3 {
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
  #[clap(visible_alias = "ds")]
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
  /// - GPT-5 shortcut
  #[clap(name = "gpt5", visible_alias = "gpt", visible_alias = "gp")]
  Gpt5 {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - GPT-5 mini shortcut
  #[clap(name = "gpt5m", visible_alias = "gm")]
  Gpt5Mini {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - GPT-5 nano shortcut
  #[clap(name = "gpt5n", visible_alias = "gn")]
  Gpt5Nano {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - gpt-4.1 shortcut
  #[clap(name = "gpt41")]
  Gpt41 {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - gpt-4.1-mini shortcut
  #[clap(name = "gpt41m")]
  Gpt41Mini {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - gpt-4.1-nano shortcut
  #[clap(name = "gpt41n")]
  Gpt41Nano {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - o1-pro shortcut
  #[clap(name = "o1p")]
  O1Pro {
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
  #[clap(name = "opus", visible_alias = "claude", visible_alias = "cl")]
  ClaudeOpus {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - Claude Sonnet
  #[clap(name = "sonnet", visible_alias = "so")]
  ClaudeSonnet {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - Claude Haiku
  #[clap(name = "haiku", visible_alias = "ha")]
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

  #[clap(
    about = color_print::cformat!(
      "\n<u><em><b!>{:<60}</b!></em></u>", "💻 CODING"
    ),
    verbatim_doc_comment,
    name = "\u{00A0}\u{00A0}" // Non-breaking space placeholder
  )]
  // Non-breaking space
  SectionCoding {},

  /// Use Bash development as the prompt context
  Bash {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use C development as the prompt context
  C {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use C++ development as the prompt context
  Cpp {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use C# development as the prompt context
  Cs {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Docker development as the prompt context
  Docker {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Elm development as the prompt context
  Elm {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Fish development as the prompt context
  Fish {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use F# development as the prompt context
  Fs {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Godot and GDScript development as the prompt context
  Gd {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Git development as the prompt context
  Git {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Gleam development as the prompt context
  Gl {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Go development as the prompt context
  Golang {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Haskell development as the prompt context
  Hs {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Java development as the prompt context
  Java {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use JavaScript development as the prompt context
  Js {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Kotlin development as the prompt context
  Kt {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use LilyPond development as the prompt context
  Ly {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Lua development as the prompt context
  Lua {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Nix development as the prompt context
  Nix {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use OCaml development as the prompt context
  Oc {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use PHP development as the prompt context
  Php {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Postgres development as the prompt context
  Pg {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use PureScript development as the prompt context
  Ps {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Python development as the prompt context
  Py {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Ruby development as the prompt context
  Rb {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Rust development as the prompt context
  Rs {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use SQLite development as the prompt context
  Sql {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Swift development as the prompt context
  Sw {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use TypeScript development as the prompt context
  Ts {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Typst development as the prompt context
  Ty {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Wolfram Language and Mathematica development as the prompt context
  Wl {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// Use Zig development as the prompt context
  Zig {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },

  /// Use jq development as the prompt context
  #[clap(name = "jq")]
  Jq {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
}

impl std::fmt::Display for Commands {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{self:?}")
  }
}

impl Commands {
  pub fn to_string_pretty(&self) -> Option<String> {
    match &self {
      Commands::Fast { .. } => None,
      Commands::Local { .. } => None,
      Commands::Value { .. } => Some("Value"),
      Commands::Svg { .. } => Some("SVG"),
      Commands::Ocr { .. } => Some("OCR"),
      Commands::Rename { .. } => Some("Rename"),
      Commands::Changelog { .. } => Some("Changelog"),
      Commands::Reply { .. } => Some("Reply"),
      Commands::Rewrite { .. } => Some("Rewrite"),
      Commands::Transcribe { .. } => Some("Transcribe"),
      Commands::Image { .. } => Some("Image"),

      // Models
      Commands::SectionModels { .. } => None,
      Commands::All { .. } => None,

      Commands::Google { .. } => None,
      Commands::Gemini { .. } => None,
      Commands::GeminiFlash { .. } => None,
      Commands::Groq { .. } => None,
      Commands::Cerebras { .. } => None,
      Commands::Deepseek { .. } => None,
      Commands::Llama3 { .. } => None,
      Commands::Openai { .. } => None,
      Commands::Gpt5 { .. } => None,
      Commands::Gpt5Mini { .. } => None,
      Commands::Gpt5Nano { .. } => None,
      Commands::Gpt41 { .. } => None,
      Commands::Gpt41Mini { .. } => None,
      Commands::Gpt41Nano { .. } => None,
      Commands::O1Pro { .. } => None,
      Commands::Anthropic { .. } => None,
      Commands::ClaudeOpus { .. } => None,
      Commands::ClaudeSonnet { .. } => None,
      Commands::ClaudeHaiku { .. } => None,
      Commands::Xai { .. } => None,
      Commands::Grok { .. } => None,
      Commands::Llamafile { .. } => None,
      Commands::Ollama { .. } => None,

      // Coding
      Commands::SectionCoding { .. } => None,
      Commands::Bash { .. } => Some("Bash"),
      Commands::C { .. } => Some("C"),
      Commands::Cpp { .. } => Some("C++"),
      Commands::Cs { .. } => Some("C#"),
      Commands::Docker { .. } => Some("Docker"),
      Commands::Elm { .. } => Some("Elm"),
      Commands::Fish { .. } => Some("Fish"),
      Commands::Fs { .. } => Some("F#"),
      Commands::Gd { .. } => Some("GDScript"),
      Commands::Git { .. } => Some("Git"),
      Commands::Gl { .. } => Some("Gleam"),
      Commands::Golang { .. } => Some("Go"),
      Commands::Hs { .. } => Some("Haskell"),
      Commands::Java { .. } => Some("Java"),
      Commands::Js { .. } => Some("JavaScript"),
      Commands::Kt { .. } => Some("Kotlin"),
      Commands::Lua { .. } => Some("Lua"),
      Commands::Ly { .. } => Some("LilyPond"),
      Commands::Nix { .. } => Some("Nix"),
      Commands::Oc { .. } => Some("OCaml"),
      Commands::Pg { .. } => Some("Postgres"),
      Commands::Php { .. } => Some("PHP"),
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
      Commands::Jq { .. } => Some("JQ"),
    }
    .map(|s| s.to_string())
  }
}
