# GPT-OSS Local Inference

This guide covers running GPT-OSS locally inside OpenAgents and using the unified `local-infer` runner.

## Quick Start

1. Start a GPT-OSS server (llama.cpp `llama-server` or compatible endpoint):

```bash
~/code/llama.cpp/build/bin/llama-server \
  -m ~/models/gpt-oss-20b/gguf/gpt-oss-20b-Q3_K_S.gguf \
  --port 8000 -ngl 999 -c 512 -b 256 -ub 256 --no-warmup --no-mmap
```

2. Run a prompt via the unified runner (fast RAW mode):

```bash
scripts/local-infer.sh --backend gpt-oss --raw \
  --url http://localhost:8000 --model gpt-oss-20b \
  --max-tokens 64 --temperature 0 "Hello from GPT-OSS"
```

3. Enable local tools (browser, python, apply_patch, ui_pane):

```bash
scripts/local-infer.sh --backend gpt-oss --tools "Summarize this repo"
```

Notes:
- `--raw` skips Harmony formatting for speed. Use it for quick completions.
- Harmony mode is slower but required for tool-use and structured outputs.
- `--no-mmap` is **critical** for fast decode on macOS (loads model into RAM).

## Configuration

- `--url` overrides the backend base URL.
- `--model` selects the model ID passed to the backend.
- `--workspace` sets the root for `apply_patch`.
- `--record` enables rlog recording in `docs/logs/YYYYMMDD/`.

You can also set `GPT_OSS_URL` to change the default base URL.

## Tool Calls

GPT-OSS uses Harmony tool calls: assistant messages that set `recipient` to the tool name and include JSON parameters as the message content. `GptOssSession::send_with_tools` runs the tool loop automatically when `--tools` is enabled in `local-infer`.

## Harmony Prompt Formatting

GPT-OSS models expect Harmony-formatted prompts. The `gpt-oss` crate exposes a small adapter:

```rust
use gpt_oss::{HarmonyPromptConfig, HarmonyRenderer, HarmonyRole, HarmonyTurn};

let renderer = HarmonyRenderer::gpt_oss()?;
let turns = vec![HarmonyTurn::new(HarmonyRole::User, "Hello from Harmony")];
let prompt = renderer.render_prompt_with_config(
    &turns,
    &[],
    Some(&HarmonyPromptConfig::new()),
)?;
```

`GptOssSession` uses the Harmony renderer by default to format prompts and parse assistant output.

## Responses API (Tools + Reasoning)

`GptOssClient::responses()` targets `/v1/responses` and supports tool definitions and reasoning effort:

```rust
use gpt_oss::{GptOssClient, GptOssReasoningEffort, GptOssResponsesRequest};

let client = GptOssClient::builder()
    .base_url("http://localhost:8000")
    .build()?;

let request = GptOssResponsesRequest::new("gpt-oss-20b", "Summarize this repo.")
    .with_reasoning_effort(GptOssReasoningEffort::Low);

let response = client.responses(request).await?;
println!("{}", response.output_text());
```

## Examples

- `docs/gpt-oss/examples/basic_client.rs` - Minimal `GptOssClient` usage
- `docs/gpt-oss/examples/agent_session.rs` - Agent wrapper with session tracking

## Related Docs

- `docs/local-inference.md` - performance notes and model setup
- `docs/gpt-oss/API.md` - HTTP surface and Rust type mapping
- `docs/gpt-oss/BENCHMARKS.md` - benchmark harness and run instructions
- `crates/gpt-oss-agent/src/tools/` - tool implementations
