# GPT-OSS GGUF via Pylon log (2026-01-03)

## Summary
I wired GPT-OSS GGUF to run through Pylon (CPU path) and stream live telemetry into `/gptoss` so the web UI can show load + inference stages and token output without a browser-side GGUF load.

## Work completed
- Promoted the GPT-OSS GGUF runtime into a reusable engine (`GptOssEngine`) and refactored the CLI to use it.
- Added `GptOssGgufBackend` in `ml` and registered it in Pylon behind the `gpt-oss-gguf` feature.
- Added `pylon infer` (CLI streaming), plus `pylon api` with `/health`, `/v1/models`, `/v1/completions` SSE.
- Extended Pylon SSE payloads to include `extra.telemetry` events (load/inference stages + token telemetry).
- Wired `/gptoss` to default to `pylon://127.0.0.1:9899` and parse SSE telemetry into the existing viz panels.
- Auto-select the GPT-OSS model id when multiple models are returned.
- Added a default GGUF path fallback in Pylon (repo-local path) so `GPT_OSS_GGUF_PATH` is optional.
- Added missing load/inference telemetry for model config, token limits, tokenizer summary, moe mode, and prefill completion.
- Updated `crates/ml/docs/plans/gptoss-pylon-gguf-plan.md` with progress logs as work proceeded.

## Files touched (high-signal)
- `crates/ml/src/gptoss_engine.rs`
- `crates/ml/src/gptoss_backend.rs`
- `crates/ml/src/gptoss_tokenizer.rs`
- `crates/pylon/src/cli/api.rs`
- `crates/pylon/src/cli/infer.rs`
- `crates/pylon/src/provider.rs`
- `crates/web/client/src/gptoss_runtime.rs`
- `crates/web/client/src/state.rs`
- `crates/ml/docs/plans/gptoss-pylon-gguf-plan.md`

## Tests run
- `cargo check -p pylon --features gpt-oss-gguf` (passes; only existing dead-code warning in tokenizer)

## Suggested next steps
1) Add lightweight `weights_fetch` progress from Pylon so the load panel shows bytes/total during CPU weight streaming.
2) Add a `pylon://` health banner in `/gptoss` (call `/health` and show connected/failed state).
3) Add a small integration test that hits `pylon api` with a tiny prompt and asserts SSE token flow (skipped if GGUF missing).
4) Consider a Pylon-side option to stream raw `token_id` and a decoded `token_text` that is guaranteed to align with tokenizer (helps debug gibberish output).
5) Add explicit UI controls for `model`, `layers`, and `moe_fallback` in the `/gptoss` page when in Pylon mode (optional, but useful for troubleshooting).

