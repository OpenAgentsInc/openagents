# Psionic Train System

> Status: updated 2026-03-14 after reviewing `docs/MVP.md`,
> `docs/OWNERSHIP.md`,
> `docs/audits/2026-03-13-intellect-lessons-for-psionic-train-audit.md`,
> `docs/audits/2026-03-14-covenant-code-lessons-for-psionic-train-audit.md`,
> `crates/psionic/README.md`,
> `crates/psionic/docs/ARCHITECTURE.md`,
> `crates/psionic/docs/AUTORESEARCH_INTEGRATION_PLAN.md`,
> `crates/psionic/psionic-runtime/src/lib.rs`,
> `crates/psionic/psionic-datastream/src/lib.rs`,
> `crates/psionic/psionic-collectives/src/lib.rs`,
> `crates/psionic/psionic-train/src/lib.rs`,
> `crates/psionic/psionic-adapters/src/lib.rs`, and
> `crates/psionic/psionic-sandbox/src/lib.rs`, plus the current open and
> recently closed issue backlog through `#3609`.

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

## Doc Authority

- `crates/psionic/docs/TRAIN_SYSTEM.md` is the canonical training subsystem
  spec.
- `crates/psionic/docs/ARCHITECTURE.md` is the canonical Psionic-wide system
  spec that defines the lower execution substrate this doc builds on.
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

- It is not a promise that full model training already works in the repo today.
- It is not a Python trainer hidden behind Rust wrappers.
- It is not an app-owned workflow inside `apps/*`.
- It is not just "cluster execution, but for training."
- It is not just checkpoint files and background notes.

The honest description today is:

> Psionic already owns real training-class truth surfaces plus a bounded
> training-core reference loop, but it does not yet own the full distributed
> train system.

## Canonical Train Objects

The full train system needs a formal object model. Today only some of these
objects have concrete repo types; the rest are planned and should become the
stable vocabulary for train-class execution.

| Object | Purpose | Current Repo Status |
| --- | --- | --- |
| `TrainingRun` | Root identity for one training program | `planned` |
| `TrainingStage` | One named phase such as SFT, agentic SFT, or RL | `planned` |
| `TrainingWindow` | One synchronized contribution or trainer interval with its own contributor set and transition state | `planned` |
| `TrainerStep` | One optimizer update over one trainer batch | `implemented_early` |
| `PolicyRevision` | Versioned policy or weight state used by workers and trainer | `implemented_early` |
| `RolloutArtifact` | One worker-produced trajectory or completion bundle | `implemented_early` |
| `TrainerBatch` | One accepted batch of rollout or corpus inputs for a trainer step | `implemented_early` |
| `EnvironmentPackage` | One versioned environment definition used by training and eval | `implemented_early` |
| `BenchmarkPackage` | One validator-owned packaged benchmark or reference evaluation profile | `planned` |
| `EvalRun` | One online or offline evaluation execution | `planned` |
| `CheckpointPointer` | One stable pointer to the latest accepted checkpoint for a run, stage, or window | `planned` |
| `CheckpointManifest` | One shard, digest, writer, and durability manifest for a checkpoint flush | `planned` |
| `Checkpoint` | Recoverable training state and lineage anchor | `partial` |
| `ValidatorVerdict` | Verification result attached to one rollout, batch, or eval artifact | `planned` |

Today the concrete object vocabulary is strongest around:

- `TrainingCheckpointReference`
- `TrainingRecoveryContext`
- `TrainingDeviceMeshContext`
- `TrainingCollectiveContext`
- `DatastreamManifest` and `DatastreamManifestRef`

Current checkpoint substrate is carried today by
`TrainingCheckpointReference` plus checkpoint-scoped datastream manifests.

The rest of the train object model still needs to be built explicitly.

What is still missing most clearly from the current vocabulary is:

- explicit `TrainingWindow` identity rather than inferring windows from
  scheduler cadence
- explicit `CheckpointPointer` and `CheckpointManifest` objects rather than
  treating latest-checkpoint and shard metadata as loose convention
- explicit `BenchmarkPackage` truth for validator-owned, repeatable benchmark
  evaluation rather than folding that concept into generic eval notes

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
| Datastream | `implemented_early` | resumable manifests, checkpoint bindings, dataset bindings, delivery receipts |
| Collectives | `implemented_early` | elastic mesh observation, benchmark-gated quantized collective planning |
| Train session state | `implemented_early` | membership observation, async checkpoint state, durability transitions, live-recovery planning |
| Data contracts | `implemented_early` | `psionic-data` now owns versioned dataset manifests, tokenizer digests, split declarations, resumable iteration cursors, and long-context packing policies |
| Adapters | `implemented_early` | adapter identity, package manifests, hosted adapter binding lineage |
| Sandbox for RL/train workloads | `partial` | bounded execution and background jobs exist, but not RL-throughput pooling or environment-native loops |
| Training core | `implemented_early` | `psionic-train` now has a typed fixed-budget trainer-step loop with explicit parameter groups, optimizer state/residency, step telemetry, and checkpoint restore lineage over explicit gradient batches |
| Orchestrator | `planned` | no first-class rollout scheduler, batch assembler, or policy propagation engine |
| Environment ABI | `implemented_early` | `psionic-environments` now owns the package ABI, versioned key, tool/rubric contracts, and deterministic runtime session state machine, while registry and authority truth remain in kernel/Nexus |
| Eval runtime | `implemented_early` | `psionic-eval` now owns held-out eval runs, rubric-scored sample/runtime contracts, benchmark packages, repeat-run aggregation, and local validator simulation, while kernel/Nexus still own canonical eval-run authority truth |
| Synthetic-data flows | `partial_outside_psionic` | synthetic-data job creation, append, finalize, and verification flows exist in kernel/Nexus, but no Psionic-native generation runtime exists yet |
| Rollout artifacts | `implemented_early` | `psionic-train` now has checkpoint-aware policy revisions, proof-bearing rollout artifacts, and deterministic trainer-batch assembly with policy-lineage digests |
| Validator-aware RL verification | `planned` | no rollout verification bundle family or sampled adjudication loop |

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
    streamed iteration contracts, and long-context packing rules
- `psionic-collectives`
  - elastic mesh observation and benchmark-gated collective planning
- `psionic-environments`
  - environment package ABI, execution entrypoints, tool and rubric hooks,
    artifact expectations, versioned dataset bindings, and deterministic
    runtime sessions
- `psionic-eval`
  - held-out eval runs, rubric-scored sample/runtime contracts, benchmark
    packages, repeat-run aggregation, and operator-local validator simulation
- `psionic-train`
  - training-session truth for checkpointing, live recovery,
    elastic-membership posture, the fixed-budget training-core reference loop,
    and RL-facing rollout or batch contracts
- `psionic-adapters`
  - adapter package identity and hosted binding lineage
- `psionic-sandbox`
  - bounded sandbox execution substrate and background-job lifecycle
- `psionic-cluster`
  - durable ordered-state, cluster admission, catch-up, and topology truth

The broader OpenAgents tree now also has train-adjacent authority surfaces
outside Psionic for:

- environment package descriptors and registry behavior
- compute evaluation-run lifecycle
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
- `ServedArtifact`
- `AdapterPackage`

Its manifests already support:

- payload digesting
- stable chunk descriptors
- dataset bindings
- checkpoint bindings
- resumable transfer cursors
- restart-safe client progress
- final delivery receipts

That means the train system already has a real substrate for:

- dataset shard transport
- checkpoint transport
- later policy-weight broadcast
- eval-bundle movement
- adapter-package distribution

What is still missing is not "a data plane exists or not." The missing work is
the training-specific policy over that data plane: freshness windows,
policy-weight revision cutoffs, staged broadcasts, and artifact retention
policy.

### 3. Collective planning already exists

`psionic-collectives` already implements a real, inspectable collective
planning substrate.

The important current pieces are:

- `ElasticCollectivePlanner`
- `CollectiveMeshMember`
- `QuantizedCollectiveBenchmark`
- `observe_mesh`
- `record_benchmark`
- `plan_collective`

The current planner already does several important things honestly:

- validates that declared mesh axes match member count
- ensures mesh members are actually active in the current membership set
- increments mesh revision only when mesh truth changes
- requires explicit benchmark approval before planning a quantized collective
- emits a `CollectiveExecutionPlan` with:
  - runtime-visible collective posture
  - explicit ring handoffs
  - a low-level `RuntimeWorkItem`

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

### 6. Sandbox substrate exists, but not the RL-oriented shape yet

`psionic-sandbox` already owns:

- runtime detection
- profile realization
- bounded job execution
- background jobs
- file transfer
- execution receipts

This is enough to support bounded compiled runners.

It is not yet the mature RL/post-training sandbox shape described by the
Intellect papers. It still lacks:

- pooled warm sandboxes
- fast repeated multi-turn execution loops
- push-based readiness
- environment-bound lifecycle contracts
- explicit RL-throughput surfaces

### 7. Environment, eval, and synthetic-data authority surfaces now exist outside Psionic

The recent issue closures matter because they changed the broader system around
Psionic even though they did not create new Psionic runtime crates.

The tree now has broader OpenAgents support for:

- environment package descriptors and registry behavior
- environment refs bound into compute products and delivery proofs
- evaluation-run creation, sample ingestion, and finalize flows
- synthetic-data job creation, append, finalize, and verification flows

Those capabilities currently live in kernel/proto and Nexus-control surfaces.

So the accurate reading is:

- the larger platform now has a home for environment, eval, and synthetic-data
  truth
- Psionic still does not have the native runtime crates that would execute
  those workflows inside the compute substrate itself

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
- training-related artifact lineage is beginning to exist as first-class data
  rather than opaque side files
- the broader OpenAgents stack now has authority-layer environment, eval, and
  synthetic-data flows that Psionic can eventually target as execution clients

That is a meaningful base.

## What Psionic Cannot Honestly Claim Yet

Psionic cannot honestly claim any of the following yet:

- full Rust-native model training
- full Rust-native RL or post-training
- trainer-step execution
- optimizer state ownership
- backward/autodiff training graph substrate
- rollout artifact recording
- off-policy accounting
- environment package execution
- shared training/eval environment contract
- training-window state transitions with deterministic assignment as first-class
  train truth
- checkpoint pointer and checkpoint-manifest protocol semantics
- validator-aware rollout verification
- validator-owned benchmark packages with repeat-run scoring and local
  validator-simulation parity
- orchestrator-owned batch assembly and weight propagation
- productionized RL sandbox throughput

Those are still planned.

## The Gap, Precisely

The gap is no longer "there is no train subtree."

The gap is:

> Psionic now has the recovery, checkpoint, data-plane, and collective
> substrate for training-class execution, but it still lacks the actual
> Rust-native trainer, orchestrator, environment, eval, rollout, and validator
> layers that would turn that substrate into a full train system.

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

What is still missing is the train-specific receipt family. The mature train
system should emit at least these receipts.

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

- collective quantization approval and benchmark posture
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

An honest all-Rust Psionic train system requires all of these to exist inside
the Rust subtree:

- training core
- optimizer ownership
- rollout artifacts
- environment ABI
- data and corpus contracts
- eval runtime
- compiled runner and crash boundary

The current repo only has the lower-level substrate for that system.

Psionic cannot honestly claim an all-Rust train system until trainer steps,
optimizer ownership, rollout artifacts, environment execution, eval runtime,
and crash-safe runner boundaries all exist inside the Rust subtree.

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
| Weight broadcast | datastream substrate exists | staged policy-weight broadcast with freshness cutoffs and relay policy |
| Training steps | typed fixed-budget reference loop present | broader Rust-native trainer-step engine |
| RL rollouts | typed rollout and trainer-batch contracts present | freshness enforcement, worker protocols, and validator-ready lineage |
| Environment ABI | typed runtime ABI present | broader package loading, composition, and environment system |
| Eval runtime | absent | shared online/offline eval and rubric runtime |
| Sandbox throughput | bounded one-shot substrate exists | RL-throughput warm pools and repeated environment loops |
| Validators for RL | absent | rollout-verification bundles and sampled adjudication |
| Operator surfaces | absent in Psionic-local train form | inspection, diagnostics, and receipts across all train subsystems |

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

### Wave 1: core all-Rust train platform

Needed next:

- actual training core
- rollout artifacts
- environment ABI
- data layer
- eval runtime
- run graph and participant lifecycle
- richer checkpoint and datastream policy
- orchestrator state machine
- worker protocol and validator flows
- RL-throughput sandbox

### Wave 2: full productization of train execution

Needed after core:

- distributed optimizer and memory-sharding discipline
- interoperability with model and tokenizer formats
- reproducibility guarantees
- security and provenance hardening
- artifact retention and cold restore policy
- scheduling, priority, and cost attribution
- chaos and failure testing
- benchmark acceptance thresholds

### Later scope

After the above:

- model promotion and release governance
- human preference and critique ingestion

Those later items matter, but they are not prerequisites for the core
environment-first Intellect-style train stack.

## Proposed GitHub Issue Program

The issue program below is written from the current repository state, not from
the older "there is no `psionic-train` crate" assumption.

This program is now instantiated on GitHub as issues `#3564` through `#3593`.

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

The canonical runbook and harness are now:

- `crates/psionic/docs/TRAINING_CORE_FIXED_BUDGET_REFERENCE.md`
- `scripts/release/check-psionic-training-core.sh`

The current step path is intentionally an explicit-gradient reference loop over
`f32` tensor payloads. That is enough to make trainer-step truth real without
pretending the later tensor, autodiff, distributed-optimizer, and orchestrator
issues are already solved.

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

### 6. `Psionic Train: define canonical run graph, topology revisions, and participant lifecycle`

The current session substrate knows about membership and checkpoints, but it
does not yet model one full training run. This issue should define stable run
ids, stage ids, `TrainingWindow` ids, participant roles, topology revisions,
and lifecycle events for join, leave, crash, timeout, eviction, and rejoin. It
should also make heartbeat, departure, window-transition, and deterministic
assignment semantics first-class train truth.

### 7. `Psionic Train: extend checkpoint lineage, recovery modes, and catch-up receipts`

`psionic-train` already has `begin_async_checkpoint`, `mark_checkpoint_durable`,
and `plan_live_recovery`. This issue should build on that foundation by adding
explicit recovery modes such as blocking catch-up, overlapped catch-up, and
resume-from-last-stable-checkpoint, then record those decisions in durable run
receipts. It should also add explicit `CheckpointPointer` and
`CheckpointManifest` semantics, preferred-read hierarchy, listing fallback,
deterministic shard-uploader assignment, and fake object-store tests for
partial-upload and missing-pointer cases. The goal is to turn recovery policy
from an internal heuristic into a queryable contract.

### 8. `Psionic Collectives: add bandwidth-aware elastic sync planning and quantized policy surfaces`

`psionic-collectives` already observes meshes and plans benchmark-approved
quantized collectives. This issue should extend it with explicit local-versus-
global sync planning, transport-observation feedback, replan triggers, and
receipt-bearing sync cadence policy. It should become the systems truth for how
elastic training synchronization actually happens.

### 9. `Psionic Datastream: add sharded policy-weight broadcast and freshness control`

Datastream already covers checkpoints and datasets, but asynchronous training
and RL require policy-weight distribution as a first-class data-plane concern.
This issue should add shard manifests, pipelined weight delivery,
assembled-artifact digests, freshness windows, stale-artifact rejection, and
mirror or relay metadata where appropriate. This is the bridge from generic
artifact movement to training control-plane correctness.

### 10. `Psionic Train: build the orchestrator state machine and trainer-batch assembly contracts`

The orchestrator is still absent. This issue should add a first-class control
plane that owns rollout scheduling, worker assignments, batch assembly, policy
revision tracking, and online eval interleaving. The result should be
inspectable orchestrator state, not a loose pile of helper functions.

### 11. `Psionic Train: implement off-policy budget rules and stale-rollout pruning`

Asynchronous RL only works when stale work is bounded and rejected honestly.
This issue should define maximum policy age or revision drift, stale-rollout
quarantine rules, discard receipts, and visibility into accepted versus dropped
samples. It should sit directly on top of the rollout artifact model and the
orchestrator state machine.

### 12. `Psionic Train: define the inference-worker protocol for trustless rollout generation`

Psionic needs a protocol for workers that generate rollouts under a declared
policy revision. This issue should define heartbeats, task claims,
sample-selection seeds, rollout upload rules, stale-weight handling, and worker
outcome receipts. It should also explicitly distinguish trusted trainer roles
from untrusted or semi-trusted rollout workers.

### 13. `Validator Service: add rollout-verification bundles and sampled adjudication protocols`

Validator work for train-class execution should widen beyond generic execution
proofs. This issue should define rollout-verification bundles, cheap universal
checks, sampled expensive checks, stale-policy rejection, schema and sanity
validation, and validator verdict artifacts that the train system can attach to
accepted or rejected outcomes. It should also cover timer-integrity, token
accounting, final-state, and declared-strategy verification when a benchmark
package or validator policy requires them. This is the main integrity layer for
permissionless or semi-trusted RL.

### 14. `Environments: define a package contract for SFT, RL, and eval`

The environment ABI issue defines execution mechanics; this issue should define
the package contents and product shape. Packages should carry dataset refs,
tool schemas, rubric refs, sandbox requirements, difficulty metadata, and
policy references so the same environment artifact can power SFT, RL, online
eval, and offline eval. The same package family should also be able to describe
validator-owned benchmark profiles so local simulation and validator execution
can share one packaged contract. This is how environment-bound training becomes
composable.

### 15. `Environments Registry: add package install, version pinning, composition, and eval parity`

Once environment packages exist, Psionic needs registry and composition flows.
This issue should add install, resolution, pinning, dependency management,
mixed-environment groups, and strict reuse across training and eval. The
acceptance bar is that the orchestrator does not need bespoke code for each
environment mix.

### 16. `Psionic Sandbox: add RL-throughput primitives for pooled, repeated agentic execution`

`psionic-sandbox` already owns bounded execution and background jobs, but RL and
agentic post-training need a different hot path. This issue should add warm
pools, fast readiness, repeated command loops, streamed image or artifact
staging, and receipts for pool reuse and acquisition latency. The target is a
sandbox substrate optimized for thousands of short-lived environment actions,
not only one-shot remote execution.

### 17. `Psionic Train: add SFT trace ingestion, stage transitions, and agentic pre-RL flows`

The train system should explicitly model multi-stage programs rather than
assuming RL is the first and only stage. This issue should add SFT trace
ingestion, general-SFT to agentic-SFT to RL transitions, checkpoint promotion
between stages, and typed lineage for tool-call and long-context traces. The
goal is to make stage sequencing part of the train system rather than operator
glue.

### 18. `Psionic Train: implement curriculum, filtering, and non-zero-advantage gates`

The Intellect papers make curriculum and filtering a first-class control
surface. This issue should add difficulty metadata consumption, trivial-sample
suppression, non-zero-advantage gates, online and offline sampling filters, and
receipt-bearing filtering policy. It should make training-sample selection
inspectable and reproducible.

### 19. `Psionic Train: add instability telemetry, halt policies, and risky-optimization gating`

The mature train system needs operational safety signals. This issue should add
telemetry for gradient norms, clipping ratios, entropy drift, stale-rollout
drop rate, checkpoint catch-up latency, topology churn, environment failure
rates, and sandbox failure rates, then connect those signals to machine-legible
halt or quarantine policy. Risky runtime optimizations should become explicit
policy, not hidden flags.

### 20. `Kernel and Nexus: add training and eval receipt families, policy registries, and read models`

Once train and eval become economic or productized objects, their outcomes
need authority-facing truth. This issue should add durable receipt families,
read models, and policy registries for environment packages, checkpoint
families, validator posture, and accepted train or eval outcomes. It is the
bridge from Psionic-local execution truth into higher-level OpenAgents market
or authority truth. It should also prefer typed Rust client and payload-builder
surfaces for those train, eval, and validator-facing authority contracts rather
than ad hoc JSON glue.

### 21. `Desktop and autopilotctl: expose training operator surfaces and diagnostics`

A real train system must be operable. This issue should surface topology
revisions, checkpoint lineage, stale-rollout drops, validator verdicts,
environment versions, sandbox pool health, and orchestrator state in the
existing app-owned control plane and thin CLI. The result should be operator
inspection without dropping straight into logs or ad hoc scripts.

### 22. `Reference Program: run one end-to-end agentic SFT plus RL pilot on the full stack`

The train system should not be declared complete from isolated subsystem
benchmarks. This issue should run one reference program that exercises
environment packages, dataset and checkpoint lineage, rollout workers,
validator-aware adjudication, sandbox reuse, online eval, and operator
inspection together. It is the main integration gate for the core stack.

### Production Completion And Hardening

### 23. `Psionic Train: define distributed optimizer, precision, and memory-sharding contracts`

The training core issue is necessary but not specific enough about the actual
distributed optimizer family. This issue should make parameter sharding,
optimizer-state sharding, gradient accumulation, activation checkpointing or
rematerialization, long-run memory planning, and precision policy explicit.
That moves the train system from "a loop exists" to "the distributed training
model is real and inspectable."

### 24. `Model IO: add Rust-native checkpoint, tokenizer, and model-format interoperability`

A train system that cannot interoperate with serving and artifact ecosystems
will strand its outputs. This issue should add import and export contracts for
checkpoint formats, tokenizer assets, serving-compatible model formats, and
adapter merge or unmerge flows. It should make trained artifacts portable
without relying on bespoke conversion scripts.

### 25. `Training Truth: add deterministic replay and reproducibility guarantees`

Receipts and lineage are not enough unless they support re-execution or replay
where feasible. This issue should define seed discipline, replayable trainer
batches, deterministic sample-selection rules, pinned environment and tool
versions, and reproducible eval posture. The target is "same receipt, same
recomputation rules" instead of best-effort repeatability.

### 26. `Security: harden environment packages, artifact provenance, and untrusted worker admission`

The validator loop is only one part of security. This issue should define the
malicious-worker threat model, environment-package verification, artifact
signing and trust roots, rollout spam or poisoning controls, admission control,
and rate limiting for untrusted workers. It should connect rollout validation
to a broader security posture for the train system.

### 27. `Artifact Storage: define retention, garbage collection, archival, and cold-restore policy`

The full train stack will generate large volumes of checkpoints, rollouts, eval
artifacts, and logs. This issue should define retention windows, storage tiers,
archival classes, deduplication, garbage collection, and cold-restore
objectives for those artifacts. Storage lifecycle must become part of train
truth rather than an operator afterthought.

### 28. `Scheduling and Accounting: add budget, priority, preemption, and cost attribution`

Once the stack is shared or decentralized, it needs operator economics. This
issue should add budget caps, queue classes, preemption rules, validator cost
visibility, environment cost attribution, and role-aware accounting across
trainer, rollout, eval, and sandbox workloads. The goal is to make the system
operable under real capacity constraints rather than only functionally correct.

### 29. `Reliability: add chaos and failure-injection suites for topology, checkpoint, and validator flows`

The train system is explicitly elastic and partially untrusted, so it needs a
failure-testing program rather than only unit tests. This issue should add
topology churn simulation, network degradation drills, stale-weight flood
tests, checkpoint corruption drills, validator-sampling stress tests, and
orchestrator restart and recovery tests. It is the main acceptance layer for
reliability claims.

### 30. `Benchmarking: define performance acceptance thresholds for trainer, sandbox, datastream, and validation`

The final production gap is quantitative acceptance. This issue should define
trainer throughput, rollout ingestion throughput, sandbox reuse latency,
checkpoint recovery latency, validator-cost targets, and scaling curves under
elastic membership. It should answer not only "does it work?" but "is it good
enough to run seriously?"

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

That means the train system is no longer hypothetical.

But the current tree still stops short of a full all-Rust training system.

The missing center of gravity is:

- the training core
- the orchestrator
- the rollout artifact model
- the environment and eval runtime
- the validator-aware RL loop
- the production operating discipline around all of the above

That is the path Psionic still has to build.
