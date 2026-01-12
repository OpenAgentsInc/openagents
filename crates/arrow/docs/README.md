# Arrow

Happy path testing binary for Autopilot with GPT-OSS integration.

## Overview

Arrow is a test harness that verifies Autopilot can run correctly with local LLM inference via GPT-OSS/llama-server. It provides:

- **Environment detection** - OS, arch, API keys
- **Backend detection** - GPT-OSS server health check
- **Auto-start** - Launches llama-server if not running
- **Inference test** - Basic completion with Harmony format
- **DSPy chain test** - Full code change pipeline test

## Usage

```bash
# Basic run (inference test)
cargo run -p arrow

# DSPy chain test
cargo run -p arrow -- --test-dspy-chain

# With verbose logging
cargo run -p arrow -- --verbose

# Skip inference test
cargo run -p arrow -- --skip-inference

# Disable auto-start
cargo run -p arrow -- --no-auto-start

# Custom model path
cargo run -p arrow -- --model-path /path/to/model.gguf
```

## DSPy Chain Test

The `--test-dspy-chain` flag runs a complete DSPy code change pipeline:

1. **Task Understanding** - Parse user request into structured task
2. **Code Exploration** - Identify relevant files and search queries
3. **Code Edit Generation** - Generate unified diff
4. **Verification** - Check if changes meet requirements

Example output:
```
[Stage 1] Understanding task...
  Task type: Feature
  Scope: Small
  Requirements:
    - Add a '--version' command-line flag to the CLI parser
    - When the flag is present, display the version string
  Confidence: 95.0%

[Stage 2] Exploring code...
  Queries: ["Args struct", "clap Parser"]
  Lanes: ["src/main.rs"]

[Stage 3-4] Generating code edit...
  File: src/main.rs
  Summary: Added --version flag using clap
  Diff:
    +    #[arg(long)]
    +    version: bool,

[Stage 5] Verifying changes...
  Status: Pass
  Confidence: 99.0%
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `LLAMA_MODEL_PATH` | Path to `.gguf` model file |
| `OPENAI_API_KEY` | OpenAI API key (optional) |
| `CEREBRAS_API_KEY` | Cerebras API key (optional) |

## Model Discovery

Arrow auto-discovers models in these locations:
- `~/models/gpt-oss/*.gguf`
- `LLAMA_MODEL_PATH` environment variable

## Requirements

- `llama-server` binary in PATH (from llama.cpp)
- GPT-OSS model file (`.gguf` format)

## License

MIT
