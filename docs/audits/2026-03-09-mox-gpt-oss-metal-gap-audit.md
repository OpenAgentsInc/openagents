# 2026-03-09 Mox GPT-OSS Metal Gap Audit

> Historical note: this audit is a point-in-time snapshot from 2026-03-09. Current product and architecture authority lives in `docs/MVP.md`, `docs/OWNERSHIP.md`, and `crates/mox/docs/ROADMAP.md`. File paths and implementation-status claims here may be superseded by later commits.

## Scope

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `crates/mox/docs/ROADMAP.md`
- `docs/audits/2026-03-08-mox-vs-llama-cpp-gpt-oss-performance-audit.md`
- `crates/mox/mox-serve/src/gpt_oss.rs`
- `crates/mox/mox-serve/src/openai_http.rs`
- `crates/mox/mox-serve/src/bin/mox-gpt-oss-server.rs`
- `crates/mox/mox-serve/src/lib.rs`
- `crates/mox/mox-backend-metal/src/lib.rs`
- `crates/mox/mox-backend-cuda/src/lib.rs`
- `crates/mox/mox-runtime/src/validation.rs`
- `crates/mox/docs/HARDWARE_VALIDATION_MATRIX.md`
- current Metal and CUDA text-generation tests under `crates/mox/mox-serve/tests/`

## Executive Summary

Recent work closed the NVIDIA GPT-OSS bring-up track: Mox now loads the real
`gpt-oss-20b-mxfp4.gguf`, serves it through a Mox-owned OpenAI-compatible HTTP
surface, and uses Mox-owned CUDA quantized kernels instead of delegating
execution to `llama.cpp`.

Metal is not at that state. The current Apple path is still the earlier dense
`mox.text_generation` lane: a `safetensors` artifact-backed `ArtifactWordDecoder`
running over a tiny Metal graph surface with only `input`, `constant`,
`matmul`, and `add`. There is no Metal GPT-OSS GGUF service, no Metal GPT-OSS
OpenAI server, no Metal quantized byte-storage path, and no Metal support for
the `Q8_0` / `MXFP4` quantized matvec seam that the shipped CUDA GPT-OSS path
depends on.

The shortest honest path to "GPT-OSS runs on Metal" is not a full
`llama.cpp`-class Metal rewrite. It is:

1. add Metal-owned raw-byte storage plus `Q8_0` / `MXFP4` quantized matvec APIs,
2. mirror the current CUDA GGUF GPT-OSS service on top of those APIs, and
3. add Apple-specific validation, receipts, and server wiring so the new lane
   is truthful.

That would get GPT-OSS executing on Apple Silicon while keeping the current
host-owned Rust decode loop. Performance work beyond that is a separate phase.

## What Is Already Done

The following pieces do not need to be invented again for Metal:

- GPT-OSS / OpenAI-MoE GGUF loading and tensor-layout validation already exist
  in `mox-models`.
- Harmony prompt rendering and output parsing already exist and are backend
  neutral.
- The real GGUF-backed GPT-OSS decoder logic already exists in
  `crates/mox/mox-serve/src/gpt_oss.rs`.
- Session ownership, KV-cache integration, shared-prefix handling, and served
  artifact identity already exist at the `mox-serve` / `mox-runtime` seam.
- The current CUDA GPT-OSS path already shows the narrow accelerator seam Mox
  is using today: backend-owned quantized byte uploads plus quantized matvec
  kernels, with the rest of the decode loop still controlled from Rust.

This matters because Metal does not need a brand-new GPT-OSS design. It needs a
Metal implementation of the same current seam.

## Current Metal Reality

### 1. Metal text generation is still the dense artifact-backed lane

The shipped Metal service is `MetalModelTextGenerationService` in
`crates/mox/mox-serve/src/lib.rs`. It loads `ArtifactWordDecoder` from a
`safetensors` artifact, builds a simple dense graph, and executes that graph on
Metal. The parity tests under `crates/mox/mox-serve/tests/metal_*text_generation*`
also exercise that same artifact-backed path.

This is useful shipped groundwork, but it is not the GPT-OSS path.

### 2. Metal only advertises a tiny dense op surface

`crates/mox/mox-backend-metal/src/lib.rs` defines both
`EMBEDDINGS_SUPPORTED_OPS` and `TEXT_GENERATION_SUPPORTED_OPS` as the dense
surface `["input", "constant", "matmul", "add"]`.

That matches the current artifact-backed Metal path. It does not cover the
quantized GPT-OSS lane.

### 3. Metal explicitly refuses quantized constant storage today

`buffer_from_tensor_data(...)` in `crates/mox/mox-backend-metal/src/lib.rs`
returns an error for `TensorData::QuantizedBlocks(...)` with the message that
the Metal backend does not support quantized constant storage.

That single refusal is enough to block the current GPT-OSS implementation from
being ported directly, because the shipped CUDA path depends on preserving GGUF
`Q8_0` and `MXFP4` bytes in backend-owned storage.

### 4. GPT-OSS serving code only has CPU and CUDA backends

`crates/mox/mox-serve/src/gpt_oss.rs` currently defines:

- `CpuGgufGptOssTextGenerationService`
- `CudaGgufGptOssTextGenerationService`
- `CudaQuantizedMatrix`
- `CudaQuantizedExpertTensor`
- `load_cuda_quantized_matrix(...)`
- `load_cuda_quantized_expert_tensor(...)`

There is no `MetalGgufGptOssTextGenerationService`, no Metal GGUF loader path,
and no Metal quantized matrix/expert types.

### 5. The OpenAI-compatible GPT-OSS server is CUDA-only

`crates/mox/mox-serve/src/openai_http.rs` defines
`GptOssCudaOpenAiCompatServer` and `GptOssCudaWorker`. The server health
endpoint reports `backend: "cuda"`. The binary
`crates/mox/mox-serve/src/bin/mox-gpt-oss-server.rs` wires only that CUDA
server.

So even if Metal execution existed inside `gpt_oss.rs`, there is still no Metal
HTTP bring-up path today.

### 6. The current validation matrix would overclaim if reused unchanged

`crates/mox/mox-runtime/src/validation.rs` and
`crates/mox/docs/HARDWARE_VALIDATION_MATRIX.md` currently treat
`metal + mox.text_generation` as the shipped claim
`metal.text_generation.apple_silicon`, and the green tests for that claim are
the artifact-backed dense tests.

That is truthful for the current first Metal text-generation lane, but not for
GPT-OSS. If a future Metal GPT-OSS path reused the same claim without adding
new rows and tests, the capability/receipt surfaces would overstate what had
actually been validated.

## What Still Blocks GPT-OSS On Metal

### 1. Metal needs a backend-owned quantized byte-storage path

The current CUDA GPT-OSS path works because `CudaBackend` can:

- upload raw GGUF bytes into backend-owned buffers with `byte_buffer(...)`
- execute `quantized_matvec(...)`
- execute `quantized_matvec_with_offset(...)`

Metal has no equivalent API today. That means the first blocker is not
attention or Harmony. It is basic backend-owned storage for GGUF quantized
payloads.

Minimum required work:

- add a Metal raw-byte buffer path analogous to CUDA `byte_buffer(...)`
- permit backend-owned quantized storage instead of rejecting it in
  `buffer_from_tensor_data(...)`
- preserve byte-offset addressing so expert tensors can use one uploaded blob
  with per-expert offsets

### 2. Metal needs `Q8_0` and `MXFP4` quantized matvec kernels

The shipped GPT-OSS CUDA path is built around row-wise quantized matvec calls
for:

- ordinary quantized matrices
- expert tensors addressed through byte offsets

The minimum Metal port needs the same two execution forms:

- `quantized_matvec(weights, mode, rows, cols, input)`
- `quantized_matvec_with_offset(weights, byte_offset, mode, rows, cols, input)`

And the first honest mode set is exactly the GPT-OSS set already used on CUDA:

- `QuantizationMode::GgmlQ8_0`
- `QuantizationMode::GgmlMxfp4`

Without those kernels, the current GPT-OSS Mox path cannot run on Metal at
all.

### 3. `gpt_oss.rs` needs a Metal mirror of the current CUDA service

Once Metal can hold quantized bytes and run quantized matvecs, the next blocker
is serve-layer wiring. The current file structure strongly suggests the minimal
implementation:

- add `GPT_OSS_METAL_BACKEND`
- add `MetalGgufGptOssTextGenerationService`
- add `MetalGgufGptOssGenerationModel`
- add `MetalQuantizedMatrix`
- add `MetalQuantizedExpertTensor`
- add `load_metal_quantized_matrix(...)`
- add `load_metal_quantized_expert_tensor(...)`

The important point is that this does not require a brand-new decode design.
The current CUDA GPT-OSS path is still a Rust-owned loop. That means the first
Metal bring-up can keep:

- CPU RMSNorm
- CPU RoPE application
- CPU KV-cache reads/writes
- CPU attention
- CPU router top-k and softmax
- CPU MoE weighting and aggregation

and only offload the quantized projection and expert matvec calls to Metal.

That will not be the final performance architecture, but it is enough to get
truthful GPT-OSS execution running on Apple Silicon.

### 4. The server and binary surfaces need a Metal lane

Even after `gpt_oss.rs` grows Metal execution, the current HTTP path would still
be wrong because it is hard-coded as CUDA:

- `GptOssCudaOpenAiCompatServer`
- `GptOssCudaWorker`
- `/health` returning `backend: "cuda"`
- `mox-gpt-oss-server` always constructing the CUDA server

Minimum required work:

- make the GPT-OSS OpenAI server backend-selectable, or
- add a parallel Metal-specific server/worker path

The honest requirement is simple: when the server is on Metal, the health,
diagnostic, and provenance surfaces must all say `metal`, not just return valid
text.

### 5. Truthful validation and receipt coverage must be expanded

The current validation matrix only proves:

- dense Apple Metal embeddings
- dense Apple Metal text generation
- explicit Metal refusal off platform

It does not prove GGUF GPT-OSS on Metal.

Before this lane can be called shipped, Mox needs one of two things:

- a new claim such as a GPT-OSS-specific Apple Metal row, or
- a narrower redefinition of `mox.text_generation` claims plus updated tests

Either way, the following must move together:

- `mox-runtime` validation mapping
- `crates/mox/docs/HARDWARE_VALIDATION_MATRIX.md`
- provider capability/receipt references if they expose the lane
- Metal GPT-OSS green tests on Apple Silicon

Without that, Metal GPT-OSS would exist as code but still lack truthful product
evidence.

### 6. Real Apple Silicon validation is still missing

The roadmap correctly notes that active host execution work was validated on the
NVIDIA machine. For Metal, the required proof is different:

- compile on macOS
- load the real local GPT-OSS GGUF on Apple Silicon
- execute a deterministic request end to end
- prove no silent fallback to CPU while claiming Metal

Today there is no such GPT-OSS-on-Metal test lane.

## What Is Not Required For First Bring-Up

The 2026-03-08 CUDA performance audit correctly showed that the current GPT-OSS
path is still slower than `llama.cpp` because it is a host-owned decode loop
with accelerator subcalls.

That is important, but it is not the first Metal gate.

These items are performance follow-on work, not bring-up prerequisites:

- device-resident KV cache
- backend-owned RoPE and RMSNorm
- backend-owned attention or flash attention
- grouped expert execution
- backend-owned MoE routing and aggregation
- graph-compiled GPT-OSS decode on Metal
- `llama.cpp`-class Metal throughput tuning

If those get coupled to the bring-up, the work becomes much larger than it
needs to be.

## Recommended Implementation Order

### Phase 1: Metal quantized substrate

- add Metal raw-byte uploads for backend-owned GGUF quantized tensors
- add `Q8_0` quantized matvec
- add `MXFP4` quantized matvec
- add byte-offset execution for expert tensors
- add backend unit tests against CPU quantized reference math

### Phase 2: Metal GPT-OSS serve path

- mirror the CUDA GPT-OSS service/model types in `crates/mox/mox-serve/src/gpt_oss.rs`
- load GGUF quantized matrices and expert tensors into Metal buffers
- keep the current host-owned decode loop for the first version
- surface truthful `backend_selection` and diagnostics for Metal

### Phase 3: OpenAI-compatible server wiring

- make the GPT-OSS server backend-selectable or add a Metal server type
- ensure `/health` and related diagnostics report `metal`
- keep the binary naming and command-line story explicit; do not silently start
  a CPU path while presenting it as Metal

### Phase 4: Validation and documentation

- add Apple Silicon GPT-OSS integration coverage
- update `mox-runtime` validation mapping
- update `crates/mox/docs/HARDWARE_VALIDATION_MATRIX.md`
- extend `crates/mox/docs/ROADMAP.md` with a new post-`MOX-183` Metal GPT-OSS
  track if this work is now active

## Suggested Issue Split

The current roadmap has no active post-`MOX-183` Metal GPT-OSS item. If this is
the new priority, the cleanest follow-on is a new small track such as:

1. Metal GGUF quantized storage and `Q8_0` / `MXFP4` matvec substrate
2. Metal GPT-OSS serve path and OpenAI server wiring
3. Apple Silicon validation-matrix expansion and end-to-end proof

That keeps the work aligned with current ownership boundaries:

- `mox-backend-metal` owns the accelerator substrate
- `mox-serve` owns the GPT-OSS serving path and HTTP surface
- `mox-runtime` plus docs own the validation/evidence truth

## Bottom Line

GPT-OSS on Metal is not blocked by missing model semantics. Those are already
landed. It is blocked by missing Apple-side quantized execution substrate and
by the absence of a Metal-specific GGUF serving path above that substrate.

The shortest honest plan is to port the current CUDA seam to Metal first, then
add validation and HTTP wiring. Only after that should the work expand into the
much larger "make Metal GPT-OSS fast" problem.
