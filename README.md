# review-agent

`review-agent` is a Rust CLI that packages a git branch diff, sends it to a local Ollama model for review, saves the markdown review, renders it to a self-contained HTML report, and opens that report in your browser.

## Installation

```bash
cargo install --path .
```

## Prerequisites

- Ollama installed locally (for local models)
- Or API keys for cloud providers (see below)

### Setting up API keys

For OpenAI or Anthropic providers, set the appropriate environment variable in your `.zshrc` or `.bashrc`:

```bash
# For OpenAI
export OPENAI_API_KEY="sk-..."

# For Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."
```

### Default Models

Each provider has a default model:

| Provider | Default Model | Documentation |
|----------|---------------|---------------|
| `ollama` | `qwen3.5` | |
| `openai` | `gpt-5.4` | [OpenAI Models](https://platform.openai.com/docs/models) |
| `anthropic` | `claude-sonnet-4-6` | [Anthropic Models](https://docs.anthropic.com/en/docs/about-claude/models) |

Override the default with `--model <model>`:

For local Ollama, pull a model:

```bash
ollama pull qwen3.5:27b
```

- Start Ollama before running reviews:

```bash
ollama serve
```

If Ollama is not running, `review-agent` will exit with:

```text
Ollama is not running. Start it with: ollama serve
```

## Usage

### `review-agent pack [base-branch] [output-dir] [--template <template>] [--uncommitted]`

Runs the bundled shell workflow against the git repo in your current working directory.

Examples:

```bash
review-agent pack
review-agent pack origin/main
review-agent pack origin/main /tmp/review-my-branch
review-agent pack origin/main --template rust
review-agent pack --template angular
review-agent pack origin/main /Users/t/projects/R/review-agent/review-feat/uncommitted-review --uncommitted
```

### `review-agent run <input> [--provider <provider>] [--model <model>] [--host <host>] [--no-open] [--no-think]`

Reviews an existing package directory or a `.zip` file created from one.

- Reads `AGENTS.md` as the system prompt
- Reads `summary.md` and every file under `patches/` recursively as the user prompt
- Streams LLM output live to stdout
- Writes `review.md`
- Renders `review.html`
- Opens the HTML report unless `--no-open` is set

Examples:

```bash
# Local Ollama (default)
review-agent run ./review-my-branch
review-agent run ./review-my-branch --provider ollama --model qwen3.5:27b
review-agent run ./review-my-branch --host 192.168.1.100:11434

# OpenAI
review-agent run ./review-my-branch --provider openai --model gpt-4o

# Anthropic
review-agent run ./review-my-branch --provider anthropic --model claude-sonnet-4-6
```

### `review-agent review [--base-branch <branch>] [--template <template>] [--provider <provider>] [--model <model>] [--host <host>] [--no-open] [--no-think]`

Packages the current git branch first, then immediately runs the LLM review flow on the generated folder.

Examples:

```bash
# Local Ollama (default)
review-agent review
review-agent review --provider ollama --model qwen3.5:27b
review-agent review --host https://ollama.example

# OpenAI
review-agent review --provider openai --model gpt-4o

# With specific base branch
review-agent review --base-branch origin/main --template rust
```

## Template Options

The `--template` flag selects which AGENTS.md template is used for the review:

| Template | Description |
|----------|-------------|
| `general` | Generic code review guidelines (default) |
| `rust` | Rust-specific best practices and patterns |
| `angular` | Angular/TypeScript best practices |

## Flags

| Flag | Description |
|------|-------------|
| `--provider <provider>` | LLM provider to use. Options: `ollama` (default), `openai`, `anthropic` |
| `--model <model>` | Model to use. Default: `qwen3.5` for Ollama |
| `--host <host>` | Optional Ollama server address to use instead of localhost, for example `192.168.1.100:11434` or `https://ollama.example` |
| `--uncommitted` | Review working tree changes without creating a temporary branch |
| `--no-open` | Skip opening `review.html` in the browser |
| `--no-think` | Send `think: false` to Ollama for faster responses (ignored for OpenAI/Anthropic) |

## Model Selection

- `qwen3.5:9b` -> 8GB RAM
- `qwen3.5:27b` -> 16GB RAM
- `qwen3.5:35b` -> 24GB RAM
- `qwen3.5:122b` -> 64GB RAM

## Output

After a review finishes, the package directory contains:

- `review.md`: the model's markdown review
- `review.html`: a self-contained HTML rendering of the review

For `.zip` input, the archive is extracted to a temp directory first and the generated files are written there.
