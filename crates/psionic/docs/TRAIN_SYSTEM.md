# Psionic Train System

> Status: updated 2026-03-15 after reviewing `docs/MVP.md`,
> `docs/OWNERSHIP.md`,
> `docs/audits/2026-03-13-intellect-lessons-for-psionic-train-audit.md`,
> `docs/audits/2026-03-14-covenant-code-lessons-for-psionic-train-audit.md`,
> `crates/psionic/README.md`,
> `crates/psionic/docs/ARCHITECTURE.md`,
> `crates/psionic/docs/AUTORESEARCH_INTEGRATION_PLAN.md`,
> `docs/kernel/compute-training-authority.md`,
> `docs/headless-compute.md`,
> `crates/psionic/psionic-runtime/src/lib.rs`,
> `crates/psionic/psionic-datastream/src/lib.rs`,
> `crates/psionic/psionic-collectives/src/lib.rs`,
> `crates/psionic/psionic-train/src/lib.rs`,
> `crates/psionic/psionic-environments/src/lib.rs`,
> `crates/psionic/psionic-eval/src/lib.rs`,
> `crates/psionic/psionic-adapters/src/lib.rs`,
> `crates/psionic/psionic-data/src/lib.rs`, and
> `crates/psionic/psionic-sandbox/src/lib.rs`,
> `apps/autopilot-desktop/src/desktop_control.rs`, and
> `apps/autopilot-desktop/src/bin/autopilotctl.rs`, plus the recently closed
> train-adjacent issue backlog through `#3635` and the decentralized adapter
> training issue program starting at `#3636`.

## Why This Doc Exists

The March 13 Intellect audit correctly described the shape Psionic should grow
toward, but one part of it is now stale: Psionic no longer lacks a
`psionic-train` crate entirely.

The tree now has:

- `psionic-train`
- `psionic-collectives`
- `psionic-adapters`

That means the right question is no longer "should Psionic have any train
subtree at all?"

The right question is:

> what does the Psionic train system honestly implement today, what does it
> still not implement, and what is the full Rust-native path from the current
> substrate to a real training system?

This doc answers that question.

The train system assumes the execution substrate defined in
`ARCHITECTURE.md` and does not redefine runtime, cluster, sandbox, or artifact
transport behavior.

Apple-specific adapter work is no longer only later-family planning. The repo
now owns a canonical spec-and-fixture baseline for it in:

- `crates/psionic/docs/APPLE_ADAPTER_DATASET_SPEC.md`
- `crates/psionic/docs/APPLE_FMADAPTER_PACKAGE_SPEC.md`
- `crates/psionic/docs/APPLE_ADAPTER_LINEAGE_SPEC.md`
- `crates/psionic/fixtures/apple_adapter/`

and now also has:

- a repo-owned Apple training execution backend in `psionic-train`
- a first Rust-native Apple adapter SFT/export lane
- an optional Apple draft-model distillation lane
- an app-owned desktop-control and `autopilotctl` operator path that can
  launch, export, and accept one Apple training run into kernel authority

## Doc Authority

- `crates/psionic/docs/TRAIN_SYSTEM.md` is the canonical training subsystem
  spec.
- `crates/psionic/docs/ARCHITECTURE.md` is the canonical Psionic-wide system
  spec that defines the lower execution substrate this doc builds on.
- `crates/psionic/docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md` is the canonical
  framework-core acceptance split; train acceptance must not be used as a
  substitute for framework-core parity claims.
- `crates/psionic/docs/APPLE_ADAPTER_DATASET_SPEC.md`,
  `crates/psionic/docs/APPLE_FMADAPTER_PACKAGE_SPEC.md`, and
  `crates/psionic/docs/APPLE_ADAPTER_LINEAGE_SPEC.md` are the canonical
  Apple-adapter reference docs for dataset shape, package inventory, and
  lineage metadata.
- `docs/audits/2026-03-13-intellect-lessons-for-psionic-train-audit.md` is
  research rationale, not the canonical current-state spec.
- `docs/audits/2026-03-14-covenant-code-lessons-for-psionic-train-audit.md`
  is a code-grounded adaptation audit for windowed training, checkpoint
  protocol discipline, validator-owned benchmark truth, and bounded research
  loops.

## Status Vocabulary

This doc uses the canonical status vocabulary defined in `ARCHITECTURE.md`:
`implemented`, `implemented_early`, `partial`, `partial_outside_psionic`, and
`planned`.

## Short Definition

The Psionic train system is not one crate.

It is the Rust-native training-class execution stack inside `crates/psionic/`
that should eventually own:

- training-session truth
- elastic membership and recovery
- collective planning
- checkpoint and weight movement
- environment-bound training and eval execution
- rollout ingestion and validation
- trainer and orchestrator control flow
- operator-inspectable receipts for the whole system

Today Psionic implements the lower half of that stack plus a first real
trainer-step core.

It already has real substrate for:

- training recovery posture
- checkpoint lineage
- elastic membership truth
- device-mesh and collective planning
- resumable dataset and checkpoint transport
- typed fixed-budget trainer steps
- per-group optimizer state and residency policy
- reusable optimizer contracts plus typed SGD, Adam, AdamW, LARS, and LAMB
  state/update semantics
- reverse-mode autodiff, explicit detach, and training/no-grad gradient
  semantics over canonical IR primitives
- machine-legible step telemetry and checkpoint-anchored restore lineage
- checkpoint-aware policy revisions
- proof-bearing rollout artifacts and trainer-batch assembly
- versioned dataset manifests, tokenizer digests, split declarations, and
  long-context packing contracts
- environment package ABI and deterministic runtime sessions
- held-out eval runtime, benchmark packages, repeat-run aggregation, and local
  validator simulation
- adapter lineage

It does not yet implement the full distributed trainer-orchestrator-RL runtime.

## What Psionic Train Is Not

- It is not a promise that full general model training already works in the
  repo today beyond the current narrow Apple adapter lane.
- It is not a Python trainer hidden behind Rust wrappers.
- It is not an app-owned workflow inside `apps/*`.
- It is not just "cluster execution, but for training."
- It is not just checkpoint files and background notes.

The honest description today is:

> Psionic already owns real training-class truth surfaces plus a bounded
> training-core reference loop, but it does not yet own the full distributed
> train system.

## Apple Adapter Reality

The Apple Foundation Models lane now has an honest training answer, and that
answer needs to stay precise.

Yes, the repo can now train LoRA-style Apple adapter patches and export valid
`.fmadapter` packages.

That does **not** mean:

- the Swift bridge is doing the training
- Apple exposes a repo-controlled Foundation Models training API
- the current Apple lane is already a generalized distributed trainer

The current path is:

1. `psionic-data` imports Apple adapter JSONL into the repo-owned dataset
   contract, preserving tokenizer, prompt-shaping, tool/schema augmentation,
   and long-context packing lineage.
2. `psionic-environments` binds that dataset to the Apple adapter
   train/eval/benchmark environment package family so the train, held-out eval,
   and runtime-smoke lanes all point at one versioned environment truth.
3. `psionic-train` uses the repo-owned `AppleAdapterTrainingExecutionBackend`
   to turn packed samples into deterministic feature batches, freeze the base
   model, and train only the low-rank adapter parameter groups through the
   fixed-budget trainer core.
4. `run_apple_adapter_sft_export(...)` emits typed step receipts, gradient
   batch records, initial/final adapter-only bundle snapshots, adapter delta,
   summary metadata, and a valid `.fmadapter` package through
   `psionic-adapters`.
5. `psionic-eval` runs the held-out and benchmark-style adapter harnesses, and
   the bridge-backed runtime-smoke path proves the exported package can be
   loaded and exercised against the live Apple FM runtime.
6. `autopilotctl training launch ...`, `autopilotctl training export ...`, and
   `autopilotctl training accept ...` provide the shipped app-owned operator
   flow, while `autopilotctl apple-fm load ...` and `autopilotctl apple-fm
   attach ...` exercise the resulting package through the retained bridge.

The shipped Apple reference lane is intentionally narrow:

- base-model weights stay frozen
- only adapter parameter groups are updated
- the current precision posture is `f32_reference`
- the current activation-checkpoint posture is `disabled`
- the higher-level optional follow-on is draft-model distillation, not a second
  generic full-model trainer

That is why the right current claim is "repo-native Apple adapter training is
real" rather than "Psionic now has complete distributed Apple FM training."

### Relationship To Cluster And Distributed Training Substrate

The Apple lane already reuses real Psionic train substrate outside the narrow
execution backend itself:

- fixed-budget trainer-step execution
- reusable autodiff and optimizer layers
- dataset/tokenizer/packing contracts
- environment package bindings
- held-out eval and runtime-smoke harnesses
- training summaries, receipts, and accepted-outcome authority publication
- run-graph, orchestrator, and validator vocabulary used elsewhere in
  `psionic-train`

But the Apple lane does **not** yet consume the broader distributed-training
substrate as a live execution path:

- no real `psionic-cluster` multi-node Apple training run is claimed
- no collective-backed gradient exchange or sharded optimizer execution is used
  by the shipped Apple backend
- no production multi-device training kernel or memory-sharded Apple trainer is
  claimed
- no broader cluster scheduler is yet dispatching Apple training windows across
  multiple machines

What exists today is the reusable contract layer for those future steps:
`psionic-runtime` multi-device topology truth, `psionic-collectives`
collective planning, distributed-optimizer object models, orchestrator state,
and datastream/checkpoint movement. That substrate is meant to be reused later
for broader Psionic training lanes, including any future widened Apple path,
but it is not the current execution reality for the shipped Apple adapter
operator flow.

## Decentralized Adapter Training Program

Psionic is now explicitly planning decentralized adapter training as a
first-class train workload family.

That program sits on top of the already-retained train substrate:

- `TrainingRun`
- `TrainingStage`
- `TrainingWindow`
- `PolicyRevision`
- `RolloutArtifact`
- `TrainerBatch`
- `CheckpointPointer`
- `EvalRun`
- validator receipts and accepted-outcome authority projection

The new claim this doc freezes is not "the repo already has decentralized
adapter training."

The new claim is:

> the repo now has one canonical spec for decentralized adapter training, with
> Apple adapters as the first narrow lane and open adapter backends as the next
> generalized lane under the same control-plane vocabulary.

### First Lane, Next Lane, And Non-Goals

The first live execution lane remains the narrow single-host Apple adapter path
documented above.

The first decentralized lane should still begin with adapters, not full-model
weight updates, because the existing repo truth already has:

- adapter-only update semantics
- adapter package lineage
- benchmark and runtime-smoke validation posture
- accepted-outcome authority plumbing
- app-owned operator workflows that can expose contribution and review state

The first decentralized lane should therefore mean:

- one windowed multi-party adapter-training program
- one declared adapter target identity
- one declared dataset-slice identity per contribution
- one validator-owned disposition for each submitted contribution
- one aggregation decision that can produce a new policy revision or no-op

The first implementation has explicit non-goals:

- no world-scale synchronous all-reduce
- no full-model training claims
- no app-owned runtime or bridge logic inside Psionic train
- no generalized market/product claims before authority, validation, and
  operator truth exist
- no ad hoc JSON sidecars for contribution-state truth

### Canonical Workload Vocabulary

The decentralized adapter workload family should use the following additional
typed vocabulary on top of the generic train objects:

| Object | Purpose | Planned Scope |
| --- | --- | --- |
| `AdapterTrainingProgram` | Root program identity for one decentralized adapter-training family | one adapter target family, one policy family, one validator posture |
| `AdapterTrainingWindow` | One bounded contribution interval for one adapter target | contributor set, dataset-slice plan, policy revision in, seal state |
| `AdapterContributionAssignment` | One worker assignment into one adapter window | worker id, adapter target identity, dataset slice, replay budget |
| `AdapterContributionArtifact` | One uploaded local adapter delta or equivalent contribution bundle | manifest, delta/checkpoint pointer, local execution summary, provenance |
| `AdapterContributionReceipt` | One durable control-plane receipt for assignment, execution, upload, and disposition | worker id, window id, policy revision, validator result, aggregation eligibility |
| `AdapterAggregationReceipt` | One record of how accepted contributions were combined for one sealed window | accepted set, weights, output policy revision, checkpoint pointer |
| `PolicyPromotionReceipt` | One durable record of whether a sealed window promoted or held policy state | previous revision, next revision, acceptance basis, checkpoint lineage |

Every contribution receipt must bind:

- adapter target identity
- window id and contributor-set revision
- policy revision in
- dataset slice identity
- local execution summary
- uploaded artifact identity
- validator disposition
- aggregation eligibility decision
- resulting policy revision or no-promotion decision when the window seals

### Canonical Contribution Dispositions

The validator-owned contribution disposition vocabulary is:

| Disposition | Meaning |
| --- | --- |
| `accepted` | the contribution passed required checks and may participate in aggregation |
| `quarantined` | the contribution is retained for review but is not aggregation-eligible until explicitly released |
| `rejected` | the contribution failed required checks and is permanently excluded from aggregation |
| `replay_required` | the contribution cannot be trusted yet and must be regenerated or replayed under a fresh assignment |

These states are machine-legible control-plane truth, not UI-only labels.

### Canonical Window Flow

One decentralized adapter window should follow this explicit control flow:

1. The orchestrator plans one `AdapterTrainingWindow` with a sealed contributor
   set, dataset-slice plan, adapter target identity, and input policy revision.
2. Workers receive typed `AdapterContributionAssignment` records rather than
   free-form task text.
3. Each worker produces one `AdapterContributionArtifact` plus a local
   execution summary bound to the declared dataset slice and policy revision.
4. Datastream and artifact storage record upload completion as typed receipt
   state.
5. The validator emits one machine-legible disposition of `accepted`,
   `quarantined`, `rejected`, or `replay_required`.
6. The window seals only after the acceptance or replay policy is satisfied.
7. Aggregation consumes only aggregation-eligible accepted contributions and
   emits one `AdapterAggregationReceipt`.
8. Promotion emits one `PolicyPromotionReceipt` that either advances the policy
   revision or records an explicit no-promotion outcome.

This preserves deterministic replay because assignment, upload, disposition,
seal, aggregation, and promotion are all typed receipts on one window graph.

### Decentralized Adapter Acceptance Matrix

The repo must not claim decentralized adapter training is implemented until all
of the following rows are true:

| Claim Boundary | Required Truth |
| --- | --- |
| Spec frozen | this doc names decentralized adapter training as a first-class program, defines the object vocabulary above, and preserves the first-lane/non-goal boundary |
| Window contracts live | `psionic-train` represents adapter windows and contribution receipts as typed Rust objects without ad hoc JSON sidecars |
| Cluster selection live | contributor selection, membership, assignment, heartbeat, and window seal posture are bound to live `psionic-cluster` truth rather than local-only mocks |
| Artifact staging live | contribution uploads, manifests, delta/checkpoint pointers, and provenance are persisted through typed datastream/artifact contracts |
| Validator dispositions live | accepted, quarantined, rejected, and replay-required states are emitted by validator-owned code and preserved for replay or authority projection |
| Aggregation live | sealed windows can aggregate accepted contributions into a new policy revision or explicit no-promotion result with checkpoint lineage |
| Authority projection live | kernel/Nexus persist accepted window outcomes and policy-promotion truth as durable authority projections |
| Operator flow live | desktop and `autopilotctl` expose truthful contributor/operator state for window planning, submission, review, aggregation, and acceptance |
| Productization live | provider-substrate and compute-market claims are published only after the above control, validator, and authority rows are implemented |

Until every row above is true, the honest repo claim remains:

> single-host Apple adapter training is real, but decentralized adapter
> training is still a planned program with a frozen system contract.

## Canonical Train Objects

The full train system needs a formal object model. Today only some of these
objects have concrete repo types; the rest are planned and should become the
stable vocabulary for train-class execution.

| Object | Purpose | Current Repo Status |
| --- | --- | --- |
| `TrainingRun` | Root identity for one training program | `implemented_early` |
| `TrainingStage` | One named phase such as SFT, agentic SFT, or RL | `implemented_early` |
| `TrainingWindow` | One synchronized contribution or trainer interval with its own contributor set and transition state | `implemented_early` |
| `TrainerStep` | One optimizer update over one trainer batch | `implemented_early` |
| `PolicyRevision` | Versioned policy or weight state used by workers and trainer | `implemented_early` |
| `RolloutArtifact` | One worker-produced trajectory or completion bundle | `implemented_early` |
| `TrainerBatch` | One accepted batch of rollout or corpus inputs for a trainer step | `implemented_early` |
| `EnvironmentPackage` | One versioned environment definition used by training and eval | `implemented_early` |
| `BenchmarkPackage` | One validator-owned packaged benchmark or reference evaluation profile | `implemented_early` |
| `EvalRun` | One online or offline evaluation execution | `implemented_early` |
| `CheckpointPointer` | One stable pointer to the latest accepted checkpoint for a run, stage, or window | `implemented_early` |
| `CheckpointManifest` | One shard, digest, writer, and durability manifest for a checkpoint flush | `implemented_early` |
| `Checkpoint` | Recoverable training state and lineage anchor | `partial` |
| `ValidatorVerdict` | Verification result attached to one rollout, batch, or eval artifact | `implemented_early` |

Today the concrete object vocabulary is strongest around:

- `TrainingCheckpointReference`
- `TrainingRecoveryContext`
- `TrainingDeviceMeshContext`
- `TrainingCollectiveContext`
- `DatastreamManifest` and `DatastreamManifestRef`

Current checkpoint substrate is carried today by
`TrainingCheckpointReference`, explicit `CheckpointPointer` and
`CheckpointManifest` contracts, plus checkpoint-scoped datastream manifests.

The rest of the train object model still needs to be built explicitly.

What is still missing most clearly from the current vocabulary is:

- deeper checkpoint lineage policy such as checkpoint retention tiers,
  cross-window promotion rules, and cold-restore governance
- broader `ValidatorVerdict` families for trainer-batch and eval-class artifacts

### Current `RolloutArtifact` Shape

`RolloutArtifact` now exists in early form inside `psionic-train`. The current
shape already includes at least:

- `worker_id`
- `policy_revision`
- `environment_ref@version`
- `task_id` or task digest
- `token_ids`
- `logprobs`
- reward or rubric outputs
- termination reason
- proof or validator reference fields
- stable `artifact_digest`

## Current State At A Glance

| Subsystem | Current Status | What Is Real Today |
| --- | --- | --- |
| Runtime training truth | `implemented_early` | `TrainingRecoveryContext`, checkpoint refs, elastic-membership context, device-mesh context, collective context |
| Datastream | `implemented_early` | resumable manifests, checkpoint or dataset bindings, policy-weight control refs, freshness windows, and delivery receipts |
| Collectives | `implemented_early` | elastic mesh observation, bandwidth-aware local/global sync planning, transport-feedback replanning, and benchmark-gated quantized collective policy |
| Train session state | `implemented_early` | membership observation, async checkpoint state, durability transitions, live-recovery planning |
| Data contracts | `implemented_early` | `psionic-data` now owns versioned dataset manifests, tokenizer digests, split declarations, resumable iteration cursors, long-context packing policies, and Apple adapter JSONL import or validation with typed tool-schema augmentation plus tokenizer/prompt-shaping packing lineage |
| Adapters | `implemented_early` | adapter identity, package manifests, hosted adapter binding lineage, and first Apple `.fmadapter` reader/writer plus file-inventory validation |
| Sandbox for RL/train workloads | `implemented_early` | bounded execution, background jobs, warm reusable pools, staged loop inputs, pool acquisition receipts, and repeated agentic iteration receipts now exist in `psionic-sandbox` |
| Training core | `implemented_early` | `psionic-train` now has a typed fixed-budget trainer-step loop, `psionic-ir` now provides reusable reverse-mode autodiff plus explicit detach/training-mode gradient semantics beneath it, the repo-owned Apple adapter execution backend now turns packed Apple dataset batches into adapter-only gradient batches for that loop, the first higher-level Apple SFT lane closes the path through typed training summary plus `.fmadapter` export, and an explicitly separate optional Apple draft-model distillation lane now emits paired draft payloads plus latency or acceptance metadata; optimizer state/residency, step telemetry, and checkpoint restore lineage remain explicit over gradient batches |
| Training run graph | `implemented_early` | `psionic-train` now owns typed runs, contributor-set revisions, topology revisions, persistent participant ranking, heartbeats, departures, and window transitions |
| Orchestrator | `implemented_early` | `psionic-train` now owns typed window-control, assignment posture, rollout-assignment refs, rollout-admission receipts, bounded off-policy freshness budgets, rollout-worker heartbeats, claims, upload receipts, and trainer-batch assembly requests over the run graph |
| Environment ABI | `implemented_early` | `psionic-environments` now owns the package ABI, versioned key, workload/policy/difficulty/benchmark package shape, tool/rubric contracts, deterministic runtime session state machine, and a reusable Apple adapter train/eval/benchmark bundle with typed runtime refs plus train/eval parity receipts, while registry and authority truth remain in kernel/Nexus |
| Eval runtime | `implemented_early` | `psionic-eval` now owns held-out eval runs, rubric-scored sample/runtime contracts, benchmark packages, repeat-run aggregation, local validator simulation, and Apple adapter held-out plus benchmark harnesses with structured-output, tool-call, and runtime-smoke receipts, while kernel/Nexus still own canonical eval-run authority truth |
| Synthetic-data flows | `partial_outside_psionic` | synthetic-data job creation, append, finalize, and verification flows exist in kernel/Nexus, but no Psionic-native generation runtime exists yet |
| Rollout artifacts | `implemented_early` | `psionic-train` now has checkpoint-aware policy revisions, proof-bearing rollout artifacts, rollout-admission receipts, bounded stale-rollout pruning, and deterministic trainer-batch assembly with policy-lineage digests |
| Validator-aware RL verification | `implemented_early` | `psionic-train` now owns rollout-verification bundles, replay or duplicate detection, sampled benchmark checks, and typed validator verdicts; broader service productization is still later |

## Current Crate Ownership

The current train-relevant ownership split in Psionic is:

- `psionic-runtime`
  - reusable runtime truth for training recovery, device meshes, collectives,
    and work classes such as `CollectiveStep` and `CheckpointFlush`
- `psionic-datastream`
  - resumable transport for datasets, checkpoints, served artifacts, and
    adapter packages
- `psionic-data`
  - versioned dataset manifests, tokenizer digests, split declarations,
    streamed iteration contracts, long-context packing rules, and Apple
    adapter dataset import or validation with typed schema/tool augmentation
- `psionic-collectives`
  - elastic mesh observation, local/global sync planning, transport-feedback
    replanning, and benchmark-gated collective policy
- `psionic-environments`
  - environment package ABI, execution entrypoints, tool and rubric hooks,
    artifact expectations, versioned dataset bindings, deterministic runtime
    sessions, and reusable Apple adapter train/eval/benchmark bundle helpers
- `psionic-eval`
  - held-out eval runs, rubric-scored sample/runtime contracts, benchmark
  packages, repeat-run aggregation, operator-local validator simulation, and
  Apple adapter held-out/benchmark/runtime-smoke harnesses
- `psionic-train`
  - training-session truth for checkpointing, live recovery,
    elastic-membership posture, typed run graphs, contributor-set revisions,
    window lifecycle, the fixed-budget training-core reference loop, the
    repo-owned Apple adapter reference execution backend, the higher-level
    Apple SFT/export lane, the optional Apple draft-model distillation lane,
    orchestrator state, and RL-facing rollout or batch contracts
- `psionic-adapters`
  - adapter package identity, Apple `.fmadapter` parsing or writing, file
    inventory validation, and hosted binding lineage
- `psionic-sandbox`
  - bounded sandbox execution substrate and background-job lifecycle
- `psionic-cluster`
  - durable ordered-state, cluster admission, catch-up, and topology truth

The broader OpenAgents tree now also has train-adjacent authority surfaces
outside Psionic for:

- environment package descriptors and registry behavior
- compute evaluation-run, training-run, and accepted-outcome lifecycle
- narrow Apple adapter-hosting and Apple-training provider/market projection
  surfaces
- synthetic-data job and verification lifecycle

This is already a meaningful substrate split. The missing work is higher in the
stack.

## What Is Implemented Today

### 1. Runtime-level training truth already exists

`psionic-runtime` already has typed training-class truth surfaces. The most
important ones are:

- `TrainingRecoveryPosture`
  - `SteadyState`
  - `LateJoinPending`
  - `Recovering`
  - `ElasticReconfiguration`
  - `AsyncCheckpointInFlight`
- `TrainingCheckpointAvailability`
  - `None`
  - `AsyncWriteInFlight`
  - `Durable`
- `TrainingElasticMembershipContext`
  - membership epoch
  - cluster-state digest
  - topology digest
  - active, joining, draining, and offline node sets
- `TrainingCheckpointReference`
  - checkpoint family
  - stream id
  - manifest digest
  - object digest
  - writer node
  - membership epoch and topology digests
  - optional logical step and durability timestamp
- `TrainingRecoveryContext`
  - current posture
  - checkpoint availability
  - elastic-membership facts
  - optional latest checkpoint
  - recovering and late-joiner node ids
- `TrainingDeviceMeshAxis`
  - data-parallel, tensor-parallel, pipeline-parallel, and expert-parallel axes
- `TrainingDeviceMeshContext`
  - mesh id, revision, backend, communication class, members, and axes
- `TrainingCollectiveContext`
  - collective kind
  - quantization mode
  - payload bytes
  - wire-byte estimate
  - benchmark justification

This matters because the train system does not start from nothing. The runtime
already has a typed language for recovery, checkpoints, meshes, and collectives.

### 2. Datastream already owns resumable training-class artifact movement

`psionic-datastream` is not training-specific, but it already covers several
training-critical artifact families.

Its subject model already includes:

- `TokenizedCorpus`
- `EvalBundle`
- `Checkpoint`
- `PolicyWeights`
- `ServedArtifact`
- `AdapterPackage`

Its manifests already support:

- payload digesting
- stable chunk descriptors
- dataset bindings
- checkpoint bindings
- policy-weight bindings
- control-plane-visible mirror metadata
- resumable transfer cursors
- restart-safe client progress
- final delivery receipts

That means the train system already has a real substrate for:

- dataset shard transport
- checkpoint transport
- policy-weight shard transport and lightweight control-plane refs
- eval-bundle movement
- adapter-package distribution

What is still missing is not "a data plane exists or not." The missing work is
the broader lifecycle policy over that data plane: richer retention classes,
cross-region mirror governance, and tighter integration with higher-level
orchestrator freshness rules.

### 3. Collective planning already exists

`psionic-collectives` already implements a real, inspectable collective
planning substrate.

The important current pieces are:

- `ElasticCollectivePlanner`
- `CollectiveMeshMember`
- `QuantizedCollectiveBenchmark`
- `CollectiveTransportFeedback`
- `CollectiveSyncCadencePolicy`
- `observe_mesh`
- `record_benchmark`
- `plan_collective`
- `observe_transport_feedback`
- `plan_sync`

The current planner already does several important things honestly:

- validates that declared mesh axes match member count
- ensures mesh members are actually active in the current membership set
- increments mesh revision only when mesh truth changes
- requires explicit benchmark approval before planning a quantized collective
- records transport feedback and surfaces typed replan triggers when bandwidth,
  latency, stream pressure, or mesh revision cross policy boundaries
- plans local subgroup sync separately from full-mesh sync when degraded
  transport and explicit subgroup topology justify it
- emits a `CollectiveExecutionPlan` with:
  - runtime-visible collective posture
  - explicit ring handoffs
  - a low-level `RuntimeWorkItem`
- emits a `CollectiveSyncCadenceReceipt` with:
  - cadence class
  - next global sync step
  - selected quantization
  - transport degradation posture
  - typed replan triggers

This is already enough to say Psionic has training-class collective truth.

It is not enough to say Psionic has a complete distributed optimizer or
end-to-end trainer.

### 4. `psionic-train` already implements session truth for checkpointing and recovery

`psionic-train` currently owns the most concrete part of the train system that
exists today.

Its public API centers on `TrainingSessionState`, which already supports:

- `new`
- `latest_durable_checkpoint`
- `active_checkpoint_write`
- `observe_membership`
- `begin_async_checkpoint`
- `mark_checkpoint_durable`
- `plan_live_recovery`

What that means in practice:

- Psionic can derive elastic-membership epochs from authoritative cluster truth.
- Psionic can begin an async checkpoint only from a checkpoint-scoped
  datastream manifest and only when the writer node is a known ready member.
- Psionic can surface in-flight checkpoint flush work as a typed runtime work
  item.
- Psionic can transition a checkpoint from writing to durable and update the
  durable recovery posture.
- Psionic can derive explicit live-recovery plans for recovering nodes and late
  joiners.

The current recovery action set is already meaningful:

- `ResumeFromDurableCheckpoint`
- `FenceRecoveringNodes`
- `StageCheckpointForLateJoiners`
- `RebalanceWorldSize`
- `BlockUntilDurableCheckpoint`
- `ContinueSteadyState`

That is real train-substrate behavior, not just placeholder nouns.

### 5. Adapter lineage already exists for later train outputs

`psionic-adapters` is not the core training loop, but it is relevant because a
train system eventually needs to emit attributable artifacts.

The adapter subtree already owns:

- `AdapterArtifactIdentity`
- `AdapterPackageManifest`
- target-family and residency semantics
- hosted binding lineage for adapter-backed serving

This means Psionic already has an artifact vocabulary for one class of training
outputs beyond full checkpoints.

### 6. Sandbox substrate exists, and the RL-oriented shape is now early rather than absent

`psionic-sandbox` already owns:

- runtime detection
- profile realization
- bounded job execution
- background jobs
- file transfer
- warm reusable pools
- staged loop inputs
- pool acquisition receipts
- repeated agentic iteration receipts
- execution receipts

This is enough to support bounded compiled runners plus early RL/post-training
iteration contracts.

It is not yet the mature high-throughput RL/post-training sandbox shape. The
remaining gaps are:

- productionized RL throughput and pool tuning
- broader environment-owned lifecycle and policy integration
- stronger operator and security hardening for long-running train workloads

### 7. Environment, eval, training-authority, and synthetic-data truth now spans Psionic runtime crates and authority-owned kernel surfaces

The recent issue closures matter because they changed both Psionic and the
broader system around it.

The tree now has Psionic-native execution crates for:

- environment package ABI and deterministic runtime sessions in
  `psionic-environments`
- held-out eval runs, benchmark packages, repeat-run aggregation, and local
  validator simulation in `psionic-eval`
- one repo-owned Apple adapter training execution lane in `psionic-train`

The tree also has broader OpenAgents support for:

- environment package descriptors and registry behavior
- environment refs bound into compute products and delivery proofs
- evaluation-run creation, sample ingestion, and finalize flows
- training-policy registration, training-run create/finalize flows, and
  accepted-outcome publication
- narrow Apple adapter-hosting capability publication plus matching
  compute-market truth surfaces
- synthetic-data job creation, append, finalize, and verification flows

Those capabilities currently live in kernel/proto and Nexus-control surfaces.

So the accurate reading is:

- Psionic now has native environment and eval runtime clients plus one
  repo-owned Apple training lane inside the compute substrate
- the larger platform owns the canonical authority truth for environment, eval,
  training-run, and accepted-outcome records
- provider and market surfaces now expose one narrow Apple training and
  adapter-hosting projection on top of that authority truth
- synthetic-data still remains `partial_outside_psionic` because there is no
  Psionic-native generation runtime yet

## What Psionic Can Honestly Claim Today

Today Psionic can honestly claim all of the following:

- training-class execution now has typed recovery, checkpoint, mesh, and
  collective truth in reusable crates
- clustered training recovery can be reasoned about with replay-safe session
  state rather than ad hoc logs
- checkpoint transport has a resumable data-plane substrate with delivery
  receipts
- collective planning already has benchmark-gated quantization and explicit mesh
  revisions
- fixed-budget trainer-step execution is real, with explicit optimizer-state
  ownership, residency transitions, and step telemetry
- reusable autodiff plus explicit detach or no-grad semantics now live in
  `psionic-ir` rather than trainer-private code
- reusable SGD, Adam, AdamW, LARS, and LAMB primitives plus distributed-
  optimizer contracts now live in `psionic-train`
- rollout artifacts, trainer-batch assembly, policy revisions, and
  validator-aware verification are first-class typed contracts
- environment ABI and held-out eval runtime now exist in reusable Psionic
  crates
- sandbox execution now supports warm reusable pools and repeated agentic
  iteration receipts
- training-related artifact lineage is now materially first-class data rather
  than opaque side files
- the first repo-owned Apple adapter SFT lane is real in `psionic-train`, and
  the optional Apple draft-model distillation lane is now separate typed
  behavior instead of being implied by one generic training path
- the broader OpenAgents stack now has authority-layer environment, eval,
  training-run, and accepted-outcome flows that Psionic can target as
  execution clients
- the desktop app now has a truthful Apple training operator flow on top of the
  Psionic substrate, including explicit launch, eval, export, and
  accepted-outcome publication boundaries
- the broader stack also projects one narrow Apple adapter-hosting and
  Apple-training truth path into provider and compute-market surfaces, without
  pretending that broader train procurement is complete

That is a meaningful base.

## What Psionic Cannot Honestly Claim Yet

Psionic cannot honestly claim any of the following yet:

- full production-scale Rust-native model training across real multi-device
  runtime kernels
- full production-scale Rust-native RL or post-training throughput
- broad autodiff coverage across every future backend-extension and training op
- true multi-device execution kernels and ZeRO or FSDP transport and partition
  exchange
- fully mature checkpoint retention, promotion, and cold-restore governance
- broad kernel-backed accepted-outcome authority for every train artifact and
  lifecycle beyond the current Apple adapter path
- full security hardening, chaos coverage, and operator lifecycle for the train
  stack
- a broad provider-market training family or buyer-facing procurement surface
  on top of the current Apple reference lanes
- the broader research-loop or productization program beyond the current
  reference runs

Those are still planned.

## The Gap, Precisely

The gap is no longer "there is no train subtree."

The gap is:

> Psionic now has early trainer, orchestrator, rollout, environment, eval,
> validator, and reusable framework-core gradient or update substrate, but it
> still lacks the runtime breadth, hardening, and operator or product layers
> required for a complete distributed train system.

That gap is the main planning target for the rest of this doc.

## Target Train System

The target Psionic train system should be six explicit subsystems.

### 1. Training core

Owns:

- training graph or backward substrate
- optimizer state
- optimizer-state residency, offload, and prefetch policy
- gradient update policy
- checkpoint save and restore
- trainer step loop
- step-level training telemetry such as grad, update, and parameter norms

This is the engine that does the actual learning work.

### 2. Orchestrator

Owns:

- participant roles
- training-window creation, seal, score, and reconcile transitions
- rollout scheduling
- deterministic assignment for contributor, batch, and eval slices
- batch assembly
- off-policy budgeting
- policy revision tracking
- stage transitions
- online eval interleaving

This is the control plane for the train system.

### 3. Data plane

Owns:

- dataset transport
- checkpoint transport
- policy-weight broadcast
- eval-bundle transport
- artifact freshness and replay posture

This extends the current `psionic-datastream` substrate.

### 4. Environments and eval

Owns:

- environment package ABI
- benchmark package and validator-owned reference benchmark profiles
- rollout execution contracts
- tool and multi-turn abstractions
- reward and rubric contracts
- repeat-run scoring and robust aggregation rules
- operator-local validator simulation against the same packaged benchmark
  environment
- offline and online eval over the same environment definition

This is where environment-bound training becomes honest.

### 5. Validation and adjudication

Owns:

- rollout-verification bundles
- cheap universal checks
- sampled expensive checks
- stale or malformed rollout rejection
- timer, token-accounting, and final-state verification where a benchmark or
  validator package requires them
- declared execution-strategy verification for benchmark-class workloads
- validator verdict artifacts

This is the integrity loop for untrusted or semi-trusted rollout workers.

### 6. Operator and authority integration

Owns:

- training receipts
- topology and checkpoint inspection
- validator posture inspection
- environment version visibility
- accepted-outcome export into market or kernel truth when appropriate

This is how the train system becomes operable instead of remaining a research
toy.

## Canonical Planned Role Split

The full train system should separate these roles explicitly.

### Trainer

Trusted execution responsible for:

- reading trainer batches
- applying gradient updates
- producing new checkpoints or policy revisions
- emitting step and checkpoint receipts

### Orchestrator

Trusted control plane responsible for:

- scheduling rollouts
- assigning workers
- maintaining persistent participant ranking
- selecting bounded contributor sets from a wider active population
- enforcing freshness windows
- assembling trainer batches
- coordinating evaluation
- feeding the trainer the right artifacts

### Rollout workers

Untrusted or semi-trusted execution responsible for:

- generating trajectories or outputs against a declared policy revision
- returning typed rollout artifacts
- attaching enough metadata for validator review

### Validators

Integrity checkers responsible for:

- universal schema checks
- sampling-shape checks
- termination checks
- stale-policy checks
- duplicate or copycat detection
- contribution normalization and ranking feedback
- sampled high-cost verification when economics justify it

### Environment runtime

Trusted execution substrate responsible for:

- package loading
- stateful multi-turn task execution
- tool invocation
- reward or rubric application
- sandbox-bound execution where required

### Data-plane services

Responsible for:

- checkpoint and weight transfer
- resumable corpus delivery
- manifest and digest verification
- freshness and retention policy

### Contributor Selection And Ranking

The mature train system should treat active participants and contributing
participants as different sets.

That means:

- the system may keep a wider population admitted and heartbeat-visible
- only a bounded contributor set should actually produce work in a given round,
  interval, or trainer window
- contributor selection should consider freshness, persistent ranking, topology,
  and diversity rather than only "who asked first"
- duplicate or copycat behavior should reduce effective contribution weight and
  feed back into future participant ranking

This is the cleanest way to keep elastic membership open without letting every
active participant distort batch quality or network cost.

### Control Plane Versus Heavy Artifact Plane

The train control plane should not carry the heavy payloads.

The intended split is:

- the orchestrator, validators, and operator surfaces exchange run ids,
  artifact refs, digests, policy ids, and receipts
- checkpoints, policy weights, datasets, rollout payloads, and eval bundles
  move through the heavy artifact plane in `psionic-datastream`

This keeps control messages lightweight and replayable while the actual bytes
stay in the resumable artifact substrate.

## Canonical Planned Lifecycle

The mature Psionic train lifecycle should look like this:

1. A training run is created with stable run identity, policy, environment, and
   checkpoint lineage.
2. The orchestrator forms or revises the participant topology, contributor set,
   and current `TrainingWindow`.
3. The collective planner materializes the device mesh and collective posture.
4. The heavy artifact plane stages the active checkpoint, policy weights, and
   dataset or environment artifacts while the control plane carries only refs,
   digests, and policy posture.
5. Only the selected contributor subset begins rollout or trainer work under
   explicit policy, assignment, and freshness constraints.
6. The window transitions through explicit control states such as `planned`,
   `active`, `sealed`, `scored`, and `reconciled` as work is accepted and
   judged.
7. Rollout artifacts or trainer-step inputs are validated and assembled into
   trainer batches.
8. The trainer advances one or more steps and emits step-level metrics,
   receipts, and optional checkpoints.
9. Async checkpoint flushes begin and later transition to durable state.
10. Recovery, late join, reconfiguration, or eviction events update the run
   topology and checkpoint posture.
11. Online and offline eval may run against the same environment contract or
    benchmark package contract.
12. Accepted outcomes produce durable train and eval receipts and later, when
    market-relevant, can flow into kernel truth.

The current repository implements only pieces of steps 2, 3, 4, 9, and 10.

## Planned Run State Machine

The mature train system should give operators and controllers a small explicit
run-state machine.

| `TrainingRunStatus` | Meaning |
| --- | --- |
| `planned` | run identity exists but execution has not started |
| `initializing` | artifacts, participants, and execution substrate are still being prepared |
| `active` | trainer and rollout work are progressing normally |
| `recovering` | the run is reconfiguring or resuming from checkpoint-backed state |
| `paused` | the run is intentionally halted without being terminal |
| `completed` | the run reached a successful terminal outcome |
| `failed` | the run reached a terminal failure outcome |

The runtime and operator surfaces should not infer these states indirectly from
scattered logs. They should be first-class train truth.

## Training Time Semantics

Training execution depends on explicit time boundaries.

The most important ones are:

- policy freshness windows
- rollout expiry windows
- checkpoint cadence
- contributor reselection intervals
- validator sampling or adjudication intervals
- environment timeout limits
- sandbox reuse and pool lifetime limits

These time boundaries sit above the generic execution timing defined in
`ARCHITECTURE.md` and should be recorded in train policy and receipts where
they affect acceptance or rejection.

## Canonical Train Receipts

OpenAgents is receipt-first, so the train system needs explicit receipt
families rather than vague references to "logs" or "artifacts."

Today the repo already has some lower-level receipt substrate:

- `DatastreamDeliveryReceipt`
- sandbox execution receipts
- runtime execution-proof bundles
- checkpoint and recovery contexts that can feed later receipts

The first train-specific receipt family now exists through
`RolloutAdmissionReceipt`, but the mature train system should still emit at
least these broader receipts.

| Receipt | Purpose | Minimum Contents |
| --- | --- | --- |
| `TrainingRunReceipt` | One durable summary for a full run or run stage | run id, stage id, policy ids, environment refs, checkpoint lineage, validator posture, final outcome |
| `TrainingWindowReceipt` | One durable record for one contributor or trainer window transition | run id, stage id, window id, contributor-set revision, policy revision, transition state, validator posture |
| `TrainerStepReceipt` | One accepted optimizer step | run id, stage id, step id, trainer batch digest, policy revision in and out, optimizer policy, checkpoint linkage |
| `CheckpointReceipt` | One checkpoint creation or durability event | run id, stage id, checkpoint family, manifest digest, object digest, writer identity, durability state |
| `RolloutReceipt` | One rollout artifact and its acceptance result | run id, worker id, policy revision, environment version, rollout digest, reward and termination posture, acceptance result |
| `ValidatorReceipt` | One validator verdict over a rollout, batch, or eval artifact | validator policy id, sampled or universal check class, referenced artifact digests, verdict, reason codes |
| `EvalReceipt` | One online or offline evaluation result | eval run id, environment version, rubric version, policy revision, artifact digests, score summary |

The most important design rule is simple:

> every economically or operationally important train event should have a typed
> receipt family, not only a log line or an in-memory state transition.

Train objects define the durable execution vocabulary; receipts record accepted
state transitions and outcomes over those objects.

## Policy Surfaces

The full train system should make the configurable policy surfaces explicit.
The spec should say not only what happens, but what operators and higher-level
controllers are allowed to tune.

| Policy Surface | What It Governs |
| --- | --- |
| `TrainingPolicy` | trainer step budget, training-window cadence, checkpoint cadence, optimizer posture, gradient clipping, contributor caps, stage transitions, halt policy |
| `EnvironmentPolicy` | admissible environment packages, tool access, state persistence, reward and rubric posture |
| `ValidatorPolicy` | universal checks, sampled expensive checks, stale-policy tolerances, duplicate-detection posture, contribution normalization, benchmark verification posture, rejection posture, penalty posture |
| `CollectivePolicy` | mesh layout, sync cadence, quantization mode, replan triggers, communication class |
| `SandboxPolicy` | allowed profiles, warm-pool behavior, runtime limits, filesystem or network posture, retry behavior |
| `ArtifactPolicy` | artifact freshness windows, retention classes, replay rules, archival posture, provenance requirements |

Current repo truth only covers a small piece of this policy surface directly:

- collective quantization approval, benchmark posture, sync cadence, and
  transport thresholds
- cluster admission and readiness posture
- checkpoint durability posture
- sandbox profile realization

Most train policy remains to be formalized.

### Example Policy Values

The policy surfaces above become easier to reason about when rendered with
concrete examples.

| `TrainingPolicy` Field | Example Value |
| --- | --- |
| `max_policy_drift` | `3 revisions` |
| `checkpoint_interval` | `1000 steps` |
| `gradient_clip_norm` | `1.0` |
| `halt_on_entropy_drop` | `true` |
| `max_rollout_age_ms` | `30000` |
| `max_contributing_workers` | `256` |

### Policy Revision Propagation

Policy revisions should propagate through the data plane as staged artifacts,
not as implicit mutable state.

The intended model is:

- the trainer emits a new policy revision or checkpoint-backed weight state
- the revision is published through `psionic-datastream` as a staged artifact
- the orchestrator enforces freshness and admissibility before assigning work
- rollout workers and evaluators must bind their outputs to the specific policy
  revision they consumed

This keeps policy lineage replay-safe and validator-reviewable.

Control-plane coordination should carry refs, digests, and policy ids rather
than embedding the heavy policy payloads directly in orchestration messages.

## Training Failure Semantics

The train system needs explicit failure handling, not only a list of failure
classes. The table below describes the expected control policy for the mature
system.

| Failure Type | Expected System Response |
| --- | --- |
| rollout worker crash | replay or reassign the rollout task and mark prior claim incomplete |
| stale or mismatched policy revision | reject the rollout artifact and emit a stale-policy receipt |
| duplicate or copied rollout | reject or deweight the artifact, emit duplicate-detection reason codes, and update participant ranking |
| validator rejection | discard or quarantine the referenced rollout or batch and record reason codes |
| checkpoint flush failure | block any state transition that requires durability and keep the run in non-durable posture |
| orchestrator crash | resume from durable orchestrator state and latest accepted checkpoint lineage |
| trainer crash | restart from the latest durable checkpoint and replay admissible pending control-plane state |
| environment package mismatch | reject execution before rollout start and emit environment-mismatch reason codes |
| sandbox runtime failure | terminate the affected task, record runtime and profile identity, and apply retry or quarantine policy |
| topology shock or node loss | trigger elastic reconfiguration, recovery planning, and possibly world-size rebalance |
| datastream interruption | resume from the last committed cursor rather than restart blind transfer |

The system should never collapse these into one generic "training failed"
outcome. Failure handling is part of train truth.

Orchestrator durability and trainer durability are related but distinct; loss
of one must not silently imply loss of the other.

## Security Model

The train system explicitly allows for partially trusted and untrusted roles, so
the threat model belongs in the spec and not only in later issue descriptions.

| Threat | Mitigation Direction |
| --- | --- |
| malicious rollout workers | validator sampling, schema checks, stale-policy rejection, worker admission controls |
| artifact poisoning or tampering | manifest digests, object digests, provenance requirements, signed artifacts where policy requires |
| checkpoint tampering | datastream manifest verification plus checkpoint-family and writer identity linkage |
| environment compromise | signed or pinned packages, sandbox policy, version pinning, package admissibility policy |
| policy drift | explicit policy revisions, freshness windows, off-policy budget enforcement |
| copied or replayed rollouts | duplicate detection, artifact-digest lineage, contribution normalization, and participant-ranking penalties |
| worker spam or flooding | task-claim limits, admission control, rate limiting, and orchestrator-side pruning |
| orchestrator inconsistency | durable orchestrator state and replay-safe receipts |
| validator abuse or misconfiguration | validator policy versioning, sampled check receipts, adjudication reason codes |

The current repo already helps here in a limited way through:

- manifest and chunk digests in `psionic-datastream`
- explicit checkpoint identity and writer linkage in `psionic-runtime`
- benchmark-gated collective posture in `psionic-collectives`
- bounded profile and execution receipts in `psionic-sandbox`

The broader train security model is still planned.

## Train Artifact Retention Model

Retention policy affects reproducibility, cost, and later authority linkage, so
it should be named now even before enforcement exists.

| Artifact Class | Expected Retention |
| --- | --- |
| durable checkpoints | long-term or archival, because they anchor recovery and promotion lineage |
| trainer-step receipts | long-term, because they define accepted optimization history |
| rollout artifacts | medium-term by default, with longer retention for sampled, disputed, or promoted artifacts |
| validator receipts and proof refs | long-term, because they justify acceptance or rejection outcomes |
| eval summaries | long-term, because they anchor quality and release decisions |
| raw sandbox traces and transient logs | short-term by default unless attached to an incident or dispute |

The retention table does not imply the implementation already exists. It defines
the operating model the train stack should eventually enforce.

## What The Intellect Papers Change For Psionic

The March 13 audit remains directionally correct. The useful lessons from the
Intellect papers are still these.

### From INTELLECT-1

Psionic should take:

- explicit elastic topology as first-class truth
- join and recovery modes as policy rather than ad hoc behavior
- heartbeat and explicit departure semantics
- bandwidth-aware background replanning
- quantized sync only when benchmark-justified and receipt-bearing

Psionic should not copy:

- their exact Python or PyTorch stack
- their exact transport stack
- one specific pretraining topology as permanent architecture truth

### From INTELLECT-2

Psionic should take:

- trainer, orchestrator, rollout worker, and validator as distinct roles
- policy-weight distribution as its own data plane
- untrusted rollout validation with cheap universal checks and sampled expensive
  checks
- explicit off-policy budgets
- first-class curriculum and filtering
- instability telemetry as product truth

Psionic should not copy:

- one GRPO recipe as the permanent train contract
- one relay or firewall model as the only architecture
- one economic or ledger substrate as the product base layer

### From INTELLECT-3

Psionic should take:

- environment packages as independent products
- one environment contract for training and eval
- multi-turn and tool-using environments as first-class abstractions
- RL-oriented sandbox throughput
- stage transitions from SFT to agentic SFT to RL
- orchestrator state as core product truth

Psionic should not copy:

- their exact Python environment module system
- their exact Kubernetes control plane
- their exact optimizer or MoE decisions as architecture truth

## All-Rust Implication

If OpenAgents means "no Python trainer and no Python environment system," then
the completion bar is high.

An honest all-Rust Psionic train system now exists in early form across all of
these layers inside the Rust subtree:

- training core
- optimizer ownership
- rollout artifacts
- environment ABI
- data and corpus contracts
- eval runtime
- compiled runner and crash boundary

The completion bar is still high, though.

Psionic cannot honestly claim a finished all-Rust train system until
multi-device execution kernels, broader autodiff or operator coverage, mature
environment execution at scale, hardening, and operator-grade lifecycle
management all exist inside the Rust subtree.

## Planned Crate Shape

The most likely mature crate shape is:

- `psionic-train`
  - training core, run graph, checkpoint lineage, trainer state, orchestrator
    contracts
- `psionic-collectives`
  - mesh and collective planning, quantized sync policy
- `psionic-datastream`
  - dataset, checkpoint, policy-weight, and eval-bundle transport
- `psionic-eval`
  - shared online and offline evaluation runtime
- `psionic-data` or `psionic-datasets`
  - dataset manifests, tokenizer state, splits, packing, and curriculum facts
- `psionic-environments`
  - environment ABI, runtime sessions, and package-loading contracts
- `psionic-sandbox`
  - pooled execution substrate for environment-bound agentic workloads
- `psionic-adapters`
  - later train-output lineage for adapters and promoted derived artifacts

This is the architectural direction. It is not all implemented today.

The planned crate shape is canonical for current ownership direction, but it is
not a guarantee that every future subsystem lands under exactly these final
crate names.

## Current-To-Target Matrix

| Area | Current Repo Truth | Target Repo Truth |
| --- | --- | --- |
| Checkpoint lineage | present in `psionic-train` and `psionic-runtime` | durable checkpoint families, promotion, replay, and restore across full training programs |
| Elastic membership | present in `psionic-runtime` and `psionic-train` | full participant lifecycle with heartbeats, rejoin, eviction, and topology history |
| Collective planning | present in `psionic-collectives` | full local/global sync planning with distributed optimizer integration |
| Weight broadcast | present in `psionic-datastream` | staged policy-weight broadcast with freshness cutoffs and relay policy |
| Training steps | typed fixed-budget reference loop present | broader Rust-native trainer-step engine |
| RL rollouts | typed rollout, bounded stale-rollout budgeting, and worker-protocol contracts present | validator-ready lineage and sampled adjudication |
| Environment ABI | typed runtime ABI plus typed package shape present | broader package loading, composition, and environment system |
| Eval runtime | present in `psionic-eval` | shared online/offline eval and rubric runtime, benchmark packages, and local validator simulation |
| Sandbox throughput | bounded one-shot substrate exists | RL-throughput warm pools and repeated environment loops |
| Validators for RL | rollout-verification bundles and sampled adjudication contracts present | broader service productization, batch-level adjudication, and authority integration |
| Operator surfaces | app-owned desktop-control and `autopilotctl` surfaces now exist on top of Psionic, but Psionic still does not own its own operator crate or full train operator plane | inspection, diagnostics, and receipts across all train subsystems |

## Path To Completion

The path from the current repo to a real train system is best read in four
waves.

### Wave 0: implemented substrate

Already in tree:

- runtime training truth
- datastream manifests and receipts
- collective planning substrate
- session checkpoint and recovery substrate
- adapter lineage substrate
- bounded sandbox execution substrate

### Wave 1: implemented early all-Rust train platform

Now in tree:

- fixed-budget training core with reusable autodiff and optimizer layers under
  it
- rollout artifacts, policy-lineage contracts, worker protocol, and
  validator-ready verification bundles
- environment ABI plus environment registry helpers
- data, tokenizer, split, and packing contracts
- held-out eval runtime plus Apple held-out, benchmark, and runtime-smoke
  harnesses
- run graph, checkpoint lineage, orchestrator state machine, off-policy
  budgeting, and scheduling/economics contracts
- RL-throughput sandbox primitives
- repo-owned Apple training execution backend, Apple SFT/export lane, and
  optional Apple draft-model distillation lane

### Wave 2: remaining productization and scaling

Needed next:

- true multi-device execution kernels and distributed optimizer integration
- memory-sharding, partition exchange, and broader collective or optimizer
  integration at scale
- broader model, tokenizer, and artifact-format interoperability
- stronger security, provenance, and authority integration beyond the current
  Apple accepted-outcome path
- mature artifact retention, promotion, and cold-restore governance
- broader operator lifecycle and market/product surfaces beyond the current
  app-owned Apple reference workflow

### Later scope

After the above:

- model promotion and release governance
- human preference and critique ingestion

Those later items matter, but they are not prerequisites for the core
environment-first Intellect-style train stack.

## Proposed GitHub Issue Program

The issue program below is written from the current repository state, not from
the older "there is no `psionic-train` crate" assumption.

This program first landed as issues `#3564` through `#3593` and was later
extended by the framework-core follow-ons `#3602` and `#3603`, plus the
Apple-lane closures `#3616` through `#3631`.

### Core Platform Build-Out

### 1. `Psionic Train: complete the Rust-native training core beyond recovery substrate`

Status: implemented on 2026-03-14 via GitHub issue `#3564`.

Added `psionic-train` fixed-budget training-core types and behavior for:

- typed parameter groups
- explicit optimizer-state ownership
- optimizer-state residency policy and transitions
- machine-legible step telemetry for gradient, update, and parameter norms
- visible window and cadence scheduling
- checkpoint-anchored restore via `TrainingSessionState`

Issue `#3603` extends that core with a reusable optimizer layer in
`src/optimizer.rs` so SGD, Adam, AdamW, LARS, and LAMB step semantics are no
longer trainer-private. The fixed-budget loop now composes with the reusable
optimizer surface instead of carrying its own ad hoc update implementation.

Issue `#3602` adds reusable autodiff underneath that loop in `psionic-ir`:
explicit gradient-bearing graph construction, an IR-level `detach` op,
training/evaluation plus no-grad posture, symbolic backward plans, dense
reference materialization, and a trainer-integration proof that the resulting
gradients can feed the fixed-budget training core without trainer-local
gradient logic.

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAINING_CORE_FIXED_BUDGET_REFERENCE.md`
- `scripts/release/check-psionic-training-core.sh`

The current step path is intentionally an explicit-gradient reference loop over
`f32` tensor payloads, but it no longer implies trainer-private gradient logic.
Autodiff and optimizer behavior now live in reusable lower Psionic layers,
while broader operator-family coverage and higher-order training behavior still
remain future work.

On 2026-03-15, GitHub issue `#3631` added the missing repo-owned Apple
training execution backend inside `psionic-train`:

- validation over the repo-owned Apple dataset tokenizer and prompt-shaping
  lineage plus the SFT-capable Apple environment bundle
- deterministic Apple sample batching on top of the packed dataset contract
- adapter-only parameter selection with frozen-base semantics
- repo-owned forward/loss and low-rank gradient production that feeds
  `TrainingGradientBatch` and `FixedBudgetTrainingRun`
- explicit training-posture declaration for the currently supported `f32`
  reference precision path and disabled activation checkpointing

This lands the learning computation itself for the first Apple lane.

On 2026-03-15, GitHub issue `#3625` then added the higher-level Apple SFT lane
on top of that backend:

- fixed-budget step execution across the repo-owned Apple batches
- typed step receipts and final training summary for the Apple run
- initial/final adapter-only portable bundle snapshots plus derived adapter
  delta
- reproducibility metadata suitable for later authority publication
- valid `.fmadapter` export through `psionic-adapters`

That means the first honest Rust-native Apple adapter SFT path is now real in
repo code.

On 2026-03-15, GitHub issue `#3626` added the explicitly separate optional
Apple draft-model distillation lane on top of that SFT path:

- fixed-budget teacher/student distillation over the repo-owned Apple batches
- explicit teacher/draft runtime pairing and dual-precision posture capture
- deterministic latency and speculative-acceptance accounting in typed batch
  records and summary output
- portable draft checkpoint export plus paired `draft.mil` or
  `draft_weights.bin` payload emission inside `.fmadapter`

Authority publication and desktop workflow are now real through the later
Apple-lane issue closures. Broader provider-market training truth and generic
market claims beyond the current Apple reference path remain later work.

### 2. `Psionic RL: define rollout artifacts, trainer batches, and policy-lineage contracts`

Status: implemented on 2026-03-14 via GitHub issue `#3565`.

Added `psionic-train` RL-facing contracts for:

- checkpoint-aware `PolicyRevision`
- proof-bearing `RolloutArtifact`
- deterministic `TrainerBatch` assembly
- explicit `PolicyRevisionLineage`

The canonical runbook and harness are now:

- `crates/psionic/docs/ROLLOUT_ARTIFACT_POLICY_LINEAGE_REFERENCE.md`
- `scripts/release/check-psionic-rl-rollout-artifacts.sh`

This issue makes rollout payloads, trainer-batch assembly, and policy lineage
real and reusable. It does not yet claim freshness enforcement, worker
protocols, validator adjudication, or full orchestration.

### 3. `Environments: define a Rust-native environment ABI and runtime contract`

Status: implemented on 2026-03-14 via GitHub issue `#3566`.

Added the `psionic-environments` crate for:

- canonical `environment_ref@version` package identity
- Rust-native environment package ABI
- execution entrypoints, tool interfaces, rubric hooks, and artifact
  expectations
- deterministic runtime sessions with turn, tool, artifact, and rubric receipts

The canonical runbook and harness are now:

- `crates/psionic/docs/ENVIRONMENT_ABI_REFERENCE.md`
- `scripts/release/check-psionic-environment-abi.sh`

Kernel and Nexus still own registry and authority truth. This issue lands the
Psionic-side runtime and contract layer only.

On 2026-03-15, GitHub issue `#3622` extended the same crate with a reusable
Apple adapter environment bundle:

- a shared train/eval core package plus a benchmark-only package over the same
  typed environment ABI
- explicit Apple session/runtime, tool-bundle, rubric-binding, and
  structured-output refs carried as package metadata that now affects package
  digests
- train/eval group composition that proves the same pinned core package is
  reused across both surfaces through an explicit parity receipt

### 4. `Psionic Data: add Rust-native dataset, tokenizer, split, and packing contracts`

Status: implemented on 2026-03-14 via GitHub issue `#3567`.

Added the `psionic-data` crate for:

- canonical `dataset_ref@version` identity through `DatasetKey`
- typed dataset manifests bound to tokenizer digests and tokenized shard refs
- split declarations over `psionic-datastream` manifest refs with explicit
  shard-level sequence and token counts
- resumable streamed iteration contracts with deterministic shard ordering and
  epoch-wrap semantics
- sequence-packing and batch-packing policies for long-context workloads

The canonical runbook and harness are now:

- `crates/psionic/docs/DATASET_TOKENIZER_PACKING_REFERENCE.md`
- `scripts/release/check-psionic-data-contracts.sh`

This issue keeps byte movement in `psionic-datastream` but makes data lineage,
iteration, and packing policy first-class typed Psionic contracts. The
environment ABI now binds versioned dataset keys from this layer instead of
free-form dataset refs.

On 2026-03-15, GitHub issue `#3621` extended that same crate with the first
repo-owned Apple adapter dataset path:

- UTF-8 JSONL import into typed Apple message, tool, and guided-generation
  records
- fixture-backed validation of message ordering, tool definitions, and
  `response_format` schema requirements
- explicit tokenizer and prompt-shaping lineage metadata for later train/eval
  reuse
- deterministic Apple sample packing over explicit prompt/completion/tool/schema
  token captures, with typed refusal on tokenizer or prompt-shaping drift

### 5. `Psionic Eval: create the Rust-native eval and rubric runtime`

Status: implemented on 2026-03-14 via GitHub issue `#3568`.

Added the `psionic-eval` crate for:

- held-out eval-run contracts and local eval-run state machines
- rubric-scored sample construction directly from `psionic-environments`
  session summaries
- durable eval summaries with machine-legible aggregate metrics and artifacts
- explicit online/offline parity through one shared sample/runtime contract
- validator-style `BenchmarkPackage` contracts with repeat-run aggregation and
  operator-local validator simulation
- typed verification facts for timer integrity, token accounting, final-state
  capture, and declared execution strategy

The canonical runbook and harness are now:

- `crates/psionic/docs/EVAL_RUNTIME_REFERENCE.md`
- `scripts/release/check-psionic-eval-runtime.sh`

Kernel and Nexus still own canonical eval-run authority truth. This issue lands
the reusable Psionic-side runtime and benchmark-contract layer only.

On 2026-03-15, GitHub issue `#3623` extended that same crate with repo-owned
Apple adapter eval harnesses:

- held-out and benchmark scoring over imported Apple dataset fixtures plus
  observed candidate outputs
- explicit structured-output conformance and tool-call coverage checks
- bridge-backed runtime-smoke receipts that prove a `.fmadapter` package parses,
  loads, attaches, and runs against the Apple lane
- typed failure separation between dataset/config problems, package
  incompatibility, and bridge/runtime refusal

### 6. `Psionic Train: define canonical run graph, topology revisions, and participant lifecycle`

Status: implemented on 2026-03-14 via GitHub issue `#3569`.

Added run-graph contracts inside `psionic-train` for:

- stable run ids, stage ids, topology revisions, contributor-set revisions, and
  `TrainingWindow` ids
- explicit participant admission, readiness, contribution, departure, and
  suspension state
- persistent participant ranking and deterministic contributor reselection
- heartbeat, departure, rejoin, and contributor-suspension lifecycle events
- replay-safe window planning with deterministic batch/eval slice assignment
- machine-legible window transitions through `planned`, `active`, `sealed`,
  `scored`, and `reconciled`

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_RUN_GRAPH_REFERENCE.md`
- `scripts/release/check-psionic-train-run-graph.sh`

This issue makes the run graph and participant lifecycle explicit typed Psionic
truth instead of a scheduler convention. It does not yet land full
orchestrator, checkpoint-pointer, or batch-propagation policy.

### 7. `Psionic Train: extend checkpoint lineage, recovery modes, and catch-up receipts`

Status: implemented on 2026-03-14 via GitHub issue `#3570`.

Added checkpoint-lineage and restore-ladder contracts inside `psionic-train`
for:

- typed `CheckpointPointer` and `CheckpointManifest` objects over explicit run,
  stage, or window scope
- explicit durability posture on checkpoint manifests, including partial-upload
  versus durable restore eligibility
- declared `TrainingRecoveryMode` choices for blocking catch-up, overlapped
  catch-up, and resume-from-last-stable-checkpoint
- pointer-first restore planning with manifest-listing fallback when the latest
  pointer is missing, stale, or references non-durable state
- deterministic shard-uploader assignment over the accepted restore manifest
- fake object-store tests covering missing pointer, stale pointer,
  partial-upload, and listing-limit failure paths

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_CHECKPOINT_RECOVERY_REFERENCE.md`
- `scripts/release/check-psionic-train-checkpoint-recovery.sh`

This issue turns checkpoint recovery from implicit latest-checkpoint heuristics
into typed restore receipts that can explain why one source was preferred over
another. It does not yet land retention policy, cold-restore classes, or
cross-window checkpoint governance.

### 8. `Psionic Collectives: add bandwidth-aware elastic sync planning and quantized policy surfaces`

Status: implemented on 2026-03-14 via GitHub issue `#3571`.

Added sync-planning and cadence-policy contracts inside `psionic-collectives`
for:

- mesh-wide `CollectiveTransportFeedback` observations with stable digests and
  explicit bandwidth, latency, and stream-pressure metrics
- `CollectiveSyncCadencePolicy` over healthy and degraded global-sync
  intervals, transport thresholds, and local/global quantization posture
- `CollectiveSyncExecutionPlan` and `CollectiveSyncStage` so local subgroup
  sync and full-mesh sync are planned as explicit ordered stages
- `CollectiveSyncCadenceReceipt` and `CollectiveReplanTrigger` so cadence,
  transport degradation, quantization fallback, and interval-elapse decisions
  stay machine-legible
- planner-owned local-group selection, interval-based deferred global sync, and
  mesh-revision replanning over the existing benchmark-gated quantized
  collective substrate

The canonical runbook and harness are now:

- `crates/psionic/docs/COLLECTIVE_SYNC_POLICY_REFERENCE.md`
- `scripts/release/check-psionic-collective-sync.sh`

This issue makes collective sync cadence explicit Psionic truth instead of a
hidden optimizer-side heuristic. It does not yet land distributed optimizer
state integration or parameter-shard accounting.

### 9. `Psionic Datastream: add sharded policy-weight broadcast and freshness control`

Status: implemented on 2026-03-14 via GitHub issue `#3572`.

Added policy-weight broadcast contracts inside `psionic-datastream` for:

- explicit `PolicyWeights` subject identity plus `DatastreamPolicyWeightBinding`
  over policy id, revision, shard identity, assembled-artifact digest, and
  freshness window
- lightweight `DatastreamPolicyWeightControlPlaneRef` and
  `DatastreamPolicyWeightBroadcastManifest` objects so orchestrators can carry
  refs, digests, and mirror metadata instead of heavy payload bytes
- mirror or relay metadata through `DatastreamMirrorLocator`
- stale-artifact rejection at control-plane-ref export time
- `InMemoryPolicyWeightBroadcast` and
  `DatastreamPolicyWeightBroadcastReceipt` for pipelined multi-shard delivery
  over the existing resumable chunk path
- tests proving the control-plane summary stays smaller than the heavy artifact
  bytes while the heavy artifact plane remains resumable and byte-accountable

The canonical runbook and harness are now:

- `crates/psionic/docs/POLICY_WEIGHT_BROADCAST_REFERENCE.md`
- `scripts/release/check-psionic-policy-weight-broadcast.sh`

This issue makes the heavy artifact plane versus lightweight control plane
split explicit for policy weights. It does not yet land orchestrator-owned
assignment or rollout freshness budgets.

### 10. `Psionic Train: build the orchestrator state machine and trainer-batch assembly contracts`

Status: implemented on 2026-03-14 via GitHub issue `#3573`.

Added the first orchestrator module inside `psionic-train` for:

- typed `TrainingOrchestratorState` over the existing run graph, target policy
  revision, and lightweight policy-weight broadcast manifest
- orchestrator ownership of contributor selection, window planning, window
  activation, sealing, scoring, and reconciliation transitions
- deterministic `TrainingWindowAssignmentPosture` carrying assignment seed,
  policy revision id, and weight-broadcast digest
- lightweight rollout and sampled-eval assignments that exchange only ids,
  digests, policy ids, and weight-broadcast refs
- lightweight `RolloutArtifactRef` and `TrainerBatchAssemblyRequest` contracts
  so trainer-batch assembly stays control-plane-safe while still composing with
  full `RolloutArtifact` and `TrainerBatch` substrate
- replay-safe tests proving admitted participants, contributing participants,
  and resulting trainer batches can all differ in one orchestrated window

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_ORCHESTRATOR_REFERENCE.md`
- `scripts/release/check-psionic-train-orchestrator.sh`

This issue makes the orchestrator a first-class Psionic control plane instead
of a loose pile of helpers around the run graph. It does not yet land
off-policy pruning or worker protocol completion.

### 11. `Psionic Train: implement off-policy budget rules and stale-rollout pruning`

Status: implemented on 2026-03-14 via GitHub issue `#3574`.

Added bounded rollout-admission contracts inside `psionic-train` for:

- explicit `TrainingOffPolicyBudget` policy over revision drift, policy age,
  rollout age, and quarantine thresholds
- typed `RolloutAdmissionReceipt` outcomes for accepted exact, accepted
  off-policy, quarantined, and discarded rollouts
- machine-readable `RolloutAdmissionSignal` reason codes so freshness and drift
  violations stay inspectable rather than log-only
- per-window `RolloutIngestionTelemetry` and retained quarantined-versus-
  discarded rollout state on the orchestrator
- replay-safe tests proving exact acceptance, bounded off-policy acceptance,
  quarantine outside direct-accept budgets, and hard discard beyond quarantine
  budgets

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_OFF_POLICY_BUDGET_REFERENCE.md`
- `scripts/release/check-psionic-train-off-policy-budget.sh`

This issue makes stale-rollout accounting first-class train control-plane truth
instead of a batch-filtering convention. Worker claim protocol completion and
validator-owned rollout adjudication now live in the follow-on records for
issues `#3575` and `#3576`.

### 12. `Psionic Train: define the inference-worker protocol for trustless rollout generation`

Status: implemented on 2026-03-14 via GitHub issue `#3575`.

Added rollout-worker protocol contracts inside `psionic-train` for:

- explicit `RolloutWorkerTrustClass` and `RolloutWorkerIdentity` so trusted
  trainer nodes are protocol-distinct from semi-trusted or untrusted rollout
  workers
- `RolloutWorkerHeartbeatReceipt` and `RolloutTaskClaim` over heartbeat
  freshness, claim TTL, deterministic sample-selection seed, and assignment
  binding
- `RolloutUploadLocator` and upload-policy enforcement for inline versus
  external artifact delivery
- `RolloutWorkerOutcomeReceipt` that wraps local claim-expiry or upload-policy
  outcomes plus orchestrator-provided rollout-admission receipts
- replay-safe tests proving fresh-heartbeat claims, bounded off-policy upload
  handling, and local receipts for expired claims or oversized uploads

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_ROLLOUT_WORKER_PROTOCOL_REFERENCE.md`
- `scripts/release/check-psionic-train-rollout-worker-protocol.sh`

This issue makes rollout-worker coordination a first-class typed protocol
inside Psionic instead of a trainer-local convention. Validator-owned rollout
verification and sampled adjudication now live in the follow-on record for
issue `#3576`.

### 13. `Validator Service: add rollout-verification bundles and sampled adjudication protocols`

Status: implemented on 2026-03-14 via GitHub issue `#3576`.

Added rollout-validation contracts inside `psionic-train` for:

- `RolloutVerificationBundle` over one rollout artifact, worker outcome, and
  optional benchmark observation or expectation
- `RolloutValidatorPolicy` with execution-proof requirements, deterministic
  sampled expensive-check posture, benchmark-check posture, and duplicate
  normalization policy
- `ValidatorVerdict` with typed replay-detected, duplicate-detected,
  stale-policy-rejected, contribution-normalized, timer-integrity,
  token-accounting, final-state, and execution-strategy reason codes
- stateful replay and duplicate detection through artifact-digest and
  response-signature history
- benchmark-gated sampled adjudication for timer, token, final-state, and
  declared-execution-strategy checks

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_ROLLOUT_VALIDATION_REFERENCE.md`
- `scripts/release/check-psionic-train-rollout-validation.sh`

This issue makes validator-ready rollout integrity first-class typed Psionic
truth. Broader external validator services, batch-level verdicts, and authority
integration are still later layers.

### 14. `Environments: define a package contract for SFT, RL, and eval`

Status: implemented on 2026-03-14 via GitHub issue `#3577`.

Added package-shape contracts inside `psionic-environments` for:

- `EnvironmentWorkloadClass` so one package can declare SFT, RL, online-eval,
  offline-eval, and validator-benchmark use explicitly
- typed `EnvironmentPolicyReference` and `EnvironmentDifficultyMetadata`
  instead of burying those semantics in free-form metadata
- `EnvironmentBenchmarkProfile` for validator-owned benchmark identity,
  runtime-profile identity, verification posture, and declared
  execution-strategy expectations
- package validation and digest coverage for workload classes, policy refs,
  difficulty metadata, and benchmark profiles
- replay-safe tests proving one package can carry both ordinary environment
  execution contracts and a reusable benchmark profile

The canonical runbook and harness are now:

- `crates/psionic/docs/ENVIRONMENT_PACKAGE_CONTRACT_REFERENCE.md`
- `scripts/release/check-psionic-environment-package-contract.sh`

This issue makes environment packages composable across training, eval, and
validator-local simulation instead of relying on raw metadata blobs or hidden
side settings. Registry install and composition flows remain the next issue.

### 15. `Environments Registry: add package install, version pinning, composition, and eval parity`

Status: implemented on 2026-03-14 via GitHub issue `#3578`.

Added the first Psionic-native registry and composition layer inside
`psionic-environments`:

- typed install requests and install receipts for versioned environment package
  materialization
- digest-bound pin aliases so train and eval code resolve immutable package
  versions instead of floating refs
- mixed-surface composition groups and group-member contracts across train,
  eval, and benchmark surfaces
- dependency-aware group resolution and benchmark-profile validation
- explicit train/eval parity receipts for shared group members

The canonical runbook and harness are now:

- `crates/psionic/docs/ENVIRONMENT_REGISTRY_REFERENCE.md`
- `scripts/release/check-psionic-environment-registry.sh`

This issue removes the need for bespoke environment-mix glue in the
orchestrator for the first train/eval/benchmark package groups. Persistent
authority sync, package publication, and richer eval-policy productization
remain later layers.

On 2026-03-15, GitHub issue `#3622` added the first repo-owned Apple adapter
specialization on top of that registry substrate: one helper now materializes
the shared Apple core package, benchmark package, mixed-surface group, and the
train/eval parity receipt together so later train and eval layers do not have
to rebuild Apple environment wiring from app-local config.

### 16. `Psionic Sandbox: add RL-throughput primitives for pooled, repeated agentic execution`

Status: implemented on 2026-03-14 via GitHub issue `#3579`.

Added the first RL-throughput sandbox control plane inside `psionic-sandbox`:

- typed warm-pool specs, snapshots, warm receipts, and acquisition receipts
- staged-input receipts for command inputs, image frames, and context artifacts
- repeated bounded loop execution on the same acquired workspace
- explicit reuse accounting so pool health and acquisition latency are visible
  to later train or operator layers

The canonical runbook and harness are now:

- `crates/psionic/docs/SANDBOX_RL_THROUGHPUT_REFERENCE.md`
- `scripts/release/check-psionic-sandbox-rl-throughput.sh`

This issue makes the sandbox layer usable for RL-style short-lived environment
actions without forcing one bespoke background-job flow per environment.
Distributed pool management and higher-level train scheduling still remain
later layers.

### 17. `Psionic Train: add SFT trace ingestion, stage transitions, and agentic pre-RL flows`

Status: implemented on 2026-03-14 via GitHub issue `#3580`.

Added the first multi-stage train-program layer inside `psionic-train`:

- typed `TrainingStageKind` identity for `general_sft`, `agentic_sft`, and `rl`
- typed SFT trace artifacts with tool-call and long-context lineage
- stage completion receipts, checkpoint-promotion receipts, and stage-transition
  receipts
- a stage-program state machine that owns `general_sft -> agentic_sft -> rl`
  sequencing

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_STAGE_PROGRAM_REFERENCE.md`
- `scripts/release/check-psionic-train-stage-program.sh`

This issue makes stage sequencing first-class Psionic truth instead of operator
glue. Curriculum, filtering, and instability policy remain the next train
issues.

### 18. `Psionic Train: implement curriculum, filtering, and non-zero-advantage gates`

Status: implemented on 2026-03-14 via GitHub issue `#3581`.

Added the first train-side curriculum controller inside `psionic-train`:

- digest-bound curriculum policy with online and offline sampling filters
- typed training candidates constructed from SFT traces and rollout artifacts
- explicit filter receipts and batch selection receipts
- difficulty-tier consumption, trivial-reward suppression, source-budget
  suppression, and non-zero-advantage gates

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_CURRICULUM_REFERENCE.md`
- `scripts/release/check-psionic-train-curriculum.sh`

This issue makes training-sample selection inspectable and reproducible.
Instability telemetry and halt policy remain the next train issue.

### 19. `Psionic Train: add instability telemetry, halt policies, and risky-optimization gating`

Status: implemented on 2026-03-14 via GitHub issue `#3582`.

Added the first train-safety controller inside `psionic-train`:

- aggregated instability telemetry over gradient norms, clipping ratios, and
  rollout-drop rate, with explicit extension points for entropy drift,
  checkpoint catch-up latency, topology churn, and failure rates
- digest-bound threshold rules that map signals to `continue`, `quarantine`, or
  `halt`
- explicit risky-optimization rules so dangerous runtime shortcuts are policy,
  not hidden flags
- final typed verdicts carrying both signal receipts and optimization receipts

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_STABILITY_REFERENCE.md`
- `scripts/release/check-psionic-train-stability.sh`

This issue makes halt/quarantine policy machine-legible. Broader operator
product surfaces beyond the current app-owned Apple workflow remain later
layers.

### 20. `Kernel and Nexus: add training and eval receipt families, policy registries, and read models`

Once train and eval become economic or productized objects, their outcomes
need authority-facing truth. This issue should add durable receipt families,
read models, and policy registries for environment packages, checkpoint
families, validator posture, and accepted train or eval outcomes. It is the
bridge from Psionic-local execution truth into higher-level OpenAgents market
or authority truth. It should also prefer typed Rust client and payload-builder
surfaces for those train, eval, and validator-facing authority contracts rather
than ad hoc JSON glue.

Status: implemented on 2026-03-14 via GitHub issue `#3583`.

The canonical authority docs are now:

- `docs/kernel/compute-evaluation-runs.md`
- `docs/kernel/compute-training-authority.md`

The generated or typed authority path now exists in `openagents-kernel-core`
and `apps/nexus-control` for:

- checkpoint-family policy registry
- validator-policy registry
- benchmark-package registry
- training-policy registry
- training-run create/finalize/list/get
- accepted eval or training outcomes

On 2026-03-15, GitHub issue `#3624` extended the same authority surface with
Apple-specific benchmark adapter kinds plus typed training-policy and
training-run metadata validation, so Apple packages and runs now have to carry
consistent benchmark, validator, and environment bindings before Nexus accepts
them.

On 2026-03-15, GitHub issue `#3627` then extended Nexus with the first
canonical Apple training-run and accepted-outcome projection path, including
held-out eval gating, optional runtime-smoke gating, and persistence of typed
Apple package lineage on both the finalized training run and accepted outcome.

### 21. `Desktop and autopilotctl: expose training operator surfaces and diagnostics`

Implemented on Sunday, March 15, 2026 via GitHub issue `#3629`, after the
earlier Apple adapter inventory/control plumbing from issue `#3620`.

The app-owned desktop-control surface and `autopilotctl` now expose a typed
training operator view. The current projection is intentionally truthful about
what is authority-backed versus what is not yet wired from a live train
controller:

- authority-backed training runs and accepted outcomes are loaded into the
  desktop-control compute-history cache alongside proof and challenge truth
- the snapshot now exposes a dedicated `training` domain with explicit
  `control_plane_state` versus `artifact_plane_state`
- operator output includes environment versions, checkpoint refs,
  contributor-set revision hints, contributor reselection timing, stale-rollout
  discard counts, duplicate quarantine or deweight counts, validator verdict
  totals, sandbox pool readiness, and visible run-level diagnostics
- the same surface now carries an app-owned Apple operator sub-view with
  explicit `launch`, `evaluation`, `export`, and `acceptance` stage state plus
  persisted run logs and authority refs
- `autopilotctl training launch`, `training export`, and `training accept`
  drive the same desktop-control mutations instead of relying on ad hoc scripts
- `autopilotctl training status` prints the same app-owned projection directly,
  while `autopilotctl status` includes a condensed training summary

This does not claim a live Psionic train orchestrator is embedded in the
desktop app yet. It does make the currently available training truth
inspectable without reconstructing it from logs or ad hoc scripts.

On 2026-03-15, GitHub issue `#3628` projected the same accepted Apple adapter
truth into `openagents-provider-substrate` as a narrow provider-hosted
adapter-family capability, and GitHub issue `#3630` updated the compute-market
docs to match that narrow truth boundary. Those surfaces sit above Psionic and
do not yet imply a broad training procurement product.

### 22. `Reference Program: run one end-to-end agentic SFT plus RL pilot on the full stack`

Implemented on Saturday, March 14, 2026.

`psionic-train` now ships a typed reference-program runner in
`src/reference_program.rs` plus the runnable harness
`scripts/release/check-psionic-agentic-sft-rl-reference-program.sh`.

The pilot intentionally crosses the currently implemented Rust-owned stack
instead of claiming completion from isolated subsystem tests:

- one versioned weather-agent environment package is reused across SFT, RL,
  online eval, and benchmark-mode eval
- dataset lineage remains explicit through environment bindings, trace source
  refs, and eval contracts
- stage-program lineage crosses `general_sft -> agentic_sft -> rl` with
  explicit checkpoint-promotion receipts
- policy weights are delivered through `psionic-datastream` broadcast receipts
- sandbox warm-pool reuse is proven through staged-input and iteration receipts
- rollout-worker heartbeat, claim, upload, and outcome receipts run against the
  real train orchestrator state
- validator-aware adjudication emits typed verdicts over rollout bundles
- benchmark aggregation and online eval both remain machine-legible
- the trainer step consumes the orchestrator-produced trainer batch rather than
  a disconnected toy batch
- the final report includes a condensed operator view without discarding the
  underlying typed receipts, lineage, and summaries

This is the current main integration gate for the early train stack. It does
not claim that replay guarantees, security hardening, artifact lifecycle, or
research-loop layers are complete, and it does not turn the landed
distributed-optimizer or model-IO contracts into proof that the full
multi-device runtime is complete.

### Production Completion And Hardening

### 23. `Psionic Train: define distributed optimizer, precision, and memory-sharding contracts`

Implemented on Saturday, March 14, 2026.

`psionic-train` now owns an explicit distributed-optimizer layer in
`src/distributed_optimizer.rs` on top of the existing fixed-budget core.

The new contract makes all of the following first-class:

- distributed optimizer family selection
- parameter sharding per group
- gradient-buffer sharding per group
- optimizer-state sharding plus residency
- master-weight residency
- precision policy across parameter, gradient, optimizer-state, master-weight,
  and reduction paths
- activation checkpointing or rematerialization policy
- long-run host/device memory budgeting and derived memory-plan receipts
- microbatch accumulation and flush discipline
- collective sync-plan attachment to the optimizer contract itself

The runtime wrapper is still intentionally bounded. It buffers microbatches,
refuses incomplete flushes, derives an explicit memory plan, and then flushes
one accumulated step through the existing fixed-budget trainer core while
preserving the higher-level distributed receipt.

This does not claim that the full multi-device runtime already exists. It does
mean the distributed optimizer, precision, and memory-sharding model is now
typed and inspectable instead of implied by future plans.

The distributed layer now composes with the reusable optimizer surface in
`src/optimizer.rs`, so local optimizer-family step semantics are inspectable
without being trapped inside one trainer implementation.

### 24. `Model IO: add Rust-native checkpoint, tokenizer, and model-format interoperability`

Implemented on Saturday, March 14, 2026.

`psionic-train` now owns a typed model-IO portability layer in
`src/model_io.rs`.

The new layer makes these train-to-serve seams explicit:

- named state-dict traversal and assignment contracts
- portable training-group reconstruction from state-dict artifacts
- tokenizer family, digest, special-token, and version binding
- dense safetensors export and import with embedded Psionic manifest metadata
- JSON torch-style state-dict artifacts for Rust-native portability
- GGUF import with tensor inventory, tokenizer binding, and chat-template
  digest extraction
- additive adapter merge and unmerge over parameter tensors

The scope is still intentionally bounded. The current torch-compatible surface
is typed JSON rather than opaque Python checkpoint loading, and GGUF support is
currently import-focused rather than full re-export. That is still a material
shift: trained or served artifacts are now portable through one Rust-owned
contract instead of bespoke scripts or disconnected side files.

### 25. `Training Truth: add deterministic replay and reproducibility guarantees`

Implemented on Saturday, March 14, 2026.

`psionic-train` now owns a deterministic replay-truth layer in
`src/replay_truth.rs`.

The new contract makes these reproducibility seams explicit:

- assignment, trainer, and eval seed discipline
- deterministic sample-selection rules with stable worker and attempt identity
- replayable trainer-batch anchoring
- pinned environment package and tool contracts
- pinned tool-version labels
- reproducible eval posture with deterministic scheduler enforcement
- typed replay-verification receipts and drift signals

This is still not the claim that the full train system can be re-executed from
one receipt without more runtime work. It is the claim that replay-compatible
inputs, pins, and verification are now explicit enough to support "same
receipt, same recomputation rules" instead of best-effort repeatability.

### 26. `Security: harden environment packages, artifact provenance, and untrusted worker admission`

Implemented on Saturday, March 14, 2026.

`psionic-train` now owns a train-security posture layer in
`src/security_posture.rs`.

The new contract makes these hardening seams explicit:

- environment package identity and digest verification
- required environment verification and safety policy references
- artifact signing contracts plus trust roots
- minimum signature counts for admitted artifacts
- untrusted-worker rate limits and burst controls
- required execution-proof posture for untrusted workers
- duplicate-artifact rejection and duplicate-response-signature quarantine
- validator-bound security receipts with typed reason codes

This does not replace the validator loop. It does connect rollout validation to
the broader train security posture instead of leaving environment trust,
artifact provenance, and untrusted-worker admission as implicit assumptions.

### 27. `Artifact Storage: define retention, garbage collection, archival, and cold-restore policy`

Status: implemented on 2026-03-14 via GitHub issue `#3590`.

`psionic-train` now owns an explicit artifact-storage lifecycle layer in
`src/artifact_storage.rs`.

The new contract makes these storage seams explicit:

- per-artifact-class retention profiles with hot and warm thresholds
- archive classes for ephemeral, restorable, and immutable artifacts
- digest-aware deduplication for rollout or other repeatable artifact classes
- typed records for checkpoint, rollout, eval, and log bundle artifacts
- explicit sweep receipts for warm migration, archival, deduplication, and
  garbage collection
- cold-restore request and completion receipts bound to restore objectives

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_ARTIFACT_STORAGE_REFERENCE.md`
- `scripts/release/check-psionic-train-artifact-storage.sh`

This issue makes train artifact retention part of typed Psionic truth instead
of operator-local scripts. Scheduler budgeting, queue preemption, and broader
economic accounting remain the next layer.

### 28. `Scheduling and Accounting: add budget, priority, preemption, and cost attribution`

Status: implemented on 2026-03-14 via GitHub issue `#3591`.

`psionic-train` now owns an explicit scheduling and accounting layer in
`src/scheduling_accounting.rs`, and `psionic-runtime` now surfaces train-owned
runtime work classes for trainer, rollout, eval, sandbox, and validator work.

The new contract makes these operator seams explicit:

- global active-work budget caps over work units, bytes, and estimated cost
- queue classes with inspectable priority and preemption policy
- role-specific cost rates for trainer, rollout, eval, sandbox, and validator
  work
- typed admission, preemption, queueing, completion, and snapshot receipts
- validator-scoped and environment-scoped cost attribution
- queue draining after completion so queued work becomes active through typed
  state transitions rather than implicit retries

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_SCHEDULING_ACCOUNTING_REFERENCE.md`
- `scripts/release/check-psionic-train-scheduling-accounting.sh`

This issue makes train-side operator economics first-class typed Psionic truth.
Chaos testing and benchmark thresholds remain the final follow-on issues in the
train program.

### 29. `Reliability: add chaos and failure-injection suites for topology, checkpoint, and validator flows`

Status: implemented on 2026-03-14 via GitHub issue `#3592`.

`psionic-train` now owns an explicit reliability suite in
`src/reliability.rs` that runs typed chaos scenarios over existing checkpoint,
collective, orchestrator, and validator contracts.

The new contract makes these reliability seams explicit:

- topology churn drills over elastic membership and checkpoint-backed recovery
- network degradation drills over collective cadence fallback
- stale-weight flood containment over rollout admission
- checkpoint corruption drills over stale-pointer fallback
- validator sampling stress over accepted, normalized, and rejected verdicts
- orchestrator restart roundtrips that resume window control after state restore

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_RELIABILITY_REFERENCE.md`
- `scripts/release/check-psionic-train-reliability.sh`

This issue makes reliability claims a machine-checkable suite instead of a
collection of unrelated unit tests. Quantitative benchmark thresholds remain
the final train-program gap.

### 30. `Benchmarking: define performance acceptance thresholds for trainer, sandbox, datastream, and validation`

Status: implemented on 2026-03-14 via GitHub issue `#3593`.

`psionic-train` now owns a typed quantitative acceptance layer in
`src/benchmarking.rs` instead of leaving train performance closure to ad hoc
notes or one-off benchmark scripts.

The new benchmark contract makes these production thresholds explicit:

- fixed-budget trainer throughput
- rollout ingestion throughput at the orchestrator boundary
- warm sandbox reuse latency and reuse ratio
- checkpoint restore latency plus resumable datastream recovery throughput
- validator verification cost and sampled benchmark-check share
- elastic scaling curves from two to four members, including degraded transport
  fallback

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAIN_BENCHMARK_ACCEPTANCE_REFERENCE.md`
- `scripts/release/check-psionic-train-benchmark-acceptance.sh`

This issue closes the last train-system gap called out at the end of the issue
program: Psionic now has both chaos-style reliability drills and one owned
acceptance profile for deciding whether the current train substrate is fast and
stable enough to claim seriously.

## Later-Scope Issues

These are valid future issues, but they are not part of the minimum path above.

### Model promotion and release governance

Later the system will also need:

- candidate promotion gates
- release thresholds
- rollback policy
- checkpoint-to-release lineage
- human signoff hooks

### Human preference, critique, and label-ingestion pipelines

If OpenAgents expands into broader RLHF-style or critique-driven post-training,
the system will also need:

- critique and preference record schemas
- provenance and adjudication for noisy labels
- human-score and rubric blending
- reviewer-tooling integration

## Bottom Line

The current Psionic tree already contains real train-substrate work:

- runtime training truth
- datastream movement for train-relevant artifacts
- collective planning
- checkpoint and recovery session state
- early training-output lineage through adapters
- reusable environment, eval, validator, and orchestrator crates
- one real repo-owned Apple training lane with app-owned operator and accepted-
  outcome integration around it

That means the train system is no longer hypothetical.

But the current tree still stops short of a scaled, generalized, fully hardened
all-Rust training system.

The missing center of gravity is now:

- multi-device execution kernels and distributed optimizer execution at scale
- broader format interoperability and retention/promotion governance
- stronger provenance, security, and authority integration beyond the current
  Apple path
- broader operator and product surfaces beyond the current Apple reference
  workflow

That is the path Psionic still has to build from its now-real early train
system.
