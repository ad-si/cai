use std::env;
use std::fs;
use std::path::Path;

const GOOGLE_MODEL_MAPPING_SRC: [(&str, &str); 14] = [
  // Default models
  ("gemini", "gemini-2.5-flash"),
  ("g", "gemini-2.5-flash"),
  ("flash", "gemini-2.5-flash"),
  ("f", "gemini-2.5-flash"),
  ("gemini-pro", "gemini-2.5-pro"),
  ("pro", "gemini-2.5-pro"),
  ("gemini-flash-lite", "gemini-2.0-flash-lite"),
  ("flast-lite", "gemini-2.0-flash-lite"),
  ("lite", "gemini-2.0-flash-lite"),
  // Version 2.5 models
  ("gemini-2.5-flash", "gemini-2.5-flash"),
  ("gemini-2.5-pro", "gemini-2.5-pro"),
  // Version 2 models
  ("gemini-2-flash", "gemini-2.0-flash"),
  // Version 1.5 models
  ("gemini-1.5-flash", "gemini-1.5-flash"),
  ("gemini-1.5-pro", "gemini-1.5-pro"),
];

const ANTHROPIC_MODEL_MAPPING_SRC: [(&str, &str); 34] = [
  // Default models
  // Opus
  ("claude-opus", "claude-opus-4-0"),
  ("opus", "claude-opus-4-0"),
  ("op", "claude-opus-4-0"),
  ("o", "claude-opus-4-0"),
  // Sonnet
  ("claude-sonnet", "claude-sonnet-4-0"),
  ("sonnet", "claude-sonnet-4-0"),
  ("so", "claude-sonnet-4-0"),
  ("s", "claude-sonnet-4-0"),
  // Haiku
  ("claude-haiku", "claude-3-5-haiku-latest"),
  ("haiku", "claude-3-5-haiku-latest"),
  ("ha", "claude-3-5-haiku-latest"),
  ("h", "claude-3-5-haiku-latest"),
  // Version 4.1 models
  ("claude-opus-4-1", "claude-opus-4-1"),
  ("opus-4-1", "claude-opus-4-1"),
  // Version 4.0 models
  ("claude-opus-4-0", "claude-opus-4-0"),
  ("opus-4-0", "claude-opus-4-0"),
  ("claude-sonnet-4-0", "claude-sonnet-4-0"),
  ("sonnet-4-0", "claude-sonnet-4-0"),
  // Version 3.7 models
  ("claude-opus-3-7", "claude-3-opus-latest"),
  ("opus-3-7", "claude-3-opus-latest"),
  ("claude-sonnet-3-7", "claude-3-7-sonnet-latest"),
  ("sonnet-3-7", "claude-3-7-sonnet-latest"),
  // Version 3.5 models
  ("claude-sonnet-3-5", "claude-3-5-sonnet-latest"),
  ("sonnet-3-5", "claude-3-5-sonnet-latest"),
  ("claude-haiku-3-5", "claude-3-5-haiku-latest"),
  ("haiku-3-5", "claude-3-5-haiku-latest"),
  // Version 3 models
  ("claude-opus-3", "claude-3-opus-latest"),
  ("opus-3", "claude-3-opus-latest"),
  ("claude-sonnet-3", "claude-3-sonnet-20240229"),
  ("sonnet-3", "claude-3-sonnet-20240229"),
  ("claude-haiku-3", "claude-3-haiku-20240307"),
  ("haiku-3", "claude-3-haiku-20240307"),
  ("claude-sonnet-3-7", "claude-3-7-sonnet-latest"),
  ("sonnet-3-7", "claude-3-7-sonnet-latest"),
];

const GROQ_MODEL_MAPPING_SRC: [(&str, &str); 20] = [
  ///// Default models /////
  // GPT OSS
  ("gpt", "openai/gpt-oss-20b"),
  ("gp", "openai/gpt-oss-20b"),
  // Llama
  ("llama", "llama-3.1-8b-instant"),
  ("ll", "llama-3.1-8b-instant"),
  ("llama-instant", "llama-3.1-8b-instant"),
  ("llama-versatile", "llama-3.1-70b-versatile"),
  ("llama-reasoning", "llama-3.1-405b-reasoning"),
  ///// Specific versions /////
  // GPT OSS
  ("gpt-20b", "openai/gpt-oss-20b"),
  ("gpt-120b", "openai/gpt-oss-120b"),
  // Llama 3.1
  ("llama31", "llama-3.1-8b-instant"),
  ("llama31-8b", "llama-3.1-8b-instant"),
  ("llama31-70b", "llama-3.1-70b-versatile"),
  ("llama31-405b", "llama-3.1-405b-reasoning"),
  // Llama 3.0
  ("llama3", "llama3-8b-8192"),
  ("llama3-8b", "llama3-8b-8192"),
  ("llama3-70b", "llama3-70b-8192"),
  // Whisper
  ("whisper", "whisper-large-v3"),
  ("whisper-turbo", "whisper-large-v3-turbo"),
  // Qwen
  ("qwen", "qwen3-32b"),
  // DeepSeek
  ("deepseek", "deepseek-r1-distill-llama-70b"),
];

const CEREBRAS_MODEL_MAPPING_SRC: [(&str, &str); 14] = [
  ///// Default models /////
  // GPT
  ("gpt", "gpt-oss-120b"),
  // Llama
  ("llama", "llama3.1-8b"),
  ("ll", "llama3.1-8b"),
  ("l", "llama3.1-8b"),
  ("llama-8b", "llama3.1-8b"),
  ("llama-70b", "llama-3.3-70b"),
  // Deepseek
  ("deepseek", "deepseek-r1-distill-llama-70b"),
  ("deep", "deepseek-r1-distill-llama-70b"),
  ("d", "deepseek-r1-distill-llama-70b"),
  ///// Specific versions /////
  // Llama 3.1
  ("llama31", "llama-3.1-8b"),
  ("llama31-8b", "llama-3.1-8b"),
  // Llama 3.3
  ("llama33", "llama-3.3-70b"),
  ("llama33-70b", "llama-3.3-70b"),
  // Deepseek R1
  ("deepseek-r1", "deepseek-r1-distill-llama-70b"),
];

const DEEPSEEK_MODEL_MAPPING_SRC: [(&str, &str); 2] = [
  ("chat", "deepseek-chat"),
  ("reasoner", "deepseek-reasoner"), //
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

const OPENAI_MODEL_MAPPING_SRC: [(&str, &str); 37] = [
  // Default models
  ("gpt", "gpt-5"),
  ("mini", "gpt-5-mini"),
  ("m", "gpt-5-mini"),
  ("nano", "gpt-5-nano"),
  ("n", "gpt-5-nano"),
  ("image", "gpt-5"),
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
  // GPT Image
  ("gptimage", "gpt-image-1"),
  ("gpt-image", "gpt-image-1"),
  ("gpt-image-1", "gpt-image-1"),
  // DALL-E
  ("dalle", "dall-e-3"),
  ("dalle3", "dall-e-3"),
  ("dalle2", "dall-e-2"),
  // GPT-4
  ("gpt4", "gpt-4.1"),
  ("gpt4mini", "gpt-4.1-mini"),
  ("4mini", "gpt-4.1-mini"),
  ("4m", "gpt-4.1-mini"),
  ("gpt4nano", "gpt-4.1-nano"),
  ("4nano", "gpt-4.1-nano"),
  ("4n", "gpt-4.1-nano"),
  // GPT-4o
  ("gpt4o", "gpt-4o"),
  ("4o", "gpt-4o"),
  ("gpt4ominitts", "gpt-4o-mini-tts"),
  ("gpt4otranscribe", "gpt-4o-transcribe"),
  // o4
  ("o4m", "o4-mini"),
  ("o4mdr", "o4-mini-deep-research"),
  // o3
  ("o3pro", "o3-pro"),
  ("o3dr", "o3-deep-research"),
];

const XAI_MODEL_MAPPING_SRC: [(&str, &str); 6] = [
  // Default models
  ("grok", "grok-4-latest"),
  ("grok-mini", "grok-3-mini-latest"),
  ("grok-image", "grok-2-image-latest"),
  // Grok 4
  ("grok4", "grok-4-latest"),
  // Grok 3
  ("grok3", "grok-3-latest"),
  ("grok3mini", "grok-3-mini-latest"),
];

const PERPLEXITY_MODEL_MAPPING_SRC: [(&str, &str); 19] = [
  // Supported models
  ("sonar", "sonar"),
  ("s", "sonar"),
  ("sonar-pro", "sonar-pro"),
  ("sp", "sonar-pro"),
  ("sonar-reasoning", "sonar-reasoning"),
  ("sr", "sonar-reasoning"),
  ("sonar-reasoning-pro", "sonar-reasoning-pro"),
  ("srp", "sonar-reasoning-pro"),
  ("sonar-deep-research", "sonar-deep-research"),
  ("sdr", "sonar-deep-research"),
  ("r1-1776", "r1-1776"),
  ("r", "r1-1776"),
  ("offline", "r1-1776"),
  ("llama-small", "llama-3.1-sonar-small-128k-online"),
  ("ls", "llama-3.1-sonar-small-128k-online"),
  ("llama-large", "llama-3.1-sonar-large-128k-online"),
  ("ll", "llama-3.1-sonar-large-128k-online"),
  ("llama-huge", "llama-3.1-sonar-huge-128k-online"),
  ("lh", "llama-3.1-sonar-huge-128k-online"),
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
      "{google_models_pretty}",
      &pretty_print_mapping(&GOOGLE_MODEL_MAPPING_SRC),
    )
    .replace(
      "// {google_model_hashmap}",
      &GOOGLE_MODEL_MAPPING_SRC
        .iter()
        .map(|(model, constant)| format!("(\"{model}\", \"{constant}\"),\n"))
        .collect::<String>(),
    );

  fs::write(&dest_path, code).unwrap();
  println!("cargo:rerun-if-changed=build.rs");
  println!("cargo:rerun-if-changed=src_templates/models.rs");
}
