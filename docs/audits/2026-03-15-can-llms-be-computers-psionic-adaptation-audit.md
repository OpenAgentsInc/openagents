# 2026-03-15 "Can LLMs Be Computers?" Psionic Adaptation Audit

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
- `crates/psionic/psionic-models/src/lib.rs`
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

## Executive Summary

The Percepta article is a strong fit for Psionic, but only if OpenAgents reads
it as a new execution lane rather than as a drop-in improvement to current
general LLM serving.

The most important architectural reading is:

> Psionic should treat these ideas as a new executor-class model family with a
> specialized trace ABI, decoding path, cache algorithm, and validation stack.

That means:

- this belongs in `crates/psionic/*`, not in app code and not in kernel
  authority
- it is framework-core, model-runtime, cache, proof, and eval work first
- it is not current MVP product scope
- it should start on CPU as a reference lane
- it should not be bolted into `psionic.text_generation` as an invisible
  alternate kernel path for ordinary decoder models

The article really proposes four distinct things:

1. a restricted transformer regime
   - especially 2D head geometry
2. an append-only execution-trace ABI
   - interpreter state encoded as generated tokens
3. a specialized fast decoding path
   - hard-max or sparse retrieval over a hull-style cache
4. a longer-term compiler path
   - source program -> interpreter/runtime artifact -> possibly directly into
     weights

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

1. define a Psionic-native executor trace ABI and benchmark package family
2. land a CPU-only reference executor model using programmatic fixture weights
   before training claims
3. add runtime-visible decode-path identity and a first hull-cache
   implementation behind strict equivalence checks
4. add a dedicated served executor product only after the trace and proof
   surfaces are honest
5. only later connect that lane to hybrid planner-plus-executor routing

The wrong path is:

- trying to retrofit current GPT-OSS or Apple FM lanes into this regime
- claiming that a generic chat model now "computes internally" because Psionic
  added another attention optimization
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
- hybrid future architecture
  - slower general reasoning model plus a faster exact-computation executor

That is a narrower and more implementable claim than:

- "all transformers should use 2D heads now"
- "all attention can be made logarithmic with no tradeoff"
- "general LLM serving is solved by this one trick"

The honest Psionic adaptation must preserve that narrowness.

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
| C/Wasm-to-executor program pipeline | new Psionic-adjacent program artifact layer, then `psionic-models` and `psionic-runtime` | start as digest-bound artifacts and fixtures before direct weight compilation |
| exactness benchmarks | `psionic-environments`, `psionic-eval` | Sudoku, arithmetic, Hungarian, and micro-Wasm benchmark packages |
| executor serving | `psionic-serve` | dedicated served product surface, not folded into ordinary chat |
| planner-executor hybrid routing | `psionic-router` plus product/controller above Psionic | later hybrid lane after executor truth exists |

This is the key conclusion:

> the first honest Psionic target is not "faster ordinary serving." It is "a
> truthful executor model family with its own runtime contracts."

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

### 2. Psionic model descriptors are still ordinary model-family descriptors

`psionic-models` can express:

- decoder width
- head count
- head dimension
- weight formats
- tokenizers

But this lane needs more explicit model-family truth:

- executor trace ABI version
- supported opcode vocabulary digest
- whether the model is an executor or an ordinary decoder
- whether the runtime path is standard KV or hull-cache-eligible
- whether attention is hard-max, sparse softmax, or standard softmax
- whether exact-step execution is part of the contract

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
- benchmark-package correctness over long horizons

### 6. Psionic train is not yet ready for the article's training story

The article also hints at:

- training large 2D-head models
- hybrid executors plus general models
- direct compilation of programs into weights

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

### 2. CPU-first exactness as the reference path

The article's most useful systems claim is not GPU flashiness.

It is:

- exact long trace execution
- at high speed
- on CPU

That aligns with Psionic's own rule that CPU is the canonical reference lane.

So the first honest implementation target is:

- CPU-only
- exact
- benchmarked
- proof-bearing

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

### 4. Two-bucket execution state

The article's trace model strongly suggests a two-bucket posture:

- immutable program and prior trace stay in artifact/cache space
- the current decoding step consumes only the bounded lookup state it needs

Psionic should adapt that as:

- trace history as a runtime-managed artifact/cache object
- bounded decode-step state as explicit runtime context
- explicit accounting for when the fast path is active

### 5. Exactness-first benchmark packages

Psionic should not treat demos as proof.

The lane needs benchmark packages for:

- exact arithmetic
- stack and memory microprograms
- control-flow microprograms
- Sudoku
- Hungarian/min-cost matching
- longer trace workloads that stress the fast path

### 6. Hybrid planner-plus-executor routing later

The article's hybrid vision is good, but it comes later.

Psionic should eventually support:

- a general reasoning model
- an exact executor model
- router/runtime truth about when one handed work to the other

But that is later than the executor lane itself.

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

### 5. Do not skip equivalence and proof work

The whole point of this lane is exact computation.

So Psionic must require:

- linear reference execution
- hull-cache equivalence
- exact output equivalence
- exact halting behavior

before it claims the fast path is real.

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
- attention geometry contract
- decode-mode capability declaration
- program artifact compatibility contract

Likely examples:

- `ExecutorModelDescriptor`
- `ExecutionTraceAbi`
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

## Concrete Implementation Path

The path below is intentionally dependency-ordered.

### Phase 0: Declare the lane and keep it out of current MVP scope

Goals:

- name the lane honestly
- keep it below product scope
- avoid contaminating current provider work

Concretely:

- treat this as a Psionic library and research program
- do not widen current compute-market MVP around it
- do not reframe current text-generation product claims around it

### Phase 1: Land a CPU reference executor fixture

Goals:

- prove the lane can exist in Psionic at all
- do it without a training dependency

Concretely:

- add one tiny executor-class fixture model using `WeightFormat::ProgrammaticFixture`
- define one minimal trace ABI:
  - instruction tokens
  - commit events
  - output events
  - halt events
- support tiny reference programs:
  - stack push/pop
  - add/sub/mul
  - simple branching
  - simple memory lookup

Success bar:

- exact trace correctness on CPU
- exact final output correctness
- deterministic replay

This phase is the most important one because it proves the owner split and
artifact boundaries before any ambitious optimization work.

### Phase 2: Add executor model and program artifacts

Goals:

- make the lane machine-legible

Concretely:

- define digest-bound program artifacts for:
  - source identity
  - compiler/toolchain identity
  - emitted bytecode or executor program
  - opcode vocabulary digest
- define executor model descriptors with:
  - attention geometry contract
  - trace ABI contract
  - supported decode modes
  - exactness posture

This is where the lane stops being "some fixture hack" and becomes a typed
Psionic artifact path.

### Phase 3: Add benchmark and environment packages

Goals:

- replace demos with repeatable eval truth

Concretely:

- add benchmark packages for:
  - arithmetic
  - memory-lookup microprograms
  - branch/control-flow microprograms
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
- tokens or trace steps per second
- trace artifact completeness

### Phase 4: Add runtime proof bundles for executor traces

Goals:

- make execution explainable and challengeable

Concretely:

- add executor-trace proof artifacts carrying:
  - trace digest
  - program digest
  - model descriptor digest
  - decode mode
  - cache algorithm id
  - runtime backend
  - validation reference
- add manifest lineage from:
  - source program
  - compile toolchain
  - program artifact
  - executed model
  - emitted trace

This should extend existing proof-bundle discipline, not replace it.

### Phase 5: Implement the hull-cache fast path behind exact equivalence

Goals:

- get the article's core systems win honestly

Concretely:

- implement a CPU hull-style cache or geometric retrieval structure for the
  executor lane
- keep the linear reference decoder as the oracle
- add exact equivalence checks between:
  - reference linear decode
  - fast-path decode

Success bar:

- identical trace digests on supported workloads
- explicit refusal when a workload/model/cache mode is outside the validated
  regime
- real throughput win on long trace workloads

This is where the article's main technical claim should be tested.

### Phase 6: Add typed runtime capabilities instead of silent fast-path fallback

Goals:

- make the fast path inspectable

Concretely:

- expose runtime capability fields such as:
  - `supports_executor_trace`
  - `supports_hull_decode`
  - `supported_attention_modes`
  - `validated_trace_abi_versions`
- emit runtime diagnostics that say when Psionic fell back from:
  - hull cache to linear reference
  - sparse attention to exact lookup

This keeps Psionic's refusal discipline intact.

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

### Phase 8: Add research families for architecture and cache experiments

Goals:

- move beyond manual iteration

Concretely:

- extend `psionic-research` or sibling research surfaces to cover:
  - executor model architecture variants
  - trace ABI variants
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

### 6. `psionic-research`

Probable additions later:

- architecture or decode-cache experiment families
- promotion criteria based on exactness plus throughput

## Recommended Priority Order

If OpenAgents wants to pursue this article's ideas without destabilizing the
current tree, the order should be:

1. CPU executor fixture plus minimal trace ABI
2. executor model and program artifact contracts
3. environment and benchmark packages
4. executor proof bundles and trace digests
5. hull-cache fast path with exact equivalence checks
6. dedicated served executor product
7. research families for architecture and cache variants
8. larger training program and compile-to-weights exploration
9. hybrid planner-plus-executor routing

Do not start with:

- GPT-OSS retrofits
- generic serving integration
- accelerator kernel work
- direct program-to-weight compilation
- product-scope MVP integration

## Bottom Line

The Percepta article points at a real and interesting direction for Psionic,
but only if OpenAgents reads it correctly.

The correct reading is not:

> Psionic should turn every LLM into a computer.

The correct reading is:

> Psionic should grow a new executor-class model/runtime lane for exact
> long-horizon computation, with explicit trace ABI, explicit decode-path
> identity, explicit benchmark packages, and explicit proof bundles.

That lane should begin as:

- CPU-first
- fixture-backed
- benchmarked
- proof-bearing
- outside current MVP product scope

Only after that is real should OpenAgents pursue:

- hull-cache acceleration
- served executor products
- hybrid planner-plus-executor routing
- and eventually direct program-to-weight compilation

If handled that way, this article is a strong strategic input for Psionic.

If handled the wrong way, it would just become another ambiguous "faster
transformer" aspiration without truthful model-family, runtime, or validation
boundaries.
