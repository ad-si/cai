use std::env;
use std::fs;
use std::path::Path;

const GOOGLE_MODEL_MAPPING_SRC: [(&str, &str); 14] = [
  // Default models
  ("gemini", "gemini-2.5-flash-preview-04-17"),
  ("g", "gemini-2.5-flash-preview-04-17"),
  ("flash", "gemini-2.5-flash-preview-04-17"),
  ("f", "gemini-2.5-flash-preview-04-17"),
  ("gemini-pro", "gemini-2.5-pro-preview-05-06"),
  ("pro", "gemini-2.5-pro-preview-05-06"),
  ("gemini-flash-lite", "gemini-2.0-flash-lite"),
  ("flast-lite", "gemini-2.0-flash-lite"),
  ("lite", "gemini-2.0-flash-lite"),
  // Version 2.5 models
  ("gemini-2.5-flash", "gemini-2.5-flash-preview-04-17"),
  ("gemini-2.5-pro", "gemini-2.5-pro-preview-05-06"),
  // Version 2 models
  ("gemini-2-flash", "gemini-2.0-flash"),
  // Version 1.5 models
  ("gemini-1.5-flash", "gemini-1.5-flash"),
  ("gemini-1.5-pro", "gemini-1.5-pro"),
];

const ANTHROPIC_MODEL_MAPPING_SRC: [(&str, &str); 32] = [
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
  ("claude-sonnet-3-7", "claude-3-7-sonnet-20250219"),
  ("sonnet-3-7", "claude-3-7-sonnet-20250219"),
];

const GROQ_MODEL_MAPPING_SRC: [(&str, &str); 20] = [
  ///// Default models /////
  // Llama
  ("llama", "llama-3.1-8b-instant"),
  ("ll", "llama-3.1-8b-instant"),
  ("l", "llama-3.1-8b-instant"),
  ("llama-8b", "llama-3.1-8b-instant"),
  ("llama-70b", "llama-3.1-70b-versatile"),
  ("llama-405b", "llama-3.1-405b-reasoning"),
  // Mixtral
  ("mixtral", "mixtral-8x7b-32768"),
  ("mi", "mixtral-8x7b-32768"),
  ("m", "mixtral-8x7b-32768"),
  // Gemma
  ("gemma", "gemma2-9b-it"),
  ("ge", "gemma2-9b-it"),
  ("g", "gemma2-9b-it"),
  ///// Specific versions /////
  // Llama 3.1
  ("llama31", "llama-3.1-8b-instant"),
  ("llama31-8b", "llama-3.1-8b-instant"),
  ("llama31-70b", "llama-3.1-70b-versatile"),
  ("llama31-405b", "llama-3.1-405b-reasoning"),
  // Llama 3.0
  ("llama3", "llama3-8b-8192"),
  ("llama3-8b", "llama3-8b-8192"),
  ("llama3-70b", "llama3-70b-8192"),
  // Mixtral
  ("mixtral-8x7b", "mixtral-8x7b-32768"),
];

const CEREBRAS_MODEL_MAPPING_SRC: [(&str, &str); 13] = [
  ///// Default models /////
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

const OPENAI_MODEL_MAPPING_SRC: [(&str, &str); 34] = [
  // Default models
  ("gpt", "gpt-5"),
  ("mini", "gpt-5-mini"),
  ("m", "gpt-5-mini"),
  ("nano", "gpt-5-nano"),
  ("n", "gpt-5-nano"),
  ("image", "gpt-image-1"),
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

const XAI_MODEL_MAPPING_SRC: [(&str, &str); 8] = [
  // Default models
  ("grok", "grok-2-latest"),
  ("grok-mini", "grok-3-mini-latest"),
  ("grok-vision", "grok-2-vision-latest"),
  // Specific versions
  ("grok-2", "grok-2-1212"),
  ("grok-2-vision", "grok-2-vision-1212"),
  ("grok-3", "grok-3-latest"),
  ("grok-3-mini", "grok-3-mini-latest"),
  ("grok-3-vision", "grok-3-vision-latest"),
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
      "{google_models_pretty}",
      &pretty_print_mapping(&GOOGLE_MODEL_MAPPING_SRC),
    );

  fs::write(&dest_path, code).unwrap();
  println!("cargo:rerun-if-changed=build.rs");
  println!("cargo:rerun-if-changed=src_templates/models.rs");
}
