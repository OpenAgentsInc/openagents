# GPT-OSS Browser Plan (Tonight Slice)

## Intent

Ship a feasibility vertical slice that proves the **browser + wgpu inference spine** works:

`GGUF parse → range fetch → GPU buffer → Q8_0 dequant + matmul → CPU readback compare`.

If this passes, everything else is "just more kernels."

---

## Progress (2026-01-02)

### Gate A — GGUF Index (complete)

Implemented a local GGUF parser + tensor dump tool and validated against the
downloaded GPT-OSS GGUF.

Command:

```bash
cargo run -p ml --no-default-features --features native --bin gguf_dump -- \
  crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf --limit 20
```

Observed:
- `version: 3`
- `tensor_data_offset: 13008832`
- `tensor_count: 459`
- Q8_0 tensors present (e.g., `output.weight`, `token_embd.weight`)
- Unknown ggml type `39` appears for expert weights (still indexed cleanly)

### Gate B — Deterministic Range Reads (complete)

New `gguf_range` tool supports hashed range reads with optional tensor lookup.

Command:

```bash
cargo run -p ml --no-default-features --features native --bin gguf_range -- \
  crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf \
  --tensor output.weight --len 1048576 --repeat 2
```

Observed:
- `sha256: ff6dcca8ec6f88daa59b9a8d6c583e288e0a5a182d86556712c48b820b519352`
- `consistent: true` across 2 reads

### Gate C — GPU Compute (complete)

New `gguf_gate_c` tool runs a tiny Q8_0 matmul on GPU via wgpu and compares
against a CPU reference for the same slice.

Command:

```bash
cargo run -p ml --no-default-features --features native,wgpu --bin gguf_gate_c -- \
  crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf --tensor output.weight --k 128 --n 64
```

Observed:
- `max_abs_diff: 9.313226e-10`
- `mean_abs_diff: 3.012701e-10`
- Q8_0 dequant + matmul runs end-to-end on GPU with CPU match within tolerance

### Gate D — Correctness (complete)

Added a Gate D test that runs the same Q8_0 slice and asserts CPU/GPU match
within tolerance.

Command:

```bash
cargo test -p ml --no-default-features --features native,wgpu gguf_gate_d
```

Observed:
- CPU vs GPU diff remains below `0.01` tolerance for `output.weight` (K=128, N=64)
- Test run passes locally (`1 passed`)

### Browser Wiring — Gate C/D (in progress)

Started wiring Gate C/D into the `/ml-inference` WebGPU page. The browser runtime
now accepts query params and runs the same Q8_0 slice via WebGPU, then compares
against CPU reference.

Run (requires a Range-capable GGUF URL):

```
/ml-inference?gguf=<URL>&tensor=output.weight&k=128&n=64&tolerance=0.01
```

Notes:
- Browser fetches GGUF metadata (first ~16MB) and then range-fetches the Q8_0 slice.
- Gate status + diff stats render in the ML Inference HUD.
- `cargo check --target wasm32-unknown-unknown` passes for `openagents-web-client` (1 pre-existing warning about `token_id` unused).

### Phase 2 Kickoff — Shared GPU + Runtime Scaffold (in progress)

- Added a shared **GPU context** to the web app state (reuse WGPUI device/queue).
- Extracted a reusable **GGUF web parser + range fetch** module.
- GGUF parser now captures **tokenizer metadata** (tokens, token_types, merges, chat template).
- Added a browser-side **CoreBPE tokenizer** wired to GGUF vocab (O200k harmony pattern).
 - `/gptoss` now emits **tokenizer_load** + **prompt_encode** telemetry from real GGUF data.
- Added an **MXFP4 probe** (expert slice) with CPU/GPU matmul compare for ggml type 39.
- Added a **GPU limits probe** (max storage/buffer sizes, bind group caps) to catch WebGPU limit issues early.
- Parsed **model config** from GGUF metadata (blocks, heads, rope, experts) and emit in telemetry.
- Added a **chunked Q8_0 logits probe** for `output.weight` with top-k token telemetry.
- Added **RoPE application** in the block-0 probe (config-driven).
- Added **MoE router top-k** (using `ffn_gate_inp` weights + bias) with telemetry of expert picks.
- Added **MoE MLP compute** for selected experts (MXFP4 gate/up/down + swiglu + weighted sum).
- Added **single-token attention** with sink weights (sdpa for seq_len=1).
- Added a `gptoss_runtime` scaffold to start centralizing browser runtime logic.
 - Wired `/gptoss` start button handling + runtime entrypoint for streaming load telemetry.
 - `/gptoss` now streams **real GGUF bytes** in chunks on click (with progress + tensor scan events).
 - Added a local **GGUF range server** (`gguf_serve`) to stream the on-disk model with HTTP Range.
 - `gguf_serve` now responds to CORS preflight + exposes `Content-Range` for browser range fetches.
 - Added a **Q8_0 probe** that pulls a real GPT-OSS tensor slice and runs a WebGPU matmul in-browser.
 - `/gptoss` now shows a live **load progress bar** driven by real byte counts.
 - Added a **block 0 attention probe** (token embed → RMSNorm → Q/K/V → attn output) using real GPT-OSS weights.
- Added a **prefill + decode loop** with a CPU KV cache (per-layer) for multi-token forward passes.
- Added **cache telemetry** (seq_len, max_len, bytes) and live **token stream** output from real logits.
- Added **sliding window support** for attention (uses GGUF config for window size).
- Added **range/CORS validation** before parsing GGUF; emits a friendly error if the host doesn't honor Range.
- Default local `gguf_serve` now uses port **8080** (aligned with prompt plan).
- Added **tensor/expert LRU caches** (size-capped) with hit/miss/evict telemetry.
- Added **GPU alloc tracking** + **resident tensor list** in the HUD (last few weights shown).
- Added **runtime mode telemetry** (layer limit, attention mode, MoE fallback) and a **token pulse** on new output.
- Added **attention heatmap telemetry** (head 0) and a dedicated HUD panel for attention weights.
- `gguf_serve` now accepts a positional path argument in addition to `--path`.

---

## Tonight MVP (non-negotiable)

Goal: **one end-to-end compute path runs in-browser**:

1) Parse GGUF tensor table.
2) Range-fetch one **Q8_0** tensor slice.
3) Upload to GPU.
4) Run **one quantized linear** (small shape).
5) Read back output and compare to CPU reference (loose tolerance).

---

## Scope Cuts (hard)

- **Q8_0 only** (no Q4_K_M).
- **No MoE, no attention, no KV, no tokenizer/Harmony**.
- **No large buffers**: treat binding limits as tiny. Use tiled buffers even for one tensor.
- **No optimization pass**: correctness > speed tonight.

---

## Feasibility Gates (must pass)

### Gate A — GGUF Index
- Parse GGUF header + tensor table.
- Output `{name, ggml_type, dims, offset, nbytes}`.
- Confirm at least one Q8_0 tensor found.

### Gate B — Deterministic I/O
- Range-fetch a tensor slice by `(offset, nbytes)` and hash it.
- Same request yields same hash (cache optional).

### Gate C — GPU Compute
- Upload slice into `wgpu::Buffer`.
- Run compute shader that does:
  - Q8_0 dequant (f32 first; f16 optional later)
  - `Y = X @ W` for a **small shape** (e.g., `[1×K] @ [K×N]`, K,N <= 512)

### Gate D — Correctness
- CPU reference dequant + matmul.
- Read back GPU `Y` and compare within tolerance.

If Gate D passes, the browser spine is real.

---

## Phase 2 Plan — In-Browser GPT-OSS Runtime (Next)

Goal: turn the Gate C/D prototype into a real **gpt-oss** runtime in the browser,
with live telemetry streaming into `/gptoss` while weights load and tokens decode.

### Phase 2 Milestones (ordered)

1) **Shared WebGPU context**
   - Reuse the **existing** WGPUI `wgpu::Device/Queue` (no second device).
   - Surface limits/features in-state so kernel tiling respects WebGPU caps.

2) **Browser GGUF + range I/O module**
   - Extract `GGUF index + range fetch` into a reusable `gguf_web` module.
   - The runtime and gate should share the same parser + range reader.

3) **Runtime scaffold**
   - Add `gptoss_runtime` module that owns:
     - `GpuContext` (device/queue)
     - `GgufIndex`
     - weight residency manager (resident + paged experts)
   - Initial API:
     - `load_index(url) -> GgufIndex`
     - `load_tensor_slice(name, len) -> bytes`

4) **Real-shape Q8_0 linear**
   - Upgrade kernel to handle **real GPT-OSS tensor shapes** with tiling.
   - Enforce **buffer chunking**: never assume a huge single binding.
   - Emit telemetry:
     - `weights_fetch`, `weights_map`, `gguf_parse`, `kernel_dispatch`

5) **Single transformer block (no MoE)**
   - RMSNorm → RoPE → QKV → **dense attention** for one block.
   - Use a tiny test prompt; stop after one block; validate logits shape.

6) **MoE router + 1 expert path**
   - Parse expert weights (ggml type 39 in GPT-OSS GGUF).
   - Implement router top-k (k=2) and run one expert MLP.

7) **Paged expert cache**
   - Cache hot experts on GPU with LRU eviction.
   - Emit telemetry for cache hits/misses and resident memory.

8) **KV cache + decode loop**
   - Add per-layer KV cache (f16 if available).
   - Token-by-token decode; stream telemetry to `/gptoss`.

9) **Tokenizer + Harmony format**
   - Add GPT-OSS tokenizer + Harmony prompt wrapper.

10) **Correctness checks**
   - Compare a short prompt vs llama.cpp (tolerance-based).

### Full GPT-OSS Checklist (blocking items)

- GGUF tensor naming map for GPT-OSS (dense + MoE experts).
- WebGPU limits adaptation (buffer sizes, binding counts, dynamic offsets).
- Q8_0 (dev) + Q4_K_M (realistic memory) kernels.
- Attention banded window for alternating layers.
- MoE expert paging and cache accounting.
- KV cache eviction + sliding window.

---

## Minimal Kernel Plan

Start dumb, then optimize later:

- **Q8_0 block layout**: each block has a scale + 32 int8 values.
- Implement **dequant + dot** or **dequant + tiny matmul**.
- Avoid fancy tiling until correctness is proven.

---

## Recommended File Layout (minimal)

- `gguf/` — header + tensor table parsing
- `io/` — range fetch + small cache
- `gpu/` — device + buffer helpers + pipeline creation
- `kernels/` — WGSL (Q8_0 dequant + matmul)
- `tests/` — CPU reference dequant + matmul

---

## Execution Order (Tonight)

1) **GGUF parser**: load header + tensor table, dump tensor list.
2) **Range fetch**: pull a Q8_0 tensor slice, hash and verify determinism.
3) **CPU reference**: dequant + matmul for a tiny shape.
4) **WGSL kernel**: dequant + matmul for the same shape.
5) **Readback compare**: pass Gate D.

---

## Inputs

- Start with any **small GGUF** that includes Q8_0 (fast iteration).
- Once spine is proven, swap in GPT-OSS tensor slices.

---

## After Tonight (Phase 2)

Only after Gate D passes:

1) Scale GEMM shapes + tiling
2) RMSNorm + RoPE
3) Dense attention (single block)
4) MoE routing + expert cache
5) Tokenizer + Harmony last

---

## Reality Check

We do **not** promise "20B runs" tonight.
We prove the **browser inference spine** works. That's the only win that matters tonight.
