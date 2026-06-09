use std::env;
use std::fs;
use std::path::Path;

const GOOGLE_MODEL_MAPPING_SRC: [(&str, &str); 18] = [
  // Default models
  ("gemini-flash", "gemini-2.5-flash"),
  ("gemini", "gemini-2.5-flash"),
  ("g", "gemini-2.5-flash"),
  ("flash", "gemini-2.5-flash"),
  ("f", "gemini-2.5-flash"),
  ("gemini-pro", "gemini-2.5-pro"),
  ("pro", "gemini-2.5-pro"),
  ("gemini-flash-lite", "gemini-2.0-flash-lite"),
  ("flash-lite", "gemini-2.0-flash-lite"),
  ("lite", "gemini-2.0-flash-lite"),
  // Image generation model
  ("gemini-image", "gemini-2.5-flash-image"),
  ("image", "gemini-2.5-flash-image"),
  ("img", "gemini-2.5-flash-image"),
  // Version 3 models
  ("gemini-3-pro", "gemini-3-pro-preview"),
  ("gemini-3-pro-image", "gemini-3-pro-image-preview"),
  // Version 2.5 models
  ("gemini-2.5-flash", "gemini-2.5-flash"),
  ("gemini-2.5-pro", "gemini-2.5-pro"),
  // Version 2 models
  ("gemini-2-flash", "gemini-2.0-flash"),
];

const ANTHROPIC_MODEL_MAPPING_SRC: [(&str, &str); 26] = [
  // Default models
  // Fable (most powerful)
  ("claude-fable", "claude-fable-5"),
  ("fable", "claude-fable-5"),
  ("fa", "claude-fable-5"),
  // Opus
  ("claude-opus", "claude-opus-4-8"),
  ("opus", "claude-opus-4-8"),
  ("op", "claude-opus-4-8"),
  ("o", "claude-opus-4-8"),
  // Sonnet
  ("claude-sonnet", "claude-sonnet-4-6"),
  ("sonnet", "claude-sonnet-4-6"),
  ("so", "claude-sonnet-4-6"),
  ("s", "claude-sonnet-4-6"),
  // Haiku
  ("claude-haiku", "claude-haiku-4-5"),
  ("haiku", "claude-haiku-4-5"),
  ("ha", "claude-haiku-4-5"),
  ("h", "claude-haiku-4-5"),
  // Version 5 models
  ("fable-5", "claude-fable-5"),
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

const GROQ_MODEL_MAPPING_SRC: [(&str, &str); 14] = [
  ///// Default models /////
  // GPT OSS
  ("gpt", "openai/gpt-oss-20b"),
  ("gp", "openai/gpt-oss-20b"),
  // Llama
  ("llama", "llama-3.1-8b-instant"),
  ("ll", "llama-3.1-8b-instant"),
  ("llama-instant", "llama-3.1-8b-instant"),
  ("llama-versatile", "llama-3.3-70b-versatile"),
  ///// Specific versions /////
  // GPT OSS
  ("gpt-20b", "openai/gpt-oss-20b"),
  ("gpt-120b", "openai/gpt-oss-120b"),
  // Llama 3.1
  ("llama31", "llama-3.1-8b-instant"),
  ("llama31-8b", "llama-3.1-8b-instant"),
  // Llama 3.3
  ("llama33-70b", "llama-3.3-70b-versatile"),
  // Whisper
  ("whisper", "whisper-large-v3"),
  ("whisper-turbo", "whisper-large-v3-turbo"),
  // Qwen
  ("qwen", "qwen/qwen3-32b"),
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

const OPENAI_MODEL_MAPPING_SRC: [(&str, &str); 33] = [
  // Default models
  ("gpt", "gpt-5"),
  ("mini", "gpt-5-mini"),
  ("m", "gpt-5-mini"),
  ("nano", "gpt-5-nano"),
  ("n", "gpt-5-nano"),
  ("image", "gpt-image-2"),
  ("tts", "gpt-4o-mini-tts"),
  ("transcribe", "gpt-4o-transcribe"),
  // GPT-5
  ("gpt5", "gpt-5"),
  ("gpt5mini", "gpt-5-mini"),
  ("gpt5nano", "gpt-5-nano"),
  ("5", "gpt-5"),
  ("5mini", "gpt-5-mini"),
  ("5m", "gpt-5-mini"),
  ("5nano", "gpt-5-nano"),
  ("5n", "gpt-5-nano"),
  // GPT-5.1
  ("gpt5.1", "gpt-5.1"),
  ("5.1", "gpt-5.1"),
  // GPT Image
  ("gptimage", "gpt-image-2"),
  ("gpt-image", "gpt-image-2"),
  ("gpt-image-2", "gpt-image-2"),
  ("gpt-image-1.5", "gpt-image-1.5"),
  ("gpt-image-1-mini", "gpt-image-1-mini"),
  // GPT-4
  ("gpt4", "gpt-4.1"),
  ("gpt4mini", "gpt-4.1-mini"),
  ("4mini", "gpt-4.1-mini"),
  ("4m", "gpt-4.1-mini"),
  // GPT-4o
  ("gpt4o", "gpt-4o"),
  ("4o", "gpt-4o"),
  ("gpt4ominitts", "gpt-4o-mini-tts"),
  ("gpt4otranscribe", "gpt-4o-transcribe"),
  // o4
  ("o4m", "o4-mini"),
  // o3
  ("o3pro", "o3-pro"),
];

const XAI_MODEL_MAPPING_SRC: [(&str, &str); 10] = [
  // Default models
  ("grok", "grok-4"),
  ("grok-fast", "grok-4-fast"),
  ("fast", "grok-4-fast"),
  ("grok-mini", "grok-3-mini"),
  ("mini", "grok-3-mini"),
  ("grok-image", "grok-imagine-image"),
  ("image", "grok-imagine-image"),
  // Grok 4
  ("grok4", "grok-4"),
  ("grok4fast", "grok-4-fast"),
  // Grok 3
  ("grok3mini", "grok-3-mini"),
];

const MISTRAL_MODEL_MAPPING_SRC: [(&str, &str); 20] = [
  // Default models
  ("mistral", "mistral-large-latest"),
  ("m", "mistral-large-latest"),
  ("large", "mistral-large-latest"),
  ("l", "mistral-large-latest"),
  ("medium", "mistral-medium-latest"),
  ("small", "mistral-small-latest"),
  ("tiny", "mistral-tiny-latest"),
  // Code models
  ("codestral", "codestral-latest"),
  ("code", "codestral-latest"),
  ("devstral", "devstral-latest"),
  // Ministral
  ("ministral", "ministral-8b-latest"),
  ("ministral-3b", "ministral-3b-latest"),
  ("ministral-8b", "ministral-8b-latest"),
  ("ministral-14b", "ministral-14b-latest"),
  // Reasoning
  ("magistral", "magistral-medium-latest"),
  // Specialty
  ("embed", "mistral-embed"),
  ("ocr", "mistral-ocr-latest"),
  ("pixtral", "pixtral-large-latest"),
  ("voxtral", "voxtral-small-latest"),
  ("nemo", "open-mistral-nemo"),
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
