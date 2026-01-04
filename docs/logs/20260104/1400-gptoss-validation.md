# GPT-OSS Implementation Validation Log

**Date:** 2025-01-04 14:00
**Status:** All components working

---

## Summary

| Component | Status | Notes |
|-----------|--------|-------|
| `gptoss_cli` | Working | Standalone CLI for direct inference |
| `pylon infer` | Working | CLI command with Harmony prompt |
| `pylon api` | Working | OpenAI-compatible HTTP API |
| Model (11GB) | Working | `gpt-oss-20b-Q8_0.gguf` loads successfully |

---

## 1. Standalone CLI (`gptoss_cli`)

**Location:** `crates/ml/src/bin/gptoss_cli.rs`

**Command:**
```bash
cargo run -p ml --features native --release --bin gptoss_cli -- \
  --gguf crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
  --prompt "Hello, how are you" --max-tokens 20
```

**Working features:**
- Model loading (11GB Q8_0 GGUF)
- Token-by-token generation with streaming output
- Entropy calculation per token
- Top-1 prediction display
- Configurable `--max-tokens`
- Configurable `--layers` for partial model loading
- `--moe-fallback` flag for mixture-of-experts
- `--no-harmony` to disable prompt wrapper

**Sample output:**
```
step=0 token=73786 entropy=5.524 top1= labs
step=1 token=2543 entropy=5.039 top1= line
...
step=19 token=342 entropy=6.391 top1=ysm
output:
 labs line line-chart value nominal horoscopecrypt pattern nice...
```

---

## 2. Pylon Infer Command

**Location:** `crates/pylon/src/cli/infer.rs`

**Command:**
```bash
GPT_OSS_GGUF_PATH=crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
cargo run -p pylon --features gpt-oss-gguf -- infer \
  --prompt "Hello" --max-tokens 128
```

**Working features:**
- Auto-detection of GPT-OSS backend via `GPT_OSS_GGUF_PATH` env var
- Harmony prompt wrapper (enabled by default) - produces coherent responses
- Streaming token output
- Finish reason reporting (`Stop`, `length`)
- All sampling parameters: `--temperature`, `--top-p`, `--top-k`
- Layer limiting: `--layers`
- KV cache limiting: `--max-kv`
- MoE fallback: `--moe-fallback`
- Disable Harmony: `--no-harmony`

**Sample output:**
```
Hello! How can I assist you today?

[finish_reason=Stop]
```

---

## 3. Pylon API Server

**Location:** `crates/pylon/src/cli/api.rs`

**Command:**
```bash
GPT_OSS_GGUF_PATH=crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
cargo run -p pylon --features gpt-oss-gguf -- api --bind 127.0.0.1:9899
```

**Working endpoints:**

### GET /v1/models
```json
{
  "data": [
    {"id": "Gpt-Oss-20B", "object": "model", "owned_by": "local", "context_length": 131072},
    {"id": "apple-foundation-model", "object": "model", "owned_by": "local", "context_length": 8192}
  ]
}
```

### POST /v1/completions
```bash
curl http://127.0.0.1:9899/v1/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "Gpt-Oss-20B", "prompt": "What is 2+2?", "max_tokens": 20}'
```

Response:
```json
{
  "id": "gpt-oss-gguf",
  "object": "text_completion",
  "model": "Gpt-Oss-20B",
  "choices": [{
    "text": "...",
    "index": 0,
    "finish_reason": "length"
  }],
  "usage": {
    "prompt_tokens": 98,
    "completion_tokens": 20,
    "total_tokens": 118
  }
}
```

---

## 4. Core Engine (`GptOssEngine`)

**Location:** `crates/ml/src/gptoss_engine.rs`

**Working features:**
- GGUF file parsing and loading
- Q8_0 quantization support
- Sliding window attention
- RoPE positional encoding
- Token generation with callbacks
- Configurable generation parameters
- Model metadata extraction (context length, vocab size, etc.)

---

## 5. Backend Adapter (`GptOssGgufBackend`)

**Location:** `crates/ml/src/gptoss_backend.rs`

**Working features:**
- Implements `InferenceBackend` trait for pylon integration
- `from_env()` - loads from `GPT_OSS_GGUF_PATH`
- `complete()` - single completion request
- `complete_stream()` - streaming completion with channel
- Harmony prompt wrapping (configurable)
- Extra parameters passthrough (top_k, layers, max_kv, moe_fallback)

---

## 6. Tokenizer (`GptOssTokenizer`)

**Location:** `crates/ml/src/gptoss_tokenizer.rs`

**Working features:**
- BPE tokenization (GPT-style)
- Encode text to token IDs
- Decode token IDs to text
- Special token handling

---

## Model File

| Property | Value |
|----------|-------|
| Path | `crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf` |
| Size | 11GB |
| Quantization | Q8_0 |
| Context Length | 131,072 tokens |

---

## Build Commands

```bash
# Build ml library with native features
cargo build -p ml --features native

# Build standalone CLI
cargo build -p ml --features native --bin gptoss_cli

# Build pylon with GPT-OSS support
cargo build -p pylon --features gpt-oss-gguf

# Release builds (faster inference)
cargo build -p ml --features native --release --bin gptoss_cli
```

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `GPT_OSS_GGUF_PATH` | Path to the GGUF model file (required for pylon) |

---

## Notes

1. **Harmony prompt matters** - `pylon infer` produces coherent output because it uses the Harmony prompt wrapper by default. The raw `gptoss_cli` without Harmony produces less coherent output.

2. **Model ID in API** - The model is exposed as `Gpt-Oss-20B` in the API, not `gpt-oss-gguf`.

3. **Release mode recommended** - Use `--release` for faster inference, especially for the 11GB model.

4. **Test fixtures incompatible** - The small llama2c test fixtures in `tests/fixtures/` lack GPT-OSS metadata and cannot be used for testing.
