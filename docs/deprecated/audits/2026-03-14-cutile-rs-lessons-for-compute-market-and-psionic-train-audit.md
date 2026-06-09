# 2026-03-14 cuTile-rs Lessons For Compute Market And Psionic Train Audit

## Intent

This audit answers a narrower question than the broader Psionic and kernel
docs:

> after reading the local `~/code/cutile-rs` codebase, the current
> `docs/kernel/` compute-market material, the current `Psionic Train` spec, and
> the current open plus recently closed GitHub issue backlog, what should
> OpenAgents actually use from `cutile-rs`, if anything?

The useful answer is not:

- "vendor `cutile-rs` into OpenAgents"
- or "make `cutile-rs` the new compute-market substrate"
- or "treat `cutile-rs` as the missing Psionic train stack"

The useful answer is:

- `cutile-rs` is a strong local reference for a narrow class of CUDA-backend
  implementation patterns
- it is not a reference for compute-market authority, proof, settlement, train
  orchestration, or distributed system truth
- OpenAgents should borrow a few backend and compiler ideas from it, but should
  not adopt it as a direct dependency for the kernel or the train system

If reduced to one sentence:

> OpenAgents should treat `cutile-rs` as a CUDA backend and compiler-discipline
> reference for future `psionic-*` engine work, not as something the compute
> market or the train system should directly depend on.

## Scope

OpenAgents sources reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/kernel/economy-kernel.md`
- `docs/kernel/markets/compute-market.md`
- `docs/kernel/compute-training-authority.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
- `docs/audits/2026-03-13-intellect-lessons-for-psionic-train-audit.md`
- `docs/audits/2026-03-14-covenant-code-lessons-for-psionic-train-audit.md`
- `docs/audits/2026-03-14-tinygrad-parity-target-for-psionic-audit.md`

GitHub backlog reviewed on `2026-03-14` via `gh` CLI:

- `gh issue list --state open --limit 40`
- `gh issue list --state closed --limit 40`
- `gh api repos/OpenAgentsInc/openagents/issues/{3601,3604,3606,3564,3583,3586,3587}`

Note:

- `gh issue view` currently errors in this repo on deprecated classic-project
  metadata, so specific issue bodies were reviewed through `gh api` instead,
  still using the GitHub CLI as requested.

`cutile-rs` sources reviewed:

- `~/code/cutile-rs/README.md`
- `~/code/cutile-rs/Cargo.toml`
- `~/code/cutile-rs/cuda-async/{README.md,src/lib.rs,src/device_context.rs,src/scheduling_policies.rs}`
- `~/code/cutile-rs/cuda-core/{README.md,src/lib.rs}`
- `~/code/cutile-rs/cuda-tile-rs/README.md`
- `~/code/cutile-rs/cutile/{README.md,src/lib.rs,src/api.rs,src/tensor.rs,src/tile_kernel.rs}`
- `~/code/cutile-rs/cutile-compiler/{README.md,src/lib.rs,src/train_map.rs}`
- `~/code/cutile-rs/cutile-macro/src/lib.rs`
- `~/code/cutile-rs/cutile-examples/examples/{async_mlp.rs,dropout.rs,rms_norm.rs}`
- `~/code/cutile-rs/cutile-benchmarks/benches/{gemm.rs,fmha.rs}`
- `~/code/cutile-rs/cutile/tests/{error_quality.rs,span_source_location.rs}`
- `~/code/cutile-rs/cuda-async/tests/error_handling.rs`

## Executive Summary

As of `2026-03-14`, the current OpenAgents issue program is heavily focused on:

- `#3601` canonical tensor, shape, dtype, and device contracts
- `#3604` runtime split between compiled programs, allocators, and compilers
- `#3606` memory planning, schedule caching, method caching, and plan-stability
  diagnostics
- the just-closed train and authority work such as `#3564` and `#3583`

That backlog matters because it tells us where a foreign codebase is actually
allowed to help.

The right verdict is:

- the compute market should use nothing directly from `cutile-rs`
- `Psionic Train` should also not depend on `cutile-rs` directly
- a future CUDA backend inside `psionic-core`, `psionic-runtime`, or
  `psionic-compiler` may borrow a few concrete implementation patterns from
  `cutile-rs`

The patterns worth adapting are:

- explicit async device-operation DAGs and stream scheduling
- specialization-aware compiled-kernel cache keys
- compiler diagnostics and source-location testing discipline
- kernel microbenchmark structure for CUDA parity work

The parts OpenAgents should explicitly not adopt are:

- `cutile-rs` as a canonical tensor or train framework
- `cutile-rs` as compute-market truth
- `cutile-rs` as multi-device, cluster, validator, or checkpoint authority
- `cutile-rs` as a portability target, because it is tightly bound to Linux,
  NVIDIA, CUDA `13.2`, LLVM `21`, nightly Rust, and CUDA Tile MLIR

## What cuTile-rs Actually Is

`cutile-rs` is honest about being a research project.

What it actually implements is:

- a Rust DSL for tile-oriented CUDA kernels
- a Rust -> MLIR -> PTX/CUBIN compilation path
- an async CUDA host runtime with `DeviceOperation`, `DeviceFuture`, per-device
  contexts, and stream scheduling
- a GPU `Tensor` plus `Partition` abstraction for safe tile-partitioned kernel
  launch
- a small set of examples and microbenchmarks for GEMM, flash attention,
  softmax, RMSNorm, dropout, and toy async MLP flows

What it does not implement is at least as important:

- no autodiff system
- no reusable optimizer library
- no train orchestration
- no checkpoint family or lineage protocol
- no validator service in the OpenAgents sense
- no environment-package registry
- no receipts, proofs, settlement, or market authority
- no credible distributed multi-device or cluster truth

One small but important example of over-reading risk:

- `cutile-compiler/src/train_map.rs` is just a scoped compiler map type
- it is not evidence of a training subsystem

So the repo is useful, but only if we treat it as what it is: a local CUDA
compiler and runtime experiment.

## What OpenAgents Should Adapt

### 1. Async device-operation and stream-scheduling discipline

The strongest reusable pattern in `cutile-rs` is the split in `cuda-async`
between:

- device operations
- scheduled futures
- scheduling policies
- per-device execution context

That maps well to the current Psionic engine backlog, especially:

- `#3604` runtime split
- `#3608` same-type local multi-device runners

The reason this is useful is not that OpenAgents should copy the exact APIs.
The useful lesson is:

> local CUDA work should be modeled as typed execution plans with explicit
> scheduling, not as opaque "run this kernel now" helpers.

That is a good fit for:

- `psionic-runtime`
- future backend-specific crates such as a CUDA backend
- local-first train or inference execution under typed runtime contracts

It is not a fit for:

- `openagents-kernel-*`
- `apps/nexus-control`
- market authority or settlement logic

### 2. Specialization-aware kernel cache keys and invalidation posture

`cutile/src/tile_kernel.rs` has a concrete compile-cache story:

- cache keys include function identity
- type and const generics are part of the key
- stride layouts are part of the key
- launch-grid specialization is part of the key
- compiled functions are cached per device

That is directly relevant to `#3606`.

OpenAgents should adapt the idea, not the implementation:

- plan or program caches should be keyed by explicit execution assumptions
- invalidation should be typed when shape, dtype, backend, topology, or layout
  changes
- cache hits and misses should be inspectable enough to feed future evidence and
  replay tooling

This is one of the few places where `cutile-rs` lines up very well with the
current Psionic compiler direction.

### 3. Compiler diagnostics and conformance testing discipline

The most underrated thing in `cutile-rs` is its test discipline around compiler
quality:

- `cutile/tests/error_quality.rs`
- `cutile/tests/span_source_location.rs`
- `cuda-async/tests/error_handling.rs`

Those tests enforce:

- good user-facing error messages
- no leakage of internal compiler names
- exact source-location reporting
- stable formatting invariants
- basic runtime error coverage

That is worth adapting almost directly into Psionic compiler work, especially
for:

- `#3607` replay and program-identity regression gates
- broader Psionic compiler and runtime hardening

OpenAgents increasingly wants:

- machine-legible receipts
- replay-safe compiler behavior
- operator-visible diagnostics

That only works if the compiler and runtime are testable and legible. `cutile-rs`
is a good reference for that narrow discipline.

### 4. Kernel microbenchmark harness structure

The GEMM, FMHA, RMSNorm, and softmax benchmarks are not a train system, but
they are useful as examples of:

- stable benchmark inputs
- explicit tile-shape hyperparameters
- warmed runtime execution
- throughput-oriented CUDA benchmarking

This is useful for future Psionic CUDA bring-up and parity work.

It is not a reason to make `cutile-rs` part of the product tree.

The right adaptation is:

- use similar benchmark structure when closing CUDA backend gaps
- keep those benchmarks under Psionic-owned acceptance and perf harnesses

### 5. Temporary host-device interop patterns, but only as a migration seam

`cutile-rs` has a practical bridge between GPU tensors and `candle_core::Tensor`
for copy-to-device and copy-to-host flows.

That can be useful as a temporary bring-up seam for:

- experiments
- import/export adapters
- backend validation during early CUDA work

But it should stay temporary.

Why:

- `#3601` explicitly wants a canonical Psionic tensor contract
- `#3587` wants Rust-native model and tokenizer IO contracts

So Candle interop may help during experiments, but it should not become the
canonical OpenAgents tensor truth.

## What OpenAgents Should Not Use

### 1. Do not use cuTile-rs in the compute market itself

The compute market docs are explicit that mature compute products must become:

- receipt-bearing
- proof-aware
- validator-aware
- environment-bound
- operator-inspectable
- authority-settled

`cutile-rs` does none of that.

It has no objects corresponding to:

- `ComputeProduct`
- `CapacityLot`
- `DeliveryProof`
- validator challenge lifecycle
- accepted outcomes
- settlement receipts

So the compute market should use none of it directly. At most, a future Psionic
CUDA backend influenced by `cutile-rs` may produce execution facts that the
market later consumes. That is an indirect relation only.

### 2. Do not use cuTile-rs as Psionic Train architecture

`Psionic Train` now owns:

- run graphs
- windows
- rollout artifacts
- trainer batches
- checkpoint pointers and manifests
- orchestrator state
- validator verdicts
- environment and eval linkage

`cutile-rs` owns none of that.

It is not a train control plane, and it does not narrow the open train issues
such as:

- `#3586` distributed optimizer, precision, and memory sharding
- `#3587` model IO and artifact portability

So `Psionic Train` should not import it as a train dependency or design center.

### 3. Do not use cuTile-rs as canonical tensor, autodiff, or optimizer truth

This is the most important constraint from the current issue backlog.

The open issues are trying to make Psionic into a full Rust-native ML stack:

- tensor semantics
- autodiff
- optimizer primitives
- runtime contracts
- schedule and method caches

`cutile-rs` currently offers:

- a local GPU tensor wrapper
- partition semantics
- kernel-launch ergonomics

It does not offer:

- canonical eager/lazy tensor semantics
- reverse-mode autodiff
- optimizer families
- reusable model-state trees

So it can inform a backend, but not the framework core.

### 4. Do not trust its multi-device story as a Psionic cluster reference

`cutile-rs` describes multiple-device async execution in its README, but the
actual code still shows obvious immaturity:

- `DEFAULT_NUM_DEVICES` is `1`
- device detection is still a TODO
- the async MLP example literally says "Pretend we have multiple devices..."

That is enough to disqualify it as a cluster or distributed-train reference for
OpenAgents.

Psionic already has stronger native ownership for:

- cluster topology truth
- collective cadence
- elastic membership
- ordered cluster state

Those areas should continue to be driven by Psionic-native architecture, not by
`cutile-rs`.

### 5. Do not take on its portability and toolchain constraints as product truth

`cutile-rs` currently requires:

- Linux
- NVIDIA GPUs
- CUDA `13.2`
- LLVM `21` with MLIR
- nightly Rust for some features
- CUDA Tile dialect tooling

That is far too narrow to become OpenAgents product truth.

The current OpenAgents tree is explicitly wider than one NVIDIA-only CUDA stack:

- Apple Silicon and Apple FM work exists
- Metal work exists
- CPU and broader portable-runtime expectations exist

So even where `cutile-rs` is useful, it should stay isolated behind backend
experiments and never set the baseline requirements for Psionic or the compute
market.

### 6. Be careful about direct dependency and licensing drag

Most of `cutile-rs` is Apache `2.0`, but `cuda-bindings` is under the NVIDIA
software license and the stack depends on CUDA Tile MLIR infrastructure.

That does not make it unusable for experiments.

It does make it a bad candidate for casual vendoring into core OpenAgents
crates, especially when the likely value is design inspiration rather than
source reuse.

## Backlog-Aligned Verdict

As of `2026-03-14`, the current backlog says OpenAgents is trying to finish the
ML-framework and train substrate in this order:

- define tensor truth
- define runtime truth
- define compiler and cache truth
- then widen multi-device, replay, and train behavior

That means the right use of `cutile-rs` is very narrow and very local:

- use it as a reference while implementing a future CUDA backend or compiler
  path in `psionic-core`, `psionic-runtime`, or `psionic-compiler`
- borrow test and cache ideas from it
- borrow some benchmark structure from it

It also means the wrong use is clear:

- do not wire it into the compute market
- do not wire it into kernel or Nexus authority
- do not make it the Psionic train control plane
- do not let it define Psionic tensor or model architecture

## Final Recommendation

OpenAgents should not add `cutile-rs` as a direct dependency for the compute
market or the train system.

OpenAgents should keep three very specific follow-up ideas from it:

- adapt its explicit device-operation and scheduling split when landing the
  Psionic runtime contract
- adapt its specialization-aware compile-cache and invalidation posture when
  landing Psionic compiler cache work
- adapt its compiler error and source-location test discipline for Psionic
  compiler conformance

Everything else should remain out of scope unless a future CUDA-only backend
spike proves a clearly bounded benefit inside Psionic-owned engine crates.
