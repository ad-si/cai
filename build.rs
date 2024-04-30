use std::env;
use std::fs;
use std::path::Path;

const CONST_ASSIGNMENTS: [(&str, &str); 9] = [
  // GROQ
  ("GROQ_LLAMA", "llama3-8b-8192"),
  ("GROQ_LLAMA_70", "llama3-70b-8192"),
  ("GROQ_MIXTRAL", "mixtral-8x7b-32768"),
  ("GROQ_GEMMA", "gemma-7b-it"),
  // OPENAI
  ("OPENAI_GPT", "gpt-4"),
  ("OPENAI_GPT_TURBO", "gpt-4-turbo"),
  // ANTHROPIC
  ("CLAUDE_OPUS", "claude-3-opus-20240229"),
  ("CLAUDE_SONNET", "claude-3-sonnet-20240229"),
  ("CLAUDE_HAIKU", "claude-3-haiku-20240307"),
];

const GROQ_MODEL_MAPPING_SRC: [(&str, &str); 12] = [
  ("llama3", "GROQ_LLAMA"),
  ("llama", "GROQ_LLAMA"),
  ("ll", "GROQ_LLAMA"),
  ("llama3-70", "GROQ_LLAMA_70"),
  ("llama-70", "GROQ_LLAMA_70"),
  ("ll-70", "GROQ_LLAMA_70"),
  ("ll70", "GROQ_LLAMA_70"),
  ("mixtral", "GROQ_MIXTRAL"),
  ("mix", "GROQ_MIXTRAL"),
  ("mi", "GROQ_MIXTRAL"),
  ("gemma", "GROQ_GEMMA"),
  ("ge", "GROQ_GEMMA"),
];

const OLLAMA_MODEL_MAPPING_SRC: [(&str, &str); 11] = [
  ("llama", "llama3"),
  ("ll", "llama3"),
  ("llama2", "llama2"),
  ("ll2", "llama2"),
  ("mix", "mixtral"),
  ("mi", "mixtral"),
  ("mis", "mistral"),
  ("ge", "gemma"),
  ("cg", "codegemma"),
  ("cr", "command-r"),
  ("crp", "command-r-plus"),
];

fn pretty_print_mapping(use_lookup: bool, mapping: &[(&str, &str)]) -> String {
  mapping
    .iter()
    .map(|(alias, model)| {
      let full_name = if use_lookup {
        CONST_ASSIGNMENTS
          .iter()
          .find(|(constant_name, _)| constant_name == model)
          .unwrap()
          .1
      } else {
        model
      };
      format!("  {: <9} → {full_name}\n", *alias)
    })
    .collect::<String>()
}

fn main() {
  let models_rs_content = include_str!("src_templates/models.rs");

  let out_dir = env::var("OUT_DIR").unwrap();
  let dest_path = Path::new(&out_dir).join("models.rs");

  // Write the hashmap and its pretty representation to the file
  let code = models_rs_content
    .replace(
      "// {const_assignments}",
      &CONST_ASSIGNMENTS
        .iter()
        .map(|(constant, value)| {
          format!("pub const {constant}: &str = \"{value}\";\n")
        })
        .collect::<String>(),
    )
    .replace(
      "// {groq_model_hashmap}",
      &GROQ_MODEL_MAPPING_SRC
        .iter()
        .map(|(model, constant)| {
          let full_name = CONST_ASSIGNMENTS
            .iter()
            .find(|(constant_name, _)| constant_name == constant)
            .unwrap()
            .1;
          format!("(\"{model}\", \"{full_name}\"),\n")
        })
        .collect::<String>(),
    )
    .replace(
      "{groq_models_pretty}",
      &pretty_print_mapping(true, &GROQ_MODEL_MAPPING_SRC),
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
      &pretty_print_mapping(false, &OLLAMA_MODEL_MAPPING_SRC),
    );

  fs::write(&dest_path, code).unwrap();
  println!("cargo:rerun-if-changed=build.rs");
}
