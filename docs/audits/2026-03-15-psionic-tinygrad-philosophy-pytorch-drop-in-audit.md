# 2026-03-15 Psionic Tinygrad Philosophy And PyTorch Drop-In Audit

## Intent

This audit answers a synthesis question created by the earlier Tinygrad and
PyTorch audits:

> if Psionic should absorb the best of George Hotz's Tinygrad philosophy, but
> still move toward being a practical drop-in replacement for PyTorch for most
> AI workloads, what architectural choices should it actually make?

This is not a "pick one side" document.

It is a design-choice document.

The aim is to say clearly:

- what Tinygrad philosophy Psionic should import
- what PyTorch semantics Psionic should still target
- which parts must remain explicitly Psionic-native
- how to avoid collapsing into either:
  - a Rust clone of Tinygrad
  - or a Rust clone of PyTorch's whole historical bulk

## Relationship To Prior Audits

This audit is a follow-on to:

- [2026-03-14-tinygrad-parity-target-for-psionic-audit.md](/Users/christopherdavid/code/openagents/docs/audits/2026-03-14-tinygrad-parity-target-for-psionic-audit.md)
- [2026-03-15-tinygrad-test-suite-port-audit.md](/Users/christopherdavid/code/openagents/docs/audits/2026-03-15-tinygrad-test-suite-port-audit.md)
- [2026-03-15-pytorch-test-suite-port-audit.md](/Users/christopherdavid/code/openagents/docs/audits/2026-03-15-pytorch-test-suite-port-audit.md)
- [2026-03-15-full-pytorch-port-to-rust-in-psionic-audit.md](/Users/christopherdavid/code/openagents/docs/audits/2026-03-15-full-pytorch-port-to-rust-in-psionic-audit.md)

Those documents already established:

- Tinygrad is the best compact reference for a visible ML stack
- PyTorch is the best oracle for framework semantics and compatibility pressure
- Psionic already has the beginnings of a framework core, but not broad
  framework completion

This audit adds the missing architectural stance:

> Tinygrad should shape the framework-core philosophy.
>
> PyTorch should shape the compatibility bar.
>
> Psionic should own the system truth above both.

## Scope

OpenAgents sources reviewed:

- [docs/MVP.md](/Users/christopherdavid/code/openagents/docs/MVP.md)
- [docs/OWNERSHIP.md](/Users/christopherdavid/code/openagents/docs/OWNERSHIP.md)
- [crates/psionic/README.md](/Users/christopherdavid/code/openagents/crates/psionic/README.md)
- [crates/psionic/docs/ARCHITECTURE.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/ARCHITECTURE.md)
- [crates/psionic/docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md)
- [crates/psionic/docs/TRAIN_SYSTEM.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/TRAIN_SYSTEM.md)
- [crates/psionic/docs/plan.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/plan.md)
- [crates/psionic/docs/deep-research-tinygrad.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/deep-research-tinygrad.md)
- [crates/psionic/psionic-core/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-core/src/lib.rs)
- [crates/psionic/psionic-ir/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-ir/src/lib.rs)
- [crates/psionic/psionic-ir/src/autodiff.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-ir/src/autodiff.rs)
- [crates/psionic/psionic-compiler/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-compiler/src/lib.rs)
- [crates/psionic/psionic-runtime/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-runtime/src/lib.rs)
- [crates/psionic/psionic-models/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-models/src/lib.rs)
- [crates/psionic/psionic-train/src/model_io.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-train/src/model_io.rs)
- [crates/psionic/psionic-train/src/optimizer.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-train/src/optimizer.rs)

Prior audit inputs reviewed:

- [2026-03-14-tinygrad-parity-target-for-psionic-audit.md](/Users/christopherdavid/code/openagents/docs/audits/2026-03-14-tinygrad-parity-target-for-psionic-audit.md)
- [2026-03-15-tinygrad-test-suite-port-audit.md](/Users/christopherdavid/code/openagents/docs/audits/2026-03-15-tinygrad-test-suite-port-audit.md)
- [2026-03-15-pytorch-test-suite-port-audit.md](/Users/christopherdavid/code/openagents/docs/audits/2026-03-15-pytorch-test-suite-port-audit.md)
- [2026-03-15-full-pytorch-port-to-rust-in-psionic-audit.md](/Users/christopherdavid/code/openagents/docs/audits/2026-03-15-full-pytorch-port-to-rust-in-psionic-audit.md)

Input premise also reviewed:

- the user-provided summary of George Hotz's Tinygrad philosophy and its cited
  primary-source themes

## Executive Summary

Psionic should not choose between Tinygrad and PyTorch.

It should split their roles cleanly:

- Tinygrad should be the model for how the framework core feels:
  - small
  - visible
  - hackable
  - backend-portable
  - compiler-first
- PyTorch should be the model for what the framework must eventually support
  for most real AI workloads:
  - tensor semantics
  - operator breadth
  - autograd correctness
  - module and state semantics
  - optimizer behavior
  - serialization and compatibility expectations
- Psionic should remain distinct in the layers neither Tinygrad nor PyTorch are
  designed to own:
  - receipts
  - replay truth
  - runtime manifests
  - topology truth
  - sandbox policy
  - clustered execution
  - provider capability and authority seams

So the right architectural answer is not:

- "build Rust Tinygrad"
- "port all of PyTorch into Rust immediately"

The right answer is:

> make Psionic tinygrad-shaped in framework-core philosophy, PyTorch-credible
> in semantics and compatibility, and unapologetically Psionic-native in
> system truth.

## One-Line Synthesis

Psionic should be:

- tinygrad-like at the bottom
- PyTorch-like in the middle
- Psionic-like at the top

That is the architecture.

## What Hotz Is Right About For Psionic

The user-provided summary is directionally correct on the important point:
George Hotz is not mainly making a frontend-Python argument.

He is making:

- a hardware-portability argument
- a compiler visibility argument
- a complexity-discipline argument
- a debug-the-machine argument

Those are all relevant to Psionic.

### 1. Hardware portability should be a first-class goal

Hotz's strongest point is that framework architecture should make new hardware
easier to bring up, not harder.

That maps directly onto Psionic because Psionic already has explicit backend
families:

- CPU
- Metal
- CUDA
- AMD KFD
- AMD userspace
- Apple FM bridge

Psionic should preserve and deepen that posture.

The important design choice is:

- backends must stay explicit
- backend capability and refusal surfaces must stay explicit
- backend bring-up should target a small visible core, not a giant hidden stack

### 2. The compiler and IR should remain visible

Hotz is also right that "can one person still see the path from model code to
kernel execution?" is not cosmetic.

It is a governance question.

Psionic already leans the right way:

- `psionic-core` keeps foundational types small
- `psionic-ir` exposes a compact graph and op model
- `psionic-compiler` exposes a visible lowering boundary
- `psionic-runtime` keeps runtime descriptors and diagnostics typed

That should remain a hard constraint as Psionic grows.

### 3. Laziness, fusion, and plan reuse should be native to the mental model

Tinygrad's default mental model is that the system reasons about the graph
before materializing work.

Psionic should keep that instinct.

Even if Psionic exposes an eager-friendly surface later, the internals should
still prefer:

- explicit graphs
- explicit plans
- stable plan digests
- cacheable compiled paths
- visible fusion and lowering choices

This is already consistent with:

- `Graph`
- `ExecutionPlan`
- topology-sensitive compile digests
- replay fixtures and plan identity

### 4. Kernel and runtime visibility matters more than frontend cosmetics

Tinygrad's emphasis on seeing what kernels ran, what was fused, and what the
runtime actually did is strategically right for Psionic.

Psionic should therefore continue to prefer:

- explicit execution plans
- machine-legible runtime diagnostics
- resource and cache reports
- proof-bearing execution receipts
- replay-stable program identity

That is not "extra product logic."

That is one of Psionic's core advantages over both Tinygrad and PyTorch.

### 5. Smallness should be treated as an architectural constraint, not branding

This is probably the most important Hotz lesson.

The point of a small IR, a small visible lowering path, and a narrow backend
contract is not aesthetic minimalism.

It is that:

- backends stay understandable
- invariants stay testable
- performance work stays debuggable
- portability stays tractable

Psionic should protect that smallness in its framework core even if the overall
repo grows much larger around it.

## What Psionic Should Not Import From Tinygrad

Tinygrad's philosophy is useful.

Tinygrad's whole shape is not.

Psionic should not copy:

- Python-first architecture as the durable execution truth
- environment variables as the long-term runtime control plane
- example-server ergonomics as product contracts
- "small enough repo" as an excuse for weak module or serialization semantics
- research-repo looseness around compatibility surfaces

That is exactly why [crates/psionic/docs/plan.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/plan.md)
is right to say:

> the goal should not be "Rust tinygrad"

That line should remain true.

## Where Current Psionic Already Looks More Tinygrad-Like Than PyTorch-Like

Today Psionic already feels closer to Tinygrad than to PyTorch in a few
important ways.

### 1. The core types are still compact

[psionic-core/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-core/src/lib.rs)
defines a small engine-facing type set:

- `TensorId`
- `DType`
- `QuantizationMode`
- `BackendExtensionKind`
- `BackendExtensionOp`

That is much closer to Tinygrad's "small visible core" instinct than to
PyTorch's enormous substrate.

### 2. The graph surface is still legible

[psionic-ir/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-ir/src/lib.rs)
still exposes a visible graph with a compact `OpKind` set:

- input
- constant
- detach
- add
- mul
- matmul
- reshape and movement ops
- reduce
- backend extension ops

That is exactly the kind of visible primitive surface Psionic should preserve.

### 3. The compiler boundary is still honest

[psionic-compiler/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-compiler/src/lib.rs)
is still a small lowering interface with:

- explicit lowering passes
- plan builders
- compiled execution plans
- stable digests over plan plus topology

That is a good architectural sign.

### 4. The runtime already cares about typed truth

[psionic-runtime/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-runtime/src/lib.rs)
already owns:

- device descriptors
- capability qualifiers
- typed local-runtime diagnostics
- proof and validation modules
- local multi-device truth

This is where Psionic should remain intentionally broader than Tinygrad.

### 5. Model IO and optimizer state are already explicit subsystems

[psionic-models/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-models/src/lib.rs),
[psionic-train/src/model_io.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-train/src/model_io.rs),
and [psionic-train/src/optimizer.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-train/src/optimizer.rs)
already make two important design choices:

- model and state IO are explicit, typed surfaces
- optimizer logic is explicit, reusable, and inspectable

Those are good choices.

They are more in line with "visible framework machinery" than with a hidden
monolith.

## Where Psionic Must Become Much More PyTorch-Like

The current framework-core posture is promising, but still far too narrow to be
called a practical PyTorch replacement for most AI workloads.

That broader bar still comes from PyTorch, not Tinygrad.

### 1. Tensor semantics need far more breadth

The current small core is good.

It is not enough.

To be a real drop-in for most workloads, Psionic still needs much broader
coverage of:

- views and aliasing semantics
- dtype promotion and casting rules
- indexing behavior
- reductions and shape ops
- mutation semantics where compatibility demands them
- meta or fake execution for shape-only reasoning

This is a PyTorch-style pressure, not a Tinygrad-style one.

### 2. Autograd needs broader operator coverage and stronger edge semantics

The repo now has real autodiff substrate.

That is important.

But PyTorch remains the standard for:

- breadth of gradient definitions
- shared-path accumulation behavior
- detach and no-grad semantics
- edge-case mutation and alias interaction
- higher-level transform expectations

If Psionic wants to run most PyTorch-authored workloads, it needs PyTorch-level
semantic discipline here even if the implementation stays tinygrad-shaped.

### 3. Module and state-tree behavior cannot stay underspecified

This is where Tinygrad is too small to be the final oracle.

A practical PyTorch replacement needs:

- trainable parameter containers
- buffer semantics
- module composition
- deterministic state-tree naming
- strict and non-strict load behavior
- portable state-dict expectations

This is one of the biggest current architectural gaps.

### 4. Serialization and checkpoint interop matter

Tinygrad is useful on GGUF and direct model IO.

PyTorch is much more useful on:

- checkpoint expectations
- state restore semantics
- optimizer state roundtrip
- module state compatibility discipline

Psionic does not need to copy all of `torch.save` history.

But it does need a compatibility story strong enough that "drop-in for most AI
workloads" is not empty language.

### 5. Test architecture must become PyTorch-like even if the core does not

One major conclusion from the prior PyTorch audit should carry forward:

Psionic should borrow PyTorch's test architecture more than its product shape.

That means:

- operator registries
- declarative conformance matrices
- module registries
- optimizer registries
- fake and symbolic execution test surfaces

This is how a small core stays honest while growing broad semantics.

## The Right Architectural Split

Psionic should explicitly adopt a three-layer framework architecture.

## Layer 1: Tinygrad-Shaped Framework Core

This layer should remain small, visible, and backend-portable.

It should include:

- tensor identity and metadata
- primitive graph IR
- autodiff substrate
- lowering and scheduling
- compiled execution plans
- backend traits
- runtime execution and profiling hooks

In current crate terms, this is mostly:

- `psionic-core`
- `psionic-ir`
- `psionic-compiler`
- `psionic-runtime`

This layer should optimize for:

- understandability
- deterministic replay
- backend bring-up
- inspectable lowering
- explicit refusal semantics

This is where Tinygrad philosophy belongs most directly.

## Layer 2: PyTorch-Credible Semantics And Compatibility

This layer is what turns a compact framework core into something that can
actually stand in for PyTorch on real workloads.

It should include:

- a broad operator registry and dispatch story
- fake or meta execution for symbolic shapes and compile planning
- module or state-tree semantics
- optimizer families and scheduler behavior
- serialization and checkpoint compatibility surfaces
- declarative parity harnesses against PyTorch

This layer is only partly present today.

It is the layer Psionic must build next if "drop-in replacement for most AI
workloads" is the goal.

Most importantly:

- this layer should be broad
- but it should not bloat Layer 1

That means compatibility surfaces should sit above the tiny visible core, not
inside it.

## Layer 3: Psionic-Native System Truth

This is the layer neither Tinygrad nor PyTorch is designed to own.

It should remain explicitly Psionic-native:

- receipts
- manifests
- replay truth
- runtime proofs
- topology plans
- local and clustered execution truth
- sandbox profiles
- provider capability surfaces
- evaluation, training, and accepted-outcome seams

In current crate terms this is already spread across:

- `psionic-datastream`
- `psionic-cluster`
- `psionic-net`
- `psionic-sandbox`
- `psionic-provider`
- `psionic-eval`
- `psionic-train`

This layer is a feature, not bloat.

It is the reason Psionic should not become merely "PyTorch rewritten in Rust."

## The Key Architectural Choice: Keep Compatibility Above The Core

If Psionic tries to become PyTorch-compatible by stuffing all compatibility
behavior directly into the core IR and runtime types, it will lose the design
advantage Hotz is pointing at.

If Psionic tries to stay tiny and refuses to grow broad semantics, it will
never be a practical PyTorch replacement.

So the only stable choice is:

- keep the core minimal and visible
- add broad semantics in a higher compatibility layer
- validate that higher layer continuously against PyTorch

This is the middle path that preserves both goals.

## Architectural Non-Negotiables

If Psionic follows this path, several rules should become explicit.

### 1. Do not let product or provider logic leak into framework-core crates

This is already mandated by [docs/OWNERSHIP.md](/Users/christopherdavid/code/openagents/docs/OWNERSHIP.md).

It matters even more if Psionic grows into a broader framework.

### 2. Do not let PyTorch compatibility dictate the core IR shape

Psionic should target PyTorch semantics.

It should not feel forced to clone PyTorch's internal structure line by line.

The core IR should remain compact enough that backend bring-up and lowering
work stay legible.

### 3. Do not let tinygrad-style minimalism excuse missing semantics

Tinygrad philosophy is a design constraint.

It is not a waiver from:

- operator breadth
- module semantics
- state-dict discipline
- serialization interop
- compatibility harnesses

### 4. Do not make environment variables the durable control plane

Tinygrad's env-var culture is fine for a research framework.

Psionic should continue to prefer:

- typed backend descriptors
- typed runtime configuration
- machine-legible capability truth
- explicit health and refusal surfaces

### 5. Do not add a Python-compatibility shell before the substrate is real

This is the biggest trap.

A PyTorch-looking surface without:

- broad ops
- module behavior
- optimizer behavior
- checkpoint semantics
- symbolic shape infrastructure

is not a drop-in replacement.

It is a demo.

## What "Drop-In Replacement For Most AI Workloads" Should Mean

This phrase needs a narrower, honest meaning.

It should not mean:

- full upstream PyTorch feature closure
- all legacy TorchScript behavior
- every backend and extension ecosystem feature
- every process-group and distributed API shape
- every quantization and mobile edge case

It should mean:

- common tensor workloads run with PyTorch-credible semantics
- common training workloads have working autodiff, modules, optimizers, and
  state restore
- common LLM and embedding inference workloads run directly
- common checkpoint and safetensors interop is practical
- graph capture, lowering, and caching are good enough for modern compiled
  execution
- at least one Python-interop path eventually exists for existing model code

That is still a very large target.

But it is honest and tractable.

## Recommended Build Order

If the actual goal is "Tinygrad philosophy plus PyTorch drop-in direction,"
then the build order should be:

### Stage 1: freeze the tiny visible core as a non-negotiable design boundary

Keep:

- primitive ops compact
- backend extensions explicit
- plan digests explicit
- runtime descriptors explicit

Do not allow compatibility breadth to muddy this layer.

### Stage 2: build a real operator registry and meta-execution layer

This is the first big PyTorch-shaped expansion Psionic needs.

Without it, compatibility growth will stay ad hoc.

### Stage 3: import PyTorch-derived op, module, and optimizer parity matrices

This follows the conclusions from the earlier test-suite audits.

Tinygrad and PyTorch should both remain oracles:

- Tinygrad for runtime, GGUF, JIT, and serving patterns
- PyTorch for semantics breadth and parity expectations

### Stage 4: build the missing module and state-tree layer

This is where "drop-in for most workloads" becomes more real.

A framework with ops and autodiff but weak module/state behavior still falls
short of most training and inference code.

### Stage 5: make checkpoint and serialization interop practical

This does not require cloning PyTorch's full historical artifact surface.

It does require a compatibility story that real users can rely on.

### Stage 6: widen backend coverage without widening the core abstraction

This is the Hotz discipline point:

- port more backends
- keep the core simple
- keep backend capability truth explicit

### Stage 7: only then consider a PyTorch-facing compatibility shell

If a Python or Torch-like frontend exists, it should sit on top of a real
substrate, not substitute for one.

## Recommendation

Psionic should explicitly adopt this rule:

> the framework core should remain tinygrad-philosophy-driven, while the
> semantics and compatibility bar should remain PyTorch-driven.

In practice that means:

- preserve a small, inspectable, backend-portable core
- aggressively widen semantics above that core
- use PyTorch as the main oracle for compatibility and completeness
- keep receipts, replay, topology, sandbox, and provider truth fully
  Psionic-native

That is how Psionic can move toward being a practical replacement for most
PyTorch workloads without losing the architectural virtues that make Tinygrad
interesting in the first place.

## Bottom Line

George Hotz's best lesson for Psionic is not "copy Tinygrad."

It is:

> keep the engine small enough that hardware bring-up, lowering, and debugging
> stay visible.

PyTorch's best lesson for Psionic is not "copy all of PyTorch."

It is:

> broad semantics, module behavior, optimizer behavior, and compatibility
> discipline are what make a framework truly usable.

So the right Psionic architecture is:

- tinygrad discipline in the core
- PyTorch coverage above the core
- Psionic-native truth around the core

That is the path that can plausibly produce a Rust-native framework that is
both understandable to build and credible to use.
