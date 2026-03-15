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

## Objective

Build Psionic into a full Rust-native compute and ML library with:

- a small, visible, backend-portable framework core
- PyTorch-credible semantics for most practical AI workloads
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
- fake or meta execution
- module and state-tree semantics
- optimizer and scheduler breadth
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

## Success Bar

Psionic should be judged against four progressively stronger claims.

### Claim 1: serious framework core

This means the framework-core acceptance matrix is honestly green at meaningful
breadth, not only at representative happy-paths.

### Claim 2: usable ML library

This means common inference and training workloads can run with:

- broad tensor and autodiff semantics
- real module/state behavior
- real optimizer behavior
- real model IO and checkpoint restore

### Claim 3: truthful execution substrate

This means Psionic can honestly expose:

- backend capability
- topology truth
- manifests and receipts
- replay identity
- router and cache behavior
- cluster and sandbox behavior

### Claim 4: practical PyTorch replacement for most AI workloads

This does **not** mean full upstream PyTorch closure.

It means:

- most common inference and training workloads are semantically credible
- common checkpoints and model code are portable or convertible
- the framework is broad enough that "drop-in for most workloads" is honest

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
- serialization and checkpoint compatibility are still narrower than a
  practical PyTorch replacement needs
- GGUF quant decode remains incomplete, especially K-family formats
- backend execution breadth is uneven beyond the current truthful lanes
- cluster and sandbox truth exist, but the full library-level execution story
  still needs convergence
- train-class execution is real but still narrow and adapter-first

## Roadmap Shape

This roadmap is organized into seven epics.

| Epic | Theme | Outcome |
| --- | --- | --- |
| Epic 0 | Governance and acceptance | one canonical library roadmap and claim vocabulary |
| Epic 1 | Framework core completion | serious tinygrad-disciplined tensor/compiler/runtime substrate |
| Epic 2 | Semantics and compatibility | PyTorch-credible ops, modules, optimizers, and state behavior |
| Epic 3 | Model IO and runtime families | truthful serving, model-family loading, cache, router, and runtime behavior |
| Epic 4 | Backend truth and performance | explicit, benchmarked, honest backend lanes |
| Epic 5 | Cluster, sandbox, and execution truth | reusable distributed execution and bounded compute substrate |
| Epic 6 | Training, eval, and research | decentralized adapter training first, then broader train-class closure |
| Epic 7 | Interop and adoption | practical path from "good Rust engine" to "usable for most PyTorch workloads" |

Only one of these epics currently has a live dedicated GitHub issue block:

- decentralized adapter training:
  [#3649](https://github.com/OpenAgentsInc/openagents/issues/3649) and
  [#3636](https://github.com/OpenAgentsInc/openagents/issues/3636) through
  [#3648](https://github.com/OpenAgentsInc/openagents/issues/3648)

For the rest of the roadmap, the issue IDs below are currently roadmap-local
IDs. Open matching GitHub issues when an epic becomes active rather than
mass-opening the whole future program immediately.

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

## Epic 1: Framework Core Completion

### Goal

Finish the tinygrad-disciplined framework core without letting it turn into an
opaque monolith.

### Exit Criteria

- tensor, graph, autodiff, compile, memory, replay, and local multi-device
  categories are green at real breadth
- the primitive core stays small and inspectable
- backend bring-up can target explicit narrow contracts instead of a hidden
  giant stack

### Shipped Foundations

- `psionic-core` tensor metadata and quantization substrate
- `psionic-ir` graph and autodiff foundation
- `psionic-compiler` deterministic lowering and replay fixtures
- `psionic-runtime` runtime descriptors, proof, cache, and local multi-device
  substrate

### Issues

| ID | Status | Work |
| --- | --- | --- |
| `PLIB-101` | planned | Expand tensor semantics to credible view, alias, indexing, shape, reduction, and dtype-promotion breadth. |
| `PLIB-102` | planned | Add a real operator registry, schema layer, and dispatch/composite/meta execution split above the current primitive op set. |
| `PLIB-103` | planned | Add fake or meta tensor execution for shape-only planning, compile validation, and compatibility harnesses. |
| `PLIB-104` | planned | Expand autodiff coverage to broader operator families while keeping typed refusal on unsupported gradients. |
| `PLIB-105` | planned | Deepen compiler passes: schedule formation, fusion policy, memory planning, plan cache identity, and compile-cache evidence. |
| `PLIB-106` | planned | Complete same-type local multi-device behavior beyond implemented-early substrate, including sharding policy and refusal taxonomy. |
| `PLIB-107` | planned | Promote framework-core acceptance from representative proof to broad contract coverage with fixture-backed replay and failure tests. |

## Epic 2: Semantics And Compatibility

### Goal

Build the PyTorch-credible layer above the compact framework core.

### Exit Criteria

- the library can support common training and inference code with credible
  tensor, module, optimizer, and state behavior
- compatibility breadth is achieved through explicit registries and harnesses
- the semantics layer does not bloat the core IR and runtime

### Why This Epic Exists

This is the epic that turns "serious engine substrate" into "practical ML
library."

It is the most important missing middle layer in current Psionic.

### Issues

| ID | Status | Work |
| --- | --- | --- |
| `PLIB-201` | planned | Add a first-class module, parameter, buffer, and state-tree system. |
| `PLIB-202` | planned | Add deterministic `state_dict`-style naming, strict and non-strict load behavior, and size-mismatch refusal semantics. |
| `PLIB-203` | planned | Widen optimizer coverage with scheduler integration, parameter-group semantics, and stronger state behavior. |
| `PLIB-204` | planned | Define serialization and checkpoint compatibility boundaries for practical PyTorch interoperability without inheriting every historical artifact path. |
| `PLIB-205` | planned | Add a PyTorch-derived operator parity matrix analogous to `op_db` / `OpInfo` for Rust-native conformance. |
| `PLIB-206` | planned | Add a PyTorch-derived module parity matrix analogous to `module_db`. |
| `PLIB-207` | planned | Add a PyTorch-derived optimizer parity matrix analogous to `optim_db`. |
| `PLIB-208` | planned | Add symbolic-shape, fake-tensor, and compiler-hygiene parity harnesses informed by modern PyTorch compiler tests. |
| `PLIB-209` | planned | Make the semantics layer honest about what is `PyTorch-credible` versus what remains `PyTorch-compatible later`. |

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

## Epic 4: Backend Truth And Performance

### Goal

Make backend support explicit, benchmarked, and honest, while preserving the
small visible core that makes new backend bring-up tractable.

### Exit Criteria

- each declared backend lane has explicit readiness, capability, risk, and
  performance truth
- benchmark acceptance exists where throughput claims matter
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

## Epic 5: Cluster, Sandbox, And Execution Truth

### Goal

Turn Psionic's existing cluster, sandbox, net, datastream, and provider
substrate into one coherent execution-truth layer for local and networked
compute.

### Exit Criteria

- cluster identity, topology, placement, and execution truth are reusable and
  honest
- sandbox execution remains explicit and bounded
- provider capability surfaces derive from runtime truth rather than ad hoc
  product glue
- receipts, manifests, replay, and proof bundles remain first-class

### Shipped Foundations

- `psionic-net`
- `psionic-datastream`
- `psionic-cluster`
- `psionic-sandbox`
- `psionic-provider`
- lane-specific cluster, Metal, and FM roadmap work

### Issues

| ID | Status | Work |
| --- | --- | --- |
| `PLIB-501` | planned | Reconcile the existing cluster substrate and lane-specific roadmap into one full-library cluster contract. |
| `PLIB-502` | planned | Finish topology-aware placement and scheduling as reusable library behavior rather than lane-local logic. |
| `PLIB-503` | planned | Converge datastream, artifact staging, and replay receipts into a single execution-evidence model. |
| `PLIB-504` | planned | Define bounded sandbox execution as a first-class Psionic library lane with explicit runtime, file, network, and policy truth. |
| `PLIB-505` | planned | Tighten provider capability derivation so all advertised compute products map back to explicit runtime evidence and refusal conditions. |
| `PLIB-506` | planned | Clarify kernel/Nexus authority boundaries for execution evidence, accepted outcomes, and later settlement-facing projections. |
| `PLIB-507` | planned | Add chaos and failure-injection coverage across cluster, artifact, replay, and sandbox paths. |

## Epic 6: Training, Eval, And Research

### Goal

Make Psionic's training-class substrate real first through decentralized
adapter training, then widen toward broader train-class library closure.

### Exit Criteria

- decentralized adapter training is honest and end-to-end
- eval, validator, and accepted-outcome surfaces are reusable
- training is broader than a single narrow Apple-only lane
- research and hillclimb loops build on the same receipts and validator truth

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

## Epic 7: Interop And Adoption

### Goal

Provide the path from "good Rust-native engine" to "realistically adoptable by
teams with PyTorch-shaped workflows."

### Exit Criteria

- practical checkpoint and model migration paths exist
- compatibility claims are explicit and limited
- at least one Python or Torch-facing interop story exists after the substrate
  is real
- the project can honestly describe where it is a PyTorch replacement and where
  it is not

### Issues

| ID | Status | Work |
| --- | --- | --- |
| `PLIB-701` | planned | Freeze the library's compatibility contract: `PyTorch-class`, `PyTorch-credible`, and `PyTorch-compatible` must mean different things and be tested differently. |
| `PLIB-702` | planned | Add safe conversion tools for common checkpoint and artifact migration into Psionic-owned formats. |
| `PLIB-703` | planned | Add a Python-facing or Torch-facing compatibility shell only after Epics 1 through 3 are materially real. |
| `PLIB-704` | planned | Build a reference model zoo and migration guide proving common families can run without app-local glue. |
| `PLIB-705` | planned | Define distribution and packaging for Psionic as a standalone library, not only as an internal repo subtree. |
| `PLIB-706` | planned | Publish a clear adoption story for teams choosing between Psionic-native, PyTorch-compatible, and mixed-runtime usage. |

## Current Execution Order

This is the recommended dependency order for the next full-library work.

### Phase 1: lock the library architecture and framework-core shape

1. `PLIB-002` canonical claim vocabulary
2. `PLIB-101` tensor semantics breadth
3. `PLIB-102` operator registry and dispatch layer
4. `PLIB-103` fake or meta execution
5. `PLIB-105` compiler, schedule, memory-planning, and plan-cache depth

### Phase 2: build the missing semantics layer

6. `PLIB-201` module and state-tree system
7. `PLIB-202` strict and non-strict state loading
8. `PLIB-203` optimizer and scheduler breadth
9. `PLIB-204` serialization and checkpoint compatibility
10. `PLIB-205` through `PLIB-208` parity harness registries

### Phase 3: finish runtime-family truth

11. `PLIB-301` GGUF K-family and remaining quant decode
12. `PLIB-302` tokenizer and prompt fidelity
13. `PLIB-303` cache and paged-storage completion
14. `PLIB-305` through `PLIB-307` serving and router contract convergence

### Phase 4: keep backend truth honest while widening execution breadth

15. `PLIB-402` Metal lane refresh and closure
16. `PLIB-403` CUDA lane refresh and closure
17. `PLIB-404` and `PLIB-405` AMD execution closure
18. `PLIB-406` backend evidence and benchmark acceptance

### Phase 5: reconcile distributed execution truth

19. `PLIB-501` through `PLIB-507`

### Phase 6: execute the training and eval program

20. `PLIB-601` through `PLIB-606`

### Phase 7: only then expose broader adoption and compatibility shells

21. `PLIB-701` through `PLIB-706`

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

### 5. Do not open a Python-compatibility shell before the substrate is real

A PyTorch-looking shell without real semantics is not adoption.

It is a demo.

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

That is now the canonical Psionic roadmap.
