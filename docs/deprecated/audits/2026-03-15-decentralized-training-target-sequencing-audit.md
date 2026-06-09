# 2026-03-15 Decentralized Training Target Sequencing Audit

## Intent

This audit answers a forward-looking product and systems question:

> if the priority is decentralized training where many people around the world
> contribute compute toward a shared model, what should OpenAgents actually
> build first, and in what order, if the chosen target is decentralized
> training of adapters?

This audit now assumes that OpenAgents is adopting that plan.

This is not an MVP proposal for `Autopilot`.

It is a Psionic and system-strategy audit written against the repo's current
state.

## Relationship To Current Repo Scope

Per [docs/MVP.md](/Users/christopherdavid/code/openagents/docs/MVP.md), the
active product MVP is still the compute-provider earn loop, not decentralized
training.

Per [docs/OWNERSHIP.md](/Users/christopherdavid/code/openagents/docs/OWNERSHIP.md),
any reusable training substrate belongs in `crates/psionic/*`, not in app-owned
desktop flows.

That means the right reading of this audit is:

- it is a later-family Psionic strategy document
- it should reuse the repo's existing training and authority vocabulary
- it should not invent app-owned training architecture

## Sources Reviewed

OpenAgents sources reviewed:

- [docs/MVP.md](/Users/christopherdavid/code/openagents/docs/MVP.md)
- [docs/OWNERSHIP.md](/Users/christopherdavid/code/openagents/docs/OWNERSHIP.md)
- [crates/psionic/docs/TRAIN_SYSTEM.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/TRAIN_SYSTEM.md)
- [crates/psionic/docs/TRAIN_RUN_GRAPH_REFERENCE.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/TRAIN_RUN_GRAPH_REFERENCE.md)
- [crates/psionic/docs/TRAIN_ORCHESTRATOR_REFERENCE.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/TRAIN_ORCHESTRATOR_REFERENCE.md)
- [crates/psionic/docs/TRAIN_OFF_POLICY_BUDGET_REFERENCE.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/TRAIN_OFF_POLICY_BUDGET_REFERENCE.md)
- [crates/psionic/docs/DISTRIBUTED_OPTIMIZER_REFERENCE.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/DISTRIBUTED_OPTIMIZER_REFERENCE.md)
- [crates/psionic/docs/COLLECTIVE_SYNC_POLICY_REFERENCE.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/COLLECTIVE_SYNC_POLICY_REFERENCE.md)
- [docs/kernel/compute-training-authority.md](/Users/christopherdavid/code/openagents/docs/kernel/compute-training-authority.md)
- [2026-03-14-covenant-code-lessons-for-psionic-train-audit.md](/Users/christopherdavid/code/openagents/docs/audits/2026-03-14-covenant-code-lessons-for-psionic-train-audit.md)

## Executive Summary

OpenAgents should treat decentralized training of adapters as the chosen
later-family Psionic program, with Apple Foundation Model adapters as the first
bounded live lane.

That decision should be read precisely:

- Apple adapters are the right first decentralized training **product lane**
  because the repo already has real train/eval/export/authority closure for
  them.
- The actual implementation target is a generic decentralized **adapter
  training system**, not an Apple-only island.
- Worldwide decentralized training should begin with asynchronous bounded
  contributor windows, not with immediate world-scale synchronous collectives.

The reason is simple:

- Apple adapters are bounded, inspectable, and already partly real in this repo
- worldwide decentralized training is latency-heavy, failure-heavy, and
  heterogenous, so it should start with asynchronous windowed contribution
  rather than immediate global synchronous collectives
- Apple hardware and Apple FM runtime constraints make that lane too narrow to
  be the long-term center of gravity for a global training network

The committed sequence is therefore:

1. use Apple adapters to prove the first narrow decentralized training market
   and acceptance loop
2. make that loop windowed and asynchronous before attempting live collectives
3. generalize the same control and authority model to an open adapter lane on
   broader hardware
4. only after that add regional or homogeneous synchronous collective-backed
   training
5. only after that attempt something truly massive

If reduced to one sentence:

> OpenAgents should build decentralized adapter training by using Apple
> adapters as the first proving lane, while keeping the control plane,
> validator model, and authority model generic enough to widen to non-Apple
> adapter backends.

## Current Repo Reality

The repo now has a narrow but real Apple adapter training lane.

Per [crates/psionic/docs/TRAIN_SYSTEM.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/TRAIN_SYSTEM.md),
that means:

- repo-owned Apple adapter dataset import exists
- repo-owned Apple train/eval/benchmark environment binding exists
- `psionic-train` owns a real Apple adapter execution backend
- a Rust-native Apple adapter SFT/export path exists
- held-out eval and runtime-smoke validation exist
- accepted outcome authority and typed Apple adapter metadata discipline exist

That same spec is equally explicit that the current Apple path is still narrow:

- base weights stay frozen
- only adapter parameter groups are updated
- the current path is not yet a real `psionic-cluster` multi-node training run
- the shipped Apple backend does not yet use live collective-backed gradient
  exchange or sharded optimizer execution

At the same time, the broader Psionic train substrate already has meaningful
later-family building blocks:

- `TrainingWindow` and run-graph state
- orchestrator-owned contributor selection and assignment posture
- bounded off-policy admission
- collective sync cadence planning
- typed distributed optimizer contracts
- checkpoint and authority vocabulary

That combination matters.

It means OpenAgents does **not** need to ask "what is the first training shape
at all?"

It needs to ask:

> which training lane best exploits the current narrow Apple closure while
> preserving a path to a broader decentralized system?

## Program Decision: Apple First, Generic Adapter System

OpenAgents should now commit to Apple adapters as the first live decentralized
adapter-training lane.

That commitment is correct because the near-term goal is:

- proving that many independent contributors can participate in one bounded
  training program
- proving that the system can assign work, accept artifacts, validate results,
  and publish accepted training outcomes
- proving a first seller-side training market where contributed work is small
  enough to move and inspect

Apple is still **not** the long-term center of gravity if the goal is:

- maximizing contributor count as fast as possible
- using heterogeneous global hardware immediately
- building the definitive substrate for large synchronized model training

That distinction is now the core architectural rule for the program:

> Apple is the first execution lane, not the permanent architecture.

### Why Apple Adapters Are The Right First Lane

Apple adapters are strategically attractive because they constrain the problem.

They reduce several hard dimensions at once:

- the base model is frozen
- the trainable state is much smaller than full-model training
- exported artifacts already have a typed package format
- held-out eval and runtime-smoke checks already exist in-repo
- accepted-outcome authority for the Apple lane already exists

That makes Apple adapters a good way to prove:

- contribution assignment
- bounded artifact movement
- checkpoint lineage and acceptance
- validator-owned evaluation
- economic or operator acceptance criteria

This is exactly the kind of narrow lane that should be used to harden:

- `TrainingWindow`
- `PolicyRevision`
- `CheckpointPointer`
- `CheckpointManifest`
- `ComputeTrainingRun`
- `ComputeAcceptedOutcome`

without immediately dragging the project into full distributed optimizer and
large-model memory-sharding complexity.

### Why Apple Adapters Are Not The Whole Plan

Apple adapters are still the wrong place to anchor the whole long-term system.

The main reasons are:

- contributor hardware is narrower than the eventual worldwide network needs
- the Apple FM runtime is product-specific rather than a broad open substrate
- the current Apple lane is still single-host in execution reality
- large worldwide training systems need hardware-agnostic coordination first,
  not runtime-specific coupling first

Most importantly, "people around the world contribute compute" is almost never
a good first fit for immediate synchronous all-reduce.

A global volunteer or marketplace network implies:

- high latency
- high churn
- uneven bandwidth
- heterogeneous hardware
- mixed trust
- variable uptime

Those conditions point toward windowed asynchronous contribution first, not
tight collective training first.

Apple adapters fit that asynchronous proof better than they fit the eventual
massive-cluster end state.

## What The First Decentralized Training Loop Should Actually Look Like

The first decentralized training loop should not be:

- one world-scale optimizer step
- one fragile global all-reduce
- one giant synchronized mesh of random internet participants

It should be:

1. publish one training policy revision and one current checkpoint pointer
2. open a bounded `TrainingWindow`
3. deterministically assign dataset slices or contribution slices to admitted
   participants
4. let participants train locally against the pinned policy revision
5. collect lightweight receipts plus heavier artifact refs
6. validate submitted work through held-out eval, benchmark, and policy checks
7. aggregate accepted contributions into the next policy revision
8. publish the next window

That is much closer to the adaptation lesson already recorded in
[2026-03-14-covenant-code-lessons-for-psionic-train-audit.md](/Users/christopherdavid/code/openagents/docs/audits/2026-03-14-covenant-code-lessons-for-psionic-train-audit.md)
than to a first-pass FSDP or ZeRO rollout across random public machines.

The thing to copy first is window discipline.

The thing to delay is global synchronization.

## Committed Build Order

## Stage 1: Close The Narrow Apple Adapter Training Truth Surface

Use the current Apple lane as the first controlled proving ground and close the
remaining gap between single-host Apple training and real decentralized adapter
windows.

Priority outcomes:

- make the Apple training lane fully legible through Psionic train objects and
  receipts
- keep all authority truth in the existing training-policy, training-run, eval,
  and accepted-outcome contracts
- make the contributor-facing work unit a real `TrainingWindow`, not an ad hoc
  operator action

Concrete goals:

- define one narrow Apple adapter contribution workload family
- define the accepted artifact kinds for that family
- define exact held-out eval and runtime-smoke gates for contribution
  acceptance
- define one aggregation rule that turns accepted contributions into a promoted
  next policy revision

This stage is where Apple is strongest.

The repo already has much of the data, eval, export, and authority machinery
for it.

## Stage 2: Add Windowed Decentralized Contribution Before Any Live Collective Training

Use the existing run-graph and orchestrator substrate as the core control model.

Priority outcomes:

- participants are admitted and ranked deterministically
- each window has a fixed contributor set and assignment seed
- stale or drifted submissions are accepted, quarantined, or discarded under a
  typed off-policy budget
- heavy artifacts move by manifest or ref, not by bloated control messages

Concrete functionality:

- contributor claim and heartbeat protocol
- artifact upload and manifest publication
- deterministic shard or sample assignment
- per-window sealing, scoring, and reconciliation
- validator-owned replay or sampled verification

This is the first real decentralized training system milestone.

It matters more than adding collectives early.

## Stage 3: Make Apple Adapter Contributions Asynchronous And Aggregatable

Once windowing exists, use Apple as the first live decentralized adapter
training lane.

The correct first design is not "everyone participates in one simultaneous
gradient exchange."

It is:

- each selected participant trains locally on a bounded slice
- each participant submits an adapter delta or bounded update artifact
- the validator or trainer side evaluates and aggregates accepted deltas
- the system promotes a new adapter policy revision only after explicit
  acceptance

This gives OpenAgents the first honest answer to:

> can multiple independent machines contribute useful training work to one
> evolving model under typed policy and validator control?

If the answer is no, it is much cheaper to learn that on Apple adapters than on
large synchronized training.

## Stage 4: Add A Broad Open Adapter Lane On Non-Apple Hardware

This is the stage where decentralized training stops being Apple-specific and
starts becoming a true worldwide substrate.

The next target should be an open-model adapter family on hardware that more
participants can actually contribute:

- CUDA
- Metal outside Apple FM constraints
- possibly CPU-only low-end participation for narrow tasks

The key design rule is:

- keep the control-plane objects generic
- keep the validator and accepted-outcome path generic
- make only the execution backend model-specific

By this point, Apple should be one workload family, not the architecture.

This stage is likely the first place where network effects become meaningful,
because the contributor pool becomes much wider.

## Stage 5: Add Regional Or Homogeneous Synchronous Collective Training

Only after the asynchronous and validator-owned contribution model works should
Psionic lean heavily on:

- `DISTRIBUTED_OPTIMIZER_REFERENCE.md`
- `COLLECTIVE_SYNC_POLICY_REFERENCE.md`

This is the right stage for:

- regional clusters
- datacenter or near-datacenter participation
- homogeneous hardware groups
- low-latency sync domains
- sharded optimizer and collective-backed gradient exchange

This stage is where synchronous training starts to make sense.

It is not the right first internet-wide design.

The correct shape is likely hierarchical:

- local or regional subgroup sync first
- periodic global reconciliation second

not one flat worldwide mesh.

## Stage 6: Build Toward Something Massive

"Something massive" should mean more than "a larger bill."

It should mean a system that can reliably do all of the following:

- keep contributor selection deterministic
- keep stale or malicious updates bounded and inspectable
- keep checkpoint and policy revision lineage explicit
- evaluate candidate revisions before promotion
- absorb participant churn without collapsing the run
- use synchronous collectives only where the network and hardware justify them

At that point the likely growth path is:

1. larger open-model adapter training
2. continued pretraining or domain adaptation on larger corpora
3. distillation from stronger teacher systems into tractable student models
4. eventually partial or full-model synchronized training inside trusted
   regional clusters

That is a realistic path to "needs lots of compute."

Jumping directly from single-host training to world-scale full-model collective
training is not.

## The Logical Capability Order

If the goal is to build the system in the right dependency order, the logical
capability sequence is:

1. typed training policy, checkpoint, and accepted-outcome truth
2. deterministic window planning and participant admission
3. artifact manifest and restore protocol
4. validator-owned replay and benchmark scoring
5. asynchronous contribution aggregation
6. contributor quality and reliability scoring
7. open-hardware adapter training lane
8. regional collective-backed optimizer execution
9. hierarchical multi-region promotion and checkpoint publication

That order is important because each stage validates a harder assumption:

- first that the authority and eval model is sound
- then that multi-party contribution is sound
- then that broad participation is sound
- only then that synchronized high-throughput training is sound

## GitHub Issue Program

This audit is no longer only a strategy note.

The following GitHub issue program now exists as the concrete execution plan
for decentralized adapter training:

- [#3649](https://github.com/OpenAgentsInc/openagents/issues/3649) Roadmap:
  deliver decentralized adapter training on Psionic.
  This is the master task and the canonical exit-criteria issue for the whole
  program.
- [#3636](https://github.com/OpenAgentsInc/openagents/issues/3636) Psionic
  Docs: freeze the decentralized adapter training system spec and acceptance
  matrix.
  This issue makes the program canonical in Psionic docs and fixes the object
  vocabulary, acceptance states, aggregation rules, and explicit non-goals.
- [#3637](https://github.com/OpenAgentsInc/openagents/issues/3637) Psionic
  Train: implement decentralized adapter training window contracts and
  contribution receipts.
  This is the first core train issue because the generic `TrainingWindow`
  object must gain adapter-specific contribution truth before any live
  multi-party execution can be honest.
- [#3638](https://github.com/OpenAgentsInc/openagents/issues/3638) Psionic
  Cluster: connect live cluster membership and contributor selection to adapter
  training windows.
  This turns the existing run-graph and contributor-selection model into a live
  multi-machine admission and reselection path.
- [#3639](https://github.com/OpenAgentsInc/openagents/issues/3639) Psionic
  Train: add decentralized adapter worker claim, heartbeat, and assignment
  protocol.
  This issue defines how contributors actually participate in one active window
  and is the first live worker-facing protocol step.
- [#3640](https://github.com/OpenAgentsInc/openagents/issues/3640) Psionic
  Datastream and Artifact Storage: stage adapter delta uploads, manifests, and
  checkpoint pointers per window.
  This closes the artifact-movement gap so contributions are resumable,
  inspectable, and recoverable instead of being local-only outputs.
- [#3641](https://github.com/OpenAgentsInc/openagents/issues/3641) Security:
  sign contributor submissions and bind adapter artifacts to worker and session
  provenance.
  This is required because decentralized adapter training assumes untrusted
  participants; upload success alone is not enough for acceptance.
- [#3642](https://github.com/OpenAgentsInc/openagents/issues/3642) Psionic
  Eval and Validator: add adapter contribution replay, sampled verification,
  and window scoring.
  This turns validator-owned review into a per-contribution and per-window
  acceptance mechanism rather than only a final exported-model check.
- [#3643](https://github.com/OpenAgentsInc/openagents/issues/3643) Psionic
  Train: aggregate accepted adapter deltas and promote policy revisions.
  This is the issue that makes decentralized training real, because accepted
  multi-party work must deterministically become the next policy revision.
- [#3644](https://github.com/OpenAgentsInc/openagents/issues/3644) Kernel and
  Nexus: persist decentralized adapter window receipts and contribution
  acceptance projections.
  This closes the authority gap so run, window, contribution, and promotion
  truth are queryable and durable beyond local execution state.
- [#3645](https://github.com/OpenAgentsInc/openagents/issues/3645) Compute
  Market and Provider Substrate: add the decentralized adapter training
  contributor product family.
  This issue turns decentralized adapter training from an operator-only system
  into a real contributor market with explicit capability and settlement hooks.
- [#3646](https://github.com/OpenAgentsInc/openagents/issues/3646) Autopilot
  Desktop and autopilotctl: ship decentralized adapter training operator and
  contributor flows.
  This provides the first honest operator and contributor UX while preserving
  the app's role as orchestration over crate-owned runtime truth.
- [#3647](https://github.com/OpenAgentsInc/openagents/issues/3647) Psionic
  Train: generalize decentralized adapter execution beyond Apple FM to an open
  adapter backend.
  This is the widening step that prevents the whole program from hardening into
  an Apple-only architecture.
- [#3648](https://github.com/OpenAgentsInc/openagents/issues/3648) QA and
  Reference Program: add end-to-end decentralized adapter training gates, chaos
  tests, and reference runs.
  This is the final confidence issue and is part of the definition of done, not
  a later polish pass.

The dependency spine of the program is:

1. `#3636` defines the canonical spec.
2. `#3637` through `#3643` make decentralized adapter contribution and
   promotion real inside Psionic execution truth.
3. `#3644` and `#3645` project that truth into authority and market semantics.
4. `#3646` exposes the operator and contributor surfaces.
5. `#3647` proves the architecture is broader than Apple.
6. `#3648` proves the system works under end-to-end and failure conditions.

## Recommendation

The program decision is now:

- yes, Apple Foundation Model adapters are a good first decentralized training
  target for OpenAgents
- no, they should not be treated as the eventual universal decentralized
  training substrate

The strategic move is to use Apple adapters as the first narrow proving ground
for:

- training windows
- contribution receipts
- validator-owned acceptance
- checkpoint and policy revision promotion

Then quickly generalize the same system to an open adapter lane on broader
hardware.

That gives Psionic a ladder instead of a cliff:

- Apple first for bounded system proof
- open adapters second for broad participation
- regional collectives third for throughput
- truly massive training only after the earlier control and validation layers
  are trustworthy

## Bottom Line

If the question is "what should we do first to eventually reach a giant
decentralized training system?", the answer is not "start with giant
synchronized training."

The answer is:

> start with a bounded adapter-training lane that already has strong eval and
> authority hooks, make it windowed and decentralized, then widen the hardware
> and model scope only after the validator, checkpoint, and contribution model
> is proven.

In this repo, Apple adapters are the best first bounded lane for that proof.

They are not the final destination.
