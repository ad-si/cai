#[cfg(test)]
mod tests {
  use assert_cmd::Command;
  use predicates::prelude::*;

  #[test]
  fn test_default_cai_execution() {
    let mut cmd = Command::cargo_bin("cai").unwrap();

    cmd
      .args(&["Which year did the Titanic sink?", "(Just the number)"])
      .assert()
      .success()
      .stderr("")
      .stdout(predicate::str::contains("Groq llama-3"))
      .stdout(predicate::str::contains("1912"));
  }
  #[test]
  fn test_ollama_cai_execution() {
    let mut cmd = Command::cargo_bin("cai").unwrap();

    cmd
      .args(&[
        "ollama", "llama3", "Which", "year", "did", "the", "Titanic", "sink?",
        "(Just", "the", "number)",
      ])
      .assert()
      .success()
      .stderr("")
      .stdout(predicate::str::contains("Ollama"))
      .stdout(predicate::str::contains("1912"));
  }
  #[test]
  fn test_ollama_shortcut_cai_execution() {
    let mut cmd = Command::cargo_bin("cai").unwrap();
    cmd
      .args(&[
        "ol", "ll", "Which", "year", "did", "the", "Titanic", "sink?", "(Just",
        "the", "number)",
      ])
      .assert()
      .success()
      .stderr("")
      .stdout(predicate::str::contains("Ollama"))
      .stdout(predicate::str::contains("1912"));
  }
  #[test]
  fn test_ollama_fails_cai_execution() {
    let mut cmd = Command::cargo_bin("cai").unwrap();
    cmd
      .args(&["ollama", "xxx", "prompt"])
      .assert()
      .failure()
      .stderr(predicate::str::contains("Ollama"))
      .stderr(predicate::str::contains("api_error"))
      .stderr(predicate::str::contains("not found"))
      .stdout("");
  }
}
