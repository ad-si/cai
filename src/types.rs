// Includes `GROQ_MODEL_MAPPING` and `OLLAMA_MODEL_MAPPING` from `/build.rs`
include!(concat!(env!("OUT_DIR"), "/models.rs"));

use clap::Subcommand;
use serde_derive::Serialize;

#[derive(Subcommand, Debug, PartialEq, Clone, Serialize)]
#[clap(args_conflicts_with_subcommands = false, arg_required_else_help(true))]
pub enum Commands {
  /// Shortcut for `cerebras gpt-oss-120b`
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

  /// Answer the prompt in a short, compact, and focused manner
  Short {
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

  /// Generate a shell command from a prompt and confirm before executing it
  Run {
    /// Description of what the shell command should do
    prompt: Vec<String>,
  },

  /// Run an agentic loop with tool use to fulfill a request
  Agent {
    /// When to ask for permission before running a tool
    #[clap(long, value_parser = ["always", "edit", "never"])]
    ask_for_permission: Option<String>,
    /// The task for the agent to perform
    prompt: Vec<String>,
  },

  /// Generate an image using GPT-image-2
  #[clap(visible_alias = "img")]
  Image {
    /// Background behavior for the generated image
    #[clap(long, value_parser = ["transparent", "opaque", "auto"])]
    background: Option<String>,
    /// The prompt describing the image to generate
    prompt: Vec<String>,
  },

  /// Generate a photorealistic image that looks like a camera photo
  Photo {
    /// The prompt describing the photo to generate
    prompt: Vec<String>,
  },

  /// Edit 1 or more images using GPT-image-2
  /// (pass image files followed by the edit prompt as the last argument)
  #[clap(
    name = "imgedit",
    visible_alias = "imge",
    override_usage = "cai imgedit [--background <BG>] <IMAGE>... <PROMPT>"
  )]
  ImgEdit {
    /// Background behavior for the edited image
    #[clap(long, value_parser = ["transparent", "opaque", "auto"])]
    background: Option<String>,
    /// One or more image files followed by the edit prompt as the last argument
    #[clap(required = true, num_args = 1.., value_name = "IMAGE>... <PROMPT")]
    args: Vec<String>,
  },

  /// Convert text to speech using OpenAI's TTS model
  #[clap(visible_alias = "tts")]
  Say {
    /// The text to convert to speech
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

  /// Extract text from an image using Google Gemini with high resolution
  #[clap(visible_alias = "gocr")]
  GoogleOcr {
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

  /// Commit modified files with AI-generated commit messages and
  /// group related changes into separate commits
  Commit {},

  /// Generate an SVG graphic from a textual description
  Svg {
    /// The prompt that describes the SVG to create
    prompt: Vec<String>,
  },

  /// Open your editor to write the prompt
  Edit {},

  /// Print the configuration settings loaded from the config file
  Config {},

  #[clap(
    about = color_print::cformat!(
      "\n<u><em><b!>{:<60}</b!></em></u>", "📚 MODELS"
    ),
    verbatim_doc_comment,
    name = "\u{00A0}" // Non-breaking space placeholder
  )]
  SectionModels {},

  /// List all models offered by every supported provider
  /// (OpenAI, Anthropic, Gemini, Groq, Cerebras, DeepSeek, xAI,
  /// Perplexity, Ollama, Mistral)
  #[clap(verbatim_doc_comment)]
  Models {},

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
  /// - Google Gemini Image shortcut
  #[clap(name = "google-image", visible_alias = "gimg")]
  GoogleImage {
    /// The prompt describing the image to generate
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
  /// Perplexity
  #[clap(visible_alias = "pe")]
  Perplexity {
    #[clap( help = perplexity_models_pretty!(
      "Following aliases are available
(Check out https://docs.perplexity.ai/getting-started/models for all supported model ids):"
    ))]
    model: String,
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - Sonar
  #[clap(name = "son")]
  Sonar {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - Sonar Pro
  #[clap(name = "sonpro", visible_alias = "sp")]
  SonarPro {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - Sonar Reasoning
  #[clap(name = "sonreas", visible_alias = "sr")]
  SonarReasoning {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - Sonar Reasoning Pro
  #[clap(name = "sonreaspro", visible_alias = "srp")]
  SonarReasoningPro {
    /// The prompt to send to the AI model
    prompt: Vec<String>,
  },
  /// - Sonar Deep Research
  #[clap(name = "sondeep", visible_alias = "sdr")]
  SonarDeepResearch {
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

  #[clap(
    about = color_print::cformat!(
      "\n<u><em><b!>{:<60}</b!></em></u>", "🗄️ DATABASE"
    ),
    verbatim_doc_comment,
    name = "\u{00A0}\u{00A0}\u{00A0}" // Non-breaking space placeholder
  )]
  SectionDatabase {},

  /// Query a SQLite database using natural language
  Query {
    /// Path to the SQLite database file
    #[clap(required = true)]
    database: String,
    /// The natural language query/question about the data
    #[clap(required = true)]
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
      Commands::Edit { .. } => None,
      Commands::Fast { .. } => None,
      Commands::Local { .. } => None,
      Commands::Value { .. } => Some("Value"),
      Commands::Short { .. } => Some("Short"),
      Commands::Svg { .. } => Some("SVG"),
      Commands::Ocr { .. } => Some("OCR"),
      Commands::GoogleOcr { .. } => Some("Google OCR"),
      Commands::Rename { .. } => Some("Rename"),
      Commands::Changelog { .. } => Some("Changelog"),
      Commands::Commit { .. } => Some("Commit"),
      Commands::Reply { .. } => Some("Reply"),
      Commands::Run { .. } => Some("Run"),
      Commands::Agent { .. } => Some("Agent"),
      Commands::Rewrite { .. } => Some("Rewrite"),
      Commands::Config { .. } => None,
      Commands::Transcribe { .. } => Some("Transcribe"),
      Commands::Say { .. } => Some("Say"),
      Commands::Image { .. } => Some("Image"),
      Commands::Photo { .. } => Some("Photo"),
      Commands::ImgEdit { .. } => Some("Image Edit"),

      // Models
      Commands::SectionModels { .. } => None,
      Commands::Models { .. } => None,
      Commands::All { .. } => None,

      Commands::Google { .. } => None,
      Commands::Gemini { .. } => None,
      Commands::GeminiFlash { .. } => None,
      Commands::GoogleImage { .. } => Some("Google Image"),
      Commands::Groq { .. } => None,
      Commands::Perplexity { .. } => None,
      Commands::Sonar { .. } => None,
      Commands::SonarPro { .. } => None,
      Commands::SonarReasoning { .. } => None,
      Commands::SonarReasoningPro { .. } => None,
      Commands::SonarDeepResearch { .. } => None,
      Commands::Cerebras { .. } => None,
      Commands::Deepseek { .. } => None,
      Commands::Llama3 { .. } => None,
      Commands::Openai { .. } => None,
      Commands::Gpt5 { .. } => None,
      Commands::Gpt5Mini { .. } => None,
      Commands::Gpt5Nano { .. } => None,
      Commands::Gpt41 { .. } => None,
      Commands::Gpt41Mini { .. } => None,
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

      // Database
      Commands::SectionDatabase { .. } => None,
      Commands::Query { .. } => Some("Query"),
    }
    .map(|s| s.to_string())
  }

  /// Config key prefix used to override a shortcut's default model via
  /// `~/.config/cai/config.yaml` (the looked-up key is `<prefix>_model`).
  ///
  /// Returns `None` for commands that don't have an overridable default model,
  /// i.e. commands that already take an explicit model argument
  /// (e.g. `anthropic`, `openai`) or that are tied to a specific modality
  /// (e.g. image generation, text-to-speech, OCR).
  pub fn config_key(&self) -> Option<&'static str> {
    match self {
      // Generic chat shortcuts
      Commands::Fast { .. } => Some("fast"),
      Commands::Local { .. } => Some("local"),
      Commands::Value { .. } => Some("value"),
      Commands::Short { .. } => Some("short"),
      Commands::Svg { .. } => Some("svg"),
      Commands::Reply { .. } => Some("reply"),
      Commands::Rewrite { .. } => Some("rewrite"),

      // Provider default-model shortcuts
      Commands::Gemini { .. } => Some("gemini"),
      Commands::GeminiFlash { .. } => Some("flash"),
      Commands::Llama3 { .. } => Some("llama"),
      Commands::Gpt5 { .. } => Some("gpt5"),
      Commands::Gpt5Mini { .. } => Some("gpt5m"),
      Commands::Gpt5Nano { .. } => Some("gpt5n"),
      Commands::Gpt41 { .. } => Some("gpt41"),
      Commands::Gpt41Mini { .. } => Some("gpt41m"),
      Commands::ClaudeOpus { .. } => Some("opus"),
      Commands::ClaudeSonnet { .. } => Some("sonnet"),
      Commands::ClaudeHaiku { .. } => Some("haiku"),
      Commands::Grok { .. } => Some("grok"),
      Commands::Sonar { .. } => Some("sonar"),
      Commands::SonarPro { .. } => Some("sonpro"),
      Commands::SonarReasoning { .. } => Some("sonreas"),
      Commands::SonarReasoningPro { .. } => Some("sonreaspro"),
      Commands::SonarDeepResearch { .. } => Some("sondeep"),

      // Coding (language context) shortcuts
      Commands::Bash { .. } => Some("bash"),
      Commands::C { .. } => Some("c"),
      Commands::Cpp { .. } => Some("cpp"),
      Commands::Cs { .. } => Some("cs"),
      Commands::Docker { .. } => Some("docker"),
      Commands::Elm { .. } => Some("elm"),
      Commands::Fish { .. } => Some("fish"),
      Commands::Fs { .. } => Some("fs"),
      Commands::Gd { .. } => Some("gd"),
      Commands::Git { .. } => Some("git"),
      Commands::Gl { .. } => Some("gl"),
      Commands::Golang { .. } => Some("go"),
      Commands::Hs { .. } => Some("hs"),
      Commands::Java { .. } => Some("java"),
      Commands::Js { .. } => Some("js"),
      Commands::Kt { .. } => Some("kt"),
      Commands::Lua { .. } => Some("lua"),
      Commands::Ly { .. } => Some("ly"),
      Commands::Nix { .. } => Some("nix"),
      Commands::Oc { .. } => Some("oc"),
      Commands::Pg { .. } => Some("pg"),
      Commands::Php { .. } => Some("php"),
      Commands::Ps { .. } => Some("ps"),
      Commands::Py { .. } => Some("py"),
      Commands::Rb { .. } => Some("rb"),
      Commands::Rs { .. } => Some("rs"),
      Commands::Sql { .. } => Some("sql"),
      Commands::Sw { .. } => Some("sw"),
      Commands::Ts { .. } => Some("ts"),
      Commands::Ty { .. } => Some("ty"),
      Commands::Wl { .. } => Some("wl"),
      Commands::Zig { .. } => Some("zig"),
      Commands::Jq { .. } => Some("jq"),

      // Everything else has no overridable default model
      _ => None,
    }
  }
}
