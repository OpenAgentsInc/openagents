# Distributed Training Launch Status

Status: current launch comparison  
Scope date: 2026-04-11

This document compares the admitted-node distributed-training MVP defined in
`docs/pylon/distributed-training-mvp-roadmap.md` and
`docs/pylon/distributed-training-phase-tracker.md` against the current operator
truth and the public launch posture described in `docs/transcripts/222.md`.

It should be read alongside the literature review in:

- `docs/training/distributed-llm-training-runs-diloco-distro-demo-sparseloco-audit.md`

The important distinction is simple:

- the admitted-node MVP roadmap is about whether the core architecture and
  issue-tracked implementation landed
- the training audit is about how the main public distributed-training systems
  and algorithms relate to each other
- the launch posture is about whether that implementation now behaves like the
  public story we are telling users next week

Those are related, but they are not identical.

## Short Version

The original admitted-node distributed-training MVP is materially complete in
the sense the roadmap intended. The kernel contracts, `Nexus` windows, `TRN`
publication, `Pylon` training capability surfaces, `Psionic` machine runtime,
checkpoint handoff, validator vocabulary, and weak-device-bearing proof lane
all landed through the `workspace#9` and `workspace#11` follow-on issue sets.

What is **not** yet fully closed is the launch-grade automation and product
integration implied by the current public story. The operator can now rehearse:

- one local actual-lane dry-run
- retained checkpoint recording
- checkpoint handoff from one node to another
- joiner-side resume over Tailnet

That is real distributed-training control-plane and runtime behavior. It is
not yet the same thing as a zero-touch public training market where any running
`Pylon` auto-updates, receives the right slice of work, moves all needed
artifacts automatically, returns accepted output, gets paid for that output,
and exposes the whole process cleanly on the public `Nexus` surfaces without
operator assistance.

## What The Roadmap Already Closed

The roadmap and tracker were trying to close one specific admitted-node MVP.
Against that narrower target, the important pieces are in place:

- one frozen work-class, window, checkpoint, and validator contract family
- admitted-node registration and scheduler-facing capability publication
- `Pylon` supervision and retained training runtime state
- `Psionic` machine-mode launch, checkpoint, serve-checkpoint, resume, and
  validator surfaces
- `Nexus` authority objects for training runs, windows, contributions, closeout,
  and accepted outcomes
- `TRN` publication for networks, nodes, windows, receipts, verdicts, artifact
  locators, and closeout state
- one weak-device-bearing accepted-outcome lane

The original roadmap should therefore still be read as a true description of
what the admitted-node MVP implementation program closed.

## What The Current Public Launch Promise Adds

Transcript `222` adds a stronger public promise than the original roadmap did.
It says, in plain language, that:

- the `Pylon` network is live and growing now
- `Pylon` nodes currently receiving placeholder sats will start receiving real
  pieces of a decentralized training run next week
- different machines should receive work that fits their machine
- public stats and visualizations should show what those machines are doing
- the network should be legible as a real decentralized training run, not just
  a liveness miner

That requires a tighter product-level integration across `Nexus`, `Pylon`, and
`Psionic` than the original roadmap exit gates required.

## Current Operator Truth

The current system is best described as an operator-driven retained training
runtime with real multi-machine control-plane continuity.

In practice, the exercised system can:

- plan one actual-lane run locally
- retain machine-readable status and checkpoint artifacts
- hand an accepted checkpoint from a source node to a joiner node
- validate runtime identity on the joiner
- resume the retained run root on the joiner over Tailnet

That is already beyond "placeholder architecture." It proves that the retained
run model, checkpoint lineage, and multi-node recovery surfaces are real.

The current system still assumes an operator understands:

- run manifests
- admitted build identity
- retained artifact surfaces
- remote worktree discipline
- when a handoff receipt points at source-host-local files rather than a shared
  artifact location

That is the main difference between "admitted-node MVP landed" and
"next-week public launch is already fully automatic."

## Remaining Gaps To The Transcript-222 Launch Story

### 1. `Nexus` still needs stronger live work orchestration

`Nexus` now has the vocabulary and issue-tracked control-plane pieces for
training, but the launch story requires it to behave like a live work
coordinator for many heterogeneous `Pylon` nodes. It needs to make explicit,
durable work-class decisions and project them publicly.

The next-week launch claim is strongest when `Nexus` can visibly answer:

- which nodes are merely online
- which nodes are admitted for current work
- which windows are active
- which work classes are active
- which nodes are contributing accepted progress
- what the current aggregate and checkpoint state is

If `Nexus` public surfaces cannot show that difference, the launch reads like
presence tracking with training language layered on top.

### 2. `Pylon` still needs launch-grade automatic work intake

`Pylon` now has truthful training capability publication, an explicit
`provider.training_capability_envelope.v2` surface for work-class and
replica-type eligibility, internal `psionic-train` supervision, retained
artifact courier primitives, and authority-sync hooks. What it still needs for
the launch story is smoother automatic assignment intake and runtime packaging.

The next-week public claim assumes a running `Pylon` can:

- auto-update into the training-capable version
- receive work appropriate to its machine
- fetch the correct retained artifacts
- launch the correct `Psionic` path
- persist outcome and receipts
- report success or failure back to `Nexus`

Today that path is not yet fully equivalent to "operator handcrafted a manifest
for a known machine." The node-side pieces are present, but the full public
experience still depends too much on operator discipline.

That new capability envelope matters because it freezes one measurable machine
contract for launch:

- `validation_replay` remains the default weak-device lane
- `evaluation` and `adapter_training` require a real trainer-tier machine
- `grouped_replica_stage_execution` and
  `full_island_local_update_training` require island-grade posture
- replica-type eligibility is explicit instead of being inferred from prose

### 3. `Psionic` still needs a cleaner artifact mobility story

The Tailnet rehearsal proved that checkpoint handoff and resume are real. It
also exposed the remaining launch-grade gap: the current handoff receipt was
not self-sufficient, and the joiner needed checkpoint pointer and manifest
artifacts staged at the expected paths before resume would succeed.

That means the runtime still needs one of:

- self-contained handoff envelopes
- a shared artifact-store model that joiners can resolve automatically
- a stronger courier path that rewrites or rematerializes source-host-local
  references before resume

Until that is done, the runtime is real and useful, but not yet automatic in
the way the launch story implies.

### 4. Public weak-device truth needs to stay explicit

The broader architecture work correctly moved OpenAgents away from the false
premise that only large home clusters matter. The system now models weaker-node
or mixed-role contribution through work classes, grouped stages, and accepted
closeout surfaces. For launch, that still needs one clear public-facing proof
story.

The strongest version is:

- one weaker device gets real `validation_replay` work
- that work reaches accepted closeout or accepted contribution state
- the public surfaces and receipts make that visible

Without that, the small-machine claim stays technically plausible but socially
harder to defend.

For launch, that default matters more than the broader architecture ambitions.
`Psionic` already contains grouped-stage and weak-device-proof surfaces, but the
next-week public contract should not quietly depend on grouped-replica work
landing as a launch blocker. The bounded validator replay lane is the honest
default because it already has the retained manifest, artifact rematerialization,
receipt, and refusal vocabulary needed for weak-device real work. Grouped
replicas remain a later expansion lane unless the launch docs change
explicitly.

## Honest Launch Statement

The strongest honest launch statement today is:

OpenAgents has finished the admitted-node distributed-training MVP and now has
real retained runtime, checkpoint handoff, resume, validator, and public-state
surfaces across `Nexus`, `Pylon`, and `Psionic`. The next public launch should
be described as a live distributed-training beta where nodes begin receiving
real admitted work, stronger nodes can do checkpoint-bearing training-class
work, weaker nodes can participate through bounded accepted work classes with
`validation_replay` as the default weak-device lane, and the public stats
surfaces begin showing accepted progress rather than just presence.

The weaker but still honest fallback statement is:

The architecture is landed, the control plane is real, and the next step is to
finish launch-grade automation, artifact mobility, and public-state clarity
before describing the system as a fully automatic decentralized training market.

## Practical Reading Rule

Read the current docs this way:

- `docs/pylon/distributed-training-mvp-roadmap.md`
  - the frozen admitted-node MVP architecture and implementation plan
- `docs/pylon/distributed-training-phase-tracker.md`
  - the phase closure state for that original implementation program
- `docs/training/distributed-llm-training-runs-diloco-distro-demo-sparseloco-audit.md`
  - the prior-art and terminology map for DiLoCo, DisTrO/DeMo, SparseLoCo,
    Templar, Prime, and related public distributed-training systems
- `docs/pylon/distributed-training-launch-status.md`
  - the current comparison between that implemented MVP and the stronger
    launch-grade public story now being told

That separation keeps the roadmap honest without pretending the launch story is
already fully automatic.
