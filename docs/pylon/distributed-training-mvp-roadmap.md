# Pylon Distributed Training MVP Roadmap

Status: openagents cross-repo implementation roadmap  
Scope date: 2026-04-09

Distributed training across many machines has two different problems that often
get flattened into one "decentralization" label. One problem is runtime
execution: launching workers, pinning environments, moving checkpoints,
recovering from failures, sealing work into windows, and keeping the run alive
when nodes leave or rejoin. The other problem is public accountability:
deciding who is allowed to contribute, who validates the work, which receipts
count as accepted outcomes, how rejected work is surfaced, and how the network
can inspect a durable trail of decisions. This roadmap is written from the
premise that those problems should be separated, because the first honest MVP
does not need to solve both at full permissionless strength on day one.

The external references split across those layers in a useful way. `prime` and
`prime-diloco` are the clearest references for serious admitted-node operator
discipline: shared software, pinned environments, heartbeat and liveness
tracking, elastic membership, object-store-backed checkpoints, recovery
plumbing, and a real control plane around the training runtime. `templar`, and
more broadly the Bittensor family of miner and validator systems, add the other
half of the picture: distinct contributor and validator roles, deterministic
windows, replay or scoring loops, public identity, and an ongoing reputation
trail tied to accepted, rejected, or escalated work. Those are different
strengths. A practical roadmap should borrow from both without pretending they
solve the same layer.

This document therefore takes a selective approach. The runtime and operator
model for the MVP should be much closer to Prime than to a public miner market.
The publication and validation model should borrow the useful discipline from
Templar and Bittensor-like systems without copying their full chain,
metagraph, stake-weighting, or permissionless-admission machinery. We want
sealed windows, validator reruns, explicit receipts, artifact lineage, and
reputation labels. We do not need stake-weighted consensus, open public
admission, or hostile-network cryptoeconomic enforcement in the first shipped
version.

That split maps cleanly onto the current `openagents` surfaces and the sibling
`Psionic` repo. `Psionic` should own the actual training and validator replay
runtime. `Pylon` should become the machine-local supervisor, node operator
shell, artifact courier, and authority client. `Nexus` should own admission, run planning,
assignment, lease management, sealing, challenge scheduling, and final
reconciliation. `TRN` should carry the signed public coordination trail for
network, node, window, receipt, verdict, artifact, and closeout state without
pretending that Nostr relays are the training runtime or the source of
consensus.

The target that falls out of this is narrow but meaningful: one admitted-node
distributed training system where contributors only do work through our own
`Pylon` plus `Psionic` stack, where `Nexus` coordinates the run, where
validators rerun sampled work before acceptance, and where accepted outcomes
and reputation signals are published in a machine-legible way. That is much
narrower than a permissionless Bittensor-style network. It is also much more
concrete than a product claim that says "distributed training" without a real
runtime, validator lane, checkpoint story, or closeout discipline behind it.

This roadmap exists to turn that synthesis into an implementation sequence. It
follows the 2026-04-09 distributed-training reference audit, but it is written
as a standalone work plan for a reader who has not memorized that earlier
analysis. The purpose here is to make the MVP shape explicit, freeze the first
set of contracts, and list the specific repo work required to ship one
defensible distributed training lane before expanding outward into broader
validator markets or more hostile trust assumptions.

## Purpose

This document turns the 2026-04-09 distributed-training audit into the concrete
worklist required to ship one honest MVP across:

- `../psionic`
- `apps/pylon`
- `apps/nexus-control`
- `crates/nostr`

It is intentionally narrower than a permissionless or hostile-network design.
The target is one admitted-node distributed training system where contributors
only do work through our own `Pylon` + `Psionic` software and where `Nexus`
coordinates the run.

## MVP Scope

The MVP is:

- admitted nodes only
- one `Nexus` authority and scheduler
- `Pylon` as the node operator and coordinator client
- `Psionic` as the actual training and validation runtime
- `TRN` as the signed Nostr publication layer for network, node, window,
  receipt, artifact, verdict, reputation, and closeout metadata
- off-Nostr artifact transport for checkpoints, deltas, logs, proof bundles,
  and score snapshots
- sampled validator reruns and replay checks before acceptance
- explicit run receipts, checkpoint lineage, window lineage, and closeout
  records

The MVP is not:

- permissionless training
- stake-weighted validator economics
- on-chain consensus
- hostile-network trustless verification
- generalized public miner markets
- full Bittensor-equivalent cryptoeconomic enforcement

## Recommended MVP Architecture

Use this split and do not blur it:

- `Psionic`
  - training runtime
  - validator replay runtime
  - checkpoint and recovery machinery
  - evidence and receipt production
- `Pylon`
  - machine-local supervisor
  - node admission identity
  - `Nexus` client
  - `TRN` publisher
  - artifact courier
  - local status and operator controls
- `Nexus`
  - admitted-node registry
  - run creation
  - window planning
  - assignment and lease management
  - validator challenge scheduling
  - reconciliation, acceptance, closeout, and reputation projection
- `TRN`
  - signed public coordination and publication rail
  - not the source of consensus
  - not the heavy artifact rail

## MVP Exit Criteria

The MVP is done only when all of these are true:

- `Nexus` can create one real training run with frozen policy, checkpoint, and
  validator bindings.
- admitted `Pylon` nodes can register and advertise real training capability
  instead of the current inert default.
- `Nexus` can match nodes into one active run and publish the run and window
  state through `TRN`.
- `Pylon` can launch and supervise `Psionic` training workers from a
  machine-consumable run manifest.
- workers can join, leave, fail, and rejoin without manual metadata surgery.
- checkpoints can be materialized locally, uploaded durably, served to joiners,
  and resumed from.
- `Nexus` can seal a window, schedule validator work, ingest verdicts, and
  reconcile accepted versus rejected contributions.
- contribution outcomes and accepted sealed-window closeouts can be issued with
  signed receipts and artifact pointers.
- `TRN` node records, windows, receipts, verdicts, artifact locators, and
  closeouts are emitted by live code rather than draft docs alone.
- `NIP-32` reputation labels are derived from real verdict and closeout state.
- one multi-node rehearsal can run end-to-end and survive at least one injected
  worker-loss or checkpoint-recovery drill.
- the operator can inspect status from `Pylon` and `Nexus` without reading raw
  JSON files by hand.

## Current Starting Point

The current codebase is a strong substrate but not a shipped training system.

What already exists:

- `openagents-provider-substrate` already models
  `adapter_training_contributor`, match verdicts, and settlement hooks.
- `openagents-kernel-core` already has typed objects for training policies,
  training runs, adapter windows, contribution outcomes, accepted outcomes, and
  validator challenges.
- `nexus-control` already exposes HTTP routes for those kernel objects under
  `/v1/kernel/compute/...`.
- `psionic-train` already has strong contract and evidence vocabulary for
  trusted-cluster runs, decentralized contribution windows, checkpoint
  recovery, validator scoring, signed node identity, and launch contracts.
- `psionic/TRAIN` already gives `Psion` one real operator entrypoint for the
  actual lane.
- the TRN NIP text now covers the public coordination vocabulary needed for the
  MVP.

What is still missing:

- `Pylon` still reports
  `adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default()`
  in `apps/pylon/src/lib.rs`.
- `Pylon` does not yet supervise `psionic-train` as a long-running distributed
  runtime.
- `Nexus` has the authority object model, but not the full scheduler,
  assignment, reconciliation, and `TRN` publication loops for real training.
- `crates/nostr` has the TRN spec text, but not the full typed Rust
  emit/parse helpers and live publisher wiring.
- the current `Psionic` actual-lane launcher explicitly stops short of claiming
  completed distributed cluster execution.

## Phase 0 Contract Freezes

These items are no longer left open in this roadmap. This section is the MVP
contract freeze.

### 0.1 Acceptance Unit

The canonical accepted unit for the MVP is:

- one sealed window

The operational consequences are:

- every assignment still gets a contribution-level receipt and a terminal
  contribution disposition
- contribution dispositions are used for aggregation eligibility, replay, and
  reputation
- `accepted` at the contribution level means "eligible for aggregation," not
  "final economic acceptance"
- `Nexus` builds one aggregate output from accepted contributions only
- `ComputeAcceptedOutcome`, `TRN kind:39530` closeout, and provider settlement
  all happen at the sealed-window level
- the existing `AcceptedContribution` settlement trigger stays in the codebase
  as a future option, but it is disabled for the MVP

This is the simplest honest choice because it keeps:

- validator work centered on one window summary and one aggregate artifact
- payout and closeout logic centered on one final unit
- contribution-level sampling and exclusion explicit without forcing
  per-contribution settlement into the first version

### 0.2 First Supported Topology

The first real MVP topology is:

- homogeneous CUDA workers only
- data parallel only
- one process per admitted GPU worker
- fixed world size inside a window
- elastic membership only at window boundaries
- one canonical checkpoint family
- one canonical environment family for both workers and validators
- one adapter-delta bundle contribution artifact family above that checkpoint
  lineage

Out of scope for the MVP:

- mixed CUDA plus MLX worker sets
- cross-backend collectives
- tensor parallel
- pipeline parallel
- heterogeneous worker families inside one active window
- mid-window world-size change
- full permissionless contributor heterogeneity

The honest MVP shape is therefore:

- one admitted homogeneous worker cluster
- one checkpoint-backed window cadence
- one validator lane replaying sampled contributions and one sealed-window
  aggregate on the same admitted environment family

The broader roadmap target is:

- NVIDIA GPU support through homogeneous CUDA windows
- Apple Silicon support through homogeneous Metal or MLX-backed windows
- one shared `Pylon` plus `Psionic` plus `Nexus` control plane across both
  admitted backend families

That means CUDA is still the first shipped topology, but Apple Silicon is part
of the same roadmap rather than a separate product. What remains out of scope
until later is mixed-backend execution inside one active window.

### 0.3 Run Manifest V1

The canonical machine-consumable wire format is:

- UTF-8 JSON
- one top-level object
- stable schema string
- canonical SHA-256 digest over the JSON with `manifest_digest` omitted or
  empty during hashing
- breaking changes require a new schema string
- additive fields are allowed and unknown fields must be ignored by parsers

The frozen schema id is:

- `openagents.pylon_training_run_manifest.v1`

The manifest is issued by `Nexus`, persisted by `Pylon`, and consumed by
`Psionic`.

The canonical top-level shape is:

```json
{
  "schema_version": "openagents.pylon_training_run_manifest.v1",
  "manifest_id": "manifest.run.alpha.worker.node01.lease03",
  "manifest_digest": "sha256:...",
  "issued_at_ms": 0,
  "expires_at_ms": 0,
  "network_id": "trainnet.alpha",
  "run_id": "run.alpha",
  "window_id": "window.000123",
  "assignment_id": "assign.node01.window000123",
  "lease_id": "lease.node01.window000123",
  "lease_sequence": 3,
  "membership_revision": "members.rev7",
  "role": "worker",
  "node_pubkey": "<hex>",
  "coordinator_pubkey": "<hex>",
  "authority_base_url": "https://nexus.example",
  "training_policy_ref": "policy://training/adapter/v1",
  "validator_policy_ref": "policy://validator/mvp/v1",
  "environment_ref": "env.openagents.cuda.train",
  "environment_version": "2026.04.09",
  "execution_backend": "psionic_train",
  "topology": {},
  "checkpoint": {},
  "artifacts": {},
  "trn": {},
  "dataset": {},
  "validator": {},
  "resume_from": {}
}
```

Required for every manifest:

- `schema_version`
- `manifest_id`
- `manifest_digest`
- `issued_at_ms`
- `expires_at_ms`
- `network_id`
- `run_id`
- `window_id`
- `assignment_id`
- `lease_id`
- `lease_sequence`
- `membership_revision`
- `role`
- `node_pubkey`
- `coordinator_pubkey`
- `authority_base_url`
- `training_policy_ref`
- `validator_policy_ref`
- `environment_ref`
- `environment_version`
- `execution_backend`
- `topology`
- `checkpoint`
- `artifacts`
- `trn`

Role-conditional requirements:

- `worker`
  - `dataset` is required
  - `validator` is omitted
  - `resume_from` is optional
- `validator`
  - `validator` is required
  - `dataset` is omitted unless the replay policy needs a specific slice
  - `resume_from` is optional
- `recovery_source`
  - `resume_from` is required
  - `dataset` and `validator` are omitted

In the MVP, `coordinator` is a logical `Nexus` role, not a leased
`Pylon`-consumed manifest role.

Frozen nested-field requirements:

- `topology`
  - `backend_family`
  - `world_size`
  - `rank`
  - `local_device_ids`
  - `collective_kind`
  - `elastic_boundary`
- `checkpoint`
  - `checkpoint_family`
  - `checkpoint_ref`
  - `manifest_digest`
  - `latest_pointer_ref`
- `artifacts`
  - `bucket_uri`
  - `run_prefix`
  - `window_prefix`
  - `local_run_root`
  - `credential_source`
- `trn`
  - `network_coordinate`
  - `window_coordinate`
  - `relay_urls`

Worker-only dataset fields:

- `dataset.dataset_id`
- `dataset.slice_id`
- `dataset.slice_digest`
- `dataset.assignment_seed`

Validator-only fields:

- `validator.challenge_id`
- `validator.challenge_kind`
- `validator.target_assignment_ids`
- `validator.expected_manifest_digests`
- `validator.retry_attempt`

The manifest digest must also appear in the corresponding `kind:39511`
assignment-published receipt through the TRN `manifest` tag so `Pylon` can
cross-check the HTTP-delivered manifest against the signed public receipt.

### 0.4 Artifact Backend Contract

The artifact backend is frozen for the MVP:

- Google Cloud Storage only

The MVP does not support:

- S3 as a first-class alternative
- arbitrary peer-to-peer artifact fetch
- backend selection per node

The object layout is:

```text
gs://<bucket>/networks/<network_id>/runs/<run_id>/windows/<window_id>/
```

Required subpaths under that prefix:

- `manifests/run_manifest.json`
- `checkpoints/latest_pointer.json`
- `checkpoints/step-<optimizer_step>/checkpoint_manifest.json`
- `contributions/<assignment_id>/adapter_delta_bundle.json`
- `contributions/<assignment_id>/proof_bundle.json`
- `validators/<challenge_id>/verdict.json`
- `windows/<window_id>/sealed_window_bundle.json`
- `windows/<window_id>/score_snapshot.json`

The frozen artifact-digest policy is:

- `sha256` over raw file bytes for uploaded objects
- `sha256` over canonical JSON UTF-8 for JSON manifests and receipts
- every TRN artifact locator must carry the file digest in `x`
- every manifest-backed locator must also carry the manifest digest in
  `manifest`

The frozen credential model is:

- Google Application Default Credentials only
- resolve `GOOGLE_APPLICATION_CREDENTIALS` first
- fall back to instance metadata when running on admitted cloud workers
- persist only the credential-source name
  `google_application_default_credentials` in manifests and receipts
- never persist raw service-account JSON, tokens, or secrets in local ledgers,
  TRN events, or retained receipts

Upload completion is also frozen:

- `Pylon` may not emit a terminal `artifact_uploaded` receipt until every
  required object for that artifact family is uploaded and every digest matches
  locally
- `Nexus` may not publish a `kind:39520` locator with `status=stored` or
  `status=accepted` until the manifest and object digests validate against the
  scheduler's expected bundle shape

### 0.5 Validator Policy V1

The MVP validator policy is frozen as:

- `minimum_validator_count = 1`
- `escalation_validator_count = 2`
- every sealed window gets one aggregate validator replay
- every sealed window also gets sampled contribution replay

The frozen contribution sampling policy is:

- sample count = `min(8, max(2, ceil(admitted_contributions * 0.25)))`
- always include the highest aggregation-weight contribution
- always include the lexicographically earliest accepted assignment id
- fill the remaining sample set deterministically from
  `sha256(window_id || assignment_id)`

The frozen verdict semantics are:

- `accepted`
  - contribution may remain in the aggregate
- `quarantined`
  - contribution is excluded from the aggregate
- `rejected`
  - contribution is excluded from the aggregate and counts as a hard negative
- `replay_required`
  - window closeout is blocked until replay resolves or times out

The frozen acceptance rule is:

- a window is accepted only after:
  - sampled contributions have reached terminal verdicts
  - all `quarantined` and `rejected` contributions have been excluded
  - the aggregate has been rebuilt from surviving contributions
  - one aggregate validator verdict returns `accepted`

The frozen escalation rule is:

- if the first aggregate verdict is `quarantined`, `rejected`, or
  `replay_required`, schedule a second validator
- if the second validator matches, accept that terminal disposition
- if the two terminal verdicts disagree, mark the window `held`

The frozen retry policy is:

- one retry on the same validator for transient runtime or transport failure
- one reassignment to a second validator if the first validator still does not
  produce a terminal verdict
- after two failed attempts total, the affected challenge becomes `held`
- a held challenge blocks sealed-window acceptance

This means the MVP does not auto-accept on incomplete validation, and it does
not need majority voting or stake weighting to be operationally sound.

### 0.6 Failure Ownership

The failure boundary is now frozen as:

- `Pylon` is authoritative for local process state, local file state, local
  upload attempts, and explicit operator drain intent
- `Nexus` is authoritative for leases, assignment freshness, window state,
  accepted outcomes, closeouts, and reputation projection

That resolves the main edge cases as follows.

Lease expiry during upload:

- `Pylon` may finish or abort the local upload attempt
- `Nexus` decides whether the assignment was still live
- any first-seen upload receipt after `expires_at_ms` is stale and ineligible
- stale uploaded objects may remain in storage, but they are not accepted into
  the active window

Partial checkpoint publish:

- `Pylon` owns local checkpoint materialization and object upload
- `Pylon` must not emit a terminal upload receipt for a partial bundle
- `Nexus` treats incomplete bundles as `staged` only
- `Nexus` must not advance the latest accepted checkpoint pointer from a staged
  bundle

Worker drain during window seal:

- `Pylon` may publish `drain_requested` and finish its in-flight local step or
  checkpoint
- `Nexus` alone decides whether that contribution lands in the current window
  or the next membership revision
- `Pylon` cannot self-seal or self-promote the current window

Worker crash after local checkpoint but before durable upload:

- `Pylon` records a local failure receipt
- `Nexus` keeps the prior accepted checkpoint pointer authoritative
- a later recovery source may continue from the last accepted durable pointer,
  not the crashed node's unpublished local state

TRN publication failure:

- local kernel or local operator state remains authoritative
- TRN publication is retried asynchronously
- publication failure blocks public visibility, but it does not silently
  mutate accepted kernel truth

### 0.7 Reputation Policy V1

The MVP uses NIP-32 `kind:1985` label events exactly as labels, not as a second
consensus system.

The frozen namespaces are the same ones already called for by the TRN draft:

- `trn/contributor`
- `trn/validator`
- `trn/build`
- `trn/checkpoint`

This is slightly narrower than NIP-32's reverse-domain recommendation, but it
keeps implementation aligned with the current TRN text instead of creating a
second competing vocabulary.

For the MVP, `Nexus` is the sole publisher of authoritative training
reputation labels. `Pylon` may cache and surface them locally, but it does not
mint authority labels on its own.

Every label event in the MVP must:

- use exactly one namespace per event, matching NIP-32 guidance
- target the affected actor through `p` when there is an actor
- also target the affected event or addressable object through `e` or `a` when
  there is a concrete verdict, closeout, window, build, or checkpoint object

The frozen label mapping is:

- `trn/contributor`
  - `good`
    - published when a contributor is part of a rewarded accepted sealed window
  - `poor`
    - published immediately on refused closeout
    - also published when the same contributor accrues two rejected sampled
      contributions inside the rolling last seven sealed windows
  - `quarantined`
    - published on quarantined closeout
  - `fraud`
    - published only from explicit fraud evidence, never from ordinary replay
      failure
- `trn/validator`
  - `good`
    - published when a validator's terminal verdict matches the final accepted
      window outcome
  - `poor`
    - published when a validator times out or fails two consecutive challenges
      without terminal verdict
  - `inconsistent`
    - published when a validator's terminal verdict is overturned by escalation
      review
- `trn/build`
  - `admitted`
  - `stale`
  - `revoked`
- `trn/checkpoint`
  - `warning`
    - published when the active checkpoint pointer is stale, missing, or
      digest-mismatched for the current window
  - `revoked`
    - published when an admitted checkpoint is removed from the valid lineage

The frozen scheduler treatment is:

- labels are descriptive protocol events
- gating behavior happens in scheduler policy, not in the label event itself
- hard gates:
  - `trn/build=revoked`
  - `trn/contributor=fraud`
  - active `trn/contributor=quarantined`
  - active `trn/validator=inconsistent`
  - `trn/checkpoint=revoked`
- soft preference only:
  - `good`
  - `poor`
  - `quarantined` after the hard-gate period
  - `inconsistent` after the hard-gate period
  - `stale`
  - `warning`

The frozen decay policy is:

- `good` and `poor`
  - scheduler half-life of 14 days
  - ignored after 30 days
- `quarantined` and `inconsistent`
  - hard gate for 7 days
  - then soft negative until 30 days
- `fraud` and `revoked`
  - no automatic decay
  - must be explicitly superseded by later authority action

## Workstream 1: Freeze The MVP Contract

This work should happen first because every later code path depends on it.

- Encode the Phase 0 acceptance choice in code.
  Requirement: the MVP uses sealed-window acceptance and
  `AcceptedSealedWindow` closeout only.
- Encode the first supported topology in code.
  Requirement: homogeneous CUDA data-parallel workers with elastic changes only
  at window boundaries.
- Implement the manifest builder, parser, digesting, and validation rules for
  `openagents.pylon_training_run_manifest.v1`.
- Implement the frozen GCS artifact layout, locator mapping, and credential
  handling in both `Pylon` and `Psionic`.
- Implement validator policy v1 exactly, including sample size, escalation, and
  retry behavior.
- Implement the frozen `Pylon` versus `Nexus` failure ownership rules so edge
  cases land deterministically.
- Implement reputation policy v1 and scheduler projection over the frozen
  `NIP-32` namespaces and labels.
- Add tests that fail if a later change drifts from these Phase 0 choices
  without an intentional schema or policy version bump.

## Workstream 2: Psionic Runtime Work

This is the biggest runtime gap. The MVP needs `Psionic` to move from
contract-rich training surfaces to a `Pylon`-launchable distributed runtime.

### 2.1 Machine-consumable run entrypoints

- Add one stable `psionic-train` entrypoint for `Pylon` to invoke from code.
- Stop relying on the human-oriented `./TRAIN` shell surface as the only way to
  launch work.
- Define one machine-consumable run manifest format with:
  - run id
  - role
  - network id
  - window id
  - cluster membership revision
  - checkpoint source
  - dataset slice or shard assignment
  - validator policy ref
  - artifact roots
  - runtime env bindings
- Add machine-stable exit codes and refusal classes for:
  - bad config
  - missing checkpoint
  - stale assignment
  - unsupported topology
  - validator replay refusal
  - artifact upload failure

### 2.2 Cluster membership and liveness

- Implement one real cluster session state machine.
- Add heartbeats from live workers to the coordinator.
- Add lease renewal semantics so stale workers become explicit instead of
  implicit.
- Add best-effort deathrattle or explicit drain signaling on shutdown.
- Add membership revision tracking so reconfiguration is machine-visible.
- Add join, rejoin, drain, failed, and replaced worker states.
- Add coordinator-side world revision receipts.
- Bind every worker identity to one signed node identity and one build digest.

### 2.3 Checkpointing and live recovery

- Build one fast local checkpoint path suitable for active distributed runs.
- Add async background upload from local checkpoint staging into durable
  storage.
- Add one local manifest per checkpoint with:
  - optimizer step
  - checkpoint ref
  - object digests
  - optimizer and scheduler presence
  - training-progress digest
  - source run id
- Add one live checkpoint serving path for late joiners and rejoiners.
- Add checkpoint fetch, digest verification, and restore logic for joiners.
- Add checkpoint retention, pruning, and latest-pointer updates.
- Add recovery-source role support so a node can serve state without being a
  full current trainer.
- Add recovery receipts for:
  - local materialization
  - durable upload
  - peer handoff
  - restore success
  - restore refusal

### 2.4 Window execution and contribution packaging

- Add one runtime notion of a training window that maps cleanly onto the kernel
  and TRN window model.
- Add deterministic slice or shard assignment materialization inside the run
  manifest.
- Emit contribution receipts with stable ids and digests.
- Emit artifact manifests for:
  - checkpoints
  - adapter delta bundles
  - proof bundles
  - replay inputs
  - score snapshots
- Emit one sealed-window summary with counts and digests matching
  `ComputeAdapterTrainingWindow`.

### 2.5 Validator replay and scoring runtime

- Add one runnable validator mode in `psionic-train`.
- Implement replay of assigned contributions or checkpoint deltas against the
  canonical replay inputs.
- Emit stable validator verdict receipts with:
  - accepted
  - quarantined
  - rejected
  - replay_required
- Emit score artifacts that can be published through `TRN kind:39520
  class=score`.
- Add refusal states for stale checkpoints, missing replay inputs, mismatched
  assignment receipts, or unsupported runtime environments.

### 2.6 Metrics, evidence, and operator surfaces

- Extend the current actual-lane evidence surfaces so they work for live
  distributed runs, not just local operator rehearsals.
- Add live step, throughput, checkpoint age, and worker-health metrics.
- Add one machine-readable run-status packet for `Pylon` to ingest.
- Add one machine-readable window-status packet for `Nexus` reconciliation.
- Add explicit artifacts for:
  - launch manifest
  - membership revision
  - checkpoint pointer
  - recovery receipt
  - validator score receipt
  - sealed-window bundle
  - final closeout bundle

### 2.7 Release identity and admission hardening

- Reuse the signed node identity contract surface in live code instead of only
  fixture and contract form.
- Stamp every runnable build with:
  - release id
  - build digest
  - git commit
  - admitted environment ref
- Add runtime refusal when the admitted release or environment does not match
  the run manifest.
- Add node-side publication of software attestation and capability projection
  so `Pylon` and `Nexus` can enforce admitted builds.

### 2.8 Psionic test and rehearsal coverage

- Add unit coverage for run-manifest parsing and role-state transitions.
- Add multi-process integration tests for join, fail, rejoin, and drain.
- Add checkpoint upload and restore integration tests.
- Add validator replay integration tests against accepted and rejected samples.
- Add failure-injection rehearsals for:
  - lost worker
  - stale assignment
  - corrupted checkpoint pointer
  - missing uploaded artifact
  - validator mismatch

## Workstream 3: Pylon Node Operator Work

`Pylon` is currently the main missing product shell for training. It needs to
become the admitted-node supervisor around `Psionic`.

### 3.1 Real training capability detection

- Replace the inert training contributor default in `apps/pylon/src/lib.rs`.
- Detect actual `Psionic` training runtime availability.
- Detect available backend, memory, storage, and network posture.
- Populate:
  - `contributor_supported`
  - `coordinator_match_supported`
  - `authority_receipt_supported`
  - execution backends
  - validator policy refs
  - checkpoint families
  - environment refs
  - minimum and available memory
  - settlement trigger
- Refuse sellable training capability when the machine fails admission.

### 3.2 Pylon training config and local state

- Extend `PylonConfig` with training sections for:
  - allowed training networks
  - role claims
  - local run root
  - artifact store credentials source names
  - checkpoint serve address
  - `Nexus` training authority URL
  - `TRN` publication relays
  - validator enablement
  - disk quota and retention limits
- Add one persisted training-runtime state store separate from the inference
  ledger.
- Add one local cache for run manifests, lease state, window state, and latest
  published TRN ids.

### 3.3 Psionic process supervision

- Add one supervised child-process runner for `psionic-train`.
- Support launch, stop, drain, restart, and status.
- Capture stdout, stderr, exit code, and last heartbeat.
- Tie child-process state to the assigned run id and window id.
- Prevent multiple conflicting active training assignments on one node.
- Preserve logs and failure receipts across restart.

### 3.4 Nexus coordinator client

- Add `Pylon -> Nexus` training RPC client code.
- Reuse existing `nexus-control` HTTP routes for:
  - training policy lookup
  - training run lookup
  - adapter window record and query
  - contribution outcome query
  - accepted outcome query
  - validator challenge schedule, lease, and finalize
- Add new coordination calls as needed for:
  - node admission
  - run lease
  - heartbeat
  - assignment ack
  - drain notice
  - failure notice
  - window progress
  - checkpoint publication
- Make those flows idempotent.

### 3.5 Artifact courier and checkpoint serving

- Add upload/download logic for the frozen GCS backend covering checkpoints,
  proof bundles, and score artifacts.
- Add digest verification before publication.
- Add retry and backoff around object-store failures.
- Add one local checkpoint-serving path for recovery-source behavior.
- Add artifact garbage collection once retention policy permits deletion.
- Add local manifest inspection commands for operators.

### 3.6 TRN publication from live Pylon state

- Publish `kind:39501` Training Node Records from live `Pylon` capability
  state.
- Publish node liveness updates and status changes through amended or renewed
  node records when appropriate.
- Publish `kind:39511` receipts for assignment ack, artifact upload,
  replay-request, and other node-originated events.
- Publish `kind:39520` artifact locators for uploaded checkpoints and proof
  bundles.
- Publish node-side labels or references needed for later `NIP-32`
  attribution.
- Persist the event ids and `a` references of everything `Pylon` publishes.

### 3.7 Closeout, settlement, and reputation ingestion

- Add training-aware settlement handling alongside the current inference
  settlement ledger.
- Ingest contribution outcomes and accepted sealed-window closeouts from
  `Nexus`.
- Project closeout state into the local operator view.
- Ingest `NIP-32` labels relevant to the node and surface them locally.
- Refuse automatic re-advertisement as sellable when local node reputation or
  build status has been downgraded.

### 3.8 Pylon operator surface

- Add training-aware `pylon status` output.
- Add training-aware admin HTTP endpoints for:
  - current run
  - active window
  - last checkpoint
  - validator queue
  - last published TRN events
  - recent refusals and failures
- Add one concise local doctor command for training readiness.
- Add one local command to publish or refresh the node record on demand.

### 3.9 Pylon test coverage

- Add tests for capability detection.
- Add tests for config validation and refusal states.
- Add tests for `Psionic` child supervision and restart handling.
- Add tests for `Nexus` client idempotency.
- Add tests for artifact upload/download and digest mismatch handling.
- Add tests for TRN publication and deduplication.

## Workstream 4: Nexus Authority And Scheduler Work

`Nexus` already has much of the typed kernel surface. The missing work is the
live coordinator plane.

### 4.1 Admitted-node registry

- Add one registry of admitted training nodes keyed by node pubkey and build
  identity.
- Track:
  - role claims
  - build digest
  - last heartbeat
  - capability envelope
  - storage and memory posture
  - last successful run
  - active reputation labels
- Distinguish raw provider presence from admitted training presence.

### 4.2 Run creation and scheduler loop

- Add one training scheduler service above the existing kernel objects.
- Create and persist the run, then bind it to the canonical:
  - checkpoint family
  - validator policy
  - benchmark package set
  - environment package
  - artifact roots
- Match admitted nodes against run requirements.
- Issue leases and assignments to nodes.
- Track active membership and expired leases.
- Handle worker replacement and re-assignment.

### 4.3 Window planning and reconciliation

- Turn the existing `ComputeAdapterTrainingWindow` object model into a live
  coordinator loop.
- Open windows with deterministic ids and assignment seeds.
- Bind every window to:
  - training run id
  - stage id
  - contributor-set revision
  - validator policy ref
  - checkpoint pointer
  - dataset or shard slice plan
- Seal windows when the acceptance conditions or timeout conditions are met.
- Reconcile accepted, quarantined, rejected, and replay-required
  contributions.
- Record the canonical window summary and contribution outcomes through the
  existing kernel routes.

### 4.4 Validator automation

- Schedule validator work automatically from sealed windows or sampled
  contributions.
- Lease validator challenges to admitted validator nodes.
- Ingest finalized verdicts and score artifacts.
- Promote the verdicts into contribution outcomes and window disposition.
- Refuse acceptance if validator evidence is incomplete.
- Support the frozen replay retry, escalation, timeout, quarantine, and held
  flows from validator policy v1.

### 4.5 Accepted outcomes and closeouts

- Issue accepted outcomes for runs or accepted windows when promotion criteria
  are satisfied.
- Generate closeouts for:
  - rewarded
  - no_reward
  - held
  - quarantined
  - refused
  - slashed only if we actually implement that behavior
- Link closeouts to artifact locators, verdicts, and accepted outcomes.
- Compute settlement eligibility and payout hooks from those closeouts.

### 4.6 TRN publication from Nexus

- Publish `kind:39500` Training Network Contracts for the admitted training
  network.
- Publish `kind:39510` Training Windows as the canonical window record.
- Publish coordinator-originated `kind:39511` receipts for planned, active,
  sealed, reconciled, and replay-requested window states.
- Publish authoritative `kind:39530` closeouts.
- Publish authoritative `kind:39520 class=score` locators for score snapshots.
- Link `TRN` events back to kernel receipt ids and object ids.

### 4.7 Reputation projection

- Derive `NIP-32` labels from:
  - validator verdict quality
  - closeout state
  - build revocation
  - checkpoint-warning conditions
- Publish both actor-targeted and event-targeted labels.
- Feed reputation back into scheduler preference and admission policy.

### 4.8 Nexus operator views and metrics

- Add run-level metrics to `/stats` and any internal operator views.
- Expose:
  - admitted training nodes online
  - active training runs
  - active windows
  - sealed windows pending validation
  - validator challenges queued and open
  - checkpoint age
  - artifact upload failures
  - payout-eligible closeouts
- Add one operator endpoint or report for a full run summary.

### 4.9 Nexus persistence and replay safety

- Ensure all training scheduler state survives restart.
- Persist leases, window state, accepted contributions, and active challenges.
- Rebuild `TRN` publication pointers after restart without double-publishing
  logically new receipts.
- Recompute runtime state from persisted kernel truth when possible.

### 4.10 Nexus test coverage

- Add scheduler tests for node matching and lease expiry.
- Add window planner tests for deterministic assignment.
- Add reconciliation tests for accepted, rejected, quarantined, and
  replay-required cases.
- Add validator timeout and retry tests.
- Add closeout and reputation projection tests.
- Add restart-replay tests over persisted training state.

## Workstream 5: TRN And Nostr Code Implementation

The NIP text is in decent shape. The missing work is live Rust implementation.

### 5.1 Typed TRN event support

- Add typed Rust structs and builders for:
  - `kind:39500`
  - `kind:39501`
  - `kind:39510`
  - `kind:39511`
  - `kind:39512`
  - `kind:39520`
  - `kind:39530`
- Add parse, validate, and normalize helpers in `crates/nostr`.
- Add helpers for the current actor `p`-tag recommendations.
- Add helpers for score-snapshot locator publication.

### 5.2 Event mapping helpers

- Add one mapping layer from `Nexus` kernel objects into TRN events.
- Add one mapping layer from `Pylon` node state into TRN node records and
  receipts.
- Add one mapping layer from validator results into verdict events and `NIP-32`
  labels.

### 5.3 NIP-32 reputation helpers

- Add TRN-specific `NIP-32` namespace helpers for:
  - `trn/contributor`
  - `trn/validator`
  - `trn/build`
  - `trn/checkpoint`
- Add canonical label builders from verdict and closeout state.
- Add canonical subject tagging to target both pubkeys and event or addressable
  objects when appropriate.
- Add scheduler-facing decay and gating projection code matching reputation
  policy v1 instead of leaving label interpretation ad hoc.

### 5.4 Relay publication reliability

- Add publish retry, dedupe, and local persistence for TRN event emission.
- Track publication outcome per event and relay.
- Support replay or catch-up after temporary relay outage.
- Add local indexes so `Pylon` and `Nexus` can answer “what did we publish?”
  without searching raw relay history every time.

### 5.5 TRN test coverage

- Add roundtrip serialization tests.
- Add required-tag validation tests.
- Add actor-tag and label helper tests.
- Add cross-process emission tests from `Pylon` and `Nexus`.

## Workstream 6: Shared Artifact, Identity, And Security Plumbing

These pieces cut across all three owned repos and are required for defensible
MVP behavior.

- Freeze one object-store backend and one credential-source model.
- Freeze one artifact naming convention for checkpoints, deltas, proof bundles,
  and scores.
- Freeze one stable digest policy and make every publisher use it.
- Bind every training node to:
  - node pubkey
  - admitted release id
  - build digest
  - operator settlement identity if needed
- Add revocation handling for bad builds or removed nodes.
- Add one redaction policy so retained receipts do not leak raw secrets.
- Add one clock and id-generation policy for run ids, window ids, challenge
  ids, and artifact ids.
- Add one minimal randomness-source policy for assignment seeds so window
  planning is deterministic and replayable.
- Add a source-of-truth dataset identity or slice manifest binding for every
  window.

## Workstream 7: Defensibility Requirements For The Admitted MVP

These are not “hostile-network verification.” They are the minimum safeguards
needed so the admitted-node MVP is technically defensible.

- Only admit nodes running named approved `Pylon` and `Psionic` release ids.
- Require signed node identity and build attestation before scheduling work.
- Require artifact digests for every checkpoint and contribution output.
- Require validator evidence before accepted closeout.
- Require `Nexus` to reject stale assignments, stale checkpoints, and digest
  mismatches.
- Require refusal states rather than silent fallback to unverified paths.
- Require one sampled replay or validator rerun cadence, even on admitted
  nodes.
- Require persisted audit trails for:
  - assignment
  - upload
  - replay
  - verdict
  - acceptance
  - closeout
- Require local and authority-visible handling for bad build, bad artifact, or
  bad validator outcomes.

## Workstream 8: End-To-End Test And Rehearsal Matrix

Do not ship the MVP without this test matrix.

- one-node dry run through `Pylon -> Psionic -> Nexus`
- two-node admitted training start and healthy completion
- late joiner from live checkpoint
- worker crash and recovery-source restore
- lease expiry and reassignment
- checkpoint upload failure and retry
- validator replay accepted case
- validator replay rejected case
- validator timeout case
- sealed-window reconciliation case
- TRN publication outage and later catch-up
- `Nexus` restart during active run
- `Pylon` restart during active run
- closeout and reputation publication after reconciliation

## Workstream 9: Apple Silicon And Metal Expansion

This work happens after the first honest admitted CUDA run, but it is part of
the same distributed-training roadmap. The target state is dual admitted
backend support:

- NVIDIA GPU nodes through the existing CUDA-first lane
- Apple Silicon nodes through one admitted Metal or MLX-backed lane

The first Apple expansion is frozen as:

- homogeneous Apple Silicon workers only
- data parallel only
- one canonical Apple environment family
- one canonical Apple validator environment family
- no mixed CUDA plus Apple workers inside one active window
- no cross-backend collectives
- no claim of checkpoint or optimizer portability across backend families until
  replay and restore parity are proven

The work required is:

- `Psionic`
  - add one machine-launchable Apple training lane under the same run-manifest
    contract used by CUDA
  - bind that lane to one admitted Apple environment ref and one admitted build
    family
  - add Apple validator replay on the same backend family so Apple windows are
    validated on Apple
  - add Apple checkpoint, artifact, and score emission using the same digest
    and locator policies already frozen for CUDA
- `Pylon`
  - detect Apple Silicon training capability, memory posture, storage posture,
    and admitted environment identity
  - advertise Apple training support in node capability publication
  - supervise the Apple training runtime through the same assignment, lease,
    receipt, and artifact lifecycle used by CUDA nodes
- `Nexus`
  - schedule backend-specific homogeneous windows
  - require worker and validator environment-family matching per window
  - keep acceptance, reconciliation, closeout, and reputation policy identical
    across backend families
  - refuse accidental mixed-backend windows until a later policy version
    explicitly enables them
- `TRN`
  - reuse the same run, node, window, receipt, verdict, artifact, and closeout
    event shapes
  - ensure node records and assignment receipts keep backend family and
    environment refs explicit so CUDA and Apple windows are distinguishable in
    public coordination state

The minimum Apple rehearsal matrix is:

- one Apple single-node dry run through `Pylon -> Psionic -> Nexus`
- one admitted multi-node Apple training rehearsal
- one Apple validator replay accepted case
- one Apple checkpoint restore and rejoin drill
- one scheduler run proving CUDA and Apple nodes can coexist in the same
  network while only being matched into backend-homogeneous windows

Do not widen to mixed CUDA plus Apple windows until all of these are true:

- checkpoint restore is proven across the chosen portability boundary
- validator replay truth is stable on both backend families
- the scheduler can express backend-specific admission and validator matching
  without fallback ambiguity
- any cross-backend artifact portability claim is backed by tests and retained
  evidence

## Implementation Order

This is the recommended build order.

### Phase 0: Contract Freeze

- freeze the admitted-node MVP shape
- freeze run manifest and artifact naming
- freeze role model and acceptance gate
- freeze object-store and relay assumptions

### Phase 1: Psionic Under Pylon Supervision

- make `Pylon` launch and supervise one `Psionic` training process
- make `Pylon` advertise real training capability
- add machine-consumable run manifests and status packets
- prove one local end-to-end run through `Nexus`

### Phase 2: Admitted Multi-Node Runtime

- add membership, leases, heartbeats, and rejoin flows
- add durable checkpoint and live recovery plumbing
- run one admitted multi-node rehearsal with failure injection

### Phase 3: Nexus Windows And Validator Loop

- add live window planning, sealing, reconciliation, and challenge scheduling
- add validator replay execution and verdict ingestion
- promote accepted outcomes and closeouts

### Phase 4: TRN Publication And Reputation

- emit live TRN network, node, window, receipt, verdict, artifact, and
  closeout events
- emit `NIP-32` reputation labels from real state
- make scheduler preference consume those labels

### Phase 5: Dress Rehearsal And Launch

- run the full rehearsal matrix
- verify restart and catch-up behavior
- verify operator visibility from `Pylon` and `Nexus`
- only then call the distributed training MVP real

### Phase 6: Apple Silicon And Metal Support

- add one admitted Apple Silicon training environment under the same roadmap
- make `Pylon` advertise and supervise Apple-capable nodes
- make `Psionic` execute training and validator replay on Apple
- make `Nexus` schedule backend-homogeneous Apple windows beside CUDA windows
- run the Apple rehearsal matrix and only then claim dual NVIDIA plus Apple
  Silicon support

## What Does Not Need To Block MVP

These can wait until after the first honest admitted-node run:

- mixed CUDA plus Apple windows
- threshold-signed window seals
- Bitcoin anchoring for windows or randomness
- permissionless admission
- stake or bond mechanics
- fully open validator markets
- automatic slashing economics
- blockchain-level consensus or finality
- generalized hostile-network verifiability

## Bottom Line

The roadmap is large, but the main gap is not “invent a new protocol.”

The main gap is:

- turning `Psionic` into the actual runnable distributed training and validator
  engine
- turning `Pylon` into the real admitted-node operator shell around it
- turning `Nexus` into the live scheduler, reconciler, closeout authority, and
  TRN publisher

The existing codebase already contains much of the vocabulary needed for that
system. The MVP work is now mostly implementation and hardening, not basic
concept invention.
