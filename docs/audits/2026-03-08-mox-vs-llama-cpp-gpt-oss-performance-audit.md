# 2026-03-08 Mox vs llama.cpp GPT-OSS Performance Audit

> Historical note: This audit is a point-in-time snapshot from 2026-03-08. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, `docs/OWNERSHIP.md`, and `docs/kernel/`. File paths and implementation-status claims here may be superseded by later commits.

## Scope

- Mox GPT-OSS inference path:
  - `crates/mox/mox-serve/src/openai_http.rs`
  - `crates/mox/mox-serve/src/lib.rs`
  - `crates/mox/mox-serve/src/gpt_oss.rs`
  - `crates/mox/mox-backend-cuda/src/lib.rs`
  - `crates/mox/mox-backend-cuda/src/kernels/quantized_matvec.cu`
  - `crates/mox/mox-models/src/lib.rs`
  - `crates/mox/mox-models/src/harmony.rs`
- `llama.cpp` GPT-OSS / OpenAI-MoE reference path:
  - `~/code/llama.cpp/src/models/openai-moe-iswa.cpp`
  - `~/code/llama.cpp/src/llama-graph.cpp`
  - `~/code/llama.cpp/src/llama-model.cpp`
  - `~/code/llama.cpp/ggml/src/ggml-cuda/mmvq.cu`
  - `~/code/llama.cpp/ggml/src/ggml-cuda/vecdotq.cuh`
  - `~/code/llama.cpp/src/llama-chat.cpp`
  - `~/code/llama.cpp/common/chat.cpp`
- Harmony reference:
  - `~/code/harmony/src/chat.rs`
  - `~/code/harmony/src/encoding.rs`

## Benchmark Setup

- Model:
  - `/home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf`
- Host:
  - local NVIDIA machine used for both runs
- Prompt path:
  - OpenAI-compatible `POST /v1/chat/completions`
  - same conversation structure
  - warm once, time the second request
- Deterministic comparison prompt:
  - user asked for a one-sentence response with predictable content
- Observed visible output from both servers:
  - `HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.`

## Measured Result

- Mox:
  - `37` completion tokens in `2.211s`
  - `16.74 tok/s`
- `llama.cpp`:
  - `42` completion tokens in `0.245s`
  - `171.22 tok/s`
- Relative gap:
  - `llama.cpp` is about `10.2x` faster on this workload

The token counts differ because the two servers do not report usage identically, but the visible output text matched exactly. The gap is real and large enough that reporting differences do not change the conclusion.

## Executive Summary

Mox is slower because its current GPT-OSS CUDA path is not a GPU-resident inference engine. It is a Rust-controlled CPU decode loop that calls a small number of CUDA quantized matvec kernels, then immediately synchronizes and copies results back to host memory. Attention, KV-cache traversal, RoPE application, RMSNorm, router selection, MoE weighting, and expert aggregation still run on the CPU. `llama.cpp` does not work that way. It builds the GPT-OSS/OpenAI-MoE token step as a ggml graph, keeps the hot path on the backend, uses flash-attention and grouped MoE backend ops, and ships far more mature CUDA kernels for GGUF quantized inference.

The short version is:

1. Mox uses CUDA as an accelerator for isolated matrix-vector products.
2. `llama.cpp` uses CUDA as the execution home for the decode step.

That architectural difference is the main reason for the roughly `10x` gap.

## What Is Not Causing The 10x Gap

### Harmony prompt rendering and parsing

Harmony is important for correctness, but it is not the dominant performance issue here.

- In Mox, Harmony prompt rendering happens once per request in `crates/mox/mox-serve/src/openai_http.rs` before generation, and Harmony output parsing happens once after generation.
- In `llama.cpp`, GPT-OSS chat-template application and parsing also happen outside the inner token loop in `src/llama-chat.cpp` and `common/chat.cpp`.
- Mox already uses the authoritative Rust Harmony crate in `crates/mox/mox-models/src/harmony.rs`, which wraps `openai_harmony`.

Harmony can affect startup and correctness. It does not explain a steady-state decode gap of this size.

### GGUF parsing itself

Mox's GGUF loader is not the primary cause of low steady-state tok/s either. Loader differences matter for startup and cold-load latency more than for warm decode throughput.

## Exact Hot-Path Comparison

### 1. Mox runs a token step as a CPU loop with CUDA subcalls

In `crates/mox/mox-serve/src/gpt_oss.rs`, `forward_step` does the following for each token:

- decodes the token embedding on CPU with `token_embedding.decode_row(...)`
- allocates host `Vec<f32>` buffers for `cache_key` and `cache_value`
- for each layer:
  - runs RMSNorm on CPU
  - runs Q/K/V matvecs via CUDA wrapper calls
  - adds bias on CPU
  - applies RoPE on CPU
  - writes K/V into host cache buffers
  - performs attention on CPU
  - runs attention output matvec via CUDA
  - runs router dense matvec on CPU
  - performs top-k routing and softmax on CPU
  - runs each selected expert as separate matvec calls
  - accumulates expert outputs on CPU
- runs final RMSNorm on CPU
- runs final output projection via CUDA

This is not backend-owned decode. It is host-owned decode with some GPU assists.

### 2. Mox attention is entirely CPU-side

`attend_impl` in `crates/mox/mox-serve/src/gpt_oss.rs` iterates over cached entries, computes attention logits with scalar dot products, performs softmax on CPU, and accumulates values on CPU.

That means:

- the KV cache is logically host-resident for decode use
- every decode step pays CPU attention cost
- the GPU never sees a fused attention problem it can solve efficiently

By contrast, `llama.cpp` routes attention through ggml graph ops and uses `ggml_flash_attn_ext(...)` when the path is available in `src/llama-graph.cpp`.

### 3. Mox router selection and MoE combination are host-side

In Mox:

- router logits come from `DenseMatrix::matvec(...)` on CPU
- expert selection uses host `top_k_indices(...)`
- route weights use host `softmax_selected(...)`
- selected experts are executed one expert at a time
- expert outputs are accumulated by Rust loops over `Vec<f32>`

That is especially expensive for GPT-OSS because GPT-OSS is MoE-heavy. The MoE path is not an edge case here. It is the model.

In `llama.cpp`, `build_moe_ffn(...)` in `src/llama-graph.cpp` keeps gating, selection, weighting, expert matmuls, and aggregation in the graph. The critical expert execution primitive is `build_lora_mm_id(...)` / `ggml_mul_mat_id(...)`, which allows grouped expert execution on the backend rather than a host-controlled loop of tiny launches.

### 4. Mox pays repeated host-device transfer and synchronization costs per matvec

This is the single clearest low-level performance mistake in the current Mox path.

`CudaQuantizedMatrix::matvec(...)` and `CudaQuantizedExpertTensor::expert_matvec(...)` in `crates/mox/mox-serve/src/gpt_oss.rs` both call into `CudaBackend::quantized_matvec(...)` or `quantized_matvec_with_offset(...)`.

Inside `crates/mox/mox-backend-cuda/src/lib.rs`, `quantized_matvec_with_offset(...)`:

- allocates a device input buffer from `input.to_vec()`
- allocates a device output buffer
- begins a submission
- launches one quantized matvec kernel
- commits with `CudaCommandWait::Completed`
- synchronizes the CUDA stream
- reads the output back to host `Vec<f32>`

The submission path also creates and destroys streams around these operations.

That means Mox repeatedly does all of the following in the inner decode loop:

- host-to-device copy for the input activation
- device kernel launch
- stream synchronize
- device-to-host copy for the output activation
- host-side follow-on compute

This destroys backend residency and leaves almost no room for normal GPU latency hiding.

`llama.cpp` does not structure decode this way. It keeps tensors inside backend buffers, uses the ggml backend scheduler, and avoids per-op "copy in, sync, copy out" semantics for the whole decode step.

### 5. Mox prompt prefill is still token-by-token eager execution

In `crates/mox/mox-serve/src/lib.rs`, prompt processing iterates through prompt tokens and calls `execute_step(...)` once per token. Decode then continues with another one-token-at-a-time eager loop.

That means Mox is missing:

- graph-level prompt prefill
- reusable compiled decode plan for GPT-OSS
- larger batched backend work units

`llama.cpp` builds forward graphs and hands them to the backend scheduler rather than issuing a Rust-level sequence of isolated micro-operations.

### 6. Mox kernels are far simpler than `llama.cpp` kernels

Mox's CUDA quantized path currently supports a narrow set of formats and uses a straightforward row-wise reduction kernel in `crates/mox/mox-backend-cuda/src/kernels/quantized_matvec.cu`.

`llama.cpp` uses substantially more mature CUDA code:

- `ggml-cuda/mmvq.cu` supports many quant formats including `Q8_0` and `MXFP4`
- `ggml-cuda/vecdotq.cuh` has specialized dot-product paths such as `vec_dot_mxfp4_q8_1`
- `ggml-cuda/fattn.cu` provides flash-attention kernels
- `ggml-cuda/getrows.cu` supports GPU row gathering for embeddings
- `ggml-cuda/mmid.cu` supports grouped expert execution

This matters twice:

1. `llama.cpp` offloads more of the model.
2. The code it offloads is also better optimized.

### 7. `llama.cpp` maps GPT-OSS naturally onto its graph/runtime model

`src/models/openai-moe-iswa.cpp` in `llama.cpp` builds GPT-OSS/OpenAI-MoE directly as a graph:

- input embedding gather
- Q/K/V projections
- RoPE
- attention
- RMSNorm
- MoE branch
- output projection

The backend can then schedule that graph coherently.

Mox instead manually orchestrates those same steps from Rust code, crossing the CPU/GPU boundary repeatedly. That is the architectural reason the gap is so large.

## Detailed Root Causes Ranked By Impact

### 1. Wrong execution boundary: Mox keeps activations on the host

This is the highest-impact root cause.

As long as each matvec returns a host `Vec<f32>`, the rest of the step will continue to happen on the CPU. That forces:

- repeated copies
- repeated sync points
- CPU ownership of norms, RoPE, attention, routing, and aggregation

No kernel tuning can fix that on its own.

### 2. CPU attention and host KV cache

For autoregressive decode, attention is one of the two dominant hot regions. Mox currently computes it in Rust over host memory. `llama.cpp` solves attention as a backend op and can use flash attention. This is a major share of the gap.

### 3. CPU-side MoE routing and expert dispatch

GPT-OSS is not a dense-only model. Its routing and expert execution path matters a lot. Mox currently pays:

- CPU router matvec
- CPU top-k selection
- multiple per-expert kernel launches
- host-side weighted aggregation

`llama.cpp` turns this into grouped backend work. That is another major share of the gap.

### 4. No graph-based decode or prompt prefill

Mox issues eager one-token Rust steps. `llama.cpp` reuses a graph-oriented runtime. Even if Mox moved more ops to CUDA, it would still leave performance on the table without a graph or plan-oriented execution model for GPT-OSS.

### 5. Less optimized CUDA kernels

Once the execution boundary is fixed, kernel quality becomes the next big factor. Right now Mox is behind on both execution structure and kernel maturity.

### 6. Loader and startup differences

These matter, but mostly for startup, memory efficiency, and cold behavior. They do not explain the steady-state `10x` decode gap by themselves.

## Why Harmony Still Matters

Harmony is not the throughput bottleneck, but it is still important to performance work for two reasons:

1. It determines the exact prompt/control-token surface for GPT-OSS.
2. It constrains what the generation endpoint must preserve in streaming and parsing.

The authoritative Harmony repo shows that channel parsing and message-boundary parsing are structured and strict in `src/encoding.rs` and `src/chat.rs`. `llama.cpp` also treats GPT-OSS specially in `src/llama-chat.cpp` and `common/chat.cpp`, including preserved tokens like `<|channel|>`, `<|message|>`, `<|start|>`, and `<|end|>`.

That means Mox should keep using authoritative Harmony semantics while optimizing the model runtime underneath. The correct performance target is "faster runtime under the same Harmony behavior," not "simplify the prompt format."

## Path To Reach llama.cpp-Class Performance

The path is clear, but it is not small. Mox needs a real GPT-OSS backend runtime, not more incremental tuning of the current host-owned loop.

### Phase 0: Measure the current baseline properly

Before more major changes, add instrumentation that can be trusted:

- NVTX ranges around:
  - prompt render
  - token embedding
  - each layer block
  - attention
  - router
  - expert dispatch
  - logits
- explicit counters for:
  - host-to-device bytes
  - device-to-host bytes
  - stream sync count
  - per-token kernel count
- Nsight Systems capture for the exact HTTP benchmark flow
- Nsight Compute on the quantized matvec kernels

This will not change performance directly, but it will stop the team from guessing after the first rewrite.

### Phase 1: Change the CUDA contract from "matvec returns Vec" to "decode state stays on device"

This is the first mandatory rewrite.

Needed changes:

- replace `Vec<f32>` activation handoff with device buffers
- make `CudaQuantizedMatrix::matvec(...)` write into preallocated device destinations
- do not create/destroy a stream for every matvec
- do not synchronize after every matvec
- keep the hidden state, Q, K, V, attention output, FFN intermediates, and logits on device until the token step is complete

This phase alone should remove the worst host-device ping-pong.

### Phase 2: Move KV cache, RoPE, RMSNorm, and attention onto CUDA

This is the second mandatory rewrite.

Needed changes:

- device-resident KV cache for GPT-OSS
- CUDA kernels or backend ops for:
  - RMSNorm
  - RoPE
  - K/V writeback
  - attention score/value application
- flash-attention-class decode path for causal attention

Until this is done, Mox will continue to spend too much time in host-side attention loops regardless of matvec speed.

### Phase 3: Replace the host MoE loop with grouped GPU expert execution

This is the GPT-OSS-specific mandatory rewrite.

Needed changes:

- device-side router logits
- device-side top-k expert selection
- device-side route softmax/normalization
- grouped expert execution primitive similar to `ggml_mul_mat_id(...)`
- device-side expert aggregation

If Mox keeps expert dispatch as "for each selected expert, run three separate calls and add into a host vector," it will never approach `llama.cpp` on GPT-OSS.

### Phase 4: Promote GPT-OSS from an eager Rust loop to a compiled decode graph

Needed changes:

- build a reusable GPT-OSS decode graph or equivalent compiled plan
- batch prompt prefill instead of token-by-token eager stepping
- cache and reuse decode plans for stable shapes
- keep backend scheduling authority in the runtime rather than in the Rust loop

This is how Mox stops behaving like a correctness reference implementation and starts behaving like an inference engine.

### Phase 5: Replace the current CUDA kernels with tiled, architecture-aware kernels

Once the execution boundary and graph are correct, kernel work becomes worth the effort.

Needed changes:

- move beyond the current row-wise reduction kernel
- add tiling and warp-level specialization for quantized matvec
- optimize specifically for `MXFP4` and `Q8_0` activation/weight combinations relevant to GPT-OSS
- tune for Ada GPUs, including this RTX 4080 host
- benchmark directly against `llama.cpp` kernels as the reference bar

This is the phase that should close the remaining gap after the structural fixes.

### Phase 6: Improve load/startup without destabilizing decode

Secondary but still worthwhile:

- preserve mmap-backed behavior where possible
- reduce full-byte materialization at model-build time
- explore more direct backend buffer creation from mapped GGUF pages where safe

This should improve startup and memory pressure, but it is not the first place to spend time if the goal is tok/s.

## Three Changes That Matter Most

If this work needs to be prioritized aggressively, the first three changes should be:

1. Keep activations on device for the whole token step.
2. Move attention and KV cache to CUDA.
3. Replace the host MoE loop with grouped GPU expert execution.

Those three are the shortest path from "CUDA-assisted CPU inference" to "real GPU inference."

## Definition Of Done

Mox should not claim parity with `llama.cpp` until all of the following are true on this exact host and model:

- same model:
  - `/home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf`
- same HTTP route shape:
  - `POST /v1/chat/completions`
- same Harmony-visible behavior:
  - prompt rendering
  - assistant channel parsing
  - streaming semantics
- same warm-benchmark method:
  - warm once
  - time the second request
- throughput target:
  - first milestone: `>= 80 tok/s`
  - parity milestone: within `20%` of `llama.cpp`

On the current measured baseline, that means Mox needs to move from about `16.74 tok/s` to at least the high double digits quickly, then into the `135+ tok/s` range to be within `20%` of the current `llama.cpp` result.

## Final Conclusion

`llama.cpp` is about `10x` faster here because it is solving the right problem with the right execution boundary:

- backend-owned decode graph
- backend-resident activations and KV
- backend attention
- backend MoE routing and expert execution
- mature quantized CUDA kernels

Mox is still solving GPT-OSS with a host-owned decode loop and GPU subroutines. That is the main reason for the gap, and it also defines the remedy: Mox needs a real GPU-resident GPT-OSS runtime, not more local tuning of the current per-matvec API.
