# 2026-03-08 Psionic vs llama.cpp GPT-OSS Performance Audit

> Updated 2026-03-09 after the CUDA/runtime checkpoint that landed the perf
> groundwork through GitHub issues `#3242` through `#3246`, and after the
> direct `llama.cpp` alignment checkpoint that ported the first graph-driven
> CUDA fusion ideas into Psionic and produced a new live benchmark on the local
> RTX 4080 host. This file is the current audit for the GPT-OSS throughput gap;
> later product truth still lives in `docs/MVP.md`, `docs/OWNERSHIP.md`,
> `crates/psionic/docs/ROADMAP.md`, and the referenced issues.

## Scope

- Psionic GPT-OSS path:
  - `crates/psionic/psionic-serve/src/gpt_oss.rs`
  - `crates/psionic/psionic-serve/src/lib.rs`
  - `crates/psionic/psionic-backend-cuda/src/lib.rs`
  - `crates/psionic/psionic-backend-cuda/src/kernels/quantized_matvec.cu`
  - `crates/psionic/psionic-backend-cuda/build.rs`
  - `crates/psionic/psionic-models/src/harmony.rs`
- `llama.cpp` reference path:
  - `~/code/llama.cpp/src/models/openai-moe-iswa.cpp`
  - `~/code/llama.cpp/src/llama-graph.cpp`
  - `~/code/llama.cpp/src/llama-context.cpp`
  - `~/code/llama.cpp/ggml/src/ggml-cuda/mmvq.cu`
  - `~/code/llama.cpp/ggml/src/ggml-cuda/vecdotq.cuh`
  - `~/code/llama.cpp/ggml/src/ggml-cuda/fattn.cu`
  - `~/code/llama.cpp/ggml/src/ggml-cuda/mmid.cu`
- Harmony reference:
  - `~/code/harmony/src/chat.rs`
  - `~/code/harmony/src/encoding.rs`

## Benchmark Contract

- Model:
  - `/home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf`
- Host:
  - local NVIDIA host with `NVIDIA GeForce RTX 4080`
- API flow:
  - `POST /v1/chat/completions`
  - `127.0.0.1:8099`
  - `-c 4096`
  - `-ngl 999`
  - warm once, time the second request
- Deterministic prompt:
  - system and developer messages match the local benchmark script
  - user asks for exactly one sentence:
    `HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.`

## Measured History

### Original baseline

- Psionic:
  - `37` completion tokens in `2.211s`
  - `16.74 tok/s`
- `llama.cpp`:
  - `42` completion tokens in `0.245s`
  - `171.22 tok/s`
- Gap:
  - `10.23x`

### Current checkpoint

- Psionic:
  - `37` completion tokens in `1.049s`
  - `35.26 tok/s`
- `llama.cpp`:
  - `42` completion tokens in `0.251s`
  - `167.27 tok/s`
- Gap:
  - `4.74x`
- Psionic improvement over original baseline:
  - `2.11x`

### Direct-alignment checkpoint

- Psionic:
  - `37` completion tokens in `1.028s`
  - `35.99 tok/s`
- `llama.cpp`:
  - `42` completion tokens in `0.252s`
  - `166.74 tok/s`
- Gap:
  - `4.63x`
- Psionic improvement over the prior checkpoint:
  - `1.02x`

The visible output text matched exactly in the current benchmark:

`HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.`

## What Landed Between The Two Measurements

The performance gap is smaller than it was, and the reason is concrete.

The current Psionic checkpoint added:

- benchmark JSON summaries and request-level perf receipts
- device-resident CUDA step-plan buffers in `psionic-serve`
- CUDA KV-cache mirrors, CUDA RMSNorm, CUDA RoPE, and CUDA decode attention
- CUDA router top-k softmax and CUDA MoE projection / aggregation surfaces
- a single CUDA submission per generated token instead of one submission per
  layer plus a separate logits submission
- `Q8_1` scratch quantization plus `Q8_0 x Q8_1` fast-path kernels
- a direct MMVQ-style warp-scheduled matvec port for the `Q8_0` and `MXFP4`
  logits/projection path
- a CUDA argmax fast path for the greedy decode lane
- removal of two explicit residual-buffer copies from the steady-state decode
  token path
- an explicit high-level GPT-OSS decode-graph representation in Psionic, with a
  graph-derived CUDA step-plan digest and surfaced decode-graph node counts
- CUDA build tuning in `psionic-backend-cuda/build.rs`:
  - `-O3`
  - `--use_fast_math`
  - architecture detection from `CUDAARCHS`, `PSI_CUDA_ARCH`, or `nvidia-smi`

Three correctness fixes were also necessary before the faster path was usable:

- GGML `Q8_1` block size had to be corrected to `36` bytes, not `34`
- `Q8_1` scratch sizing had to be widened to the maximum actual projection
  width, not just hidden size
- CUDA argmax shared scratch had to be sized for the maximum possible warp count
  per block, not only the original narrow-row case

That work is why Psionic moved from `16.74 tok/s` to `35.26 tok/s` without changing
the benchmark contract or delegating execution to `llama.cpp`.

The direct-alignment checkpoint then added the first explicit ports of
`llama.cpp` CUDA fusion ideas:

- a fused RoPE + KV-write + decode-attention kernel for the GPT-OSS single-token
  path, mirroring the role of `ggml_cuda_should_fuse_rope_set_rows(...)` and the
  backend-owned attention/KV write path in `ggml-cuda.cu`
- a fused residual-add + post-attention RMSNorm kernel that preserves the
  intermediate `ffn_inp` surface instead of only normalizing it
- an `f16` device KV mirror for the Psionic CUDA GPT-OSS path, closer to the
  `llama.cpp` KV cache representation
- backend CUDA tests that check the fused attention path against the separate
  Psionic RoPE + attention + cache-write path before using it in the live model

That work materially reduced kernel count but did not materially change
end-to-end throughput. That is important evidence, not a failure to record.

## Current Hard Evidence From The Psionic Benchmark

The latest Psionic benchmark receipt for the timed request reports:

- `step_count = 37`
- `layer_visit_count = 888`
- `graph_node_count = 266`
- `graph_layer_node_count = 11`
- `host_to_device_bytes = 426240`
- `device_to_host_bytes = 148`
- `submission_count = 37`
- `sync_count = 37`
- `kernel_launches = 11692`

Important interpretation:

- the execution-boundary work helped a lot
  - decode is down to one submission and one sync per generated token
- host-to-device traffic is no longer the main problem
  - about `11.5 KB` per token in the timed lane
- full-logits readback is no longer the dominant problem either
  - the greedy lane now reads back only one token ID per step
- kernel launch count is lower than the previous checkpoint, but still very high
  - about `316` launched operations per generated token
- the direct `llama.cpp`-style fusion slice cut kernel launches by about `27.5%`
  - from `16132` to `11692`
- despite that launch reduction, tok/s moved only from about `35.5` to `36.0`
  - that is strong evidence that the remaining gap is now mostly inside the
    heavyweight attention/MoE kernels and the graph scheduler, not in small
    per-op overhead alone
- throughput barely changed after removing almost all logits readback
  - that is the strongest evidence that the remaining gap is now mostly
    compute-shape, fusion, and dispatch architecture
- the first explicit decode-graph alignment step did not materially change
  throughput by itself
  - the latest rerun stayed at about `35.50 tok/s`
  - that is expected because the change made the graph shape explicit and
    testable, but did not yet add new fusion or kernel families
- the first direct CUDA fusion ports also did not materially change throughput
  by themselves
  - the latest rerun stayed at about `35.99 tok/s`
  - that means further parity work has to target `llama.cpp`'s bigger wins:
    flash attention, grouped `mul_mat_id`, CUDA graph capture/reuse, and the
    backend scheduler's fusion/dispatch policy

Timing caveat:

- the current stage timing fields are trustworthy for broad direction but not
  for precise per-stage attribution after the single-submission rewrite
- the byte counters, submission count, sync count, kernel-launch count, and
  end-to-end benchmark numbers are the solid evidence to use for optimization
  decisions right now

## What `llama.cpp` Actually Does For GPT-OSS

The most important thing about `llama.cpp` is not one individual CUDA kernel.
It is the way the GPT-OSS path is represented and scheduled end to end.

### 1. The GPT-OSS model path is built as one explicit graph

`~/code/llama.cpp/src/models/openai-moe-iswa.cpp` builds the model in the exact
decode order that matters for throughput:

- input embedding
- RMSNorm
- Q / K / V projection
- RoPE on Q and K
- attention through `build_attn(...)`
- residual add
- post-attention RMSNorm
- MoE through `build_moe_ffn(...)`
- residual add
- output RMSNorm
- final lm-head projection

That graph is not just descriptive. It is the input to the scheduler and the
backend fusion logic.

### 2. Attention and MoE are graph-level constructs, not ad-hoc side paths

`~/code/llama.cpp/src/llama-graph.cpp` does three throughput-critical things
that Psionic still does only partially:

- `build_attn(...)` routes eligible shapes through `ggml_flash_attn_ext(...)`
  instead of keeping attention as a simple hand-authored decode kernel
- `build_moe_ffn(...)` builds the full gating path, top-k expert selection,
  expert weights, grouped expert projections, OAI SWIGLU, and expert
  aggregation as graph nodes that the CUDA backend can reason about
- the grouped-expert path uses `ggml_mul_mat_id(...)` and expert views so the
  backend sees the routed-expert structure directly instead of reconstructing it
  from Rust-side imperative sequencing

### 3. The context and scheduler are designed around graph reserve, reuse, and capture

`~/code/llama.cpp/src/llama-context.cpp` reserves graphs, computes them through
`ggml_backend_sched_graph_compute_async(...)`, reports graph-node and graph-split
counts, and maintains reuse-oriented graph/result state. On this host the live
server reported:

- `graph nodes = 1352`
- `graph splits = 2`
- `USE_GRAPHS = 1`

That is materially different from Psionic's current "encode every token's ops
from Rust into one submission" design.

### 4. CUDA fusion and dispatch are driven by the graph, not by isolated call sites

`~/code/llama.cpp/ggml/src/ggml-cuda/ggml-cuda.cu` is where the bigger
throughput story lives:

- `ggml_cuda_should_fuse_mul_mat_vec_q(...)` decides when MMVQ is the right
  quantized path
- `ggml_cuda_mul_mat_id(...)` chooses grouped-expert fast paths and only falls
  back when the graph shape or hardware disqualifies them
- the backend fuses top-k MoE selection, `mul_mat(+id)` plus bias plus GLU,
  RMSNorm patterns, and RoPE-plus-KV write patterns when the graph layout
  permits it
- CUDA graph execution and concurrent-event scheduling are part of the backend
  machinery, not a product-layer afterthought

That is the architecture Psionic still lacks.

## Exact Reason Psionic Is Still Slower

This is no longer a generic "CPU vs GPU" story. The current gap is now mostly an
execution-architecture story.

### 1. Psionic still encodes decode as Rust-owned imperative work, not as a true backend graph

Psionic now has a reusable CUDA step plan and only one submission per token.
That part is real progress.

But the hot path in `crates/psionic/psionic-serve/src/gpt_oss.rs` still manually
walks every layer and explicitly emits the sequence of kernels from Rust each
token. `llama.cpp` instead builds the OpenAI-MoE path as a graph and lets the
backend decide:

- which nodes can fuse
- which kernels to dispatch
- when CUDA graphs are reusable
- how concurrent regions are scheduled

One submission per token is not the same thing as `llama.cpp`'s graph-driven
decode architecture.

### 2. The current MMVQ port is only a slice of the real `llama.cpp` kernel story

The recent Psionic MMVQ-style port was correct and worthwhile, but it is still
only a partial analogue of `mmvq.cu` plus `vecdotq.cuh`.

What `llama.cpp` has that Psionic still does not:

- full dispatch policy around MMVQ vs MMQ, not just one replacement kernel
- fusion-aware matvec use inside bias and GLU subgraphs
- grouped-expert `mul_mat_id` integration
- broader architecture-aware parameter selection

Psionic improved the row kernel. `llama.cpp` improves the whole graph around that
kernel.

### 3. Psionic does not yet mirror `llama.cpp`'s grouped-expert execution model

`llama.cpp`'s `build_moe_ffn(...)` plus `ggml_mul_mat_id(...)` and
`ggml-cuda/mmid.cu` compact work by expert and keep the grouped-expert structure
visible to the backend.

Psionic's current MoE path is still more imperative and less compact:

- routing and execution are CUDA-backed, but not yet driven by the same grouped
  expert graph shape
- the real-model `MXFP4` fast path is still not trustworthy enough to be the
  universal live path
- the remaining kernel-launch count strongly suggests too much small-grain work
  is still being emitted

This is a major reason Psionic remains far behind on the exact GPT-OSS 20B model,
whose expert lane is where `MXFP4` matters most.

### 4. Psionic attention is CUDA-backed, but not `fattn.cu`-class

The current Psionic attention path is much better than the old CPU fallback, but
it is still a simpler decode kernel than the `llama.cpp` flash-attention path.

`llama.cpp` does not just have "a faster attention kernel." It has:

- graph-level `build_attn(...)` integration
- `ggml_flash_attn_ext(...)` eligibility and dispatch
- architecture-specific kernel families inside `fattn.cu`

Psionic is still missing that full stack.

### 5. CUDA graph capture and fusion reuse are still a `llama.cpp` advantage

The live `llama.cpp` run on this host reported `USE_GRAPHS = 1`, and the backend
code clearly separates capture/update logic from ordinary graph execution.

Psionic currently has:

- reusable buffers
- reusable step-plan structures
- one submission per token

Psionic does not yet have the same class of:

- decode-graph identity
- capture/update/reuse contract
- graph-driven fusion regions
- concurrent stream/event scheduling

That missing layer is why the remaining work should start with a graph/fusion
alignment issue before more micro-kernel tweaking.

### 6. This is not a GGUF or Harmony semantics problem

The current gap is not caused by prompt rendering or GPT-OSS parsing semantics.
Those have already been aligned well enough that:

- the exact visible benchmark output matches
- the model loads and runs through the Psionic-owned GGUF path
- Harmony structure survives the real HTTP flow

That matters because it narrows the refactor target: the remaining gap is in the
CUDA/runtime architecture, not the GPT-OSS format semantics.

## What Should Be Ported Directly

If the goal is to bring Psionic exactly in line with `llama.cpp`, these are the
right direct-port candidates:

- `ggml-cuda/vecdotq.cuh`
  - the quantized vector-dot implementations and VDR choices for GPT-OSS-relevant
    types
- `ggml-cuda/mmvq.cu`
  - warp/row scheduling and MMVQ dispatch shape
- `ggml-cuda/mmid.cu`
  - expert-ID compaction and grouped-expert execution helpers
- `ggml-cuda/fattn.cu`
  - the attention-kernel families and their eligibility rules for the supported
    GPT-OSS dimensions
- relevant subgraph-fusion decisions in `ggml-cuda/ggml-cuda.cu`
  - top-k MoE fusion
  - `mul_mat(+id)` plus bias / GLU fusion
  - RMSNorm fusion
  - RoPE / KV-write fusion

What should remain Psionic-owned in Rust:

- the product-facing HTTP surface
- GGUF model loading and truth metadata
- Harmony prompt/render/parse behavior
- observability, receipts, and benchmark evidence
- a Rust graph/runtime representation that mirrors `llama.cpp` semantically
  without making `llama.cpp` a runtime dependency

## Clear Path To llama.cpp-Class Speed

The remaining sequence should be executed in this order:

1. Land `#3249`
   - mirror `openai-moe-iswa.cpp`, `llama-graph.cpp`, `llama-context.cpp`, and
     the relevant `ggml-cuda.cu` fusion/dispatch rules in the Psionic-owned
     runtime architecture
2. Land `#3247`
   - port the relevant `llama.cpp` CUDA kernels and dispatch policy directly,
     especially `vecdotq.cuh`, `mmvq.cu`, `mmid.cu`, and `fattn.cu`
3. Keep `#3248` open until the benchmark contract is actually met
   - same model
   - same host
   - same HTTP flow
   - same visible output
   - within `20%` of `llama.cpp`

The important change in direction is this:

- the next work should not be "find another isolated hotspot"
- the next work should be "make Psionic's GPT-OSS CUDA/runtime architecture look
  like `llama.cpp` on purpose"

## Honest Status

Psionic is no longer "nowhere close." The current checkpoint is a real improvement:

- the exact Psionic-owned GPT-OSS HTTP flow works
- the visible benchmark output matches `llama.cpp`
- Psionic is about `2.11x` faster than it was at the start of the perf track
- eliminating logits readback for the greedy lane proved that readback was no
  longer the primary limiter

But Psionic is still not in the same speed class as `llama.cpp` on this host.
The current measured gap is still large enough that `#3249`, `#3247`, and
`#3248` must remain open.

The correct summary is:

- the format, prompt, and execution-boundary issues are mostly solved
- the remaining gap is now mostly graph/fusion architecture, grouped-expert
  execution quality, attention-kernel quality, and direct CUDA kernel parity
