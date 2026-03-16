# 2026-03-16 Tassadar Trained-Executor Gap Audit

## Intent

This audit revisits the March 15 `Can LLMs Be Computers?` adaptation work with
a narrower question:

> after all of the landed `Tassadar` substrate work, what do we actually have,
> what do we still not have, and what is the shortest honest path to training a
> model that really does Percepta-style in-model execution?

The concrete starter target in this audit is Sudoku, because that is the
clearest place where the repo can currently sound more complete than it is.

## Short Answer

No: Psionic does not yet have a trained executor model that "does the article"
in the Percepta sense.

What Psionic has today is valuable, but it is different:

- a WebAssembly-first executor artifact and trace ABI
- imperative Rust reference, fixture, hull-cache, and sparse-top-k runners
- proof, benchmark, serving, planner-routing, and research scaffolding
- a very narrow training lane that learns arithmetic kernels on a tiny
  validation corpus

What Psionic does **not** yet have:

- an autoregressive transformer that emits and continues execution traces
- tokenized program and trace sequences for that model to learn
- real 2D-head executor weights or a real executor forward pass
- hull-cache or sparse-top-k decoding over actual neural KV states
- compiled-weight executors that run without delegating back to the imperative
  runtime
- article-grade Sudoku or Hungarian workloads

The right summary is:

> `Tassadar` is currently an honest executor substrate and simulator stack, not
> yet a trained in-model executor.

## Sources Reviewed

Primary current-state sources reviewed for this audit:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/audits/2026-03-15-can-llms-be-computers-psionic-adaptation-audit.md`
- `crates/psionic/README.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/ROADMAP.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
- `crates/psionic/psionic-runtime/src/tassadar.rs`
- `crates/psionic/psionic-models/src/tassadar.rs`
- `crates/psionic/psionic-eval/src/tassadar.rs`
- `crates/psionic/psionic-train/src/tassadar.rs`
- `crates/psionic/psionic-serve/src/tassadar.rs`
- `crates/psionic/psionic-research/src/lib.rs`
- `crates/psionic/psionic-research/src/runner.rs`

Article basis:

- user-provided text of Percepta's `Can LLMs Be Computers?`
- user-provided author clarifications already captured in the March 15 audit

## Issue Tracker

This audit now doubles as the narrative tracker for the remaining
trained-executor work.

The canonical GitHub tracker is:

- `#3776` Psionic Tassadar Trained Executor Epic: move from executor substrate
  to a real Psionic-only trained model

### Existing Open Psionic / PLIB Prerequisites Reviewed

These were already open before the trained-executor gap review and are relevant
prerequisites or substrate dependencies for fully training a real executor
inside Psionic:

- `#3741` Psionic Epic 1 Master: framework core completion
- `#3742` Psionic Epic 2 Master: semantics and compatibility
- `#3709` PLIB-107 broad framework-core acceptance coverage
- `#3710` PLIB-108 refusal taxonomy
- `#3712` PLIB-110 RNG, seeding, generator-state, and deterministic-algorithm
  contracts
- `#3713` PLIB-111 custom-op schema, kernel registration, and backend dispatch
  contracts
- `#3716` PLIB-201 module, parameter, buffer, and state-tree system
- `#3718` PLIB-203 optimizer coverage with scheduler integration and stronger
  state behavior
- `#3719` PLIB-204 serialization and checkpoint compatibility boundaries
- `#3727` PLIB-212 framework-wide reproducibility semantics
- `#3733` PLIB-218 dataset, iterable-streaming, sampler, and staging
  abstractions
- `#3735` PLIB-220 advanced operator-family and attention semantics
- `#3736` PLIB-221 exportable graph and deployment artifact contracts

Not all of these block the very first 4x4 Sudoku-v0 experiment equally, but
they are the currently-open Psionic-library issues most directly relevant to
fully training, checkpointing, replaying, evaluating, and iterating on a real
executor model without leaving the Psionic stack.

### Dedicated Tassadar Trained-Executor Issue Spine

These issues track the executor-specific work that is still missing:

- `#3777` Phase 1: widen the Wasm subset for real Sudoku search
- `#3778` Phase 2: replace the placeholder Sudoku benchmark with a real 4x4
  solver corpus
- `#3779` Phase 3: add trace-token vocabulary and sequence dataset generation
- `#3780` Phase 4: implement a real executor transformer family
- `#3781` Phase 5: add next-token trace training and exact-trace evaluation
- `#3782` Phase 6: benchmark real neural linear decode against CPU reference
- `#3783` Phase 7: execute the first Psionic-only Sudoku-v0 training run
- `#3784` Phase 8: add telemetry, trace logging, and failure-analysis artifacts
- `#3785` Phase 9: review the first run and land the next-run plan
- `#3786` Phase 10: add neural hull-cache decode for the trained executor model
- `#3787` Phase 11: scale from Sudoku-v0 to real 9x9 Sudoku-class training

## The Main Correction

The March 15 audit is still directionally useful as an architecture document.

It is **not** a good current-state document if someone reads:

- "Phase 9B landed"
- "Phase 9C landed"
- "Phase 9D landed"

as meaning:

- "we trained a model that executes WebAssembly inside transformer weights"
- "we have a real Percepta-style executor model"
- "we solved Sudoku with an in-model learned or compiled executor"

That is not what the current tree shows.

The current repo docs in `README.md`, `ARCHITECTURE.md`, `ROADMAP.md`, and
`TRAIN_SYSTEM.md` all describe real landed substrate, but they currently make
it too easy to confuse:

- substrate completion

with:

- trained executor-model completion

This audit exists to separate those.

## What Tassadar Actually Is Today

### 1. The runtime is an imperative executor, not a neural executor

`crates/psionic/psionic-runtime/src/tassadar.rs` implements:

- `TassadarCpuReferenceRunner`
- `TassadarFixtureRunner`
- `TassadarHullCacheRunner`
- `TassadarSparseTopKRunner`

All four run `TassadarProgram` instructions through explicit Rust state
transitions over:

- `stack`
- `locals`
- `memory`
- `outputs`
- `pc`

The direct runner executes the program imperatively.

The linear runner reconstructs state from prior trace steps.

The hull-cache runner keeps explicit `local_last_write_step` and
`memory_last_write_step` arrays.

The sparse-top-k runner keeps explicit recent-write histories.

That is a useful simulator stack for the executor regime.

It is not a transformer forward pass.

### 2. The current "weights" are fixture metadata, not trained executor weights

`crates/psionic/psionic-models/src/tassadar.rs` defines
`TassadarExecutorFixture` and `TassadarExecutorWeightBundle`.

That bundle contains programmatic tensors for things like:

- opcode stack effects
- opcode semantics
- profile limits
- trace ABI flags

This is an explicit fixture boundary using `WeightFormat::ProgrammaticFixture`.

That is honest and useful.

It is not a learned 2D-head executor model.

### 3. The current training lane is a tiny arithmetic surrogate

`crates/psionic/psionic-train/src/tassadar.rs` does not train a transformer.

It trains three tiny arithmetic kernels:

- `add_kernel` over `[lhs, rhs]`
- `sub_kernel` over `[lhs, rhs]`
- `mul_kernel` over `[lhs * rhs]`

The supervision is extracted by `collect_supervision_examples()` only from
`BinaryOp` events in the current validation corpus.

The resulting `TassadarSmallExecutorModel` still executes:

- local reads and writes
- memory reads and writes
- control flow
- output
- return

through handwritten Rust interpreter logic.

Only the arithmetic result is learned.

That means the current Phase 9B lane is better described as:

> a bounded arithmetic-kernel learning experiment inside the executor ABI

not:

> a trained model that implements Percepta-style in-model execution

### 4. The current compiled-weight path is still metadata plus delegation

`TassadarCompiledProgramExecutor` in
`crates/psionic/psionic-models/src/tassadar.rs` looks stronger than it is.

It does package:

- compiled program header tensors
- compiled instruction stream tensors
- compiled initial-memory tensors
- decode-contract tensors

But its `execute(...)` path still calls:

- `execute_tassadar_executor_request(...)`

which routes back into the imperative runtime executor.

So the current compile-to-weights path is:

- a program-specialized artifact contract
- proof and lineage metadata
- deterministic bundling

not:

- a compiled neural network that independently executes the program

### 5. The current served and planner surfaces are truthful wrappers

`crates/psionic/psionic-serve/src/tassadar.rs` gives us:

- `psionic.executor_trace`
- `psionic.planner_executor_route`
- typed request/response/refusal contracts
- trace streaming
- planner routing truth

That is good substrate.

But these surfaces currently expose the imperative executor lane, not a trained
executor model.

## What The Sudoku And Hungarian Benchmarks Actually Are

This is the most important concrete gap.

The repo currently names article-class benchmark targets:

- `MicroWasmKernel`
- `SudokuClass`
- `HungarianMatching`

But the actual cases in `crates/psionic/psionic-runtime/src/tassadar.rs` are:

- `micro_wasm_kernel`
  - an unrolled weighted-sum/checksum micro-kernel
- `sudoku_class`
  - "sum-based exact completion for two missing values in a tiny 4x4
    Sudoku-style instance"
- `hungarian_matching`
  - "tiny fixed 2x2 matching instance with branch-selected winning assignment
    and exact cost"

Those are useful placeholders.

They are not the article's workloads.

In particular:

- the current Sudoku case is not a real search or constraint-propagation solver
- the current Hungarian case is not a general Hungarian implementation
- the expected traces for those cases are generated by
  `TassadarCpuReferenceRunner`, not by compiling and validating an external
  article-grade solver

So the current article-class corpus is still:

- a placeholder benchmark family

not:

- evidence that Tassadar can solve hard Sudoku or 10x10 assignment problems

## Gap Table: Percepta Vision vs Current Tassadar

| Percepta-style requirement | Current Tassadar | Actual gap |
| --- | --- | --- |
| WebAssembly interpreter lives in transformer weights | imperative Rust runners plus fixture metadata | no real executor model exists |
| execution trace is emitted token by token | structured `TassadarTraceStep` objects after imperative execution | no token-level autoregressive executor |
| 2D attention is part of the real model | research metadata can describe `head_dim == 2` | no trained 2D-head model in runtime |
| hull cache accelerates neural attention lookup | runtime keeps last-write indices and explicit histories | no neural hull-cache decode path |
| sparse top-k is an approximate attention mode | runtime keeps bounded recent-write histories | no neural sparse decode path |
| compiled programs become working executor weights | compiled artifacts still delegate to runtime executor | no executable compiled neural weights |
| training learns executor behavior | current training learns only arithmetic kernels | no trained full executor semantics |
| Sudoku benchmark is a real solver workload | tiny 4x4 fill-in by row-sum arithmetic | not real Sudoku solving |
| Hungarian benchmark is a real algorithmic workload | fixed 2x2 branch choice | not a real Hungarian solver |
| million-step traces | `core_i32_v2` caps at 128 instructions and 512 steps | scale is far below article claims |

## The Single Biggest Missing Piece

The single biggest missing piece is simple:

> there is no actual executor model family whose forward pass predicts the next
> execution-trace token.

Everything else in the current Tassadar lane is downstream of that fact.

Because there is no such model yet, the repo also lacks:

- trace-token datasets
- teacher-forced next-token training
- sequence exactness evaluation for model outputs
- true executor decoding benchmarks in tokens per second
- true hull-cache-vs-linear attention benchmarks over model KV state

## What Must Exist Before "Train A Model To Actually Do It" Is A True Claim

To make the claim honest, Psionic needs all of the following.

### 1. A real executor model architecture

Psionic needs a model family that explicitly represents:

- token embeddings for program and trace tokens
- constrained head geometry for the executor path
- executor attention mode such as hard-max lookup
- an executor output head over the trace vocabulary

This cannot stay as a fixture-only descriptor.

### 2. A real token vocabulary and sequence interface

The article is about sequence execution inside a transformer.

So Tassadar needs a token-level contract for at least:

- program bytes or lowered Wasm op tokens
- trace-step commit tokens
- output tokens
- halt tokens
- any structured markers needed for append-only execution

Right now the repo has:

- structured Rust instructions
- structured Rust trace events

That is not the same thing.

### 3. A real training corpus

The current training data is just arithmetic supervision extracted from three
microprograms.

A real executor training corpus needs:

- many digest-bound Wasm programs
- CPU-reference execution traces for those programs
- tokenized program prefixes
- tokenized step-by-step trace continuations
- train/validation/test splits that do not collapse into the same toy cases

### 4. A real training objective

The current trainer optimizes scalar kernels.

A real executor model needs at least:

- next-token prediction over execution traces
- exact-trace evaluation
- exact-output evaluation
- halt correctness
- length-generalization checks

Later it may need:

- curriculum over program complexity
- mixed program/traces batches
- search-heavy trace weighting

### 5. A real neural fast path

The current hull and sparse paths are runtime simulators.

To actually implement the article's claim, Psionic needs:

- model KV tensors from the executor forward pass
- a linear decode baseline over those KV tensors
- a hull-based retrieval path over real attention keys
- exact or explicitly approximate equivalence reporting between those two

Until that exists, the current fast-path benchmarking is about simulator
variants, not model inference variants.

### 6. A harder Wasm subset

The current opcode set is too small for honest Sudoku or Hungarian targets.

At minimum, a real Sudoku-first plan will likely need more support for:

- comparisons
- equality tests
- richer branch/control shapes
- loop-heavy or backtracking-friendly control flow
- possibly bitwise or mask-like operations

The current hull and sparse validation paths also reject backward branches.

That matters because real Sudoku search is exactly the kind of workload that
wants loops and backtracking.

## Sudoku: The Best Honest Starter Target

If the next goal is "train a model to actually do one article-like thing,"
Sudoku is the best starter target, but only if it is defined honestly.

The right first Sudoku milestone is **not**:

- the current `sudoku_class` placeholder
- or immediate 9x9 hard-Sudoku marketing

The right first milestone is:

> compile a real 4x4 Sudoku solver into the supported Wasm lane, generate exact
> CPU traces, and train an executor model to continue that trace exactly.

That milestone is hard enough to matter and small enough to finish.

### Why 4x4 first

- it keeps trace lengths manageable
- it allows real search and contradiction handling
- it forces honest control-flow support
- it gives a real exactness target before 9x9 scale

### What the Sudoku starter path needs

1. Replace the current fake `sudoku_class` case with a real compiled solver.
2. Expand the Wasm profile enough to express that solver.
3. Generate many puzzle instances, not one fixed puzzle.
4. Record exact CPU-reference traces for every instance.
5. Tokenize program plus trace into a training corpus.
6. Train an executor model to predict the next trace token.
7. Evaluate exact trace continuation and final solved grid correctness.
8. Only then talk about 9x9 and hard-Sudoku progression.

## The Smallest Honest Roadmap From Here

### Phase A: Clean up the target and track it honestly (`#3776`)

Do this first:

- explicitly rename current `SudokuClass` and `HungarianMatching` in docs as
  placeholder article-class proxies unless and until replaced
- stop treating current Phase 9B as "trained executor" in narrative docs
- define one new milestone called something like `trained_executor_v0`

This audit and the linked epic exist to make that correction explicit before
more implementation claims accumulate.

### Phase B: Build a real Sudoku-v0 workload (`#3777`, `#3778`, `#3779`)

Do this second:

- add a real 4x4 Sudoku solver program artifact path
- compile it from source into the Wasm-first lane
- widen the opcode/profile subset enough to express it
- create many puzzle instances and reference traces

### Phase C: Add a true executor model (`#3780`, `#3781`)

Do this third:

- implement a small executor transformer family in `psionic-models`
- add token vocabulary plus sequence batching
- add executor forward pass and next-token loss in `psionic-train`
- keep exact-trace evaluation in `psionic-eval`

### Phase D: Establish the trained-model baseline honestly (`#3782`)

Do this fourth:

- benchmark linear neural decode first
- compare it directly against CPU-reference execution
- keep exact-trace, halt, and final-output truth explicit

### Phase E: Run, log, and learn from the first model (`#3783`, `#3784`,
`#3785`)

Do this fifth:

- freeze the first honest Sudoku-v0 corpus and model config
- execute the first full Psionic-only training run
- persist receipts, checkpoints, eval outputs, telemetry, and failure samples
- publish the first postmortem and next-run plan from observed failure modes

### Phase F: Only then test the neural fast path and scale (`#3786`, `#3787`)

Do this sixth:

- then add hull-cache lookup over real model KV state
- then measure exactness plus speedup honestly
- move from 4x4 to 9x9 only after the first trained 4x4 lane is real
- keep the distinction between Sudoku-v0 success and article-grade scale
  completely explicit

## What OpenAgents Can Honestly Claim Today

OpenAgents can honestly claim today:

- `Tassadar` is a solid executor-substrate program
- Psionic has a truthful Wasm-first executor ABI
- Psionic has real proof, eval, serve, and routing surfaces for that lane
- Psionic has a narrow training experiment that learns arithmetic kernels
  inside that lane

OpenAgents cannot yet honestly claim:

- a trained model that executes programs inside transformer weights
- Percepta-style in-model execution
- a real trained 2D-head executor
- a real neural hull-cache decode path
- a real Sudoku solver trained into the model

## Bottom Line

The user concern is correct.

The repo has implemented a large amount of `Tassadar` infrastructure, but that
work should currently be read as:

- executor substrate complete enough to begin real model work

not:

- the real executor model work itself being done

If OpenAgents wants the first true Percepta-style milestone, the best path is:

1. stop treating the current placeholder Sudoku benchmark as sufficient
2. build a real 4x4 Sudoku Wasm solver target
3. add tokenized trace training for an actual executor model
4. prove exact trace continuation there before talking about hard Sudoku or
   general article parity

That is the shortest honest path from current `Tassadar` to "we trained a
model that actually does this."
