# 2026-03-15 Decentralized Training Target Sequencing Audit

## Intent

This audit answers a forward-looking product and systems question:

> if the priority is decentralized training where many people around the world
> contribute compute toward a shared model, are Apple Foundation Model adapters
> a good first target, and what is the logical build order from here to
> something much larger that truly needs global contributed compute?

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

Apple Foundation Model adapters are a good first target only in one specific
sense:

> they are a good first decentralized training **product lane** for proving
> contribution windows, validator-owned evaluation, accepted-outcome authority,
> and bounded artifact exchange.

They are not the best first decentralized training **substrate** if the real
goal is a worldwide training network with broad contributor participation and a
path to much larger models.

The reason is simple:

- Apple adapters are bounded, inspectable, and already partly real in this repo
- worldwide decentralized training is latency-heavy, failure-heavy, and
  heterogenous, so it should start with asynchronous windowed contribution
  rather than immediate global synchronous collectives
- Apple hardware and Apple FM runtime constraints make that lane too narrow to
  be the long-term center of gravity for a global training network

The right sequence is therefore:

1. use Apple adapters to prove the first narrow decentralized training market
   and acceptance loop
2. make that loop windowed and asynchronous before attempting live collectives
3. generalize the same control and authority model to an open adapter lane on
   broader hardware
4. only after that add regional or homogeneous synchronous collective-backed
   training
5. only after that attempt something truly massive

If reduced to one sentence:

> Apple adapters are a good first proving ground for decentralized training
> economics and control flow, but they should be the first rung of the ladder,
> not the final substrate.

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

## Is Apple Foundation Model Adapter Training A Good First Target?

Yes, with a strict qualifier.

It is a good first target if the near-term goal is:

- proving that many independent contributors can participate in one bounded
  training program
- proving that the system can assign work, accept artifacts, validate results,
  and publish accepted training outcomes
- proving a first seller-side training market where contributed work is small
  enough to move and inspect

It is **not** the best first target if the near-term goal is:

- maximizing contributor count as fast as possible
- using heterogeneous global hardware immediately
- building the definitive substrate for large synchronized model training

The distinction between those two goals is the core strategic decision.

### Why Apple Adapters Are Attractive First

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

### Why Apple Adapters Are Not Enough

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

## Recommended Build Order

## Stage 1: Close The Narrow Apple Adapter Training Truth Surface

Use the current Apple lane as the first controlled proving ground.

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

Once windowing exists, use Apple as the first live decentralized training lane.

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

## Recommendation

The best answer is:

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
