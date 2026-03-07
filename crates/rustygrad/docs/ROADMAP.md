# Rustygrad Roadmap

> Status: active planning snapshot as of 2026-03-07.
>
> This file is the single roadmap for `crates/rustygrad/`. It consolidates the
> current open GitHub issues plus the missing issue set required to replace the
> external Ollama dependency with an in-repo Rust runtime built around
> tinygrad-style primitives and truthful backend surfaces.

## Objective

Replace the desktop's external Ollama dependency with an in-process Rust runtime
that:

- keeps app UX and provider orchestration in `apps/autopilot-desktop`
- moves reusable model, runtime, backend, and serving logic into
  `crates/rustygrad/*`
- reuses the subset of Ollama behavior OpenAgents actually needs today
- remains explicit and honest about backend readiness, fallback, and hardware
  support

This is not a plan to rebuild all of Ollama. It is a plan to replace the
desktop's actual Ollama dependency boundary.

## Ownership Rules

The roadmap must respect `docs/OWNERSHIP.md`:

- `crates/rustygrad/*` owns reusable tensor/IR/compiler/runtime/model/serve and
  provider-facing engine truth
- `apps/autopilot-desktop` owns the local runtime adapter, provider UX,
  inventory presentation, admission policy, and final cutover from Ollama HTTP
  calls
- `crates/rustygrad/*` must not absorb app-only UI or product orchestration

## Tinygrad-Style Implementation Rules

Rustygrad should follow the shape that makes Tinygrad useful, without trying to
port Tinygrad line by line:

- keep a small, inspectable primitive op surface and lower model families into
  that surface
- keep backend crates explicit; discovery, allocation, lowering, execution, and
  health reporting belong to backends, not model crates
- keep model formats and model stores separate from backend execution
- keep the serving surface library-first and in-process; any compatibility HTTP
  shim should be thin and optional
- never silently run on CPU while advertising Metal, AMD, or NVIDIA readiness

The first backend-complete primitive surface should be just large enough to run
the launch product paths:

- matmul / batched matmul
- embedding lookup / gather
- reshape / transpose / concat / slice
- cast / dequantize hooks
- elementwise add / mul / silu / gelu
- rmsnorm / layernorm
- RoPE
- softmax
- KV-cache read / append / update

## Current Reality

The checked-in README still says Rustygrad is in phase 0 bootstrap. That is no
longer the full story.

The best current code baseline is `origin/rustygrad3`, which already contains:

- artifact-backed weight bundle ingestion in `rustygrad-models`
- explicit quantization metadata and artifact truth
- model-backed CPU embeddings tests and services
- model-backed CPU text-generation tests and services
- partial Metal discovery, allocation, and submission groundwork

What still does not exist as a complete in-repo replacement:

- GGUF weight loading and tokenizer extraction
- chat-template extraction and model-family prompt rendering compatibility
- Ollama manifest/blob store compatibility
- installed-model and loaded-model catalog APIs equivalent to `tags`, `ps`, and
  `show`
- explicit context-window accounting and truncation/error policy
- warm / unload / keepalive lifecycle management for real local serving
- full sampling option parity with the current Ollama-backed desktop path
- model memory planning, residency policy, and load admission control
- streaming token delivery and cancellation semantics
- local runtime observability for warm/cold/session/health transitions
- model-store integrity verification and repair diagnostics
- backend-neutral local runtime error taxonomy and fallback policy
- explicit cutover performance thresholds
- a truthful accelerated text-generation path on Metal
- any NVIDIA backend surface
- AMD execution support beyond the current discovery/truth direction
- a long-term Rustygrad-native boundary beyond Ollama-compat migration support

Important status note:

- the current live GitHub issue stack is narrower than the full replacement plan
- the current issue stack covers Metal and AMD groundwork
- it does not yet cover GGUF, Ollama compatibility, NVIDIA, or the full
  text-generation replacement path

## Ollama Behaviors The Replacement Must Make Explicit

The missing work is not just "run the model in Rust." Ollama currently supplies
several behavioral contracts implicitly, and the Rustygrad roadmap needs to
track them explicitly.

From the local Ollama source reviewed for this update:

- prompt-template sourcing is not trivial:
  - `convert/tokenizer.go` accepts `chat_template` from `tokenizer_config.json`
    either as a plain string or as a named list and selects the `"default"`
    template when present
  - the same tokenizer metadata carries BOS/EOS and `add_bos_token` /
    `add_eos_token` behavior, while `generation_config.json` can override EOS
    token IDs
- chat truncation has defined semantics:
  - `server/prompt.go` drops old messages from the front until the rendered
    prompt fits
  - it preserves system messages and always keeps the latest message
  - image-bearing prompts also count image token cost during truncation
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
  - it floors `num_ctx` and raises it for some model capabilities
  - keepalive and load timeout are operational inputs, not just nice-to-have
- memory planning already exists as structured data:
  - `ml/device.go` exposes `BackendMemory`, per-device weight/cache/graph usage,
    and `ErrNoMem`
- model-store integrity already matters:
  - `manifest` and `server/internal/cache/blob` validate digest format and blob
    content, handle missing files, and carry compatibility logic for manifests
    and blob-addressable storage

That behavior is why the roadmap needs explicit issues for prompt rendering,
context budgets, memory admission, streaming, integrity, errors, and fallback.

## Current Open GitHub Issues

These are the current open issues in the active Rustygrad roadmap, verified on
2026-03-07 via `gh api`.

| Issue | Title | Why it matters to the replacement plan |
| --- | --- | --- |
| [#3150](https://github.com/OpenAgentsInc/openagents/issues/3150) | Rustygrad phase 3: Metal-accelerated product foundation | Active master task for the first truthful accelerated backend beyond CPU. |
| [#3154](https://github.com/OpenAgentsInc/openagents/issues/3154) | Wire Metal backend selection and truthful capability reporting through Rustygrad | Required so GPU claims survive provider/runtime serialization honestly. |
| [#3155](https://github.com/OpenAgentsInc/openagents/issues/3155) | Add CPU-vs-Metal parity coverage for supported Rustygrad product paths | Required before Metal-backed serving can be trusted. |
| [#3156](https://github.com/OpenAgentsInc/openagents/issues/3156) | Ship a tested Metal-backed rustygrad.embeddings path | First accelerated served-product milestone, but still embeddings-only. |
| [#3157](https://github.com/OpenAgentsInc/openagents/issues/3157) | Rustygrad phase 4: AMD backend architecture and truthful capability surfaces | Active master task for AMD discovery/readiness truth. |
| [#3158](https://github.com/OpenAgentsInc/openagents/issues/3158) | Define the Rustygrad AMD capability, topology, and risk model | Foundation for explicit KFD vs userspace reporting. |
| [#3159](https://github.com/OpenAgentsInc/openagents/issues/3159) | Implement AMD KFD discovery and health reporting in rustygrad-backend-amd-kfd | Required lower-risk AMD backend truth surface. |
| [#3160](https://github.com/OpenAgentsInc/openagents/issues/3160) | Implement AMD userspace probe and explicit opt-in gating in rustygrad-backend-amd-userspace | Required higher-risk AMD backend truth surface. |
| [#3161](https://github.com/OpenAgentsInc/openagents/issues/3161) | Extend Rustygrad provider truth with AMD mode, topology, and recovery posture | Required so AMD-specific truth survives capability publication. |
| [#3162](https://github.com/OpenAgentsInc/openagents/issues/3162) | Publish Rustygrad AMD runbooks and readiness validation coverage | Closes the AMD truth/doc/testing loop. |

Note:

- the phase-3 issue body still references `#3151` through `#3153`, but those
  are not currently open
- some of that substrate work appears to exist on `origin/rustygrad3`, which is
  another reason the roadmap must not rely on the checked-in phase-0 docs alone

### What the open issue stack does not cover

The current open issues are necessary but insufficient for replacing Ollama.
They do not yet track:

- landing the `rustygrad3` model-backed CPU baseline on `main`
- GGUF parsing and tokenizer support
- chat-template extraction and prompt rendering compatibility
- Ollama store compatibility
- installed-model and loaded-model catalog APIs
- context-window accounting and truncation policy
- generation option parity with the current desktop path
- deterministic sampler correctness and replay behavior
- local runtime lifecycle equivalent to `show` / `ps` / `generate` behavior
- model memory planning and admission control
- streaming semantics and cancellation behavior
- embeddings batch and metadata semantics
- local runtime observability for session, queue, memory, and health changes
- model-store integrity verification and cache diagnostics
- backend-neutral error taxonomy
- explicit fallback and degraded-state policy
- performance thresholds for cutover
- Metal text generation
- NVIDIA support
- AMD execution kernels and served-product parity after the current AMD truth
  phase
- the post-migration boundary between Ollama compatibility and a Rustygrad-native
  model/runtime format

## Proposed Issues Not Yet In GitHub

These issue stubs should be opened if the repo is serious about replacing the
desktop Ollama dependency with Rustygrad.

### Epic A: Adopt the real CPU baseline

| Local ID | Proposed issue | Crates | Depends on | Why it exists |
| --- | --- | --- | --- | --- |
| `RGR-100` | Merge or recreate the `rustygrad3` phase-2 CPU baseline on `main` | `rustygrad-models`, `rustygrad-serve`, `rustygrad-provider`, `rustygrad-runtime` | none | The replacement plan cannot start from the stale phase-0 `main` state. |
| `RGR-101` | Refresh Rustygrad docs to match post-branch reality | `README.md`, `docs/BACKENDS.md`, `docs/INFERENCE_ENGINE.md`, `docs/ROADMAP.md` | `RGR-100` | Prevents the repo from continuing to describe phase-0 status after CPU model-backed products exist. |

### Epic B: GGUF, tokenizer, and prompt-compatibility support for Ollama migration

Decision: if the goal is "replace Ollama," GGUF and Ollama-store compatibility
are required. The safetensors artifact path remains useful, but it is not enough
on its own.

| Local ID | Proposed issue | Crates | Depends on | Why it exists |
| --- | --- | --- | --- | --- |
| `RGR-110` | Add `WeightFormat::Gguf` and a reusable GGUF metadata/tensor loader | `rustygrad-models` | `RGR-100` | Required to read the model format that Ollama actually points at today. |
| `RGR-111` | Implement tokenizer loading from GGUF metadata for SentencePiece and GPT-style BPE families | `rustygrad-models` | `RGR-110` | The current fixture tokenizer is not enough for real model parity. |
| `RGR-112` | Add GGUF-backed decoder model-family adapters for the first launch families (`llama`, `qwen`, `mistral`) | `rustygrad-models`, `rustygrad-serve` | `RGR-110`, `RGR-111` | Replaces model-specific construction currently hidden behind Ollama. |
| `RGR-113` | Add GGUF-backed embeddings model-family adapters for the first supported embedding families | `rustygrad-models`, `rustygrad-serve` | `RGR-110`, `RGR-111` | Keeps embeddings real rather than marketing-only. |
| `RGR-114` | Implement chat-template extraction and prompt-rendering compatibility for supported model families | `rustygrad-models`, `rustygrad-serve` | `RGR-110`, `RGR-111`, `RGR-112` | GGUF + tokenizer is not enough; the runtime also needs template selection, role rendering, BOS/EOS behavior, and stop/default prompt formatting parity for supported families. |

### Epic C: Ollama-compatible model catalog and local runtime lifecycle

The desktop does not need `pull`, `push`, Modelfiles, or cloud registry parity.
It does need a local model catalog and lifecycle that cover the existing
`tags`/`ps`/`show`/`generate` boundary.

| Local ID | Proposed issue | Crates | Depends on | Why it exists |
| --- | --- | --- | --- | --- |
| `RGR-120` | Add `rustygrad-catalog` for Ollama manifest/blob discovery and model resolution | new `rustygrad-catalog` crate | `RGR-110` | Lets Rustygrad discover already-installed Ollama models without the daemon. |
| `RGR-121` | Implement installed-model listing and model inspection APIs equivalent to `tags` and `show` | `rustygrad-catalog`, `rustygrad-serve` | `RGR-120`, `RGR-112` | Replaces the desktop's model discovery and validation calls. |
| `RGR-122` | Implement loaded-model registry, warm/load/unload, and keepalive semantics equivalent to `ps` and empty `generate` warmups | `rustygrad-serve`, `rustygrad-runtime` | `RGR-120`, `RGR-121` | Replaces the scheduler/lifecycle subset that the desktop actually depends on. |
| `RGR-123` | Expand generation options to cover `temperature`, `top_k`, `top_p`, penalties, `seed`, and `stop` | `rustygrad-serve` | `RGR-100`, `RGR-112` | Matches the option surface already normalized by the app today. |
| `RGR-124` | Add generation metrics and provenance for prompt tokens, output tokens, load time, total time, warm/cold state, and plan digest | `rustygrad-serve`, `rustygrad-provider` | `RGR-122`, `RGR-123` | Replaces the provenance and metrics the desktop already projects into receipts and UI. |
| `RGR-125` | Publish a library-first local runtime API for `list_models`, `show_model`, `loaded_models`, `warm_model`, `unload_model`, `generate`, and `embed` | `rustygrad-serve`, `rustygrad-provider` | `RGR-121`, `RGR-122`, `RGR-123`, `RGR-124` | Creates the in-process replacement boundary that the desktop can call directly. |
| `RGR-126` | Add GGUF-backed KV-cache ownership and deterministic session lifecycle for text generation | `rustygrad-serve` | `RGR-112`, `RGR-122` | Required for the text-generation job flow already proven on the CPU artifact branch. |

### Epic D: Behavioral contract and local serving policy

These issues cover the runtime behavior that Ollama currently supplies
implicitly and that the desktop will notice immediately during cutover.

| Local ID | Proposed issue | Crates | Depends on | Why it exists |
| --- | --- | --- | --- | --- |
| `RGR-127` | Add explicit context-window accounting, truncation policy, and over-limit error semantics | `rustygrad-models`, `rustygrad-serve` | `RGR-112`, `RGR-114`, `RGR-122` | Ollama currently enforces concrete prompt/embedding fit behavior, including truncate-vs-shift choices; Rustygrad needs explicit token budgeting, truncation, and refusal rules instead of ad hoc limits. |
| `RGR-128` | Add deterministic sampler implementation and replay coverage for supported generation options | `rustygrad-serve`, `rustygrad-runtime` | `RGR-123` | Option parity is not enough without a real sampler with seed determinism, stop behavior, and replay coverage. |
| `RGR-129` | Add model memory planning, residency policy, and admission control for local serving | `rustygrad-serve`, `rustygrad-runtime`, `rustygrad-provider` | `RGR-121`, `RGR-122`, `RGR-126` | Warm/load/unload is underspecified without preflight memory estimates, residency rules, and refusal behavior under load pressure. |
| `RGR-133` | Add streaming token generation semantics and cancellation behavior for the local runtime API | `rustygrad-serve`, `rustygrad-provider` | `RGR-124`, `RGR-125`, `RGR-128` | The cutover needs explicit partial-output, cancellation, final-chunk, and post-stream-error semantics instead of assuming the old HTTP stream behavior. |
| `RGR-134` | Add embeddings API parity, batch semantics, and model metadata reporting | `rustygrad-serve`, `rustygrad-provider` | `RGR-113`, `RGR-121`, `RGR-124` | Embeddings need explicit vector-dimension, normalization, batching, failure, and metadata rules rather than being a future afterthought. |
| `RGR-135` | Add local model-store integrity verification and cache-repair diagnostics | `rustygrad-catalog`, `rustygrad-models` | `RGR-120` | Reading the Ollama store is not enough; the runtime also needs digest checks, corruption behavior, and repair diagnostics. |
| `RGR-136` | Define backend-neutral local runtime error taxonomy and desktop-facing diagnostics | `rustygrad-serve`, `rustygrad-provider`, `rustygrad-runtime` | `RGR-121`, `RGR-122`, `RGR-124` | Replacing Ollama requires a stable error model for model-not-found, tokenizer mismatch, unsupported quantization, OOM, timeout, cache exhaustion, device loss, and refusal cases. |
| `RGR-137` | Add explicit backend fallback, refusal, and degraded-state policy for served products | `rustygrad-runtime`, `rustygrad-provider`, backend crates | `RGR-129`, `RGR-136`, current [#3154](https://github.com/OpenAgentsInc/openagents/issues/3154) | "No silent CPU fallback" must become a concrete, testable policy for capability changes, degraded states, and refusal behavior. |
| `RGR-138` | Define performance acceptance thresholds and cutover gates for Rustygrad runtime replacement | `rustygrad-serve`, `rustygrad-provider`, backend crates | `RGR-124` | Parity tests alone do not decide cutover; the team needs explicit thresholds for load latency, first-token latency, throughput, warm/cold behavior, and tolerance rules. |
| `RGR-139` | Decide and document LoRA/adapter support policy for the Ollama replacement boundary | `rustygrad-models`, `rustygrad-catalog`, `rustygrad-serve` | `RGR-110` | Even if adapters are deferred, the migration boundary needs an explicit policy instead of accidental incompatibility. |
| `RGR-159` | Add local runtime observability for warm/cold transitions, active sessions, queue depth, memory footprint, and backend health changes | `rustygrad-serve`, `rustygrad-provider`, `rustygrad-runtime` | `RGR-124`, `RGR-129`, `RGR-136` | The desktop cutover will be hard to debug without explicit runtime-state observability beyond per-request metrics. |

### Epic E: Metal beyond embeddings

The current open Metal phase is useful, but replacing Ollama requires a truthful
text-generation path, not just embeddings.

| Local ID | Proposed issue | Crates | Depends on | Why it exists |
| --- | --- | --- | --- | --- |
| `RGR-130` | Add Metal lowering/kernel coverage for the minimum text-generation primitive set | `rustygrad-backend-metal`, `rustygrad-compiler`, `rustygrad-runtime` | current [#3154](https://github.com/OpenAgentsInc/openagents/issues/3154), [#3155](https://github.com/OpenAgentsInc/openagents/issues/3155), [#3156](https://github.com/OpenAgentsInc/openagents/issues/3156), `RGR-112`, `RGR-126` | Embeddings-only Metal is not enough to replace the current Ollama path. |
| `RGR-131` | Add CPU-vs-Metal parity coverage for the supported text-generation product path | `rustygrad-backend-metal`, `rustygrad-serve` | `RGR-130` | Required before Metal-backed text generation is believable. |
| `RGR-132` | Ship a tested Metal-backed `rustygrad.text_generation` path | `rustygrad-backend-metal`, `rustygrad-serve`, `rustygrad-provider` | `RGR-130`, `RGR-131` | Closes the gap between the current embeddings-first Metal plan and the actual Ollama replacement target. |

### Epic F: NVIDIA backend architecture and truthful capability surfaces

If the replacement is meant to be broad local-runtime coverage rather than an
Apple-plus-AMD-only story, NVIDIA must be an explicit roadmap lane.

| Local ID | Proposed issue | Crates | Depends on | Why it exists |
| --- | --- | --- | --- | --- |
| `RGR-140` | Rustygrad phase 5: NVIDIA backend architecture and truthful capability surfaces | `rustygrad-backend-cuda` or equivalent, `rustygrad-runtime`, `rustygrad-provider` | `RGR-100` | Creates the missing master task for NVIDIA instead of treating it as implied future work. |
| `RGR-141` | Define the Rustygrad NVIDIA capability, topology, and risk model | `rustygrad-runtime`, `rustygrad-provider` | `RGR-140` | Gives CUDA/NVIDIA the same explicit truth model already planned for AMD. |
| `RGR-142` | Implement NVIDIA discovery and health reporting | `rustygrad-backend-cuda` | `RGR-141` | Makes GPU availability and degraded states explicit rather than guessed. |
| `RGR-143` | Add CUDA allocator, buffer, stream, and command submission substrate | `rustygrad-backend-cuda`, `rustygrad-runtime` | `RGR-142` | Backend execution cannot start until the runtime substrate exists. |
| `RGR-144` | Add CUDA lowering and kernel coverage for the minimum served-product primitive set | `rustygrad-backend-cuda`, `rustygrad-compiler` | `RGR-143` | Implements the tinygrad-style primitive surface on NVIDIA. |
| `RGR-145` | Wire NVIDIA backend selection and truthful capability reporting through Rustygrad | `rustygrad-runtime`, `rustygrad-provider`, `rustygrad-backend-cuda` | `RGR-142`, `RGR-144` | Required so provider/runtime contracts remain explicit and replay-safe. |
| `RGR-146` | Add CPU-vs-NVIDIA parity coverage for the first supported served product path | `rustygrad-backend-cuda`, `rustygrad-serve` | `RGR-144`, `RGR-145` | Prevents "CUDA works" claims without parity evidence. |
| `RGR-147` | Ship the first tested NVIDIA-backed served product path | `rustygrad-backend-cuda`, `rustygrad-serve`, `rustygrad-provider` | `RGR-146` | Makes NVIDIA real instead of aspirational. |

### Epic G: AMD execution after the current truth phase

The current AMD issues are about truthful discovery and reporting. They are not
the same thing as a model-execution roadmap.

| Local ID | Proposed issue | Crates | Depends on | Why it exists |
| --- | --- | --- | --- | --- |
| `RGR-150` | Rustygrad phase 6: AMD served-product execution path | `rustygrad-backend-amd-kfd`, `rustygrad-backend-amd-userspace`, `rustygrad-runtime`, `rustygrad-provider` | current [#3157](https://github.com/OpenAgentsInc/openagents/issues/3157), [#3158](https://github.com/OpenAgentsInc/openagents/issues/3158), [#3159](https://github.com/OpenAgentsInc/openagents/issues/3159), [#3160](https://github.com/OpenAgentsInc/openagents/issues/3160), [#3161](https://github.com/OpenAgentsInc/openagents/issues/3161), [#3162](https://github.com/OpenAgentsInc/openagents/issues/3162) | Turns AMD from truthful detection into actual served-product execution. |
| `RGR-151` | Add AMD KFD lowering and kernel coverage for the first supported primitive set | `rustygrad-backend-amd-kfd`, `rustygrad-compiler`, `rustygrad-runtime` | `RGR-150` | KFD is the lower-risk AMD execution path and should be first. |
| `RGR-152` | Wire served-product capability gating for AMD KFD separately from AMD userspace | `rustygrad-provider`, `rustygrad-runtime`, AMD backend crates | `RGR-151` | Preserves the KFD/userspace split even once KFD execution exists. |
| `RGR-153` | Add CPU-vs-AMD KFD parity coverage for the first supported served product path | `rustygrad-backend-amd-kfd`, `rustygrad-serve` | `RGR-151`, `RGR-152` | Prevents overclaiming AMD inference readiness. |
| `RGR-154` | Ship the first tested AMD KFD-backed served product path and keep AMD userspace explicitly gated | `rustygrad-backend-amd-kfd`, `rustygrad-serve`, `rustygrad-provider` | `RGR-153` | Closes the gap between AMD truth surfaces and actual runtime value without pretending userspace is equally ready. |

### Epic H: Cross-subtree cutover tasks outside Rustygrad ownership

These are required to finish the Ollama replacement, but they belong in
`apps/autopilot-desktop`, not in Rustygrad crates.

| Local ID | Proposed issue | Owner | Depends on | Why it exists |
| --- | --- | --- | --- | --- |
| `OA-200` | Rename `OllamaExecutionMetrics` and `OllamaExecutionProvenance` to backend-neutral names | `apps/autopilot-desktop` | none | Removes the naming debt before the runtime stops being Ollama-specific. |
| `OA-201` | Introduce an app-owned `LocalInferenceRuntime` trait and `RustygradRuntimeAdapter` | `apps/autopilot-desktop` | `RGR-125` | Preserves the app seam while swapping out the runtime implementation. |
| `OA-202` | Switch desktop default from external Ollama HTTP calls to the in-process Rustygrad runtime | `apps/autopilot-desktop` | `OA-201`, `RGR-121`, `RGR-122`, `RGR-123`, `RGR-124`, `RGR-125` | This is the actual cutover step. |
| `OA-203` | Remove the external Ollama dependency and clean up provider/UI wording | `apps/autopilot-desktop` | `OA-202` | Finishes the product truth cleanup after parity is proven. |

### Epic I: Runtime isolation and post-migration boundary

These are cross-cutting architectural issues that should be decided before the
cutover hardens the wrong assumptions.

| Local ID | Proposed issue | Crates | Depends on | Why it exists |
| --- | --- | --- | --- | --- |
| `RGR-160` | Define in-process vs subprocess isolation policy for Rustygrad local serving | `rustygrad-serve`, `rustygrad-runtime`, backend crates | `RGR-125`, `RGR-136` | The desktop needs an explicit decision on whether local serving crashes, backend resets, and device loss can take down the app process. |
| `RGR-170` | Define the boundary between Ollama-compat migration support and the long-term Rustygrad-native model/runtime format | `rustygrad-models`, `rustygrad-catalog`, `rustygrad-serve` | `RGR-120`, `RGR-125` | Migration support should not permanently dictate Rustygrad storage and runtime architecture. |

## Recommended Order

The shortest honest path is:

1. `RGR-100`: land the `rustygrad3` CPU baseline on `main`
2. `RGR-110` through `RGR-114`: add GGUF, tokenizer, and chat-template/model-family prompt compatibility
3. `RGR-120` through `RGR-126`: add Ollama-compatible catalog and local runtime
   lifecycle
4. `RGR-127` through `RGR-129`, `RGR-133` through `RGR-139`, and `RGR-159`:
   define the behavioral contract for context, streaming, embeddings, sampling,
   memory, integrity, errors, fallback, observability, performance gates, and
   adapter policy before cutover
5. finish the current open Metal work: [#3150](https://github.com/OpenAgentsInc/openagents/issues/3150), [#3154](https://github.com/OpenAgentsInc/openagents/issues/3154), [#3155](https://github.com/OpenAgentsInc/openagents/issues/3155), [#3156](https://github.com/OpenAgentsInc/openagents/issues/3156)
6. `RGR-130` through `RGR-132`: make Metal useful for text generation, not just
   embeddings
7. `RGR-140` through `RGR-147`: add NVIDIA as a first-class backend track
8. finish the current open AMD truth work: [#3157](https://github.com/OpenAgentsInc/openagents/issues/3157), [#3158](https://github.com/OpenAgentsInc/openagents/issues/3158), [#3159](https://github.com/OpenAgentsInc/openagents/issues/3159), [#3160](https://github.com/OpenAgentsInc/openagents/issues/3160), [#3161](https://github.com/OpenAgentsInc/openagents/issues/3161), [#3162](https://github.com/OpenAgentsInc/openagents/issues/3162)
9. `RGR-150` through `RGR-154`: add AMD execution only after AMD truth exists
10. `RGR-160` and `RGR-170`: lock the process-isolation posture and the
    post-migration Rustygrad-native boundary before the cutover hardens old
    assumptions
11. `OA-200` through `OA-203`: cut the desktop over and remove the daemon

## Definition Of Done For "Replace Ollama"

The external Ollama dependency is not replaced until all of the following are
true:

- Rustygrad can discover installed models from the local Ollama model store
  during migration
- Rustygrad can report installed models, loaded models, and model metadata
  without calling the Ollama daemon
- Rustygrad can match or intentionally and explicitly redefine prompt-template,
  BOS/EOS, and default stop behavior for the supported model families
- Rustygrad has explicit context-window accounting, truncation, and over-limit
  refusal behavior for generation and embeddings
- Rustygrad can warm, load, unload, and keep alive a local model lifecycle
- Rustygrad can decide whether a model may load based on memory planning,
  residency policy, and admission control rather than optimistic load attempts
- Rustygrad can execute the current text-generation path with the option surface
  the desktop already uses
- Rustygrad can stream partial output and cancellation with stable final-chunk
  semantics
- metrics, receipts, capability surfaces, error taxonomy, and fallback states
  remain truthful
- model-store integrity verification and corruption diagnostics exist for the
  local catalog path
- performance acceptance thresholds for cutover are defined and met
- the desktop uses an app-owned local runtime seam instead of `reqwest` calls to
  Ollama
- the app no longer advertises "Ollama" when the runtime is now an in-repo Rust
  engine
- the repo has an explicit boundary between temporary Ollama-compat migration
  support and the long-term Rustygrad-native model/runtime format

## Non-Goals For This Roadmap

This roadmap does not require:

- porting Ollama's cloud/registry flows
- Modelfile parity
- OpenAI-compatible HTTP endpoints as a first milestone
- multimodal parity
- multi-runner LRU complexity before the one-model MVP lifecycle is solid

The right near-term target is smaller:

- one honest in-process local runtime
- one honest model catalog
- one honest text-generation product path
- explicit Metal, NVIDIA, and AMD backend truth
