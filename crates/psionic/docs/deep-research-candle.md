# What to Adapt from Rust Candle for the Psionic Roadmap

## Executive summary

Candle is already a “Rust-native inference engine toolbox” for many of the exact gaps in your Psionic roadmap: it has production-hardened **GGUF/GGML quantized weight loading (including Metal and CUDA paths), tokenizer reconstruction from GGUF metadata, classic LLM sampling utilities (top‑k, top‑p, seeded RNG), repeat-penalty and GQA helpers, and multi-backend feature gating (CPU/MKL, macOS Accelerate, Metal, CUDA/CuDNN/NCCL)**. citeturn23view0turn23view4turn8view0turn11view0turn25view0turn37view0

For Psionic, the best adaptations are not “use Candle wholesale,” but **lift the parts that directly correspond to your remaining backlog** (GGUF/tokenizer/prompt behavior, sampler correctness, KV-cache + attention kernels, GPU memory + kernel cache policy, and multi-device readiness truth). Candle’s design patterns map closely to your “truthful backend surfaces” requirement because Candle itself is structured around explicit device backends (CPU/Metal/CUDA) and feature flags, rather than silent fallback. citeturn23view4turn35search6

The caveat: Candle does **not** solve your Ollama migration and compute-market substrate layers by itself (catalog semantics, lifecycle, scheduling/keepalive, explicit truncation policies, NDJSON-like streaming contract, receipts/evidence), and Candle’s “LLM serving at scale” features (paged attention, continuous batching) are partly in adjacent repos/crates (and bring licensing/maintenance considerations). citeturn34view0turn20view0turn10view3


## Candle snapshot that matters for Psionic

Candle is a multi-crate Rust workspace (“candle-core”, “candle-nn”, “candle-transformers”, plus GPU kernel crates) intended as a minimalist ML framework with GPU support. Its root README calls out the structure and positions it as a performance-focused Rust ML framework. citeturn23view3

Candle also explicitly exposes backends and build-time feature gating: `candle-core` has feature flags for `cuda`, `cudnn`, `nccl`, `mkl`, `accelerate`, and `metal`. This is the same shape you want for “truthful” provider capability reporting, because build/runtime capability is explicit rather than implied. citeturn23view4turn23view5turn23view6turn23view7turn23view8

Candle is dual-licensed Apache 2.0 and MIT (per badges/links in the README and the license files). That makes code reuse/vendoring feasible from a licensing standpoint, but you still need to watch third-party kernels in related repos (see paged attention note below). citeturn23view0turn20view0

## High-leverage Candle components to adapt

### Quantized GGUF/GGML loading across CPU, Metal, and CUDA

Your roadmap’s biggest “Ollama replacement” blocker starts at “read what Ollama already installed,” i.e., **GGUF**. Candle already has a real GGUF/GGML quantized loader pipeline:

- `candle-core` contains GGUF parsing (`quantized/gguf_file.rs`) that reads tensor metadata and tensor bytes. citeturn5view0  
- GGUF tensor loading flows into `qtensor_from_ggml(...)` which selects device-backed storage: **CPU, Metal, or Cuda** QStorage variants, not CPU-only. citeturn8view0  
- This is exactly the “don’t overclaim GPU readiness” pattern you want: the loader is parameterized by a `Device`, and storage differs per backend rather than silently falling back. citeturn8view0  

Why it matters to Psionic: your roadmap explicitly needs **GGUF tensor extraction**, **quantization metadata truth**, and later **accelerator coverage** (Metal text-gen, NVIDIA, AMD execution). Candle’s GGUF loader gives you (a) file-format correctness surface area, and (b) a proven device-storage abstraction for quantized weights. citeturn8view0turn23view4  

What to adapt concretely:
- The GGUF/GGML parsing approach (metadata model, tensor offsets, dtype/tag interpretation).
- The “quantized tensor → backend-specific storage” pattern (abstract QTensor/QStorage that can represent quantized buffers on CPU/Metal/CUDA). citeturn8view0  
- A follow-on improvement Candle itself hints at: its GGUF conversion path includes a TODO about an mmap-based version “to avoid copying around data,” which aligns with your “model memory planning + fast load” needs. citeturn8view0  

### Tokenizer reconstruction from GGUF metadata

One of the most actionable “lift, don’t rewrite” items is tokenizer reconstruction. Candle landed a recent feature explicitly addressing this: **“allow tokenizer to load from GGUF metadata”**. citeturn26search2turn2view0

Candle’s quantized tokenizer utilities implement `TokenizerFromGguf::from_gguf_metadata(...)`, pulling tokenizer fields out of GGUF metadata and building a `tokenizers::Tokenizer`, including BOS/EOS handling via `TemplateProcessing`. citeturn11view0

Why it matters to Psionic: your roadmap calls out tokenizer loading as a first-order missing piece, and it also needs explicit BOS/EOS and “add_bos_token / add_eos_token” behavior in parity with the current Ollama boundary. Candle already does BOS/EOS insertion semantics at tokenizer level (via `TemplateProcessing`). citeturn11view0turn34view0

What Candle does *not* solve here: true **chat-template (Jinja-ish) rendering** is separate from tokenizer reconstruction. Candle’s GGUF-tokenizer module doesn’t include `chat_template` handling. citeturn11view0  
So the recommended adaptation is: use Candle’s tokenizer-from-GGUF as the base layer, then add your own explicit “chat template extraction + rendering” layer in Psionic.

### Sampling, repeat penalty, and determinism primitives

Your Psionic roadmap repeatedly emphasizes “behavioral contract” and “deterministic replay.” Candle has ready-to-adapt building blocks:

- `candle-transformers` includes a compact generation module defining `Sampling` modes (`ArgMax`, `TopK`, `TopP`, `TopKThenTopP`, etc.) and a seeded `LogitsProcessor` (`StdRng::seed_from_u64`). citeturn25view0  
- `candle-transformers/src/utils.rs` implements `apply_repeat_penalty(...)` (with the common “divide positive logits / multiply negative logits” rule) and a `repeat_kv(...)` helper for grouped-query attention (GQA). citeturn37view0  

Why it matters to Psionic:
- You need **sampler correctness** (and seeded determinism), and you likely need **repeat penalty** and **GQA utilities** for the model families you listed (llama/qwen/mistral all commonly use GQA variants). Candle already has these in real code paths. citeturn25view0turn37view0  
- Candle’s own issue history also shows performance pitfalls (e.g., repeat penalty converting logits to CPU vec on Metal can be expensive), which is a useful warning for your “no silent fallback” and “performance gates” items: keep the penalty/sampling logic on-device where possible, or at least make its CPU hop explicit in evidence. citeturn37view0turn36search3  

### KV-cache and attention patterns, including a path to paged attention

Candle’s transformer models (e.g., llama) keep KV-cache state in an explicit cache object, and the forward pass uses that cache when enabled—this is the structural baseline you need for `PSI-126` style “deterministic KV-cache ownership and session lifecycle.” citeturn17view0

For scaling beyond naive KV growth, Candle’s ecosystem includes **paged attention** work. Hugging Face maintains `candle-paged-attention`, and its README explicitly states the kernels are adapted from vLLM’s CUDA sources. citeturn20view0turn21view3  
That matters because your roadmap’s long-term “compute-market substrate” items include batch posture, queueing, throughput truth, and later possibly `sandbox_execution`—all of which become much easier if KV-cache memory and batching are explicit.

How to adapt safely:
- Treat Candle’s baseline KV-cache patterns as the “first correct version.”
- If/when you pursue high-throughput batching, study/borrow the paged-attention interface and its shape/rank checks and storage constraints (it is CUDA-tensor-specific and explicit about it). citeturn21view4turn21view3  
- Be careful with licensing provenance: `candle-paged-attention` calls out vLLM kernel origin directly; you’ll want an explicit licensing review before vendoring any kernel code. citeturn20view0  

### Backend readiness truth via feature flags and explicit device objects

Candle’s `candle-core` feature flags are a very direct model for “truthful capability envelopes”:

- If built with `cuda`, `candle-core` pulls in `cudarc` and CUDA kernel crates; `cudnn` and `nccl` are explicit sub-features; `metal` pulls in Metal-specific dependencies and kernel crates. citeturn23view4turn23view5turn23view6  

Why it matters to Psionic:
- Your compute-market substrate wants to report “backend family, topology, concurrency posture, latency posture” etc. A practical first step is ensuring that “this binary even *has* CUDA/Metal support compiled in” is first-class capability truth. Candle’s feature model is a strong precedent. citeturn23view4turn35search6  

### Metal memory management, kernel caching, and preventing runaway allocations

Your roadmap includes “memory planning, residency policy, and admission control,” plus “explicit cutover performance thresholds.” Candle’s Metal backend code and PR history contain concrete solutions you can adapt:

- Candle’s Metal device code uses **buffer pooling keyed by size buckets**, reusing buffers when `Arc::strong_count == 1` (meaning the computation graph dropped it and only the pool retains a reference), and tracks a kernel cache for compiled kernels. citeturn30view0  
- Candle’s PR “bound temporary buffer cache and prevent runaway memory usage…” explicitly discusses adding an allocation policy and improving Metal memory detection using `iogpu.wired_limit_mb`. citeturn27view1turn27view0  

Why it matters to Psionic:
- Token generation is a “small ops, many steps” workload. Without explicit buffer reuse and trimming, Metal can look like it “leaks” even when it’s just caching; Candle’s approach gives you a defensible, testable memory story. citeturn30view0turn27view0  
- The `iogpu.wired_limit_mb` detail matters for honest capability envelopes and admission control on Apple Silicon, because “available GPU memory” can be policy-driven and may differ from naive RAM size assumptions. Candle treating this as part of allocation policy is a useful precedent. citeturn27view0  

### Custom ops as an escape hatch for fused kernels and evidence-friendly metering

Two Candle mechanisms are especially relevant to Psionic’s “tinygrad-style primitives” and later compute-market evidence needs:

- Candle’s docs emphasize embedding user-defined ops/kernels (they explicitly mention flash-attention v2 as an example). citeturn35search6  
- The `Tensor` API exposes `apply_op*` methods for custom ops, including versions without backward support (useful for inference-only serving), which is a clean template for “add a fused op with explicit backend implementation.” citeturn35search9  

Why it matters to Psionic:
- For “Metal text generation,” the hard part is often attention/softmax/rope fusions and memory movement. A custom-op escape hatch makes it possible to keep your *semantic* op surface small while still adding high-performance kernels. citeturn35search6turn35search9  
- For compute-market delivery proofs, custom ops are also a natural place to hook “metering and evidence” (e.g., emit FLOP/byte estimates, kernel-plan digests, cache-hit/miss) without scattering logic across the whole runtime (this is an inference based on Candle’s explicit kernel caching + custom-op extension points). citeturn30view0turn35search9  


## Mapping Candle to the Psionic roadmap gaps

The table below treats your roadmap as the target and Candle as a library of “already-solved subproblems.”

| Psionic roadmap gap | Candle artifact to adapt | Why it’s high leverage | What you still need to add |
|---|---|---|---|
| GGUF loading + tensor extraction | `candle-core/src/quantized/gguf_file.rs` + `ggml_file.rs` quantized load pipeline | Proven parsing + quantized tensor creation with backend-specific storage (CPU/Metal/CUDA) citeturn5view0turn8view0 | Ollama manifest/catalog semantics; robust metadata tolerance + Psionic error taxonomy |
| Tokenizer from GGUF | PR/commit enabling tokenizer load from GGUF metadata; `quantized/tokenizer.rs` | Directly matches your tokenizer + BOS/EOS needs during Ollama migration citeturn2view0turn11view0 | Chat templates (`chat_template`) + role rendering + prompt-format parity (Candle doesn’t implement this) citeturn11view0 |
| Sampler correctness + determinism | `candle-transformers/src/generation/mod.rs` `LogitsProcessor` (seeded RNG; TopK/TopP) | Solid baseline for deterministic replay testing and option parity citeturn25view0 | Penalty taxonomy beyond repeat penalty (presence/frequency), stop-sequence semantics, streaming chunk protocol |
| Repeat penalty + GQA helpers | `candle-transformers/src/utils.rs`: `apply_repeat_penalty`, `repeat_kv` | These are exactly the “death by missing little details” parts of LLM parity citeturn37view0 | Optimize away CPU roundtrips on GPU backends; integrate into Psionic evidence/metrics |
| Metal memory stability + kernel cache | Candle Metal device/buffer pool + PRs on memory detection + cache trimming | Gives you an implementation template for residency policy and preventing runaway memory on Metal citeturn30view0turn27view0turn27view1 | Integrate into Psionic’s own admission control and capability envelope reporting |
| CUDA/NCCL gating + multi-GPU direction | `candle-core` cuda/cudnn/nccl features | Explicit backend truth via compile-time features; NCCL existence signals planned multi-GPU support citeturn23view4turn23view5 | Your own topology truth + model sharding planner + compute-market-facing substitution checks |
| High-throughput KV-cache memory mgmt | `candle-paged-attention` (CUDA-only) | Concrete paged-attention implementation patterns; makes batching and cache accounting explicit citeturn21view4turn21view3 | Licensing review + Metal/AMD equivalents + integration into Psionic session model and evidence story citeturn20view0 |
| Model download + mmap + sharding patterns | Candle docs: hf-hub + memmap2 + safetensors sharding advice | Practical guidance for efficient load and multi-GPU sharding, plus warnings about mmap pitfalls citeturn34view0 | You’re migrating from Ollama store, not HF Hub, so you need a parallel “local Ollama store” catalog and integrity layer |

## How to adapt Candle without breaking Psionic’s “truthful compute-market substrate” goals

The critical design choice is *what level* you borrow Candle at. Given your roadmap’s emphasis on explicit capability/evidence/lifecycle, the strongest approach is:

- Borrow Candle for **file formats + low-level runtime mechanics** (GGUF/tokenizer, quantized tensor storage, sampling kernels/helpers, Metal allocator patterns, custom-op patterns).
- Keep Psionic’s higher-level product surfaces and compute-market substrate semantics (catalog, session lifecycle, admission/residency policy, streaming contract, evidence receipts, provider inventory truth) as Psionic-owned layers.

A practical integration flow (conceptual) looks like:

```mermaid
flowchart LR
  A[Ollama model store<br/>GGUF blobs + tokenizer files] --> B[Psionic Catalog]
  B --> C[Psionic GGUF Loader<br/>(adapt Candle quantized GGUF/GGML)]
  C --> D[Psionic Tokenizer Builder<br/>(adapt Candle tokenizer-from-GGUF)]
  D --> E[Psionic Prompt Renderer<br/>(Psionic-owned chat_template + BOS/EOS policy)]
  E --> F[Psionic Runtime Session<br/>(KV cache ownership, admission, warm/cold)]
  C --> F
  F --> G[Psionic Backends]
  G --> G1[CPU]
  G --> G2[Metal<br/>(adapt Candle buffer pool + kernel cache)]
  G --> G3[CUDA/NVIDIA<br/>(adapt Candle feature gating + kernel patterns)]
  F --> H[Autopilot local runtime seam]
  F --> I[Pylon provider execution adapter]
  F --> J[Compute-market evidence hooks<br/>(plan digests, cache hits, memory, timing)]
```

Key adaptation principles:

- **Keep backend truth explicit at the seam.** Candle’s compile-time features are a clear precedent: don’t claim CUDA/Metal readiness unless built and probed. Mirror this in Psionic capability envelopes (e.g., “compiled_with_cuda=true”, “metal_device_count=1”). citeturn23view4  
- **Treat tokenizer construction and prompt rendering as separate concerns.** Candle helps on tokenizer reconstruction (including BOS/EOS post-processing), but prompt templates are Psionic-owned and must be explicitly versioned for compute-market evidence. citeturn11view0turn34view0  
- **Sampler determinism must be testable.** Candle’s `LogitsProcessor` shows a small, testable surface (seeded RNG, top‑k/top‑p). Use that shape, then extend with your exact option surface and replay tests. citeturn25view0  
- **Memory planning must be grounded in actual backend allocator behavior.** Candle’s Metal path demonstrates real buffer pooling plus the need to bound caches and account for platform-specific GPU memory limits (`iogpu.wired_limit_mb`). This is directly relevant for your “admission control” and “cutover gates” items. citeturn30view0turn27view0  
- **Be cautious with “advanced throughput” imports (paged attention).** `candle-paged-attention` is valuable as a reference, but it is CUDA-only and explicitly derived from vLLM kernels—meaning it’s not a drop-in for “Psionic as a universal substrate” without more work (and licensing diligence). citeturn20view0turn21view4  


## Appendix: IR and a prioritized source list

### What is IR in this context?

**IR** usually means **Intermediate Representation**: a structured, programmatic representation of computation that sits between “model-level code” and “backend execution.” In an inference engine it commonly represents graphs/ops, shapes, dtypes, and sometimes scheduling/memory planning decisions—so the compiler/runtime can lower the same IR into CPU kernels, Metal kernels, CUDA kernels, etc.

Candle itself hints at this pattern via its “user-defined ops/kernels” pathway and the presence of kernel compilation machinery (e.g., compiling a kernel into a Metal compute pipeline in the Metal device implementation). citeturn30view0turn35search9turn35search6  

### Prioritized Candle sources and commits referenced

Primary sources (recommended starting points):

- Candle repository + project structure and licensing (README). citeturn23view0turn23view3  
- `candle-core` feature flags (cuda/cudnn/nccl/metal/mkl/accelerate) for explicit backend truth. citeturn23view4  
- GGUF/GGML quantized loading pipeline and device-backed storage selection. citeturn5view0turn8view0  
- Tokenizer-from-GGUF change and implementation (`TokenizerFromGguf`). citeturn2view0turn11view0  
- Sampling utilities (`LogitsProcessor`, Sampling enum). citeturn25view0  
- Repeat penalty + GQA helper (`apply_repeat_penalty`, `repeat_kv`). citeturn37view0  
- Metal allocator patterns (buffer pooling, kernel cache). citeturn30view0  
- Metal memory budget work mentioning `iogpu.wired_limit_mb`. citeturn27view0turn27view1  
- Candle documentation on memory mapping and tensor-parallel sharding patterns (plus caveats about mmap). citeturn34view0  
- Candle paged attention repo note about vLLM kernel origin (licensing/provenance flag). citeturn20view0
