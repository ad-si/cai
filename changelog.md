# Changelog

# 2024-12-08 - v0.8.0

- New sub-command `ocr` to extract text from images
- New sub-command `rename` to rename files based on their content
- Sub-commands with a programming language as the prompt context
- New sub-command `changelog` to generate a changelog from git commits
- Always use the latest versions of Anthropic's 3.5 models
- Update default Llama versions


## 2024-07-21 - v0.7.0

- Flag `--json` to activate JSON output mode
- Flag `--raw` to print the raw LLM response without any metadata
- More aliases for Anthropic, use new versions as default models
- CLI output shows resolved model id instead of the used alias
- More aliases for OpenAI models


## 2024-04-30 - v0.6.0

- Support for all Groq and Ollama models
- Groq Llama3 is the new default model
- List aliases in Ollama help text


## 2024-04-14 - v0.5.0

- Support adding text to the prompt via stdin
- Support local Ollama server
- Examples to help output


## 2024-04-13 - v0.4.0

- Support for Anthropic's Claude models
- Support running several models at once with cai all
- Syntax highlight the output as markdown with
    [bat](https://github.com/sharkdp/bat)


## 2024-03-31 - v0.3.0

- Support for sub-commands to specify the API provider
- Set up proper CLI args handling powered by [clap.rs](https://clap.rs/)


## 2024-03-29 - 0.2.0

- Try to load API keys first from `secrets.yaml` and then from env variables


## 2024-03-28 - 0.1.0

* Initial release
