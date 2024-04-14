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
Cai 0.5.0

The fastest CLI tool for prompting LLMs

Usage: cai [PROMPT]...
       cai <COMMAND>

Commands:
  mixtral        Groq's Mixtral [aliases: mi]
  gpt-turbo      OpenAI's GPT 4 Turbo [aliases: tu]
  gpt            OpenAI's GPT 4 [aliases: gp]
  claude-opus    Anthropic's Claude Opus [aliases: op]
  claude-sonnet  Anthropic's Claude Sonnet [aliases: so]
  claude-haiku   🏆 Default | Anthropic's Claude Haiku [aliases: ha]
  llamafile      Llamafile server hosted at http://localhost:8080 [aliases: lf]
  ollama         Ollama server hosted at http://localhost:11434 [aliases: ol]
  all            Send prompt to each provider's default model simultaneously (Claude Haiku, Groq Mixtral, GPT 4 Turbo,
                     Llamafile, Ollama Llama2)
  help           Print this message or the help of the given subcommand(s)

Arguments:
  [PROMPT]...  The prompt to send to the AI model

Options:
  -h, --help  Print help


Examples:
  # Send a prompt to the default model
  cai How heigh is the Eiffel Tower in meters

  # Send a prompt to each provider's default model
  cai all How heigh is the Eiffel Tower in meters

  # Send a prompt to Anthropic's Claude Opus (+ alias)
  cai claude-opus How heigh is the Eiffel Tower in meters
  cai op How heigh is the Eiffel Tower in meters

  # Send a prompt to locally running Ollama server
  cai ollama mistral How heigh is the Eiffel Tower in meters
  cai ol mistral How heigh is the Eiffel Tower in meters

  # Add data via stdin
  cat main.rs | cai Explain this code
```


## Related

- [AI CLI] - Get answers for CLI commands from ChatGPT. (TypeScript)
- [AIChat] - All-in-one chat and copilot CLI for 10+ AI platforms. (Rust)
- [ja] - CLI / TUI app to work with AI tools. (Rust)
- [llm] - Access large language models from the command-line. (Python)
- [smartcat] - Integrate LLMs in the Unix command ecosystem. (Rust)
- [tgpt] - AI chatbots for the terminal without needing API keys. (Go)

[AI CLI]: https://github.com/abhagsain/ai-cli
[AIChat]: https://github.com/sigoden/aichat
[ja]: https://github.com/joshka/ja
[llm]: https://github.com/simonw/llm
[smartcat]: https://github.com/efugier/smartcat
[tgpt]: https://github.com/aandrew-me/tgpt
