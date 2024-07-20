use std::collections::HashMap;

//////////////////////////////////////////////////
////////////////////// GROQ //////////////////////

// {const_assignments}

// Static mapping accessible from other files
pub const GROQ_MODEL_MAPPING: &[(&str, &str)] = &[
  // This will be replaced by build.rs:
  // {groq_model_hashmap}
];

fn get_groq_model(model_id: &str) -> &str {
  GROQ_MODEL_MAPPING
    .iter()
    .find(|(key, _)| key == &model_id)
    .map_or(model_id, |(_, value)| *value)
}

pub const GROQ_MODELS_PRETTY: &str =
  // This will be replaced by build.rs
  "{groq_models_pretty}";

#[macro_export]
macro_rules! groq_models_pretty {
  ($prefix: expr) => {
    // This will be replaced by build.rs
    concat!($prefix, "\n", "{groq_models_pretty}")
  };
}

//////////////////////////////////////////////////
///////////////////// OLLAMA /////////////////////

// Pretty-printed string representation of the hashmap
pub const OLLAMA_MODEL_MAPPING: &[(&str, &str)] = &[
  // This will be replaced by build.rs:
  // {ollama_model_hashmap}
];

fn get_ollama_model(model_id: &str) -> &str {
  OLLAMA_MODEL_MAPPING
    .iter()
    .find(|(key, _)| key == &model_id)
    .map_or(model_id, |(_, value)| *value)
}

pub const OLLAMA_MODELS_PRETTY: &str =
  // This will be replaced by build.rs:
  "{ollama_models_pretty}";

#[macro_export]
macro_rules! ollama_models_pretty {
  ($prefix: expr) => {
    // This will be replaced by build.rs
    concat!($prefix, "\n", "{ollama_models_pretty}")
  };
}

//////////////////////////////////////////////////
///////////////////// OPENAI /////////////////////

// Pretty-printed string representation of the hashmap
pub const OPENAI_MODEL_MAPPING: &[(&str, &str)] = &[
  // This will be replaced by build.rs:
  // {openai_model_hashmap}
];

fn get_openai_model(model_id: &str) -> &str {
  OPENAI_MODEL_MAPPING
    .iter()
    .find(|(key, _)| key == &model_id)
    .map_or(model_id, |(_, value)| *value)
}

pub const OPENAI_MODELS_PRETTY: &str =
  // This will be replaced by build.rs:
  "{openai_models_pretty}";

#[macro_export]
macro_rules! openai_models_pretty {
  ($prefix: expr) => {
    // This will be replaced by build.rs
    concat!($prefix, "\n", "{openai_models_pretty}")
  };
}
