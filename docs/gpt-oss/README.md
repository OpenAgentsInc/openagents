# GPT-OSS Local Inference

This guide covers running GPT-OSS locally inside OpenAgents and using the unified `local-infer` runner.

## Quick Start

1. Start a GPT-OSS server (llama.cpp `llama-server` or compatible endpoint):

```bash
scripts/gpt-oss-fast.sh
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

4. Fastest raw query (minimal overhead):

```bash
scripts/gpt-oss-query.sh "1+1="
```

Defaults: `GPT_OSS_MAX_TOKENS=8`, `GPT_OSS_TEMPERATURE=0`.

5. Stop keepalive loop (optional):

```bash
scripts/gpt-oss-stop-keepalive.sh
```

6. Status + quick timing sample:

```bash
scripts/gpt-oss-status.sh
```

7. Quick latency bench:

```bash
scripts/gpt-oss-bench.sh 10
```

Notes:
- `--raw` skips Harmony formatting for speed. Use it for quick completions.
- Harmony mode is slower but required for tool-use and structured outputs.
- `--no-mmap` is **critical** for fast decode on macOS (loads model into RAM).
- First request after start/idle can be slow; send a quick warmup prompt to fully page-in.
- `scripts/gpt-oss-fast.sh` auto-picks the first available quant in `~/models/gpt-oss-20b/gguf` (Q2_K → Q3_K_S → Q4_0 → Q4_K_M).
- `scripts/gpt-oss-fast.sh` supports env overrides like `GPT_OSS_GGUF_MODEL_PATH`, `GPT_OSS_PORT`, `GPT_OSS_WARMUP_COUNT`, and `GPT_OSS_WARMUP_MAX_TOKENS`.
- If your system pages out, set `GPT_OSS_KEEPALIVE_SECS=2` to keep the server hot (lower = more stable, more background load).
- Keepalive PID file defaults to `/tmp/gpt-oss-keepalive.pid` (`GPT_OSS_KEEPALIVE_PID_FILE`).
- Set `GPT_OSS_PARALLEL=4` to allow keepalive requests without blocking interactive prompts.
- Set `GPT_OSS_FORCE_WARMUP=1` to run warmup even if the server is already running.
- Scripts force IPv4 (`curl -4`) to avoid localhost IPv6 delays.

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
