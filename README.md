# review-agent

`review-agent` is a Rust CLI that packages a git branch diff, sends it to a local Ollama model for review, saves the markdown review, renders it to a self-contained HTML report, and opens that report in your browser.

## Installation

```bash
cargo install --path .
```

## Prerequisites

- Ollama installed locally
- A pulled review model, for example:

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

### `review-agent pack [base-branch] [output-dir]`

Runs the bundled shell workflow against the git repo in your current working directory.

Examples:

```bash
review-agent pack
review-agent pack origin/main
review-agent pack origin/main /tmp/review-my-branch
```

### `review-agent run <input> [--model <model>] [--no-open] [--no-think]`

Reviews an existing package directory or a `.zip` file created from one.

- Reads `AGENTS.md` as the system prompt
- Reads `summary.md` and every file under `patches/` recursively as the user prompt
- Streams Ollama output live to stdout
- Writes `review.md`
- Renders `review.html`
- Opens the HTML report unless `--no-open` is set

Examples:

```bash
review-agent run ./review-my-branch
review-agent run ./review-my-branch.zip --model qwen3.5:27b
review-agent run ./review-my-branch --no-open --no-think
```

### `review-agent review [--base-branch <branch>] [--model <model>] [--no-open] [--no-think]`

Packages the current git branch first, then immediately runs the Ollama review flow on the generated folder.

Examples:

```bash
review-agent review
review-agent review --base-branch origin/main
review-agent review --base-branch origin/main --model qwen3.5:27b --no-open
```

## Flags

- `--model <model>`: Ollama model to use. Default: `qwen3.5`
- `--no-open`: Skip opening `review.html` in the browser
- `--no-think`: Sends `think: false` to Ollama for faster responses

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
