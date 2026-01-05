# GPT-OSS Gibberish Output Debug Log

**Date:** 2025-01-04 15:09
**Issue:** GPT-OSS inference produces gibberish output

---

## Problem Statement

Input: "What is 2+2?"
Output: `fieldChart accounts l tokencrypt Land values·l...`

Even with greedy decoding (temp=0), output is deterministic garbage.

---

## What I've Verified as Correct

| Component | Status | Evidence |
|-----------|--------|----------|
| SwiGLU `+1.0` | ✅ Correct | Reference torch/model.py:256 shows `out_glu * (x_linear + 1)` |
| Sigmoid α=1.702 | ✅ Correct | Reference uses same constant |
| MXFP4 dequant table | ✅ Correct | Values match reference |
| mmap I/O | ✅ Working | Inference completes in reasonable time |
| No NaN/Inf | ✅ Verified | Debug logging shows no numeric errors |
| Tensor dimensions | ✅ Correct | token_embd [201088, 2880] = [vocab, hidden] |

---

## What I've Tried

### 1. Added Diagnostic Logging to Forward Pass
**File:** `crates/ml/src/gptoss_engine.rs`

Added logging for:
- Token embedding dimensions and first 5 values
- Hidden state after layer 0
- Output weight dimensions
- Top-5 logits with token IDs
- Sampled token at each generation step

### 2. Test Results Without Harmony Prompt

```
Input: "Hi" (no Harmony wrapper)
Token ID: 12194

Embedding: [0.219, 0.879, -0.439, 0.073, 0.293] len=2880 mean=0.018
After layer 0: [-0.153, 0.597, 0.587, 0.466, -0.654] mean=0.060
Logits: len=201088 min=-19.62 max=14.65

Top-5 tokens for each generation:
gen#0: " to", "bar", "brace", "lab", " lab"  <- NOT natural completions for "Hi"
gen#1: " wh", " pred", "ched", "�", "gu"    <- garbage
gen#2: "rey", "raad", "ogl", "aque", "rol"  <- garbage
gen#3: "og", " incon", "oru", "adar", " gross" <- garbage
gen#4: "rey", "ouche", " Damon", " incon", "ulle" <- garbage
```

### 3. Greedy vs Sampling

Both produce garbage. Greedy: " to whrey ogouche"

---

## Key Observations

1. **First token might be reasonable-ish** - " to" is at least a real word
2. **Top logits are garbage from the start** - Even the TOP candidates after "Hi" are " to", "bar", "brace" - not natural continuations
3. **Subsequent tokens are complete nonsense** - No coherent language pattern
4. **The issue is in the logits, not sampling** - Greedy (argmax) also produces garbage

---

## Current Hypotheses

### Most Likely
1. **Tensor layout transpose** - GGUF dims might be [cols, rows] not [rows, cols]
2. **Matmul direction wrong** - We might be computing W^T @ x instead of W @ x
3. **Embedding lookup wrong** - Might be reading column instead of row

### Less Likely
4. **RoPE position encoding bug** - Could cause attention to look at wrong positions
5. **KV cache bug** - Could cause attention to use wrong past context
6. **MoE routing bug** - Could route to wrong experts

---

## What To Do Next

1. **Compare with reference implementation** - Run Python gpt-oss on same input
2. **Verify first embedding values** - Check if token_id=12194 ("Hi") embedding matches reference
3. **Check matmul direction** - Verify W@x vs W^T@x
4. **Test single layer** - Isolate if issue is in attention or MLP

---

## Files Modified

- `crates/ml/src/gptoss_engine.rs` - Added debug logging
- `crates/ml/src/gptoss_native.rs` - Added mmap functions (earlier fix)

---

## Update: 2025-01-04 15:40

### What We Verified CORRECT

1. **Q8_0 Dequantization** ✅
   - Raw bytes: `3c32` (f16 scale ≈ 0.195), quants `fe fc 0a 00` = [-2, -4, 10, 0]
   - Decoded: [-0.39, -0.78, 1.95, 0] ≈ [-0.3896, -0.7793, 1.948, 0]
   - **MATCHES PERFECTLY!**

2. **Tensor Layout** ✅
   - dims = [201088, 2880] after `dims.reverse()` from GGUF
   - nbytes = 615,329,280 = 201088 * (2880/32) * 34 bytes
   - row_bytes = 3060 = (2880/32) * 34
   - This is correct: vocab_size rows of hidden_dim values

3. **Tokenizer** ✅
   - "Hi" encodes to 12194
   - Token 12194 decodes to "Hi"
   - Token 177564 decodes to " Jaz"
   - Token 316 decodes to " to"

4. **token_embd and output.weight are DIFFERENT** ✅
   - token_embd offset: 628349632
   - output.weight offset: 13008832
   - Not tied/shared

### layers=0 Test

With `--layers 0` (embedding -> norm -> lm_head only, NO transformer layers):
- Input: "Hi"
- Top-5 logits: [(177564, 21.95), (130183, 21.76), (149400, 21.47), ...]
- Top tokens: " Jaz", "pea", " DAR", " Had"
- **STILL GARBAGE** - but expected since no layers processed

### layers=24 (Full Model) Test

With full 24 layers:
- Input: "Hi"
- Top-5 logits: [(316, 14.65), (2990, 12.42), (102116, 11.97), ...]
- Top tokens: " to", "bar", "brace", "lab"
- Output: " to wh rey og ouche"
- **STILL GARBAGE**

### Math Prompt Test

Input: "1+1="
- Top-5 initial: [(6675, 14.53), (395, 13.34), ...]
- Output: " tail back Ua"
- **COMPLETE GARBAGE** - should output "2"

### Current Analysis

The issue is NOT in:
- Q8_0 dequantization (verified byte-by-byte)
- Tensor layout/dimensions (nbytes matches calculation)
- Tokenizer (round-trip works)
- Tensor offsets (embedding vs output.weight are different)

The issue MUST be in:
1. **Attention computation** - something wrong with Q/K/V or attention weights
2. **MoE routing** - wrong experts being selected
3. **RoPE** - position encoding might be corrupted
4. **Weight values themselves** - the GGUF file might be corrupt or incompatible

### Next Steps

1. Add detailed layer-by-layer debug
2. Compare Q/K/V values at layer 0 with reference
3. Check if attention weights sum to ~1
4. Verify RoPE frequencies
5. Try loading in llama.cpp to verify GGUF isn't corrupt

---

## Update: 2025-01-04 16:10 - MAJOR BUG FOUND!

### The Bug: GQA Head Mapping Was Wrong

**File:** `crates/ml/src/gptoss_native.rs` line 755

**Wrong code:**
```rust
let kv = h % kv_heads;  // WRONG - cycles through KV heads
```

**Correct code:**
```rust
let group_size = heads / kv_heads;  // 64/8 = 8 query heads per KV head
let kv = h / group_size;  // CORRECT - grouped, not cycling
```

For GQA with heads=64, kv_heads=8:
- **Wrong (cycling)**: head 0→kv0, head 1→kv1, head 2→kv2, ... head 8→kv0
- **Correct (grouped)**: heads 0-7→kv0, heads 8-15→kv1, heads 16-23→kv2, ...

The cycling mapping was scrambling attention for EVERY head, EVERY token!

### After Fix

**Prompt: "Hi"**
- BEFORE fix: " Jaz erver ac Couple Af" (complete gibberish)
- AFTER fix: " to of , ( " (real tokens, still not perfect)

**Top-5 after fix:**
- " to" (316), "," (11), "ly" (423), " (" (350), " and" (326)
- These are all real, sensible English tokens!

**Prompt: "The capital of France is"**
- AFTER fix: ", ( . ( (" (punctuation, not "Paris")
- Top-5: "," (11), "\n\n" (279), "." (13), "\n" (198), " (" (350)

### Status

The fix made a HUGE improvement - output went from random garbage tokens (" Jaz", "pea", " DAR") to real English tokens. However, the model still doesn't produce semantically correct output (should say "Paris" for France capital).

### Possible Remaining Issues

1. **MoE routing** - Expert selection might still be wrong
2. **RoPE position encoding** - Might have bugs in the YaRN scaling
3. **Attention sinks** - The sink mechanism might be misconfigured
4. **Model weights** - The GGUF might be incompatible or from wrong model version

### Next Steps

1. Check RoPE implementation for position=0 (should be identity)
2. Verify MoE expert selection
3. Try with Harmony prompt format enabled
4. Compare with reference Python implementation if available

---

## Update: 2025-01-04 16:45 - SECOND MAJOR BUG FOUND!

### The Bug: RoPE Pairing Was Wrong

**File:** `crates/ml/src/gptoss_native.rs` lines 685-718

The reference implementation pairs the FIRST HALF of head_dim with the SECOND HALF:
```python
# Reference (torch/model.py)
x1, x2 = torch.chunk(x, 2, dim=-1)  # x1 = x[..., :32], x2 = x[..., 32:]
o1 = x1 * cos - x2 * sin
o2 = x2 * cos + x1 * sin
```

**Wrong code (consecutive pairs):**
```rust
for i in (0..rope_dim).step_by(2) {
    let idx = base + i;      // 0, 2, 4, ...
    let idx2 = idx + 1;      // 1, 3, 5, ... WRONG!
    // ...
}
```

**Correct code (half-split pairs):**
```rust
let half_dim = rope_dim / 2;
for i in 0..half_dim {
    let idx1 = base + i;              // 0, 1, 2, ... 31
    let idx2 = base + half_dim + i;   // 32, 33, 34, ... 63
    // ...
}
```

For head_dim=64:
- **Wrong (consecutive)**: pairs (0,1), (2,3), (4,5), ...
- **Correct (half-split)**: pairs (0,32), (1,33), (2,34), ...

### Model Config Verified

```
heads=64 kv_heads=8 hidden=2880 ff=2880
rope_dim=64 rope_theta=150000 rope_scale=32 rope_ctx=4096
layers=24 experts=32x4
```

All values match reference implementation expectations.

### After RoPE Fix

**Prompt: "1+1=" (no Harmony)**
- Top-5: [(16, "1"), (13, "."), (17, "2"), (18, "3"), (220, " ")]
- **"2" is now in top-5!** (token 17, ranked #3)
- Output: ". ?\n\n ("

**Prompt: "Hi" (no Harmony)**
- Top-5: [(" to", 316), (",", 11), ("ly", 423), (" (", 350), (" and", 326)]
- Output: " to ,"

### Other Tests Performed

1. **Disable sinks** - No improvement, still punctuation-heavy
2. **Force expert 0 (--moe-fallback)** - No improvement
3. **Harmony prompt format** - Still punctuation output

### Current Analysis

The model is now producing:
- Real English tokens (not random garbage)
- Correct answers in top-5 for math ("2" is ranked #3 for "1+1=")
- But biased towards punctuation/function words

This suggests:
1. The core inference is now CORRECT
2. The model may need proper Harmony prompt formatting
3. Or there's still a subtle issue with attention/MoE weighting

### Remaining Hypotheses

1. **Attention sink values** - May be too strong, pulling attention away
2. **MoE weight combination** - Expert outputs may not be combined correctly
3. **Model calibration** - GPT-OSS is designed for tool-use chat format
4. **GGUF variant** - This 24-layer model may differ from reference 36-layer

---

## Summary of Bugs Fixed

| Bug | File | Line | Fix |
|-----|------|------|-----|
| GQA head mapping | gptoss_native.rs | 762 | `h % kv_heads` → `h / group_size` |
| RoPE pairing | gptoss_native.rs | 687-715 | Consecutive → half-split pairs |

---

## Commands Used

```bash
# Test without Harmony, layers=0
GPT_OSS_GGUF_PATH=crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
cargo run -p pylon --features gpt-oss-gguf -- infer \
  --prompt "Hi" --max-tokens 5 --no-harmony --temperature 0 --layers 0

# Test with full layers
GPT_OSS_GGUF_PATH=crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
cargo run -p pylon --features gpt-oss-gguf -- infer \
  --prompt "Hi" --max-tokens 5 --no-harmony --temperature 0

# Test with MoE fallback
GPT_OSS_GGUF_PATH=crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
cargo run -p pylon --features gpt-oss-gguf -- infer \
  --prompt "What is the capital of France?" --max-tokens 5 --temperature 0 --moe-fallback
```

---

## Update: 2025-01-04 17:35 - THIRD MAJOR BUG FOUND!

### The Bug: MXFP4 Dequant Layout + Scale Mismatch

GGML's MXFP4 layout is **NOT** interleaved. It stores low-nibble values in the first 16 slots,
high-nibble values in the second 16 slots of each 32-value block. It also uses E8M0 **half**
scales (`ggml_e8m0_to_fp32_half`, i.e. `exp2(scale_byte - 128)`).

**Reference (llama.cpp `ggml-quants.c`):**
```
for j in 0..15:
  y[j]     = kvalues_mxfp4[qs[j] & 0xF] * d
  y[j+16]  = kvalues_mxfp4[qs[j] >> 4] * d
```

**Our bug:**
- We interleaved low/high nibbles (abab...) instead of (aaaa...bbbb...).
- We used `2^(scale-127)` instead of GGML's `2^(scale-128)` (half-scale E8M0).

This permuted **every MXFP4 block** and mis-scaled all MoE weights.

### Fix Applied

- Updated MXFP4 table to GGML doubled values: `0, 1, 2, 3, 4, 6, 8, 12, ...`
- Corrected layout: low nibble → indices `0..15`, high nibble → `16..31`
- Scale uses GGML E8M0 half conversion (`exp2(scale_byte - 128)` / `ggml_e8m0_to_fp32_half`)
- Patched **both native + web** CPU and GPU paths:
  - `crates/ml/src/gptoss_native.rs`
  - `crates/web/client/src/gptoss_runtime.rs`
  - `crates/web/client/src/shaders/mxfp4` (inline WGSL)
- Also aligned GQA head→KV mapping in web CPU/GPU attention + debug head weights

### Results (Native, No Harmony)

```
Prompt: "1+1="
Top-5: "2", "3", "4", "0", "1"
Greedy output: "2"
```

```
Prompt: "Hi"
Greedy output: ", I am a "
```

This is the first time the model reliably outputs the **correct first token**.

### Notes

- `pylon infer` build currently fails in `crates/compute` due to `TraceEvent` field mismatches.

---

## Update: 2026-01-04 18:55 - Harmony Prompt + Telemetry Speedups

### Prompt Formatting Fix

- Updated Harmony prompt to match `openai-harmony` output:
  - System + user only (no empty developer block)
  - `# Valid channels: analysis, commentary, final` only
  - Assistant start tag is now `<|start|>assistant` (no `<|channel|>final<|message|>`)
- Removed the tools channel line when no tools are present
- `CURRENT_DATE` updated to `2026-01-04` (native + web)

### Stop Tokens

- Added `<|end|>` to native stop tokens (matches web + Harmony defaults).

### Performance Cleanup

- Removed per-token debug top-5 sorting (was O(vocab log vocab)).
- Rewrote `top_k_from_logits` to maintain a running top-k without sorting the full vocab.
- Telemetry (top-k/entropy) now computed only when a hook/callback is active.

### Results

**Harmony prompt prefill (CPU)**
- Prompt length now 71 tokens (was 98).
- Still too slow on CPU: `gptoss_cli` timed out after 5 minutes (stuck at prefill token ~40/71).
- Needs GPU or a much longer timeout to fully validate Harmony output.

**No-harmony sanity checks**
- `1+1=` → output `2\n\n- ` (top-1 token is `2`).
- `Hi` → output `, I am a `.

### Commands Used

```bash
cargo run -p ml --bin gptoss_cli -- --gguf crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf --prompt "1+1=" --max-tokens 4 --no-harmony
cargo run -p ml --bin gptoss_cli -- --gguf crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf --prompt "Hi" --max-tokens 5 --no-harmony
# Timed out (Harmony prompt, CPU)
cargo run -p ml --bin gptoss_cli -- --gguf crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf --prompt "1+1=" --max-tokens 1
```

---

## Update: 2026-01-04 19:30 - GPU (Metal) Bring-up

### Metal Backend Status

- GPT-OSS Metal backend builds cleanly when pointed at the local `gpt-oss` metal build.
- `pylon infer` now prefers `gpt-oss-metal` if `GPT_OSS_METAL_MODEL_PATH` is set (before GGUF).

### Remaining Blocker

- No Metal `model.bin` on disk yet. The backend requires the pre-converted Metal weights.
- The OpenAI repo provides `hf download openai/gpt-oss-20b --include "metal/*"` or
  `python gpt_oss/metal/scripts/create-local-model.py` from safetensors.

### Commands Used

```bash
# Build check with Metal libs
GPT_OSS_METAL_DIR=~/code/gpt-oss/gpt_oss/metal/build \
  cargo check -p compute --features gpt-oss-metal
```

### Next Steps

1. Download or build `model.bin` for gpt-oss-20b/120b.
2. Export `GPT_OSS_METAL_DIR` + `GPT_OSS_METAL_MODEL_PATH`.
3. Run:
   `cargo run -p pylon --features gpt-oss-metal -- infer --prompt "Hi" --max-tokens 16`

---

## Update: 2026-01-04 20:40 - GPU (Metal) Working on 20b

### What Was Fixed

1. **Metallib embedding**
   - Added `crates/pylon/build.rs` to embed `default.metallib` into the binary.
   - Confirmed `__METAL/__shaders` section exists in `target/debug/pylon`.

2. **Harmony cache**
   - `openai-harmony` downloads `o200k_base.tiktoken` on first use.
   - Doing that inside tokio caused a runtime panic.
   - Pre-cached the file in `TIKTOKEN_RS_CACHE_DIR=~/.cache/tiktoken-rs`.

3. **GPU memory**
   - Default context length was **131072**, allocating a ~12GB KV cache.
   - Set `GPT_OSS_METAL_CONTEXT_LENGTH=8192` to fit GPU memory.

### GPU Run (Metal, gpt-oss-20b)

```bash
TIKTOKEN_RS_CACHE_DIR=~/.cache/tiktoken-rs \
GPT_OSS_METAL_DIR=~/code/gpt-oss/gpt_oss/metal/build \
GPT_OSS_METAL_MODEL_PATH=~/models/gpt-oss-20b/metal/model.bin \
GPT_OSS_METAL_MODEL_ID=gpt-oss-20b \
GPT_OSS_METAL_CONTEXT_LENGTH=8192 \
GPT_OSS_METAL_MAX_BATCH_TOKENS=128 \
cargo run -p pylon --features gpt-oss-metal -- infer --prompt "1+1=" --max-tokens 4 --temperature 0
```

Output (first tokens):
```
<|channel|>analysis<|message|>The
```

Notes:
- The Metal backend is now selected and running.
- Output is **raw Harmony** (channel tags), so `pylon infer` shows the tags.
- Lowering `GPT_OSS_METAL_MAX_BATCH_TOKENS` below prompt length triggers assert.

### Errors Seen (Resolved)

- `failed to create Metal default library` → fixed by embedding `default.metallib`.
- `Cannot drop a runtime in a context where blocking is not allowed` → fixed by pre-caching tiktoken.
- `failed to create Metal buffer of size 12884901888` → fixed by limiting context length.

---

## Update: 2026-01-04 21:25 - Defaults + Logging + Runtime Warning

### Defaults (no long env string)

- Default model path: `~/models/gpt-oss-20b/metal/model.bin` (falls back to `~/models/gpt-oss-120b/metal/model.bin`)
- Model ID inferred from path (e.g., `gpt-oss-20b`)
- Default context length: `8192` (clamped to model max)
- Default max batch tokens: `128` (auto-bumped to prompt length)
- Default tiktoken cache dir: `~/.cache/tiktoken-rs` (auto-created)

### Logging

- `gpt_oss_metal` now logs engine init + inference start/finish + token counts.
- Pylon default log filter includes `gpt_oss_metal=info`, so logs show without `RUST_LOG`.

### CLI Selection

- `pylon infer` now prefers `gpt-oss-metal` if detected, even without `GPT_OSS_METAL_MODEL_PATH`.

### Metal Build Defaults

- `crates/pylon/build.rs` now falls back to `~/code/gpt-oss/gpt_oss/metal/build` for `default.metallib`.

### Runtime Warning (mlock)

- Warning: `mlock(... model.bin ...) failed with error 35` came from the metal C library.
- Patched `~/code/gpt-oss/gpt_oss/metal/source/model.c` to make mlock opt-in via `GPT_OSS_METAL_MLOCK=1`.
- Rebuilt `libgptoss.a` after patch.

### Minimal Command (now works with defaults)

```bash
cargo run -p pylon --features gpt-oss-metal -- infer --prompt "1+1=" --max-tokens 40 --temperature 0
```

Notes:
- Requires the Metal `model.bin` at the default path above (or set `GPT_OSS_METAL_MODEL_PATH`).
- Ensure `o200k_base.tiktoken` is present in `~/.cache/tiktoken-rs` to avoid Harmony download at runtime.

---

## Update: 2026-01-05 02:42 - Default Runs (Metal)

### Run: max_tokens=4

```bash
cargo run -p pylon --features gpt-oss-metal -- infer --prompt "1+1=" --max-tokens 4 --temperature 0
```

Logs (excerpt):
- `Defaulted TIKTOKEN_RS_CACHE_DIR=/Users/christopherdavid/.cache/tiktoken-rs`
- `GPT-OSS Metal engine initialized` (model_id `gpt-oss-20b`, context_length `8192`)
- `GPT-OSS Metal inference started` (prompt_tokens `60`, max_tokens `4`, max_batch_tokens `128`)

Output (partial before timeout):
```
<|channel|>analysis
```

Result: command timed out after ~124s (CLI timeout), inference still running.

### Run: max_tokens=40

```bash
cargo run -p pylon --features gpt-oss-metal -- infer --prompt "1+1=" --max-tokens 40 --temperature 0
```

Logs (excerpt):
- `GPT-OSS Metal inference started` (prompt_tokens `60`, max_tokens `40`)

Output (partial before timeout):
```
<|channel|>analysis<|message|>The user
```

Result: command timed out after ~304s (CLI timeout), inference still running.

---

## Update: 2026-01-05 03:40 - Speed Investigation (Metal)

### Code Changes (OpenAgents)

- `gpt-oss-metal` now respects `extra.harmony` (so `--no-harmony` works on Metal).
- Added chunked sampling (`GPT_OSS_METAL_SAMPLE_CHUNK_TOKENS`, default 8).
- Logs now show time-to-first-token + tokens/sec throughput.
- Metal build defaults now prefer `~/code/gpt-oss/gpt_oss/metal/build-release` when present.

### gpt-oss Metal Build Change (external repo)

- Patched `~/code/gpt-oss/gpt_oss/metal/source/model.c`:
  - Added `GPT_OSS_METAL_MAX_THREADGROUPS` override.
  - Ensures `max_threadgroups >= 64`.
  - Logs GPU cores + chosen max_threadgroups.
- Rebuilt release libs in `~/code/gpt-oss/gpt_oss/metal/build-release`.

### Measured Throughput (Metal, no-harmony)

```bash
cargo run -p pylon --features gpt-oss-metal -- infer \
  --prompt "1+1=" --max-tokens 1 --temperature 0 --no-harmony
```

Observed:
- prompt tokens: 4
- time to first token: ~12–19s
- throughput: ~0.05–0.08 tokens/sec
 - log shows GPU cores detected: 19, max_threadgroups defaulted to 64

```bash
cargo run -p pylon --features gpt-oss-metal -- infer \
  --prompt "1+1=" --max-tokens 2 --temperature 0 --no-harmony
```

Observed:
- time to first token: ~12.7s
- total time: ~66s (≈0.03 tokens/sec)

### Conclusion (Current Bottleneck)

Single-token generation on this machine is **extremely slow** (tens of seconds per token),
even after:
- release metal build,
- no-harmony prompt,
- increased max threadgroups.

This points to the Metal kernels for small batch sizes + 20B model size as the dominant bottleneck.

### Next Ideas (if needed)

- Try `GPT_OSS_METAL_MAX_THREADGROUPS=256` (may help unembedding parallelism).
- Use `GPT_OSS_METAL_MLOCK=1` if you can raise `ulimit -l` to keep model pages resident.
- If speed is the priority, consider a smaller model or a quantized backend for interactive runs.

### Pending

1. Parse Harmony output for `pylon infer` (strip channel tags, show final content).
2. Tune speed/latency (GPU load time still heavy on first run).

---

## Update: 2026-01-04 21:20 - mlock Warning Fixed

### Root Cause

`libgptoss.a` always calls `mlock()` on the 13GB model mapping.
On macOS, this fails with error 35 (resource unavailable) under default memlock limits,
but it is **not** fatal.

### Fix Applied

- Patched the local `gpt-oss` metal source to only call `mlock()` when
  `GPT_OSS_METAL_MLOCK` is explicitly set (otherwise skip silently).
- Updated `crates/gpt-oss-metal/build.rs` to rerun when `libgptoss.a`,
  `libmetal-kernels.a`, or `default.metallib` change.

### Verified

Rebuilt `libgptoss.a` and reran Metal inference: warning is gone.
  Used `gptoss_cli` for testing instead.
- Harmony prompt prefill is slow on CPU (98 tokens). For now, testing done with `--no-harmony`.
  Need a longer run (or GPU) to verify full Harmony responses.

---

## Update: 2026-01-04 22:09 - Speed Battleplan (Sprint)

### Goal (Tonight)

- Reduce **time-to-first-token** and **tokens/sec** for Metal on gpt-oss-20b by at least **5–10x**.
- Get interactive feedback: TTFT under ~2s for short prompts and >=1 tok/sec steady-state.

### Baseline Commands (Use These Every Time)

```
cargo run -p pylon --features gpt-oss-metal -- infer --prompt "1+1=" --max-tokens 1 --temperature 0 --no-harmony
cargo run -p pylon --features gpt-oss-metal -- infer --prompt "1+1=" --max-tokens 2 --temperature 0 --no-harmony
```

Record:
- prompt tokens
- time-to-first-token
- sample chunk duration
- tokens/sec
- first-run vs second-run delta (pipeline compile vs steady-state)

### Phase 0 - Baseline + Sanity (10–15 min)

- Run baseline commands twice (fresh + warmed).
- Confirm model path, build-release libs, and `model.bin` size.
- Validate that prompt length is minimal (`--no-harmony` for perf triage).

### Phase 1 - Instrumentation (Now)

Add per-step timings so we can see where time goes:
- model load time
- context creation time
- prompt append time
- per-sample chunk time (tokens + ms)
- first-sample time (prefill + first decode)

What to look for:
- Is the time dominated by **append** (prefill) or **sample** (decode)?
- Is the **first sample** uniquely slow (pipeline compile)?
- Does sample time scale with `sample_chunk_tokens`?

### Phase 2 - Diagnose Hotspot (After Phase 1)

Based on timings:
- **If first sample is huge:** add a warmup path to build pipelines before the real prompt.
- **If append dominates:** reduce prompt length, ensure Harmony prompt is only used when needed.
- **If decode dominates:** tune Metal kernel params (threadgroups, simdgroup size, buffer storage).

### Phase 3 - Kernel/Memory Tuning (Metal repo)

Targets:
- Increase parallelism via `max_threadgroups` and threadgroup sizing.
- Use `storageModePrivate` for large GPU buffers where possible.
- Reduce CPU↔GPU sync points and avoid per-token allocs.
- Optional: `GPT_OSS_METAL_MLOCK=1` if memlock limits allow (cuts page faults).

### Phase 4 - Aggressive Speedups (If Still Too Slow)

- Implement prefill/decoding split and speculative decoding (draft model).
- Reduce model size for interactive runs (7B/4B) for guaranteed wins.
- Consider alternative Metal path (MPSGraph for matmul/attention).

### Phase 5 - Validate + Lock In

- Rerun baseline commands.
- Capture before/after metrics in this log.
- Add a short "known-good" command with defaults for future checks.

---

## Update: 2026-01-05 04:15 - Phase 1 Instrumentation + Baseline

### Phase 1 Changes (OpenAgents)

- Added timing logs for:
  - model load
  - context creation
  - prompt append
  - per-sample chunk (tokens + ms, prefill vs decode)

### Baseline Run (cold-ish)

```bash
cargo run -p pylon --features gpt-oss-metal -- infer \
  --prompt "1+1=" --max-tokens 1 --temperature 0 --no-harmony
```

Logs (excerpt):
- model load: **9490ms**
- context create: **0ms**
- prompt append: **0ms**
- sample chunk (prefill+decode, 1 tok): **13300ms**
- time to first token: **13309ms**
- throughput: **0.075 tok/sec**
- output: `2`

### Baseline Run (warm)

Same command immediately after:
- model load: **7020ms**
- sample chunk (prefill+decode, 1 tok): **10987ms**
- time to first token: **10994ms**
- throughput: **0.091 tok/sec**
- output: `2`

### Interpretation

- The entire latency is inside `gptoss_context_sample` (prefill+decode).
- Context creation + append are negligible.
- Warm run is only ~15–20% faster, so this is **not just pipeline compile**.
- Phase 2 should focus on Metal kernel time or per-token compute cost.

---

## Update: 2026-01-05 04:25 - Phase 2 Bench Harness + Threadgroup Tests

### Bench Harness (OpenAgents)

`pylon` builds are currently blocked by unrelated `rlm` compile errors, so I added a tiny
Metal-only bench binary to isolate the backend:

- New bin: `crates/gpt-oss-metal/src/bin/gpt_oss_metal_bench.rs`
- Added `tracing-subscriber` to print `gpt_oss_metal` logs.
- Requires `--features link` to link the Metal libs.

Example:

```bash
cargo run -p gpt-oss-metal --features link --bin gpt_oss_metal_bench -- \
  --prompt "1+1=" --max-tokens 1 --temperature 0 --no-harmony
```

### Threadgroup Sweep (no-harmony, 1 token)

All runs use the same prompt and defaults, only `GPT_OSS_METAL_MAX_THREADGROUPS` changed.

**Default (64):**
- sample chunk: **~13.97s**
- TTFT: **~13.98s**
- throughput: **~0.071 tok/sec**

**256:**
- sample chunk: **~13.56s**
- TTFT: **~13.57s**
- throughput: **~0.074 tok/sec**

**512:**
- sample chunk: **~13.43s**
- TTFT: **~13.43s**
- throughput: **~0.074 tok/sec**

### Interpretation

- Increasing `max_threadgroups` barely changes TTFT.
- The bottleneck is not unembedding parallelism alone.
- Next step: instrument GPU command buffer time in the Metal library itself and inspect kernel-level costs.

---

## Update: 2026-01-05 04:35 - GPU Timing + Dense Kernel Forcing

### GPU Timing Instrumentation (gpt-oss repo, external)

- Added `GPT_OSS_METAL_TIMING=1` switch to log GPU time from `MTLCommandBuffer`.
- Logs now show GPU time for `gptoss_context_sample` (and prefill when used).

Result (no-harmony, 1 token):
- `Metal sample GPU time` ≈ **11.6s**
- Total sample chunk ≈ **13.5s**

Conclusion: the GPU is doing ~11s of work per token. This is *real* GPU time, not CPU overhead.

### Force Dense Kernels (external experiment)

- Set `min_tokens_for_dense_matmul_kernels = 1`
- Set `min_tokens_for_dense_moe_kernels = 1`

Result (no-harmony, 1 token):
- GPU time ≈ **9.9s**
- Total sample chunk ≈ **11.9s**

This is only ~15–20% faster. Still far from interactive.

### Metallib -O3 (external experiment)

- Rebuilt Metal shaders with `xcrun metal -O3 -g ...`
- No meaningful improvement (GPU time stayed ~10–11s).

### Current Read

- The custom Metal kernels are the dominant bottleneck.
- We need either:
  - a much faster kernel path (MPS/GEMM or heavily tuned Metal kernels), or
  - a smaller model for interactive performance.

---

## Update: 2026-01-05 05:00 - Layer/Threadgroup Experiments

### Env Overrides (gpt-oss repo, external)

Added env overrides for quick tuning:
- `GPT_OSS_METAL_NUM_BLOCKS` (limit layers)
- `GPT_OSS_METAL_*_TG` for threadgroup sizes (embeddings, attn qkv/out, mlp gate/swiglu/out/acc, unembedding)

### Layer Count Scaling (no-harmony, 1 token)

Full model (24 blocks):
- GPU time ≈ **9.9–11.6s**
- TTFT ≈ **12–13.5s**
- Output: `2`

12 blocks:
- GPU time ≈ **4.5s**
- TTFT ≈ **6.1s**
- Output: `!` (quality degraded)

16 blocks:
- GPU time ≈ **4.35s**
- TTFT ≈ **6.3s**
- Output: `!`

20 blocks:
- GPU time ≈ **6.8s**
- TTFT ≈ **8.7s**
- Output: `!`

8 blocks:
- GPU time ≈ **1.8s**
- TTFT ≈ **3.3s**
- Output: `!`

4 blocks:
- GPU time ≈ **1.0s**
- TTFT ≈ **2.6s**
- Output: `!`

Takeaway: speed scales with layer count, but quality collapses before we reach sub-2s TTFT.

### Threadgroup Overrides (full model)

Unembedding TG:
- `GPT_OSS_METAL_UNEMBED_TG=1024` → **slower** (GPU ~11.9s)
- `GPT_OSS_METAL_UNEMBED_TG=256` → **slower** (GPU ~11.2s)

QKV TG:
- `GPT_OSS_METAL_ATTN_QKV_TG=512` → **small improvement** (GPU ~10.5s vs ~11s)
  - TTFT ≈ **12.5s**

Takeaway: simple TG tuning doesn’t provide multi‑x gains.

### Current Conclusion

We need a **fundamental kernel speedup** (MPS/Metal matmul rewrite or deep kernel tuning) to hit the interactive target. Tweaking thresholds and threadgroups isn’t enough.

---

## Update: 2026-01-05 05:10 - MoE Top‑1 Experiment

### What Changed (gpt-oss repo, external)

- Added optional `GPT_OSS_METAL_NUM_ACTIVE_EXPERTS=1` to force top‑1 experts.
- Added k1 kernels for topk + scatter/gather/accumulate.

### Result (full model, no-harmony, 1 token)

```
GPT_OSS_METAL_NUM_ACTIVE_EXPERTS=1 GPT_OSS_METAL_TIMING=1 ...
```

- GPU time ≈ **11.8s**
- TTFT ≈ **13.9s**
- Output: `2`

### Takeaway

Reducing active experts **did not improve** latency. The dominant cost appears to be attention/QKV, not MoE.
