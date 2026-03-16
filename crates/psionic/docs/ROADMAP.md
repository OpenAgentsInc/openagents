# Psionic Full Library Roadmap

> Status: rewritten 2026-03-15 after reviewing `docs/MVP.md`,
> `docs/OWNERSHIP.md`, `crates/psionic/README.md`,
> `crates/psionic/docs/ARCHITECTURE.md`,
> `crates/psionic/docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md`,
> `crates/psionic/docs/TRAIN_SYSTEM.md`,
> `crates/psionic/docs/ROADMAP_CLUSTER.md`,
> `crates/psionic/docs/ROADMAP_FM.md`,
> `crates/psionic/docs/ROADMAP_METAL.md`, and the recent audits:
> `docs/audits/2026-03-14-tinygrad-parity-target-for-psionic-audit.md`,
> `docs/audits/2026-03-15-tinygrad-test-suite-port-audit.md`,
> `docs/audits/2026-03-15-pytorch-test-suite-port-audit.md`,
> `docs/audits/2026-03-15-full-pytorch-port-to-rust-in-psionic-audit.md`,
> `docs/audits/2026-03-15-decentralized-training-target-sequencing-audit.md`,
> and
> `docs/audits/2026-03-15-psionic-tinygrad-philosophy-pytorch-drop-in-audit.md`.
>
> This is now the canonical full-library roadmap for `crates/psionic/*`.
> `ROADMAP_CLUSTER.md`, `ROADMAP_FM.md`, and `ROADMAP_METAL.md` remain useful
> lane-specific references, but they are no longer the canonical answer to
> "what is the Psionic program overall?"

## Executive Summary

- Psionic is now a full-library program, not a single runtime-swap project.
- The architecture has three layers: compact core, broad semantics, and
  Psionic-native execution truth.
- The next critical missing layer is semantics and compatibility, not another
  host-specific benchmark queue.
- CPU semantic truth and replay truth are foundational and should gate later
  backend claims.
- Backend lanes widen only through explicit reusable contracts, not through
  lane-local shortcuts.
- Training should broaden only after module, state, and checkpoint semantics
  are honest.
- PyTorch-facing shells or interop layers come after the Rust-native substrate
  is real, not before.
- An implemented-early executor-class reference lane codenamed `Tassadar` now
  exists as WebAssembly-first, CPU-reference-first, library-owned work rather
  than MVP product scope, with Phase 3 benchmark/environment packages, Phase 4
  proof/lineage surfaces, the first Phase 5 hull-cache fast path, and the
  first Phase 6 runtime capability/selection truth now landed above it.

## Why This Doc Exists

The old roadmap set drifted toward:

- host-specific throughput queues
- lane-specific bring-up checklists
- historical issue chains optimized around local runtime replacement

That work was real, but it is no longer an honest map of Psionic as a whole.

Psionic is now clearly a larger program:

- a Rust-native framework core
- a reusable serving and model-runtime library
- a backend and runtime bring-up program
- a clustered and sandboxed execution substrate
- an eval and training substrate
- a later interoperability path toward "usable for most PyTorch workloads"

So this roadmap now answers the broader question:

> what is the dependency-ordered roadmap for Psionic as a full reusable
> library, not just as one local-runtime replacement track?

## Relationship To Product Scope

This roadmap does not widen active product MVP scope in [docs/MVP.md](/Users/christopherdavid/code/openagents/docs/MVP.md).

It is a library and engine roadmap.

Per [docs/OWNERSHIP.md](/Users/christopherdavid/code/openagents/docs/OWNERSHIP.md):

- `crates/psionic/*` owns reusable execution substrate
- `apps/*` own product behavior and UX
- kernel and Nexus own authority truth, not runtime execution engines

Nothing in this roadmap should be read as permission to move app logic,
wallet/payout logic, or authority logic into Psionic crates.

That includes the `Tassadar` executor-class reference lane. It is a
library/runtime program inside `crates/psionic/*`, not a product-scope change
to the current desktop/provider MVP.

## Objective

Build Psionic into a full Rust-native compute and ML library with:

- a small, visible, backend-portable framework core
- PyTorch-credible semantics for most practical AI workloads
- framework-level distributed training semantics, not only clustered execution
- first-class precision, quantization, reproducibility, and profiling systems
- explicit extensibility for operators, kernels, autograd, and backends
- advanced tensor, dtype, math, and data-pipeline capability families
- truthful model IO, serving, routing, and runtime behavior
- explicit backend, topology, replay, and proof surfaces
- reusable cluster, sandbox, eval, and training substrate
- a path to decentralized adapter training and broader train-class execution

This roadmap is not:

- a plan to make Psionic only an Ollama replacement
- a plan to make Psionic only a provider runtime
- a plan to build "Rust tinygrad"
- a plan to blindly port all of PyTorch

It is a plan to build:

> a tinygrad-disciplined framework core, a PyTorch-credible semantics layer,
> and Psionic-native system truth around both.

## Explicit Non-Goals

These are not goals of this roadmap:

- full upstream PyTorch closure
- eager support for every historical PyTorch serialization artifact
- every historical distributed mode or backend mode PyTorch has ever exposed
- silent backend fallbacks that hide capability gaps
- benchmark wins achieved through lane-specific behavior that bypasses reusable
  library contracts
- Python-first API polish before Rust-native semantics are credible
- distributed training breadth before single-host training semantics are honest
- every legacy artifact or deployment path from the PyTorch ecosystem
- every domain library in the broader PyTorch ecosystem
- product-specific routing, wallet, payout, or UX logic inside `crates/psionic/*`
- host-specific throughput wins treated as equivalent to full-library progress
- ambiguous claims that imply upstream PyTorch closure when the implementation
  is only `PyTorch-credible` or `PyTorch-compatible` in a bounded sense

This roadmap does target the major framework capability families required to
make "replacement for most AI workloads" an honest claim.

## Architectural Direction

The governing architectural rule is:

- tinygrad-like at the bottom
- PyTorch-like in the middle
- Psionic-like at the top

That means:

- the framework core should stay small, inspectable, and backend-portable
- compatibility breadth should grow above that core, not by bloating it
- receipts, manifests, topology truth, replay truth, sandbox policy, provider
  capability, and train/eval evidence remain explicitly Psionic-native

This roadmap therefore uses three structural layers.

### Layer 1: framework core

Owns:

- tensor metadata and identity
- graph IR
- autodiff substrate
- lowering, scheduling, and plan identity
- runtime and backend contracts
- local multi-device execution substrate

Current crates:

- `psionic-core`
- `psionic-ir`
- `psionic-compiler`
- `psionic-runtime`

### Layer 2: semantics and compatibility

Owns:

- operator breadth
- advanced tensor, dtype, storage, and math-family semantics
- fake or meta execution
- precision, quantization, RNG, and determinism policy
- module and state-tree semantics
- optimizer and scheduler breadth
- data-pipeline, transform, and distributed-training semantics
- exportable graph and deployment-facing library artifacts
- extensibility contracts for custom ops, kernels, autograd, and backends
- serialization and checkpoint compatibility
- parity harnesses against PyTorch

This layer is only partially present today.

### Layer 3: Psionic-native system truth

Owns:

- model catalog and serving runtime truth
- router and placement truth
- artifact manifests and replay receipts
- cluster, sandbox, and networked execution truth
- eval, train, validator, and accepted-outcome seams
- provider capability and runtime evidence

Current crates already span this layer broadly.

## Crate Boundary Rules

The layer split above needs implementation guardrails.

- `psionic-core` owns minimal tensor, value, dtype, device, and layout
  identity only.
- `psionic-ir` owns graph and autodiff representation, not PyTorch-shaped user
  semantics.
- `psionic-compiler` owns lowering, scheduling, planning, replay identity, and
  cache identity.
- `psionic-runtime` owns runtime and execution contracts, not model-family
  quirks or product logic.
- semantics and compatibility code must live above the core crates rather than
  backflowing into them.
- model-family and serving logic must not backflow into compiler or runtime
  primitives except through explicit reusable contracts.
- cluster, sandbox, provider, and training truth must remain separate from
  app-owned product orchestration and UX.

## Distributed Scope Split

Distributed work in Psionic has three separate layers.

- execution substrate: cluster membership, process groups, collectives,
  topology, remote execution, and elastic membership
- framework semantics: data parallel, sharding, pipeline schedules, and
  distributed checkpoint semantics
- product or runtime policy: placement strategy, reliability policy, provider
  exposure, and settlement-facing outcomes

Those layers map to different owners and different epics. They must not be
collapsed into one vague idea of "distributed support."

## Reference Truth Rule

- CPU is the canonical reference backend for semantic correctness.
- replay fixtures and parity harnesses must pass on CPU before accelerated
  backend claims are accepted as equivalent behavior.
- accelerated backends may narrow capability through explicit refusal, but they
  may not silently widen semantics or silently fall back to incomparable
  alternate behavior.
- unsupported semantics on an accelerated backend must fail as a typed refusal
  against the capability matrix, not silently reroute through a semantically
  different execution path.
- backend-specific performance wins do not count as framework progress if the
  CPU reference and replay truth are still disputed.

## Success Bar

Psionic should be judged against five progressively stronger claims.

### Claim 1: `framework-core`

This means the framework-core acceptance matrix is honestly green at meaningful
breadth, not only at representative happy-paths.

### Claim 2: `library-usable`

This means common inference and training workloads can run with:

- broad tensor and autodiff semantics
- real module/state behavior
- real optimizer behavior
- honest precision, quantization, data, and reproducibility behavior
- real model IO and checkpoint restore

### Claim 3: `execution-truthful`

This means Psionic can honestly expose:

- backend capability
- topology truth
- manifests and receipts
- replay identity
- router and cache behavior
- cluster and sandbox behavior

### Claim 4: `PyTorch-credible`

This means common PyTorch-shaped workloads see credible breadth in:

- operator semantics
- autodiff behavior
- modules and state
- optimizers
- checkpoints and conversion paths
- distributed training semantics
- precision-policy and quantization systems
- reproducibility and determinism behavior
- profiling, observability, and refusal diagnostics
- extensibility and registration contracts
- advanced tensor and math families
- data-pipeline capability
- fake or symbolic compiler hygiene

This still does **not** mean full upstream PyTorch closure.

All PyTorch-derived parity matrices and credibility claims must target an
explicit bounded upstream version window, not an unversioned idea of PyTorch.

### Claim 5: `PyTorch-compatible`

This means bounded, explicit, separately tested interop or shell surfaces exist
for existing PyTorch-shaped workflows.

This claim is narrower than full upstream compatibility and must stay explicitly
scoped.

## Claim Acceptance Scoreboard

| Claim | Required green surfaces | Disqualifiers |
| --- | --- | --- |
| `framework-core` | tensor and storage semantics, autodiff breadth floor, compiler replay, deterministic CPU reference, fake/meta path, RNG and determinism substrate, local multi-device correctness | backend-specific shortcuts, missing replay identity, happy-path-only coverage |
| `library-usable` | modules, parameters and buffers, state load/save, optimizer and scheduler behavior, common model IO, precision policy, data ingress, and basic train and infer smoke suites | checkpoint loads only for curated demos, no strict/non-strict load semantics |
| `execution-truthful` | capability reporting, refusal taxonomy, receipts and manifests, topology truth, cache truth, provider truth | hidden fallbacks, unverifiable runtime claims, lane-local evidence only |
| `PyTorch-credible` | parity matrices, dense and sharded checkpoint migration, practical operator, module, transform, distributed, quantization, and profiler breadth, fake/symbolic/compiler hygiene | PyTorch-looking shell without tested semantics |
| `PyTorch-compatible` | explicitly bounded and separately tested interop shells | ambiguous marketing or roadmap language implying upstream closure |

The phrase "drop-in replacement for most AI workloads" only becomes honest when
Claims 1 through 4 are substantively green and Claim 5 is bounded rather than
hand-waved.

For clarity, "drop-in replacement for most AI workloads" still means:

- most common inference and training workloads are semantically credible
- common checkpoints and model code are portable or convertible
- distributed, precision, quantization, reproducibility, and data-path behavior
  are explicit and tested
- profiler, export, and diagnostic surfaces are good enough to replace opaque
  lane-local reasoning
- the framework is broad enough that "drop-in for most workloads" is honest

## Acceptance Artifact Rule

No roadmap item is complete unless it lands with at least one acceptance
artifact:

- a capability matrix
- a replay or refusal fixture set
- a parity harness block
- a benchmark or profiler evidence suite
- a migration or artifact compatibility matrix

Code without one of those artifacts is not roadmap completion.

## Capability Matrix Tiering

Capability matrices should classify coverage explicitly:

- Tier A: required for "most workloads"
- Tier B: important, but not blocking the replacement claim
- Tier C: advanced, experimental, or long-tail

Tiering is required to avoid matrix explosion and scattered partial closure.

## Minimum Bar For "Replacement For Most AI Workloads"

Do not use that phrase unless all of the following are true:

- Claim 1 through Claim 4 are green at meaningful breadth
- dense checkpoint migration is real
- at least one distributed training lane is honest end to end
- mixed-precision and quantization behavior are tested
- profiler and refusal diagnostics are usable without lane-local debugging
- supported, convertible, and unsupported matrices are published

## Current Baseline

Psionic already has real foundations.

### Framework-core foundations already present

Per [crates/psionic/docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md),
Psionic already has implemented-early substrate for:

- tensor metadata and layout semantics
- reverse-mode autodiff foundations
- reusable optimizer families
- model and state IO foundations
- deterministic compiler replay
- memory and cache truth
- same-type local multi-device substrate

That is real, but still not broad framework completion.

### Serving and model-runtime foundations already present

Per [crates/psionic/README.md](/Users/christopherdavid/code/openagents/crates/psionic/README.md),
the repo already has:

- reusable model descriptors and loaders
- GGUF and safetensors paths
- generic server surfaces for chat, responses, and embeddings
- structured output, tool calling, reasoning, and router seams
- adapter packaging and hosted adapter execution
- Apple FM integration and Apple adapter operator flow

### System-truth foundations already present

Psionic already owns meaningful system-truth substrate for:

- artifact staging
- cluster topology and scheduling truth
- bounded sandbox profiles
- provider capability derivation
- eval runtime and benchmark packages
- early train orchestration and accepted-outcome seams

### Biggest current gaps

The largest open gaps are now more architectural than existential:

- framework-core breadth is still much thinner than PyTorch or Tinygrad
- operator and module parity harnesses are still too sparse
- module/state-tree semantics are still underdeveloped
- distributed-training semantics are still mostly below the framework line
- serialization and checkpoint compatibility are still narrower than a
  practical PyTorch replacement needs
- precision-policy, quantization, and determinism systems are not yet first-class
- RNG, storage, sparse, nested, masked, and transform semantics are underspecified
- GGUF quant decode remains incomplete, especially K-family formats
- data-ingress, profiler, and export/deployment artifact stories are still thin
- custom-op, backend-extension, and quantizer plugin contracts are not explicit
- advanced math families like linalg, fft, special, distributions, and complex
  semantics are not yet roadmap-visible enough
- backend execution breadth is uneven beyond the current truthful lanes
- cluster and sandbox truth exist, but the full library-level execution story
  still needs convergence
- train-class execution is real but still narrow, adapter-first, and not yet
  equivalent to framework-level distributed training semantics
- structured interactive environment turns, trajectory receipts, and benchmark
  bridges are still too text-session-oriented for ARC-AGI-3-class and similar
  interactive eval families
- train-class operator, collective, and model-state coverage is still too
  narrow for HRM-class and similar non-adapter model ports

## Roadmap Shape

This roadmap is organized into seven epics.

| Epic | Theme | Outcome |
| --- | --- | --- |
| Epic 0 | Governance and acceptance | one canonical library roadmap and claim vocabulary |
| Epic 1 | Framework core completion | serious tinygrad-disciplined tensor/compiler/runtime substrate |
| Epic 2 | Semantics and compatibility | PyTorch-credible ops, modules, transforms, precision, quantization, data, and state behavior |
| Epic 3 | Model IO and runtime families | truthful serving, model-family loading, cache, router, and runtime behavior |
| Epic 4 | Backend truth and performance | explicit, benchmarked, honest backend lanes |
| Epic 5 | Cluster, sandbox, and execution truth | reusable distributed execution and bounded compute substrate |
| Epic 6 | Training, eval, and research | decentralized adapter training first, then broader train-class closure |
| Epic 7 | Interop and adoption | practical path from "good Rust engine" to "usable for most PyTorch workloads" |

This roadmap now has four live GitHub issue blocks:

- decentralized adapter training:
  [#3649](https://github.com/OpenAgentsInc/openagents/issues/3649) and
  [#3636](https://github.com/OpenAgentsInc/openagents/issues/3636) through
  [#3648](https://github.com/OpenAgentsInc/openagents/issues/3648)
- framework core completion:
  [#3741](https://github.com/OpenAgentsInc/openagents/issues/3741) with child
  issues [#3703](https://github.com/OpenAgentsInc/openagents/issues/3703)
  through [#3715](https://github.com/OpenAgentsInc/openagents/issues/3715)
- semantics and compatibility:
  [#3742](https://github.com/OpenAgentsInc/openagents/issues/3742) with child
  issues [#3716](https://github.com/OpenAgentsInc/openagents/issues/3716)
  through [#3736](https://github.com/OpenAgentsInc/openagents/issues/3736)
- executor-class in-model compute lane:
  [#3743](https://github.com/OpenAgentsInc/openagents/issues/3743) and
  [#3744](https://github.com/OpenAgentsInc/openagents/issues/3744)

Epic 0 and later epics beyond 2 still use roadmap-local IDs until activated.

### Tassadar Executor-Class Reference Lane

This track is now implemented early as a Psionic-owned reference lane and
remains dependency-ordered beyond Phase 4.

It is a cross-epic lane that depends on:

- Epic 1 framework-core extensibility and cache identity
- Epic 2 semantics, export, and attention-family extension contracts
- Epic 3 model/runtime-family truth

Its declared scope is:

- owner: `crates/psionic/*`
- first target: WebAssembly-first executor semantics
- landed Phase 1 bar: CPU reference fixture and exact parity harness
- landed Phase 2 bar: digest-bound program artifacts and explicit
  model/program compatibility contracts
- landed Phase 3 bar: typed environment bundle plus package-driven exactness
  benchmark suite with CPU and reference-linear baselines
- landed Phase 4 bar: emitted trace artifacts, runtime-manifest lineage, and
  proof-bundle integration for replay-stable executor evidence
- landed Phase 5 bar: explicit `HullCache` fast-path decode identity, exact
  CPU/reference-linear/hull-cache equivalence checks on the validated acyclic
  subset, typed refusal for backward-branch workloads outside that subset, and
  benchmark reporting for hull-cache throughput, speedup over linear decode,
  and remaining gap vs direct CPU
- landed Phase 6 bar: machine-legible runtime capability reports plus
  direct/fallback/refused decode selection diagnostics covering hull-cache,
  approximate sparse-top-k fallback, unsupported ABI/profile requests, and
  model-effective decode mismatches
- landed Phase 7A bar: explicit served `psionic.executor_trace` product
  semantics in `psionic-serve`, including typed request/response contracts,
  pull-driven trace streaming, final output extraction helpers, typed refusal
  responses, and served evidence bundles that preserve decode selection, trace
  proof, and runtime-manifest lineage
- landed Phase 7B bar: widened `core_i32_v2` Wasm profile, profile-aware
  runner construction, and article-class benchmark coverage for
  `MicroWasmKernel`, `SudokuClass`, and `HungarianMatching` with exact
  CPU/reference-linear/hull-cache parity plus published speedup and CPU-gap
  metrics
- landed Phase 8A bar: typed `psionic-research` executor-variant family with
  benchmark/proof/lineage-backed bounded runs and machine-readable sweep
  records for reproducible same-contract candidate comparison
- landed Phase 8B bar: validated `SparseTopK` decode mode with explicit direct
  selection on the current subset, exact fallback on unsupported shapes, and
  benchmark reporting against CPU, reference-linear, and hull-cache baselines
- landed Phase 9A bar: planner-owned hybrid routing through
  `psionic.planner_executor_route`, with executor preflight, replay-stable
  routing decisions, typed completed/fallback/refused outcomes, and explicit
  planner-visible policy, budget, proof, selection, and refusal truth
- landed Phase 9B bar: bounded small-model executor training in
  `psionic-train`, with package-backed Tassadar supervision, fixed-budget
  training receipts, proof-aware exactness comparison against the handcrafted
  reference lane, and explicit validation-corpus-only scope claims
- landed crate surfaces:
  - `psionic-runtime::tassadar`
  - `psionic-models::TassadarExecutorFixture`
  - `psionic-environments::TassadarEnvironmentBundle`
  - `psionic-eval::run_tassadar_reference_fixture_benchmark`
  - `psionic-eval::run_tassadar_article_class_benchmark`
  - `psionic-runtime::build_tassadar_execution_evidence_bundle`
  - `psionic-serve::LocalTassadarExecutorService`
  - `psionic-serve::LocalTassadarPlannerRouter`
  - `psionic-train::train_tassadar_small_executor`
  - `psionic-research::ExperimentFamily::ExecutorVariants`
  - `psionic-runtime::TassadarSparseTopKRunner`
- strategic value: inner exact-computation substrate for larger reasoning
  systems
- non-goals: current MVP product scope, kernel authority, or replacement of
  native CPU execution

The current issue spine is:

- Phase 0 scope, ownership, and issue-spine declaration:
  [#3743](https://github.com/OpenAgentsInc/openagents/issues/3743)
- Phase 1 CPU reference WebAssembly executor fixture and exact parity harness:
  [#3744](https://github.com/OpenAgentsInc/openagents/issues/3744)
- Phase 2 executor model and program artifact contracts:
  [#3745](https://github.com/OpenAgentsInc/openagents/issues/3745)
- Phase 3 benchmark and environment packages:
  [#3746](https://github.com/OpenAgentsInc/openagents/issues/3746)
- Phase 4 executor trace proof bundles and manifest lineage:
  [#3747](https://github.com/OpenAgentsInc/openagents/issues/3747)
- Phase 5 hull-cache fast path behind exact equivalence:
  [#3748](https://github.com/OpenAgentsInc/openagents/issues/3748)
- Phase 6 typed runtime capability and fallback diagnostics:
  [#3749](https://github.com/OpenAgentsInc/openagents/issues/3749)
- Phase 7A dedicated executor serving surface and trace product:
  [#3760](https://github.com/OpenAgentsInc/openagents/issues/3760)
- Phase 7B widened WebAssembly profile and article-class benchmark coverage:
  [#3761](https://github.com/OpenAgentsInc/openagents/issues/3761)
- Phase 8A executor architecture, ABI, and cache research families:
  [#3762](https://github.com/OpenAgentsInc/openagents/issues/3762)
- Phase 8B validated sparse-top-k executor decode path:
  [#3763](https://github.com/OpenAgentsInc/openagents/issues/3763)
- Phase 9A hybrid planner-plus-executor routing:
  [#3764](https://github.com/OpenAgentsInc/openagents/issues/3764)
- Phase 9B honest small executor training in `psionic-train`:
  [#3765](https://github.com/OpenAgentsInc/openagents/issues/3765)

What Phases 1 through 9B now concretely provide:

- one machine-legible WebAssembly-first profile
- one append-only trace ABI
- one direct CPU reference runner
- one handcrafted fixture runner
- exact parity and deterministic replay helpers
- typed refusal surfaces for unsupported programs
- one `WeightFormat::ProgrammaticFixture` executor model descriptor and weight
  bundle
- one digest-bound `TassadarProgramArtifact` contract
- explicit executor-family compatibility, decode-mode, and exactness claims for
  model/program pairing
- one typed `TassadarEnvironmentBundle` for eval and benchmark package binding
- one package-driven benchmark suite over the current validation corpus
- exact output/step/halt scoring with explicit CPU and reference-linear
  throughput metrics
- explicit `HullCache` decode-mode identity and exact three-way equivalence
  harnesses over direct CPU, reference-linear, and hull-cache execution
- typed backward-branch refusal so workloads outside the first validated
  fast-path subset fail explicitly rather than silently falling back
- hull-cache throughput, linear-decode speedup, and direct-CPU gap reporting in
  the same benchmark package family
- one machine-legible `TassadarRuntimeCapabilityReport`
- one machine-legible `TassadarExecutorSelectionDiagnostic` that can say
  direct, fallback, or refused before execution begins
- explicit approximate sparse-top-k to exact reference fallback reporting
- explicit unsupported ABI/profile and model-effective decode refusal reporting
- runtime capability and selection artifacts in the eval benchmark surface
- one emitted `TassadarTraceArtifact` plus `TassadarTraceProofArtifact`
- runtime-manifest lineage from source program through model descriptor and
  emitted trace artifact
- canonical `ExecutionProofBundle` integration for replay-stable executor
  evidence
- one explicit `psionic.executor_trace` served product family in
  `psionic-serve`
- one typed `TassadarExecutorRequest` / `TassadarExecutorOutcome` contract
- one pull-driven trace stream that can emit capability, selection, trace-step,
  output, and terminal events without pretending to be chat completion
- one local reference service that preserves exact refusal, fallback, proof,
  and lineage truth through the serving boundary
- one planner-owned `psionic.planner_executor_route` routing contract distinct
  from ordinary chat completion semantics
- one executor preflight surface so planner policy and budget gates can act on
  model compatibility and decode-selection truth before delegation
- one replay-stable `TassadarPlannerRoutingDecision` carrying planner request
  digest, routing digest, policy, budget, capability, selection, and
  contract-error truth
- one typed `TassadarPlannerRoutingOutcome` that can complete, return a typed
  planner fallback, or refuse while preserving executor proof and refusal
  surfaces across the routing boundary
- one bounded `psionic-train` small-executor lane over the Tassadar validation
  benchmark package rather than only local synthetic fixtures
- one fixed-budget training receipt family for learned Tassadar arithmetic
  kernels using the existing Psionic training core
- one proof-aware exactness comparison surface that checks trained traces,
  outputs, and halt posture against the handcrafted reference lane and
  preserves the reference proof-bundle digests
- one explicit validation-corpus-only claim scope for trained Tassadar small
  models so the learned lane does not erase the handcrafted/proved baseline
- one widened `core_i32_v2` Wasm profile with explicit limits for article-class
  workloads
- profile-aware CPU reference, reference-linear, and hull-cache runner
  construction that fails explicitly on unsupported profile ids
- one widened article-class corpus and benchmark package family covering
  `MicroWasmKernel`, `SudokuClass`, and `HungarianMatching`
- exact parity plus throughput, speedup, and remaining gap-vs-CPU reporting on
  that widened corpus
- one typed executor-variant research family in `psionic-research` covering
  architecture, trace ABI, Wasm profile, decode-cache, and attention-mode
  surfaces
- one bounded research runner path that consumes the real Tassadar benchmark
  backend instead of synthetic executor placeholders
- benchmark, proof-bundle, runtime-manifest, and benchmark-report artifacts as
  first-class experiment inputs and outputs
- one machine-readable sweep record for reproducible same-contract executor
  candidate comparison
- one validated `SparseTopK` executor decode mode with a real runtime path
  rather than only fallback diagnostics
- explicit sparse-top-k validation gates plus truthful fallback on unsupported
  control-flow or program-size shapes
- sparse-top-k throughput, speedup-over-reference-linear, and remaining
  gap-vs-CPU reporting inside the same benchmark package family
- one explicit program-specialized compiled-weight deployment path in
  `psionic-models`, with digest-bound compiled executor artifacts, exact
  program-artifact binding, runtime-contract truth, and compile-time
  proof/runtime-manifest lineage
- one compiled-weight suite artifact surface for research runs so
  `program_compiled` candidates emit first-class compiled deployment outputs
  rather than being forced back into handcrafted-only declarations
- explicit larger 2D-head architecture metadata in `psionic-research`,
  including head-count, implied `d_model`, and deterministic parameter-count
  estimates for comparable executor-family sweeps
- compiled-weight bundle-byte and compiled-program-count metrics alongside the
  existing exactness, speedup, and CPU-gap reporting
- one non-handwavy program-to-weight exploration path that stays honest about
  being program-specialized and compile-time verified rather than a generic
  learned compile-to-weights runtime
- one typed learned-plus-compiled and learned-circuit research family in
  `psionic-research`, with explicit research-line, instruction-set,
  execution-proxy, claim-boundary, and proof-expectation surfaces
- one benchmarkable circuit-research runner path that always compares back to
  the handcrafted Wasm baseline and the bounded small-executor training lane
  on the validation corpus
- explicit claim-boundary enforcement so learned-circuit research results stay
  `research_only` unless they are literally using the bounded
  `validation_corpus_only` trained-small comparator path
- first-class trained-small receipt artifacts plus compiled-weight suite
  artifacts inside the same research result contract
- a truthful bridge to SUBLEQ and minimal-instruction exploration that keeps
  those lines in research space without claiming that a learned-circuit runtime
  product is already shipped

Later phases remain dependency-ordered by the March 15 audit and now continue
through the current closed Phase 9D issue spine, with no remaining open
Tassadar executor-phase issues from that March 15 audit:

- hybrid learned-plus-compiled and learned-circuit executor research:
  [#3767](https://github.com/OpenAgentsInc/openagents/issues/3767)

## Epic 0: Governance And Acceptance

### Goal

Make the full-library claim vocabulary explicit and keep future lane work from
drifting back into siloed or host-specific roadmaps.

### Exit Criteria

- one canonical full-library roadmap exists
- claim families are explicit and non-overlapping
- lane-specific roadmaps are treated as supporting references, not the primary
  queue
- execution order is anchored on library architecture, not on one host or one
  benchmark chain

### Issues

| ID | Status | Work |
| --- | --- | --- |
| `PLIB-001` | landed | Rewrite the full-library roadmap from scratch and supersede the old host-specific framing. This document closes that issue. |
| `PLIB-002` | planned | Freeze the canonical claim vocabulary: `framework-core`, `library-usable`, `execution-truthful`, `PyTorch-credible`, and `PyTorch-compatible`. |
| `PLIB-003` | planned | Refresh `ROADMAP_CLUSTER.md`, `ROADMAP_FM.md`, and `ROADMAP_METAL.md` against this roadmap so they become lane deep dives rather than competing primaries. |
| `PLIB-004` | planned | Add one compact roadmap-to-acceptance index across architecture, framework-core, inference, train, and future compatibility docs. |
| `PLIB-005` | planned | Freeze the upstream PyTorch version window that parity matrices, harnesses, and compatibility claims target. |

## Epic 1: Framework Core Completion

### Goal

Finish the tinygrad-disciplined framework core without letting it turn into an
opaque monolith.

### Exit Criteria

- tensor, graph, autodiff, compile, memory, replay, and local multi-device
  categories are green at real breadth
- storage, RNG, determinism, and extension-registration substrate are explicit
- the primitive core stays small and inspectable
- backend bring-up can target explicit narrow contracts instead of a hidden
  giant stack

### Shipped Foundations

- `psionic-core` tensor metadata and quantization substrate
- `psionic-ir` graph and autodiff foundation
- `psionic-compiler` deterministic lowering and replay fixtures
- `psionic-runtime` runtime descriptors, proof, cache, and local multi-device
  substrate

Master issue:
[#3741](https://github.com/OpenAgentsInc/openagents/issues/3741)

### Issues

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `PLIB-101` / [#3703](https://github.com/OpenAgentsInc/openagents/issues/3703) | done (2026-03-16) | `psionic-core` now owns explicit broadcast, alias-preserving view, and dtype-promotion rules; `psionic-ir` lowers broadcasted binary ops through inserted `expand` views; and CPU/reference tests prove the semantics over indexed views plus axis reduction. |
| `PLIB-102` / [#3704](https://github.com/OpenAgentsInc/openagents/issues/3704) | done (2026-03-16) | `psionic-ir` now publishes a built-in operator registry with stable schemas, implementation families, and meta-execution posture; graph construction routes through that contract; and execution plans can be revalidated against the registry before later fake/meta or compatibility work lands. |
| `PLIB-103` / [#3705](https://github.com/OpenAgentsInc/openagents/issues/3705) | done (2026-03-16) | `psionic-ir` now exposes fake or meta execution over graphs and plans, explicit capability-gated backend-kernel coverage checks, and shape-only reports for outputs and step traces; `psionic-compiler` now proves compiled plans can be validated without material tensor data. |
| `PLIB-104` / [#3706](https://github.com/OpenAgentsInc/openagents/issues/3706) | done (2026-03-16) | `psionic-ir::autodiff` now makes the gradient support matrix explicit, regression-tests the full current primitive-family surface across reshape/view/concat/axis-reduction paths, and refuses every current backend-extension family through stable typed labels instead of one generic fallback. |
| `PLIB-105` / [#3707](https://github.com/OpenAgentsInc/openagents/issues/3707) | planned | Deepen compiler passes: schedule formation, fusion policy, memory planning, plan cache identity, and compile-cache evidence. |
| `PLIB-106` / [#3708](https://github.com/OpenAgentsInc/openagents/issues/3708) | planned | Complete same-type local multi-device behavior beyond implemented-early substrate, including sharding policy and refusal taxonomy. |
| `PLIB-107` / [#3709](https://github.com/OpenAgentsInc/openagents/issues/3709) | planned | Promote framework-core acceptance from representative proof to broad contract coverage with fixture-backed replay and failure tests. |
| `PLIB-108` / [#3710](https://github.com/OpenAgentsInc/openagents/issues/3710) | planned | Define one cross-library refusal taxonomy covering unsupported ops, unsupported gradients, unsupported layouts, unsupported backend capabilities, serialization incompatibility, sandbox policy denial, and topology mismatch. |
| `PLIB-109` / [#3711](https://github.com/OpenAgentsInc/openagents/issues/3711) | planned | Formalize storage identity, alias invariants, view semantics, and complex or advanced dtype rules as core contracts rather than backend accidents. |
| `PLIB-110` / [#3712](https://github.com/OpenAgentsInc/openagents/issues/3712) | planned | Add framework-core RNG, seeding, generator-state, and deterministic-algorithm contracts that survive replay, checkpoint, multi-device execution, and distributed execution replay. |
| `PLIB-111` / [#3713](https://github.com/OpenAgentsInc/openagents/issues/3713) | planned | Define stable custom-op schema, kernel registration, and backend dispatch contracts so extensibility does not fork the core. |
| `PLIB-112` / [#3714](https://github.com/OpenAgentsInc/openagents/issues/3714) | planned | Add transform-safe graph and functionalization foundations so higher-level program transforms and export contracts can build on explicit IR rules. |
| `PLIB-113` / [#3715](https://github.com/OpenAgentsInc/openagents/issues/3715) | planned | Add sparse, nested, masked, and storage-aware meta execution foundations so non-dense families can enter through typed contracts rather than ad hoc exceptions. |

## Epic 2: Semantics And Compatibility

### Goal

Build the PyTorch-credible layer above the compact framework core.

### Exit Criteria

- the library can support common training and inference code with credible
  tensor, module, optimizer, state, precision, quantization, data, and
  reproducibility behavior
- distributed-training semantics are explicit above cluster truth rather than
  implied by execution substrate alone
- transforms, export surfaces, and extension contracts are real enough for
  third parties to build on them honestly
- compatibility breadth is achieved through explicit registries and harnesses
- the semantics layer does not bloat the core IR and runtime

### Why This Epic Exists

This is the epic that turns "serious engine substrate" into "practical ML
library."

It is the most important missing middle layer in current Psionic.

Master issue:
[#3742](https://github.com/OpenAgentsInc/openagents/issues/3742)

### Issues

### State And Training Semantics

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `PLIB-201` / [#3716](https://github.com/OpenAgentsInc/openagents/issues/3716) | planned | Add a first-class module, parameter, buffer, and state-tree system. |
| `PLIB-202` / [#3717](https://github.com/OpenAgentsInc/openagents/issues/3717) | planned | Add deterministic `state_dict`-style naming, strict and non-strict load behavior, and size-mismatch refusal semantics. |
| `PLIB-203` / [#3718](https://github.com/OpenAgentsInc/openagents/issues/3718) | planned | Widen optimizer coverage with scheduler integration, parameter-group semantics, and stronger state behavior. |
| `PLIB-204` / [#3719](https://github.com/OpenAgentsInc/openagents/issues/3719) | planned | Define serialization and checkpoint compatibility boundaries for practical PyTorch interoperability without inheriting every historical artifact path. |

### Parity And Compiler Hygiene

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `PLIB-205` / [#3720](https://github.com/OpenAgentsInc/openagents/issues/3720) | planned | Add a PyTorch-derived operator parity matrix analogous to `op_db` / `OpInfo` for Rust-native conformance. |
| `PLIB-206` / [#3721](https://github.com/OpenAgentsInc/openagents/issues/3721) | planned | Add a PyTorch-derived module parity matrix analogous to `module_db`. |
| `PLIB-207` / [#3722](https://github.com/OpenAgentsInc/openagents/issues/3722) | planned | Add a PyTorch-derived optimizer parity matrix analogous to `optim_db`. |
| `PLIB-208` / [#3723](https://github.com/OpenAgentsInc/openagents/issues/3723) | planned | Add symbolic-shape, fake-tensor, and compiler-hygiene parity harnesses informed by modern PyTorch compiler tests. |
| `PLIB-209` / [#3724](https://github.com/OpenAgentsInc/openagents/issues/3724) | planned | Make the semantics layer honest about what is `PyTorch-credible` versus what remains `PyTorch-compatible later`. |

### Tensor, Dtype, And Reproducibility Semantics

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `PLIB-210` / [#3725](https://github.com/OpenAgentsInc/openagents/issues/3725) | planned | Add first-class sparse, nested, masked, and storage-aware tensor semantics with explicit capability matrices and refusal behavior. |
| `PLIB-211` / [#3726](https://github.com/OpenAgentsInc/openagents/issues/3726) | planned | Add complex-number, low-precision, and advanced dtype semantics with explicit promotion, casting, and backend-capability rules. |
| `PLIB-212` / [#3727](https://github.com/OpenAgentsInc/openagents/issues/3727) | planned | Define framework-wide reproducibility semantics covering global and per-device RNG, deterministic modes, and RNG state restore across train, eval, and distributed replay. |
| `PLIB-213` / [#3728](https://github.com/OpenAgentsInc/openagents/issues/3728) | planned | Add an autocast-style precision-policy system with backend-aware rules, typed refusal surfaces, and numerics diagnostics. |
| `PLIB-214` / [#3729](https://github.com/OpenAgentsInc/openagents/issues/3729) | planned | Add gradient scaling plus overflow and underflow handling for train-class mixed precision. |

### Quantization And Transforms

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `PLIB-215` / [#3730](https://github.com/OpenAgentsInc/openagents/issues/3730) | planned | Build quantization as a library capability family: PTQ, QAT, quantization configuration, backend quantization contracts, compiler or export-aware quantization flows, and quantized execution semantics above file-format decode. |
| `PLIB-216` / [#3731](https://github.com/OpenAgentsInc/openagents/issues/3731) | planned | Add functionalization and program-transform capability for symbolic rewrites, export-safe graphs, and future `vmap` / `jvp` / `jacobian`-class transforms. |

### Extensibility And Data Systems

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `PLIB-217` / [#3732](https://github.com/OpenAgentsInc/openagents/issues/3732) | planned | Publish user-facing extension contracts for custom ops, custom kernels, custom autograd, backend plugins, and quantizer plugins. |
| `PLIB-218` / [#3733](https://github.com/OpenAgentsInc/openagents/issues/3733) | planned | Add dataset, iterable-streaming, sampler, batch-sampler, and host-device staging abstractions as reusable library capability, not app glue. |
| `PLIB-219` / [#3734](https://github.com/OpenAgentsInc/openagents/issues/3734) | planned | Add distributed and sharded data-feed semantics, including sampler partitioning, worker coordination, and replay-safe input ordering. |

### Advanced Operator Families And Export

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `PLIB-220` / [#3735](https://github.com/OpenAgentsInc/openagents/issues/3735) | planned | Add advanced operator-family programs for linalg, fft or signal, distributions, special functions, and attention-family semantics. |
| `PLIB-221` / [#3736](https://github.com/OpenAgentsInc/openagents/issues/3736) | planned | Add exportable graph and deployment artifact contracts so the semantics layer can hand off stable graph units independent of raw checkpoints. |

## Epic 3: Model IO And Runtime Families

### Goal

Finish the serving, model-loading, tokenizer, cache, router, and runtime-family
layers so Psionic is a truthful reusable runtime for common model classes.

### Exit Criteria

- common GGUF and safetensors model families load truthfully
- cache, tokenizer, prompt, and runtime semantics are explicit
- serving surfaces converge on reusable contracts rather than lane-specific glue
- router and placement behavior remain machine-legible

### Shipped Foundations

- reusable model descriptors and loaders in `psionic-models`
- early GGUF and safetensors support
- chat, responses, embeddings, structured-output, tool, and reasoning surfaces
- router-owned placement and reliability controls

### Issues

| ID | Status | Work |
| --- | --- | --- |
| `PLIB-301` | planned | Complete GGUF quant decode, especially K-family formats and remaining block-layout coverage. |
| `PLIB-302` | planned | Strengthen tokenizer, prompt-template, and control-token fidelity across GGUF and safetensors families. |
| `PLIB-303` | planned | Finish KV cache, prefix cache, paged storage, and explicit cache admission or refusal behavior. |
| `PLIB-304` | planned | Generalize model-family runtime adapters beyond the current strongest decoder families. |
| `PLIB-305` | planned | Converge chat, responses, and embeddings surfaces on one coherent library contract and capability-reporting model. |
| `PLIB-306` | planned | Harden structured output, tool calling, reasoning, and continuation semantics across supported families. |
| `PLIB-307` | planned | Deepen router placement, warm/cold policy, prefix reuse, and cache-aware scheduling truth. |
| `PLIB-308` | planned | Add model-family and runtime smoke suites that defend the actual reusable runtime contracts, not only one-off lanes. |

`PLIB-301` is necessary for model-family closure, but GGUF quant decode alone is
not quantization capability parity. GGUF quant decode MUST NOT be described as
quantization support. PTQ, QAT, backend quant contracts, and quantized
execution semantics live in Epic 2.

## Epic 4: Backend Truth And Performance

### Goal

Make backend support explicit, benchmarked, and honest, while preserving the
small visible core that makes new backend bring-up tractable.

### Exit Criteria

- each declared backend lane has explicit readiness, capability, risk, and
  performance truth
- each declared backend lane MUST publish capability matrix coverage before
  readiness claims count
- benchmark acceptance exists where throughput claims matter
- backend capability matrices cover determinism, precision, quantization, and
  profiler surfaces rather than only "can it run"
- backend bring-up uses a small visible substrate rather than backend-specific
  hidden forks

### Shipped Foundations

- CPU reference execution lane
- Metal execution and Apple-specific serving lanes
- CUDA architecture and truthful readiness surface
- AMD KFD and AMD userspace discovery and readiness substrate

### Issues

| ID | Status | Work |
| --- | --- | --- |
| `PLIB-401` | planned | Complete CPU as the canonical deterministic reference backend with the strongest replay and conformance story. |
| `PLIB-402` | partially active | Refresh the Metal lane against the new full-library roadmap and close the remaining correctness and benchmark gaps for real model families. |
| `PLIB-403` | partially active | Refresh the CUDA lane against the new full-library roadmap and shift remaining work from host-specific throughput queues to reusable backend and runtime closure. |
| `PLIB-404` | planned | Move AMD KFD from discovery or readiness truth into executable backend closure with explicit capability limits. |
| `PLIB-405` | planned | Define AMD userspace as an explicitly gated experimental backend with strong risk posture, not a silent alternate path. |
| `PLIB-406` | planned | Add backend profiler and kernel-evidence surfaces that make execution and performance machine-legible. |
| `PLIB-407` | planned | Write a backend bring-up kit and acceptance ladder so future accelerators can target the core honestly. |
| `PLIB-408` | planned | Publish backend capability matrices for determinism, autocast, grad scaling, sparse or nested coverage, quantization, and distributed collectives so precision and train claims stay honest. |

## Epic 5: Cluster, Sandbox, And Execution Truth

### Goal

Turn Psionic's existing cluster, sandbox, net, datastream, and provider
substrate into one coherent execution-truth layer for local and networked
compute.

This epic is about distributed execution truth, not the full user-visible
semantics of DDP, FSDP, tensor sharding, or pipeline training. Those training
semantics land in Epic 6 on top of the execution substrate defined here.

Execution substrate does NOT imply framework-level distributed training.

DDP, FSDP, tensor sharding, and pipeline semantics MUST be implemented
explicitly above the execution substrate.

### Exit Criteria

- cluster identity, topology, placement, and execution truth are reusable and
  honest
- process-group, collective, remote-execution, and elastic-membership substrate
  are explicit enough for higher-level training semantics to depend on them
- sandbox execution remains explicit and bounded
- provider capability surfaces derive from runtime truth rather than ad hoc
  product glue
- receipts, manifests, replay, and proof bundles remain first-class
- graph, plan, kernel, cache, refusal, and topology explanations are
  inspectable enough that the system remains visibly debuggable

### Shipped Foundations

- `psionic-net`
- `psionic-datastream`
- `psionic-cluster`
- `psionic-sandbox`
- `psionic-provider`
- lane-specific cluster, Metal, and FM roadmap work

### Issues

The interactive-environment contracts in `PLIB-512` through `PLIB-514` must
remain:

- benchmark-agnostic
- action-schema-agnostic
- score-policy-agnostic
- game-state-taxonomy-agnostic

| ID | Status | Work |
| --- | --- | --- |
| `PLIB-501` | planned | Reconcile the existing cluster substrate and lane-specific roadmap into one full-library cluster contract. |
| `PLIB-502` | planned | Finish topology-aware placement and scheduling as reusable library behavior rather than lane-local logic. |
| `PLIB-503` | planned | Converge datastream, artifact staging, and replay receipts into a single execution-evidence model. |
| `PLIB-504` | planned | Define bounded sandbox execution as a first-class Psionic library lane with explicit runtime, file, network, and policy truth. |
| `PLIB-505` | planned | Tighten provider capability derivation so all advertised compute products map back to explicit runtime evidence and refusal conditions. |
| `PLIB-506` | planned | Clarify kernel/Nexus authority boundaries for execution evidence, accepted outcomes, and later settlement-facing projections. |
| `PLIB-507` | planned | Add chaos and failure-injection coverage across cluster, artifact, replay, and sandbox paths. |
| `PLIB-508` | planned | Add one cross-library observability and debug surface covering graph, module, and plan inspection, kernel and memory evidence, cache reasoning, refusal and mismatch explanation, topology explanation, and train/eval receipt inspection. |
| `PLIB-509` | planned | Define reusable process-group and collective semantics, distinct from lane-local cluster membership, so framework-level distributed training has a stable substrate. |
| `PLIB-510` | planned | Add bounded remote-execution and RPC contracts with explicit provenance, capability, refusal, and receipt semantics. |
| `PLIB-511` | planned | Add elastic membership and fault-tolerant distributed run-control substrate for restart, rejoin, and topology revision without hiding failures. |
| `PLIB-512` | planned | Add structured interactive environment turn contracts with typed observations, actions, resets, terminal transitions, and resume-safe session snapshots while keeping the substrate benchmark-agnostic, action-schema-agnostic, score-policy-agnostic, and game-state-taxonomy-agnostic. |
| `PLIB-513` | planned | Add episode and trajectory receipt families with per-step observation/action/result hashing and final episode summaries distinct from text-session transcripts, without embedding benchmark-specific action or score semantics. |
| `PLIB-514` | planned | Add a generic bridge from interactive environment sessions into eval samples, repeated-run aggregation, and benchmark evidence packs without hard-coding benchmark-specific aggregation or policy logic. |
| `PLIB-515` | planned | Turn collective semantics into actual train-class collective execution paths with evidence for `all_reduce`, `all_gather`, and related multi-rank behavior that higher-level training lanes can rely on. |

## Epic 6: Training, Eval, And Research

### Goal

Make Psionic's training-class substrate real first through decentralized
adapter training, then widen toward broader train-class library closure.

### Exit Criteria

- decentralized adapter training is honest and end-to-end
- eval, validator, and accepted-outcome surfaces are reusable
- training is broader than a single narrow Apple-only lane
- framework-level distributed training semantics are explicit, testable, and
  clearly separated from cluster substrate truth
- research and hillclimb loops build on the same receipts and validator truth

Training readiness requires:

- deterministic checkpoint restore
- optimizer state replay
- gradient equivalence across runs

### Shipped Foundations

- train system substrate in `psionic-train`
- eval runtime and benchmark package substrate
- Apple adapter training/export lane
- distributed optimizer, collective, run-graph, orchestrator, and checkpoint
  foundations

### Active GitHub Block

This epic already has an active GitHub issue program:

- umbrella:
  [#3649](https://github.com/OpenAgentsInc/openagents/issues/3649)
- support and spec:
  [#3636](https://github.com/OpenAgentsInc/openagents/issues/3636)
- execution truth:
  [#3637](https://github.com/OpenAgentsInc/openagents/issues/3637),
  [#3638](https://github.com/OpenAgentsInc/openagents/issues/3638),
  [#3639](https://github.com/OpenAgentsInc/openagents/issues/3639),
  [#3640](https://github.com/OpenAgentsInc/openagents/issues/3640),
  [#3641](https://github.com/OpenAgentsInc/openagents/issues/3641),
  [#3642](https://github.com/OpenAgentsInc/openagents/issues/3642),
  [#3643](https://github.com/OpenAgentsInc/openagents/issues/3643),
  [#3644](https://github.com/OpenAgentsInc/openagents/issues/3644)
- provider and operator surfaces:
  [#3645](https://github.com/OpenAgentsInc/openagents/issues/3645),
  [#3646](https://github.com/OpenAgentsInc/openagents/issues/3646)
- widening and QA:
  [#3647](https://github.com/OpenAgentsInc/openagents/issues/3647),
  [#3648](https://github.com/OpenAgentsInc/openagents/issues/3648)

### Issues

| ID | Status | Work |
| --- | --- | --- |
| `PLIB-601` | open | Deliver decentralized adapter training end to end via the active `#3649` issue block. |
| `PLIB-602` | open | Treat Apple adapters as the first bounded lane, not the final architecture, and widen to an open adapter backend through `#3647`. |
| `PLIB-603` | planned | Build the broader module and state training layer needed for non-adapter common training workloads. |
| `PLIB-604` | planned | Deepen validator-owned benchmark and accepted-outcome truth so train/eval objects remain reusable outside one product lane. |
| `PLIB-605` | planned | Build research and hillclimb loops over the same typed train/eval receipts instead of sidecar experiment logic. |
| `PLIB-606` | planned | Reconcile train-class cluster and collective execution with the broader full-library cluster substrate in Epic 5. |
| `PLIB-607` | planned | Add framework-level data-parallel semantics, including parameter synchronization, gradient reduction, buffer broadcast, and failure/refusal behavior. |
| `PLIB-608` | planned | Add parameter, optimizer-state, and tensor-placement or sharding semantics for FSDP or DTensor-class workloads. |
| `PLIB-609` | planned | Add pipeline-parallel stage, schedule, microbatch, and activation-lifetime semantics as a reusable training capability family. |
| `PLIB-610` | planned | Add dense and sharded distributed checkpoint semantics, including optimizer-state checkpointing and restart or recovery contracts for train-class runs. |
| `PLIB-611` | planned | Build elastic and fault-tolerant distributed training semantics on top of Epic 5 membership contracts instead of lane-specific orchestration. |
| `PLIB-612` | planned | Expand train-class operator coverage for non-adapter models: gather, scatter-add, pad, argmax, BCE-with-logits, and softmax cross-entropy. |
| `PLIB-613` | planned | Strengthen model-state and train-state IO with stable manifests, promoted checkpoint shapes, and small-model local training harnesses suitable for research workloads beyond the current adapter-first lane. |
| `PLIB-614` | planned | Add training-class attention, sparse-embedding, and ACT-style loop support so small recursive and HRM-class models can target Psionic through reusable contracts rather than local exceptions. |

## Epic 7: Interop And Adoption

### Goal

Provide the path from "good Rust-native engine" to "realistically adoptable by
teams with PyTorch-shaped workflows."

### Exit Criteria

- practical checkpoint and model migration paths exist
- dense, sharded, and exported-graph migration paths are named and bounded
- compatibility claims are explicit, limited, and version-bounded
- supported, convertible, and unsupported capability families are published
- the project can honestly describe where it is a PyTorch replacement and where
  it is not

### Issues

| ID | Status | Work |
| --- | --- | --- |
| `PLIB-701` | planned | Freeze the library's compatibility contract so `library-usable`, `execution-truthful`, `PyTorch-credible`, and `PyTorch-compatible` remain distinct, versioned, and differently tested. |
| `PLIB-702` | planned | Add safe conversion tools for common checkpoint and artifact migration into Psionic-owned formats. |
| `PLIB-703` | planned | Add bounded workflow-compatibility adapters around checkpoints, exported graphs, or runtime contracts only after Epics 1 through 3 are materially real. |
| `PLIB-704` | planned | Build a reference model zoo and migration guide proving common families can run without app-local glue. |
| `PLIB-705` | planned | Define distribution and packaging for Psionic as a standalone library, not only as an internal repo subtree. |
| `PLIB-706` | planned | Publish a clear adoption story for teams choosing between Psionic-native, PyTorch-compatible, and mixed-runtime usage. |
| `PLIB-707` | planned | Add migration tooling and policy for dense and sharded checkpoints, with explicit supported, convertible, and unsupported artifact classes. |
| `PLIB-708` | planned | Define realistic distributed-state migration and restart story for training workloads instead of implying every PyTorch distributed artifact will import directly. |
| `PLIB-709` | planned | Add stable exported-graph, package, and deployment artifact contracts so adoption does not depend only on raw checkpoints. |
| `PLIB-710` | planned | Publish capability-family matrices covering supported, convertible, and unsupported status across ops, distributed, quantization, export, and backend surfaces. |

### Migration Staircase

The adoption path should stay explicit:

1. Psionic-native internal models and runtimes.
2. Imported checkpoints through explicit conversion.
3. Exported graphs, packaged runtime units, and bounded mixed-runtime usage.
4. Explicit compatibility shells or adapters only where the capability matrix is
   already honest.
5. Only after the earlier steps are honest: "most workloads" adoption claims.

## Current Execution Order

This is the recommended dependency order for the next full-library work.

### Phase 1: lock the claim vocabulary and framework-core shape

1. `PLIB-002` canonical claim vocabulary
2. `PLIB-005` upstream PyTorch version window
3. `PLIB-101` tensor semantics breadth
4. `PLIB-102` operator registry and dispatch layer
5. `PLIB-103` fake or meta execution
6. `PLIB-105` compiler, schedule, memory-planning, and plan-cache depth
7. `PLIB-108` refusal taxonomy
8. `PLIB-109` storage and alias contracts
9. `PLIB-110` RNG and determinism substrate
10. `PLIB-112` transform-safe graph foundations
11. `PLIB-104`, `PLIB-106`, and `PLIB-107` autodiff breadth, local
    multi-device closure, and broad core acceptance

### Phase 2a: state, training, and reproducibility semantics

12. `PLIB-201` through `PLIB-204` module, state, optimizer, and checkpoint
    semantics
13. `PLIB-113` non-dense core foundations
14. `PLIB-210` through `PLIB-214` tensor-family, dtype, reproducibility, and
    precision semantics

### Phase 2b: parity, transforms, advanced families, and export

15. `PLIB-205` through `PLIB-209` parity harnesses and capability-scope truth
16. `PLIB-215` and `PLIB-216` quantization and transform capability
17. `PLIB-220` and `PLIB-221` advanced operator families and export artifacts

### Phase 2c: extension and data systems

18. `PLIB-111` extension-ready core registration contracts
19. `PLIB-217` through `PLIB-219` extension surfaces and data systems

### Phase 3: finish runtime-family truth

20. `PLIB-301` GGUF K-family and remaining quant decode
21. `PLIB-302` tokenizer and prompt fidelity
22. `PLIB-303` cache and paged-storage completion
23. `PLIB-305` through `PLIB-307` serving and router contract convergence

### Phase 4: keep backend truth honest while widening execution breadth

24. `PLIB-401` CPU reference closure
25. `PLIB-402` Metal lane refresh and closure
26. `PLIB-403` CUDA lane refresh and closure
27. `PLIB-404` and `PLIB-405` AMD execution closure
28. `PLIB-406` through `PLIB-408` backend evidence, profiler, and capability
    matrices

### Phase 5: reconcile distributed execution truth

29. `PLIB-501` through `PLIB-515`

### Phase 6: execute the training and eval program

30. `PLIB-601` through `PLIB-614`

### Phase 7: only then expose broader adoption and compatibility shells

31. `PLIB-701` through `PLIB-710`

## Program Risks And Dependency Hazards

### Risk 1: operator breadth before operator schema discipline

If `PLIB-101` and `PLIB-102` are weak, later parity work becomes expensive
churn rather than reusable growth.

### Risk 2: module and state semantics lag training and checkpoint work

If `PLIB-201` and `PLIB-202` slip, Epic 6 will become lane-specific and
fragile instead of library-reusable.

### Risk 3: backend closure racing ahead of CPU reference truth

If accelerated lanes advance faster than `PLIB-401` and the CPU reference rule,
correctness disputes will become hard to adjudicate.

### Risk 4: runtime-family glue outrunning reusable contracts

If model-family work lands as family-specific exceptions, Epic 3 becomes
another pile of local wins rather than a coherent library runtime layer.

### Risk 5: interop pressure arriving before semantics are real

If `PLIB-703` starts early, the team will optimize a demo surface instead of
the substrate.

### Risk 6: refusal taxonomy drift across crates

If `PLIB-108` does not land early, the system will become harder to reason
about exactly where and why behavior is unsupported.

### Risk 7: observability scattered instead of designed

If inspection, receipts, replay, cache, and refusal explanation remain spread
across ad hoc surfaces, Psionic will lose the "small, visible core" advantage
even if the code remains technically modular.

### Risk 8: cluster truth mistaken for distributed training semantics

If Epic 5 is treated as equivalent to DDP, FSDP, tensor sharding, or pipeline
semantics, the roadmap will overclaim framework capability while only shipping
execution substrate.

### Risk 9: quant decode mistaken for quantization closure

If `PLIB-301` lands without `PLIB-213` through `PLIB-215`, the project will
look quantization-aware in demos while still lacking a real quantization
system.

### Risk 10: capability-matrix explosion without prioritization

If operator, dtype, tensor-family, backend, distributed, quantization, and
export matrices all expand at once without tiered targets, the roadmap will
produce scattered partial coverage instead of honest replacement lanes.

### Risk 11: interactive benchmark growth staying trapped in text-session abstractions

If `PLIB-512` through `PLIB-514` slip, interactive benchmark families will keep
rebuilding environment and trajectory logic outside the reusable Psionic layer.

### Risk 12: adapter-first training substrate mistaken for broader model closure

If `PLIB-612` through `PLIB-614` do not land, non-adapter model ports will
either stall or force benchmark-specific training primitives into higher layers.

## Roadmap Rules

### 1. Do not treat one host or one benchmark chain as the whole roadmap

Host-specific perf work still matters.

It is no longer the canonical map of Psionic.

### 2. Do not let compatibility breadth bloat the framework core

Compatibility belongs above the core, not inside it.

### 3. Do not let tinygrad-style minimalism justify missing semantics

Small visible core is a design constraint.

It is not a waiver from broad operator, module, optimizer, and checkpoint
behavior.

### 4. Do not let app or authority logic leak into Psionic crates

This remains non-negotiable under `docs/OWNERSHIP.md`.

### 5. Do not open a compatibility shell before the substrate is real

A compatibility shell without real semantics is not adoption.

It is a demo.

### 6. Do not confuse artifact decode or cluster truth with framework parity

GGUF quant decode MUST NOT be described as quantization support.

Cluster execution truth is not distributed training semantics.

### 7. New capability families enter through framework-generic contracts first

Do not add one-off support because one ecosystem package happens to need it.

New capability families should enter through reusable framework contracts first.

### 8. Unsupported backend semantics must refuse explicitly

Unsupported backend semantics MUST fail as typed refusal rather than silently
falling back to CPU.

## Benchmark Honesty Rules

- every benchmark claim must declare backend, dtype, quantization, batch
  shape, cache state, warm/cold status, and refusal conditions
- no benchmark acceptance based on app-local patches or hidden environment
  assumptions
- performance wins are not roadmap progress if they bypass reusable library
  contracts
- unsupported workloads must refuse explicitly, not degrade silently into
  incomparable alternate paths
- benchmark evidence must remain attributable to the library path being claimed,
  not to a lane-local workaround

## Bottom Line

The old Psionic roadmaps were optimized for narrower questions:

- can we replace one local runtime
- can we make one host truthful
- can we bring up one lane

This roadmap is optimized for the larger question:

> how does Psionic become a full reusable Rust-native library?

The answer is:

- finish the compact framework core
- build the missing semantics and compatibility layer
- keep model/runtime/backend/cluster/train truth explicit and machine-legible
- then add adoption and compatibility shells on top of a real substrate

The program is not complete when Psionic can run impressive demos. It is
complete when capability matrices, refusal behavior, replay truth, and
migration paths make its replacement claims auditable.

That is now the canonical Psionic roadmap.
