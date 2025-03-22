# `cai` - The fastest CLI tool for prompting LLMs

## Features

- Build with Rust 🦀 for supreme performance and speed! 🏎️
- Support for models by [Groq], [OpenAI], [Anthropic], and local LLMs. 📚
- Prompt several models at once. 🤼
    ![Demo of cai's all command](screenshots/2024-04-13t1627_all.png)
- Syntax highlighting for better readability of code snippets. 🌈

[Groq]: https://console.groq.com/docs/models
[OpenAI]: https://platform.openai.com/docs/models
[Anthropic]: https://docs.anthropic.com/claude/docs/models-overview


## Demo

![`cai` demo](./demos/main.gif)


## Installation

```sh
cargo install cai
```


## Usage

Before using Cai, an API key must be set up.
Simply execute `cai` in your terminal and follow the instructions.

Cai supports the following APIs:

- **Groq** - [Create new API key](https://console.groq.com/keys).
- **OpenAI** - [Create new API key](https://platform.openai.com/api-keys).
- **Anthropic** -
    [Create new API key](https://console.anthropic.com/settings/keys).
- **Llamafile** - Local [Llamafile] server running at http://localhost:8080.
- **Ollama** - Local [Ollama] server running at http://localhost:11434.

[Llamafile]: https://github.com/Mozilla-Ocho/llamafile
[Ollama]: https://github.com/ollama/ollama

Afterwards, you can use `cai` to run prompts directly from the terminal:

```sh
cai List 10 fast CLI tools
```

Or a specific model, like Anthropic's Claude Opus:

```sh
cai op List 10 fast CLI tools
```

Full help output:

```txt
$ cai help
Cai 0.10.0

The fastest CLI tool for prompting LLMs

Usage: cai [OPTIONS] [PROMPT]... [COMMAND]

Commands:
  google     Google [aliases: go]
  ge         - Gemini Pro shortcut
  gf         - Gemini Flash shortcut
  groq       Groq [aliases: gr]
  ll         - Llama 3 shortcut (🏆 Default)
  mi         - Mixtral shortcut
  cerebras   Cerebras [aliases: ce]
  deepseek   DeepSeek [aliases: de]
  openai     OpenAI [aliases: op]
  gp         - GPT-4o shortcut
  gm         - GPT-4o mini shortcut
  anthropic  Anthropic [aliases: an]
  cl         - Claude Opus
  so         - Claude Sonnet
  ha         - Claude Haiku
  xai        xAI
  grok       - Grok
  llamafile  Llamafile server hosted at http://localhost:8080 [aliases: lf]
  ollama     Ollama server hosted at http://localhost:11434 [aliases: ol]
  all        Simultaneously send prompt to each provider's default model:
             - Groq Llama 3.1
             - Antropic Claude Sonnet 3.7
             - Google Gemini 2.0 Flash
             - OpenAI GPT-4o mini
             - Ollama Llama 3
             - Llamafile
  changelog  Generate a changelog starting from a given commit using OpenAI's GPT-4o
  rename     Analyze and rename a file with timestamp and description
  ocr        Extract text from an image
  bash       Use Bash development as the prompt context
  c          Use C development as the prompt context
  cpp        Use C++ development as the prompt context
  cs         Use C# development as the prompt context
  elm        Use Elm development as the prompt context
  fish       Use Fish development as the prompt context
  fs         Use F# development as the prompt context
  gd         Use Godot and GDScript development as the prompt context
  gl         Use Gleam development as the prompt context
  golang     Use Go development as the prompt context
  hs         Use Haskell development as the prompt context
  java       Use Java development as the prompt context
  js         Use JavaScript development as the prompt context
  kt         Use Kotlin development as the prompt context
  ly         Use LilyPond development as the prompt context
  lua        Use Lua development as the prompt context
  oc         Use OCaml development as the prompt context
  php        Use PHP development as the prompt context
  pg         Use Postgres development as the prompt context
  ps         Use PureScript development as the prompt context
  py         Use Python development as the prompt context
  rb         Use Ruby development as the prompt context
  rs         Use Rust development as the prompt context
  sql        Use SQLite development as the prompt context
  sw         Use Swift development as the prompt context
  ts         Use TypeScript development as the prompt context
  ty         Use Typst development as the prompt context
  wl         Use Wolfram Language and Mathematica development as the prompt context
  zig        Use Zig development as the prompt context
  help       Print this message or the help of the given subcommand(s)

Arguments:
  [PROMPT]...  The prompt to send to the AI model

Options:
  -r, --raw                        Print raw response without any metadata
  -j, --json                       Prompt LLM in JSON output mode
      --json-schema <JSON_SCHEMA>  JSON schema to validate the output against
  -h, --help                       Print help


Examples:
  # Send a prompt to the default model
  cai Which year did the Titanic sink

  # Send a prompt to each provider's default model
  cai all Which year did the Titanic sink

  # Send a prompt to Anthropic's Claude Opus
  cai anthropic claude-opus Which year did the Titanic sink
  cai an claude-opus Which year did the Titanic sink
  cai cl Which year did the Titanic sink
  cai anthropic claude-3-opus-latest Which year did the Titanic sink

  # Send a prompt to locally running Ollama server
  cai ollama llama3 Which year did the Titanic sink
  cai ol ll Which year did the Titanic sink

  # Add data via stdin
  cat main.rs | cai Explain this code

  # Use a JSON schema to specify the output format
  cai --json-schema='{"properties":{"age":{"type":"number"}},"required":["age"]}' gp Barack Obama
  
```


## Related

- [AI CLI] - Get answers for CLI commands from ChatGPT. (TypeScript)
- [AIChat] - All-in-one chat and copilot CLI for 10+ AI platforms. (Rust)
- [Ell] - CLI tool for LLMs written in Bash.
- [ja] - CLI / TUI app to work with AI tools. (Rust)
- [llm] - Access large language models from the command-line. (Python)
- [smartcat] - Integrate LLMs in the Unix command ecosystem. (Rust)
- [tgpt] - AI chatbots for the terminal without needing API keys. (Go)

[AI CLI]: https://github.com/abhagsain/ai-cli
[AIChat]: https://github.com/sigoden/aichat
[Ell]: https://github.com/simonmysun/ell
[ja]: https://github.com/joshka/ja
[llm]: https://github.com/simonw/llm
[smartcat]: https://github.com/efugier/smartcat
[tgpt]: https://github.com/aandrew-me/tgpt
