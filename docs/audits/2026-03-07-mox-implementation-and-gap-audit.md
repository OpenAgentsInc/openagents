# 2026-03-07 Mox Implementation And Gap Audit

> Historical note: This audit is a point-in-time snapshot from its date. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, and `docs/OWNERSHIP.md`. File paths, issue states, and implementation-status claims here may be superseded by later commits.

Author: Codex  
Status: Complete  
Audit target: current checkout at `d5630eac5`

## Objective

Explain what Mox actually implements in the current repo checkout, correlate that against the Mox docs and GitHub issues, and state clearly what still needs to be built.

## Sources Reviewed

- Product and ownership authority:
  - `docs/MVP.md`
  - `docs/OWNERSHIP.md`
- Mox docs and README:
  - `crates/mox/README.md`
  - `crates/mox/docs/ARCHITECTURE.md`
  - `crates/mox/docs/BACKENDS.md`
  - `crates/mox/docs/INFERENCE_ENGINE.md`
  - `crates/mox/docs/PROVIDER_INTEGRATION.md`
  - `crates/mox/docs/plan.md`
  - `crates/mox/docs/deep-research-report.md`
- Mox source and tests:
  - every `crates/mox/*/src/lib.rs`
  - `crates/mox/mox-serve/tests/smoke_embeddings.rs`
  - `crates/mox/mox-serve/tests/text_generation_reference.rs`
- Relevant GitHub issues reviewed with `gh`:
  - closed: `#3129`, `#3130`, `#3131`, `#3132`, `#3133`, `#3134`, `#3136`, `#3137`, `#3138`, `#3139`, `#3140`, `#3141`, `#3143`, `#3144`, `#3145`, `#3146`, `#3147`, `#3148`, `#3149`
  - open: `#3150`, `#3151`, `#3152`, `#3153`, `#3154`, `#3155`, `#3156`, `#3157`, `#3158`, `#3159`, `#3160`, `#3161`, `#3162`
- Validation:
  - `cargo test -p mox-core -p mox-ir -p mox-compiler -p mox-runtime -p mox-backend-cpu -p mox-models -p mox-serve -p mox-provider`

## Executive Verdict

Mox on the current `main` checkout is a deterministic, CPU-only reference engine with two real end-to-end product paths:

- `mox.embeddings`: a smoke embeddings flow backed by a tiny byte-feature model and the CPU backend
- `mox.text_generation`: a fixture text-generation flow backed by a tiny reference decoder, an in-memory session store, and the CPU backend

What is honestly implemented today:

- the workspace subtree and crate boundaries
- foundational tensor, IR, compiler, runtime, model, serve, and provider crates
- one real CPU execution backend
- deterministic digests, JSON contracts, and end-to-end tests for the smoke/reference paths
- placeholder crate identities for Metal and AMD backends

What is not implemented in this checkout:

- model-backed CPU phase-2 work
- artifact-backed weight loading
- quantization support
- Metal runtime/device discovery
- AMD KFD or AMD userspace discovery
- real accelerated execution
- model-grade transformer inference
- streaming, batching, paged KV, tiered KV, or MoE execution

The most important repo-truth problem is status drift:

- `crates/mox/README.md` and `crates/mox/docs/INFERENCE_ENGINE.md` still describe a phase-0 bootstrap.
- GitHub issues `#3143` through `#3149` are closed as if model-backed CPU phase 2 landed.
- the current `main` checkout and its local Mox history only contain phase-0 and phase-1 work.
- the missing phase-2 work does exist on remote branch `origin/mox3`, but it is not merged into the audited `main` checkout.

So the truthful short description of current Mox is:

> a phase-0-plus-phase-1 CPU reference implementation, not yet the phase-2 model-backed CPU engine described by the later closed issues.

## Issue Tracker Reconciliation

### Issues whose implementation is present in the current checkout

- `#3130` bootstrap subtree and docs
- `#3129` phase 0 CPU-backed embeddings hello world
- `#3131` core, IR, compiler, runtime skeletons
- `#3132` CPU backend and minimal tensor execution
- `#3133` embeddings serve/provider contracts
- `#3134` tested CPU embeddings smoke flow
- `#3136` phase 1 CPU reference text-generation foundation
- `#3137` layout/view ops in core, IR, and CPU backend
- `#3138` decoder/tokenizer/fixture-weight abstractions
- `#3139` generation sessions and in-memory KV cache
- `#3140` provider capability and receipt support for text generation
- `#3141` tested CPU reference text-generation flow

These issues line up with the code and with the Mox path history:

- `b69c689ad` bootstrap subtree
- `cb8a35c4e` engine skeleton
- `8128a3de8` CPU backend
- `1a711af89` embeddings contracts
- `506af7911` embeddings smoke flow
- `dc77e7637` phase-1 CPU text-generation foundation

### Issues that are closed in GitHub but not present in the current checkout

- `#3143` phase 2: first model-backed CPU product paths
- `#3144` artifact-backed weight bundle ingestion
- `#3145` quantization metadata and CPU execution support
- `#3146` first CPU embedding model family
- `#3147` first CPU decoder model family
- `#3148` provider bundle/model truth for model-backed CPU products
- `#3149` end-to-end model-backed CPU verification

Evidence that those phase-2 claims are not in this checkout:

- `git log -- crates/mox` stops at the phase-1 merge.
- `rg` finds no code symbols for the closed phase-2 concepts such as model-backed services, quantization types, safetensors loading, or bundle-evidence types.
- The only `safetensors` mention in the current tree is a future-tense comment in `mox-models`.

Remote evidence now confirms that phase-2 work does exist outside the audited checkout:

- issue comments for `#3144` through `#3149` point to specific `mox3` commits:
  - `3e271f45a` artifact-backed bundle ingestion
  - `b47723898` quantization metadata and CPU quantization posture
  - `8c34cc947` model-backed CPU embeddings
  - `9fa616292` model-backed CPU text generation
  - `c231845ab` provider bundle/model truth
  - `10484e872` phase-2 completion cleanup
- `git fetch origin mox3` shows those commits on `origin/mox3`
- `git diff --stat refs/remotes/origin/main..refs/remotes/origin/mox3 -- crates/mox` shows substantial unmerged Mox changes:
  - 14 files changed
  - 2579 insertions
  - 337 deletions
  - new tests including `model_backed_embeddings.rs` and `model_backed_text_generation.rs`

So the accurate statement is not "phase 2 was falsely closed." It is:

> phase 2 was implemented on `mox3`, but that work is not present in the current `main` checkout being audited.

That still means the repo-level truth problem remains real for anyone reading current `main`.

### Issues that are still entirely future work

- `#3150` through `#3156`: Metal phase
- `#3157` through `#3162`: AMD phase

Those align with the code: both accelerated backend families are still placeholder crates.

## Current Implementation

## Workspace And Dependency Shape

Mox is a real workspace subtree wired into the root workspace:

- `mox-core`
- `mox-ir`
- `mox-compiler`
- `mox-runtime`
- `mox-models`
- `mox-serve`
- `mox-provider`
- `mox-backend-cpu`
- `mox-backend-metal`
- `mox-backend-amd-kfd`
- `mox-backend-amd-userspace`

Actual dependency direction in code:

| Crate | Current role | Current dependencies |
| --- | --- | --- |
| `mox-core` | tensor metadata and lazy-op facade | `serde` |
| `mox-ir` | canonical graph and execution-plan types | `mox-core` |
| `mox-compiler` | insertion-order lowering | `mox-ir` |
| `mox-runtime` | runtime traits and health/errors | `mox-core`, `mox-ir` |
| `mox-backend-cpu` | CPU reference execution | `mox-compiler`, `mox-core`, `mox-ir`, `mox-runtime` |
| `mox-models` | smoke/reference model metadata and fixtures | `mox-core` |
| `mox-serve` | serve contracts plus CPU-backed reference services | `mox-backend-cpu`, `mox-compiler`, `mox-core`, `mox-ir`, `mox-models`, `mox-runtime` |
| `mox-provider` | capability/receipt types and digests | `mox-runtime`, `mox-serve` |
| `mox-backend-metal` | placeholder | none |
| `mox-backend-amd-kfd` | placeholder | none |
| `mox-backend-amd-userspace` | placeholder | none |

Important boundary observation:

- `crates/mox/docs/ARCHITECTURE.md` says `mox-serve` depends on models/runtime/core.
- The actual code couples `mox-serve` directly to `mox-backend-cpu`, `mox-compiler`, and `mox-ir`.
- That is acceptable for a bootstrap reference path, but it means the serve layer is not backend-agnostic yet.

## `mox-core`

Current implementation:

- `TensorId`
- `DType` with only `F32`
- `DeviceKind` with `Cpu`, `Metal`, `AmdKfd`, and `AmdUserspace`
- `Device`
- `Shape`
- `Layout` with contiguous, permute, slice, select, and broadcast-like expand helpers
- `TensorSpec`
- `TensorData` with only `F32(Vec<f32>)`
- `LazyOp`
- `Tensor`

What this means:

- Core models tensor metadata and logical views cleanly.
- Core already reserves explicit device kinds for future accelerated backends.
- Core does not implement quantization, non-`f32` dtypes, buffers, autograd, or executable kernels.

## `mox-ir`

Current implementation:

- graph-time `OpKind`
- `Node`
- `Graph`
- `ExecutionOp`
- `ExecutionStep`
- `ExecutionPlan`
- `GraphBuilder`
- deterministic `stable_digest()` and `stable_debug()` helpers

Supported graph operations:

- `input`
- `constant`
- `add`
- `mul`
- `matmul`
- `reshape`
- `permute`
- `slice`
- `select`
- `concat`
- `expand`
- `reduce_sum`

What this means:

- There is a real canonical graph layer and a real executable-plan layer.
- The IR is deterministic and snapshot-friendly.
- There is no separate schedule IR, fusion IR, alias analysis, memory planning, kernel codegen IR, or backend-specific lowering payload yet.

## `mox-compiler`

Current implementation:

- `LoweringPass` trait
- `PlanBuilder`
- `InsertionOrderLowering`
- `CompilerPipeline`
- `compile_graph()`

What the compiler actually does:

- It walks graph nodes in insertion order.
- It converts `OpKind` directly into `ExecutionOp`.
- It emits a plan that is basically the graph in execution order.

What is missing:

- optimization
- fusion
- scheduling
- memory planning
- backend-specific code generation
- graph capture/replay artifacts beyond digests

## `mox-runtime`

Current implementation:

- `RuntimeError`
- `DeviceDescriptor`
- `HealthStatus`
- `RuntimeHealth`
- `ExecutionMetrics`
- `BufferHandle`
- `DeviceDiscovery`
- `Allocator`
- `ExecutionBackend`
- `ExecutionResult`

What this means:

- Runtime authority is trait-based and reusable.
- The runtime model already has readiness/degraded/offline semantics.
- Device descriptors only carry backend name, device, supported dtypes, and optional memory capacity.
- There is no runtime-level backend selection policy, command queue abstraction, profiling, topology model, quantization capability model, or richer metrics yet.

## `mox-backend-cpu`

Current implementation:

- `CpuBuffer`
- `CpuBackend`
- CPU `DeviceDiscovery`, `Allocator`, and `ExecutionBackend`
- kernels for:
  - `input`
  - `constant`
  - `add`
  - `mul`
  - `matmul`
  - `reshape`
  - `permute`
  - `slice`
  - `select`
  - `expand`
  - `concat`
  - `reduce_sum`

Behavioral characteristics:

- one logical CPU device only
- host-resident `Vec<f32>` buffers only
- no threading, no SIMD, no parallel scheduler
- view ops are represented by cloning backing storage and changing layout metadata
- execution metrics are only `steps_executed`

This is a good reference backend. It is not a production runtime.

## `mox-models`

Current implementation is entirely fixture/smoke-oriented:

- `EmbeddingModelDescriptor`
- `ModelDescriptor`
- `EmbeddingNormalization`
- `TokenId`, `TokenSequence`, `TokenVocabulary`
- `TokenizerBoundary`
- `FixtureWordTokenizer`
- `DecoderAttentionConfig`, `DecoderFeedForwardConfig`, `DecoderBlockConfig`, `DecoderConfig`
- `WeightFormat` with `ProgrammaticFixture` and future `SafeTensors`
- `WeightSource` with `Fixture` and future `ExternalArtifact`
- `WeightTensorMetadata`
- `WeightBundleMetadata`
- `DecoderModelDescriptor`
- `DecoderFixtureWeights`
- `DecoderWeightLoader`
- `FixtureDecoderLoader`
- `ReferenceWordDecoder`
- `SmokeByteEmbedder`

What is actually modeled:

- one toy embeddings model that featurizes bytes into 16 buckets and projects to an 8-dimensional vector
- one toy decoder model with:
  - a 10-token vocabulary
  - one programmatically generated weight bundle
  - one-layer reference config

What is not modeled yet:

- artifact-backed weights
- real tokenizer imports
- real model families
- quantized weights
- embedding-model families beyond the smoke fixture
- transformer execution semantics beyond the reference toy graph

## `mox-serve`

This crate currently mixes three responsibilities:

- public request/response contracts
- in-memory session and KV-cache helpers
- concrete CPU-backed reference services

Current public contracts:

- embeddings:
  - `EmbeddingRequest`
  - `EmbeddingVector`
  - `EmbeddingResponseMetadata`
  - `EmbeddingResponse`
  - `EmbeddingsExecutor`
- text generation:
  - `SessionId`
  - `GenerationInput`
  - `DecodeStrategy` with only `Greedy`
  - `GenerationOptions`
  - `GenerationRequest`
  - `GenerationOutput`
  - `GenerationUsage`
  - `TerminationReason`
  - `GenerationResponse`
  - `TextGenerationExecutor`

Current session/cache implementation:

- `InMemoryKvCache`
- `GenerationSession`
- `GenerationSessionState`
- `InMemoryGenerationSessionStore`
- `InMemoryGenerationModelRegistry`

Current concrete services:

- `SmokeEmbeddingsService`
- `CpuReferenceGenerationModel`
- `CpuReferenceTextGenerationService`

### Actual `mox.embeddings` implementation

The embeddings path is real, but intentionally tiny:

- `SmokeByteEmbedder` converts a string into a deterministic 16-wide feature vector.
- `build_smoke_graph()` constructs a graph:
  - `features -> matmul(projection) -> add(bias)`
- `SmokeEmbeddingsService` executes that graph on the CPU backend once per input string.

What this path proves:

- request contracts exist
- the compiler/runtime/backend split is real
- provider capability/receipt helpers can sit on top of real execution

What it does not prove:

- model import
- batched embeddings execution
- normalization handling
- artifact-backed weight identity
- parity with any external model family

Important truth gap in the current embeddings path:

- `SmokeEmbeddingsService::embed()` validates only `request.model.model.model_id`.
- `EmbeddingResponse::new()` copies dimensions and normalization from the caller-supplied request descriptor, not from the executing model.
- A caller can therefore provide a matching `model_id` but inconsistent dimensions, family, revision, or normalization metadata and get a response whose metadata does not honestly describe the executed model.

That should be fixed before Mox is presented as a truthful provider surface even for the smoke path.

### Actual `mox.text_generation` implementation

The text-generation path is also real, but it is a fixture path rather than a model-grade decoder runtime.

What exists:

- tokenizer-neutral prompt boundary
- sessions with create/reset/close semantics
- in-memory cache state scoped per session
- a deterministic compiled graph used for each token step
- a tested request -> execution -> response -> receipt flow

What the reference graph actually computes:

- one-hot token input times token embedding
- one-hot position input times position embedding
- context input times context projection
- sum those into hidden state
- project hidden state through LM head and bias into logits

How generation actually works:

- prompt text is tokenized with the fixture tokenizer and prepended with BOS
- each prompt token is executed through the reference graph
- the cache stores the hidden vector for each processed token
- future context is the mean of cached values
- decoding is greedy argmax only

This is useful as a deterministic reference product path. It is not yet a real transformer inference engine.

Important limits:

- no attention implementation
- no rotary embeddings
- no grouped-query attention
- no sampler beyond greedy argmax
- no streaming
- no batching
- no cancellation/timeouts
- no model-backed artifact loading
- no acceleration beyond CPU

Important KV-cache truth gap:

- the cache stores identical hidden vectors as both key and value
- generation never reads keys at all
- runtime context is just `mean_cache_value()`
- the service explicitly rejects cache geometries where `kv_width != hidden_size`

So the code has session continuity and replay-safe cache ownership, but not a model-grade KV cache.

## `mox-provider`

Current implementation:

- embeddings provider capability envelope:
  - backend family
  - product id
  - runtime backend
  - model id
  - model family
  - dimensions
  - readiness
- embeddings execution receipt:
  - request digest
  - model id
  - output dimensions/vector count
  - timestamps
  - success/failure
- text-generation capability envelope:
  - backend family
  - runtime backend
  - model id/family/revision
  - max context
  - KV cache mode
  - batch posture
  - readiness
- text-generation receipt:
  - request digest
  - optional execution-plan digest
  - model id
  - optional session id
  - token counts
  - cache token count
  - termination
  - timestamps
  - success/failure
- deterministic digest helpers for embedding and generation requests

What is missing:

- bundle/weight evidence
- quantization truth
- device/topology truth
- resource usage truth beyond token counts and step counts
- accelerated backend truth
- payout linkage or higher-level market objects

That missing surface matches the not-yet-present phase-2 and accelerated-backend work.

## Placeholder Accelerated Backends

Current implementation:

- `mox-backend-metal`: one `CRATE_ROLE` constant
- `mox-backend-amd-kfd`: one `CRATE_ROLE` constant
- `mox-backend-amd-userspace`: one `CRATE_ROLE` constant

These crates currently provide no:

- device discovery
- health reporting
- allocators
- buffers
- kernels
- topology models
- readiness probes

Remote note:

- `origin/mox3` has already moved beyond pure placeholders for Metal, with follow-on commits `48b4a8856` and `d9c67ba5c`.
- those Metal changes are also not present in the audited `main` checkout, so this audit still describes local truth correctly.

## Tests And Validation Coverage

The current Mox subtree has good bootstrap-level coverage:

- `mox-core` tests shape/layout basics
- `mox-ir` tests deterministic graph digests and layout-sensitive graph building
- `mox-compiler` tests deterministic plan digests
- `mox-runtime` tests runtime trait wiring
- `mox-backend-cpu` tests CPU execution across arithmetic, layout views, concat, and reductions
- `mox-models` tests smoke/reference fixture determinism
- `mox-serve` tests contract serialization, session isolation, reset behavior, deterministic generation, and missing-session failure
- `mox-provider` tests JSON stability, round-trips, failure receipts, and deterministic digests
- integration tests verify:
  - smoke embeddings response/capability/receipt flow
  - reference text-generation response/receipt flow

What the current tests do not cover:

- accelerated backend parity
- artifact loading
- quantization behavior
- model-backed CPU products
- truthful rejection of mismatched embeddings descriptors
- large-model memory pressure or long-context behavior

## Documentation Drift

## Docs That Understate The Current Code

- `crates/mox/README.md` still says Mox is in phase-0 bootstrap.
- `crates/mox/docs/INFERENCE_ENGINE.md` still says phase 0 does not implement KV cache support.

Those statements are stale relative to the current code, which does include:

- phase-1 reference text-generation contracts
- in-memory session/KV-cache infrastructure
- a CPU reference generation path

## Docs That Overstate Or Aspirationalize The Current Code

- `crates/mox/docs/plan.md` is a long-range full-engine spec, not a status document.
- `crates/mox/docs/deep-research-report.md` is Tinygrad/Pylon background research, not Mox implementation status.
- GitHub issues `#3143` through `#3149` are currently closed in a way that overstates what `main` actually contains.

## What Still Needs Doing

## 1. Fix Truth And Status Drift First

Before adding more code, Mox needs one honest repo-level story.

Required cleanup:

- update `crates/mox/README.md` to describe the current state as phase 0 plus phase 1 CPU reference work
- update `crates/mox/docs/INFERENCE_ENGINE.md` so it no longer says KV cache is entirely absent
- decide whether the `origin/mox3` phase-2 work should be merged, cherry-picked, or restated as branch-local progress in the issue tracker
- tighten the smoke embeddings descriptor validation so response metadata cannot be caller-forged

## 2. Finish Honest CPU Productization

The next code milestone that still fits the current architecture is the missing phase-2 CPU work:

- artifact-backed weight bundle ingestion
- model-backed embeddings service
- model-backed text-generation service
- quantization metadata and explicit execution posture
- richer provider bundle/model truth
- integration tests proving the served products do not silently fall back to the smoke/reference fixtures

Until that lands, Mox is still serving fixtures rather than model-backed products.

## 3. Refactor `mox-serve` Toward Backend-Agnostic Serving

Right now `mox-serve` is CPU-specific in practice.

To support Metal or AMD honestly, the serve layer still needs:

- execution interfaces that do not hard-code `CpuBackend`
- model/service types that can target an injected runtime/backend
- clearer separation between:
  - public contracts
  - session/cache management
  - reference implementations
  - productized services

If this refactor does not happen, each new backend will either duplicate serve logic or continue to leak backend ownership into the serve crate.

## 4. Replace The Fixture Decoder With A Real Inference Engine

Against `crates/mox/docs/plan.md` and `crates/mox/docs/INFERENCE_ENGINE.md`, the current generation path still lacks the actual inference engine:

- real transformer block execution
- attention and Q/K/V semantics
- rotary embeddings or equivalent positional mechanism
- grouped-query or multi-head attention support
- sampler variants beyond greedy decode
- streaming token delivery
- batching and admission control
- cancellation and timeout handling
- paged or tiered KV-cache modes

The current path proves architecture. It does not yet satisfy the plan’s definition of a full inference engine.

## 5. Build A Real Embeddings Engine

The smoke embeddings path still needs to become an honest product family:

- real embedding model interface
- model-backed weights
- explicit normalization enforcement
- batched execution that is real, not a per-input loop over the CPU backend
- stable model/bundle identity in responses and receipts

## 6. Implement Accelerated Backends

The open issue sequence is still the right high-level order:

- Metal:
  - device discovery
  - readiness truth
  - buffers and command submission
  - kernel/op coverage for the supported product path
  - CPU-vs-Metal parity tests
- AMD:
  - explicit AMD capability/topology/risk model
  - KFD discovery and health
  - AMD userspace probe and opt-in gating
  - provider truth for AMD mode/topology/recovery posture
  - runbooks and readiness validation

Today the accelerated backend crates are placeholders only.

## 7. Enrich Provider Truth

The provider layer needs more than the current phase-0/phase-1 receipts if Mox is going to back market-facing compute:

- bundle and artifact identity
- quantization mode
- device/topology evidence
- runtime health/degraded reasons from accelerated backends
- execution resource usage
- richer capability derivation from actual backend/runtime state

## Recommended Immediate Next Step

The highest-leverage next move is not Metal or AMD yet.

It is:

1. reconcile status truth across README, docs, and issues
2. either merge the missing phase-2 branch work or reopen those issues
3. harden the phase-0/phase-1 truth gaps in the current code
4. only then start phase 3 Metal work from an honest CPU baseline

That sequence preserves the design principle Mox keeps repeating in its own docs: capability surfaces must be explicit and truthful.

## Bottom Line

Mox currently succeeds at the thing its earliest issues set out to prove:

- a real workspace subtree exists
- the tensor/IR/compiler/runtime split is real
- the CPU backend can execute real graphs
- served product contracts exist
- provider receipts and capabilities exist
- deterministic tests pass

Mox does not yet succeed at the thing its later issues and full-plan docs describe:

- model-backed CPU products
- quantized execution
- accelerated backends
- a production-grade inference engine
- a production-grade embeddings engine

If the repo wants to stay truthful, it should describe Mox today as:

> a CPU reference engine with smoke/reference served products and placeholder accelerated backends, plus a larger planned roadmap that is only partially implemented on `main`.
