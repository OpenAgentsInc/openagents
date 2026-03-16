# 2026-03-16 MLX Full Rust Port Into Psionic Audit

## Intent

This audit answers the practical version of the user request:

> if OpenAgents wants a true MLX-class framework inside Psionic, implemented in
> Rust and merged with the current Psionic substrate where relevant, what does
> that actually mean, what already fits, what does not fit, and what is the
> only credible integration sequence?

The useful answer is not:

- "copy Apple's repo file-for-file in Rust"
- "treat MLX as just another Metal backend"
- "wrap current Psionic serving code in MLX-like names and call it a port"

The useful answer is:

- define what a full MLX port means in subsystem terms
- map MLX's subsystems onto Psionic's current ownership and crate boundaries
- say clearly what should be ported, what should be redesigned, and what
  should remain explicitly Psionic-native
- sequence the work so it produces a real framework rather than a compatibility
  facade over incomplete semantics

This is a library and architecture audit. It does not widen active product MVP
scope in [docs/MVP.md](/Users/christopherdavid/code/openagents/docs/MVP.md),
and it does not alter ownership boundaries in
[docs/OWNERSHIP.md](/Users/christopherdavid/code/openagents/docs/OWNERSHIP.md).

## Scope

OpenAgents sources reviewed:

- [docs/MVP.md](/Users/christopherdavid/code/openagents/docs/MVP.md)
- [docs/OWNERSHIP.md](/Users/christopherdavid/code/openagents/docs/OWNERSHIP.md)
- [crates/psionic/README.md](/Users/christopherdavid/code/openagents/crates/psionic/README.md)
- [crates/psionic/docs/ROADMAP.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/ROADMAP.md)
- [crates/psionic/docs/ARCHITECTURE.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/ARCHITECTURE.md)
- [crates/psionic/docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md)
- [crates/psionic/docs/TRAIN_SYSTEM.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/TRAIN_SYSTEM.md)
- [crates/psionic/docs/deep-research-mlx.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/deep-research-mlx.md)
- [crates/psionic/docs/METAL_GPT_OSS_MLX_LM_LESSONS.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/METAL_GPT_OSS_MLX_LM_LESSONS.md)
- [crates/psionic/psionic-core/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-core/src/lib.rs)
- [crates/psionic/psionic-ir/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-ir/src/lib.rs)
- [crates/psionic/psionic-compiler/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-compiler/src/lib.rs)
- [crates/psionic/psionic-runtime/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-runtime/src/lib.rs)
- [crates/psionic/psionic-backend-cpu/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-backend-cpu/src/lib.rs)
- [crates/psionic/psionic-train/src/model_io.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-train/src/model_io.rs)
- [crates/psionic/psionic-train/src/optimizer.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-train/src/optimizer.rs)
- [crates/psionic/psionic-train/src/distributed_optimizer.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-train/src/distributed_optimizer.rs)
- [crates/psionic/psionic-models/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-models/src/lib.rs)
- [crates/psionic/psionic-serve/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-serve/src/lib.rs)

Upstream MLX sources reviewed from `~/code/mlx`:

- [~/code/mlx/README.md](/Users/christopherdavid/code/mlx/README.md)
- [~/code/mlx/docs/src/index.rst](/Users/christopherdavid/code/mlx/docs/src/index.rst)
- [~/code/mlx/docs/src/usage/lazy_evaluation.rst](/Users/christopherdavid/code/mlx/docs/src/usage/lazy_evaluation.rst)
- [~/code/mlx/docs/src/usage/unified_memory.rst](/Users/christopherdavid/code/mlx/docs/src/usage/unified_memory.rst)
- [~/code/mlx/docs/src/usage/function_transforms.rst](/Users/christopherdavid/code/mlx/docs/src/usage/function_transforms.rst)
- [~/code/mlx/docs/src/usage/compile.rst](/Users/christopherdavid/code/mlx/docs/src/usage/compile.rst)
- [~/code/mlx/docs/src/usage/export.rst](/Users/christopherdavid/code/mlx/docs/src/usage/export.rst)
- [~/code/mlx/docs/src/usage/distributed.rst](/Users/christopherdavid/code/mlx/docs/src/usage/distributed.rst)
- [~/code/mlx/docs/src/usage/saving_and_loading.rst](/Users/christopherdavid/code/mlx/docs/src/usage/saving_and_loading.rst)
- [~/code/mlx/docs/src/python/memory_management.rst](/Users/christopherdavid/code/mlx/docs/src/python/memory_management.rst)
- [~/code/mlx/mlx/array.h](/Users/christopherdavid/code/mlx/mlx/array.h)
- [~/code/mlx/mlx/transforms.h](/Users/christopherdavid/code/mlx/mlx/transforms.h)
- [~/code/mlx/mlx/compile.h](/Users/christopherdavid/code/mlx/mlx/compile.h)
- [~/code/mlx/mlx/export.h](/Users/christopherdavid/code/mlx/mlx/export.h)
- [~/code/mlx/mlx/distributed/distributed.h](/Users/christopherdavid/code/mlx/mlx/distributed/distributed.h)
- [~/code/mlx/python/mlx/nn/layers/base.py](/Users/christopherdavid/code/mlx/python/mlx/nn/layers/base.py)
- [~/code/mlx/python/mlx/nn/utils.py](/Users/christopherdavid/code/mlx/python/mlx/nn/utils.py)
- [~/code/mlx/python/mlx/nn/layers/distributed.py](/Users/christopherdavid/code/mlx/python/mlx/nn/layers/distributed.py)
- [~/code/mlx/python/mlx/optimizers/optimizers.py](/Users/christopherdavid/code/mlx/python/mlx/optimizers/optimizers.py)
- top-level tree layout under `mlx/`, `python/`, `tests/`, and `python/tests/`

## Executive Summary

A full MLX port into Rust inside Psionic is plausible, but only if everyone is
honest about the size and shape of the program.

The correct mental model is:

> this is not "add a few more Metal kernels."
>
> this is "build an MLX-class semantics layer above Psionic's existing core,
> compiler, runtime, backend, collectives, and train substrate."

The good news is that Psionic already owns several layers MLX itself does not:

- typed runtime and refusal contracts
- replay identity and compile/cache truth
- manifest, receipt, and proof-bearing execution surfaces
- reusable cluster, datastream, sandbox, collectives, and train substrate
- explicit backend truth across CPU, Metal, CUDA, AMD, and Apple FM lanes

The bad news is that Psionic still does not have the main user-facing MLX
surface:

- a lazy array type with explicit `eval` semantics
- function-transform-first API shape
- MLX-class `nn.Module` and parameter-tree semantics
- optimizer-tree APIs and scheduler families
- framework-level distributed group semantics and helpers
- function export/import analogous to `.mlxfn`
- broad operator and test closure at MLX scale

So the right architecture is:

- keep Psionic's current lower layers
- add a Rust-native MLX-class semantics layer above them
- make compatibility with MLX naming and file formats a later shell, not the
  first deliverable

If this is executed correctly, Psionic becomes:

- MLX-like in framework feel
- Rust-native in implementation
- Psionic-native in manifests, receipts, replay, cluster truth, sandbox truth,
  and provider/runtime evidence

That is the only version of "full MLX port" that makes sense in this repo.

## What "Full MLX Port" Actually Means

Most people say "port MLX" when they mean one of three different targets.

### Level 1: borrow MLX ideas for one backend lane

This means:

- use MLX lessons for Metal execution
- maybe add some lazy graph behavior
- maybe copy some model or caching ideas

This is not a full port.

### Level 2: build an MLX-class framework in Rust

This means:

- lazy arrays and graph construction
- explicit evaluation and scheduling semantics
- `grad`, `value_and_grad`, `jvp`, `vjp`, `vmap`, `checkpoint`, and compile
- module and parameter-tree semantics
- optimizer and scheduler families
- model and array IO
- function export/import
- framework-level distributed communication and sharding helpers
- CPU, Metal, and CUDA backend support under one consistent API

This is the minimum level that deserves the phrase "full MLX port" inside
Psionic.

### Level 3: MLX compatibility shell

This adds:

- MLX naming and API-compat layers
- optional `.mlxfn` compatibility
- optional Python, C, or Swift facing surfaces
- importer or exporter paths for MLX artifacts and tests

This is useful, but it is not the first priority.

### Recommended target

Psionic should target:

- Level 2 first
- selected Level 3 compatibility after Level 2 is real

Anything else risks shipping a facade over missing semantics.

## What Upstream MLX Actually Contains

The upstream repo is already large enough that "just port it" is misleading.

Rough scale from the checked-out tree:

- `mlx/`: about `380` C/C++ files and about `110k` lines
- `python/`: about `69` Python files and about `32k` lines
- `mlx/backend/metal`: about `173` files and about `50k` lines
- `mlx/backend/cuda`: about `178` files and about `25k` lines
- `tests/`: about `23` C++ test files and about `11k` lines
- `python/tests/`: about `38` Python test files and about `23k` lines

The backend counts overlap with the broader C/C++ tree, but the point stands:
MLX is already a full framework stack, not a compact example repo.

## Upstream MLX Subsystem Map

| Subsystem | Upstream evidence | What it means for Psionic |
| --- | --- | --- |
| Lazy array graph core | `mlx/array.h`, `mlx/ops.*`, `mlx/primitives.*`, `mlx/scheduler.*` | Psionic needs a real public array/tensor facade above `psionic-core` and `psionic-ir`, not only internal graph types |
| Function transforms | `mlx/transforms.h`, docs in `usage/function_transforms.rst` | Psionic needs a first-class transforms crate or module layer, not just reverse-mode internals |
| Compile-as-transform | `mlx/compile.h`, docs in `usage/compile.rst` | Psionic needs compilation surfaced as a callable transform with cache identity and purity rules |
| Export/import | `mlx/export.h`, docs in `usage/export.rst` | Psionic needs a graph/function export story distinct from weight-only IO |
| Serialization breadth | docs in `usage/saving_and_loading.rst`, `mlx/io/*` | Psionic already has strong GGUF and safetensors weight IO, but not MLX-style general array save/load surface |
| Module/state system | `python/mlx/nn/layers/base.py` | Psionic needs general module trees, parameter freeze/trainable semantics, and save/load rules above model-family code |
| Optimizers and schedulers | `python/mlx/optimizers/optimizers.py`, `schedulers.py` | Psionic can reuse train substrate ideas but needs a framework-wide optimizer API, not train-only wiring |
| Framework-level distributed | `mlx/distributed/*`, docs in `usage/distributed.rst`, `python/mlx/nn/layers/distributed.py`, `python/mlx/nn/utils.py` | Psionic already has collectives and cluster substrate, but not MLX-like framework-facing distributed semantics |
| Multi-backend runtime | `backend/common`, `backend/cpu`, `backend/metal`, `backend/cuda` | Psionic already has the right backend split, but still needs broader operator and stream coverage |
| Memory and debug tooling | docs in `python/memory_management.rst`, `dev/metal_debugger.rst`, `dev/metal_logging.rst` | Psionic should expose memory counters, cache truth, and backend debug hooks as framework tools, not only internal diagnostics |
| Conformance pressure | `tests/*`, `python/tests/*` | Psionic needs a dedicated MLX-parity harness if it wants this claim to be honest |

## The MLX Benefits Psionic Should Actually Import

If the goal is "all of the benefits MLX has, but here in Rust and merged with
Psionic," the benefits worth importing are:

- lazy evaluation with explicit `eval`
- dynamic graph construction without whole-program static tracing requirements
- function transforms as the main semantics surface
- compile as an optimization layer, not as a separate framework
- module trees with trainable and frozen parameter posture
- tree-aware optimizer updates
- practical serialization for arrays and model weights
- backend neutrality across CPU, Metal, and CUDA
- framework-level distributed helpers for data parallel, tensor parallel, and
  FSDP-class execution
- backend-usable memory counters, cache controls, and debug tooling

The benefits not worth importing as primary design truth are:

- Python-first public API as the center of gravity
- Apple-specific global assumptions baked into every abstraction
- a compatibility story that outruns semantic correctness
- file format or launch-tool compatibility as a substitute for native truth

## Fit Against Current Psionic

| MLX area | Current Psionic posture | Assessment |
| --- | --- | --- |
| Tensor metadata and graph core | `psionic-core`, `psionic-ir`, and `psionic-compiler` already own explicit dtype, layout, graph, lowering, and cache identity | Strong substrate fit |
| Lazy user-facing array surface | No current general-purpose public `Array`/`Tensor` facade with MLX-style `eval` semantics | Missing layer |
| Reverse-mode autodiff | `psionic-ir` already owns real reverse-mode AD and typed refusal | Strong partial fit |
| JVP, VJP, vmap, checkpoint, custom transform hooks | Not a first-class public Psionic surface today | Major gap |
| Compile as callable transform | Compiler exists, but not MLX-style compiled-function semantics with public purity/debug story | Major gap |
| Module/state tree semantics | `psionic-models` has model families and `psionic-train` has training state, but not a general `Module` tree substrate | Major gap |
| Optimizer trees and schedulers | `psionic-train` already has reusable SGD, Adam, AdamW, LARS, and LAMB contracts | Good lower-layer fit, missing framework shell |
| Weight and checkpoint IO | `psionic-train::model_io` and `psionic-models` already cover safetensors and GGUF strongly | Strong fit |
| Array save/load surface | No MLX-class `.npy` / `.npz` style general array API surface | Gap |
| Function export/import | No current `.mlxfn`-class or Psionic-native graph export layer above the IR | Gap |
| Distributed collectives | `psionic-collectives`, `psionic-cluster`, and train distributed optimizer contracts already exist | Strong lower-layer fit |
| Framework-level distributed helpers | No direct equivalent yet to MLX group/init, no-op singleton semantics, sharded linear helpers, or `average_gradients` API shape | Gap above the substrate |
| CPU / Metal / CUDA backends | Existing backend crates already match the needed owner split | Strong fit |
| Unified-memory-aware scheduling | Psionic can support this on Apple lanes, but it is not yet expressed as a first-class framework capability | Partial fit |
| Memory and cache tooling | `psionic-runtime` already has substantial cache and residency truth | Good fit, missing framework-facing wrapper |
| Test and parity harness | Framework-core acceptance exists, but there is no MLX parity matrix | Gap |

## Port Directly Versus Redesign Deliberately

### Port directly

These should be ported in spirit and behavior with little philosophical
resistance:

- lazy arrays and explicit evaluation
- function transforms
- compile-as-transform
- parameter-tree modules
- tree-aware optimizer families
- array and weight serialization breadth
- distributed helper semantics above collectives
- backend debug and memory instrumentation

### Redesign deliberately

These should not be copied literally.

#### 1. Unified memory should be a capability, not a universal law

MLX can say "arrays live in shared memory" because Apple silicon is its design
center. Psionic cannot make that a global invariant across CUDA, AMD, or
sandboxed distributed execution.

So Psionic should expose something like:

- unified-memory-capable device descriptors
- stream and dependency rules that can exploit shared memory where available
- explicit fallback behavior where shared memory is not available

This keeps the benefit without lying on non-Apple backends.

#### 2. `.mlxfn` should not become Psionic's primary native artifact

MLX function export is useful, but Psionic already owns stronger native truth:

- graph identity
- compile identity
- runtime manifests
- replay receipts
- proof bundles

So the right order is:

1. build a Psionic-native graph/function artifact surface first
2. optionally add `.mlxfn` import/export compatibility later

Otherwise the framework will inherit an artifact story that is weaker than the
rest of Psionic.

#### 3. `mlx.launch` should map onto Psionic control and cluster truth, not replace it

MLX's distributed launcher is practical, but Psionic already owns:

- cluster topology truth
- sandboxed execution
- datastream and artifact staging
- app-owned operator paths

So the right analogue is:

- a thin developer or operator CLI for framework-level distributed jobs
- implemented on top of `psionic-cluster`, `psionic-collectives`,
  `psionic-sandbox`, and existing control surfaces

not:

- a new MLX-shaped standalone launch world that bypasses Psionic's execution
  truth

#### 4. Python, C, and Swift compatibility should come late

The current repo wants Rust-native substrate. A Python-first compatibility push
too early would recreate exactly the layering drift Psionic docs warn about.

So:

- Rust-native semantics first
- optional compatibility layers second

#### 5. Psionic should keep its system-truth advantage

MLX is primarily a research framework. Psionic is trying to be a reusable
execution substrate with receipts, manifests, policy surfaces, train/eval
truth, and provider/runtime evidence.

That means:

- MLX should shape the framework surface
- Psionic should keep owning the machine-legible truth above that surface

## Recommended Crate And Ownership Shape

Whether this lands as new crates or as carefully separated new modules, the
semantic split should look like this.

| Proposed surface | Recommended owner | Reason |
| --- | --- | --- |
| `psionic-array` | `crates/psionic/*` above `psionic-core` and `psionic-ir` | Public lazy array facade should not bloat `psionic-core` |
| `psionic-transforms` | `crates/psionic/*` above `psionic-ir` and `psionic-compiler` | `grad`, `vjp`, `jvp`, `vmap`, `checkpoint`, and compile need a public semantics layer |
| `psionic-nn` | `crates/psionic/*` above array/transforms | Module tree, layers, losses, initializers, and quantized layer wrappers belong above the core |
| `psionic-optimizers` | `crates/psionic/*` above `psionic-train` primitives | Reuse train optimizer math, but expose a framework-wide API rather than a train-only one |
| `psionic-export` or `psionic-fn` | `crates/psionic/*` above IR/compiler/runtime | Function export/import should be a first-class framework surface |
| `psionic-distributed` | `crates/psionic/*` above `psionic-collectives` and `psionic-cluster` | MLX-style framework distributed groups and helpers should not leak low-level transport details |
| `psionic-mlx-compat` | optional later compatibility crate | MLX naming and artifact compatibility should stay optional and separated from native truth |

### Important boundary rule

None of the above should move:

- app UX into Psionic
- wallet or market logic into Psionic
- kernel or Nexus authority logic into Psionic

This is fully consistent with
[docs/OWNERSHIP.md](/Users/christopherdavid/code/openagents/docs/OWNERSHIP.md)
and [crates/psionic/docs/ROADMAP.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/ROADMAP.md).

## How Current Psionic Pieces Should Be Reused

The right integration is not a rewrite. It is a layer-up reuse program.

### `psionic-core`

Keep as the minimal metadata substrate.

Extend for:

- more dtypes
- more layout and view rules
- stream and device capability descriptors
- storage and alias semantics needed by the array facade

Do not turn it into an MLX-shaped public facade crate.

### `psionic-ir`

Keep as the canonical graph and autodiff layer.

Extend for:

- wider operator coverage
- forward-mode hooks
- vmap rules
- custom transform registration
- graph export support
- side-effect and purity modeling needed by compile-as-transform

### `psionic-compiler`

Keep as the lowering and cache identity layer.

Extend for:

- public compiled-function wrappers
- shapeless or symbolic-shape compilation modes
- transform-aware cache keys
- more aggressive fusion and memory planning

### `psionic-runtime`

Keep as runtime truth.

Extend for:

- stream scheduling exposed to the public framework surface
- `eval` and `async_eval` semantics
- unified-memory-aware capability flags
- framework-facing memory and cache counters
- backend debug hooks

### `psionic-backend-cpu`, `psionic-backend-metal`, `psionic-backend-cuda`

Keep as backend owners.

Extend for:

- broad operator closure
- custom kernel interfaces
- quantized kernels
- stream/event/fence coverage
- MLX-class backend debug and profiling support

### `psionic-train`

Reuse what already exists:

- optimizer math
- distributed optimizer contracts
- checkpoint lineage
- model IO

But move framework-facing optimizer and module semantics above train-specific
run orchestration.

### `psionic-collectives` and `psionic-cluster`

Reuse as the transport and coordination substrate.

Add a framework-facing distributed layer above them that exposes:

- group init and split
- all-reduce or all-sum
- all-gather
- reduce-scatter
- send/recv
- framework sharding helpers

### `psionic-models` and `psionic-serve`

These should consume the future MLX-class framework layer, not masquerade as
that layer.

This is important because current Psionic model and serving crates are already
system-facing and product-adjacent in ways that a general ML framework surface
should not be.

## Phased Port Plan

### Phase 0: define the parity contract

Before writing major code, define one canonical target:

- "Rust-native MLX-class framework inside Psionic"

Create a new acceptance matrix that splits:

- array semantics
- transforms
- compile
- modules and state
- optimizers and schedulers
- export and serialization
- distributed semantics
- backend and tooling coverage

Without this, the program will drift into isolated backend wins.

### Phase 1: land the public lazy array surface

This is the first critical missing layer.

Deliverables:

- public lazy `Array` or equivalent facade
- explicit `eval` and `async_eval`
- device and stream routing
- graph-backed arithmetic and indexing operations
- dynamic graph construction with stable digests and replay identity

This phase should extend the existing framework-core acceptance bar rather than
invent a parallel one.

### Phase 2: land MLX-class transforms and compile

Deliverables:

- `grad`
- `value_and_grad`
- `vjp`
- `jvp`
- `vmap`
- `checkpoint`
- custom transform hooks
- compile as a callable transformation
- purity, side-effect, and debug posture

This is the phase where Psionic stops being "just a runtime substrate" and
starts feeling like an ML framework.

### Phase 3: land `nn`, parameter trees, and optimizer APIs

Deliverables:

- `Module`-class tree semantics
- parameter, buffer, frozen, and trainable posture
- module save/load rules
- initializers
- loss functions
- scheduler families
- framework-level optimizer surface reusing `psionic-train` math where possible

This is also where MLX-style quantized layers and distributed layer wrappers
should start landing.

### Phase 4: land export and serialization closure

Deliverables:

- array save/load surface
- safetensors and GGUF interop through the framework facade
- native graph or function export/import artifact
- optional later `.mlxfn` compatibility lane

This phase should integrate with Psionic's existing manifest and receipt truth,
not replace it.

### Phase 5: land framework-level distributed semantics

Deliverables:

- framework-visible distributed groups
- singleton no-op semantics when world size is `1`
- all-sum, all-gather, reduce-scatter, send, recv
- gradient averaging helpers
- tensor-parallel sharded linear helpers
- FSDP-class apply-gradients helper
- thin developer or operator launch surface mapped onto Psionic cluster truth

This phase should reuse `psionic-collectives`, `psionic-cluster`,
`psionic-sandbox`, and train distributed-optimizer contracts rather than
rebuilding them.

### Phase 6: backend, operator, and tooling closure

Deliverables:

- broad op-family closure on CPU
- broad accelerated coverage on Metal and CUDA
- memory counters and cache controls
- backend availability checks
- profiling and debug capture hooks
- custom kernel authoring surface

This is where the MLX-class promise becomes practical on Apple and Linux
machines, not just semantically interesting.

### Phase 7: optional MLX compatibility shell

Only after the above is real:

- add MLX naming adapters where useful
- optionally support `.mlxfn` import/export
- optionally add Python, C, or Swift bindings
- optionally run direct compatibility test suites against imported artifacts

This keeps compatibility from distorting the native architecture.

## Testing And Acceptance Strategy

The current framework-core acceptance matrix is a strong foundation, but a full
MLX port claim needs its own explicit oracle.

### Recommended acceptance sources

Port and adapt upstream MLX tests in phases.

Highest-value C++ test families:

- `array_tests.cpp`
- `autograd_tests.cpp`
- `compile_tests.cpp`
- `eval_tests.cpp`
- `export_import_tests.cpp`
- `ops_tests.cpp`
- `scheduler_tests.cpp`
- `vmap_tests.cpp`

Highest-value Python test families:

- `test_array.py`
- `test_autograd.py`
- `test_compile.py`
- `test_eval.py`
- `test_export_import.py`
- `test_memory.py`
- `test_nn.py`
- `test_optimizers.py`
- `test_quantized.py`
- `test_vmap.py`
- `mlx_distributed_tests.py`
- backend-specific distributed tests once the framework distributed layer exists

### Acceptance rules

Psionic should keep its current discipline:

- CPU reference truth first
- accelerated backends through explicit capability widening
- no silent fallback that changes semantics
- replay and cache identity visible in artifacts and diagnostics

That means MLX parity testing should be layered onto:

- `FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md`
- backend-specific capability matrices
- train and collectives truth where distributed semantics are involved

## What Makes Immediate Integration Sense

The following work makes sense now because it matches Psionic's current shape:

- add a public lazy array facade above current core and IR
- promote compiler and autodiff internals into public transform APIs
- extract framework-wide optimizer and module semantics from train-only usage
- add function export/import and general array serialization
- build a framework distributed layer on top of collectives and cluster truth
- expand backend tooling and operator coverage

The following work does not make sense as the first move:

- port Python wrappers before the Rust core exists
- make `.mlxfn` the primary artifact before a Psionic-native function artifact
  exists
- bypass `psionic-cluster` with an MLX-style launcher clone
- treat Metal-specific wins as equivalent to MLX closure

## Main Risks

### 1. API-shell-first failure

If the program starts with MLX naming and bindings before semantic closure, it
will produce a fake port.

### 2. Backend-local drift

If the program becomes "make Metal faster" without landing the framework
surface, it will produce another backend lane, not an ML framework.

### 3. Over-copying Apple-specific assumptions

If unified memory, launch tooling, or research-only defaults are copied
literally, Psionic will become less honest on CUDA, AMD, and clustered paths.

### 4. Underusing current Psionic substrate

If the port ignores existing work in:

- `psionic-collectives`
- `psionic-cluster`
- `psionic-datastream`
- `psionic-sandbox`
- `psionic-train`
- `psionic-runtime`

then the result will be weaker than both MLX and current Psionic.

## Bottom Line

Psionic should not "become Apple's MLX repo in Rust."

It should do something stronger:

- port MLX's framework benefits and semantics into Rust
- keep Psionic's current core, runtime, collectives, train, and evidence layers
- add the missing public framework layer above them
- delay compatibility shells until the native semantics are honest

The right one-line target is:

> make Psionic MLX-class as a Rust-native ML framework, while keeping receipts,
> manifests, replay, cluster truth, sandbox truth, and train/eval evidence
> explicitly Psionic-native.

That is a large program, but it is coherent with the current Psionic roadmap,
crate ownership, and existing substrate in a way a literal line-by-line port
would not be.
