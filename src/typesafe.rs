//! Support for TypeSafe's System One models (Jev).
//!
//! Unlike chat models, System One models don't generate text.
//! They evaluate a `state` against typed questions and return
//! calibrated probabilities (https://docs.typesafe.ai/api).

use std::error::Error;
use std::time::Instant;

use clap::{error::ErrorKind, Arg, ArgAction, ArgMatches};
use color_print::{cformat, cprintln};
use serde_derive::Serialize;
use serde_json::{json, Map, Value};

use crate::{
  format_elapsed_time, get_base_url, get_full_config, get_secrets_path_str,
  ExecOptions,
};

pub const DEFAULT_JEV_MODEL: &str = "jev-latest";

/// Key under which a single question is sent and its answer returned
const QUESTION_ID: &str = "answer";

/// The three question types supported by System One models
#[derive(Debug, PartialEq, Clone, Serialize)]
pub enum JevQuestion {
  /// Yes/no question, answered with the probability of "yes"
  Noul,
  /// Pick one of the options, given as `name` or `name=description`
  Choice(Vec<String>),
  /// Rate along the ordered levels
  Score(Vec<String>),
}

impl JevQuestion {
  fn to_json(&self, instructions: &str) -> Value {
    match self {
      JevQuestion::Noul => json!({
        "type": "noul",
        "instructions": instructions,
      }),
      JevQuestion::Choice(options) => {
        let criteria = options
          .iter()
          .map(|option| match option.split_once('=') {
            Some((name, description)) => (
              name.trim().to_string(),
              Value::String(description.trim().to_string()),
            ),
            None => (option.trim().to_string(), Value::Null),
          })
          .collect::<Map<String, Value>>();
        json!({
          "type": "choice",
          "instructions": instructions,
          "criteria": criteria,
        })
      }
      JevQuestion::Score(levels) => json!({
        "type": "score",
        "instructions": instructions,
        "criteria": levels,
      }),
    }
  }
}

/// A question with the id its answer is returned under
#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct NamedJevQuestion {
  pub name: String,
  pub instructions: String,
  pub question: JevQuestion,
}

/// Several questions passed as order-dependent CLI flags, e.g.
/// `--noul a='Is it urgent?' --choice b='Which team?' -o billing -o sales`.
/// Each `-o/--option` and `-l/--level` belongs to the preceding
/// `--choice` or `--score` question.
#[derive(Debug, PartialEq, Clone, Default, Serialize)]
pub struct JevQuestions(pub Vec<NamedJevQuestion>);

/// Split `name=question` into its parts.
/// Values whose part before the first `=` isn't an identifier
/// (e.g. `Is 1+1=2?`) are used as question and get an auto-generated name.
fn parse_named_question(value: &str, position: usize) -> (String, String) {
  match value.split_once('=') {
    Some((name, question))
      if !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') =>
    {
      (name.to_string(), question.trim().to_string())
    }
    _ => (format!("q{position}"), value.trim().to_string()),
  }
}

impl JevQuestions {
  const ARG_IDS: [&'static str; 5] =
    ["noul", "choice", "option", "score", "level"];

  /// Build the questions from `(arg id, value)` pairs in command line order
  fn from_ordered_args(args: &[(&str, String)]) -> Result<Self, String> {
    let mut questions: Vec<NamedJevQuestion> = Vec::new();

    for (arg_id, value) in args {
      match *arg_id {
        "noul" | "choice" | "score" => {
          let (name, instructions) =
            parse_named_question(value, questions.len() + 1);
          if questions.iter().any(|question| question.name == name) {
            Err(format!("Question name `{name}` is used more than once"))?
          }
          let question = match *arg_id {
            "noul" => JevQuestion::Noul,
            "choice" => JevQuestion::Choice(Vec::new()),
            _ => JevQuestion::Score(Vec::new()),
          };
          questions.push(NamedJevQuestion {
            name,
            instructions,
            question,
          });
        }
        "option" => match questions.last_mut().map(|q| &mut q.question) {
          Some(JevQuestion::Choice(options)) => options.push(value.clone()),
          _ => Err(format!(
            "`-o/--option {value}` must follow a `--choice` question"
          ))?,
        },
        "level" => match questions.last_mut().map(|q| &mut q.question) {
          Some(JevQuestion::Score(levels)) => levels.push(value.clone()),
          _ => Err(format!(
            "`-l/--level {value}` must follow a `--score` question"
          ))?,
        },
        _ => unreachable!("Unknown jev argument `{arg_id}`"),
      }
    }

    for NamedJevQuestion { name, question, .. } in &questions {
      match question {
        JevQuestion::Choice(options) if options.is_empty() => Err(format!(
          "`--choice {name}` needs at least one `-o/--option`"
        ))?,
        JevQuestion::Score(levels) if levels.is_empty() => {
          Err(format!("`--score {name}` needs at least one `-l/--level`"))?
        }
        _ => {}
      }
    }

    if questions.is_empty() {
      Err("Provide at least one `--noul`, `--choice`, or `--score` question")?
    }

    Ok(JevQuestions(questions))
  }
}

impl clap::Args for JevQuestions {
  fn augment_args(cmd: clap::Command) -> clap::Command {
    cmd
      .arg(
        Arg::new("noul")
          .long("noul")
          .value_name("[NAME=]QUESTION")
          .action(ArgAction::Append)
          .help("Yes/no question, answered with the probability of yes"),
      )
      .arg(
        Arg::new("choice")
          .long("choice")
          .value_name("[NAME=]QUESTION")
          .action(ArgAction::Append)
          .help("Pick one of the subsequent `-o/--option`s"),
      )
      .arg(
        Arg::new("option")
          .long("option")
          .short('o')
          .value_name("NAME[=DESCRIPTION]")
          .action(ArgAction::Append)
          .help("An option of the preceding `--choice` question"),
      )
      .arg(
        Arg::new("score")
          .long("score")
          .value_name("[NAME=]QUESTION")
          .action(ArgAction::Append)
          .help("Rate along the subsequent `-l/--level`s"),
      )
      .arg(
        Arg::new("level")
          .long("level")
          .short('l')
          .value_name("DESCRIPTION")
          .action(ArgAction::Append)
          .help(
            "A level of the preceding `--score` question, \
            from lowest to highest",
          ),
      )
  }

  fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
    Self::augment_args(cmd)
  }
}

impl clap::FromArgMatches for JevQuestions {
  fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
    // Restore the command line order, which clap doesn't keep across args
    let mut indexed_args: Vec<(usize, &str, String)> = Vec::new();
    for arg_id in Self::ARG_IDS {
      if let (Some(indices), Some(values)) = (
        matches.indices_of(arg_id),
        matches.get_many::<String>(arg_id),
      ) {
        indexed_args.extend(
          indices
            .zip(values)
            .map(|(index, value)| (index, arg_id, value.clone())),
        );
      }
    }
    indexed_args.sort_by_key(|(index, ..)| *index);
    let args = indexed_args
      .into_iter()
      .map(|(_, arg_id, value)| (arg_id, value))
      .collect::<Vec<_>>();

    Self::from_ordered_args(&args)
      .map_err(|msg| clap::Error::raw(ErrorKind::ArgumentConflict, msg + "\n"))
  }

  fn update_from_arg_matches(
    &mut self,
    matches: &ArgMatches,
  ) -> Result<(), clap::Error> {
    *self = Self::from_arg_matches(matches)?;
    Ok(())
  }
}

/// Use structured JSON input (object or array) as structured state,
/// everything else as plain text
fn parse_state(state: &str) -> Value {
  match serde_json::from_str::<Value>(state) {
    Ok(value @ (Value::Object(_) | Value::Array(_))) => value,
    _ => Value::String(state.to_string()),
  }
}

fn jev_request_body(
  model: &str,
  state: &str,
  questions: &[NamedJevQuestion],
) -> Value {
  let questions_obj = questions
    .iter()
    .map(|named| {
      (
        named.name.clone(),
        named.question.to_json(&named.instructions),
      )
    })
    .collect::<Map<String, Value>>();

  json!({
    "state": parse_state(state),
    "model": model,
    "questions": questions_obj,
  })
}

/// Probabilities of a choice or score answer as `(key, probability)` pairs
fn answer_probabilities(answer: &Value) -> Vec<(String, f64)> {
  answer["probabilities"]
    .as_object()
    .map(|probs| {
      probs
        .iter()
        .map(|(key, prob)| (key.clone(), prob.as_f64().unwrap_or_default()))
        .collect()
    })
    .unwrap_or_default()
}

/// The bare value of an answer (probability, option, or score)
fn answer_value(answer: &Value, is_raw: bool) -> String {
  let answer_type = answer["type"].as_str().unwrap_or_default();
  match &answer[answer_type] {
    Value::String(string) => string.clone(),
    Value::Number(number) if !is_raw => {
      format!("{:.2}", number.as_f64().unwrap_or_default())
    }
    value => value.to_string(),
  }
}

fn answer_confidence(answer: &Value) -> Option<String> {
  answer["confidence"]
    .as_f64()
    .map(|conf| cformat!("<dim>(confidence: {conf:.2})</dim>"))
}

/// Format an answer with all its probabilities for display.
/// Raw mode only prints the bare value.
fn format_answer(answer: &Value, is_raw: bool) -> String {
  let value = answer_value(answer, is_raw);
  if is_raw {
    return value;
  }

  let confidence = answer_confidence(answer)
    .map(|conf| format!(" {conf}"))
    .unwrap_or_default();

  match answer["type"].as_str().unwrap_or_default() {
    "noul" => value,
    "choice" => {
      let mut probs = answer_probabilities(answer);
      probs.sort_by(|a, b| b.1.total_cmp(&a.1));
      let width = probs.iter().map(|(key, _)| key.len()).max().unwrap_or(0);
      let rows = probs
        .iter()
        .map(|(key, prob)| format!("  {key:<width$}  {prob:.2}"))
        .collect::<Vec<_>>()
        .join("\n");
      format!("{value}{confidence}\n\n{rows}")
    }
    "score" => {
      let mut probs = answer_probabilities(answer);
      probs.sort_by_key(|(level, _)| level.parse::<usize>().unwrap_or(0));
      let legends = probs
        .iter()
        .map(|(level, _)| answer["legend"][level].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
      let width = legends.iter().map(|legend| legend.len()).max().unwrap_or(0);
      let rows = probs
        .iter()
        .zip(legends)
        .map(|((level, prob), legend)| {
          format!("  {level}  {legend:<width$}  {prob:.2}")
        })
        .collect::<Vec<_>>()
        .join("\n");
      format!("{value}{confidence}\n\n{rows}")
    }
    _ => serde_json::to_string_pretty(answer).unwrap_or_default(),
  }
}

/// Format the answers to several questions as one line per question,
/// in the order the questions were asked.
/// Raw mode prints tab separated `name` and `value` pairs.
fn format_answers_table(
  questions: &[NamedJevQuestion],
  answers: &Value,
  is_raw: bool,
) -> String {
  let rows = questions
    .iter()
    .map(|named| {
      let answer = &answers[&named.name];
      (
        named.name.as_str(),
        answer_value(answer, is_raw),
        answer_confidence(answer),
      )
    })
    .collect::<Vec<_>>();

  if is_raw {
    return rows
      .iter()
      .map(|(name, value, _)| format!("{name}\t{value}"))
      .collect::<Vec<_>>()
      .join("\n");
  }

  let name_width = rows.iter().map(|(name, ..)| name.len()).max().unwrap_or(0);
  let value_width = rows
    .iter()
    .filter(|(.., confidence)| confidence.is_some())
    .map(|(_, value, _)| value.chars().count())
    .max()
    .unwrap_or(0);

  rows
    .iter()
    .map(|(name, value, confidence)| match confidence {
      Some(conf) => {
        format!("{name:<name_width$}  {value:<value_width$}  {conf}")
      }
      None => format!("{name:<name_width$}  {value}"),
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// Send the questions to a System One model.
/// Prints the full response in JSON mode and returns `None`,
/// otherwise prints the header line (unless in raw mode)
/// and returns the answers.
async fn request_answers(
  opts: &ExecOptions,
  model: &str,
  state: &str,
  questions: &[NamedJevQuestion],
) -> Result<Option<Value>, Box<dyn Error + Send + Sync>> {
  if state.trim().is_empty() {
    Err(
      "Pipe the text to evaluate into cai via stdin, \
      e.g. `echo 'Help, my payouts fail!' | cai noul Is this urgent`",
    )?
  }
  if let Some(named) = questions
    .iter()
    .find(|named| named.instructions.trim().is_empty())
  {
    Err(format!("Question `{}` is empty", named.name))?
  }

  let secrets_path_str = get_secrets_path_str();
  let full_config = get_full_config(&secrets_path_str)?;
  let api_key = full_config
    .get("typesafe_api_key")
    .filter(|key| !key.is_empty())
    .ok_or(format!(
      "A TypeSafe API key must be provided. Use one of the following options:\n\
      \n\
      1. Set `typesafe_api_key` in {secrets_path_str}\n\
      2. Set the env variable CAI_TYPESAFE_API_KEY\n\
      3. Set the env variable TYPESAFE_API_KEY\n\
      \n\
      Create a new API key at https://console.typesafe.ai/keys\n"
    ))?;
  let base_url = get_base_url(
    &full_config,
    "typesafe_base_url",
    "https://api.typesafe.ai/v1",
  );

  let start = Instant::now();
  let resp = reqwest::Client::new()
    .post(format!("{base_url}/systemone"))
    .bearer_auth(api_key)
    .json(&jev_request_body(model, state, questions))
    .send()
    .await?;
  let (elapsed_time, time_unit) =
    format_elapsed_time(start.elapsed().as_millis());

  let status = resp.status();
  let resp_text = resp.text().await?;
  let resp_json = serde_json::from_str::<Value>(&resp_text);

  if !status.is_success() {
    let details = resp_json
      .ok()
      .and_then(|json| serde_json::to_string_pretty(&json).ok())
      .unwrap_or(resp_text);
    return Err(format!("TypeSafe API returned {status}\n\n{details}").into());
  }
  let mut resp_json = resp_json?;

  if opts.is_json {
    println!("{}", serde_json::to_string_pretty(&resp_json)?);
    return Ok(None);
  }

  if let Some(named) = questions
    .iter()
    .find(|named| resp_json["answers"][&named.name].is_null())
  {
    Err(format!(
      "Response contains no answer for `{}`:\n{resp_json}",
      named.name
    ))?
  }

  if !opts.is_raw {
    let subcommand = opts
      .subcommand
      .as_ref()
      .and_then(|x| x.to_string_pretty())
      .map(|subcom| format!("➡️ {subcom} | "))
      .unwrap_or_default();
    let used_model = resp_json["model"].as_str().unwrap_or(model);
    cprintln!(
      "<bold>{subcommand}🧠 TypeSafe {used_model} | ⏱️ {elapsed_time} {time_unit}</bold>\n",
    );
  }

  Ok(Some(resp_json["answers"].take()))
}

/// Ask a System One model a question about the given state
/// and print its answer with all probabilities
pub async fn ask_jev(
  opts: &ExecOptions,
  model: &str,
  state: &str,
  instructions: &str,
  question: &JevQuestion,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let questions = [NamedJevQuestion {
    name: QUESTION_ID.to_string(),
    instructions: instructions.to_string(),
    question: question.clone(),
  }];
  if let Some(answers) = request_answers(opts, model, state, &questions).await?
  {
    println!("{}", format_answer(&answers[QUESTION_ID], opts.is_raw));
  }
  Ok(())
}

/// Ask a System One model several questions about the given state
/// in one request and print one answer per line
pub async fn ask_jev_many(
  opts: &ExecOptions,
  model: &str,
  state: &str,
  questions: &[NamedJevQuestion],
) -> Result<(), Box<dyn Error + Send + Sync>> {
  if let Some(answers) = request_answers(opts, model, state, questions).await? {
    println!("{}", format_answers_table(questions, &answers, opts.is_raw));
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn single(
    instructions: &str,
    question: JevQuestion,
  ) -> Vec<NamedJevQuestion> {
    vec![NamedJevQuestion {
      name: QUESTION_ID.to_string(),
      instructions: instructions.to_string(),
      question,
    }]
  }

  fn args(pairs: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    pairs
      .iter()
      .map(|(arg_id, value)| (*arg_id, value.to_string()))
      .collect()
  }

  #[test]
  fn test_noul_request_body() {
    let body = jev_request_body(
      "jev-latest",
      "Help! My payouts fail.",
      &single("Is this urgent?", JevQuestion::Noul),
    );
    assert_eq!(
      body,
      json!({
        "state": "Help! My payouts fail.",
        "model": "jev-latest",
        "questions": {
          "answer": { "type": "noul", "instructions": "Is this urgent?" }
        }
      })
    );
  }

  #[test]
  fn test_choice_request_body_parses_descriptions() {
    let options = vec![
      "billing = Payment issues".to_string(),
      "technical=Bugs, a=b".to_string(),
      "sales".to_string(),
    ];
    let questions = single("Which team?", JevQuestion::Choice(options));
    let body = jev_request_body("jev-latest", "x", &questions);
    assert_eq!(
      body["questions"]["answer"]["criteria"],
      json!({
        "billing": "Payment issues",
        "technical": "Bugs, a=b",
        "sales": null,
      })
    );
  }

  #[test]
  fn test_score_request_body_keeps_level_order() {
    let levels = vec!["Calm".to_string(), "Angry".to_string()];
    let questions = single("How angry?", JevQuestion::Score(levels));
    let body = jev_request_body("jev-latest", "x", &questions);
    assert_eq!(body["questions"]["answer"]["type"], "score");
    assert_eq!(
      body["questions"]["answer"]["criteria"],
      json!(["Calm", "Angry"])
    );
  }

  #[test]
  fn test_json_input_is_sent_as_structured_state() {
    let questions = single("?", JevQuestion::Noul);
    let body = jev_request_body("jev-latest", r#"{"a": 1}"#, &questions);
    assert_eq!(body["state"], json!({ "a": 1 }));

    // JSON scalars stay plain text
    let body = jev_request_body("jev-latest", "42", &questions);
    assert_eq!(body["state"], json!("42"));
  }

  #[test]
  fn test_parse_named_question() {
    let cases = [
      ("urgent=Is it urgent?", ("urgent", "Is it urgent?")),
      ("is_repeat_2=Again?", ("is_repeat_2", "Again?")),
      ("Is 1+1=2?", ("q3", "Is 1+1=2?")),
      ("no name", ("q3", "no name")),
      ("=Empty name?", ("q3", "=Empty name?")),
    ];
    for (value, (name, question)) in cases {
      assert_eq!(
        parse_named_question(value, 3),
        (name.to_string(), question.to_string()),
        "`{value}`"
      );
    }
  }

  #[test]
  fn test_questions_from_ordered_args() {
    let questions = JevQuestions::from_ordered_args(&args(&[
      ("noul", "human=Asks for a human?"),
      ("choice", "team=Which team?"),
      ("option", "billing=Payments"),
      ("option", "sales"),
      ("score", "How angry?"),
      ("level", "Calm"),
      ("level", "Angry"),
    ]))
    .unwrap();

    let summary = questions
      .0
      .iter()
      .map(|named| (named.name.as_str(), named.question.clone()))
      .collect::<Vec<_>>();
    assert_eq!(
      summary,
      vec![
        ("human", JevQuestion::Noul),
        (
          "team",
          JevQuestion::Choice(vec!["billing=Payments".into(), "sales".into()])
        ),
        (
          "q3",
          JevQuestion::Score(vec!["Calm".into(), "Angry".into()])
        ),
      ]
    );
  }

  #[test]
  fn test_questions_from_invalid_args() {
    let cases = [
      (vec![], "at least one"),
      (vec![("option", "sales")], "must follow a `--choice`"),
      (
        vec![("noul", "a=?"), ("option", "x")],
        "must follow a `--choice`",
      ),
      (
        vec![("choice", "a=?"), ("level", "x")],
        "must follow a `--score`",
      ),
      (vec![("choice", "a=?")], "`--choice a` needs at least one"),
      (vec![("score", "a=?")], "`--score a` needs at least one"),
      (
        vec![("noul", "a=?"), ("noul", "a=!")],
        "`a` is used more than once",
      ),
    ];
    for (pairs, expected) in cases {
      let err = JevQuestions::from_ordered_args(&args(&pairs)).unwrap_err();
      assert!(err.contains(expected), "{pairs:?}: {err}");
    }
  }

  #[test]
  fn test_cli_flags_keep_command_line_order() {
    use clap::{Args, FromArgMatches};
    let cmd = JevQuestions::augment_args(clap::Command::new("jev"));
    let matches = cmd
      .try_get_matches_from([
        "jev", "--score", "s=?", "-l", "low", "--choice", "c=?", "-o", "x",
        "--score", "t=?", "-l", "high", "--noul", "n=?",
      ])
      .unwrap();
    let questions = JevQuestions::from_arg_matches(&matches).unwrap();
    let summary = questions
      .0
      .iter()
      .map(|named| (named.name.as_str(), named.question.clone()))
      .collect::<Vec<_>>();
    assert_eq!(
      summary,
      vec![
        ("s", JevQuestion::Score(vec!["low".into()])),
        ("c", JevQuestion::Choice(vec!["x".into()])),
        ("t", JevQuestion::Score(vec!["high".into()])),
        ("n", JevQuestion::Noul),
      ]
    );
  }

  #[test]
  fn test_format_raw_answers() {
    let noul = json!({ "type": "noul", "noul": 0.95 });
    assert_eq!(format_answer(&noul, true), "0.95");

    let choice = json!({
      "type": "choice",
      "choice": "technical",
      "confidence": 0.78,
      "probabilities": { "technical": 0.85, "billing": 0.15 }
    });
    assert_eq!(format_answer(&choice, true), "technical");

    let score = json!({
      "type": "score",
      "score": 1.05,
      "confidence": 0.92,
      "legend": { "0": "Calm", "1": "Frustrated" },
      "probabilities": { "0": 0.0, "1": 1.0 }
    });
    assert_eq!(format_answer(&score, true), "1.05");
  }

  #[test]
  fn test_format_choice_sorts_by_probability() {
    let choice = json!({
      "type": "choice",
      "choice": "technical",
      "confidence": 0.78,
      "probabilities": { "billing": 0.15, "sales": 0.0, "technical": 0.85 }
    });
    let formatted = format_answer(&choice, false);
    let rows = formatted.lines().skip(2).collect::<Vec<_>>();
    assert_eq!(
      rows,
      vec![
        "  technical  0.85",
        "  billing    0.15",
        "  sales      0.00"
      ]
    );
  }

  #[test]
  fn test_format_score_sorts_levels_numerically() {
    let levels = (0..12)
      .map(|level| (level.to_string(), json!(format!("L{level}"))))
      .collect::<Map<String, Value>>();
    let probs = (0..12)
      .map(|level| (level.to_string(), json!(0.0)))
      .collect::<Map<String, Value>>();
    let score = json!({
      "type": "score",
      "score": 0.0,
      "confidence": 1.0,
      "legend": levels,
      "probabilities": probs,
    });
    let formatted = format_answer(&score, false);
    let first_cols = formatted
      .lines()
      .skip(2)
      .map(|row| row.split_whitespace().next().unwrap())
      .collect::<Vec<_>>();
    assert_eq!(
      first_cols,
      (0..12).map(|level| level.to_string()).collect::<Vec<_>>()
    );
  }

  #[test]
  fn test_format_answers_table_keeps_question_order() {
    let questions = JevQuestions::from_ordered_args(&args(&[
      ("noul", "zeta=?"),
      ("choice", "team=?"),
      ("option", "technical"),
      ("noul", "alpha=?"),
    ]))
    .unwrap()
    .0;
    let answers = json!({
      "alpha": { "type": "noul", "noul": 0.1 },
      "team": { "type": "choice", "choice": "technical", "confidence": 0.75 },
      "zeta": { "type": "noul", "noul": 0.97 },
    });

    assert_eq!(
      format_answers_table(&questions, &answers, true),
      "zeta\t0.97\nteam\ttechnical\nalpha\t0.1"
    );

    let table = format_answers_table(&questions, &answers, false);
    let lines = table.lines().collect::<Vec<_>>();
    assert_eq!(lines[0], "zeta   0.97");
    assert!(lines[1].starts_with("team   technical  "), "{}", lines[1]);
    assert!(lines[1].contains("(confidence: 0.75)"), "{}", lines[1]);
    assert_eq!(lines[2], "alpha  0.10");
  }
}
