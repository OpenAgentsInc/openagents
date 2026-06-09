# 2026-03-14 Covenant Code Lessons for Psionic Train Audit

## Intent

This audit follows the earlier Bittensor / Opentensor ecosystem review and the
`Covenant-72B` paper review with a narrower question:

> after reading the local `~/code/covenant` codebase, which concrete Covenant
> implementation patterns should OpenAgents adapt into Psionic, especially
> `Psionic Train`, and which parts should remain explicitly out of scope?

This is not a paper-level or ecosystem-level answer.

This is a code-grounded answer.

The most important conclusion is:

> Covenant is useful to Psionic when it acts as a reference for training-window
> discipline, checkpointing, validator-owned benchmark truth, typed control
> clients, and fixed-budget experimentation.
>
> Covenant is not a reference for OpenAgents' economic authority, settlement,
> implementation language, or product interface.

## Relationship To Prior Audits

This audit is a follow-on to:

- [2026-03-13-opentensor-ecosystem-adaptation-audit.md](/Users/christopherdavid/code/openagents/docs/audits/2026-03-13-opentensor-ecosystem-adaptation-audit.md)
- [2026-03-13-intellect-lessons-for-psionic-train-audit.md](/Users/christopherdavid/code/openagents/docs/audits/2026-03-13-intellect-lessons-for-psionic-train-audit.md)
- [2026-03-14-autoresearch-hillclimb-targets-for-expanded-psionic-audit.md](/Users/christopherdavid/code/openagents/docs/audits/2026-03-14-autoresearch-hillclimb-targets-for-expanded-psionic-audit.md)

The earlier Opentensor audit used the Bittensor stack and the Covenant paper as
an architectural and market reference.

This audit answers the narrower engineering question:

- what does Covenant's code prove in practice
- which parts of that code should Psionic reuse as design patterns
- which parts of that code should OpenAgents explicitly not import into its own
  architecture

## Scope

OpenAgents sources reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
- [2026-03-13-opentensor-ecosystem-adaptation-audit.md](/Users/christopherdavid/code/openagents/docs/audits/2026-03-13-opentensor-ecosystem-adaptation-audit.md)
- [2026-03-13-intellect-lessons-for-psionic-train-audit.md](/Users/christopherdavid/code/openagents/docs/audits/2026-03-13-intellect-lessons-for-psionic-train-audit.md)

Covenant sources reviewed:

- `~/code/covenant/README.md`
- `~/code/covenant/templar/README.md`
- `~/code/covenant/templar/docs/validator.md`
- `~/code/covenant/templar/neurons/trainer.py`
- `~/code/covenant/templar/tests/test_dcp_checkpoint.py`
- `~/code/covenant/crusades/README.md`
- `~/code/covenant/crusades/docs/Validator.md`
- `~/code/covenant/crabtensor/README.md`
- `~/code/covenant/Megakernels/README.md`
- `~/code/covenant/autoresearch/README.md`

## Executive Summary

The Covenant repo is not one thing.

It is several related experiments:

- `templar` is decentralized training around synchronized windows, shared
  artifact storage, and validator-scored contributions
- `crusades` is validator-owned benchmark evaluation of submitted training code
- `crabtensor` is a typed Rust client for Bittensor interaction
- `Megakernels` is a low-latency kernel and runtime optimization lab
- `autoresearch` is a fixed-budget autonomous experiment loop

The right OpenAgents reading is therefore not "copy Covenant."

The right reading is:

- `templar` is useful for `Psionic Train` control-plane structure, checkpoint
  object discipline, optimizer-state residency policy, and training telemetry
- `crusades` is useful for validator benchmark harness design, repeat-run
  scoring, and containerized evaluation truth
- `crabtensor` is useful for typed Rust control clients and compile-time
  metadata discipline
- `autoresearch` is useful for later hillclimb loops over train and eval policy
- `Megakernels` is useful as a performance reference, but not as a market or
  train contract

What OpenAgents should take is mostly discipline.

What OpenAgents should not take is the Covenant product shape:

- no Bittensor weight-setting as compute settlement
- no emissions tournament as the product loop
- no Python control plane as architecture truth
- no raw `train.py` URL submission as the default Psionic train interface
- no hardcoding of Docker, Basilica, CUDA, A100, or H200 assumptions as system
  truth

If reduced to one sentence:

> Psionic should borrow Covenant's checkpointing, validator harness, and
> fixed-budget experimentation discipline, while rejecting Covenant's chain,
> emissions, Python, and raw-code-submission assumptions.

## What Covenant Code Proves Beyond The Paper

### 1. Windowed decentralized training is a systems pattern, not just a paper idea

The `templar` codebase makes the decentralized training loop concrete:

- synchronized windows
- deterministic data assignment per participant and window
- explicit inner-step and outer-window structure
- gradient sharing through shared storage
- validator-side replay of selected participant work

This matters because it turns the earlier Covenant paper lesson into an
implementation lesson:

> training coordination should be modeled as explicit rounds or windows with
> deterministic assignment and bounded synchronization, not as a fuzzy stream of
> unstructured updates.

Psionic should adapt this as:

- `TrainingWindow` as a first-class control-plane object
- deterministic assignment rules for rollout, batch, or evaluation slices
- explicit separation between per-window control truth and heavy artifact
  transfer
- reason-coded window transitions such as `planned`, `active`, `sealed`,
  `scored`, and `reconciled`

Psionic should not copy:

- Templar's exact chain-clock or blockchain block coupling
- DCT plus top-k compression as architecture truth
- Python training loops as the durable control model

The right adaptation is the window discipline, not the exact mechanism.

### 2. Optimizer-state residency is important enough to be a first-class subsystem

`templar/neurons/trainer.py` is useful because it does not treat training as
"just run backward and step."

It contains real operational training concerns:

- separate optimizer configuration paths for `AdamW` vs `Muon`
- parameter-group distinction between embeddings, scalars, heads, and hidden
  2D weights
- gradient accumulation aware stepping
- optimizer-state offload and prefetch between CPU and GPU
- global metric aggregation for grad norm, update norm, and parameter norm
- explicit scheduler-window logic

That is the important lesson.

Psionic Train should pull in the systems discipline:

- optimizer state residency and movement should be explicit policy
- trainer metrics should include grad norm, update norm, parameter norm, step
  timing, and policy freshness
- optimizer families should be pluggable behind typed Rust contracts rather than
  buried inside one trainer loop
- window cadence and optimizer cadence should be modeled directly instead of
  becoming accidental side effects of one implementation

What Psionic should not copy is the exact optimizer stack.

OpenAgents does not need "Muon parity because Templar used Muon."

It needs:

- reusable optimizer primitives
- state-residency control
- instrumentation good enough for long-lived distributed runs

### 3. Templar's checkpoint discipline is one of the best concrete patterns to steal

The strongest code-level lesson in the Covenant repo is the checkpointing
discipline visible in `templar/tests/test_dcp_checkpoint.py`.

That test suite shows a checkpoint model with:

- an explicit "own bucket vs highest-stake validator bucket" read preference
- `_LATEST.json` pointer discovery
- fallback discovery by listing checkpoint prefixes when the pointer is absent
- extra metadata stored next to checkpoint shards
- round-robin upload assignment across ranks
- object-store based recovery tests with fake comms and fake S3

This is exactly the kind of boring but critical systems truth Psionic needs.

Psionic should adapt this almost directly, but in Rust-native terms:

- `CheckpointPointer` object for latest accepted revision
- `CheckpointManifest` with shard list, digests, writer set, and extra metadata
- deterministic uploader assignment per shard
- restore ladder: preferred pointer, then listing fallback, then explicit
  revision selection
- object-store test doubles that exercise failure and partial-upload paths

This is a much better reference than a generic "save checkpoints sometimes"
story.

The lesson is not "use S3."

The lesson is:

> checkpoint recovery must be a typed, testable protocol with pointer,
> manifest, shard-assignment, and restore-order semantics.

### 4. Crusades proves validator-owned benchmark truth is a separate product family

`crusades` is useful because it makes a different design move than `templar`.

Instead of scoring gradients directly, it scores submitted training code inside
an evaluator-controlled environment.

The validator flow in `crusades/README.md` and `crusades/docs/Validator.md` is:

- read commitments
- reveal/decrypt code location
- download submitted `train.py`
- run it in a validator-controlled environment
- repeat multiple evaluation runs
- verify loss, logits, timer integrity, token counts, and final weights
- compute MFU
- apply a leaderboard and thresholding rule

OpenAgents should not copy the submission surface:

- no raw URL-hosted `train.py` as the canonical Psionic train interface
- no winner-takes-emissions benchmark contest as the default market design
- no chain commitment scheme as the base product loop

But OpenAgents should adapt the validator harness patterns:

- validator-owned benchmark environments
- "simulate the validator locally" operator path using the exact same packaged
  environment
- multi-run evaluation with median or robust aggregation
- majority-success requirements rather than one-shot pass/fail
- verification of timer integrity, token accounting, final state, and declared
  strategy
- strict environment scanning and allowed-surface policy

This is especially relevant to future Psionic product families such as:

- train benchmark lanes
- optimizer and scheduler tournaments
- environment package certification
- eval-only challenge lanes

The key design lesson is:

> benchmark truth belongs to validator-controlled packaged environments, not to
> trust in claimant-provided logs.

### 5. Control plane and heavy artifact plane must stay separate

The earlier Covenant paper already suggested this.

The Covenant code reinforces it.

Across `templar` and `crusades`, the pattern is consistent:

- the authority or coordination layer carries identity, timing, selection, and
  scoring inputs
- heavy artifacts travel through buckets, local files, or evaluator runtimes

That maps cleanly onto Psionic's current direction.

Psionic should make this separation explicit:

- control plane: run identity, window identity, participant selection, policy
  revision, validator posture, receipt references
- heavy artifact plane: checkpoints, rollout bundles, model states, benchmark
  traces, logs, large metrics, and environment payloads

This also supports the earlier Opentensor and Intellect conclusions:

- many active participants
- bounded contributor sets
- persistent ranking
- heavy artifacts outside the authority path

### 6. Typed Rust client discipline is worth copying from Crabtensor

`crabtensor` is not directly a training framework, but it is still useful.

Its strongest lessons are:

- compile-time metadata generation
- typed runtime APIs
- typed storage access
- typed extrinsic payload helpers
- controlled signing and submission paths

OpenAgents should adapt this discipline anywhere Psionic or Nexus talks to an
external authority, chain, or validator network.

The lesson is not "adopt Bittensor."

The lesson is:

> do not let market-critical or validator-critical control paths devolve into
> ad hoc JSON and stringly-typed RPC glue when a typed Rust client surface is
> possible.

### 7. Fixed-budget experimentation from Autoresearch is the right future hillclimb shape

`autoresearch` is intentionally simple:

- one mutable program surface
- one fixed time budget
- one objective metric
- keep or discard after each run

That is much closer to the kind of honest hillclimb loop OpenAgents should want
than open-ended autonomous code mutation over a distributed cluster.

Psionic should adapt this later as:

- fixed-budget policy experiments
- typed experiment specs and promotion records
- keep/discard based on evaluator-owned metrics
- local-first and bounded loops before any shared or distributed loop

This fits both:

- inference-policy hillclimbs
- training-policy hillclimbs once the train runtime exists

The lesson is to keep experimentation bounded, typed, and metric-driven.

### 8. Megakernels is a useful optimization reference, not a system-spec reference

`Megakernels` is the least directly reusable part of the repo for Psionic Train.

It is still useful as:

- a performance reference
- a fused-kernel design reference
- a low-latency runtime benchmark reference

But it should not distort Psionic's architecture.

It does not define:

- train control truth
- checkpoint truth
- validator truth
- market truth

So it belongs in the "performance research reference" bucket, not the "system
contract" bucket.

## What Psionic Should Adapt

The highest-value Covenant pull-ins for Psionic are:

### 1. A first-class training-window protocol

Psionic Train should have explicit objects for:

- training window identity
- participant admission
- bounded contributor selection
- deterministic assignment
- window seal and score transitions

This should be part of the control plane, not a convention hidden in one
trainer.

### 2. A real checkpoint protocol

Psionic Train should formalize:

- checkpoint pointer objects
- checkpoint manifests
- shard digests
- preferred read hierarchy
- uploader assignment
- restore fallback rules

This should be exercised in fake object-store tests.

### 3. Optimizer-state residency and movement policy

Psionic Train should own:

- state offload and prefetch policy
- optimizer family abstraction
- training instrumentation for optimizer behavior
- scheduler-window interplay

### 4. Validator benchmark packages

Psionic should eventually support a validator-owned evaluation package with:

- exact environment package reference
- run-count policy
- aggregation rule
- verification bundle requirements
- operator-local simulation path

### 5. Strong verification bundles for train and eval

Validator-facing verification bundles should include things like:

- policy revision
- environment package version
- timer and token accounting
- final state digests
- selected metric outputs
- declared execution strategy

### 6. Typed Rust integration surfaces

Any external network or authority integration that matters to train or validator
behavior should prefer:

- generated or typed protocol clients
- explicit payload builders
- compile-time metadata binding when possible

### 7. Fixed-budget research loops

Once Psionic Train is real, the first research loops should hillclimb:

- training policy
- contributor selection policy
- validator sampling policy
- environment and eval mix

They should not begin with arbitrary source mutation over the full train stack.

## What Psionic Should Explicitly Not Copy

### 1. No Bittensor settlement or emissions design

OpenAgents should not replace:

- receipts
- validator challenge outcomes
- wallet-confirmed settlement
- kernel authority

with:

- chain weight-setting
- subnet emissions
- winner-takes-emissions tournament logic

That is not the OpenAgents product.

### 2. No Python-first control plane

The useful Covenant logic should be translated into:

- Rust-native runtime objects
- Rust-native receipts
- Rust-native train control logic
- Rust-native sandbox and environment packages

The fact that Covenant expresses these patterns in Python is not a reason for
Psionic to do the same.

### 3. No raw code-URL submission as the default train interface

`crusades` uses raw code submission because it is a competition format.

Psionic should prefer:

- signed environment packages
- manifest-bound train or eval packages
- explicit artifact lineage
- typed train configuration and policy objects

over arbitrary remote scripts.

### 4. No hardcoded hardware or cloud assumptions

Covenant docs are full of stack-specific assumptions such as:

- `2x A100` benchmark expectation
- `4x H200` validator minimums
- Docker image truth
- Basilica remote evaluation mode

These are useful examples of evaluator packaging, but they must not become
Psionic architecture truth.

Psionic should define:

- backend-neutral contracts
- topology-neutral contracts
- environment package requirements
- proof and receipt requirements

without baking in one hardware lane.

## Practical Bottom Line For OpenAgents

The best Covenant-inspired additions to Psionic are not glamorous.

They are:

- training windows
- explicit contributor selection
- object-store aware checkpoint protocols
- optimizer-state residency policy
- validator-owned benchmark packages
- repeat-run verification bundles
- typed Rust control clients
- fixed-budget hillclimb loops

Those are exactly the kinds of things that make a distributed training system
honest and operable.

The things OpenAgents should reject are equally clear:

- emissions-first economics
- chain-weight settlement
- Python as control truth
- arbitrary raw-script submission as the core interface
- hardware-specific assumptions as the system contract

The right target is therefore not:

> make Psionic look like Covenant

It is:

> make Psionic a Rust-native training and evaluation substrate that has learned
> Covenant's best operational discipline without inheriting Covenant's chain,
> stack, or product assumptions.

## Final Recommendation

OpenAgents should treat Covenant as:

- a strong reference for `Psionic Train` lifecycle and checkpoint truth
- a strong reference for validator benchmark harness design
- a moderate reference for typed Rust control clients
- a strong reference for bounded autoresearch loops
- a weak reference for market economics and default product surface

The right parity target is selective.

OpenAgents should aim for near-parity with Covenant's discipline around:

- checkpointing
- validator packaging
- training-window control
- verification bundles
- experiment budgeting

OpenAgents should deliberately diverge on:

- settlement
- authority
- implementation language
- default submission surface
- chain dependence
