# Mox Roadmap

> Status: updated 2026-03-08 after PR [#3163](https://github.com/OpenAgentsInc/openagents/pull/3163) merged to `main` and after a deep-research pass over Candle and Tinygrad.
>
> This is the live roadmap for `crates/mox/`. The phase-2/3/4 baseline is now
> merged. The remaining work below is the gap between "we have a local Rust
> runtime subtree" and "the desktop fully replaces Ollama with Mox and uses it
> as truthful compute-market substrate."

## Objective

Replace the desktop's external Ollama dependency with an in-process Rust
runtime that:

- keeps app UX and provider orchestration in `apps/autopilot-desktop`
- keeps reusable model, runtime, backend, and serving logic in `crates/mox/*`
- matches the subset of Ollama behavior OpenAgents actually depends on
- remains explicit about backend readiness, fallback, hardware support,
  lifecycle, and evidence

This is not a plan to rebuild all of Ollama.

This is also not enough, by itself, to satisfy the broader compute-market plan.
Mox must become the reusable execution substrate for truthful `inference` and
`embeddings` supply first, with later bounded `sandbox_execution` only if that
family stays explicit and machine-checkable.

## Ownership Rules

The roadmap must keep `docs/OWNERSHIP.md` intact:

- `crates/mox/*` owns reusable tensor, IR, compiler, runtime, model, serve, and
  provider-facing engine truth
- `apps/autopilot-desktop` owns the local runtime adapter, provider UX,
  inventory presentation, admission policy, and final cutover from Ollama HTTP
  calls
- `crates/mox/*` must not absorb app-specific UI or product orchestration

## Tinygrad-Style Rules

Mox should preserve the parts of Tinygrad that matter architecturally without
trying to port Tinygrad line by line:

- keep a small, inspectable primitive op surface
- keep backend crates explicit; discovery, allocation, lowering, execution, and
  health reporting belong to backends
- keep model formats and model stores separate from backend execution
- keep the serving surface library-first and in-process
- treat JIT capture, compile plans, kernel-cache behavior, batching, queueing,
  and topology as explicit runtime policy
- keep quantized tensor storage backend-backed instead of forcing eager
  CPU-only dequantization paths
- keep memory-mapped model blob access and paged tensor storage explicit so
  large local models do not require eager copies
- keep an explicit fused/custom-op escape hatch for backend-specific attention,
  quantized GEMM, and normalization kernels
- treat backend allocator pools, kernel caches, and device-memory budgets as
  explicit runtime policy rather than backend internals
- never silently run on CPU while advertising Metal, AMD, or NVIDIA readiness

The first backend-complete primitive surface still needs to be just large enough
to run the launch `inference` and `embeddings` product paths:

- matmul / batched matmul
- embedding lookup / gather
- reshape / transpose / concat / slice
- cast / dequantize hooks
- elementwise add / mul / silu / gelu
- rmsnorm / layernorm
- RoPE
- softmax
- KV-cache read / append / update

## Shipped On Main

`main` now includes the merged phase-2/3/4 baseline from PR `#3163`
(`53f31280a`).

### Delivered in the merged baseline

- artifact-backed bundle ingestion in `mox-models`
- explicit quantization metadata and capability truth
- tested model-backed CPU `mox.embeddings` and `mox.text_generation` paths
- provider/receipt truth for model-backed CPU products
- Metal discovery, allocation, command submission, truthful backend selection,
  and tested Metal-backed `mox.embeddings`
- AMD topology/risk/recovery metadata plus separate `amd_kfd` and
  `amd_userspace` discovery/readiness surfaces
- provider-facing AMD context and operator runbook
- Rustygrad subtree rename to Mox

### GitHub issue status

Verified on 2026-03-08 via `gh issue view` and `gh issue create`:

| Issue span | State | What landed |
| --- | --- | --- |
| [#3143](https://github.com/OpenAgentsInc/openagents/issues/3143), [#3144](https://github.com/OpenAgentsInc/openagents/issues/3144) to [#3149](https://github.com/OpenAgentsInc/openagents/issues/3149) | Closed | Phase-2 CPU baseline: artifact-backed bundles, quantization truth, CPU embeddings, CPU text generation, provider truth, and tested model-backed flows. |
| [#3150](https://github.com/OpenAgentsInc/openagents/issues/3150), [#3151](https://github.com/OpenAgentsInc/openagents/issues/3151) to [#3156](https://github.com/OpenAgentsInc/openagents/issues/3156) | Closed | Phase-3 Metal baseline: discovery, allocator/submission substrate, minimum kernel coverage, truthful backend selection, parity coverage, and Metal embeddings. |
| [#3157](https://github.com/OpenAgentsInc/openagents/issues/3157), [#3158](https://github.com/OpenAgentsInc/openagents/issues/3158) to [#3162](https://github.com/OpenAgentsInc/openagents/issues/3162) | Closed | Phase-4 AMD truth baseline: AMD metadata model, KFD/userspace discovery, provider truth, and runbook coverage. |

Current open GitHub issues seeded from this roadmap:

| Issue | Title | Why it is open now |
| --- | --- | --- |
| [#3164](https://github.com/OpenAgentsInc/openagents/issues/3164) | MOX-115: Add GGML/GGUF quant block decode coverage and backend-backed quantized tensor storage | First-wave GGUF and quantized runtime substrate. |
| [#3165](https://github.com/OpenAgentsInc/openagents/issues/3165) | MOX-116: Add memory-mapped model blob access and paged tensor storage for local GGUF and Ollama blobs | First-wave local model ingress and load policy. |
| [#3166](https://github.com/OpenAgentsInc/openagents/issues/3166) | MOX-117: Build an Ollama-to-Mox conformance suite for catalog, generation, embeddings, and error semantics | Cutover should be gated by harnessed conformance, not hand inspection. |
| [#3167](https://github.com/OpenAgentsInc/openagents/issues/3167) | MOX-118: Add golden prompt-rendering and tokenizer fixtures from real GGUF and Ollama installs | Prevent silent prompt/tokenizer drift. |
| [#3168](https://github.com/OpenAgentsInc/openagents/issues/3168) | MOX-119: Define numerical parity tolerances and drift budgets across CPU and accelerated backends | Parity work needs explicit tolerances before backend claims widen. |
| [#3169](https://github.com/OpenAgentsInc/openagents/issues/3169) | MOX-126A: Add paged KV-cache layout, accounting, and spill policy for long-context text generation | Separates KV paging from model-blob paging. |
| [#3170](https://github.com/OpenAgentsInc/openagents/issues/3170) | MOX-156: Add backend-specific quantized execution kernels and parity coverage for supported GGUF quant families | Separates quantized load truth from quantized execution truth. |
| [#3171](https://github.com/OpenAgentsInc/openagents/issues/3171) | MOX-161: Define allowed fallback lattice for Mox served products | Makes correctness-versus-speed fallback policy explicit before cutover. |

## Current Reality

The checked-in repo is no longer at "phase 0 bootstrap." The current truthful
baseline on `main` is:

- CPU model-backed embeddings and text generation exist and are tested
- Metal has a truthful accelerated embeddings path, but not text generation
- AMD has truthful discovery/readiness surfaces, but not execution kernels
- provider-facing capability and receipt truth is ahead of the app cutover
- Mox still does not replace the desktop's Ollama dependency

## What Still Blocks Full Ollama Replacement

The remaining gaps are not "make it faster." They are mostly behavioral
contract, compatibility, lifecycle, and cutover work.

### Model compatibility and prompt behavior

- GGUF loading and tensor extraction
- GGML/GGUF quant block decode coverage and backend-backed quantized tensor
  storage
- tokenizer loading for supported families
- chat-template extraction and normalization
- role rendering, BOS/EOS handling, stop defaults, and family-specific prompt
  formatting

### Catalog and lifecycle

- Ollama manifest/blob discovery and installed-model resolution
- memory-mapped blob access and paged tensor storage for large local models
- `tags` / `show` / `ps` equivalent local catalog APIs
- warm / unload / keepalive / loaded-model lifecycle
- deterministic KV-cache ownership and session lifecycle

### Behavioral contract

- context-window accounting and truncation policy
- sampler correctness and deterministic replay
- model memory planning and load admission control
- streaming semantics and cancellation behavior
- embeddings batch semantics and metadata reporting
- model-store integrity verification
- backend-neutral error taxonomy
- explicit fallback and degraded-state policy
- backend allocator pooling, kernel-cache bounds, and device-memory-budget
  reporting
- runtime observability and cutover performance gates

### Accelerator coverage

- Metal text generation
- a fused/custom-op surface for backend-specific attention, quantized GEMM,
  RoPE, and normalization kernels
- NVIDIA discovery, truth, and execution
- AMD execution after the current discovery/readiness work

### Desktop cutover and compute-market substrate

- app-owned local runtime seam instead of Ollama HTTP calls
- rename/remove remaining Ollama-specific app contracts and wording
- compute-market capability qualifiers, batching truth, topology truth, warm/cold
  cache truth, and delivery-proof evidence
- stable execution-plan digests, kernel counts, bytes moved, plan-cache
  hit/miss, and KV-growth evidence
- process isolation policy and the long-term Mox-native model/runtime boundary

## Ollama Behaviors That Must Become Explicit

The missing work is not just "run the model in Rust." Ollama currently supplies
several behavioral contracts implicitly, and Mox needs explicit semantics for
them.

From the local Ollama source reviewed for this roadmap:

- prompt-template sourcing is not trivial:
  - `convert/tokenizer.go` accepts `chat_template` from `tokenizer_config.json`
    either as a plain string or as a named list and selects the `"default"`
    template when present
  - tokenizer metadata also carries BOS/EOS and `add_bos_token` /
    `add_eos_token` behavior, while `generation_config.json` can override EOS
    token IDs
- chat truncation has defined semantics:
  - `server/prompt.go` drops old messages from the front until the rendered
    prompt fits
  - it preserves system messages and always keeps the latest message
  - image-bearing prompts count image token cost during truncation
- embeddings have their own over-limit behavior:
  - `server/routes.go` tokenizes input, adjusts context budget for BOS/EOS
    insertion, optionally truncates, errors when truncation cannot make the
    input fit, normalizes vectors, and optionally re-normalizes dimension-cut
    outputs
- streaming has wire semantics:
  - `streamResponse` uses `application/x-ndjson`
  - chunks are newline-delimited JSON objects
  - errors before the first chunk are returned as normal JSON errors
  - errors after streaming starts are emitted as streamed JSON error objects and
    terminate the stream
  - final chunks carry `done` and `done_reason`
- scheduling has admission and residency policy:
  - `server/sched.go` enforces a max pending queue, max loaded model count, and
    one active model load at a time
  - keepalive and load timeout are operational inputs, not optional extras
- memory planning already exists as structured data:
  - `ml/device.go` exposes per-device memory usage and `ErrNoMem`
- model-store integrity already matters:
  - `manifest` and `server/internal/cache/blob` validate digest format and blob
    content, handle missing files, and carry compatibility logic for manifests
    and blob-addressable storage

That is why the roadmap still needs explicit work for prompt rendering, context
budgets, memory admission, streaming, integrity, errors, and fallback.

## Tinygrad Findings That Expand Compute Market Scope

Reviewing `~/code/tinygrad` adds scope beyond simple Ollama parity:

- `gguf_load` and `ggml_data_to_tensor` make GGUF parsing plus quant block
  decode a first-class runtime concern, not a thin metadata shim
- the Llama path makes KV-cache lifecycle, `start_pos` accounting, and the
  token-by-token JIT fast path explicit
- `apply_graph_to_jit`, `GraphRunner`, and `MultiGraphRunner` show that stable
  replayable execution plans should be a productized runtime primitive
- `GlobalCounters` and the realize path make per-request kernel count, memory
  traffic, and timing evidence straightforward to collect
- the disk device and disk-backed GGUF loading path show a credible route to
  mmap-backed model ingestion and later spill-style storage
- the AMD runtime docs make interface risk explicit, including the unsafe
  userspace driver path that may unbind `amdgpu`

That adds missing roadmap work for:

- capability-envelope qualifiers beyond simple backend selection
- batch posture, queue discipline, and throughput-class truth
- multi-device or sharded execution planning
- execution-plan caching and warm/cold compile-path evidence
- runtime evidence for compute delivery proofs
- a bounded execution-profile model for later `sandbox_execution`

## Candle Findings That Tighten Runtime And Backend Scope

Reviewing `~/code/candle` sharpens several implementation buckets that the
existing roadmap treated too loosely:

- GGUF/GGML loading in Candle is tied to backend-backed quantized tensor
  storage, not just metadata parsing
- tokenizer reconstruction from GGUF metadata already carries BOS/EOS and
  template-processing implications, which means Mox should not treat tokenizer
  loading and prompt rendering as one issue
- seeded sampler utilities, repeat penalty, and GQA helpers show that sampler
  correctness needs to include the surrounding decode helpers, not just RNG
- explicit compile-time backend features (`metal`, `cuda`, `cudnn`, `nccl`,
  `accelerate`, `mkl`) are a strong model for compiled-vs-probed capability
  truth
- Metal buffer pooling, bounded caches, and device-memory budget work show that
  backend allocator policy is a roadmap item, not a cleanup task
- Candle's custom-op and fused-kernel escape hatches are a practical template
  for how Mox should land backend-specific attention and quantized GEMM kernels

That means the roadmap should explicitly track:

- quantized GGUF block decode and backend-backed quantized tensor storage
- memory-mapped model blob access and paged tensor storage
- backend allocator pooling, kernel-cache bounds, and device-memory-budget
  reporting
- a fused/custom-op surface for backend-specific kernels

## Proposed Issues Not Yet In GitHub

These are the next issues that should be opened now that the phase-2/3/4
baseline is merged.

### Epic A: GGUF, tokenizer, prompt, and conformance baseline

| Local ID | Proposed issue | Scope | Why it exists |
| --- | --- | --- | --- |
| `MOX-110` | Add `WeightFormat::Gguf` and a reusable GGUF metadata/tensor loader | `mox-models` | Required to read the format Ollama actually points at during migration. |
| `MOX-111` | Implement tokenizer loading from GGUF metadata for SentencePiece and GPT-style BPE families | `mox-models` | Fixture tokenizers are not enough for real model parity. |
| `MOX-115` | Add GGML/GGUF quant block decode coverage and backend-backed quantized tensor storage | `mox-models`, `mox-runtime`, backend crates | Candle and Tinygrad both treat quantized tensor decode/storage as a core loader/runtime boundary; Mox should not stop at metadata parsing. |
| `MOX-116` | Add memory-mapped model blob access and paged tensor storage for local GGUF and Ollama blobs | `mox-catalog`, `mox-models`, `mox-runtime` | Large local models should load through mmap/paged storage instead of eager copies when possible. |
| `MOX-117` | Build an Ollama-to-Mox conformance suite for `tags` / `show` / `ps` / `generate` / `embed` behavior, prompt rendering, truncation, stop handling, streaming, and error semantics | `mox-catalog`, `mox-serve`, `mox-provider`, test fixtures | Cutover should be decided by repeatable conformance evidence, not hand inspection. |
| `MOX-118` | Add golden prompt-rendering and tokenizer fixtures for supported model families from real GGUF and Ollama installs | `mox-models`, `mox-serve`, test fixtures | Prompt and tokenizer behavior drifts silently without a real golden corpus. |
| `MOX-119` | Define numerical parity tolerances and drift budgets across CPU and accelerated backends for embeddings and text generation | `mox-serve`, backend crates, `mox-provider` | Backend parity needs explicit tolerance rules across quant modes, decode loops, and embeddings outputs. |
| `MOX-112` | Add GGUF-backed decoder model-family adapters for first launch families (`llama`, `qwen`, `mistral`) | `mox-models`, `mox-serve` | Replaces model-family construction still hidden behind Ollama. |
| `MOX-113` | Add GGUF-backed embeddings model-family adapters for the first supported embedding families | `mox-models`, `mox-serve` | Keeps embeddings real rather than demo-only. |
| `MOX-114` | Implement chat-template extraction and prompt-rendering compatibility for supported model families | `mox-models`, `mox-serve` | GGUF plus tokenizer is still not enough without prompt formatting parity. |

### Epic B: Ollama-compatible catalog and local runtime lifecycle

| Local ID | Proposed issue | Scope | Why it exists |
| --- | --- | --- | --- |
| `MOX-120` | Add `mox-catalog` for Ollama manifest/blob discovery and model resolution | new `mox-catalog` crate | Lets Mox discover already-installed Ollama models without the daemon. |
| `MOX-121` | Implement installed-model listing and inspection APIs equivalent to `tags` and `show` | `mox-catalog`, `mox-serve` | Replaces current desktop model discovery and validation calls. |
| `MOX-122` | Implement loaded-model registry, warm/load/unload, and keepalive semantics equivalent to `ps` and warmups | `mox-serve`, `mox-runtime` | Replaces the local lifecycle subset the desktop actually depends on. |
| `MOX-123` | Expand generation options to cover `temperature`, `top_k`, `top_p`, penalties, `seed`, and `stop` | `mox-serve` | Matches the option surface already normalized by the app. |
| `MOX-124` | Add generation metrics and provenance for prompt tokens, output tokens, load time, total time, warm/cold state, and plan digest | `mox-serve`, `mox-provider` | Preserves truthful receipts and UI projections after cutover. |
| `MOX-125` | Publish a library-first local runtime API for `list_models`, `show_model`, `loaded_models`, `warm_model`, `unload_model`, `generate`, and `embed` | `mox-serve`, `mox-provider` | Creates the in-process replacement boundary the app can call directly. |
| `MOX-126` | Add GGUF-backed KV-cache ownership and deterministic session lifecycle for text generation | `mox-serve` | Required for real text-generation serving instead of fixture-shaped flows. |
| `MOX-126A` | Add paged KV-cache layout, accounting, and spill policy for long-context text generation | `mox-serve`, `mox-runtime`, `mox-provider` | Model-blob paging and KV paging are different operational problems and need separate policy. |

### Epic C: Behavioral contract and serving policy

| Local ID | Proposed issue | Scope | Why it exists |
| --- | --- | --- | --- |
| `MOX-127` | Add explicit context-window accounting, truncation policy, and over-limit error semantics | `mox-models`, `mox-serve` | Mox needs concrete token budgeting, truncation, and refusal rules. |
| `MOX-128` | Add deterministic sampler implementation and replay coverage for supported generation options | `mox-serve`, `mox-runtime` | Option parity without sampler correctness is not enough. |
| `MOX-129` | Add model memory planning, residency policy, and admission control for local serving | `mox-serve`, `mox-runtime`, `mox-provider` | Warm/load/unload is underspecified without memory planning and refusal behavior. |
| `MOX-133` | Add streaming token generation semantics and cancellation behavior for the local runtime API | `mox-serve`, `mox-provider` | The app needs explicit partial-output and final-chunk semantics. |
| `MOX-134` | Add embeddings API parity, batch semantics, and model metadata reporting | `mox-serve`, `mox-provider` | Embeddings need explicit dimension, normalization, and failure behavior. |
| `MOX-135` | Add local model-store integrity verification and cache-repair diagnostics | `mox-catalog`, `mox-models` | Reading the Ollama store is not enough without digest and corruption checks. |
| `MOX-136` | Define backend-neutral local runtime error taxonomy and desktop-facing diagnostics | `mox-serve`, `mox-provider`, `mox-runtime` | Replacing Ollama requires a stable error model. |
| `MOX-137` | Add explicit backend fallback, refusal, and degraded-state policy for served products | `mox-runtime`, `mox-provider`, backend crates | "No silent CPU fallback" must become concrete and testable. |
| `MOX-138` | Define performance acceptance thresholds and cutover gates for Mox runtime replacement | `mox-serve`, `mox-provider`, backend crates | The team needs explicit launch gates before cutover. |
| `MOX-139` | Decide and document LoRA/adapter support policy for the Ollama replacement boundary | `mox-models`, `mox-catalog`, `mox-serve` | Even a deferral needs an explicit migration policy. |
| `MOX-157` | Add backend allocator pooling, bounded kernel caches, and device-memory-budget reporting | `mox-runtime`, backend crates, `mox-provider` | Candle's Metal path shows this is required for truthful memory admission and stable warm/cold behavior. |
| `MOX-158` | Add a fused/custom-op extension surface for backend-specific attention, quantized GEMM, RoPE, and normalization kernels | `mox-compiler`, `mox-runtime`, backend crates | Metal, CUDA, and AMD text generation will need backend-specific fused kernels without breaking the small visible primitive surface. |
| `MOX-159` | Add local runtime observability for warm/cold transitions, active sessions, queue depth, memory footprint, and backend health changes | `mox-serve`, `mox-provider`, `mox-runtime` | The desktop cutover will be hard to debug without explicit runtime-state observability. |

### Epic D: Quantized execution and accelerated backends after the merged baseline

| Local ID | Proposed issue | Scope | Why it exists |
| --- | --- | --- | --- |
| `MOX-156` | Add backend-specific quantized execution kernels and parity coverage for supported GGUF quant families | `mox-compiler`, `mox-runtime`, backend crates, `mox-serve` | Loading quantized models truthfully is not enough if execution immediately falls back to dequantized slow paths. |
| `MOX-130` | Add Metal lowering/kernel coverage for the minimum text-generation primitive set | `mox-backend-metal`, `mox-compiler`, `mox-runtime` | Embeddings-only Metal is not enough to replace Ollama. |
| `MOX-131` | Add CPU-vs-Metal parity coverage for the supported text-generation product path | `mox-backend-metal`, `mox-serve` | Required before Metal-backed text generation is believable. |
| `MOX-132` | Ship a tested Metal-backed `mox.text_generation` path | `mox-backend-metal`, `mox-serve`, `mox-provider` | Closes the biggest remaining Metal gap. |
| `MOX-140` | Mox phase 5: NVIDIA backend architecture and truthful capability surfaces | `mox-backend-cuda` or equivalent, `mox-runtime`, `mox-provider` | NVIDIA must be explicit if local-runtime coverage is meant to be broad. |
| `MOX-141` | Define the Mox NVIDIA capability, topology, and risk model | `mox-runtime`, `mox-provider` | Gives NVIDIA the same explicit truth model AMD already has. |
| `MOX-142` | Implement NVIDIA discovery and health reporting | `mox-backend-cuda` | Makes GPU availability and degraded states explicit. |
| `MOX-143` | Add CUDA allocator, buffer, stream, and command submission substrate | `mox-backend-cuda`, `mox-runtime` | NVIDIA execution cannot start without runtime substrate. |
| `MOX-144` | Add CUDA lowering and kernel coverage for the minimum served-product primitive set | `mox-backend-cuda`, `mox-compiler` | Implements the tinygrad-style primitive surface on NVIDIA. |
| `MOX-145` | Wire NVIDIA backend selection and truthful capability reporting through Mox | `mox-runtime`, `mox-provider`, `mox-backend-cuda` | Keeps provider/runtime contracts explicit and replay-safe. |
| `MOX-146` | Add CPU-vs-NVIDIA parity coverage for the first supported served product path | `mox-backend-cuda`, `mox-serve` | Prevents "CUDA works" claims without evidence. |
| `MOX-147` | Ship the first tested NVIDIA-backed served product path | `mox-backend-cuda`, `mox-serve`, `mox-provider` | Makes NVIDIA real instead of aspirational. |
| `MOX-150` | Mox phase 6: AMD served-product execution path | `mox-backend-amd-kfd`, `mox-backend-amd-userspace`, `mox-runtime`, `mox-provider` | Turns AMD from truthful detection into actual execution. |
| `MOX-151` | Add AMD KFD lowering and kernel coverage for the first supported primitive set | `mox-backend-amd-kfd`, `mox-compiler`, `mox-runtime` | KFD is the lower-risk AMD execution lane and should come first. |
| `MOX-152` | Wire served-product capability gating for AMD KFD separately from AMD userspace | `mox-provider`, `mox-runtime`, AMD backend crates | Preserves the KFD/userspace split after execution lands. |
| `MOX-153` | Add CPU-vs-AMD KFD parity coverage for the first supported served product path | `mox-backend-amd-kfd`, `mox-serve` | Prevents overclaiming AMD readiness. |
| `MOX-154` | Ship the first tested AMD KFD-backed served product path and keep AMD userspace explicitly gated | `mox-backend-amd-kfd`, `mox-serve`, `mox-provider` | Delivers AMD value without pretending userspace is equally ready. |

### Epic E: App cutover and long-term boundary

| Local ID | Proposed issue | Scope | Why it exists |
| --- | --- | --- | --- |
| `OA-200` | Rename `OllamaExecutionMetrics` and `OllamaExecutionProvenance` to backend-neutral names | `apps/autopilot-desktop` | Removes naming debt before the app stops being Ollama-specific. |
| `OA-201` | Introduce an app-owned `LocalInferenceRuntime` trait and `MoxRuntimeAdapter` | `apps/autopilot-desktop` | Preserves the app seam while swapping runtime implementations. |
| `OA-202` | Switch desktop default from external Ollama HTTP calls to the in-process Mox runtime | `apps/autopilot-desktop` | This is the actual product cutover. |
| `OA-203` | Remove the external Ollama dependency and clean up provider/UI wording | `apps/autopilot-desktop` | Finishes product truth cleanup after parity is proven. |
| `MOX-160` | Define in-process vs subprocess isolation policy for Mox local serving | `mox-serve`, `mox-runtime`, backend crates | The desktop needs an explicit crash/reset isolation decision. |
| `MOX-161` | Define allowed fallback lattice for Mox served products: refuse, degrade, replan, retry, or same-backend slow path | `mox-runtime`, `mox-provider`, `mox-serve` | Teams will otherwise improvise correctness-vs-speed fallback behavior inconsistently. |
| `MOX-170` | Define the boundary between Ollama-compat migration support and the long-term Mox-native model/runtime format | `mox-models`, `mox-catalog`, `mox-serve` | Migration support should not permanently dictate Mox architecture. |

### Epic F: Compute-market execution substrate beyond Ollama parity

See [CONFORMANCE_AND_EVIDENCE_CONTRACT.md](./CONFORMANCE_AND_EVIDENCE_CONTRACT.md)
for the minimum conformance harness scope and runtime evidence schema that
`MOX-117`, `MOX-171` through `MOX-175`, and `OA-201` / `OA-202` must satisfy.

| Local ID | Proposed issue | Scope | Why it exists |
| --- | --- | --- | --- |
| `MOX-171` | Expand Mox capability surfaces for compute-market inventory, topology, and performance qualifiers | `mox-provider`, `mox-runtime` | Compute-market inventory needs more than "GPU available"; it also needs compiled-vs-probed backend and toolchain truth. |
| `MOX-172` | Add batch execution posture, queueing policy, and throughput-class capability reporting | `mox-serve`, `mox-runtime`, `mox-provider` | Batch behavior affects what supply a provider can honestly advertise. |
| `MOX-173` | Add multi-device and sharded execution planning for supported product paths | `mox-runtime`, `mox-compiler`, backend crates, `mox-provider` | The market eventually needs same-type multi-device truth, declarative sharding plans, and topology-aware large-model support. |
| `MOX-174` | Add execution-plan caching, kernel-cache policy, and warm/cold compile-path evidence | `mox-runtime`, `mox-compiler`, backend crates, `mox-provider` | Tinygrad's graph runners and Candle's backend caches both point to plan identity and cache policy as first-class runtime truth. |
| `MOX-175` | Extend Mox runtime evidence with compute-market delivery-proof fields and settlement-linkage inputs | `mox-serve`, `mox-provider`, `mox-runtime` | The market needs kernel counts, bytes moved, plan-cache hit/miss, KV growth, and stable plan digests, not app-local reconstruction. |
| `MOX-176` | Define a reusable Mox execution-profile model for bounded `sandbox_execution` | `mox-runtime`, `mox-provider` | Later `sandbox_execution` must stay bounded and machine-checkable. |
| `MOX-177` | Add reusable sandbox-execution receipt and evidence contracts compatible with compute-market supply | `mox-runtime`, `mox-provider` | If sandbox execution lands, it needs explicit digests, resource summaries, and exit reasons. |
| `MOX-178` | Add topology-aware substitution and deliverability checks for accelerator-sensitive compute offers | `mox-provider`, `mox-runtime` | The market needs reusable promised-vs-delivered capability comparison inputs. |

## Recommended Order

The shortest honest path from today's `main` is:

1. Open and land `MOX-110` through `MOX-119` and `MOX-120` through `MOX-126A`
   so Mox can actually read, mmap, catalog, prove conformance for, and serve supported
   Ollama-installed models.
2. Land `MOX-127` through `MOX-139`, plus `MOX-157` through `MOX-159`, before
   app cutover so context, sampling, streaming, embeddings, integrity,
   fallback, allocator policy, fused-kernel escape hatches, observability, and
   performance gates are explicit.
3. Land `MOX-156` before broad backend claims so quantized execution parity is
   not hidden behind truthful load-only support.
4. Finish Metal as a real text-generation backend via `MOX-130` through
   `MOX-132`.
5. Open NVIDIA explicitly via `MOX-140` through `MOX-147`.
6. Turn AMD truth into AMD execution via `MOX-150` through `MOX-154`.
7. Lock process-isolation, fallback-lattice, and migration-boundary decisions
   via `MOX-160`, `MOX-161`, and `MOX-170`.
8. Land the cutover contract from
   [CONFORMANCE_AND_EVIDENCE_CONTRACT.md](./CONFORMANCE_AND_EVIDENCE_CONTRACT.md)
   before hardening backend and app cutover work.
9. Add compute-market capability and evidence substrate via `MOX-171` through
   `MOX-175`.
10. Cut the app over via `OA-200` through `OA-203`.
11. Only after inference and embeddings are truthful, consider
   `MOX-176` through `MOX-178` for bounded `sandbox_execution`.

## Definition Of Done For "Replace Ollama"

The external Ollama dependency is not replaced until all of the following are
true:

- Mox can discover installed models from the local Ollama model store during
  migration
- Mox can report installed models, loaded models, and model metadata without
  calling the Ollama daemon
- Mox passes a repeatable Ollama-to-Mox conformance suite for catalog,
  generation, embeddings, prompt rendering, truncation, stop handling,
  streaming, and error semantics
- Mox has golden prompt and tokenizer fixtures for supported model families
  sourced from real GGUF or Ollama installs
- Mox can match or explicitly redefine prompt-template, BOS/EOS, and default
  stop behavior for the supported model families
- Mox has explicit context-window accounting, truncation, and over-limit refusal
  behavior for generation and embeddings
- Mox can warm, load, unload, and keep alive a local model lifecycle
- Mox has explicit paged-KV policy for long-context text generation or explicit
  refusal when that policy is unsupported
- Mox can decide whether a model may load based on memory planning, residency
  policy, and admission control
- Mox can execute the current text-generation path with the option surface the
  desktop already uses
- Mox can stream partial output and cancellation with stable final-chunk
  semantics
- metrics, receipts, capability surfaces, error taxonomy, and fallback states
  remain truthful
- model-store integrity verification and corruption diagnostics exist for the
  local catalog path
- performance acceptance thresholds for cutover are defined and met
- the desktop uses an app-owned local runtime seam instead of `reqwest` calls
  to Ollama
- the app no longer advertises "Ollama" when the runtime is now an in-repo Rust
  engine
- the repo has an explicit boundary between temporary Ollama-compat migration
  support and the long-term Mox-native model/runtime format

## Additional Definition Of Done For Mox As Compute-Market Substrate

Mox is not yet a credible compute-market substrate until all of the following
are also true:

- Mox can publish truthful capability-envelope fields for backend family,
  execution kind, model family or policy, accelerator vendor/family, compiled
  backend/toolchain features, memory, topology, concurrency posture, and
  latency posture
- batchability, queueing, admission, warm/cold behavior, and compile/cache
  posture are explicit runtime policy
- runtime evidence includes stable digests and summaries that can feed
  compute-market delivery proofs without re-deriving execution truth in app code
- runtime evidence includes execution-plan digest, compile digest, kernel count,
  bytes moved, queue wait, warm/cold load state, KV growth, backend/interface
  mode, and refusal/degraded reason codes
- multi-device or sharded execution is either explicitly supported for a product
  path or explicitly refused with stable diagnostics
- accelerator-sensitive offers can compare promised versus delivered topology
  and capability truth
- if `sandbox_execution` is added later, Mox exposes a bounded execution
  profile and machine-checkable evidence surface

## Non-Goals

This roadmap does not require:

- porting Ollama cloud or registry flows
- Modelfile parity as a first milestone
- OpenAI-compatible HTTP endpoints as the first milestone
- multimodal parity
- multi-runner LRU complexity before the one-model MVP lifecycle is solid
- raw accelerator trading before backend-specific compute products and
  capability envelopes are truthful

The right near-term target is smaller:

- one honest in-process local runtime
- one honest model catalog
- one honest text-generation product path
- explicit Metal, NVIDIA, and AMD backend truth
