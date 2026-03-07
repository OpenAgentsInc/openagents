# 2026-03-07 Rustygrad Ollama Replacement Audit

> Historical note: This audit is a point-in-time snapshot from its date. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, and `docs/OWNERSHIP.md`. File paths, issue states, and implementation-status claims here may be superseded by later commits.

Author: Codex  
Status: Complete  
Audit target: current checkout at `09e8ca91ec`

## Objective

Determine how OpenAgents can replace the current external Ollama dependency with an in-repo Rust implementation, using and extending `crates/rustygrad/`, without violating MVP scope or ownership boundaries.

## Sources Reviewed

- Product and ownership authority:
  - `docs/MVP.md`
  - `docs/OWNERSHIP.md`
- Existing local audits:
  - `docs/audits/2026-03-07-rustygrad-implementation-and-gap-audit.md`
  - `docs/audits/2026-03-06-ollama-nip90-compute-provider-audit.md`
- Current OpenAgents Ollama-facing implementation:
  - `apps/autopilot-desktop/src/ollama_execution.rs`
  - `apps/autopilot-desktop/src/input/reducers/jobs.rs`
  - `apps/autopilot-desktop/src/state/provider_runtime.rs`
  - `apps/autopilot-desktop/src/provider_nip90_lane.rs`
  - `apps/autopilot-desktop/src/kernel_control.rs`
  - `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs`
  - `apps/autopilot-desktop/src/pane_renderer.rs`
  - `apps/autopilot-desktop/src/apple_fm_bridge.rs`
- Rustygrad docs and source:
  - `crates/rustygrad/README.md`
  - `crates/rustygrad/docs/ARCHITECTURE.md`
  - `crates/rustygrad/docs/INFERENCE_ENGINE.md`
  - `crates/rustygrad/rustygrad-runtime/src/lib.rs`
  - `crates/rustygrad/rustygrad-backend-cpu/src/lib.rs`
  - `crates/rustygrad/rustygrad-backend-metal/src/lib.rs`
  - `crates/rustygrad/rustygrad-backend-amd-kfd/src/lib.rs`
  - `crates/rustygrad/rustygrad-backend-amd-userspace/src/lib.rs`
  - `crates/rustygrad/rustygrad-models/src/lib.rs`
  - `crates/rustygrad/rustygrad-serve/src/lib.rs`
  - `crates/rustygrad/rustygrad-provider/src/lib.rs`
- Local Ollama docs and upstream source:
  - `/Users/christopherdavid/code/ollama/README.md`
  - `/Users/christopherdavid/code/ollama/ARCHITECTURE_AND_RUST_PORT.md`
  - `/Users/christopherdavid/code/ollama/ollama/api/types.go`
  - `/Users/christopherdavid/code/ollama/ollama/manifest/manifest.go`
  - `/Users/christopherdavid/code/ollama/ollama/server/model.go`
  - `/Users/christopherdavid/code/ollama/ollama/server/routes.go`
  - `/Users/christopherdavid/code/ollama/ollama/server/sched.go`
  - `/Users/christopherdavid/code/ollama/ollama/llm/server.go`
  - `/Users/christopherdavid/code/ollama/ollama/ml/backend.go`
  - `/Users/christopherdavid/code/ollama/ollama/ml/backend/ggml/ggml.go`
  - `/Users/christopherdavid/code/ollama/ollama/model/model.go`
  - `/Users/christopherdavid/code/ollama/ollama/model/models/llama/model.go`
  - `/Users/christopherdavid/code/ollama/ollama/runner/ollamarunner/runner.go`
  - `/Users/christopherdavid/code/ollama/ollama/fs/ggml/ggml.go`
  - `/Users/christopherdavid/code/ollama/ollama/fs/ggml/gguf.go`

## Executive Verdict

Yes, OpenAgents can replace the current external Ollama integration with an in-repo Rust runtime, but not by doing a literal one-file swap from `reqwest` calls to current `rustygrad` on `main`.

The correct path is:

1. Keep the desktop integration seam app-owned.
2. Move model catalog, GGUF loading, model execution, lifecycle, and metrics into reusable Rustygrad crates.
3. Start with the exact subset OpenAgents actually uses today, not the full Ollama product.

The most important conclusion is scope:

- We do not need to port all of Ollama.
- We do need to replace all current desktop behaviors that presently depend on the local Ollama daemon.
- We cannot do that truthfully on current `main` until Rustygrad grows from a reference engine into a real model-serving runtime.

The first hard blocker is already documented in `docs/audits/2026-03-07-rustygrad-implementation-and-gap-audit.md`:

- current `main` Rustygrad is still a phase-0-plus-phase-1 reference engine
- it does not yet have merged model-backed GGUF loading and real model execution
- it has no accelerated backend implementation at all

So the correct short answer is:

> replace the external Ollama daemon with a Rustygrad-backed local runtime, but only after recovering model-backed Rustygrad and adding an Ollama-compatible local model catalog and GGUF execution path.

## What The Current OpenAgents "Ollama Integration" Actually Is

The replacement boundary is larger than `apps/autopilot-desktop/src/ollama_execution.rs`.

### App-owned execution worker

`apps/autopilot-desktop/src/ollama_execution.rs` currently owns:

- local-only base URL validation
- installed model discovery via `GET /api/tags`
- loaded model discovery via `GET /api/ps`
- model validation via `POST /api/show`
- warm/unload lifecycle via empty `POST /api/generate` requests with `keep_alive`
- text generation via `POST /api/generate`
- normalization of prompt and generation params
- provenance and metrics snapshots stored in app state

This is already an app-owned seam, which is good. It means the replacement should preserve the seam and swap the implementation under it.

### Provider routing and admission control

`apps/autopilot-desktop/src/input/reducers/jobs.rs` currently:

- routes NIP-90 kind `5050` text generation to the Ollama worker
- blocks admission unless the local runtime is reachable and a serving model is ready
- enforces model policy and output MIME restrictions
- stores execution output and app-local provenance after completion

Important current scope fact:

- only text generation is executable through this path
- provider execution for `5050` is backend-routed to `Ollama` or `Apple Foundation Models`
- non-`5050` jobs still fall back to Codex

### Provider publication, kernel, receipts, and UI

The current Ollama dependency is projected into:

- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
  - publishes handler metadata with `backend = "ollama"` and the serving model
  - currently advertises only kind `5050`
- `apps/autopilot-desktop/src/state/provider_runtime.rs`
  - runtime readiness and health projection
- `apps/autopilot-desktop/src/kernel_control.rs`
  - launch compute product bindings for `ollama.text_generation` and `ollama.embeddings`
  - metering rule IDs
  - delivery evidence projection
- `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs`
  - receipt tags and evidence refs derived from Ollama provenance
- `apps/autopilot-desktop/src/pane_renderer.rs`
  - provider HUD/UI state for installed models, loaded models, and last generation metrics

### Naming debt already visible

`apps/autopilot-desktop/src/apple_fm_bridge.rs` reuses `OllamaExecutionMetrics` and `OllamaExecutionProvenance`.

That means "Ollama" is already overloaded as "generic local inference metrics/provenance."

Before deleting the external Ollama dependency, the app should first rename these types to backend-neutral names such as:

- `InferenceExecutionMetrics`
- `InferenceExecutionProvenance`

That is a low-risk prerequisite and makes the later migration materially simpler.

## What Upstream Ollama Does That Matters Here

The upstream Ollama codebase is much broader than what OpenAgents uses.

From the reviewed source, the behaviors that matter for replacement are:

### 1. Local model catalog and storage

From `manifest/manifest.go` and `server/model.go`:

- models are stored under a manifest/blob layout
- manifests resolve model names to blobs
- the model blob path ultimately points to GGUF weights

### 2. GGUF metadata and tensor parsing

From `fs/ggml/ggml.go` and `fs/ggml/gguf.go`:

- Ollama reads GGUF magic, version, key-value metadata, tensor names, tensor shapes, kinds, and offsets
- this metadata selects the architecture and informs runtime allocation

### 3. Architecture selection and model construction

From `ml/backend.go`, `ml/backend/ggml/ggml.go`, `model/model.go`, and `model/models/llama/model.go`:

- backend creation is driven by model metadata
- architecture registration maps GGUF metadata to a concrete model implementation
- tokenizer construction also comes from model metadata
- forward passes are model-specific even when the backend is shared

### 4. Runtime lifecycle and scheduling

From `server/sched.go`, `llm/server.go`, and `runner/ollamarunner/runner.go`:

- models have load, warm, unload, keepalive, and reload behavior
- requests are scheduled against loaded model runners
- generation is prompt ingest + decode loop + sampling + metrics
- embeddings are a separate route with different output semantics

### 5. API surface the desktop actually depends on

The current desktop only needs behavior equivalent to:

- `GET /api/tags`
- `GET /api/ps`
- `POST /api/show`
- `POST /api/generate`

It does not currently need:

- remote/cloud model resolution
- `pull`, `push`, `copy`, `create`, or Modelfile authoring
- OpenAI-compatible endpoints
- JS or Python client support
- chat templating, tool calls, or multimodal support

That is the central simplification that makes an in-repo Rust replacement realistic.

## Minimum Rust Replacement Surface OpenAgents Actually Needs

For MVP and for the current desktop code shape, the minimum internal runtime is:

| Subsystem | Needed now | Current source of truth | Current Rustygrad status |
| --- | --- | --- | --- |
| Local model catalog | yes | Ollama manifests/blobs | missing |
| Installed model listing | yes | `ListHandler` / manifests | missing |
| Loaded model listing | yes | scheduler state / `PsHandler` | missing |
| Model validation and metadata | yes | `ShowHandler` / GGUF metadata | missing |
| Warm and unload lifecycle | yes | scheduler + keepalive semantics | missing |
| Prompt text generation | yes | runner decode loop | reference-only, not real model-backed on `main` |
| Sampling knobs | yes | Ollama generate options | missing |
| Generation metrics | yes | generate response durations and token counts | missing |
| Embeddings runtime | not for current NIP-90 path, but product-facing soon | embeddings route | reference smoke only |
| GGUF tokenizer + model-family loading | yes | GGUF/model packages | missing |
| Real accelerated backend | effectively yes for product replacement | GGML/llama.cpp/GPU stack | missing |

## What Rustygrad Already Gives Us

Rustygrad is not empty. It already contains useful foundations for this replacement.

### Useful existing pieces

From current `main`:

- `rustygrad-runtime`
  - reusable runtime traits
- `rustygrad-backend-cpu`
  - a real executable CPU reference backend
- `rustygrad-models`
  - reusable model descriptors, tokenizer boundary, weight metadata types
- `rustygrad-serve`
  - generation and embeddings request/response contracts
  - in-memory generation model registry
  - in-memory KV cache
  - generation session store
- `rustygrad-provider`
  - capability and receipt contracts
  - deterministic request digests

Those are the correct layers to extend.

### Useful architectural fit

Rustygrad’s current layering already matches the replacement shape better than app-owned ad hoc code would:

- file formats and model descriptors belong in reusable crates
- execution runtime belongs in reusable crates
- app/provider admission policy stays in `apps/autopilot-desktop`

That matches `docs/OWNERSHIP.md`.

## What Rustygrad Does Not Yet Have But Must Gain

This is the actual gap list for a truthful Ollama replacement.

### 1. Real model-backed loading on current `main`

Current `main` still lacks merged:

- GGUF-backed real model loading
- artifact-backed weight ingestion
- real quantization handling
- real decoder model families
- real embedding model families

Per the March 7 Rustygrad audit, this work exists on `origin/rustygrad3` but is not merged into the audited `main` checkout.

That makes "merge or recreate phase-2 Rustygrad" the first real step.

### 2. GGUF as a first-class Rustygrad weight format

`rustygrad-models` currently has:

- `WeightFormat::ProgrammaticFixture`
- `WeightFormat::SafeTensors`

It does not yet have:

- `WeightFormat::Gguf`
- tokenizer extraction from GGUF metadata
- tensor-name mapping for Llama/Qwen/Mistral-style GGUF weights

### 3. Real tokenizer support

Rustygrad currently has `FixtureWordTokenizer`.

A real replacement needs:

- SentencePiece support
- GPT-style BPE support
- tokenizer configuration derived from GGUF metadata
- stable encode/decode parity for the served model family

### 4. Real sampling and option mapping

Current Rustygrad `GenerationOptions` only models:

- `max_output_tokens`
- `DecodeStrategy::Greedy`

The current OpenAgents Ollama path already accepts and normalizes:

- `temperature`
- `top_k`
- `top_p`
- `frequency_penalty`
- `presence_penalty`
- `seed`
- `stop`

So Rustygrad must grow a real sampling layer before it can replace the existing worker faithfully.

### 5. Runtime lifecycle and metrics

Current Rustygrad runtime metrics are minimal:

- `ExecutionMetrics { steps_executed }`

The current desktop expects or benefits from:

- total duration
- load duration
- prompt token count
- generated token count
- warm/cold start state
- loaded-model visibility

Those need to exist either in `rustygrad-serve` or in a thin runner layer above it.

### 6. Acceleration

This is the biggest product-risk gap.

Current Rustygrad backends:

- CPU: implemented reference backend
- Metal: placeholder
- AMD KFD: placeholder
- AMD userspace: placeholder

There is no accelerated backend implementation in the current audited tree.

That means a full "replace external Ollama" cut on current `main` would be:

- CPU-only
- slower than the current external runtime
- not truthful as a general replacement for local inference supply

A CPU-only dev cut is possible.
A product-grade replacement is not.

## Recommended Architecture In This Repo

The right architecture is not "move Ollama logic into the desktop app."

The right architecture is:

- keep provider behavior, NIP-90 routing, kernel projection, and UI state in `apps/autopilot-desktop`
- move model catalog, GGUF loading, execution, and lifecycle into Rustygrad

### Recommended crate shape

#### Extend `rustygrad-models`

Add:

- `WeightFormat::Gguf`
- GGUF-derived decoder and embeddings descriptors
- tokenizer family metadata and concrete tokenizer loaders
- quantization metadata
- real loader traits for GGUF-backed model families

#### Add a new Rustygrad artifact/catalog crate

A new crate is justified here. Suggested names:

- `rustygrad-artifacts`
- `rustygrad-catalog`

This crate should own:

- local model discovery
- manifest/blob resolution
- GGUF file location
- installed model enumeration
- model metadata inspection

Important recommendation:

> First read the existing Ollama on-disk manifest/blob layout instead of inventing a new model store.

Why:

- zero user migration for already-installed models
- no need to port `ollama pull` before execution replacement
- much smaller MVP

This crate can be explicitly "Ollama store compatible" without requiring the external Ollama daemon.

#### Extend `rustygrad-serve`

Add a reusable library surface for:

- `list_models`
- `show_model`
- `warm_model`
- `unload_model`
- `loaded_models`
- `generate`
- `embed`

Also add:

- a simple loaded-model registry
- single-model or bounded multi-model lifecycle management
- richer generation metrics
- request option mapping for sampling and stop sequences

#### Extend `rustygrad-provider`

Add or revise provider-facing types for:

- runtime readiness tied to actual loaded models
- richer latency/token metrics
- provenance fields that replace current app-specific `base_url` semantics
- model digest / weight digest / plan digest as explicit evidence

### Recommended app shape

Do not let the app depend directly on raw Rustygrad internals.

Instead:

1. Replace `ollama_execution.rs` with a backend-neutral worker seam.
2. Keep the state/update protocol shape.
3. Swap the implementation from `OllamaHttpAdapter` to `RustygradRuntimeAdapter`.

Suggested app-owned seam:

- `LocalInferenceRuntime` trait
  - `refresh()`
  - `list_installed_models()`
  - `list_loaded_models()`
  - `validate_model()`
  - `warm_model()`
  - `unload_model()`
  - `generate()`
  - later `embed()`

That lets the desktop migrate in small steps without forcing a big-bang rewrite of:

- reducers
- provider runtime state
- NIP-90 publication
- kernel control
- receipt projection
- UI panes

## What Not To Port From Ollama First

The wrong project would be "rebuild all of Ollama in Rust."

OpenAgents does not need that.

Do not start with:

- cloud model support
- OpenAI-compatible endpoints
- Modelfile parsing/building
- registry pull/push
- JS/Python client surfaces
- multimodal support
- chat templating parity
- multi-runner LRU eviction complexity

Current desktop scope is much smaller:

- local-only
- prompt-in, text-out
- one provider request at a time
- one serving model at a time is acceptable for MVP

That smaller target is the correct one.

## Migration Plan

## Phase 0: Unblock Rustygrad

Before touching the desktop swap:

1. Merge or recreate the unmerged Rustygrad model-backed phase-2 work referenced by `docs/audits/2026-03-07-rustygrad-implementation-and-gap-audit.md`.
2. Add GGUF as a real weight format and tokenizer source.
3. Add at least one real GGUF decoder family.

Without this phase, the replacement is not real.

## Phase 1: Neutralize the app seam

In `apps/autopilot-desktop`:

1. Rename `OllamaExecutionMetrics` and `OllamaExecutionProvenance` to backend-neutral names.
2. Rename or wrap `ollama_execution.rs` behind a backend-neutral worker trait.
3. Keep the existing reducers, provider state, receipts, and UI projections stable.

This step reduces migration risk without changing product behavior yet.

## Phase 2: Add an Ollama-compatible model catalog inside Rustygrad

Implement a Rustygrad crate that can:

- inspect `$OLLAMA_MODELS`
- read manifests and blobs
- list installed models
- inspect model metadata
- locate GGUF weights

This replaces `api/tags` and `api/show` dependency first.

## Phase 3: Add model lifecycle and generation runtime

Implement in Rustygrad:

- loaded-model registry
- warm/load/unload
- loaded-model enumeration
- generation request execution
- token and latency metrics

This replaces `api/ps`, `api/generate`, and keepalive behavior.

For MVP, keep the lifecycle simple:

- one active loaded model
- explicit warm/unload
- bounded single-request concurrency

That matches current provider reality better than Ollama’s full scheduler.

## Phase 4: Swap desktop default to Rustygrad

Once the Rust runtime matches the current worker contract:

1. add `RustygradRuntimeAdapter`
2. keep `OllamaHttpAdapter` only as fallback during bring-up
3. switch default provider execution to the Rust runtime
4. delete external Ollama dependency after parity tests pass

## Phase 5: Fix product truth around embeddings

Current repo state already exposes `ollama.embeddings` in kernel/inventory surfaces, but there is no matching executable provider path in `jobs.rs`.

During the migration:

- either remove or disable embeddings product publication until Rustygrad embeddings actually work
- or implement the embeddings service before switching backend truth

Do not carry the current truth gap forward.

## Product And Ownership Notes

This replacement can stay inside repo rules if ownership stays explicit.

### Should stay in `apps/autopilot-desktop`

- provider mode orchestration
- NIP-90 ingress/publication
- job admission policy
- payout/receipt projection
- pane/UI state
- backend preference and product toggles

### Should move into `crates/rustygrad/*`

- local model catalog
- GGUF parsing
- tokenizer loading
- weight loading
- generation/embedding execution
- runtime lifecycle
- reusable metrics/provenance contracts

### Should not move into `crates/wgpui`

- any of the above

## Biggest Risks

### 1. CPU-only replacement would be a regression

If the swap happens before Metal or another accelerated backend lands, the result will work only as a slow reference runtime.

That is acceptable for a dev flag.
It is not acceptable as a truthful default replacement for the current Ollama path.

### 2. Naming will become misleading if not fixed

If OpenAgents stops depending on the external Ollama daemon but continues to say:

- `backend = "ollama"`
- `Ollama base URL`
- `Local Ollama backend`

the UI and receipts become misleading.

Short-term compatibility labels may be tolerable.
Long-term product truth requires renaming the internal runtime family or clearly calling it "Ollama-compatible."

### 3. Big-bang port risk

Trying to copy Ollama’s full scheduler, HTTP API, and model-management surface before getting one real GGUF generation path working will burn time and delay the actual replacement.

The correct order is:

- model-backed Rustygrad first
- minimal local runtime next
- compatibility and polish after that

## Recommended Immediate Next Steps

1. Treat Rustygrad phase-2 recovery as the prerequisite task, not optional follow-on work.
2. Add a backend-neutral execution/provenance seam in the desktop before changing runtime behavior.
3. Build a Rustygrad local model catalog that reads the existing Ollama manifest/blob store.
4. Implement one real GGUF text-generation family and match the current worker’s option surface.
5. Stop advertising or quoting embeddings unless there is a real executable embeddings path behind the selected backend.

## Bottom Line

OpenAgents should not keep a permanent dependency on an external Ollama daemon if the long-term plan is a Rust-native compute engine in this repo.

But the honest path is not "rewrite Ollama in Rust all at once."

The honest path is:

- use Rustygrad as the execution foundation
- add an Ollama-compatible local model catalog and GGUF ingestion layer
- preserve the app-owned worker seam
- replace the external daemon incrementally
- do not call the migration done until model-backed Rustygrad and at least one truthful accelerated backend exist
