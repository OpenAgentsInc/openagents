# Psionic Metal GPT-OSS Unified Plan

> Status: draft plan, 2026-03-09
>
> Purpose: replace the current "deferred Metal GPT-OSS lane" posture with one
> unified implementation plan that is grounded in the latest Psionic CUDA
> learnings and the current `llama.cpp` Metal architecture.
>
> Execution note: keep this planning and backlog track on the current working
> branch; do not switch to or create a side branch for this doc-and-issue pass.

## Why This Doc Exists

`crates/psionic/docs/ROADMAP.md` now captures a large amount of real GPT-OSS
parity work on NVIDIA. The main lesson from that work is that the remaining gap
to `llama.cpp` is not one isolated kernel or one host copy. The bigger win comes
from matching the whole production path:

- shared model graph shape
- backend-owned fusion and dispatch
- graph reserve and reuse
- device-resident KV and prompt-prefix state
- backend-side greedy sampling when raw logits are not needed

Psionic should apply those lessons to Metal directly, instead of building a
second Metal-only ad-hoc GPT-OSS runtime that later needs to be replaced.

## Sources Reviewed

### Psionic

- `crates/psionic/docs/ROADMAP.md`
- `crates/psionic/docs/HARDWARE_VALIDATION_MATRIX.md`
- `crates/psionic/docs/EXO_UNIFIED_INTEGRATION_PLAN.md`
- `crates/psionic/psionic-backend-metal/src/lib.rs`
- `crates/psionic/psionic-backend-cuda/src/lib.rs`
- `crates/psionic/psionic-serve/src/gpt_oss.rs`
- `crates/psionic/psionic-serve/src/lib.rs`
- `crates/psionic/psionic-serve/tests/metal_text_generation_parity.rs`
- GitHub issue `#3249` and its 2026-03-09 checkpoint comments

### `llama.cpp`

- `~/code/llama.cpp/src/models/openai-moe-iswa.cpp`
- `~/code/llama.cpp/src/llama-context.cpp`
- `~/code/llama.cpp/ggml/src/ggml-metal/ggml-metal-context.m`
- `~/code/llama.cpp/ggml/src/ggml-metal/ggml-metal-common.cpp`
- `~/code/llama.cpp/ggml/src/ggml-metal/ggml-metal-ops.cpp`
- `~/code/llama.cpp/ggml/src/ggml-metal/ggml-metal-device.cpp`
- `~/code/llama.cpp/ggml/src/ggml-metal/ggml-metal.metal`

The main GitHub audit thread that currently captures the architectural parity
lessons is `#3249`. I did not find a separate fresher Metal-specific GPT-OSS
issue thread that supersedes it.

## Current Truth

### Psionic Metal today is not a real GPT-OSS backend

Current code truth:

- `psionic-backend-metal` exposes only the dense surface
  `["input", "constant", "matmul", "add"]`.
- `TEXT_GENERATION_SUPPORTED_OPS` is currently identical to the embeddings
  surface.
- quantized constant storage is rejected in Metal today.
- the only compiled Metal kernels in the current backend are the inline
  `psionic_add` and `psionic_matmul` pipelines.
- `MetalModelTextGenerationService` loads
  `ArtifactWordDecoder::from_safetensors_artifact(...)`, not a GGUF GPT-OSS
  model.
- `metal_text_generation_parity.rs` validates the small dense artifact-backed
  generation graph, not the GPT-OSS GGUF path.
- `HARDWARE_VALIDATION_MATRIX.md` has no Metal GPT-OSS row.
- `EXO_UNIFIED_INTEGRATION_PLAN.md` correctly treats Metal GPT-OSS as deferred
  and ineligible for cluster execution today.

This means Psionic should not describe Metal as "almost there" for GPT-OSS.
The current Metal text-generation lane is a first dense product path, not a
real OpenAI-MoE execution backend.

### Psionic CUDA now contains the right lessons, but not yet the final shape

The current CUDA lane already moved closer to `llama.cpp`:

- explicit GPT-OSS decode graph signature
- q8_1 fast paths for `Q8_0` and `MXFP4`
- device-resident `f16` KV mirror
- shared-prefix residency and prompt reuse
- CUDA graph replay for steady-state decode
- backend argmax fast path for greedy decode
- async graph-owned host staging

The latest same-host checkpoint in `ROADMAP.md` and `#3249` is:

- Psionic: about `92.32 tok/s`
- `llama.cpp`: about `166.46 tok/s`

The remaining NVIDIA gap is now concentrated in the exact areas that matter for
Metal too:

- ids-enabled `mul_mat_vec_q` / grouped expert dispatch parity
- greedy-logits path that avoids unnecessary full-logit materialization
- graph/runtime policy parity rather than more isolated micro-optimizations

The important planning implication is: Metal should inherit the good CUDA
lessons, but it should not clone the current CUDA-specific Rust step engine as
its long-term architecture.

### Carry-Over Lessons From The CUDA Parity Track

The 2026-03-09 CUDA checkpoints in `ROADMAP.md` and `#3249` already ruled out a
few tempting but losing directions. Metal should not spend early effort
repeating them.

Lessons to carry over:

- device-resident KV and prompt-prefix residency mattered
- graph replay and stable decode shape mattered
- backend-side argmax mattered for greedy decode correctness and honesty, even
  if it was not the whole gap
- more literal `llama.cpp` dispatch behavior is now a better bet than local
  substitutes

Detours already shown to be weak or losing on NVIDIA:

- treating logits readback as the main remaining bottleneck
- replacing the GPT-OSS grouped MoE path with a simpler generic per-expert
  MMVQ or atomic route
- routing projection work through dense `f16` mirrors in hopes that a generic
  GEMV path will beat the model-specific quantized dispatch
- spending more time on host-copy cleanup alone after async graph-owned staging
  is already in place

That does not prove Metal will have identical bottlenecks, but it is strong
enough evidence to bias Metal toward the proven `llama.cpp` architecture first.

### `llama.cpp` already shows the target architecture

From the reviewed `llama.cpp` sources:

- `openai-moe-iswa.cpp` builds GPT-OSS as one backend-agnostic OpenAI-MoE graph
  shape.
- `llama-context.cpp` reserves worst-case graphs, reuses graph shapes when
  possible, and avoids raw-logit copies when backend samplers can own the
  output.
- `ggml-metal-context.m` uses fusion, concurrency, graph optimization, and
  multiple command buffers.
- `ggml-metal-common.cpp` reorders graphs for safe concurrency.
- `ggml-metal-ops.cpp` and `ggml-metal-device.cpp` expose the GPT-OSS-relevant
  Metal primitives already missing in Psionic:
  `MUL_MAT_ID`, `MUL_MV_ID`, `RMS_NORM`, `ROPE`, `FLASH_ATTN_EXT`, `ARGMAX`,
  and `TOP_K`.
- `ggml-metal.metal` contains a broad quantized kernel family rather than a
  dense-only shader pair.

So the proven reference is not "a faster matmul." It is "shared graph model +
runtime reserve/reuse + wide backend primitive surface + backend-side sampling."

## Target Architecture

The Metal plan should aim for the following steady-state shape.

### 1. Shared GPT-OSS graph contract

Psionic should have one reusable GPT-OSS graph/runtime contract owned in
`crates/psionic/*`, not one CUDA-specific step engine and then a second Metal
copy. The high-level node ordering should stay aligned with the OpenAI-MoE graph
already mirrored from `openai-moe-iswa.cpp`:

- `attn_norm`
- `attn_qkv`
- `attn_q_rope`
- `attn_k_rope`
- `attn_out`
- `ffn_inp`
- `attn_post_norm`
- `ffn_moe_topk`
- `ffn_moe_gate_up`
- `ffn_moe_down`
- `ffn_moe_out`
- `result_norm`
- `result_output`

### 2. Backend-owned lowering and dispatch

The shared graph must lower into backend policy, not force one Rust-owned token
loop to encode every kernel choice by hand. Metal needs its own lowering and
dispatch layer for:

- quantized MMVQ or MMQ selection
- grouped expert `mul_mat_id` or `mul_mv_id` style dispatch
- flash-attention enablement
- norm, rope, and residual fusion rules
- backend-side output selection for greedy decode

### 3. Device-resident GPT-OSS state

Metal GPT-OSS should keep the hot inference state on the GPU:

- quantized weights in backend-native storage
- KV cache on device
- reusable prompt-prefix residency on device
- stable per-shape staging buffers

Rebuilding prompt state from host memory on every repeated request would repeat
the exact limitation already discovered on NVIDIA.

### 4. Shape-stable reserve and reuse

Metal needs a real prompt/decode runtime with:

- graph reserve for worst-case prompt and decode shapes
- shape identity for reuse and invalidation
- reusable command-buffer or command-encoder strategy
- explicit graph rebuild evidence when shapes or residency break reuse

### 5. Backend-side greedy sampling

When decode is greedy and no penalties require raw logits, Metal should not
materialize full logits to the host just to take argmax. The default fast path
should be:

- keep logits on device
- run `argmax` or `top_k` on device
- copy back only the chosen token or bounded candidate set

### 6. Truthful capability surfaces

Until the real path exists and is validated:

- Metal GPT-OSS must stay explicit refusal or not-yet-validated
- cluster placement must continue refusing Metal GPT-OSS execution
- the existing dense Metal text-generation claim must not be misread as GPT-OSS

## What Not To Do

- Do not shell out to or wrap `llama.cpp` for execution.
- Do not ship a Metal GPT-OSS claim before the validation matrix has an explicit
  row for it.
- Do not port the current CUDA serve-layer step engine line-for-line into a
  permanent Metal fork.
- Do not solve this by host dequantization or CPU-side sampling hidden behind a
  "Metal" label.
- Do not use cluster work to paper over a missing single-node Metal backend.

## Dependency-Ordered Implementation Plan

### Phase 0: Fix truth and create the right seam

1. Separate the current dense Metal text-generation claim from the future Metal
   GPT-OSS claim.
2. Add explicit GPT-OSS refusal or unsupported reporting for Metal today.
3. Lift the GPT-OSS graph/runtime contract out of a CUDA-specific serve-layer
   shape into a backend-agnostic Psionic seam.
4. Keep CUDA using that seam too, so Metal is not born on a dead-end path.

Exit criteria:

- the repo no longer implies that current Metal text generation equals Metal
  GPT-OSS
- there is one shared GPT-OSS graph/runtime contract that both CUDA and Metal
  can target

### Phase 1: Build the minimum Metal GPT-OSS primitive surface

1. Add Metal backend storage and upload support for GPT-OSS quantized weights.
2. Add Metal kernels and runtime plumbing for:
   - RMSNorm
   - RoPE
   - argmax
   - top-k
   - grouped-id expert matvec or matmul
3. Add Metal buffer pooling, kernel-cache, and memory-budget policy suitable
   for token-generation workloads.

Exit criteria:

- Metal can hold GPT-OSS weights without eager CPU dequantization
- the missing GPT-OSS control-path primitives exist on Metal

### Phase 2: Build the real Metal decode path

1. Add device-resident KV cache for Metal GPT-OSS.
2. Add device-resident shared-prefix residency for repeated prompts.
3. Implement Metal decode attention with a `llama.cpp`-class flash-attention
   path where the Apple GPU supports it.
4. Implement grouped expert execution that mirrors `llama.cpp`'s id-driven MoE
   path closely enough to avoid the current generic fallback shape.
5. Add backend-side greedy output selection so steady-state decode avoids full
   host logits readback.

Exit criteria:

- repeated-request GPT-OSS decode can reuse prompt state on device
- greedy decode copies back only bounded output data
- the hot token loop is no longer composed from dense-only fallbacks

### Phase 3: Match `llama.cpp` runtime behavior

1. Reserve prompt and decode graphs explicitly.
2. Reuse stable shapes across requests and decode steps.
3. Add graph optimization or node reordering where needed for Metal
   concurrency.
4. Use multiple command buffers or equivalent concurrent submission policy when
   it measurably helps on Apple hardware.
5. Surface reuse, rebuild, cache-hit, and kernel-dispatch evidence in Psionic
   performance receipts.

Exit criteria:

- Metal GPT-OSS has visible reserve/reuse behavior rather than one-off command
  submission
- performance receipts can explain whether a run was cold, warm, reused, or
  rebuilt

### Phase 4: Ship the product lane and benchmark it honestly

1. Add `MetalGgufGptOssTextGenerationService`.
2. Add seeded CPU-vs-Metal GPT-OSS correctness tests.
3. Add warm repeated-request and prompt-cache-hit throughput tests or runbook
   scripts.
4. Add a real validation-matrix row for Metal GPT-OSS only after the above is
   green.
5. Revisit cluster eligibility only after single-node Metal GPT-OSS is real and
   validated.

Exit criteria:

- Psionic serves GGUF GPT-OSS on Metal through a real product path
- hardware validation and receipt truth match reality

## Benchmark Contract

Metal GPT-OSS should be measured against same-moment `llama.cpp` on the same
Apple Silicon host, same GGUF, same prompt contract, and same sampling mode.

The benchmark contract should include at least:

- cold first request
- warm repeated request with identical prompt
- prompt-cache-hit request
- greedy decode
- non-greedy decode if penalties or sampling options force raw logits

The closure target should be relative to the same-host control, not one fixed
absolute number across all Macs:

- minimum ship gate: at least `85%` of same-moment `llama.cpp` tok/s on the
  tracked Apple host for the benchmark contract above
- stretch target: at least `90%`

Absolute tok/s should still be recorded in issue comments and docs, but the
ship gate should stay host-relative because Apple GPU classes vary widely.

## Proposed GitHub Issue Backlog

The issues below are dependency-ordered. Names are written so they can be used
directly as GitHub issue titles.

### 1. `[psionic][metal][gpt-oss] Split dense Metal text-generation truth from GPT-OSS readiness`

Description: update capability surfaces, validation docs, and refusal behavior
so the current dense safetensors Metal text-generation lane is not confused with
GGUF GPT-OSS support. Add an explicit Metal GPT-OSS refusal or
`not_yet_validated` path and keep cluster placement aligned with that truth.

### 2. `[psionic][gpt-oss] Move the GPT-OSS graph/runtime contract out of the CUDA-specific serve path`

Description: lift the current decode-graph shape, shape identity, and runtime
planning seam into backend-agnostic Psionic crates so CUDA and Metal share one
GPT-OSS execution contract. The goal is to prevent a Metal-specific fork of the
current Rust token-step engine.

### 3. `[psionic][metal][gpt-oss] Add backend-native quantized tensor storage and upload for MXFP4, Q8_0, and Q8_1 fast paths`

Description: teach `psionic-backend-metal` to upload and hold GPT-OSS quantized
weights and fast-path quantized buffers directly, instead of rejecting
quantized constants or forcing eager CPU dequantization. Include explicit
runtime truth for storage mode and fallback.

### 4. `[psionic][metal][gpt-oss] Add RMSNorm, RoPE, argmax, and top-k primitives to the Metal backend`

Description: implement the minimum non-dense primitive set required by the
GPT-OSS control path on Metal. These should be backend primitives, not
serve-layer special cases, and should be testable independent of the full model.

### 5. `[psionic][metal][gpt-oss] Add grouped expert dispatch with llama.cpp-style mul_mat_id or mul_mv_id semantics`

Description: add the Metal backend path needed for GPT-OSS MoE expert routing
and grouped selected-expert execution. The implementation should mirror
`llama.cpp`'s ids-driven path closely enough to avoid generic per-expert
fallbacks on the real decode geometry.

### 6. `[psionic][metal][gpt-oss] Add device-resident KV cache and shared-prefix residency`

Description: build the Metal analogue of the current CUDA KV mirror and shared
prefix work so repeated GPT-OSS requests can stay device-resident. Include
explicit invalidation, compatibility checks, and cache-hit evidence.

### 7. `[psionic][metal][gpt-oss] Implement Metal decode attention with a flash-attention path`

Description: add the real Metal attention path for GPT-OSS decode, including
RoPE-aware decode execution and a `llama.cpp`-class flash-attention path where
Apple hardware supports it. The design should prefer backend fusion and device
residency over piecing attention together from dense generic ops.

### 8. `[psionic][metal][gpt-oss] Add backend-side greedy sampling and bounded output readback`

Description: when the request only needs greedy decode, keep logits on device
and return only the chosen token or bounded candidate data. Full raw-logit
materialization should remain available only when the request semantics actually
need it.

### 9. `[psionic][metal][gpt-oss] Add graph reserve, graph reuse, and command-buffer reuse for steady-state decode`

Description: give the Metal GPT-OSS lane a real reserve and reuse runtime,
including stable graph identity, rebuild rules, and reusable command submission
state. Surface reuse and rebuild evidence in performance metrics.

### 10. `[psionic][metal][gpt-oss] Add Metal allocator, buffer-pool, and kernel-cache policy for token generation`

Description: implement the Metal memory-policy pieces needed for stable token
generation throughput: bounded buffer pooling, kernel cache policy, and a
truthful memory-budget view that can explain admission and reuse behavior.

### 11. `[psionic][metal][gpt-oss] Ship MetalGgufGptOssTextGenerationService through the shared runtime`

Description: add the actual product-facing GGUF GPT-OSS Metal service and wire
it through the shared GPT-OSS runtime seam instead of the current dense
artifact-backed service path.

### 12. `[psionic][metal][gpt-oss] Add CPU-vs-Metal GPT-OSS parity, validation, and benchmark evidence`

Description: add seeded correctness tests, refusal tests, warm-request and
prompt-cache-hit perf evidence, and the validation-matrix row needed to claim
Metal GPT-OSS honestly. Do not mark the lane shipped until this issue lands.

### 13. `[psionic][metal][perf] Reach same-host llama.cpp-class GPT-OSS throughput on Apple Silicon`

Description: final closure issue for the Apple path. Use the benchmark contract
in this doc and drive the Metal runtime to at least `85%` of same-moment
`llama.cpp` tok/s on the tracked host, with a stretch target of `90%`.

## Recommended Execution Order

Recommended first wave:

1. issue 1
2. issue 2
3. issue 3
4. issue 4
5. issue 5

Recommended second wave:

1. issue 6
2. issue 7
3. issue 8
4. issue 9
5. issue 10

Recommended ship wave:

1. issue 11
2. issue 12
3. issue 13

## Bottom Line

The correct goal is not "make Metal run some GPT-OSS math." The goal is to make
Metal follow the same architecture that is already proving out in `llama.cpp`
and that the CUDA parity work has now shown to matter in Psionic:

- shared graph shape
- backend-owned dispatch and fusion
- device-resident KV and prefix state
- graph reserve and reuse
- backend-side output selection
- truthful validation and benchmark evidence

That is the shortest path to both similar architecture and similar tok/s on
Apple hardware.
