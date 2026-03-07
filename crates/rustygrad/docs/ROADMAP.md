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
- Ollama manifest/blob store compatibility
- installed-model and loaded-model catalog APIs equivalent to `tags`, `ps`, and
  `show`
- warm / unload / keepalive lifecycle management for real local serving
- full sampling option parity with the current Ollama-backed desktop path
- a truthful accelerated text-generation path on Metal
- any NVIDIA backend surface
- AMD execution support beyond the current discovery/truth direction

Important status note:

- the current live GitHub issue stack is narrower than the full replacement plan
- the current issue stack covers Metal and AMD groundwork
- it does not yet cover GGUF, Ollama compatibility, NVIDIA, or the full
  text-generation replacement path

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
- Ollama store compatibility
- installed-model and loaded-model catalog APIs
- generation option parity with the current desktop path
- local runtime lifecycle equivalent to `show` / `ps` / `generate` behavior
- Metal text generation
- NVIDIA support
- AMD execution kernels and served-product parity after the current AMD truth
  phase

## Proposed Issues Not Yet In GitHub

These issue stubs should be opened if the repo is serious about replacing the
desktop Ollama dependency with Rustygrad.

### Epic A: Adopt the real CPU baseline

| Local ID | Proposed issue | Crates | Depends on | Why it exists |
| --- | --- | --- | --- | --- |
| `RGR-100` | Merge or recreate the `rustygrad3` phase-2 CPU baseline on `main` | `rustygrad-models`, `rustygrad-serve`, `rustygrad-provider`, `rustygrad-runtime` | none | The replacement plan cannot start from the stale phase-0 `main` state. |
| `RGR-101` | Refresh Rustygrad docs to match post-branch reality | `README.md`, `docs/BACKENDS.md`, `docs/INFERENCE_ENGINE.md`, `docs/ROADMAP.md` | `RGR-100` | Prevents the repo from continuing to describe phase-0 status after CPU model-backed products exist. |

### Epic B: GGUF and model-family support for Ollama compatibility

Decision: if the goal is "replace Ollama," GGUF and Ollama-store compatibility
are required. The safetensors artifact path remains useful, but it is not enough
on its own.

| Local ID | Proposed issue | Crates | Depends on | Why it exists |
| --- | --- | --- | --- | --- |
| `RGR-110` | Add `WeightFormat::Gguf` and a reusable GGUF metadata/tensor loader | `rustygrad-models` | `RGR-100` | Required to read the model format that Ollama actually points at today. |
| `RGR-111` | Implement tokenizer loading from GGUF metadata for SentencePiece and GPT-style BPE families | `rustygrad-models` | `RGR-110` | The current fixture tokenizer is not enough for real model parity. |
| `RGR-112` | Add GGUF-backed decoder model-family adapters for the first launch families (`llama`, `qwen`, `mistral`) | `rustygrad-models`, `rustygrad-serve` | `RGR-110`, `RGR-111` | Replaces model-specific construction currently hidden behind Ollama. |
| `RGR-113` | Add GGUF-backed embeddings model-family adapters for the first supported embedding families | `rustygrad-models`, `rustygrad-serve` | `RGR-110`, `RGR-111` | Keeps embeddings real rather than marketing-only. |

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

### Epic D: Metal beyond embeddings

The current open Metal phase is useful, but replacing Ollama requires a truthful
text-generation path, not just embeddings.

| Local ID | Proposed issue | Crates | Depends on | Why it exists |
| --- | --- | --- | --- | --- |
| `RGR-130` | Add Metal lowering/kernel coverage for the minimum text-generation primitive set | `rustygrad-backend-metal`, `rustygrad-compiler`, `rustygrad-runtime` | current [#3154](https://github.com/OpenAgentsInc/openagents/issues/3154), [#3155](https://github.com/OpenAgentsInc/openagents/issues/3155), [#3156](https://github.com/OpenAgentsInc/openagents/issues/3156), `RGR-112`, `RGR-126` | Embeddings-only Metal is not enough to replace the current Ollama path. |
| `RGR-131` | Add CPU-vs-Metal parity coverage for the supported text-generation product path | `rustygrad-backend-metal`, `rustygrad-serve` | `RGR-130` | Required before Metal-backed text generation is believable. |
| `RGR-132` | Ship a tested Metal-backed `rustygrad.text_generation` path | `rustygrad-backend-metal`, `rustygrad-serve`, `rustygrad-provider` | `RGR-130`, `RGR-131` | Closes the gap between the current embeddings-first Metal plan and the actual Ollama replacement target. |

### Epic E: NVIDIA backend architecture and truthful capability surfaces

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

### Epic F: AMD execution after the current truth phase

The current AMD issues are about truthful discovery and reporting. They are not
the same thing as a model-execution roadmap.

| Local ID | Proposed issue | Crates | Depends on | Why it exists |
| --- | --- | --- | --- | --- |
| `RGR-150` | Rustygrad phase 6: AMD served-product execution path | `rustygrad-backend-amd-kfd`, `rustygrad-backend-amd-userspace`, `rustygrad-runtime`, `rustygrad-provider` | current [#3157](https://github.com/OpenAgentsInc/openagents/issues/3157), [#3158](https://github.com/OpenAgentsInc/openagents/issues/3158), [#3159](https://github.com/OpenAgentsInc/openagents/issues/3159), [#3160](https://github.com/OpenAgentsInc/openagents/issues/3160), [#3161](https://github.com/OpenAgentsInc/openagents/issues/3161), [#3162](https://github.com/OpenAgentsInc/openagents/issues/3162) | Turns AMD from truthful detection into actual served-product execution. |
| `RGR-151` | Add AMD KFD lowering and kernel coverage for the first supported primitive set | `rustygrad-backend-amd-kfd`, `rustygrad-compiler`, `rustygrad-runtime` | `RGR-150` | KFD is the lower-risk AMD execution path and should be first. |
| `RGR-152` | Wire served-product capability gating for AMD KFD separately from AMD userspace | `rustygrad-provider`, `rustygrad-runtime`, AMD backend crates | `RGR-151` | Preserves the KFD/userspace split even once KFD execution exists. |
| `RGR-153` | Add CPU-vs-AMD KFD parity coverage for the first supported served product path | `rustygrad-backend-amd-kfd`, `rustygrad-serve` | `RGR-151`, `RGR-152` | Prevents overclaiming AMD inference readiness. |
| `RGR-154` | Ship the first tested AMD KFD-backed served product path and keep AMD userspace explicitly gated | `rustygrad-backend-amd-kfd`, `rustygrad-serve`, `rustygrad-provider` | `RGR-153` | Closes the gap between AMD truth surfaces and actual runtime value without pretending userspace is equally ready. |

### Epic G: Cross-subtree cutover tasks outside Rustygrad ownership

These are required to finish the Ollama replacement, but they belong in
`apps/autopilot-desktop`, not in Rustygrad crates.

| Local ID | Proposed issue | Owner | Depends on | Why it exists |
| --- | --- | --- | --- | --- |
| `OA-200` | Rename `OllamaExecutionMetrics` and `OllamaExecutionProvenance` to backend-neutral names | `apps/autopilot-desktop` | none | Removes the naming debt before the runtime stops being Ollama-specific. |
| `OA-201` | Introduce an app-owned `LocalInferenceRuntime` trait and `RustygradRuntimeAdapter` | `apps/autopilot-desktop` | `RGR-125` | Preserves the app seam while swapping out the runtime implementation. |
| `OA-202` | Switch desktop default from external Ollama HTTP calls to the in-process Rustygrad runtime | `apps/autopilot-desktop` | `OA-201`, `RGR-121`, `RGR-122`, `RGR-123`, `RGR-124`, `RGR-125` | This is the actual cutover step. |
| `OA-203` | Remove the external Ollama dependency and clean up provider/UI wording | `apps/autopilot-desktop` | `OA-202` | Finishes the product truth cleanup after parity is proven. |

## Recommended Order

The shortest honest path is:

1. `RGR-100`: land the `rustygrad3` CPU baseline on `main`
2. `RGR-110` through `RGR-113`: add GGUF and real tokenizer/model-family support
3. `RGR-120` through `RGR-126`: add Ollama-compatible catalog and local runtime
   lifecycle
4. finish the current open Metal work: [#3150](https://github.com/OpenAgentsInc/openagents/issues/3150), [#3154](https://github.com/OpenAgentsInc/openagents/issues/3154), [#3155](https://github.com/OpenAgentsInc/openagents/issues/3155), [#3156](https://github.com/OpenAgentsInc/openagents/issues/3156)
5. `RGR-130` through `RGR-132`: make Metal useful for text generation, not just
   embeddings
6. `RGR-140` through `RGR-147`: add NVIDIA as a first-class backend track
7. finish the current open AMD truth work: [#3157](https://github.com/OpenAgentsInc/openagents/issues/3157), [#3158](https://github.com/OpenAgentsInc/openagents/issues/3158), [#3159](https://github.com/OpenAgentsInc/openagents/issues/3159), [#3160](https://github.com/OpenAgentsInc/openagents/issues/3160), [#3161](https://github.com/OpenAgentsInc/openagents/issues/3161), [#3162](https://github.com/OpenAgentsInc/openagents/issues/3162)
8. `RGR-150` through `RGR-154`: add AMD execution only after AMD truth exists
9. `OA-200` through `OA-203`: cut the desktop over and remove the daemon

## Definition Of Done For "Replace Ollama"

The external Ollama dependency is not replaced until all of the following are
true:

- Rustygrad can discover installed models from the local Ollama model store
- Rustygrad can report installed models, loaded models, and model metadata
  without calling the Ollama daemon
- Rustygrad can warm, load, unload, and keep alive a local model lifecycle
- Rustygrad can execute the current text-generation path with the option surface
  the desktop already uses
- metrics, receipts, capability surfaces, and fallback states remain truthful
- the desktop uses an app-owned local runtime seam instead of `reqwest` calls to
  Ollama
- the app no longer advertises "Ollama" when the runtime is now an in-repo Rust
  engine

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
