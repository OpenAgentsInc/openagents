# Plan: GPT-OSS GGUF in Pylon (cross-platform)

Goal: Run GPT-OSS from a local GGUF file via Pylon on macOS and Linux (no Metal required). The browser UI should be able to connect to Pylon and stream tokens + telemetry.

Non-goals:
- Replacing the browser WebGPU path.
- Metal-only path (can be added later).
- Windows support (optional later).

## Current state
- GPT-OSS GGUF runtime exists as a native CPU path in `crates/ml/src/gptoss_native.rs`.
- CLI runner exists in `crates/ml/src/bin/gptoss_cli.rs`.
- Pylon has a GPT-OSS Metal backend (`gpt-oss-metal`) only.
- The `ml` provider uses Candle and only supports Llama2C + Gemma3.
- No GGUF GPT-OSS backend is registered in Pylon.

## Phase 1: Promote GPT-OSS GGUF runtime to a reusable engine (ml)
Goal: A reusable API that can run GPT-OSS GGUF with streaming tokens and optional telemetry.

Tasks:
- Extract the CLI logic into a library module, e.g. `crates/ml/src/gptoss_engine.rs`.
- Create `GptOssEngine::load(path)`:
  - Parse GGUF once (index + metadata).
  - Keep a file handle open.
  - Add a light cache for:
    - Q8_0 rows (token embeddings).
    - F32 tensors (norm weights, biases).
    - MXFP4 expert blocks.
- Implement `generate(prompt, GenerationConfig, on_token, on_telemetry)`:
  - Reuse existing Harmony prompt builder + tokenizer.
  - Support top_k/top_p/temp/max_new.
  - Optional `moe_fallback` to skip experts if needed.
- Add a small telemetry hook struct:
  - Stages (load, prefill, decode).
  - Token timing.
  - Cache stats.

Notes:
- Keep the engine CPU-only first. Add optional wgpu acceleration later.
- Avoid any new network calls in this layer (use local GGUF).

## Phase 2: Compute backend for Pylon (gpt-oss-gguf)
Goal: Pylon can list + run GPT-OSS GGUF as an inference backend.

Tasks:
- Add optional dependency `ml` in `crates/compute` behind feature `gpt-oss-gguf`.
- Implement `crates/compute/src/backends/gpt_oss_gguf.rs`:
  - `GptOssGgufBackend::from_env()` loads a GGUF path and model id.
  - Implement `list_models`, `complete`, `complete_stream`.
  - Use the `GptOssEngine` from Phase 1.
- Register in `crates/compute/src/backends/mod.rs` when the feature is enabled.
- Expose in Pylon build features (similar to `gpt-oss-metal`).

Env config (proposed):
- `GPT_OSS_GGUF_PATH=/path/to/gpt-oss-20b-Q8_0.gguf`
- `GPT_OSS_GGUF_MODEL_ID=gpt-oss-20b`
- Optional:
  - `GPT_OSS_GGUF_CONTEXT_LENGTH`
  - `GPT_OSS_GGUF_MAX_TOKENS`
  - `GPT_OSS_GGUF_MOE_FALLBACK=1`
  - `GPT_OSS_GGUF_THREADS`

Docs:
- Update `crates/pylon/docs/CONFIGURATION.md` with the above.
- Add a short `pylon` quickstart for GGUF.

## Phase 3: Pylon tunnel + /gptoss UI
Goal: /gptoss can run via Pylon and show the same telemetry stream.

Tasks:
- Define a tunnel message schema for GPT-OSS:
  - request: `{prompt, model, max_tokens, sampling...}`
  - stream: `{delta, token_id, top_k, entropy, stage, stats...}`
  - errors: `{message, stage}`
- Add a Pylon tunnel handler that:
  - Accepts GPT-OSS requests.
  - Streams tokens and telemetry from the GGUF engine.
- Update `/gptoss` UI:
  - Add "Use Pylon" toggle.
  - If connected, send request over tunnel instead of local WebGPU.
  - Show telemetry from Pylon in the same panels.

## Phase 4: Validation and gates
Goal: Proof that GGUF works on Linux and macOS via Pylon.

Checks:
- `cargo run -p pylon --features gpt-oss-gguf -- start -f`
- `pylon compute list` shows `gpt-oss-20b`.
- A streaming run returns readable text (not gibberish).
- `/gptoss` shows live telemetry when connected via Pylon.

Tests:
- Add a smoke test in `crates/ml/src/tests/` that runs if a GGUF is present.
- Keep it optional (skip if file missing) to avoid giant artifacts in git.

## Open questions
- Do we want a pure-CPU path only, or add wgpu acceleration on native next?
- Do we need to support more GGML quant types beyond Q8_0 + MXFP4?
- Should we allow per-layer fallbacks when expert tensors are missing or unsupported?
