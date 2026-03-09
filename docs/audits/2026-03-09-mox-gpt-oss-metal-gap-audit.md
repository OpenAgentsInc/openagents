# 2026-03-09 Mox GPT-OSS Metal Gap Audit

> Historical note: this audit is a point-in-time snapshot from 2026-03-09. Current product and architecture authority lives in `docs/MVP.md`, `docs/OWNERSHIP.md`, and `crates/mox/docs/ROADMAP.md`. File paths and implementation-status claims here may be superseded by later commits.

## Scope

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `crates/mox/docs/ROADMAP.md`
- `docs/audits/2026-03-08-mox-vs-llama-cpp-gpt-oss-performance-audit.md`
- `crates/mox/mox-ir/src/lib.rs`
- `crates/mox/mox-compiler/src/lib.rs`
- `crates/mox/mox-backend-metal/src/lib.rs`
- `crates/mox/mox-serve/src/gpt_oss.rs`
- `crates/mox/mox-serve/src/openai_http.rs`
- `crates/mox/mox-serve/src/bin/mox-gpt-oss-server.rs`
- `crates/mox/mox-runtime/src/validation.rs`
- `crates/mox/docs/HARDWARE_VALIDATION_MATRIX.md`
- `~/code/llama.cpp/src/llama-model.cpp`
- `~/code/llama.cpp/src/llama-graph.cpp`
- `~/code/llama.cpp/src/llama-graph.h`
- `~/code/llama.cpp/ggml/include/ggml-metal.h`
- `~/code/llama.cpp/ggml/src/ggml-metal/ggml-metal.m`
- `~/code/llama.cpp/ggml/src/ggml-metal/ggml-metal.metal`

## Executive Summary

The previous version of this audit treated the current Mox CUDA GPT-OSS path as
an acceptable architectural template for Metal. That is the wrong target if the
goal is a proper Apple path.

After inspecting `~/code/llama.cpp`, the right reference is clear:

- `llama.cpp` does not run GPT-OSS on Metal by bouncing through a host-owned
  decode loop.
- It builds OpenAI-MoE / GPT-OSS as a backend graph in `llama-model.cpp` and
  `llama-graph.cpp`.
- It executes that graph through a broad Metal backend with kernels for
  quantized `get_rows`, `mul_mat` / `mul_mat_id`, RMSNorm, RoPE, softmax, and
  `flash_attn_ext`, with capability-gated pipeline selection in
  `ggml-metal.m` and `ggml-metal.metal`.

So the correct Mox plan is not "port the current CUDA Rust loop to Metal." The
correct plan is:

1. move GPT-OSS execution toward a reusable graph/lowering path inside
   `crates/mox/*`,
2. expand Mox IR/compiler coverage to express the GPT-OSS/OpenAI-MoE decode
   graph more like `llama.cpp` does,
3. expand `mox-backend-metal` to execute that graph with backend-specialized
   kernels and device-resident state, and
4. only then wire the Apple GPT-OSS serve/HTTP/validation lane on top.

The current CUDA GPT-OSS path can remain a correctness baseline and shipped
NVIDIA path. It should not dictate the Metal architecture.

## Reference Findings From `llama.cpp`

### 1. GPT-OSS / OpenAI-MoE is assembled as a backend graph

`llm_build_openai_moe_iswa` in `~/code/llama.cpp/src/llama-model.cpp` builds
the GPT-OSS model as graph ops, not as a serve-layer token loop. The graph
contains:

- embedding `get_rows`
- Q/K/V projections
- RoPE
- backend attention
- RMSNorm
- MoE routing and expert execution
- output projection

The model logic is expressed in graph-building helpers rather than being
hard-coded as a backend-specific HTTP service.

### 2. MoE routing and expert execution are graph-native

`build_moe_ffn(...)` in `~/code/llama.cpp/src/llama-graph.cpp` keeps the MoE
path inside the graph with:

- expert logits from matrix multiply
- `soft_max`
- `top_k`
- `get_rows` to gather selected expert weights
- `mul_mat_id` for expert-indexed grouped matmuls
- graph-side weighting and aggregation

That is materially different from the current Mox GPT-OSS CUDA path, which
chooses experts and accumulates outputs in Rust.

### 3. Attention is also graph-native and backend-dispatched

`build_attn_mha(...)` in `~/code/llama.cpp/src/llama-graph.cpp` routes
attention through graph ops and uses `ggml_flash_attn_ext(...)` when the path
is available. RoPE is also expressed as graph ops via `ggml_rope_ext(...)`.

This keeps Q/K/V, RoPE, attention, and output projection inside the backend
execution boundary instead of crossing back to host code between substeps.

### 4. The Metal backend has broad, explicit kernel coverage

The Metal backend in `~/code/llama.cpp/ggml/src/ggml-metal/ggml-metal.m` and
`~/code/llama.cpp/ggml/src/ggml-metal/ggml-metal.metal` includes explicit
kernel tables and dispatch logic for the kinds of ops GPT-OSS needs:

- quantized `get_rows_q8_0`
- quantized `get_rows_mxfp4`
- RMSNorm kernels
- multiple RoPE variants including NeoX-style layouts
- softmax kernels
- `MUL_MAT_ID`
- `FLASH_ATTN_EXT`

This is not a minimal dense surface. It is a real backend execution layer.

### 5. `llama.cpp` gates Metal behavior on device capabilities

The Metal backend checks feature families such as simdgroup reduction,
simdgroup matrix support, BF16 support, and other device traits before enabling
specific kernels and execution paths.

That matters for Mox because a truthful Apple path is not just "macOS yes/no."
It needs claimable capability classes and explicit refusal or degraded behavior
when required kernel families are unavailable.

## Current Mox Reality

### 1. Mox GPT-OSS is still anchored in `mox-serve`

The real GPT-OSS implementation lives in
`crates/mox/mox-serve/src/gpt_oss.rs`. The shipped path is a serve-layer-owned
decoder with CPU and CUDA variants.

That is enough for the current NVIDIA milestone. It is not the right layer if
the goal is a llama.cpp-like Metal architecture.

### 2. Mox Metal text generation is still the earlier dense lane

The shipped Metal path is `MetalModelTextGenerationService` in
`crates/mox/mox-serve/src/lib.rs`. It executes a small dense
`ArtifactWordDecoder` graph. `mox-backend-metal` currently advertises
`TEXT_GENERATION_SUPPORTED_OPS = ["input", "constant", "matmul", "add"]`.

That path is real, but it is not remotely close to a GPT-OSS backend surface.

### 3. Mox Metal still refuses quantized GGUF constant storage

`buffer_from_tensor_data(...)` in `crates/mox/mox-backend-metal/src/lib.rs`
explicitly rejects `TensorData::QuantizedBlocks(...)`.

That alone prevents the current Metal backend from hosting GPT-OSS GGUF weights
the way `llama.cpp` Metal does.

### 4. Mox IR only covers part of the needed graph today

`mox-ir` already has reusable extension ops for:

- `rms_norm`
- `layer_norm`
- `rotary_embedding`
- `scaled_dot_product_attention`
- `quantized_matmul`

That is useful substrate, but it is still missing major GPT-OSS graph pieces
that show up directly in the `llama.cpp` reference:

- quantized `get_rows` / gather
- graph-native `softmax`
- graph-native `top_k` / argsort-style selection
- expert-indexed grouped matmul like `mul_mat_id`
- indexed bias add like `add_id`
- graph-native OpenAI-style SwiGLU MoE helper or equivalent primitive sequence

So "add Metal kernels" is not enough. The Mox graph surface itself still needs
to grow.

### 5. `mox-backend-metal` does not yet expose backend-specialized extension truth

`mox-backend-metal` currently inherits the default empty
`extension_support()` behavior from `mox-runtime`. That means the backend does
not yet truthfully advertise specialized support for the extension families Mox
already models in IR.

For a proper GPT-OSS lane, that backend-truth gap needs to close.

### 6. The GPT-OSS server surface is CUDA-only

`crates/mox/mox-serve/src/openai_http.rs` defines
`GptOssCudaOpenAiCompatServer` and `GptOssCudaWorker`, and
`crates/mox/mox-serve/src/bin/mox-gpt-oss-server.rs` only constructs that CUDA
server.

That is expected for the current NVIDIA milestone. It is not sufficient for a
future Apple path.

### 7. The current Metal validation row is too broad for GPT-OSS

`mox-runtime` and `crates/mox/docs/HARDWARE_VALIDATION_MATRIX.md` currently
treat `metal + mox.text_generation` as the shipped claim
`metal.text_generation.apple_silicon`, backed by the dense artifact tests.

That is truthful for the current first Metal text-generation lane, but not for
GPT-OSS/OpenAI-MoE. A proper Apple GPT-OSS path needs its own validation story,
not a silent reuse of the existing dense claim.

## What A Proper Metal GPT-OSS Track Actually Needs

### 1. Stop using the current CUDA serve loop as the Metal template

The current Mox CUDA GPT-OSS path is explicitly the architecture the user does
not want copied. The Metal track should not center on new types like
`MetalQuantizedMatrix::matvec(...)` or `MetalQuantizedExpertTensor::expert_matvec(...)`
as the end-state design.

Those helpers may still exist as temporary scaffolding, but if the stated goal
is "proper Metal integration like llama.cpp," they are not the plan.

### 2. Move GPT-OSS execution toward reusable graph/lowering ownership

The llama.cpp reference puts model execution shape in reusable graph/model
layers, not in the HTTP service. Mox should move in the same direction:

- `mox-models` should continue owning GPT-OSS/OpenAI-MoE metadata and tensor
  layout truth
- `mox-ir` / `mox-compiler` should gain the graph vocabulary and lowering
  needed to represent GPT-OSS execution
- `mox-backend-metal` should own the backend execution truth for that graph
- `mox-serve` should become the consumer of the compiled execution path, not
  the place where the whole decoder is manually orchestrated

That stays inside current ownership boundaries and avoids baking Metal-specific
execution policy into the serve layer.

### 3. Expand Mox IR to cover the missing GPT-OSS graph semantics

To mimic `llama.cpp` more closely, Mox needs graph-level representation for the
operations that are currently only implicit in Rust code or not representable at
all. The minimum honest set is:

- quantized row gather / `get_rows` for token embeddings and selected experts
- graph softmax
- graph top-k or equivalent deterministic expert-selection op
- grouped expert-indexed matmul analogous to `mul_mat_id`
- indexed bias add analogous to `add_id`
- a graph representation for the OpenAI-MoE feed-forward pattern, either as
  new primitives or as a reusable fused extension op

The existing extension set is a start, not the full answer.

### 4. Promote backend-owned attention, norms, and routing on Metal

The `llama.cpp` reference keeps RoPE, RMSNorm, softmax, attention, and most MoE
machinery on the backend side. A proper Mox Metal path should target the same
execution boundary.

That means `mox-backend-metal` needs to grow from dense `matmul/add` into a
backend with specialized execution for:

- quantized gather / `get_rows`
- quantized or dense grouped matmul for experts
- RMSNorm
- RoPE
- softmax
- scaled dot-product attention, ideally with a flash-attention-style path

If those stay as CPU-side Rust loops, the Metal path will inherit the exact
architecture the user rejected.

### 5. Keep weights, activations, and KV state device-resident

`llama.cpp`’s Metal path works as a backend because the graph executor owns the
buffers and command submission lifecycle. Mox needs the same posture for GPT-OSS:

- GGUF quantized tensors uploaded once into backend-owned storage
- activations kept in Metal buffers through the decode step
- KV-cache state managed as backend execution state rather than host vectors
- command submission organized around graph execution, not per-subcall host
  synchronization

This is the largest architectural difference between "real Metal integration"
and "CUDA loop port."

### 6. Add capability-gated kernel selection, not just backend presence

The `llama.cpp` Metal backend gates kernels on concrete device capabilities such
as simdgroup reduction and simdgroup matrix support. Mox should add the same
kind of truth:

- discovery of the relevant Apple-family execution traits
- pipeline selection based on those traits and model head sizes / dtypes
- explicit degraded or refused states when the required path is unavailable

This should flow through `BackendSelection`, runtime resources, diagnostics, and
validation claims.

### 7. Only after that should the Apple GPT-OSS server lane be wired

Once the reusable graph/backend path exists, then the serve layer can add:

- a Metal GPT-OSS service
- a backend-selectable OpenAI-compatible GPT-OSS server
- truthful health/provenance showing `metal`

Doing server wiring before the execution architecture is ready would just hide
the real gap.

## What Should Not Be In The Plan

- Do not make "Metal = current CUDA GPT-OSS loop but with MSL kernels" the main
  roadmap.
- Do not keep CPU attention, CPU routing, CPU aggregation, and CPU KV ownership
  as the intended Apple architecture.
- Do not treat the current dense Metal `mox.text_generation` claim as evidence
  that GPT-OSS is near-done on Apple.
- Do not advertise an Apple GPT-OSS lane as shipped until it has its own
  validation coverage and claim row.

## Recommended Implementation Order

### Phase 1: Graph Surface

- add the missing IR/compiler ops needed to model GPT-OSS/OpenAI-MoE more like
  `llama.cpp`: quantized `get_rows`, softmax, top-k selection, expert-indexed
  grouped matmul, indexed add, and any missing MoE helper ops
- move GPT-OSS execution planning out of the current `mox-serve` manual loop
  into reusable `crates/mox/*` graph/lowering code
- keep a CPU graph-backed reference path only as a correctness oracle, not as
  the target Apple architecture

### Phase 2: Metal Backend Coverage

- add backend-owned quantized GGUF storage on Metal
- add quantized `get_rows` coverage for at least `Q8_0` and `MXFP4`
- add backend-specialized support for the existing extension ops already in
  `mox-ir`
- add grouped expert execution analogous to `mul_mat_id`
- add backend softmax and attention execution, with a flash-attention-style
  path as the intended target
- add truthful `extension_support()` and capability gating in
  `mox-backend-metal`

### Phase 3: GPT-OSS Apple Path

- build a Metal GPT-OSS execution service on top of the reusable compiled path
- add backend-selectable GPT-OSS OpenAI server wiring
- ensure no silent CPU fallback while claiming Metal

### Phase 4: Validation And Claims

- add Apple Silicon end-to-end GPT-OSS tests using the real local GGUF
- update `mox-runtime` validation mapping
- update `crates/mox/docs/HARDWARE_VALIDATION_MATRIX.md`
- add a new roadmap track after `MOX-183` for proper Metal GPT-OSS execution

## Suggested Issue Split

If this becomes the next active track, the cleanest split is:

1. GPT-OSS/OpenAI-MoE graph-surface expansion in `mox-ir` / `mox-compiler`
2. Metal backend kernel and capability expansion for that graph
3. Apple GPT-OSS serve path, validation matrix, and end-to-end proof

That keeps the work aligned with current ownership boundaries:

- reusable model and lowering logic in `crates/mox/*`
- backend truth in `mox-backend-metal`
- serve/HTTP integration in `mox-serve`
- validation truth in `mox-runtime` plus docs

## Bottom Line

If the goal is merely "get some Apple output," a Metal port of the current CUDA
loop would be faster to land. That is not the goal stated here.

If the goal is a proper Metal integration that follows leading practice more
closely, then Mox should treat `llama.cpp` as the architectural reference:
graph-first GPT-OSS execution, backend-owned state, broad Metal kernel
coverage, and capability-gated dispatch. The current CUDA GPT-OSS path should
inform correctness and product truth, not the Metal architecture.
