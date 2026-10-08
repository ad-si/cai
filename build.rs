use std::env;
use std::fs;
use std::path::Path;

const GOOGLE_MODEL_MAPPING_SRC: [(&str, &str); 35] = [
  // Default models
  ("gemini-flash", "gemini-3.8-flash"),
  ("gemini", "gemini-3.8-flash"),
  ("g", "gemini-3.8-flash"),
  ("flash", "gemini-3.8-flash"),
  ("f", "gemini-3.8-flash"),
  ("gemini-pro", "gemini-3.1-pro-preview"),
  ("pro", "gemini-3.1-pro-preview"),
  ("gemini-flash-lite", "gemini-3.5-flash-lite"),
  ("flash-lite", "gemini-3.5-flash-lite"),
  ("lite", "gemini-3.5-flash-lite"),
  // Image generation models
  ("gemini-image", "gemini-3.1-flash-image"),
  ("image", "gemini-3.1-flash-image"),
  ("img", "gemini-3.1-flash-image"),
  ("image-lite", "gemini-3.1-flash-lite-image"),
  ("pro-image", "gemini-3-pro-image"),
  ("nano-banana", "nano-banana-pro-preview"),
  ("banana", "nano-banana-pro-preview"),
  // Music generation models (Lyria)
  ("lyria", "lyria-3.5"),
  ("music", "lyria-3.5"),
  ("lyria-pro", "lyria-3-pro-preview"),
  ("lyria-clip", "lyria-3-clip-preview"),
  ("clip", "lyria-3-clip-preview"),
  // Speech models
  ("tts", "gemini-3.1-flash-tts-preview"),
  ("say", "gemini-3.1-flash-tts-preview"),
  ("transcribe", "gemini-3.5-transcribe"),
  // Embedding models
  ("embed", "gemini-embedding-2"),
  ("embedding", "gemini-embedding-2"),
  // Any-to-any model (also generates videos)
  ("omni", "gemini-omni-1.1-flash"),
  ("video", "gemini-omni-1.1-flash"),
  // Open models (Gemma)
  ("gemma", "gemma-4-31b-it"),
  ("gemma-4", "gemma-4-31b-it"),
  ("gemma-moe", "gemma-4-26b-a4b-it"),
  // Robotics model
  ("robotics", "gemini-robotics-er-2-preview"),
  // Version 3 family shortcuts
  ("gemini-3-pro", "gemini-3.1-pro-preview"),
  ("gemini-3-flash", "gemini-3.8-flash"),
];

const ANTHROPIC_MODEL_MAPPING_SRC: [(&str, &str); 29] = [
  // Default models
  // Fable (most powerful)
  ("claude-fable", "claude-fable-5"),
  ("fable", "claude-fable-5"),
  ("fa", "claude-fable-5"),
  // Opus
  ("claude-opus", "claude-opus-5-5"),
  ("opus", "claude-opus-5-5"),
  ("op", "claude-opus-5-5"),
  ("o", "claude-opus-5-5"),
  // Sonnet
  ("claude-sonnet", "claude-sonnet-5-5"),
  ("sonnet", "claude-sonnet-5-5"),
  ("so", "claude-sonnet-5-5"),
  ("s", "claude-sonnet-5-5"),
  // Haiku
  ("claude-haiku", "claude-haiku-4-5"),
  ("haiku", "claude-haiku-4-5"),
  ("ha", "claude-haiku-4-5"),
  ("h", "claude-haiku-4-5"),
  // Version 5.5 models
  ("opus-5-5", "claude-opus-5-5"),
  ("sonnet-5-5", "claude-sonnet-5-5"),
  // Version 5 models
  ("fable-5", "claude-fable-5"),
  ("sonnet-5", "claude-sonnet-5"),
  // Version 4.8 models
  ("opus-4-8", "claude-opus-4-8"),
  // Version 4.7 models
  ("opus-4-7", "claude-opus-4-7"),
  // Version 4.6 models
  ("sonnet-4-6", "claude-sonnet-4-6"),
  // Version 4.5 models
  ("opus-4-5", "claude-opus-4-5"),
  ("sonnet-4-5", "claude-sonnet-4-5"),
  ("haiku-4-5", "claude-haiku-4-5"),
  // Version 4.1 models
  ("opus-4-1", "claude-opus-4-1"),
  // Version 4.0 models
  ("opus-4-0", "claude-opus-4-0"),
  ("claude-sonnet-4-0", "claude-sonnet-4-0"),
  ("sonnet-4-0", "claude-sonnet-4-0"),
];

const GROQ_MODEL_MAPPING_SRC: [(&str, &str); 11] = [
  ///// Default models /////
  // GPT OSS
  ("gpt", "openai/gpt-oss-20b"),
  ("gp", "openai/gpt-oss-20b"),
  // Llama
  ("llama", "llama-3.1-8b-instant"),
  ("ll", "llama-3.1-8b-instant"),
  ("llama-instant", "llama-3.1-8b-instant"),
  ///// Specific versions /////
  // GPT OSS
  ("gpt-20b", "openai/gpt-oss-20b"),
  ("gpt-120b", "openai/gpt-oss-120b"),
  // Llama 3.1
  ("llama31", "llama-3.1-8b-instant"),
  ("llama31-8b", "llama-3.1-8b-instant"),
  // Whisper
  ("whisper", "whisper-large-v3"),
  ("whisper-turbo", "whisper-large-v3-turbo"),
];

const CEREBRAS_MODEL_MAPPING_SRC: [(&str, &str); 5] = [
  ///// Default models /////
  // GPT
  ("gpt", "gpt-oss-120b"),
  // Z.ai GLM
  ("glm", "zai-glm-4.7"),
  ("zai", "zai-glm-4.7"),
  ///// Specific versions /////
  // Z.ai GLM 4.7
  ("glm-4.7", "zai-glm-4.7"),
  ("gpt-120b", "gpt-oss-120b"),
];

const DEEPSEEK_MODEL_MAPPING_SRC: [(&str, &str); 13] = [
  // Default models
  ("deepseek", "deepseek-v4-pro"),
  ("pro", "deepseek-v4-pro"),
  ("p", "deepseek-v4-pro"),
  ("flash", "deepseek-v4-flash"),
  ("f", "deepseek-v4-flash"),
  // Version 4 models
  ("v4-flash", "deepseek-v4-flash"),
  ("v4-pro", "deepseek-v4-pro"),
  ("deepseek-v4-flash", "deepseek-v4-flash"),
  ("deepseek-v4-pro", "deepseek-v4-pro"),
  // Legacy aliases (deprecated 2026/07/24, map to v4-flash thinking modes)
  ("chat", "deepseek-chat"),
  ("c", "deepseek-chat"),
  ("reasoner", "deepseek-reasoner"),
  ("r", "deepseek-reasoner"),
];

const OLLAMA_MODEL_MAPPING_SRC: [(&str, &str); 21] = [
  // Default models
  ("llama", "llama3.1"),
  ("ll", "llama3.1"),
  ("l", "llama3.1"),
  ("mixtral", "mixtral"),
  ("mix", "mixtral"),
  ("m", "mixtral"),
  ("mistral", "mistral"),
  ("mis", "mistral"),
  ("gemma", "gemma"),
  ("ge", "gemma"),
  ("g", "gemma"),
  ("codegemma", "codegemma"),
  ("cg", "codegemma"),
  ("c", "codegemma"),
  ("command-r", "command-r"),
  ("cr", "command-r"),
  ("command-r-plus", "command-r-plus"),
  ("crp", "command-r-plus"),
  // Specific versions
  ("llama3", "llama3.1"),
  ("llama3.0", "llama3"),
  ("llama2", "llama2"),
];

const OPENAI_MODEL_MAPPING_SRC: [(&str, &str); 45] = [
  // Default models
  ("gpt", "gpt-5.6-sol"),
  ("mini", "gpt-5.6-terra"),
  ("m", "gpt-5.6-terra"),
  ("nano", "gpt-5.6-luna"),
  ("n", "gpt-5.6-luna"),
  ("image", "gpt-image-2.5-flare"),
  ("tts", "gpt-4o-mini-tts"),
  ("transcribe", "gpt-transcribe"),
  ("diarize", "gpt-4o-transcribe-diarize"),
  // GPT-6 (Astra = flagship)
  ("gpt6", "gpt-6-astra"),
  ("6", "gpt-6-astra"),
  ("astra", "gpt-6-astra"),
  ("gpt6astra", "gpt-6-astra"),
  // GPT-5.1
  ("gpt5.1", "gpt-5.1"),
  ("5.1", "gpt-5.1"),
  // GPT-5.6 (Sol = flagship, Terra = balanced, Luna = fastest)
  ("gpt5.6", "gpt-5.6-sol"),
  ("5.6", "gpt-5.6-sol"),
  ("sol", "gpt-5.6-sol"),
  ("gpt5.6sol", "gpt-5.6-sol"),
  ("terra", "gpt-5.6-terra"),
  ("gpt5.6terra", "gpt-5.6-terra"),
  ("luna", "gpt-5.6-luna"),
  ("gpt5.6luna", "gpt-5.6-luna"),
  // GPT Image (Flare = fast generation, Sunburst = precise editing)
  ("gptimage", "gpt-image-2.5-flare"),
  ("gpt-image", "gpt-image-2.5-flare"),
  ("gpt-image-2.5", "gpt-image-2.5-flare"),
  ("flare", "gpt-image-2.5-flare"),
  ("sunburst", "gpt-image-2.5-sunburst"),
  // GPT-4
  ("gpt4", "gpt-4.1"),
  ("gpt4mini", "gpt-4.1-mini"),
  ("4mini", "gpt-4.1-mini"),
  ("4m", "gpt-4.1-mini"),
  // GPT-4o
  ("gpt4o", "gpt-4o"),
  ("4o", "gpt-4o"),
  ("gpt4ominitts", "gpt-4o-mini-tts"),
  // Transcription
  ("gpt-transcribe", "gpt-transcribe"),
  ("gpttranscribe", "gpt-transcribe"),
  // Realtime sessions only, not the file transcription endpoint
  ("gpt-live-transcribe", "gpt-live-transcribe"),
  ("live-transcribe", "gpt-live-transcribe"),
  ("gpt-4o-transcribe-diarize", "gpt-4o-transcribe-diarize"),
  ("transcribe-diarize", "gpt-4o-transcribe-diarize"),
  ("gpt4otranscribe", "gpt-4o-transcribe"),
  ("gpt-4o-mini-transcribe", "gpt-4o-mini-transcribe"),
  ("whisper", "whisper-1"),
  ("whisper-1", "whisper-1"),
];

const XAI_MODEL_MAPPING_SRC: [(&str, &str); 15] = [
  // Default models
  ("grok", "grok-4.7"),
  ("grok-fast", "grok-4.20-0309-non-reasoning"),
  ("fast", "grok-4.20-0309-non-reasoning"),
  ("grok-mini", "grok-4.3"),
  ("mini", "grok-4.3"),
  ("grok-image", "grok-imagine-image-2.0"),
  ("image", "grok-imagine-image-2.0"),
  // Grok 4.x
  ("grok4.7", "grok-4.7"),
  ("grok4.6", "grok-4.6"),
  ("grok4.5", "grok-4.5"),
  ("grok4.3", "grok-4.3"),
  // Grok Imagine
  ("imagine", "grok-imagine-image-2.0"),
  ("image2", "grok-imagine-image-2.0"),
  ("image1", "grok-imagine-image"),
  ("quality", "grok-imagine-image-quality"),
];

const MISTRAL_MODEL_MAPPING_SRC: [(&str, &str); 17] = [
  // Default models
  ("mistral", "mistral-large-4"),
  ("m", "mistral-large-4"),
  ("large", "mistral-large-4"),
  ("l", "mistral-large-4"),
  ("large4", "mistral-large-4"),
  ("large3", "mistral-large-2512"),
  ("medium", "mistral-medium-latest"),
  ("small", "mistral-small-latest"),
  // Code models
  ("codestral", "codestral-latest"),
  ("code", "codestral-latest"),
  // Ministral
  ("ministral", "ministral-8b-latest"),
  ("ministral-3b", "ministral-3b-latest"),
  ("ministral-8b", "ministral-8b-latest"),
  ("ministral-14b", "ministral-14b-latest"),
  // Specialty
  ("voxtral", "voxtral-small-latest"),
  ("leanstral", "labs-leanstral-1-5"),
  // Hosted third-party models
  ("glm", "zai-glm-latest"),
];

const PERPLEXITY_MODEL_MAPPING_SRC: [(&str, &str); 8] = [
  // Supported models
  ("sonar", "sonar"),
  ("s", "sonar"),
  ("sonar-pro", "sonar-pro"),
  ("sp", "sonar-pro"),
  ("sonar-reasoning-pro", "sonar-reasoning-pro"),
  ("srp", "sonar-reasoning-pro"),
  ("sonar-deep-research", "sonar-deep-research"),
  ("sdr", "sonar-deep-research"),
];

fn pretty_print_mapping(mapping: &[(&str, &str)]) -> String {
  mapping
    .iter()
    .map(|(alias, model)| format!("  {: <9} → {model}\n", *alias))
    .collect::<String>()
}

fn main() {
  let models_rs_content = include_str!("src_templates/models.rs");

  let out_dir = env::var("OUT_DIR").unwrap();
  let dest_path = Path::new(&out_dir).join("models.rs");

  // Write the hashmap and its pretty representation to the file
  let code = models_rs_content
    .replace(
      "// {anthropic_model_hashmap}",
      &ANTHROPIC_MODEL_MAPPING_SRC
        .iter()
        .map(|(model, constant)| format!("(\"{model}\", \"{constant}\"),\n"))
        .collect::<String>(),
    )
    .replace(
      "{anthropic_models_pretty}",
      &pretty_print_mapping(&ANTHROPIC_MODEL_MAPPING_SRC),
    )
    .replace(
      "// {cerebras_model_hashmap}",
      &CEREBRAS_MODEL_MAPPING_SRC
        .iter()
        .map(|(model, constant)| format!("(\"{model}\", \"{constant}\"),\n"))
        .collect::<String>(),
    )
    .replace(
      "{cerebras_models_pretty}",
      &pretty_print_mapping(&CEREBRAS_MODEL_MAPPING_SRC),
    )
    .replace(
      "// {deepseek_model_hashmap}",
      &DEEPSEEK_MODEL_MAPPING_SRC
        .iter()
        .map(|(model, constant)| format!("(\"{model}\", \"{constant}\"),\n"))
        .collect::<String>(),
    )
    .replace(
      "{deepseek_models_pretty}",
      &pretty_print_mapping(&DEEPSEEK_MODEL_MAPPING_SRC),
    )
    .replace(
      "// {google_model_hashmap}",
      &GOOGLE_MODEL_MAPPING_SRC
        .iter()
        .map(|(model, constant)| format!("(\"{model}\", \"{constant}\"),\n"))
        .collect::<String>(),
    )
    .replace(
      "{google_models_pretty}",
      &pretty_print_mapping(&GOOGLE_MODEL_MAPPING_SRC),
    )
    .replace(
      "// {groq_model_hashmap}",
      &GROQ_MODEL_MAPPING_SRC
        .iter()
        .map(|(model, constant)| format!("(\"{model}\", \"{constant}\"),\n"))
        .collect::<String>(),
    )
    .replace(
      "{groq_models_pretty}",
      &pretty_print_mapping(&GROQ_MODEL_MAPPING_SRC),
    )
    .replace(
      "// {ollama_model_hashmap}",
      &OLLAMA_MODEL_MAPPING_SRC
        .iter()
        .map(|(model, constant)| format!("(\"{model}\", \"{constant}\"),\n"))
        .collect::<String>(),
    )
    .replace(
      "{ollama_models_pretty}",
      &pretty_print_mapping(&OLLAMA_MODEL_MAPPING_SRC),
    )
    .replace(
      "// {openai_model_hashmap}",
      &OPENAI_MODEL_MAPPING_SRC
        .iter()
        .map(|(model, constant)| format!("(\"{model}\", \"{constant}\"),\n"))
        .collect::<String>(),
    )
    .replace(
      "{openai_models_pretty}",
      &pretty_print_mapping(&OPENAI_MODEL_MAPPING_SRC),
    )
    .replace(
      "// {xai_model_hashmap}",
      &XAI_MODEL_MAPPING_SRC
        .iter()
        .map(|(model, constant)| format!("(\"{model}\", \"{constant}\"),\n"))
        .collect::<String>(),
    )
    .replace(
      "{xai_models_pretty}",
      &pretty_print_mapping(&XAI_MODEL_MAPPING_SRC),
    )
    .replace(
      "// {perplexity_model_hashmap}",
      &PERPLEXITY_MODEL_MAPPING_SRC
        .iter()
        .map(|(model, constant)| format!("(\"{model}\", \"{constant}\"),\n"))
        .collect::<String>(),
    )
    .replace(
      "{perplexity_models_pretty}",
      &pretty_print_mapping(&PERPLEXITY_MODEL_MAPPING_SRC),
    )
    .replace(
      "// {mistral_model_hashmap}",
      &MISTRAL_MODEL_MAPPING_SRC
        .iter()
        .map(|(model, constant)| format!("(\"{model}\", \"{constant}\"),\n"))
        .collect::<String>(),
    )
    .replace(
      "{mistral_models_pretty}",
      &pretty_print_mapping(&MISTRAL_MODEL_MAPPING_SRC),
    );

  fs::write(&dest_path, code).unwrap();
  println!("cargo:rerun-if-changed=build.rs");
  println!("cargo:rerun-if-changed=src_templates/models.rs");
}
