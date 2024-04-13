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
- **localhost:8080** - Any OpenAI API compatible local server (E.g. [llamafile])

[llamafile]: https://github.com/Mozilla-Ocho/llamafile

Afterwards, you can use `cai` to run prompts directly from the terminal:

```sh
cai List 10 fast CLI tools
```

Or a specific model, like Anthropic's Claude Opus:

```sh
cai op List 10 fast CLI tools
```

For more information, run:

```sh
cai help
```


## Related

- [AIChat] - All-in-one chat and copilot CLI for 10+ AI platforms. (Rust)
- [ja] - CLI / TUI app to work with AI tools. (Rust)
- [llm] - Access large language models from the command-line. (Python)
- [smartcat] - Integrate LLMs in the Unix command ecosystem. (Rust)

[AIChat]: https://github.com/sigoden/aichat
[ja]: https://github.com/joshka/ja
[llm]: https://github.com/simonw/llm
[smartcat]: https://github.com/efugier/smartcat
