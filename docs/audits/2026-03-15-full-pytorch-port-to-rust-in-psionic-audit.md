# 2026-03-15 Full PyTorch Port To Rust In Psionic Audit

## Intent

This audit answers a harder question than the earlier PyTorch test-port note:

> if Psionic were to attempt a true full port of PyTorch into Rust, what would
> that actually mean, what subsystem path would be required, and how could it
> be executed without collapsing into a fake parity story?

The useful answer is not:

- "keep adding operators until it feels PyTorch-like"
- "wrap current Psionic serving code in a Python API and call it done"
- "treat `torch.compile` or `nn.Module` as a thin surface problem"

The useful answer is:

- define what "full PyTorch port" means in subsystem terms
- map those subsystems onto Psionic ownership boundaries
- spell out the only realistic sequence to build them in Rust
- say clearly which parts are tractable, which parts are multi-year, and which
  parts are optional compatibility layers instead of framework-core essentials

This is a reflection document, not a claim that this should become MVP scope.
`docs/MVP.md` and `docs/OWNERSHIP.md` still govern the active product boundary.

## Scope

PyTorch sources reviewed from `~/code/pytorch`:

- `README.md`
- `docs/source/tensors.rst`
- `docs/source/autograd.md`
- `docs/source/fx.md`
- `docs/source/torch.compiler_api.md`
- `docs/source/notes/serialization.rst`
- `docs/source/distributed.md`
- top-level tree layout including:
  - `aten/`
  - `c10/`
  - `torch/`
  - `torchgen/`
  - `functorch/`
  - `test/`

OpenAgents sources reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/audits/2026-03-15-pytorch-test-suite-port-audit.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md`
- `crates/psionic/docs/PSI_FOR_HUMANS.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
- `crates/psionic/psionic-compiler/src/lib.rs`
- `crates/psionic/psionic-ir/src/autodiff.rs`
- `crates/psionic/psionic-train/src/model_io.rs`
- `crates/psionic/psionic-train/src/optimizer.rs`
- `crates/psionic/psionic-train/src/distributed_optimizer.rs`
- `crates/psionic/psionic-models/src/lib.rs`
- `crates/psionic/psionic-serve/src/lib.rs`

## Executive Summary

A true full port of PyTorch to Rust inside Psionic is possible in principle,
but only if everyone is honest about what "full" means.

If "full" means:

- a Rust-native tensor system with PyTorch-class semantics
- PyTorch-class autodiff and module state behavior
- broad optimizer and serialization coverage
- a real compiler stack
- distributed training/runtime substrate

then this is not a feature project. It is a new framework program.

The correct mental model is:

> porting PyTorch to Rust means rebuilding the equivalent of `c10`, `ATen`,
> dispatch, autograd, `nn`, `optim`, serialization, compiler, and distributed,
> then optionally layering Python compatibility and ecosystem interop on top.

The good news is that Psionic already owns the right architectural direction:

- `psionic-core`
- `psionic-ir`
- `psionic-compiler`
- `psionic-runtime`
- `psionic-models`
- `psionic-serve`
- `psionic-train`
- cluster and collectives crates

The bad news is that current Psionic is still much closer to:

- a serious, growing Rust-native ML execution substrate

than to:

- a full PyTorch replacement

So the only credible path is:

1. stop thinking in terms of API parity first
2. finish the framework substrate in the order PyTorch itself is built on
3. build declarative compatibility harnesses against PyTorch as oracle
4. add frontend compatibility only after the substrate is real

If this is attempted honestly, it is likely a multi-year program.

## What "Full PyTorch Port" Actually Means

Most people say "port PyTorch" when they really mean one of three different
things.

### Level 1: inference-oriented PyTorch subset

This means:

- enough tensor ops to run selected models
- enough autograd to support some training
- enough serialization to load useful checkpoints
- maybe a compiler and backend story

This is not a full port.

### Level 2: framework-complete PyTorch-class runtime

This means:

- broad tensor semantics
- broad operator coverage
- autograd and view semantics
- modules and state trees
- optimizers
- serialization
- compiler stack
- distributed substrate

This is the minimum level that deserves the phrase "full PyTorch port" in
framework terms, even if Python compatibility is still incomplete.

### Level 3: product and ecosystem replacement

This adds:

- Python frontend compatibility
- custom-op ecosystem
- extension build story
- TorchScript / export / ONNX compatibility surface
- package and deployment tooling
- backend-lab breadth comparable to upstream PyTorch

This is the real maximal reading of "full port."

If Psionic ever attempts this, it should explicitly declare which level it is
targeting. Otherwise the program will drift into slogans.

## PyTorch's Real Subsystem Map

A true port is not one crate. It is a stack.

### 1. `c10` and tensor substrate

Equivalent responsibilities:

- device and dtype enums
- layout and memory format semantics
- storage identity
- tensor metadata
- strided view rules
- aliasing and mutation rules

### 2. `ATen` and operator library

Equivalent responsibilities:

- primitive operator definitions
- shape rules
- dtype and promotion rules
- backend-specific kernel families
- reference implementations

### 3. dispatch and operator schema

Equivalent responsibilities:

- operator registration
- backend dispatch
- composite kernels
- meta kernels
- autograd-aware operator wrapping

### 4. autograd engine

Equivalent responsibilities:

- reverse-mode AD
- forward-mode AD
- graph capture for gradients
- saved tensors and view tracking
- mutation correctness rules
- detach / no-grad / inference mode semantics

### 5. `nn` and module/state system

Equivalent responsibilities:

- trainable parameter containers
- buffers
- module composition
- `state_dict`
- strict/non-strict state loading
- device and dtype movement semantics

### 6. `optim`

Equivalent responsibilities:

- optimizer families
- optimizer state
- parameter groups
- serialization
- scheduler composition

### 7. serialization

Equivalent responsibilities:

- tensor and state persistence
- storage/view preservation rules
- state-tree restore semantics
- compatibility boundaries across versions and formats

### 8. compiler stack

Equivalent responsibilities:

- graph capture
- symbolic shapes
- graph rewriting
- export
- lowering
- code generation
- caches and recompilation
- memory planning

### 9. distributed

Equivalent responsibilities:

- collectives
- process-group semantics
- DDP/FSDP-like model partitioning and synchronization
- distributed optimizer and checkpoint behavior
- cluster initialization and failure handling

### 10. ecosystem compatibility

Equivalent responsibilities:

- Python API compatibility
- extension APIs
- ONNX or export compatibility
- packaging and deployment expectations

PyTorch is huge because it has all ten layers, not because it has many tensor
ops.

## Where Psionic Already Helps

Psionic already has the correct ownership split for a Rust-native port.

### What already maps well

- `psionic-core`
  candidate home for `c10`-class tensor metadata, dtype, device, and layout
  semantics
- `psionic-ir`
  candidate home for autograd-aware graph semantics and canonical execution IR
- `psionic-compiler`
  candidate home for lowering, replay, cache, and compiler identity
- `psionic-runtime`
  candidate home for backend contracts, execution plans, memory and cache truth
- `psionic-train`
  candidate home for optimizer, checkpoint, distributed optimizer, and training
  state
- `psionic-collectives` and `psionic-cluster`
  candidate home for distributed synchronization and topology truth

This is better than trying to graft PyTorch semantics into an app layer.

### What already exists but is still early

- explicit tensor metadata and quantized payload containers
- real reverse-mode autodiff with explicit `detach` and no-grad semantics
- reusable optimizer families
- state-dict-like portable model IO
- compiler replay fixtures and stable execution-plan digests
- multi-device and distributed optimizer contracts

These are the beginnings of a PyTorch-class substrate, but only the beginnings.

## The Hard Truth: What Is Still Missing

### Missing 1: a real tensor object model

Current Psionic has tensor specs, payloads, and graph constructs.

It does not yet expose a truly broad tensor object model with PyTorch-class
behavior around:

- views
- aliasing
- mutation
- storage identity
- dtype promotion
- indexing breadth
- eager op execution

Without that, there is no honest "Rust PyTorch" claim.

### Missing 2: a dispatcher and operator registration system

PyTorch's power is not just "many kernels." It is the dispatcher:

- op schemas
- per-backend registration
- composite fallback
- meta execution
- autograd integration

Psionic currently has backend traits and IR lowering, but not a full
dispatcher-shaped substrate.

### Missing 3: broad operator coverage

Even with current backend extensions and model-serving primitives, Psionic is
still a sparse operator environment compared to PyTorch.

### Missing 4: module semantics

Psionic has trainable parameter groups and portable model bundles.

It does not yet have a full reusable module system with:

- parameter and buffer ownership
- nested module traversal
- state-tree mutation rules
- load semantics matching PyTorch expectations

### Missing 5: compiler frontend

Psionic has IR and lowering, but not a PyTorch-class frontend stack equivalent
to:

- `fx`
- `dynamo`
- `export`
- `inductor`

### Missing 6: compatibility story

A full port must answer:

- do we support Python?
- do we support custom ops?
- do we support PyTorch checkpoint formats directly?
- do we support ONNX export?
- do we support extension authors?

Those cannot be left implicit.

## The Only Credible Path

If this program is attempted, the path has to be layered and ruthless.

### Phase 0: define the target precisely

Before code:

- declare whether the target is Level 2 or Level 3 from this audit
- declare which PyTorch surfaces are non-negotiable
- define acceptance matrices for:
  - tensor semantics
  - autodiff
  - module/state behavior
  - optimizer behavior
  - compiler identity and dynamic shapes
  - distributed semantics
  - compatibility surfaces

Without this, the project will spend years accumulating partial parity with no
closure criteria.

### Phase 1: build a Rust-native tensor and dispatch substrate

This is the real start.

Required deliverables:

- first-class tensor object with storage, view, stride, layout, dtype, and
  alias semantics
- operator schema registry
- dispatch table keyed by backend and capability
- meta kernels for shape-only execution
- reference backend for correctness

Recommended crate shape:

- `psionic-core`
  keep scalar type, device, layout, and storage primitives
- new `psionic-tensor`
  own `Tensor`, `Storage`, view rules, mutation/version counters
- new `psionic-dispatch`
  own op schemas, registration, dispatch tables, meta and composite fallback
- new `psionic-ops`
  own operator definitions and reference implementations

If this phase is weak, every higher phase will be fake.

### Phase 2: build broad operator coverage and an `OpInfo`-class harness

Do not try to port every op first. Build the harness first.

Required deliverables:

- declarative operator registry
- forward reference cases
- dtype and promotion expectations
- alias and out-variant expectations
- shape-only reference cases
- refusal cases

Start with:

- elementwise ops
- reductions
- matmul and batched matmul
- indexing and view ops
- normalization primitives
- attention primitives
- tensor creation ops

Use local PyTorch as the oracle generator for fixtures and tolerances.

### Phase 3: complete autograd properly

This is where most fake framework ports fail.

Required deliverables:

- broader reverse-mode coverage
- saved-tensor semantics
- mutation correctness checks
- view + gradient interaction rules
- forward-mode AD
- higher-order derivative support where the framework promises it
- functionalization or equivalent mutation-to-functional transform layer

Recommended ownership:

- `psionic-ir`
  graph semantics
- `psionic-autograd` or `psionic-ir::autograd`
  engine, saved tensors, context semantics

The current `psionic-ir` path is a valid seed, but nowhere near closure.

### Phase 4: build `nn`-class reusable module semantics

Required deliverables:

- reusable parameter and buffer containers
- module composition and traversal
- typed `state_dict` equivalent
- strict/non-strict load semantics
- device and dtype movement rules
- a `module_db`-class conformance harness

Recommended ownership:

- new `psionic-nn`
  reusable module substrate
- `psionic-train`
  stays focused on training orchestration and checkpoint/runtime contracts

This is where many PyTorch-trained users will decide whether the system is
really usable.

### Phase 5: complete optimizer, scheduler, and optimizer-state semantics

Required deliverables:

- optimizer family breadth
- parameter groups
- optimizer state portability
- scheduler integration
- optimizer hook semantics only if deliberately promised
- `optim_db`-class parity harness

Recommended ownership:

- `psionic-train`
  remains the main owner

The current optimizer surface is a strong early seed, not the destination.

### Phase 6: complete model and checkpoint IO

Required deliverables:

- typed stable state artifact
- clear answer on PyTorch checkpoint interop
- storage/view semantics where promised
- backward-compatible load policy
- artifact receipts and manifests

Hard choice required:

- either support PyTorch's zip + pickle compatibility directly
- or declare a safe conversion boundary and never pretend `.pt` is native

My recommendation:

- keep native Psionic artifacts typed and safe
- provide import/export converters for PyTorch compatibility
- do not make Python pickle the canonical runtime artifact of Psionic

### Phase 7: build the compiler stack in the right order

A full port does not need to literally clone PyTorch's frontend names, but it
does need equivalent capabilities.

Required deliverables:

- graph capture from a frontend
- symbolic shape environment
- guard system
- graph rewriting and normalization
- exportable stable graph form
- lowering and memory planning
- compile caches and invalidation

Recommended ownership:

- `psionic-ir`
  stable graph and exported graph forms
- `psionic-compiler`
  rewrites, lowering, scheduling, codegen boundaries, cache contracts
- new `psionic-symbolic`
  symbolic shape and guard environment
- possibly new `psionic-frontend`
  frontend capture layer if not embedded elsewhere

Important truth:

- if Psionic stays Rust-native only, it does not need a literal `torch._dynamo`
  equivalent
- if the goal is Python compatibility, it absolutely needs a frontend capture
  story for Python code

### Phase 8: complete distributed semantics on Psionic's own terms

This is where Psionic already has a chance to diverge productively.

Required deliverables:

- collective semantics broad enough to match PyTorch-class training needs
- DDP/FSDP-equivalent state and communication patterns
- distributed optimizer runtime execution, not just contracts
- elastic recovery and checkpoint restore
- failure handling and reconciliation

Recommended ownership:

- `psionic-collectives`
- `psionic-cluster`
- `psionic-train`
- `psionic-runtime`

This repo should not copy PyTorch's process-group API literally unless
Python/frontend compatibility demands it.

### Phase 9: decide whether Python compatibility is in or out

This is the strategic fork.

#### Option A: Rust-native PyTorch-class framework

Pros:

- technically coherent
- clean crate boundaries
- no Python ABI dependence

Cons:

- not a drop-in replacement for PyTorch users

#### Option B: Rust-native core with Python compatibility layer

Pros:

- much stronger adoption path
- can reuse some model ecosystems

Cons:

- frontend capture becomes much harder
- extension and custom-op story becomes much harder
- debugging complexity rises sharply

If the actual goal is "full PyTorch port," Option B is eventually required.
If the actual goal is "PyTorch-class Rust framework," Option A is enough.

### Phase 10: ecosystem and compatibility closure

Only after the substrate is real:

- ONNX import/export
- custom op extensions
- packaging and deployment
- external backend plugins
- docs, migration guides, compatibility matrix

This is the final layer, not the first.

## How This Should Map Onto Psionic Crates

The cleanest port path is to extend Psionic's architecture, not replace it.

Recommended long-term crate map:

- `psionic-core`
  scalar, dtype, device, layout, storage primitives
- `psionic-tensor`
  eager tensor object, storage, views, alias and mutation rules
- `psionic-dispatch`
  op schema, registration, dispatch keys, meta and composite kernels
- `psionic-ops`
  reference ops plus backend registrations
- `psionic-ir`
  canonical graph and exported graph form
- `psionic-autograd`
  reverse/forward AD engine and functionalization
- `psionic-nn`
  module, parameter, buffer, and state-tree semantics
- `psionic-compiler`
  graph rewrites, lowering, scheduling, cache and memory planning
- `psionic-runtime`
  backend contracts, plan execution, memory and cache truth
- `psionic-train`
  optimizers, schedulers, checkpoints, distributed optimizer execution
- `psionic-collectives`
  collective planning and runtime integration
- `psionic-cluster`
  multi-node and topology truth
- `psionic-models`
  model formats and reusable descriptors
- `psionic-serve`
  product serving surfaces

This is a large expansion, but it keeps ownership clean.

## Test and Verification Path

If the goal is a full port, the tests matter as much as the code.

### Required verification program

1. Port a Rust-native `OpInfo`-class harness.
2. Port a Rust-native `module_db`-class harness.
3. Port a Rust-native `optim_db`-class harness.
4. Add compiler replay and symbolic-shape conformance gates.
5. Add distributed correctness and failure-injection tests.
6. Run shadow validation against local PyTorch fixtures continuously.

### Oracle strategy

Use PyTorch for:

- operator outputs
- gradient outputs
- module/state semantics
- optimizer-state behavior
- compiler invariants where the semantics are frontend-independent

Use native Psionic truth for:

- receipts
- replay identity
- cluster topology truth
- sandbox and provider contracts

## Program Shape and Resourcing

This should be treated as a dedicated framework program.

If the goal is Level 2:

- likely multi-year
- likely requires a small but elite dedicated team
- must prioritize harnesses and substrate over frontend polish

If the goal is Level 3:

- substantially larger again
- Python compatibility and ecosystem support become major workstreams, not
  side quests

The fastest way to fail would be:

- one or two engineers
- no explicit parity matrices
- starting from frontend bindings instead of tensor/dispatch substrate

## Recommendations

### Recommendation 1: do not start by cloning the Python API

That is the highest-friction and lowest-truth place to begin.

### Recommendation 2: start with tensor, dispatch, and harnesses

That is where real framework credibility starts.

### Recommendation 3: keep Psionic-native truth surfaces

Even a full PyTorch-class port should not discard:

- receipts
- replay identity
- topology truth
- sandbox truth
- provider-facing evidence

Those are Psionic advantages, not distractions.

### Recommendation 4: separate native artifact truth from compatibility imports

Do not let pickle become Psionic's native artifact boundary.

### Recommendation 5: explicitly choose between "PyTorch-class" and "PyTorch-compatible"

The program cannot optimize for both at the same time from day one.

## Bottom Line

A full PyTorch port to Rust in Psionic is not absurd, but it is enormous.

The only honest path is:

- build the framework substrate first
- build the declarative parity harnesses second
- build module and optimizer semantics third
- build compiler and distributed closure next
- add Python and ecosystem compatibility only after the native framework is
  already real

If this order is reversed, the result will be a compatibility facade over an
unfinished engine.

If this order is followed, Psionic could become something stronger than "Rust
PyTorch clone":

- a PyTorch-class Rust-native framework core
- with explicit execution, replay, topology, and receipt truth that PyTorch was
  never designed to own
