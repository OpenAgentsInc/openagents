# 2026-03-15 "Can LLMs Be Computers?" Psionic Adaptation Audit

> Updated 2026-03-15 after landing the `Tassadar` Phase 1 reference substrate
> and opening dedicated follow-on issues for phases 2 through 6.

## Intent

This audit answers a specific implementation question:

> after reading the current Psionic docs and code, and reading the Percepta
> article "Can LLMs Be Computers?", what is the honest path for OpenAgents to
> implement those concepts inside Psionic?

The useful answer is not:

- "replace Psionic serving with a research demo"
- "pretend GPT-OSS or Apple FM can inherit this fast path automatically"
- "drop a convex-hull decoder into the existing generic text-generation lane
  and call the problem solved"
- "treat a special 2D-head executor model as if it were just another ordinary
  chat model"
- "widen the current compute-market MVP around this immediately"

The useful answer is:

- treat the article as a proposal for a new executor-class model and runtime
  lane inside Psionic
- keep it below product and authority layers, because this is framework-core,
  model-runtime, and execution-truth work
- start CPU-first with exactness, trace ABI, cache identity, and benchmark
  packages before pursuing accelerator claims or hybrid productization
- only later connect the executor lane to router, serve, and broader research
  or training loops

That is the line this audit makes concrete.

## Scope

Article reviewed:

- Percepta / Field Notes
- "Can LLMs Be Computers?"
- published March 11, 2026
- user-provided full text in this conversation
- user-provided follow-up author statements in this conversation:
  - the demonstrated transformer uses handcrafted weights with a proof of
    correctness tied to the WebAssembly spec
  - the point is to give LLMs inner computational ability, not replace
    conventional computers
  - direct CPU execution is still orders faster, but the transformer overhead
    is argued to be closer to a constant-factor gap than the quadratic growth
    of standard decoding
  - learned or grown internal circuits are a next-step direction, with
    SUBLEQ-style precedents noted by the author

OpenAgents sources reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `crates/psionic/README.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/ROADMAP.md`
- `crates/psionic/docs/INFERENCE_ENGINE.md`
- `crates/psionic/docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md`
- `crates/psionic/docs/RESEARCH_EXPERIMENT_REFERENCE.md`
- `crates/psionic/psionic-core/src/lib.rs`
- `crates/psionic/psionic-ir/src/lib.rs`
- `crates/psionic/psionic-compiler/src/lib.rs`
- `crates/psionic/psionic-runtime/src/lib.rs`
- `crates/psionic/psionic-runtime/src/proof.rs`
- `crates/psionic/psionic-runtime/src/parity.rs`
- `crates/psionic/psionic-runtime/src/validation.rs`
- `crates/psionic/psionic-runtime/src/gpt_oss.rs`
- `crates/psionic/psionic-runtime/src/activation_fingerprint.rs`
- `crates/psionic/psionic-runtime/src/tassadar.rs`
- `crates/psionic/psionic-models/src/lib.rs`
- `crates/psionic/psionic-models/src/tassadar.rs`
- `crates/psionic/psionic-models/src/sharding.rs`
- `crates/psionic/psionic-serve/src/lib.rs`
- `crates/psionic/psionic-serve/src/conformance.rs`
- `crates/psionic/psionic-research/src/lib.rs`
- `docs/kernel/README.md`
- `docs/kernel/economy-kernel.md`

Relevant prior audits reviewed:

- `docs/audits/2026-03-15-tinygrad-parity-target-for-psionic-audit.md`
- `docs/audits/2026-03-15-tinygrad-test-suite-port-audit.md`
- `docs/audits/2026-03-15-pytorch-test-suite-port-audit.md`
- `docs/audits/2026-03-15-full-pytorch-port-to-rust-in-psionic-audit.md`
- `docs/audits/2026-03-15-arcprize-rust-port-and-psionic-integration-audit.md`
- `docs/audits/2026-03-15-rlm-psionic-economy-kernel-integration-audit.md`

Live GitHub issue state reviewed on March 15, 2026:

- [OpenAgentsInc/openagents#3749](https://github.com/OpenAgentsInc/openagents/issues/3749)
  - `Psionic Tassadar Phase 6: add typed runtime capabilities and fallback diagnostics`
- [OpenAgentsInc/openagents#3748](https://github.com/OpenAgentsInc/openagents/issues/3748)
  - `Psionic Tassadar Phase 5: implement hull-cache fast path behind exact equivalence`
- [OpenAgentsInc/openagents#3747](https://github.com/OpenAgentsInc/openagents/issues/3747)
  - `Psionic Tassadar Phase 4: add executor trace proof bundles and manifest lineage`
- [OpenAgentsInc/openagents#3746](https://github.com/OpenAgentsInc/openagents/issues/3746)
  - `Psionic Tassadar Phase 3: add benchmark and environment packages for exact executor evaluation`
- [OpenAgentsInc/openagents#3745](https://github.com/OpenAgentsInc/openagents/issues/3745)
  - `Psionic Tassadar Phase 2: define executor model and program artifact contracts`
- [OpenAgentsInc/openagents#3744](https://github.com/OpenAgentsInc/openagents/issues/3744)
  - `Psionic Executor Lane Phase 1: land CPU reference WebAssembly executor fixture and exact parity harness` (closed)
- [OpenAgentsInc/openagents#3743](https://github.com/OpenAgentsInc/openagents/issues/3743)
  - `Psionic Executor Lane Phase 0: declare WebAssembly-first executor lane scope, ownership, and issue spine` (closed)
- [OpenAgentsInc/openagents#3742](https://github.com/OpenAgentsInc/openagents/issues/3742)
  - `Psionic Epic 2 Master: semantics and compatibility`
- [OpenAgentsInc/openagents#3741](https://github.com/OpenAgentsInc/openagents/issues/3741)
  - `Psionic Epic 1 Master: framework core completion`
- [OpenAgentsInc/openagents#3736](https://github.com/OpenAgentsInc/openagents/issues/3736)
  - `PLIB-221 Psionic Semantics: add exportable graph and deployment artifact contracts so the semantics layer can hand off stable graph units independent of raw checkpoints`
- [OpenAgentsInc/openagents#3735](https://github.com/OpenAgentsInc/openagents/issues/3735)
  - `PLIB-220 Psionic Semantics: add advanced operator-family programs for linalg, fft or signal, distributions, special functions, and attention-family semantics`
- [OpenAgentsInc/openagents#3732](https://github.com/OpenAgentsInc/openagents/issues/3732)
  - `PLIB-217 Psionic Semantics: publish user-facing extension contracts for custom ops, custom kernels, custom autograd, backend plugins, and quantizer plugins`
- [OpenAgentsInc/openagents#3723](https://github.com/OpenAgentsInc/openagents/issues/3723)
  - `PLIB-208 Psionic Semantics: add symbolic-shape, fake-tensor, and compiler-hygiene parity harnesses informed by modern PyTorch compiler tests`
- [OpenAgentsInc/openagents#3720](https://github.com/OpenAgentsInc/openagents/issues/3720)
  - `PLIB-205 Psionic Semantics: add a PyTorch-derived operator parity matrix analogous to op_db / OpInfo for Rust-native conformance`
- [OpenAgentsInc/openagents#3714](https://github.com/OpenAgentsInc/openagents/issues/3714)
  - `PLIB-112 Psionic Framework Core: add transform-safe graph and functionalization foundations so higher-level program transforms and export contracts can build on explicit IR rules`
- [OpenAgentsInc/openagents#3713](https://github.com/OpenAgentsInc/openagents/issues/3713)
  - `PLIB-111 Psionic Framework Core: define stable custom-op schema, kernel registration, and backend dispatch contracts so extensibility does not fork the core`
- [OpenAgentsInc/openagents#3710](https://github.com/OpenAgentsInc/openagents/issues/3710)
  - `PLIB-108 Psionic Framework Core: define one cross-library refusal taxonomy covering unsupported ops, unsupported gradients, unsupported layouts, unsupported backend capabilities, serialization incompatibility, sandbox policy denial, and topology mismatch`
- [OpenAgentsInc/openagents#3709](https://github.com/OpenAgentsInc/openagents/issues/3709)
  - `PLIB-107 Psionic Framework Core: promote framework-core acceptance from representative proof to broad contract coverage with fixture-backed replay and failure tests`
- [OpenAgentsInc/openagents#3707](https://github.com/OpenAgentsInc/openagents/issues/3707)
  - `PLIB-105 Psionic Framework Core: deepen compiler passes: schedule formation, fusion policy, memory planning, plan cache identity, and compile-cache evidence`

## Executive Summary

The Percepta article is a strong fit for Psionic, but only if OpenAgents reads
it as a new execution lane rather than as a drop-in improvement to current
general LLM serving.

That is no longer just a proposal. As of this update:

- Phase 0 is closed under
  [#3743](https://github.com/OpenAgentsInc/openagents/issues/3743)
- Phase 1 is closed under
  [#3744](https://github.com/OpenAgentsInc/openagents/issues/3744)
- the landed reference substrate is now codenamed `Tassadar`
- the next active issue spine is
  [#3745](https://github.com/OpenAgentsInc/openagents/issues/3745) through
  [#3749](https://github.com/OpenAgentsInc/openagents/issues/3749)

The most important architectural reading is:

> Psionic should treat these ideas as a new executor-class model family with a
> specialized trace ABI, decoding path, cache algorithm, and validation stack.

That means:

- this belongs in `crates/psionic/*`, not in app code and not in kernel
  authority
- it is framework-core, model-runtime, cache, proof, and eval work first
- it is not current MVP product scope
- it should start on CPU as a reference lane
- it should target WebAssembly semantics first, because that is where the
  author says the correctness proof is anchored
- it should not be bolted into `psionic.text_generation` as an invisible
  alternate kernel path for ordinary decoder models
- it should be positioned as inner computational substrate for larger reasoning
  systems, not as a replacement for native execution

The article plus the follow-up author clarifications really propose five
distinct things:

1. a restricted transformer regime
   - especially 2D head geometry
2. an append-only execution-trace ABI
   - interpreter state encoded as generated tokens
3. a specialized fast decoding path
   - hard-max or sparse retrieval over a hull-style cache
4. a proof-oriented execution target
   - handcrafted weights and a correctness story tied to a WebAssembly-spec
     executor
5. a longer-term compiler and learning path
   - source program -> Wasm/runtime artifact -> possibly directly into weights,
     and later learned or grown circuit families

Psionic already has the right owner split for this:

- `psionic-models`
  - model descriptors, token boundaries, runtime surface identity
- `psionic-core` / `psionic-ir` / `psionic-compiler`
  - operator and execution-plan truth
- `psionic-runtime`
  - cache identity, runtime capability, proof bundles, validation, parity
- `psionic-serve`
  - served product contracts and streaming semantics
- `psionic-eval` / `psionic-environments`
  - benchmark packages and correctness harnesses
- `psionic-research`
  - typed experiments after the substrate exists

The best near-term path is:

1. define a Psionic-native, WebAssembly-first executor trace ABI and benchmark
   package family
2. land a CPU-only reference executor model using programmatic fixture weights
   before training claims
3. benchmark it both against linear transformer decoding and against a direct
   CPU Wasm interpreter so the constant-factor story stays honest
4. add runtime-visible decode-path identity and a first hull-cache
   implementation behind strict equivalence checks
5. only later connect that lane to hybrid planner-plus-executor routing and
   broader model augmentation

The wrong path is:

- trying to retrofit current GPT-OSS or Apple FM lanes into this regime
- claiming that a generic chat model now "computes internally" because Psionic
  added another attention optimization
- claiming this replaces ordinary CPU execution rather than augmenting model
  internals
- inventing a private VM target before Psionic has a truthful WebAssembly-first
  lane
- treating direct program-to-weight compilation as the first milestone

## What The Article Actually Contributes

The article is not just "attention got faster."

The durable ideas are:

- append-only computation traces
  - the model executes by generating a trace whose tokens encode machine state
- exact or near-exact lookup-style attention
  - hard-max attention for exact lookup, with k-sparse softmax as a later
    approximation path
- 2D head geometry as a systems primitive
  - not because 2D is magic in itself, but because it makes the retrieval path
    reducible to a computational-geometry query
- a specialized cache/data structure
  - the "HullKVCache" style idea changes the dominant lookup path from linear
    history scans to logarithmic retrieval in the structured executor regime
- explicit program execution inside the model
  - C -> WebAssembly-style program -> interpreter-like execution trace
- proof-oriented construction
  - the author says the demonstrated weights are handcrafted and carry a
    correctness argument tied to the WebAssembly spec
- hybrid future architecture
  - slower general reasoning model plus a faster exact-computation executor
- model augmentation, not computer replacement
  - the point is to give LLMs inner computational ability, not displace a
    direct CPU interpreter
- asymptotic rather than absolute speed parity
  - direct CPU execution is still orders faster, so the interesting win is
    improved scaling inside transformer inference rather than beating a native
    interpreter
- a learnable follow-on direction
  - learned or grown internal circuits are a later step, with SUBLEQ-style
    precedents explicitly in view

That is a narrower and more implementable claim than:

- "all transformers should use 2D heads now"
- "all attention can be made logarithmic with no tradeoff"
- "general LLM serving is solved by this one trick"

The honest Psionic adaptation must preserve that narrowness.

It also needs to preserve the clarified posture:

- WebAssembly-first, not bespoke-ISA-first
- inner-computation augmentation, not "replace computers"
- proof and exactness before performance marketing
- learned-circuit ambitions later, not as the bootstrap story

## Why This Fits Psionic Better Than Most Repo Surfaces

This idea lands unusually cleanly inside Psionic because Psionic already owns
the exact class of truths the article depends on:

- explicit model geometry and runtime surfaces
- explicit compiler/runtime boundaries
- explicit cache and residency surfaces
- explicit proof bundles and manifest lineage
- explicit validation and parity language
- explicit refusal instead of silent capability drift

This does **not** belong in:

- `apps/autopilot-desktop`
  - product UX and orchestration, not framework-core execution
- kernel or Nexus authority
  - accepted outcome and settlement truth, not model runtime internals

If OpenAgents pursues this seriously, Psionic is where it should live.

## The Clean Psionic Reading

The cleanest way to think about the article in Psionic terms is:

| Article concept | Psionic owner | Honest first implementation |
| --- | --- | --- |
| 2D-head executor transformer | `psionic-models` | new executor-class model descriptor, not a hidden GPT-OSS variant |
| append-only execution trace | `psionic-models`, `psionic-runtime` | explicit trace token ABI plus digest-bound trace artifacts |
| hard-max or sparse lookup attention | `psionic-core`, `psionic-ir`, `psionic-runtime` | typed extension/capability path, CPU-first reference semantics |
| hull-based fast retrieval cache | `psionic-runtime` | optional decode-path with explicit cache algorithm id and proof surface |
| C/Wasm-to-executor program pipeline | new Psionic-adjacent program artifact layer, then `psionic-models` and `psionic-runtime` | start WebAssembly-first as digest-bound artifacts and fixtures before direct weight compilation |
| exactness benchmarks | `psionic-environments`, `psionic-eval` | Sudoku, arithmetic, Hungarian, and micro-Wasm benchmark packages |
| executor serving | `psionic-serve` | dedicated executor surface if needed, but primarily as substrate for planner/model augmentation rather than standalone "replacement compute" marketing |
| planner-executor hybrid routing | `psionic-router` plus product/controller above Psionic | later hybrid lane after WebAssembly-first executor truth exists |

This is the key conclusion:

> the first honest Psionic target is not "faster ordinary serving." It is "a
> truthful WebAssembly-first executor model family with its own runtime
> contracts."

## What Psionic Already Has That Helps

Psionic is not starting from zero.

### 1. Model geometry and fixture lanes already exist

`psionic-models` already owns:

- explicit `DecoderAttentionConfig`
  - including `head_dim`
- explicit runtime and serving-surface identity
- `WeightFormat::ProgrammaticFixture`
  - which is unusually useful here

That means Psionic can start with:

- tiny, exact, fixture-backed executor models
- handcrafted or programmatically constructed weights whose behavior is narrow
  enough to prove against a spec
- without waiting for a full training pipeline

That is the correct first milestone.

### 2. Backend extension seams already exist

`psionic-core` already has typed backend-extension seams for:

- RMS norm
- layer norm
- rotary embedding
- scaled dot-product attention
- quantized matmul

That means Psionic already has the right pattern for:

- introducing executor-specific attention or lookup behavior explicitly
- instead of hiding it inside one backend or one model lane

### 3. Runtime cache identity already matters in Psionic

`psionic-runtime` already treats cache and residency policy as explicit runtime
truth:

- KV cache
- prefix cache
- execution-plan cache
- admission and residency policy

So a hull-style cache belongs in Psionic conceptually.

The problem is not the owner split.

The problem is that the current cache vocabulary is still oriented around
ordinary decoder serving, not execution traces with geometric lookup
structures.

### 4. Proof bundles and manifests already exist

`psionic-runtime` already owns:

- runtime manifests
- execution proof bundles
- backend/toolchain identity
- validation references

That is exactly what this lane needs for:

- model geometry identity
- decode-path identity
- program artifact lineage
- trace-ABI lineage
- exactness and parity claims

### 5. Eval and benchmark package layers already exist

Psionic already has:

- `psionic-environments`
- `psionic-eval`
- benchmark package and environment package thinking across the subtree

So the right way to validate this lane is not ad hoc demo scripts.

It is:

- environment packages
- benchmark packages
- repeat-run aggregation
- explicit correctness and throughput contracts

## The Biggest Current Gaps

The article still conflicts with current Psionic truth in important ways.

The author clarifications tighten two requirements this audit should state
plainly:

- Psionic should target WebAssembly semantics first, because that is where the
  published correctness story actually attaches.
- Psionic should benchmark against direct CPU execution honestly, because the
  value claim is asymptotic behavior inside transformer inference, not native
  CPU replacement.

Several of the gaps below already have partial substrate coverage in the live
issue tracker.

The important distinction is:

- some open issues are real prerequisites
- some only partially overlap
- a smaller remainder of later executor-lane requirements still do **not**
  appear to have dedicated open issues yet

### Issue Coverage Already In Flight

As of March 15, 2026, the closest matching issue dependencies are:

- direct executor-lane tracking:
  - [#3743](https://github.com/OpenAgentsInc/openagents/issues/3743) (closed)
  - [#3744](https://github.com/OpenAgentsInc/openagents/issues/3744) (closed)
  - [#3745](https://github.com/OpenAgentsInc/openagents/issues/3745)
  - [#3746](https://github.com/OpenAgentsInc/openagents/issues/3746)
  - [#3747](https://github.com/OpenAgentsInc/openagents/issues/3747)
  - [#3748](https://github.com/OpenAgentsInc/openagents/issues/3748)
  - [#3749](https://github.com/OpenAgentsInc/openagents/issues/3749)
- framework-core and semantics umbrella:
  - [#3741](https://github.com/OpenAgentsInc/openagents/issues/3741)
  - [#3742](https://github.com/OpenAgentsInc/openagents/issues/3742)
- executor attention or extension substrate:
  - [#3713](https://github.com/OpenAgentsInc/openagents/issues/3713)
  - [#3732](https://github.com/OpenAgentsInc/openagents/issues/3732)
  - [#3735](https://github.com/OpenAgentsInc/openagents/issues/3735)
- export, graph, and deployment artifact substrate:
  - [#3714](https://github.com/OpenAgentsInc/openagents/issues/3714)
  - [#3736](https://github.com/OpenAgentsInc/openagents/issues/3736)
- replay, refusal, and compatibility harness substrate:
  - [#3710](https://github.com/OpenAgentsInc/openagents/issues/3710)
  - [#3709](https://github.com/OpenAgentsInc/openagents/issues/3709)
  - [#3720](https://github.com/OpenAgentsInc/openagents/issues/3720)
  - [#3723](https://github.com/OpenAgentsInc/openagents/issues/3723)
- cache, compile identity, and execution-plan substrate:
  - [#3707](https://github.com/OpenAgentsInc/openagents/issues/3707)

No currently-open issue reviewed here appears to directly cover:

- a dedicated `psionic.executor_trace` served surface

### 1. Psionic does not yet model executor-class attention semantics

Current `BackendExtensionOp` supports:

- `RmsNorm`
- `LayerNorm`
- `RotaryEmbedding`
- `ScaledDotProductAttention`
- `QuantizedMatmul`

That is not enough for the article's regime.

Psionic does not yet have explicit contracts for:

- hard-max lookup attention
- top-k sparse executor attention
- head-geometry-constrained lookup semantics
- trace-index lookup heads
- cumulative-sum or trace-accumulator style heads

The current IR can describe ordinary decoder families much better than it can
describe this executor lane.

Closest existing prerequisite issues:

- landed executor-lane foundation under
  [#3744](https://github.com/OpenAgentsInc/openagents/issues/3744)
- active contract and runtime tracking under
  [#3745](https://github.com/OpenAgentsInc/openagents/issues/3745) and
  [#3748](https://github.com/OpenAgentsInc/openagents/issues/3748)
- [#3713](https://github.com/OpenAgentsInc/openagents/issues/3713)
- [#3732](https://github.com/OpenAgentsInc/openagents/issues/3732)
- [#3735](https://github.com/OpenAgentsInc/openagents/issues/3735)
- umbrella tracking under [#3741](https://github.com/OpenAgentsInc/openagents/issues/3741)
  and [#3742](https://github.com/OpenAgentsInc/openagents/issues/3742)

### 2. Psionic model descriptors are still ordinary model-family descriptors

`psionic-models` can express:

- decoder width
- head count
- head dimension
- weight formats
- tokenizers

But this lane needs more explicit model-family truth:

- executor trace ABI version
- supported WebAssembly subset or profile
- supported opcode vocabulary digest
- whether the model is an executor or an ordinary decoder
- whether the runtime path is standard KV or hull-cache-eligible
- whether attention is hard-max, sparse softmax, or standard softmax
- whether exact-step execution is part of the contract

Closest existing prerequisite issues:

- landed executor-lane foundation under
  [#3744](https://github.com/OpenAgentsInc/openagents/issues/3744)
- direct implementation tracking under
  [#3745](https://github.com/OpenAgentsInc/openagents/issues/3745)
- [#3714](https://github.com/OpenAgentsInc/openagents/issues/3714)
- [#3736](https://github.com/OpenAgentsInc/openagents/issues/3736)
- umbrella tracking under [#3742](https://github.com/OpenAgentsInc/openagents/issues/3742)

This gap is now directly tracked rather than only implied by umbrella issues.

### 3. Psionic runtime does not yet have a hull-style decode path

Current runtime surfaces already reason about:

- KV cache lifecycle
- prefix reuse
- residency
- plan caches

But they do not yet reason about:

- computational-geometry retrieval structures
- head-dimension-constrained decode modes
- exact equivalence between linear-scan and hull-scan execution traces
- trace-step lookup correctness as a runtime contract

That is a real runtime gap, not just a performance gap.

Closest existing prerequisite issues:

- landed executor-lane foundation under
  [#3744](https://github.com/OpenAgentsInc/openagents/issues/3744)
- direct implementation tracking under
  [#3748](https://github.com/OpenAgentsInc/openagents/issues/3748) and
  [#3749](https://github.com/OpenAgentsInc/openagents/issues/3749)
- [#3707](https://github.com/OpenAgentsInc/openagents/issues/3707)
- [#3710](https://github.com/OpenAgentsInc/openagents/issues/3710)
- umbrella tracking under [#3741](https://github.com/OpenAgentsInc/openagents/issues/3741)
  and [#3742](https://github.com/OpenAgentsInc/openagents/issues/3742)

This runtime gap now has direct issue coverage rather than only substrate
adjacency.

### 4. Psionic serve is built around ordinary served products

Current served products are things like:

- `psionic.embeddings`
- `psionic.text_generation`

Those are good current products.

But an executor lane is not just "text generation with weird tokens."

It needs:

- trace-step streaming
- halt/output semantics
- program artifact binding
- exactness-oriented refusal surfaces

So it should be introduced as a distinct served product family once it is real,
not smuggled into the existing generic text-generation surface first.

No dedicated open issue reviewed here appears to cover this served-surface
split yet.

### 5. Psionic parity and validation language is not yet executor-oriented

Current validation and parity work assumes familiar comparison surfaces:

- logits parity
- embeddings parity
- seeded or unseeded generation parity

This lane needs different green bars:

- exact trace equivalence
- exact halt-state equivalence
- exact output equivalence
- exact cache-algorithm equivalence against a reference decoder
- exact equivalence against a direct CPU WebAssembly reference runner on the
  supported subset
- benchmark-package correctness over long horizons

Closest existing prerequisite issues:

- landed executor-lane foundation under
  [#3744](https://github.com/OpenAgentsInc/openagents/issues/3744)
- direct implementation tracking under
  [#3746](https://github.com/OpenAgentsInc/openagents/issues/3746) and
  [#3747](https://github.com/OpenAgentsInc/openagents/issues/3747)
- [#3709](https://github.com/OpenAgentsInc/openagents/issues/3709)
- [#3710](https://github.com/OpenAgentsInc/openagents/issues/3710)
- [#3720](https://github.com/OpenAgentsInc/openagents/issues/3720)
- [#3723](https://github.com/OpenAgentsInc/openagents/issues/3723)
- umbrella tracking under [#3742](https://github.com/OpenAgentsInc/openagents/issues/3742)

The benchmark and proof pieces of this gap are now directly tracked.

### 6. Psionic train is not yet ready for the article's training story

The article also hints at:

- training large 2D-head models
- hybrid executors plus general models
- direct compilation of programs into weights
- learned or grown internal circuits

Psionic should not pretend the current train subtree is already ready to carry
that whole program.

The current training truth is real, but still much narrower:

- fixed-budget core loop
- Apple adapter lane
- early distributed contracts

So this lane should start with:

- fixtures
- program artifacts
- runtime exactness
- eval packages

before trying to sell a full training or compile-to-weights story.

### 7. This is not current MVP compute-market scope

The MVP and current ownership rules are explicit.

This work belongs in the library and research program, not in the current
desktop/product cut.

That does not make it unimportant.

It just means:

- do not turn this into product-scope drift
- do not route active provider-MVP work through it

## What OpenAgents Should Adapt

These are the right things to take from the article.

### 1. A new executor-class model family

The first important adaptation is conceptual:

> exact long-horizon computation should become its own Psionic model/runtime
> family, not just a tool-use fallback.

That means a dedicated family with explicit:

- geometry constraints
- trace ABI
- cache mode
- program binding
- exactness claims

It should also be framed explicitly as:

- a way to give larger models internal exact-computation capacity
- not a claim that Psionic will replace ordinary computers or native runtimes

### 2. CPU-first exactness as the reference path

The article's most useful systems claim is not GPU flashiness.

It is:

- exact long trace execution
- with much better scaling than standard autoregressive decoding
- on CPU

That aligns with Psionic's own rule that CPU is the canonical reference lane.

So the first honest implementation target is:

- CPU-only
- exact
- benchmarked
- proof-bearing

But Psionic should also state this plainly:

- the executor lane will still be slower than direct CPU execution for the same
  program
- the relevant claim is better asymptotic behavior than traditional
  autoregressive decoding on long execution traces
- every benchmark report should show both native Wasm CPU baseline and
  transformer-executor throughput

### 3. Append-only execution traces as first-class runtime artifacts

The trace itself should become a real Psionic artifact family.

That means:

- step stream
- output events
- halt events
- program digest
- trace digest
- decode-path digest
- cache algorithm id

This should integrate naturally with runtime manifests and proof bundles.

For this lane, those artifacts should be anchored to WebAssembly-compatible
program identity first.

### 4. WebAssembly-first program targeting

The author's clarification materially changes the target recommendation.

Psionic should not invent a private toy VM as its primary substrate.

It should start with:

- a narrow, explicit WebAssembly subset or profile
- spec-compatible program artifacts
- a direct CPU reference interpreter or runtime for parity
- trace vocab and proof surfaces that name the supported Wasm profile

### 5. Two-bucket execution state

The article's trace model strongly suggests a two-bucket posture:

- immutable program and prior trace stay in artifact/cache space
- the current decoding step consumes only the bounded lookup state it needs

Psionic should adapt that as:

- trace history as a runtime-managed artifact/cache object
- bounded decode-step state as explicit runtime context
- explicit accounting for when the fast path is active

### 6. Exactness-first benchmark packages

Psionic should not treat demos as proof.

The lane needs benchmark packages for:

- exact arithmetic
- stack and memory microprograms
- control-flow microprograms
- micro-Wasm programs with spec-reference outputs
- Sudoku
- Hungarian/min-cost matching
- longer trace workloads that stress the fast path

Those benchmark packages should always include:

- direct CPU WebAssembly runtime throughput
- transformer linear-reference throughput
- hull-cache throughput where validated
- exactness deltas across all three

### 7. Hybrid planner-plus-executor routing later

The article's hybrid vision is good, but it comes later.

Psionic should eventually support:

- a general reasoning model
- an exact executor model
- router/runtime truth about when one handed work to the other

But that is later than the executor lane itself.

That later hybrid is also the right product framing:

- planner and reasoner models gain inner computational ability
- Psionic does not market this as "the model is now your new CPU"

## What OpenAgents Should Not Copy

These are the wrong moves.

### 1. Do not retrofit this into current general serving lanes first

Do not start by changing:

- GPT-OSS
- Apple FM
- ordinary GGUF decoder serving

Those lanes have different semantics, different model families, and different
validation bars.

### 2. Do not treat "2D heads" as a repo-wide architecture doctrine

The article is about a tractable executor regime.

That does **not** imply:

- every future Psionic model should use 2D heads
- ordinary serving should move to 2D heads by default

This should remain a model-family-specific contract until proven otherwise.

### 3. Do not hide a specialized decoder behind ordinary capability labels

If the lane exists, it should report:

- what it is
- what it is not
- when it can refuse
- when the fast path is active

It should not masquerade as plain text generation.

### 4. Do not start with direct program-to-weight compilation

That is a late-stage ambition.

The first honest milestone is:

- explicit program artifacts
- explicit runtime execution
- explicit trace exactness

Only after that is credible should Psionic pursue:

- compiling logic directly into weights
- or training large executor families
- or learned-circuit growth beyond the initial handcrafted and proved regime

### 5. Do not skip equivalence and proof work

The whole point of this lane is exact computation.

So Psionic must require:

- linear reference execution
- hull-cache equivalence
- exact output equivalence
- exact halting behavior

before it claims the fast path is real.

### 6. Do not skip the native CPU baseline

The author explicitly says direct CPU execution is still orders faster.

So Psionic should not publish performance claims that compare only:

- standard transformer decode
- versus hull-cache transformer decode

It also needs:

- direct CPU WebAssembly execution as the honest baseline
- explicit overhead ratios
- clear language that the asymptotic win is inside the transformer regime

## Recommended Psionic Architecture

The best architecture is a new executor lane layered onto existing Psionic
owners.

### Layer 1: model-family contracts

Primary owner:

- `psionic-models`

Needed additions:

- executor model descriptor
- trace vocabulary metadata
- execution-trace ABI version
- WebAssembly profile compatibility declaration
- attention geometry contract
- decode-mode capability declaration
- program artifact compatibility contract

Likely examples:

- `ExecutorModelDescriptor`
- `ExecutionTraceAbi`
- `WebAssemblyProfileCompatibility`
- `AttentionGeometryContract`
- `ExecutorDecodeMode`

Current helpful substrate already present:

- `WeightFormat::ProgrammaticFixture`
- tokenizer boundaries
- `DecoderAttentionConfig`

### Layer 2: graph and runtime semantics

Primary owners:

- `psionic-core`
- `psionic-ir`
- `psionic-compiler`
- `psionic-runtime`

Needed additions:

- executor-specific attention semantics
- explicit fast-path capability gates
- WebAssembly-first reference execution path
- linear reference decoder
- hull-cache decoder
- exact-step parity harnesses
- runtime-visible decode-path identity

The important rule here is:

> do not make the first version depend on custom accelerator kernels.

First make the CPU reference honest.

### Layer 3: proof, manifest, and validation

Primary owner:

- `psionic-runtime`

Needed additions:

- trace proof artifact family
- program artifact lineage fields
- WebAssembly profile and reference-runner identity fields
- decode-path identity fields
- cache algorithm identity fields
- executor-specific validation references

This is where Psionic's existing proof-bundle and validation-matrix discipline
should help the most.

### Layer 4: environment, eval, and research

Primary owners:

- `psionic-environments`
- `psionic-eval`
- `psionic-research`

Needed additions:

- executor benchmark packages
- executor environment packages
- exactness metrics
- throughput metrics
- native CPU baseline metrics
- experiment families for decode path and model-architecture variants

This should be benchmark-package-first, not demo-first.

### Layer 5: served executor product and hybrid routing

Primary owners:

- `psionic-serve`
- `psionic-router`

Later additions:

- `psionic.executor_trace` or equivalent served product family
- trace streaming contract
- planner-to-executor routing truth

This is later because a served product should sit on top of a truthful
runtime, not serve as the runtime's prototype.

The primary strategic value is still likely:

- an internal compute substrate for richer planner models
- with a standalone served surface only where that is operationally useful

## Concrete Implementation Path

The path below is intentionally dependency-ordered.

### Phase 0: Declare the lane and keep it out of current MVP scope

Status:

- completed under
  [#3743](https://github.com/OpenAgentsInc/openagents/issues/3743)

Goals:

- name the lane honestly
- keep it below product scope
- avoid contaminating current provider work

Concretely:

- treat this as a Psionic library and research program
- do not widen current compute-market MVP around it
- do not reframe current text-generation product claims around it
- state explicitly that the goal is inner computational ability for models, not
  replacement of ordinary CPU execution

Existing tracker dependency posture:

- this lane is now directly tracked by
  [#3743](https://github.com/OpenAgentsInc/openagents/issues/3743)
- the implementation spine now begins with
  [#3744](https://github.com/OpenAgentsInc/openagents/issues/3744)
- the next active implementation spine is
  [#3745](https://github.com/OpenAgentsInc/openagents/issues/3745) through
  [#3749](https://github.com/OpenAgentsInc/openagents/issues/3749)
- the lane still sits under the umbrella planning already represented by
  [#3741](https://github.com/OpenAgentsInc/openagents/issues/3741) and
  [#3742](https://github.com/OpenAgentsInc/openagents/issues/3742)

### Phase 1: Land a CPU reference executor fixture

Status:

- completed under
  [#3744](https://github.com/OpenAgentsInc/openagents/issues/3744)
- landed as the `Tassadar` reference substrate in:
  - `psionic-runtime::tassadar`
  - `psionic-models::TassadarExecutorFixture`

Goals:

- prove the lane can exist in Psionic at all
- do it without a training dependency

Concretely:

- add one tiny executor-class fixture model using `WeightFormat::ProgrammaticFixture`
- define one minimal WebAssembly-first trace ABI:
  - instruction tokens
  - commit events
  - output events
  - halt events
- support a narrow Wasm subset with tiny reference programs:
  - local or stack push/pop
  - add/sub/mul
  - simple branching
  - simple load/store or lookup
- keep the construction and proof posture explicit:
  - handcrafted or programmatically constructed weights
  - spec-locked to the supported Wasm subset

Success bar:

- exact trace correctness on CPU
- exact final output correctness
- exact equivalence with a direct CPU Wasm reference runner on the supported
  subset
- deterministic replay

Closest existing prerequisite issues:

- landed direct implementation under
  [#3744](https://github.com/OpenAgentsInc/openagents/issues/3744)
- [#3709](https://github.com/OpenAgentsInc/openagents/issues/3709) for
  fixture-backed replay and failure coverage
- [#3710](https://github.com/OpenAgentsInc/openagents/issues/3710) for typed
  refusal and unsupported-mode surfaces

This phase is the most important one because it proves the owner split and
artifact boundaries before any ambitious optimization work.

### Phase 2: Add executor model and program artifacts

Goals:

- make the lane machine-legible

Concretely:

- define digest-bound program artifacts for:
  - source identity
  - compiler/toolchain identity
  - emitted WebAssembly bytecode or validated executor program
  - supported Wasm profile or subset id
  - opcode vocabulary digest
- define executor model descriptors with:
  - attention geometry contract
  - trace ABI contract
  - Wasm profile compatibility
  - supported decode modes
  - exactness posture

This is where the lane stops being "some fixture hack" and becomes a typed
Psionic artifact path.

Closest existing prerequisite issues:

- direct implementation tracking under
  [#3745](https://github.com/OpenAgentsInc/openagents/issues/3745)
- [#3714](https://github.com/OpenAgentsInc/openagents/issues/3714)
- [#3736](https://github.com/OpenAgentsInc/openagents/issues/3736)

### Phase 3: Add benchmark and environment packages

Goals:

- replace demos with repeatable eval truth

Concretely:

- add benchmark packages for:
  - arithmetic
  - memory-lookup microprograms
  - branch/control-flow microprograms
  - micro-Wasm kernels
  - Sudoku
  - Hungarian/min-cost matching
- add environment packages that bind:
  - program artifact
  - expected input/output contract
  - step correctness rubric
  - timeout and trace-budget policy

Metrics should include:

- final output exactness
- step exactness
- halt correctness
- native CPU reference throughput
- tokens or trace steps per second
- trace artifact completeness

Closest existing prerequisite issues:

- direct implementation tracking under
  [#3746](https://github.com/OpenAgentsInc/openagents/issues/3746)
- [#3709](https://github.com/OpenAgentsInc/openagents/issues/3709)
- [#3720](https://github.com/OpenAgentsInc/openagents/issues/3720)
- [#3723](https://github.com/OpenAgentsInc/openagents/issues/3723)

### Phase 4: Add runtime proof bundles for executor traces

Goals:

- make execution explainable and challengeable

Concretely:

- add executor-trace proof artifacts carrying:
  - trace digest
  - program digest
  - Wasm profile id
  - model descriptor digest
  - decode mode
  - cache algorithm id
  - runtime backend
  - reference-runner identity
  - validation reference
- add manifest lineage from:
  - source program
  - compile toolchain
  - program artifact
  - executed model
  - emitted trace

This should extend existing proof-bundle discipline, not replace it.

Direct implementation tracking:

- [#3747](https://github.com/OpenAgentsInc/openagents/issues/3747)

### Phase 5: Implement the hull-cache fast path behind exact equivalence

Goals:

- get the article's core systems win honestly

Concretely:

- implement a CPU hull-style cache or geometric retrieval structure for the
  executor lane
- keep the linear reference decoder as the oracle
- keep a direct CPU Wasm runner as the non-transformer baseline
- add exact equivalence checks between:
  - direct CPU reference execution
  - reference linear decode
  - fast-path decode

Success bar:

- identical trace digests on supported workloads
- explicit refusal when a workload/model/cache mode is outside the validated
  regime
- real throughput win over linear transformer decode on long trace workloads
- explicit measurement of remaining overhead versus direct CPU execution

This is where the article's main technical claim should be tested.

Closest existing prerequisite issues:

- direct implementation tracking under
  [#3748](https://github.com/OpenAgentsInc/openagents/issues/3748)
- [#3707](https://github.com/OpenAgentsInc/openagents/issues/3707) for cache
  identity and compile-cache evidence
- [#3710](https://github.com/OpenAgentsInc/openagents/issues/3710) for honest
  fallback and refusal semantics

### Phase 6: Add typed runtime capabilities instead of silent fast-path fallback

Goals:

- make the fast path inspectable

Concretely:

- expose runtime capability fields such as:
  - `supports_executor_trace`
  - `supports_hull_decode`
  - `supported_wasm_profiles`
  - `supported_attention_modes`
  - `validated_trace_abi_versions`
- emit runtime diagnostics that say when Psionic fell back from:
  - hull cache to linear reference
  - sparse attention to exact lookup

This keeps Psionic's refusal discipline intact.

Closest existing prerequisite issues:

- direct implementation tracking under
  [#3749](https://github.com/OpenAgentsInc/openagents/issues/3749)
- [#3710](https://github.com/OpenAgentsInc/openagents/issues/3710)

### Phase 7: Add a dedicated served executor product

Goals:

- make the lane consumable without pretending it is ordinary chat

Concretely:

- introduce a dedicated served product family such as:
  - `psionic.executor_trace`
- support:
  - program submission
  - trace streaming
  - structured output events
  - final output extraction
- keep product semantics explicit:
  - this is executor streaming
  - not ordinary chat completion
  - and primarily useful as a computation substrate for broader reasoning
    systems

### Phase 8: Add research families for architecture and cache experiments

Goals:

- move beyond manual iteration

Concretely:

- extend `psionic-research` or sibling research surfaces to cover:
  - executor model architecture variants
  - trace ABI variants
  - WebAssembly subset or profile variants
  - decode cache variants
  - attention mode variants
- treat benchmark packages and trace proofs as the evaluator backend

This should happen only after the substrate is typed and benchmarked.

### Phase 9: Pursue training and compile-to-weights later

Goals:

- explore the article's long-term vision without pretending it is current repo
  truth

Concretely:

- later use `psionic-train` once framework semantics broaden enough to train
  small executor models honestly
- only then explore:
  - larger 2D-head executor models
  - direct program-to-weight compilation
  - hybrid learned-plus-compiled executor systems
  - learned-circuit growth, including minimal-instruction or SUBLEQ-like
    research lines where useful

This is late because it depends on:

- honest model-family contracts
- honest runtime exactness
- honest benchmark packages

## Specific Psionic Changes That Would Likely Be Needed

These are the likely concrete contract additions.

### 1. `psionic-models`

Probable additions:

- `ExecutorModelDescriptor`
- `ExecutionTraceAbi`
- `AttentionGeometryContract`
- `ExecutorProgramCompatibility`

Likely new claims:

- exact trace vocab
- supported Wasm subset or profile
- decode mode compatibility
- head-dimension restrictions
- supported attention semantics

### 2. `psionic-core` and `psionic-ir`

Probable additions:

- new backend-extension families or an executor-specific extension seam for:
  - hard-max lookup attention
  - sparse-top-k executor attention
  - executor trace lookup or reduction semantics

Important caution:

- do not bloat the visible primitive surface until at least one full executor
  lane exists
- start with typed backend extensions or family-scoped ops

### 3. `psionic-runtime`

Probable additions:

- hull-cache or equivalent geometric cache data structure
- executor trace runtime reports
- cache algorithm identity
- Wasm reference-runner identity and parity surfaces
- exact-trace parity helpers
- executor trace proof artifacts

### 4. `psionic-serve`

Probable additions:

- executor request/response types
- trace-event streaming
- program and trace headers
- refusal surfaces for unsupported execution modes

### 5. `psionic-eval` and `psionic-environments`

Probable additions:

- executor environment package family
- executor benchmark packages
- exactness and throughput scoring contracts
- required direct CPU reference baselines

### 6. `psionic-research`

Probable additions later:

- architecture or decode-cache experiment families
- promotion criteria based on exactness plus throughput
- later learned-circuit experiment families once the Wasm-first lane is stable

## Recommended Priority Order

If OpenAgents wants to pursue this article's ideas without destabilizing the
current tree, the order should be:

1. CPU executor fixture plus minimal WebAssembly-first trace ABI
   - landed under [#3744](https://github.com/OpenAgentsInc/openagents/issues/3744)
2. executor model and Wasm program artifact contracts
   - tracked under [#3745](https://github.com/OpenAgentsInc/openagents/issues/3745)
3. environment and benchmark packages with direct CPU baselines
   - tracked under [#3746](https://github.com/OpenAgentsInc/openagents/issues/3746)
4. executor proof bundles and trace digests
   - tracked under [#3747](https://github.com/OpenAgentsInc/openagents/issues/3747)
5. hull-cache fast path with exact equivalence checks
   - tracked under [#3748](https://github.com/OpenAgentsInc/openagents/issues/3748)
6. runtime capabilities and fallback diagnostics
   - tracked under [#3749](https://github.com/OpenAgentsInc/openagents/issues/3749)
7. research families for architecture and cache variants
8. dedicated served executor product where useful
9. hybrid planner-plus-executor routing
10. larger training program, compile-to-weights exploration, and learned-circuit
   follow-ons

Do not start with:

- GPT-OSS retrofits
- generic serving integration
- accelerator kernel work
- bespoke VM invention ahead of WebAssembly-first parity
- direct program-to-weight compilation
- product-scope MVP integration

## Bottom Line

The Percepta article points at a real and interesting direction for Psionic,
but only if OpenAgents reads it correctly.

The correct reading is not:

> Psionic should turn every LLM into a replacement for ordinary computers.

The correct reading is:

> Psionic should grow a new WebAssembly-first executor-class model/runtime lane
> for exact long-horizon computation, with explicit trace ABI, explicit
> decode-path identity, explicit benchmark packages, and explicit proof
> bundles, so larger models can gain inner computational ability.

That lane should begin as:

- CPU-first
- fixture-backed
- benchmarked
- proof-bearing
- outside current MVP product scope

It should also stay honest about performance:

- native CPU execution remains the faster baseline
- the meaningful systems claim is improved scaling inside transformer decoding
- benchmark reports should always show that distinction directly

Only after that is real should OpenAgents pursue:

- hull-cache acceleration
- hybrid planner-plus-executor routing
- standalone executor serving where it is actually useful
- and eventually direct program-to-weight compilation
- and later learned-circuit growth

If handled that way, this article is a strong strategic input for Psionic.

If handled the wrong way, it would just become another ambiguous "faster
transformer" aspiration without truthful model-family, runtime, or validation
boundaries.
