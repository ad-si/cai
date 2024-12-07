use std::env;
use std::fs;
use std::path::Path;

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
  ("gemma", "gemma-7b-it"),
  ("ge", "gemma-7b-it"),
  ("g", "gemma-7b-it"),
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

const OLLAMA_MODEL_MAPPING_SRC: [(&str, &str); 20] = [
  // Default models
  ("llama", "llama3"),
  ("ll", "llama3"),
  ("l", "llama3"),
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
  ("llama3", "llama3"),
  ("llama2", "llama2"),
];

const OPENAI_MODEL_MAPPING_SRC: [(&str, &str); 13] = [
  // Default models
  ("gpt", "gpt-4o"),
  ("omni", "gpt-4o"),
  ("mini", "gpt-4o-mini"),
  ("m", "gpt-4o-mini"),
  ("turbo", "gpt-4-turbo"),
  ("t", "gpt-4-turbo"),
  // Specific versions
  ("4o", "gpt-4o"),
  ("gpt4", "gpt-4"),
  ("4", "gpt-4"),
  ("turbo4", "gpt-4-turbo"),
  ("t4", "gpt-4-turbo"),
  ("turbo35", "gpt-3.5-turbo"),
  ("t35", "gpt-3.5-turbo"),
];

const ANTHROPIC_MODEL_MAPPING_SRC: [(&str, &str); 22] = [
  // Default models
  ("claude-opus", "claude-3-opus-latest"),
  ("opus", "claude-3-opus-latest"),
  ("op", "claude-3-opus-latest"),
  ("o", "claude-3-opus-latest"),
  ("claude-sonnet", "claude-3-5-sonnet-latest"),
  ("sonnet", "claude-3-5-sonnet-latest"),
  ("so", "claude-3-5-sonnet-latest"),
  ("s", "claude-3-5-sonnet-latest"),
  ("claude-haiku", "claude-3-5-haiku-latest"),
  ("haiku", "claude-3-5-haiku-latest"),
  ("ha", "claude-3-5-haiku-latest"),
  ("h", "claude-3-5-haiku-latest"),
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
      "// {anthropic_model_hashmap}",
      &ANTHROPIC_MODEL_MAPPING_SRC
        .iter()
        .map(|(model, constant)| format!("(\"{model}\", \"{constant}\"),\n"))
        .collect::<String>(),
    )
    .replace(
      "{anthropic_models_pretty}",
      &pretty_print_mapping(&ANTHROPIC_MODEL_MAPPING_SRC),
    );

  fs::write(&dest_path, code).unwrap();
  println!("cargo:rerun-if-changed=build.rs");
}
