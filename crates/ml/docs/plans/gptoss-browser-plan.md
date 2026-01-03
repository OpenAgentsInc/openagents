Below is a **full implementation spec** for running **gpt-oss-20b GGUF** **in-browser** using **Rust + wgpu (WebGPU)** (and sharing the same `wgpu::Device/Queue` you already use for graphics).

This is optimized for: **“it loads and runs on consumer laptops”** + **“slow is acceptable”** + **“scales to async agents via batching/speculation.”**

---

# Rust WebGPU LLM Inference Spec (gpt-oss-20b, GGUF)

## 0) Facts we must design around

* `wgpu` runs on top of **WebGPU on wasm**. ([Docs.rs][1])
* gpt-oss models are **MoE**; `gpt-oss-20b` is **~21B total params, ~3.6B active per token**, **24 layers**, **GQA group size 8**, and **alternating dense + locally banded sparse attention**. ([OpenAI][2])
* WebGPU has **adapter-dependent limits** (buffer sizes, binding sizes, workgroup sizes). You must query them (`GPUAdapter.limits` / `GPUSupportedLimits`). ([MDN Web Docs][3])
* On Chrome you can request higher `maxBufferSize` via **requiredLimits** (when supported). ([Chrome for Developers][4])
* For speed/memory, use `shader-f16` when available (WGSL: `enable f16;`). wgpu exposes this as `FeaturesWebGPU::SHADER_F16`. ([Chrome for Developers][5])
* Not all GPUs support `shader-f16` (notably some Vulkan paths). ([GitHub][6])
* GGUF is designed for **fast loading / memory mapping** with per-tensor metadata including **offsets**. ([GitHub][7])

---

## 1) Goals / Non-goals

### Goals

* Run `gpt-oss-20b` **entirely in browser** (desktop Chrome/Edge/Firefox WebGPU).
* Use **Rust** compiled to `wasm32-unknown-unknown`.
* Reuse your existing `wgpu` instance/device for compute + rendering.
* Support GGUF quant variants (**Q8_0 first** for dev/correctness, **Q4_K_M later** for memory efficiency).
* "Slow is fine" but must be **stable** and **doesn't OOM**.
* Support **multiple async sessions** with continuous batching.

### Non-goals (initially)

* iOS Safari parity (WebGPU + memory pressure makes this hard).
* Training/fine-tuning in browser.
* Full llama.cpp feature parity.

---

## 1.5) Feasibility Gates (non-negotiable)

These gates must pass before proceeding to full implementation:

### Gate A — GGUF Index ✅
- Parse GGUF header + tensor table.
- Output `{name, ggml_type, dims, offset, nbytes}`.
- Confirm at least one Q8_0 tensor found.

### Gate B — Deterministic I/O ✅
- Range-fetch a tensor slice by `(offset, nbytes)` and hash it.
- Same request yields same hash (cache optional).

### Gate C — GPU Compute ✅
- Upload slice into `wgpu::Buffer`.
- Run compute shader that does Q8_0 dequant + `Y = X @ W`.

### Gate D — Correctness ✅
- CPU reference dequant + matmul.
- Read back GPU `Y` and compare within tolerance.

### Gate E — WebGPU Limits Compliance
- Dump adapter limits + device features in HUD at startup.
- Log: `maxBufferSize`, `maxStorageBufferBindingSize`, `maxBindGroups`, `maxBindingsPerBindGroup`, f16 support.
- Hard-assert that all buffer sizes / bindings / bind-group counts / dynamic offsets stay within limits.
- If not, auto-switch to smaller tile sizes and retry.
- All kernels must select tile sizes from these limits (no hardcoded constants).

### Gate F — Identify ggml Type 39
- Must map ggml type 39 to a known quant format (MXFP4/block-fp4) or explicitly branch-support it.
- If unknown after investigation: fallback = "router selects expert 0 always" mode (dense-only subset path).
- This still generates tokens, just worse quality until MoE lands.

### Gate G — End-to-End Logits
- With real weights, run: `token_embd → (1-2 layers) → lm_head → logits`.
- Attention can be stubbed to identity initially.
- Show top-5 tokens in HUD.
- This is the first "user-visible" checkpoint.

---

## 2) High-level architecture

### Components (Rust crates/modules)

1. **`gguf`**: Parse GGUF header, tensor table → `(name, ggml_type, dims, file_offset, nbytes)`.
2. **`fetch_cache`**: Fetch GGUF via HTTP Range (or File API), persist into IndexedDB/CacheStorage, provide async `read_range(offset,len)`.
3. **`gpu`** (wgpu backend):

   * device/queue integration
   * buffer pool (staging + persistent)
   * compute pipelines + bind layouts
4. **`kernels_wgsl`**: WGSL shaders for primitives:

   * dequant + matmul (ggml Q4_K / Q8_0)
   * RMSNorm
   * RoPE
   * attention (dense + sliding window)
   * MoE router + top-k
   * MoE MLP (experts) + combine
5. **`runtime`**:

   * model graph execution (layer loop)
   * KV cache manager
   * batching scheduler
6. **`tokenizer`**: GPT-OSS tokenizer (BPE) + Harmony formatting (the HF card warns it expects Harmony format). ([Hugging Face][8])
7. **`sampler`**: temperature/top-p/top-k, repetition penalty.
8. **`telemetry`**: tok/s, GPU time, memory usage, cache hit rate.

### Process layout

* Browser app boots graphics as usual.
* LLM init obtains the **same** `wgpu::Device` + `wgpu::Queue`.
* LLM compute submits command buffers alongside render passes.

---

## 3) WebGPU limits strategy (critical)

### At startup

* Query adapter supported limits/features (via `wgpu` + WebGPU).
* Request device with:

  * `shader-f16` if present (f16 storage + math), else f32 fallback. ([Docs.rs][9])
  * Increased `maxBufferSize` if adapter supports and your browser allows requesting it. ([Chrome for Developers][4])

### Binding size constraints

* Some platforms effectively cap `maxStorageBufferBindingSize` (often ~128MiB unless raised/available). WebGPU spec ties it to maxBufferSize. ([W3C][10])
  **Spec implication:** do **not** assume you can bind a 12GB weight buffer. Design for **many smaller buffers**.

---

## 4) Model file handling (GGUF in the web)

### Input modes

1. **Remote hosted GGUF** (preferred):

   * Server must support `Accept-Ranges: bytes`.
   * `fetch_cache` reads only the ranges needed.
2. **User-provided file**:

   * `File` → stream reader; implement your own range reads.

### Cache

* Store fetched chunks in IndexedDB keyed by `(url, offset, len, etag/sha)`.
* Evict LRU when storage quota is hit.

### GGUF index

* Parse tensor table once; store:

  * `tensor_data_base_offset`
  * per-tensor `(ggml_type, dims, absolute_offset, nbytes)`
* GGUF docs and HF doc confirm tensor metadata and fast loading intent. ([GitHub][7])

---

## 5) Weight residency plan (two-tier)

### Tier A: “Resident” small weights (always on GPU)

Keep these persistent:

* token embeddings
* layer norms (RMSNorm)
* attention projection weights
* router weights (for MoE gating)
* lm_head

### Tier B: “Paged” large weights (experts)

MoE means most params are in experts; only a few experts are used per token. ([OpenAI][2])
So:

* Keep an **expert cache** on GPU: `N` expert tensors worth (configurable).
* On each layer step:

  1. run router
  2. select top-k experts
  3. ensure those experts’ weights are loaded (cache hit or fetch+upload)
  4. run expert MLP and combine

This is the biggest reason 20B can be feasible on constrained memory.

---

## 6) KV cache design (must be memory-safe)

### Storage

* KV cache per session per layer.
* Use f16 if possible, else f32.
* Add **sliding window** mode to cap KV memory (align with “locally banded sparse attention” layers). ([OpenAI][2])

### Compression (optional v2)

* Int8/fp8 KV with per-block scales to increase session capacity.

### Paged KV

* Store KV in fixed-size pages to avoid realloc and fragmentation (vLLM-style idea, but simpler).

---

## 7) Execution modes: Prefill vs Decode

### Prefill (prompt ingestion)

* Batch across many sessions.
* Use larger matmuls, high GPU occupancy.
* Load experts with best-effort prefetch based on router outputs.

### Decode (token-by-token)

* Latency is dominated by per-layer overhead and cache churn.
* **To reduce paging cost**, implement:

#### Chunked verification (prefill chunks during decode)

Even though decode is “one token”, you can:

* Use speculative decoding (below) so the verifier runs *chunks*.

---

## 8) Speculative decoding (high leverage in browser)

* Run a **small draft model** (could be a tiny WebGPU model or WASM CPU model) that proposes **K tokens**.
* Run the big model (gpt-oss-20b) in **prefill_chunk(K)** once, accept the matching prefix, repeat.
  This amortizes:
* expert paging
* kernel dispatch overhead
* JS/WASM ↔ WebGPU overhead

Target: K=8–32.

---

## 9) Compute kernels (WGSL) — Phased Implementation

Split implementation into three phases to keep shipping:

### Phase 2a: Linear + Norm + RoPE (bring-up)

These are the minimum kernels to produce logits:

1. **Q8_0 dequant + matmul** ✅ GPU (done)
   * Kernel: `y = x @ Wq` where `Wq` is packed Q8_0 blocks.
   * Tiled GEMM with workgroup memory.

2. **RMSNorm** ❌ CPU (needs GPU)
   * Required before every attention/FFN block.
   * Simple: compute RMS, scale by learned weights.
   * **Must create `rmsnorm.wgsl`**

3. **RoPE** ❌ CPU (needs GPU)
   * Config-driven base frequency (10k local, 1M global for GPT-OSS).
   * Apply to Q and K after projection.
   * **Must create `rope.wgsl`**

4. **Residual add** ❌ CPU (low priority)
   * `x = x + sublayer_output`

5. **LM head logits + CPU sampling** ✅ (acceptable)
   * logits → copy to CPU → top-k/top-p sampling there.
   * GPU sampling is Phase 2c optimization.

**Done when:** `token_embd → RMSNorm(GPU) → lm_head → logits → top-5 tokens displayed`

### Phase 2b: Attention (STAGED — decode first)

**CRITICAL: Attention is currently CPU. Implement in stages, not one big leap.**

#### Stage 2b-1: Decode-only GPU attention (PRIORITY)
* Implement `attention_decode.wgsl` for seq_len=1 decode
* Input: current Q + cached K/V
* Softmax over cached tokens
* **This is the critical win** - decode is the hot path

#### Stage 2b-2: GPU KV cache append
* Store K/V in wgpu::Buffer (not CPU Vec)
* Support sliding window (ring buffer overwrite)
* Can use `queue.write_buffer` initially

#### Stage 2b-3: Prefill GPU attention (lower priority)
* Full causal masking for seq_len > 1
* Can be slower initially
* Prefill can stage through CPU while decode works

**Current state:** CPU attention runs but is O(n²) slow. Unacceptable for production.

**Done when (Stage 2b-1):**
- `attention_decode.wgsl` exists and runs
- Decode step uses GPU attention
- HUD shows "Attention: GPU (decode)"
- **CPU attention in decode path is BANNED after this**

**Done when (Stage 2b-2):**
- KV cache is wgpu::Buffer
- Cache grows correctly per token
- Sliding window works

### Phase 2c: GPT-OSS Exactness

1. **Sliding-window / banded attention**
   * 1024-token local window for alternating layers.
   * Only needed for long contexts (>1024 tokens).

2. **MoE router + experts (ggml type 39)**
   * Router top-k selection (k=2 for GPT-OSS).
   * Expert MLP with MXFP4/block-fp4 weights.
   * Weighted combine of expert outputs.

3. **Speculative decode** (optional but huge leverage)
   * Draft model proposes K tokens.
   * Verifier runs in prefill mode.
   * Amortizes expert paging and dispatch overhead.

**Done when:** Full GPT-OSS forward pass with MoE runs, expert cache hits/misses visible in HUD.

### f16 usage

* If `shader-f16` is enabled, use `f16` buffers and math for bandwidth wins; WGSL requires `enable f16;`. ([Chrome for Developers][5])
* Fallback path: f32.

### Q4_K_M is Phase 3

**Explicit decision:** Q4_K_M support is optimization/memory work, not correctness work. Land Q8_0 end-to-end first, then add Q4_K_M for production memory efficiency.

---

## 10) Scheduling & batching

### Continuous batching

* Maintain an “active decode set” of sessions.
* Each decode step:

  * gather sessions needing a token
  * pad to a batch size
  * run one fused “layer pass” per layer for batch

### Expert prefetch

* After router on layer L:

  * kick async fetch+upload for (layer L+1) experts predicted likely hot (simple heuristic: reuse previous token’s experts; it works surprisingly well).

---

## 11) Integration with your existing wgpu renderer

### Single device, shared queue

* Use one `wgpu::Device` + `wgpu::Queue`.
* Submit:

  * `encoder_llm.finish()` and `encoder_render.finish()` in the same frame loop.
* If you need fairness:

  * throttle LLM compute by time budget per frame (e.g., 4–8ms) or run LLM on a separate “tick loop” when UI idle.

### Resource ownership

* Keep LLM buffers in a dedicated allocator namespace.
* Avoid mapping render buffers while LLM is running; prefer `queue.write_buffer` staging.

---

## 12) Public API surface (Rust → JS)

Expose via `wasm-bindgen`:

* `init_llm(config) -> LlmHandle`
* `load_model(url_or_file) -> progress stream`
* `create_session(system_prompt, params) -> SessionId`
* `prefill(session, text)`
* `decode_next(session) -> token/text`
* `generate(session, max_tokens) -> async iterator`
* `stats() -> { tok_s, cache_hit, gpu_mem_est }`

---

## 13) Testing & correctness

* Golden test suite:

  * Run same prompt in llama.cpp and your WebGPU runtime; compare token-by-token for small seeds (expect some drift with different math/quant unless strictly matched).
* Kernel unit tests:

  * CPU reference for dequant formats (Q4_K/Q8_0) and matmul checks.

---

## 14) MVP milestones (in order)

1. **WebGPU compute smoke test** using your existing wgpu setup (`ComputePipeline` works on wasm). ([Docs.rs][12])
2. GGUF parser + tensor dump (names/types/dims/offsets). ([GitHub][7])
3. Implement **Q8_0** dequant+matmul first (simpler).
4. Implement a **single transformer block** end-to-end (no MoE yet).
5. Add MoE router + one expert path.
6. Add paging expert cache.
7. Add KV cache + decode loop.
8. Add speculative decode.
9. Add continuous batching.

---

## 15) Weight Naming + Layout Discovery

At startup, build a `ModelLayout` by regex-matching tensor names:

```rust
pub struct ModelLayout {
    /// token_embd.weight
    pub embeddings: TensorRef,

    /// output.weight (lm_head)
    pub lm_head: TensorRef,

    /// Per-layer tensors
    pub layers: Vec<LayerLayout>,
}

pub struct LayerLayout {
    /// blk.{N}.attn_norm.weight
    pub attn_norm: TensorRef,
    /// blk.{N}.ffn_norm.weight
    pub ffn_norm: TensorRef,

    /// blk.{N}.attn_q.weight, attn_k.weight, attn_v.weight, attn_output.weight
    pub attn_q: TensorRef,
    pub attn_k: TensorRef,
    pub attn_v: TensorRef,
    pub attn_output: TensorRef,

    /// blk.{N}.ffn_gate_inp.weight (MoE router)
    pub router: Option<TensorRef>,

    /// blk.{N}.ffn_gate.{E}.weight, ffn_up.{E}.weight, ffn_down.{E}.weight
    pub experts: Option<Vec<ExpertLayout>>,

    /// Dense FFN fallback (non-MoE layers)
    pub ffn_gate: Option<TensorRef>,
    pub ffn_up: Option<TensorRef>,
    pub ffn_down: Option<TensorRef>,
}
```

**Tensor name patterns for GPT-OSS GGUF:**
- `token_embd.weight` → embeddings
- `output.weight` → lm_head
- `blk.{N}.attn_norm.weight` → per-layer attention norm
- `blk.{N}.attn_q.weight` → Q projection
- `blk.{N}.attn_k.weight` → K projection
- `blk.{N}.attn_v.weight` → V projection
- `blk.{N}.attn_output.weight` → output projection
- `blk.{N}.ffn_gate_inp.weight` → MoE router (ggml type varies)
- `blk.{N}.ffn_gate.{E}.weight` → expert gate (ggml type 39 = MXFP4)
- `blk.{N}.ffn_up.{E}.weight` → expert up projection
- `blk.{N}.ffn_down.{E}.weight` → expert down projection

**Fail loudly** if any required tensor is missing. HUD should display missing tensor names.

---

## 16) Model Hosting Requirements

Don't assume Hugging Face Range + CORS will behave. Specify hosting requirements:

### Preferred: Your `gguf_serve` Range Server
- Already built and tested (`cargo run -p ml --bin gguf_serve`).
- Local development: `http://localhost:8080/model.gguf`
- Full CORS + Range support guaranteed.

### Required HTTP Features
Any host must support:
- `Accept-Ranges: bytes` header in response
- `Content-Range` header for partial responses
- Permissive CORS headers:
  - `Access-Control-Allow-Origin: *`
  - `Access-Control-Allow-Headers: Range`
  - `Access-Control-Expose-Headers: Content-Range, Content-Length`

### Startup Validation
If Range/CORS not present, loader should immediately show:
> "Host does not support Range/CORS. Use `gguf_serve` or a compatible host."

### URL Priority
1. `?gguf=<local gguf_serve URL>` (development)
2. `?gguf=<your controlled host>` (production)
3. HuggingFace URLs are "user-provided" not default (CORS issues)

---

## 17) Memory Budgets

Explicit caps to avoid silent OOM:

### Resident GPU Bytes Cap: 2-4GB
- All permanently loaded weights (embeddings, norms, attention projections, router weights, lm_head).
- For 20B model with Q8_0: ~3.6GB active params × 1 byte = ~3.6GB.
- Must fit in this budget.

### Expert Cache Cap: 1-2GB
- LRU cache for hot expert weights.
- Each expert ~100-200MB for 20B model.
- Cache holds 8-16 experts.
- Emit telemetry: cache size, hit rate, evictions.

### KV Cache Cap Per Session: 256-512MB
- With sliding window (1024 tokens), this is manageable.
- Per-layer KV: `seq_len × num_kv_heads × head_dim × 2 × dtype_size`
- For 24 layers, 8 KV heads, 128 head_dim, 1024 tokens, f16:
  - `1024 × 8 × 128 × 2 × 2 × 24 = ~100MB` per session.

### Eviction Policy
- KV cache: sliding window (drop tokens beyond window).
- Expert cache: LRU eviction.
- If GPU memory pressure detected, prefer evicting experts over KV.

---

## 18) Key design choices (my recommendation)

* **Start with `gpt-oss-20b-Q8_0.gguf`** for correctness/dev simplicity; then add `Q4_K_M` for real memory/perf.
* Implement **paged expert cache + speculative decode** early—this is what makes browser inference tolerable.
* Treat WebGPU limits as first-class: always query `adapter.limits` and adapt buffer tiling/bindings accordingly. ([MDN Web Docs][3])

---

If you want, I can turn this into a concrete repo plan:

* exact crate layout
* `wgpu::DeviceDescriptor` limits/features setup for WebGPU
* a concrete WGSL interface for `qlinear_ggml(Q4_K_M/Q8_0)`
* and a “tensor naming mapper” for the Unsloth GGUF (so MoE tensors get discovered automatically).

[1]: https://docs.rs/wgpu/latest/wasm32-unknown-unknown/wgpu/?utm_source=chatgpt.com "wgpu - Rust"
[2]: https://openai.com/index/introducing-gpt-oss/?utm_source=chatgpt.com "Introducing gpt-oss"
[3]: https://developer.mozilla.org/en-US/docs/Web/API/GPUSupportedLimits?utm_source=chatgpt.com "GPUSupportedLimits - Web APIs | MDN"
[4]: https://developer.chrome.com/blog/new-in-webgpu-133?utm_source=chatgpt.com "What's New in WebGPU (Chrome 133) | Blog"
[5]: https://developer.chrome.com/blog/new-in-webgpu-120?utm_source=chatgpt.com "What's New in WebGPU (Chrome 120) | Blog"
[6]: https://github.com/gpuweb/gpuweb/issues/5006?utm_source=chatgpt.com "\"shader-f16\" requirements exclude all Qualcomm devices"
[7]: https://github.com/ggml-org/ggml/blob/master/docs/gguf.md?utm_source=chatgpt.com "ggml/docs/gguf.md at master · ggml-org/ggml"
[8]: https://huggingface.co/openai/gpt-oss-20b?utm_source=chatgpt.com "openai/gpt-oss-20b"
[9]: https://docs.rs/wgpu/latest/wasm32-unknown-unknown/wgpu/struct.FeaturesWebGPU.html?utm_source=chatgpt.com "FeaturesWebGPU in wgpu - Rust"
[10]: https://www.w3.org/TR/webgpu/?utm_source=chatgpt.com "WebGPU"
[11]: https://cdn.openai.com/pdf/419b6906-9da6-406c-a19d-1bb078ac7637/oai_gpt-oss_model_card.pdf?utm_source=chatgpt.com "gpt-oss-120b & gpt-oss-20b Model Card"
[12]: https://docs.rs/wgpu/latest/wasm32-unknown-unknown/wgpu/struct.ComputePipeline.html?utm_source=chatgpt.com "ComputePipeline in wgpu - Rust"
