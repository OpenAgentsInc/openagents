# 2026-03-09 Psionic GPT-OSS Metal Gap Audit

> Historical note: this audit is a point-in-time snapshot from 2026-03-09. Current product and architecture authority lives in `docs/MVP.md`, `docs/OWNERSHIP.md`, and `crates/psionic/docs/ROADMAP.md`. File paths and implementation-status claims here may be superseded by later commits.

## Scope

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `crates/psionic/docs/ROADMAP.md`
- `docs/audits/2026-03-08-psionic-vs-llama-cpp-gpt-oss-performance-audit.md`
- `crates/psionic/psionic-ir/src/lib.rs`
- `crates/psionic/psionic-compiler/src/lib.rs`
- `crates/psionic/psionic-backend-metal/src/lib.rs`
- `crates/psionic/psionic-serve/src/gpt_oss.rs`
- `crates/psionic/psionic-serve/src/openai_http.rs`
- `crates/psionic/psionic-serve/src/bin/psionic-gpt-oss-server.rs`
- `crates/psionic/psionic-runtime/src/validation.rs`
- `crates/psionic/docs/HARDWARE_VALIDATION_MATRIX.md`
- `~/code/llama.cpp/src/llama-model.cpp`
- `~/code/llama.cpp/src/llama-graph.cpp`
- `~/code/llama.cpp/src/llama-graph.h`
- `~/code/llama.cpp/ggml/include/ggml-metal.h`
- `~/code/llama.cpp/ggml/src/ggml-metal/ggml-metal.m`
- `~/code/llama.cpp/ggml/src/ggml-metal/ggml-metal.metal`

## Executive Summary

The previous version of this audit treated the current Psionic CUDA GPT-OSS path as
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

So the correct Psionic plan is not "port the current CUDA Rust loop to Metal." The
correct plan is:

1. move GPT-OSS execution toward a reusable graph/lowering path inside
   `crates/psionic/*`,
2. expand Psionic IR/compiler coverage to express the GPT-OSS/OpenAI-MoE decode
   graph more like `llama.cpp` does,
3. expand `psionic-backend-metal` to execute that graph with backend-specialized
   kernels and device-resident state, and
4. only then wire the Apple GPT-OSS serve/HTTP/validation lane on top.

The current CUDA GPT-OSS path can remain a correctness baseline and shipped
NVIDIA path. It should not dictate the Metal architecture.

Reviewing the live GitHub issue track for the NVIDIA work changes one concrete
thing in this audit: the implementation order should mirror the staged sequence
already being used to dismantle the old host-loop design. The active CUDA track
is:

- `#3243` device-resident activations and submission reuse on CUDA (already
  closed)
- `#3244` device KV, RMSNorm, RoPE, and attention
- `#3245` grouped GPU MoE routing and expert execution
- `#3246` graph-based prefill and decode runtime
- `#3247` kernel tuning toward `llama.cpp`-class throughput
- `#3248` parity closure on the real HTTP path

That issue sequence is materially aligned with the `llama.cpp` reference. So
the Metal track should not only copy the destination architecture; it should
also copy the broad order of operations: device residency first, then hot-path
ops, then MoE, then compiled graph runtime, then kernel tuning, and only then
ship/claim the Apple lane.

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

That is materially different from the current Psionic GPT-OSS CUDA path, which
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

That matters for Psionic because a truthful Apple path is not just "macOS yes/no."
It needs claimable capability classes and explicit refusal or degraded behavior
when required kernel families are unavailable.

## Current Psionic Reality

### 1. Psionic GPT-OSS is still anchored in `psionic-serve`

The real GPT-OSS implementation lives in
`crates/psionic/psionic-serve/src/gpt_oss.rs`. The shipped path is a serve-layer-owned
decoder with CPU and CUDA variants.

That is enough for the current NVIDIA milestone. It is not the right layer if
the goal is a llama.cpp-like Metal architecture.

### 2. Psionic Metal text generation is still the earlier dense lane

The shipped Metal path is `MetalModelTextGenerationService` in
`crates/psionic/psionic-serve/src/lib.rs`. It executes a small dense
`ArtifactWordDecoder` graph. `psionic-backend-metal` currently advertises
`TEXT_GENERATION_SUPPORTED_OPS = ["input", "constant", "matmul", "add"]`.

That path is real, but it is not remotely close to a GPT-OSS backend surface.

### 3. Psionic Metal still refuses quantized GGUF constant storage

`buffer_from_tensor_data(...)` in `crates/psionic/psionic-backend-metal/src/lib.rs`
explicitly rejects `TensorData::QuantizedBlocks(...)`.

That alone prevents the current Metal backend from hosting GPT-OSS GGUF weights
the way `llama.cpp` Metal does.

### 4. Psionic IR only covers part of the needed graph today

`psionic-ir` already has reusable extension ops for:

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

So "add Metal kernels" is not enough. The Psionic graph surface itself still needs
to grow.

### 5. `psionic-backend-metal` does not yet expose backend-specialized extension truth

`psionic-backend-metal` currently inherits the default empty
`extension_support()` behavior from `psionic-runtime`. That means the backend does
not yet truthfully advertise specialized support for the extension families Psionic
already models in IR.

For a proper GPT-OSS lane, that backend-truth gap needs to close.

### 6. The GPT-OSS server surface is CUDA-only

`crates/psionic/psionic-serve/src/openai_http.rs` defines
`GptOssCudaOpenAiCompatServer` and `GptOssCudaWorker`, and
`crates/psionic/psionic-serve/src/bin/psionic-gpt-oss-server.rs` only constructs that CUDA
server.

That is expected for the current NVIDIA milestone. It is not sufficient for a
future Apple path.

### 7. The current Metal validation row is too broad for GPT-OSS

`psionic-runtime` and `crates/psionic/docs/HARDWARE_VALIDATION_MATRIX.md` currently
treat `metal + psionic.text_generation` as the shipped claim
`metal.text_generation.apple_silicon`, backed by the dense artifact tests.

That is truthful for the current first Metal text-generation lane, but not for
GPT-OSS/OpenAI-MoE. A proper Apple GPT-OSS path needs its own validation story,
not a silent reuse of the existing dense claim.

## Cross-Check Against The Active CUDA Issue Track

The open NVIDIA issues make it clear that the CUDA effort is no longer arguing
for the original host-owned GPT-OSS design. It is actively replacing that
design with the same class of architecture this audit recommends for Metal.

That means the Metal plan should change in one important way: the ordering
should be more explicit and should track the proven dependency chain already
being used on CUDA.

The useful mapping is:

1. device-resident activations, quantized storage, KV ownership, and submission
   reuse
2. backend RMSNorm, RoPE, softmax, and attention
3. backend MoE routing, grouped expert execution, and aggregation
4. compiled graph/plan execution for prefill and decode
5. backend kernel tuning toward `llama.cpp`-class throughput
6. real HTTP-path validation, hardware claims, and closure

That is a better implementation order than the earlier version of this audit,
which separated "graph surface" and "backend coverage" too broadly and did not
make the device-residency and hot-path staging explicit enough.

## What A Proper Metal GPT-OSS Track Actually Needs

### 1. Stop using the current CUDA serve loop as the Metal template

The current Psionic CUDA GPT-OSS path is explicitly the architecture the user does
not want copied. The Metal track should not center on new types like
`MetalQuantizedMatrix::matvec(...)` or `MetalQuantizedExpertTensor::expert_matvec(...)`
as the end-state design.

Those helpers may still exist as temporary scaffolding, but if the stated goal
is "proper Metal integration like llama.cpp," they are not the plan.

### 2. Move GPT-OSS execution toward reusable graph/lowering ownership

The llama.cpp reference puts model execution shape in reusable graph/model
layers, not in the HTTP service. Psionic should move in the same direction:

- `psionic-models` should continue owning GPT-OSS/OpenAI-MoE metadata and tensor
  layout truth
- `psionic-ir` / `psionic-compiler` should gain the graph vocabulary and lowering
  needed to represent GPT-OSS execution
- `psionic-backend-metal` should own the backend execution truth for that graph
- `psionic-serve` should become the consumer of the compiled execution path, not
  the place where the whole decoder is manually orchestrated

That stays inside current ownership boundaries and avoids baking Metal-specific
execution policy into the serve layer.

### 3. Expand Psionic IR to cover the missing GPT-OSS graph semantics

To mimic `llama.cpp` more closely, Psionic needs graph-level representation for the
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
machinery on the backend side. A proper Psionic Metal path should target the same
execution boundary.

That means `psionic-backend-metal` needs to grow from dense `matmul/add` into a
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
buffers and command submission lifecycle. Psionic needs the same posture for GPT-OSS:

- GGUF quantized tensors uploaded once into backend-owned storage
- activations kept in Metal buffers through the decode step
- KV-cache state managed as backend execution state rather than host vectors
- command submission organized around graph execution, not per-subcall host
  synchronization

This is the largest architectural difference between "real Metal integration"
and "CUDA loop port."

### 6. Add capability-gated kernel selection, not just backend presence

The `llama.cpp` Metal backend gates kernels on concrete device capabilities such
as simdgroup reduction and simdgroup matrix support. Psionic should add the same
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
- Do not treat the current dense Metal `psionic.text_generation` claim as evidence
  that GPT-OSS is near-done on Apple.
- Do not advertise an Apple GPT-OSS lane as shipped until it has its own
  validation coverage and claim row.

## Recommended Implementation Order

### Phase 1: Device-Resident Metal Substrate And Graph Seam

- add backend-owned quantized GGUF storage on Metal
- add device-destination activation APIs and reusable scratch / submission
  ownership so decode is not forced through host vectors
- make KV-cache state backend-owned on Metal rather than host-owned
- start the missing IR/compiler surface for the ops Metal will need, but use
  that work to define a reusable execution seam rather than to justify keeping
  host orchestration as the end state

### Phase 2: Attention Stack On Metal

- add backend-specialized support and truthful `extension_support()` for the
  attention-adjacent ops Psionic already models
- move RMSNorm, RoPE, softmax, and scaled dot-product attention onto Metal
- add capability-gated attention selection, with a flash-attention-style path
  as the intended fast lane
- remove CPU attention and host-owned KV handling from the claimed Metal hot
  path

### Phase 3: MoE Routing And Expert Execution On Metal

- add the missing graph/backend ops needed for GPT-OSS/OpenAI-MoE:
  quantized `get_rows`, top-k selection, expert-indexed grouped matmul,
  indexed add, and any missing MoE helper/fusion surface
- move router scoring, route normalization, expert dispatch, and aggregation
  onto Metal
- remove the host-side per-expert loop from the intended Apple architecture

### Phase 4: Graph-Based Prefill And Decode Runtime

- build a reusable GPT-OSS compiled graph/plan for stable decode shapes
- batch prompt prefill through that compiled path rather than stepping every
  token through a handwritten Rust loop
- minimize per-token serve-layer orchestration and keep `psionic-serve` as the
  consumer of compiled execution rather than the owner of model math

### Phase 5: Kernel Tuning And Apple Throughput Parity

- tune the real Metal kernels for the execution shape above instead of tuning
  isolated micro-kernels first
- target the same class of coverage `llama.cpp` has on Metal for quantized
  gather, expert matmul, RMSNorm, RoPE, softmax, and attention
- benchmark against `llama.cpp` on the same Apple host/model using the real
  GPT-OSS HTTP path

### Phase 6: Apple GPT-OSS Serve, Validation, And Claims

- build the Metal GPT-OSS execution service and backend-selectable OpenAI
  server wiring on top of the reusable compiled path
- add Apple Silicon end-to-end GPT-OSS tests using the real local GGUF
- update `psionic-runtime` validation mapping
- update `crates/psionic/docs/HARDWARE_VALIDATION_MATRIX.md`
- add a new roadmap track after `PSI-183` for proper Metal GPT-OSS execution
- do not advertise the Apple GPT-OSS lane as shipped until the real HTTP path
  is validated and the claim row is justified

## Suggested Issue Split

If this becomes the next active track, the cleanest split is:

1. Metal device-resident storage / activations / KV / submission substrate
2. Metal RMSNorm, RoPE, softmax, attention, and capability gating
3. Metal GPT-OSS MoE routing and grouped expert execution
4. Graph/plan-based GPT-OSS prefill and decode runtime
5. Metal kernel tuning toward `llama.cpp`-class throughput
6. Apple GPT-OSS real HTTP-path validation, claim row, and parity closure

That keeps the work aligned with current ownership boundaries:

- reusable model and lowering logic in `crates/psionic/*`
- backend truth in `psionic-backend-metal`
- serve/HTTP integration in `psionic-serve`
- validation truth in `psionic-runtime` plus docs

## Bottom Line

If the goal is merely "get some Apple output," a Metal port of the current CUDA
loop would be faster to land. That is not the goal stated here.

If the goal is a proper Metal integration that follows leading practice more
closely, then Psionic should treat `llama.cpp` as the architectural reference:
graph-first GPT-OSS execution, backend-owned state, broad Metal kernel
coverage, and capability-gated dispatch. The active CUDA issue track reinforces
that direction now; it no longer supports copying the old host loop. The
current CUDA GPT-OSS implementation should inform correctness and product truth,
while the staged CUDA issue ordering should inform how the Metal bring-up is
sequenced.
