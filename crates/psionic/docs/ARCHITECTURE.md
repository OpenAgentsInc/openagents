# Psionic System Spec

> Status: updated 2026-03-14 after reviewing `docs/MVP.md`,
> `docs/OWNERSHIP.md`, `crates/psionic/README.md`,
> `crates/psionic/docs/TRAIN_SYSTEM.md`,
> `docs/audits/2026-03-14-covenant-code-lessons-for-psionic-train-audit.md`,
> `crates/psionic/docs/INFERENCE_ENGINE.md`,
> `crates/psionic/psionic-runtime/src/lib.rs`,
> `crates/psionic/psionic-cluster/src/lib.rs`,
> `crates/psionic/psionic-datastream/src/lib.rs`,
> `crates/psionic/psionic-sandbox/src/lib.rs`,
> `crates/psionic/psionic-collectives/src/lib.rs`,
> `crates/psionic/psionic-train/src/lib.rs`, and
> `crates/psionic/psionic-adapters/src/lib.rs`, plus the current open and
> recently closed issue backlog through `#3609`.

## Why This Doc Exists

Psionic already has enough surface area that a short layering note is no longer
sufficient.

This document is the canonical system spec for Psionic as a whole. It answers:

- what Psionic is
- what Psionic owns and does not own
- what is implemented now
- how the subtree is layered
- what kinds of work Psionic runs
- what artifact and receipt families Psionic should emit
- how Psionic execution flows end to end
- how failures and security are handled at the substrate level

This doc should be read together with:

- `crates/psionic/docs/TRAIN_SYSTEM.md`
  - deep subsystem spec for training-class execution
- `crates/psionic/docs/INFERENCE_ENGINE.md`
  - narrower completion criteria for inference-engine behavior
- `crates/psionic/docs/LLAMA_VLLM_SGLANG_INFERENCE_SPEC.md`
  - inference-completion plan and issue program

The Psionic Train system builds on Psionic runtime, cluster, datastream,
sandbox, and collective layers defined in this document.

## Doc Authority

- `crates/psionic/README.md` is the entrypoint and map.
- `crates/psionic/docs/ARCHITECTURE.md` is the canonical Psionic-wide system
  spec.
- `crates/psionic/docs/TRAIN_SYSTEM.md` is the canonical training subsystem
  spec.
- research audits explain why the system should move in a given direction, but
  they are not the authoritative current-state spec.

## Status Vocabulary

The status labels in Psionic docs use these meanings:

| Label | Meaning |
| --- | --- |
| `implemented` | landed and materially usable as a current substrate |
| `implemented_early` | landed, real, and usable, but still clearly early or incomplete |
| `partial` | some of the subsystem exists, but major required pieces are still missing |
| `partial_outside_psionic` | the broader OpenAgents stack has the authority or control surface, but Psionic does not yet own the native runtime or execution layer |
| `planned` | still a design target rather than a landed subsystem |

## Short Definition

Psionic is the Rust-native execution substrate for compute workloads inside
OpenAgents.

Psionic owns reusable substrate for:

- runtime execution
- backend capability and execution planning
- clustered topology and ordered state
- artifact staging and resumable transport
- runtime and environment manifest binding
- session-bound execution identity for networked lanes
- sandbox execution
- serving contracts
- training-class recovery and collective planning
- execution evidence and proof bundles

Psionic does not own:

- app UX
- wallet or payout flows
- buyer or provider product orchestration
- kernel authority or final market settlement

## What Psionic Owns

Psionic owns the machine-facing execution truth for compute lanes.

In practical terms that means:

- what artifacts were bound to execution
- what runtime or environment manifest package was actually used
- what transport or session identity claims were attached to execution
- what backend or topology ran the work
- what staged data was transferred and verified
- what proof posture or evidence was available
- what recovery or reconfiguration happened
- what receipts and execution metadata the rest of the system can consume

## What Psionic Does Not Own

Psionic is not the whole OpenAgents stack.

It must not own:

- pane-facing or desktop UX
- payout and wallet behavior
- marketplace procurement or settlement authority
- final collateral, claim, or adjudication authority
- app-owned control flows that belong in `apps/autopilot-desktop`

That boundary is intentional. Psionic explains what happened at execution time.
It does not decide what the market counts or what the product UI should do.

## Non-Goals

Psionic is also not:

- final market or settlement authority
- a home for app workflows
- a claim that every compute lane is mature today
- a hidden Python control plane behind Rust wrappers

## System Status At A Glance

| Area | Current Status | Current Repo Truth |
| --- | --- | --- |
| Local inference substrate | `implemented_early` | runtime, backend, model, and serve crates exist with CPU and partial Metal lanes |
| Clustered serving substrate | `implemented_early` | `psionic-cluster` owns ordered state, placement, catch-up, and sharded serving topology truth |
| Datastream and artifact staging | `implemented_early` | resumable manifests, chunk transport, and delivery receipts exist in `psionic-datastream` |
| Data contracts | `implemented_early` | `psionic-data` now owns versioned dataset manifests, tokenizer digests, split declarations, streamed iteration, and long-context packing policies |
| Sandbox execution | `implemented_early` | bounded execution, runtime detection, background jobs, file transfer, and receipts exist in `psionic-sandbox` |
| Execution proof bundles | `implemented_early` | canonical execution-proof bundles live in `psionic-runtime` |
| Collectives | `implemented_early` | elastic device-mesh observation, bandwidth-aware local/global sync planning, and benchmark-gated collective cadence receipts exist in `psionic-collectives` |
| Train recovery substrate | `implemented_early` | checkpoint, live-recovery, elastic-membership session truth, explicit checkpoint manifests or pointers, and restore receipts exist in `psionic-train` |
| Training run graph | `implemented_early` | `psionic-train` now owns typed training runs, contributor-set revisions, topology revisions, participant lifecycle, and window transitions |
| Adapter lineage | `implemented_early` | adapter identity, packaging, and hosted binding lineage exist in `psionic-adapters` |
| Eval runtime | `implemented_early` | `psionic-eval` now owns held-out eval runs, rubric-scored sample/runtime contracts, benchmark packages, repeat-run aggregation, and operator-local validator simulation, while kernel/Nexus still own canonical eval-run authority truth |
| Environment package runtime | `implemented_early` | `psionic-environments` now owns the runtime ABI, tool/rubric hooks, expected artifact contracts, and deterministic reference sessions, while kernel/Nexus still own registry and authority truth |
| Training core reference loop | `implemented_early` | `psionic-train` now owns a typed fixed-budget trainer-step path with parameter groups, optimizer state, residency transitions, checkpoint restore lineage, and step telemetry; broader distributed trainer completion is still planned |
| Full synthetic-data or research loop | `partial_outside_psionic` | synthetic-data job and verification flows now exist in kernel/Nexus, but no Psionic-native generation runtime or research-loop crate family exists yet |

Recent issue closure changed one important reading of this table:

> environment packages, eval runs, and synthetic-data authority flows now exist
> in the broader OpenAgents stack, and Psionic now owns the first environment
> plus eval runtime crates, but broader generation loops still remain
> unfinished.

## Canonical Layer Model

Psionic should be understood as a layered subtree with clear dependency
direction.

### System Diagram

```text
Applications / Operators / Authority
        |
        v
  psionic-provider
        |
        v
 psionic-serve / psionic-models
        |
        v
 psionic-train / psionic-eval / psionic-data / psionic-collectives / psionic-adapters
        |
        v
 psionic-cluster / psionic-datastream / psionic-sandbox / psionic-net
        |
        v
 backend crates
        |
        v
 psionic-runtime / psionic-compiler / psionic-ir / psionic-core
```

### Layering By Crate

1. `psionic-core`
   - foundational tensor, dtype, shape, and device types
2. `psionic-ir`
   - canonical graph and execution-plan representation
3. `psionic-compiler`
   - lowering and scheduling boundaries over IR
4. `psionic-runtime`
   - runtime traits, runtime planning, execution-proof bundles, training-class
     runtime truth
5. `psionic-sandbox`
   - bounded execution profiles, runtime detection, execution receipts, and
     background-job lifecycle
6. `psionic-net`
   - peer identity, transport sessions, relay-backed rendezvous, trust and
     candidate state
7. `psionic-datastream`
   - resumable manifests, chunk transfer, and delivery receipts for artifacts
8. `psionic-data`
   - versioned dataset manifests, tokenizer digests, split declarations,
     streamed iteration, and packing policy contracts
9. `psionic-eval`
   - held-out eval runs, rubric-scored runtime contracts, benchmark packages,
     repeat-run aggregation, and local validator simulation
10. `psionic-cluster`
   - ordered state, cluster admission, catch-up, scheduling, topology and
     placement truth
11. `psionic-collectives`
   - elastic device-mesh, local/global sync planning, transport-feedback
     replanning, and quantized collective policy
12. `psionic-train`
   - training-session truth for checkpointing, live recovery,
     elastic-membership posture, checkpoint pointers/manifests, and restore
     receipts
13. `psionic-adapters`
   - adapter identity, packaging, and hosted binding lineage
14. backend crates
   - backend-specific runtime implementations only
15. `psionic-models`
   - reusable model definitions and metadata
16. `psionic-serve`
   - request, response, and execution contracts for served products
17. `psionic-router`
   - reusable multi-model routing inventory, policy filters, and worker-path
     selection for served fleets
18. `psionic-provider`
   - provider-facing capability, readiness, and receipt types at the OpenAgents
     boundary

The crate list and layering are canonical for current ownership and dependency
direction, but they are not a guarantee that every planned subsystem will land
under exactly these final crate names.

### Dependency Direction

- lower crates must not depend on higher product-facing crates
- no crate in `crates/psionic/` may path-depend on `apps/*`
- reusable engine crates must not own app workflows or market authority
- `psionic-provider` is the boundary adapter, not a place to hide app logic

## Canonical Psionic Work Classes

Psionic needs two different notions of work class:

- product-level execution classes
- low-level runtime scheduling classes

### Product-Level Work Classes

| Work Class | Meaning | Current Status |
| --- | --- | --- |
| Inference | generate model outputs for served requests | `implemented_early` |
| Embeddings | generate vectors or embedding outputs | `implemented_early` |
| Clustered serving | execute inference across replicas or sharded topology | `implemented_early` |
| Sandbox execution | run bounded remote or local sandbox jobs | `implemented_early` |
| Artifact staging | move datasets, checkpoints, served artifacts, and adapter bundles | `implemented_early` |
| Training-class coordination | coordinate checkpoints, recovery, collectives, and elastic membership | `implemented_early` |
| Full training | execute trainer-step and optimizer updates | `planned` |
| Eval | run shared held-out or online evaluation | `planned` |
| Synthetic-data generation | generate or score new data under the same substrate | `planned` |
| Adapter-backed serving | serve a base artifact plus attributed adapter lineage | `implemented_early` |

### Low-Level Runtime Work Classes

These are the scheduler-facing classes already encoded in
`psionic-runtime::RuntimeWorkClass`.

| Runtime Work Class | Meaning |
| --- | --- |
| `DecodeToken` | one latency-sensitive decode step |
| `PrefillBatch` | one prefill or preparation batch |
| `DatastreamChunk` | one chunk transfer over the data plane |
| `CollectiveStep` | one collective or synchronization step |
| `CheckpointFlush` | one checkpoint or persistence flush step |

The system-wide rule is:

> product work classes explain what Psionic is doing for the platform, while
> low-level runtime work classes explain how the runtime schedules the work.

## Canonical System Objects

Psionic needs a stable object vocabulary across serving, staging, sandbox, and
training subsystems.

| Object | Owner | Purpose | Current Status |
| --- | --- | --- | --- |
| `RuntimeWorkItem` | `psionic-runtime` | one low-level schedulable unit of work | `implemented` |
| `ExecutionProofBundle` | `psionic-runtime` | canonical execution evidence for runtime work | `implemented` |
| `DatastreamManifest` | `psionic-datastream` | full resumable manifest for one artifact stream | `implemented` |
| `DatastreamManifestRef` | `psionic-datastream` | compact artifact reference embedded in other contracts, including explicit distributed KV spill/restore locators | `implemented` |
| `DatasetManifest` | `psionic-data` | versioned dataset, tokenizer, split, and shard-lineage contract | `implemented_early` |
| `DatasetIterationContract` | `psionic-data` | resume-safe split iteration over datastream-backed shards | `implemented_early` |
| `DatasetPackingPolicy` | `psionic-data` | long-context sequence packing and token-budget batch planning contract | `implemented_early` |
| `RuntimeManifest` | planned proof/environment layer | digest-bound package for artifact, static-config, and runtime lineage used at execution time | `planned` |
| `DatastreamDeliveryReceipt` | `psionic-datastream` | verified proof of delivered bytes and chunk progress | `implemented` |
| `ClusterState` | `psionic-cluster` | authoritative cluster membership and ordered-state truth | `implemented` |
| `SessionClaimsBundle` | planned `psionic-net` / proof layer | signed claims that bind session identity to runtime, environment, and artifact digests | `planned` |
| `TrainingCheckpointReference` | `psionic-runtime` | stable identity for one training checkpoint | `implemented` |
| `TrainingRecoveryContext` | `psionic-runtime` | runtime-visible recovery posture for training-class execution | `implemented` |
| `TrainingDeviceMeshContext` | `psionic-runtime` | runtime-visible elastic device-mesh posture | `implemented` |
| `TrainingCollectiveContext` | `psionic-runtime` | runtime-visible collective posture and benchmark evidence | `implemented` |
| `AdapterArtifactIdentity` | `psionic-adapters` | stable identity for one adapter artifact | `implemented` |
| `AdapterPackageManifest` | `psionic-adapters` | package manifest for adapter bytes tied to datastream | `implemented` |
| `ProviderSandboxExecutionReceipt` | `psionic-sandbox` | receipt for one bounded sandbox run | `implemented` |
| `TrainingRun` | `psionic-train` | root identity, participant graph, and lifecycle state for one training program | `implemented_early` |
| `TrainingWindow` | `psionic-train` | one synchronized contribution or trainer interval with contributor-set and transition state | `implemented_early` |
| `CollectiveSyncCadenceReceipt` | `psionic-collectives` | typed cadence, transport-feedback, and replan-trace receipt for one sync step | `implemented_early` |
| `CheckpointPointer` | `psionic-train` | stable pointer to the latest accepted checkpoint for a run, stage, or window | `implemented_early` |
| `CheckpointManifest` | `psionic-train` | typed shard, digest, writer, and durability description for one checkpoint flush | `implemented_early` |
| `EnvironmentPackage` | `psionic-environments` | reusable task, rubric, tool, dataset, and artifact environment package | `implemented_early` |
| `BenchmarkPackage` | `psionic-eval` | validator-owned packaged benchmark harness or reference evaluation profile with repeat-run aggregation | `implemented_early` |
| `EvalRun` | `psionic-eval` | one local evaluation execution over a declared environment and artifact set | `implemented_early` |

The important point is not that every object already exists. The important
point is that Psionic should converge on a typed object model rather than
passing loosely structured blobs between subsystems.

Psionic enforces capability envelopes at runtime, while higher-level compute
products define the admissible execution contract exposed to buyers, operators,
and authority layers.

## Glossary

| Term | Meaning |
| --- | --- |
| execution truth | what the Psionic runtime and cluster can honestly say happened at execution time |
| authority truth | what higher-level OpenAgents services accept as canonical outcome |
| artifact truth | what manifests, digests, package refs, and staged bytes were actually bound to execution |
| runtime identity | the verified execution origin responsible for a work item |
| session claims bundle | the signed session-scoped claim set that ties peer or session keys to runtime and artifact identity |
| training window | one bounded contributor or trainer interval with explicit control-plane state |
| checkpoint lineage | the chain of checkpoint identities, manifests, and durability transitions that define recoverable train state |
| checkpoint pointer | the stable reference to the latest accepted checkpoint for a run, stage, or window |
| checkpoint manifest | the typed shard, digest, writer, and durability description for one checkpoint flush |
| policy revision | the specific weight or policy version a worker, trainer, or eval run consumed |
| environment package | a versioned task, rubric, tool, and sandbox contract used by training or eval |
| benchmark package | a validator-owned packaged benchmark or reference evaluation profile reused for repeatable scoring |
| proof posture | the declared strength and availability of execution evidence |
| validator posture | the declared verification policy and adjudication expectations for a workload |
| manifest registry | a versioned allowlist or policy registry for manifests, proof profiles, or environment packages |
| receipt | the typed record of an accepted state transition or outcome |
| collective posture | the mesh, communication, quantization, and benchmark facts attached to one collective step |

## Artifact Model

Psionic is also an artifact system, not only an execution engine.

### Canonical Artifact Families

| Artifact | Current Carrier | Meaning |
| --- | --- | --- |
| Served artifact | `DatastreamSubjectKind::ServedArtifact` | model or sharded serving artifact used for inference |
| Checkpoint | `DatastreamSubjectKind::Checkpoint` plus `TrainingCheckpointReference` | recoverable training or optimizer state |
| Tokenized corpus | `DatastreamSubjectKind::TokenizedCorpus` | tokenized dataset shard delivered for training or eval |
| Eval bundle | `DatastreamSubjectKind::EvalBundle` | benchmark or evaluation harness artifact |
| Benchmark package | `psionic-eval` | validator-owned packaged benchmark harness or reference evaluation profile |
| Adapter package | `DatastreamSubjectKind::AdapterPackage` plus adapter manifests | adapter or LoRA artifact delivered with lineage |
| Proof artifact | execution-proof bundle or augmentation | evidence about what the runtime or cluster actually did |
| Sandbox artifact | sandbox input/output digest sets | staged inputs and outputs of bounded execution |
| Environment package | `psionic-environments` | versioned task, tool, rubric, dataset, and sandbox contract |

### Artifact Rules

- artifacts should be digest-bound
- artifacts should be referenceable through compact manifest refs where
  possible
- runtime and environment identity should distinguish digest-bound measured or
  static config from mutable runtime variables
- artifacts should carry enough lineage to explain what execution actually
  consumed
- policy-meaningful lanes should reference versioned manifest or profile
  registries rather than opaque free-form strings
- Psionic should not rely on unnamed side files for economically or
  operationally important artifacts

## Receipts And Truth Boundaries

Psionic is receipt-first, but it is not authority-first.

The tree should be understood through four truth domains.

| Truth Domain | Owned By | What It Says |
| --- | --- | --- |
| Runtime truth | `psionic-runtime` and lower execution crates | what device, work class, and proof posture actually ran |
| Artifact truth | `psionic-datastream`, `psionic-adapters`, `psionic-eval`, and `psionic-environments` | what bytes, manifests, packages, and digests were actually staged or referenced |
| Cluster and sandbox truth | `psionic-cluster`, `psionic-sandbox`, `psionic-collectives`, `psionic-train` | what topology, recovery posture, sandbox runtime, and collective decisions actually occurred |
| Authority truth | outside Psionic in kernel and control services | what the platform or market accepts as final outcome |

The key boundary is:

> Psionic determines execution truth. Higher-level OpenAgents services determine
> authority truth.

### Runtime Identity

Runtime identity means the verified execution origin responsible for a work
item, including provider node identity, sandbox instance identity, or cluster
member identity.

Runtime identity matters because it anchors:

- proof attribution
- validator checks
- receipt lineage

### Session Claims And Manifest Discipline

For proof-bearing networked execution, transport identity should carry a signed
session-claims bundle that references runtime, environment, and artifact
digests.

Psionic should also distinguish:

- digest-bound measured or static config
- mutable runtime variables
- higher-level policy profiles or manifest registries evaluated outside Psionic

That split keeps runtime truth honest without collapsing execution evidence and
policy authority into one crate.

### Canonical Receipt Families

| Receipt Family | Current Status | Producer |
| --- | --- | --- |
| runtime execution proof bundles | `implemented` | `psionic-runtime` |
| datastream delivery receipts | `implemented` | `psionic-datastream` |
| sandbox execution receipts | `implemented` | `psionic-sandbox` |
| clustered execution evidence | `implemented_early` | `psionic-cluster` |
| training run, trainer step, and eval receipts | `planned` | future `psionic-train` and `psionic-eval` layers |
| adapter package and hosted binding lineage | `implemented_early` | `psionic-adapters` |

## Canonical Execution Lifecycle

Every Psionic workload should fit the same high-level lifecycle even when the
details differ by lane.

1. Work is declared through typed contracts.
2. Artifact bindings and execution prerequisites are resolved.
3. Capability and topology are checked against the requested work.
4. Required artifacts are staged or resumed through datastream contracts.
5. Runtime or cluster planning produces executable work items and topology
   posture.
6. Execution occurs on the declared backend, sandbox, or cluster.
7. Evidence and receipts are emitted from the execution substrate.
8. Operator or authority surfaces consume the typed result rather than raw
   process logs.

## Time Semantics

Psionic execution participates in several time boundaries:

- artifact freshness windows
- checkpoint durability windows
- execution timeouts
- sandbox lifetime limits
- transport retry and resume windows

Training-class and clustered execution build additional timing contracts on top
of these substrate-level boundaries rather than inventing a separate execution
clock.

### Serving Variant

For serving lanes this typically means:

- served artifact resolution
- backend and capability gating
- queue admission, fairness, mixed prefill/decode work, and explicit TTFT/ITL
  plus prefill/decode handoff truth when the lane supports it
- hierarchical KV residency truth across host, device, and any explicit
  externalized tier contract the lane can actually surface
- structured outputs, tool or response-state semantics, and optional multi-model
  routing
- optional clustered placement and shard handoff
- response and proof emission

### Sandbox Variant

For sandbox lanes this typically means:

- profile realization
- bounded runtime selection
- input staging
- job execution
- output and receipt emission

### Training-Class Variant

For training-class lanes this should eventually mean:

- checkpoint and dataset staging
- participant topology formation
- mesh and collective planning
- trainer or rollout execution
- checkpoint flush and recovery handling
- train-specific receipt emission

The training variant is only partially implemented today.

## Control Plane And Observation Boundaries

Psionic exports typed state; it does not own the operator shell.

### App-Owned Control Plane

The desktop app and `autopilotctl` should consume Psionic truth for:

- capability and readiness
- runtime or cluster state
- manifest refs and session-claims posture
- artifact staging progress
- queue or admission posture, shard or cache placement, and sandbox pool health
- sandbox job state
- challenge or validator status when the lane uses one
- training and eval diagnostics, once those exist

### Authority Plane

Kernel and control services should consume Psionic truth for:

- receipts
- proof bundles
- staged artifact references
- cluster and recovery posture
- validator-facing evidence

### What Psionic Must Not Do Here

Psionic must not:

- own app workflows
- invent settlement authority
- collapse operator presentation and execution truth into one crate

## Failure Model

Psionic should handle failure explicitly and typefully.

| Failure | Expected Substrate Handling |
| --- | --- |
| backend unsupported or unavailable | fail capability checks early and expose truthful readiness posture |
| node loss during clustered execution | trigger catch-up, reconfiguration, or recovery according to cluster and train posture |
| network degradation | replan collective or transport decisions when observations degrade materially |
| datastream interruption | resume from cursor and committed bytes rather than restart whole transfer blindly |
| checkpoint flush failure | keep checkpoint non-durable and block any state transition that requires durability |
| sandbox crash | emit bounded execution failure receipt and apply retry or quarantine policy outside the sandbox engine |
| cluster membership mismatch | reject the state transition rather than silently rebasing to a different cluster |
| detached or invalid session claims | reject policy-meaningful networked execution rather than treating transport identity alone as sufficient |
| unapproved quantized collective request | reject planning rather than silently downgrade without record |
| stale artifact or policy revision | reject or quarantine the work item under explicit freshness rules |
| proof augmentation unavailable | emit explicit proof posture rather than pretending strong proof exists |

Psionic must surface failure as typed, reason-coded events rather than opaque
runtime exceptions.

Psionic should prefer:

- reason-coded failure
- replay-safe state transitions
- explicit degraded posture

It should avoid:

- silent fallback that changes truth without record
- opaque runtime-only failure behavior

## Security Model

Psionic is not the whole platform security model, but it does own several core
security surfaces.

| Threat | Mitigation Direction In Psionic |
| --- | --- |
| artifact tampering | manifest digests, chunk digests, object digests, provenance linkage |
| checkpoint tampering | checkpoint-family binding, writer identity, manifest verification, durable checkpoint posture |
| cluster spoofing or false membership | peer identity, admission policy, ordered-state truth, cluster mismatch rejection |
| detached transport identity or forged proof-bearing sessions | session-claims bundles bound to peer or session keys plus manifest refs and policy checks |
| sandbox escape or undeclared runtime behavior | bounded profiles, explicit runtime detection, execution receipts |
| proof opacity | explicit proof augmentation posture instead of hidden assumptions |
| manifest or policy-registry drift | versioned manifest registries and explicit profile identifiers carried through receipts and authority integrations |
| stale or mismatched policy artifacts | freshness windows and policy-revision binding in planned train layer |
| malicious rollout workers | planned validator sampling and train-layer admission control |
| transport degradation or relay ambiguity | explicit transport observations and candidate state in `psionic-net` and `psionic-cluster` |

The system-wide rule is:

> Psionic should always prefer explicit identity, digest binding, and typed
> degraded posture over implicit trust.

## Current And Planned Psionic Scope

Psionic already has real system scope across:

- runtime execution
- clustered serving
- sandbox execution
- artifact transport
- proof bundles
- training-class recovery substrate

Psionic is still growing into:

- full inference-engine maturity
- full Rust-native train core
- environment and eval runtime
- synthetic-data and research loops
- production-hardening around reproducibility, storage, and security

Those planned areas should still land inside the same system model described
here, not as a disconnected parallel stack.

## Companion Subsystem Specs

- `crates/psionic/docs/TRAIN_SYSTEM.md`
  - deep specification for the training subsystem
- `crates/psionic/docs/INFERENCE_ENGINE.md`
  - narrow inference completion criteria
- `crates/psionic/docs/LLAMA_VLLM_SGLANG_INFERENCE_SPEC.md`
  - detailed inference build-out and issue plan

## Review Checklist

- Is this logic in the lowest Psionic crate that can honestly own it?
- Does the change keep execution truth separate from app or market authority?
- Are artifacts and receipts typed and inspectable?
- Is degraded or missing proof posture stated explicitly?
- Does the change preserve the boundary between reusable Psionic substrate and
  app-owned or authority-owned control flow?

## Bottom Line

Psionic is already more than an inference experiment. It is the reusable Rust
execution substrate for OpenAgents compute lanes.

Today it already owns:

- runtime execution truth
- clustered topology truth
- artifact staging
- sandbox execution
- proof bundles
- early training-class recovery and collective truth

What it still lacks is not a new architectural direction. It lacks completion
of the same direction:

- mature inference engine behavior
- full environment and eval layers
- broader distributed training completion
- production-grade receipt, security, and operating discipline across the whole
  subtree

That is the Psionic program.
