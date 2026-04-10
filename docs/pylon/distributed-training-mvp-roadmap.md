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
- `Pylon` now carries one explicit `training` config block plus one separate
  retained runtime-state store under the training run root for manifests,
  leases, windows, active runtime state, and latest published TRN pointers.
- the TRN NIP text now covers the public coordination vocabulary needed for the
  MVP.

What is still missing:

- `Pylon` now runs a bounded local training capability probe from
  `apps/pylon/src/lib.rs`, but it still only projects one admitted CUDA/H100
  contributor envelope from local `Psionic` runtime discovery plus host
  GPU/disk/network posture.
- `Pylon` now has one retained internal child-process supervision core for
  `psionic-train`, but it is not yet wired into live `Nexus` coordination,
  operator/admin surfaces, or public training status publication.
- `Pylon` now has one retained training-coordination client for existing kernel
  training lookups plus new idempotent node-side coordination calls, but
  `Nexus` still lacks the admitted-node registry and live scheduler state those
  calls need behind the full production lane.
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

The frozen machine-readable implementation for this manifest now lives in:

- `crates/openagents-kernel-core/src/pylon_training.rs`

### 0.4 Artifact Backend Contract

The artifact backend is frozen for the MVP:

- Google Cloud Storage only

The MVP does not support:

- S3 as a first-class alternative
- arbitrary peer-to-peer artifact fetch
- backend selection per node

The object layout is:

```text
run root:
gs://<bucket>/networks/<network_id>/runs/<run_id>/

window root:
gs://<bucket>/networks/<network_id>/runs/<run_id>/windows/<window_id>/
```

Required run-scoped objects:

- `manifests/run_manifest.json`
- `checkpoints/latest_pointer.json`
- `checkpoints/step-<optimizer_step>/checkpoint_manifest.json`

Required window-scoped objects:

- `contributions/<assignment_id>/adapter_delta_bundle.json`
- `contributions/<assignment_id>/proof_bundle.json`
- `validators/<challenge_id>/verdict.json`
- `sealed_window_bundle.json`
- `score_snapshot.json`

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

The normalized GCS path helpers and bundle-completeness checks now also live in:

- `crates/openagents-kernel-core/src/pylon_training.rs`

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

The frozen sampling and escalation helpers now also live in:

- `crates/openagents-kernel-core/src/pylon_training.rs`

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

The canonical ownership map for these edge cases now also lives in:

- `crates/openagents-kernel-core/src/pylon_training.rs`

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

The first scheduler-facing reputation projection helpers now also live in:

- `crates/openagents-kernel-core/src/pylon_training.rs`
- `crates/nostr/core/src/nip_trn_reputation.rs`

### 0.8 Shared Machine Artifact Versioning Policy

The manifest versioning discipline is also frozen for the rest of the
machine-readable MVP surfaces.

Every machine-readable JSON artifact in the MVP must:

- be UTF-8 JSON
- use one top-level object
- carry an explicit `schema_version` string
- compute any retained JSON digest over canonical JSON UTF-8 bytes
- ignore unknown additive fields during parsing
- require a new schema string for breaking changes

This versioning rule applies to at least:

- run-status packets
- window-status packets
- checkpoint manifests
- latest-pointer manifests
- sealed-window bundles
- proof bundles
- recovery receipts
- validator verdict receipts
- score-snapshot manifests
- closeout bundles

For the MVP, a change is breaking if it changes:

- required fields
- state-machine semantics
- digest basis
- enum meaning
- actor or artifact binding rules

That means "same field names, different meaning" is not an additive change. It
requires a schema bump.

### 0.9 Frozen Timing And Timeout Policy

The MVP now also freezes one default timing policy so `Pylon`, `Psionic`, and
`Nexus` do not implement different liveness assumptions while claiming
conformance to the same roadmap.

The frozen defaults are:

- `heartbeat_interval_ms = 15000`
- `heartbeat_expiry_ms = 60000`
- `lease_duration_ms = 600000`
- `lease_renewal_threshold_ms = 180000`
- `window_max_duration_ms = 1800000`
- `seal_grace_period_ms = 120000`
- `validator_timeout_ms = 900000`
- `upload_timeout_ms = 1200000`

The frozen retry backoff policy is:

- exponential backoff from `5000ms`
- retry schedule `5s -> 15s -> 30s -> 60s -> 120s`
- cap later retries at `300000ms`
- treat jitter as optional local transport hygiene, not as protocol-visible
  randomness

The operational interpretation is:

- four missed heartbeats is enough to make worker liveness stale
- leases must be renewed before less than three minutes remain
- windows that exceed the max duration move toward seal or refusal rather than
  remaining indefinitely active
- validator work that exceeds the timeout enters retry or held handling under
  validator policy v1
- uploads that exceed the timeout remain non-terminal and ineligible for
  accepted publication

Changing these defaults requires an explicit policy revision and a coordinated
doc update. They are not implementation-local knobs for the MVP.

### 0.10 Shared Refusal And Error Taxonomy

The MVP uses one cross-repo refusal taxonomy for machine supervision, local
operator state, and public receipt mapping.

Every refusal surfaced outside a local process must carry:

- stable `code`
- human-readable `message`
- `retryable` boolean
- `owner` naming the subsystem that owns the next authoritative transition
- the best available artifact, assignment, challenge, or manifest reference

The frozen minimum refusal set is:

| Code | Owner | Retryable | Default receipt mapping |
| --- | --- | --- | --- |
| `bad_config` | `Pylon` | no | local refusal receipt only |
| `stale_assignment` | `Nexus` | no | assignment refused or stale receipt |
| `lease_expired` | `Nexus` | sometimes | assignment stale or expired receipt |
| `unsupported_topology` | `Pylon` | no | launch refused receipt |
| `checkpoint_missing` | `Pylon` | sometimes | recovery or launch refused receipt |
| `checkpoint_digest_mismatch` | `Pylon` | no | refused checkpoint or warning receipt |
| `artifact_incomplete` | `Pylon` | sometimes | non-terminal staged upload receipt |
| `artifact_digest_mismatch` | `Pylon` | no | upload refused receipt |
| `validator_timeout` | `Nexus` | yes | validator verdict timeout or held receipt |
| `validator_disagreement` | `Nexus` | no | held-window or escalation receipt |
| `environment_mismatch` | `Pylon` | no | launch refused receipt |
| `build_revoked` | `Nexus` | no | build-revoked receipt and label |

`Psionic` may still use finer-grained local exit codes internally, but once the
failure is reflected into retained operator state or scheduler state it must
map back to this shared taxonomy.

### 0.11 Minimal Observability Contract

The MVP also freezes a smallest required telemetry envelope so cross-node
debugging stays possible when runs fail in the middle.

Every machine-emitted log line, metric label set, status packet, and retained
receipt must carry these identifiers whenever the object exists:

- `network_id`
- `run_id`
- `window_id`
- `assignment_id`
- `challenge_id`
- `node_pubkey`
- `membership_revision`
- `manifest_digest`

The contract is:

- do not invent placeholder ids
- omit a field only when that object truly does not exist for the event
- keep field names stable across repos
- do not emit one equivalent identifier under different names in different
  repos

This is the minimum observability floor, not the full metric catalog.

The frozen observability field set now also lives in:

- `crates/openagents-kernel-core/src/pylon_training.rs`

## Canonical Lifecycle State Tables

These lifecycle tables are normative for the MVP. Local implementations may
keep narrower sub-states, but any externally visible state must map back to one
of the following canonical states.

### Assignment Lifecycle

| State | Meaning | Transition owner | Next states |
| --- | --- | --- | --- |
| `planned` | `Nexus` created the assignment but has not leased it yet. | `Nexus` | `leased` |
| `leased` | Lease issued and manifest published, waiting for node acceptance. | `Pylon` or `Nexus` | `acked`, `expired`, `drained` |
| `acked` | `Pylon` accepted the manifest and reserved local execution. | `Pylon` | `active`, `failed`, `expired` |
| `active` | `Psionic` is running under a live lease. | `Pylon` reports, `Nexus` decides freshness | `completed`, `failed`, `drained`, `expired` |
| `completed` | Local execution and required upload work finished for the assignment. | `Nexus` | terminal |
| `expired` | Lease freshness was lost before successful completion. | `Nexus` | terminal |
| `drained` | Work was ended at a boundary by operator intent or coordinator action. | `Nexus` | terminal |
| `failed` | Runtime, checkpoint, or artifact contract failed for this assignment. | `Nexus` after local report | terminal |

### Contribution Lifecycle

| State | Meaning | Transition owner | Next states |
| --- | --- | --- | --- |
| `received` | `Nexus` ingested the contribution receipt and bundle pointer. | `Nexus` | `eligible`, `rejected` |
| `eligible` | Bundle shape and timing allow inclusion in the aggregate. | `Nexus` | `sampled`, `accepted`, `quarantined`, `rejected` |
| `sampled` | Contribution was selected for validator replay. | `Nexus` and validator lane | `accepted`, `quarantined`, `rejected`, `replay_required` |
| `accepted` | Contribution may remain in the aggregate. This is not final economic acceptance. | `Nexus` | terminal |
| `quarantined` | Contribution is excluded pending the frozen policy outcome. | `Nexus` | terminal |
| `rejected` | Contribution is excluded and counts as a hard negative. | `Nexus` | terminal |
| `replay_required` | Closeout is blocked until replay resolves or times out. | `Nexus` | terminal |

### Window Lifecycle

| State | Meaning | Transition owner | Next states |
| --- | --- | --- | --- |
| `planned` | Window id, dataset slice plan, and contributor set are defined. | `Nexus` | `active` |
| `active` | Assignments are live and contributions are still arriving. | `Nexus` | `sealing`, `refused` |
| `sealing` | No new contributions should enter; bundle and aggregate work is being finalized. | `Nexus` | `sealed`, `refused` |
| `sealed` | Candidate aggregate and summary are frozen for validation. | `Nexus` | `validating` |
| `validating` | Sampled replay and aggregate validation are running. | `Nexus` | `accepted`, `held`, `refused` |
| `accepted` | Window reached terminal accepted closeout conditions. | `Nexus` | terminal |
| `held` | Validation disagreement or incomplete challenge handling blocks closeout. | `Nexus` | terminal for MVP automation |
| `refused` | The window cannot produce an accepted closeout under the frozen rules. | `Nexus` | terminal |

### Artifact Bundle Lifecycle

| State | Meaning | Transition owner | Next states |
| --- | --- | --- | --- |
| `local_only` | Material exists only on the node. | `Pylon` | `staged` |
| `staged` | Bundle is assembled locally but not yet durably complete. | `Pylon` | `uploaded`, `local_only` |
| `uploaded` | Required objects reached durable storage. | `Pylon` | `verified`, `staged` |
| `verified` | Local and scheduler-side digest and shape checks passed. | `Nexus` | `published`, `staged` |
| `published` | Public locator and receipt publication succeeded. | `Nexus` | `accepted` |
| `accepted` | Bundle is part of accepted scheduler truth. | `Nexus` | terminal |

### Validator Challenge Lifecycle

| State | Meaning | Transition owner | Next states |
| --- | --- | --- | --- |
| `leased` | Validator work was assigned and is waiting to start. | `Nexus` or validator node | `running`, `held` |
| `running` | Validator replay is actively executing. | validator node and `Nexus` | `terminal`, `retrying`, `held` |
| `retrying` | Prior execution timed out or failed transiently within frozen policy. | `Nexus` | `running`, `held` |
| `terminal` | Validator reached a terminal verdict or timeout result. | `Nexus` | terminal |
| `held` | Challenge cannot progress without manual or later policy action. | `Nexus` | terminal for MVP automation |

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
- Freeze the canonical lifecycle state tables so assignment, contribution,
  window, artifact, and validator transitions do not drift across repos.
- Freeze the shared timing and timeout policy instead of leaving operational
  liveness to repo-local defaults.
- Freeze the shared refusal taxonomy, machine-artifact versioning policy, and
  minimal observability envelope.
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
- Use a live local probe rather than config-only claims:
  - resolve the sibling `Psionic` machine training surface
  - combine that with host GPU, disk, and network telemetry
  - project the bounded admitted CUDA/H100 contributor lane honestly
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

- Land the first explicit `training` config block and separate retained
  runtime-state store before process supervision.
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
- Current status: the retained internal runner, attempt-log rotation, heartbeat
  capture, failure receipts, and conflicting-assignment refusal now exist in
  `apps/pylon/src/lib.rs`; later issues still need to connect that foundation
  to real coordinator control flow and operator surfaces.

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
- Current status: `apps/pylon/src/lib.rs` now wraps the existing kernel
  training-policy and training-run lookup routes and defines the retained
  node-side coordination client for node admission, run lease, heartbeat,
  assignment ack, drain notice, failure notice, window progress, and checkpoint
  publication, with retry and env-only bearer-token support. Later issues still
  need to connect those calls to real `Nexus` authority state and operator
  workflows.

### 3.5 Artifact courier and checkpoint serving

- Add upload/download logic for the frozen GCS backend covering checkpoints,
  proof bundles, and score artifacts.
- Add digest verification before publication.
- Add retry and backoff around object-store failures.
- Add one local checkpoint-serving path for recovery-source behavior.
- Add artifact garbage collection once retention policy permits deletion.
- Add local manifest inspection commands for operators.
- Current status: `apps/pylon/src/lib.rs` now contains the first retained GCS
  courier and checkpoint-serving foundation. It can upload and redownload
  checkpoint, contribution-proof, and score bundles against the frozen `gs://`
  layout with digest verification and retry, expose a bounded local checkpoint
  HTTP path, inspect local manifest and artifact state through
  `pylon training artifacts inspect`, and prune the retained download cache
  through `pylon training artifacts gc`. Later issues still need to connect
  those retained transport/reporting paths to live `Nexus` publication,
  closeout, and operator-admin workflows.

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
- Current status: `apps/pylon/src/lib.rs` now contains the first retained
  node-side TRN publication lane. It can publish `kind:39501` training node
  records, `kind:39511` assignment-accepted and artifact-uploaded receipts,
  and `kind:39520` staged artifact locators from retained manifest state
  through `pylon training publish`, while persisting event ids and address
  refs into the retained training runtime-state store. This is still the
  node-claim lane rather than the final authoritative `Nexus` publication
  lane, so later issues still need status/admin projection, closeout
  reconciliation, and `NIP-32`-driven reputation ingestion on top of the
  retained publication pointers.

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

## Admitted-Network Abuse Appendix

The MVP is not a hostile-network trustless system, but it still needs one
explicit abuse and failure-response matrix for the admitted network.

| Case | Detection surface | Immediate response | Receipt or label outcome | Scheduling effect |
| --- | --- | --- | --- | --- |
| admitted node uploads malformed but well-addressed bundle | digest or manifest validation in `Pylon` and `Nexus` | refuse terminal upload, keep bundle staged or refused, do not advance pointers | `artifact_digest_mismatch` or `artifact_incomplete` receipt | no credit for the bundle; repeated cases feed negative scheduler preference |
| validator intentionally withholds verdict | validator timeout and retry counters in `Nexus` | retry once, reassign once, then move challenge or window to `held` | validator timeout receipt and possible `trn/validator=poor` label | blocks acceptance; repeated cases reduce validator preference and can hard-gate under policy |
| operator replays stale manifest | manifest digest, lease sequence, and expiry checks in `Pylon` and `Nexus` | refuse launch, keep prior assignment authoritative | `stale_assignment` or `lease_expired` receipt | no execution credit; stale manifest is ignored |
| node advertises stale build after revocation | build digest and admitted-release check in `Nexus` and `Pylon` | refuse lease or revoke active scheduling immediately | `trn/build=revoked` label and build-revoked receipt | hard gate |
| repeated drain and join flapping to manipulate membership timing | membership revision churn and repeated drain frequency in `Nexus` | stop assigning new windows until behavior stabilizes | local instability receipts and contributor negative reputation when policy threshold is crossed | soft negative first, then hard gate if the frozen policy says so |
| checkpoint pointer rollback attempt | checkpoint lineage and digest regression checks in `Nexus` | refuse pointer advance and keep the last accepted durable checkpoint authoritative | checkpoint warning or revoked label plus refusal receipt | rollback lineage is not eligible for scheduling or acceptance |

## First Vertical Slice Milestone

Before the full backlog expands into the whole rehearsal matrix, the program
needs one named proving slice.

The first proving slice is:

- one worker
- one validator
- one window
- one local checkpoint
- one durable upload
- one sealed-window closeout
- one published TRN trail

This slice is complete only when all of these are true:

- `Pylon` launches `Psionic` from a real run manifest
- `Nexus` leases work, seals the window, and ingests a validator result
- the required artifact bundle reaches durable storage and verifies cleanly
- one `kind:39511` receipt trail and one `kind:39530` closeout are published
- the operator can inspect the run without reconstructing state from raw files

The purpose of this milestone is to prove the core loop before the team spends
time on the full matrix of failure injection, larger node sets, or backend
expansion.

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

## Repo Ownership Map

This roadmap is intentionally cross-repo. Ownership still needs to stay
explicit.

| Surface | Source of truth repo | Main consumers | Direct owner | Blocking dependencies |
| --- | --- | --- | --- | --- |
| training runtime, validator replay, checkpoint truth, build attestation | `../psionic` | `apps/pylon`, `apps/nexus-control` | `Psionic` runtime owner | admitted environment family, manifest contract, artifact contract |
| node supervision, local state, artifact courier, node-side TRN publication | `openagents` | `../psionic`, `apps/nexus-control`, `crates/nostr` | `Pylon` operator owner | machine-launchable `psionic-train`, GCS contract, status packet schemas |
| admitted-node registry, scheduling, sealing, reconciliation, closeouts, reputation projection | `openagents` | `apps/pylon`, `crates/nostr` | `Nexus` authority owner | kernel objects, validator policy v1, persisted scheduler state |
| TRN event typing, relay publication, `NIP-32` mapping, local publish indexes | `openagents` | `apps/pylon`, `apps/nexus-control` | `TRN` and `nostr` owner | frozen event shapes, refusal mapping, relay persistence |
| release gating, rehearsals, and cross-repo policy freeze | `openagents/docs` with linked `../psionic` docs | all runtime and control-plane repos | cross-repo release owner | all rows above |

## Implementation Order

This is the recommended build order.

### Phase 0: Contract Freeze

- freeze the admitted-node MVP shape
- freeze run manifest and artifact naming
- freeze role model and acceptance gate
- freeze state tables, refusal taxonomy, timers, observability, and artifact
  versioning
- freeze object-store and relay assumptions

### Phase 1: Psionic Under Pylon Supervision

- complete the first proving slice with one worker, one validator, one window,
  one durable upload, one sealed-window closeout, and one published TRN trail
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

Features outside the frozen Phase 0 contracts do not enter the MVP by
opportunistic implementation unless they are required to satisfy an exit
criterion.

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

## Appendix: GitHub Issue Backlog

The following backlog translates this roadmap into copy-ready GitHub issue
drafts. The numbering here is document-local ordering, not future GitHub issue
numbers. The issue bodies below keep the frozen MVP contracts, the phased build
order, the Apple follow-on work, and the explicit non-blockers aligned with the
roadmap above. The later lifecycle, timing, refusal, observability, abuse, and
ownership freezes in this revision should be folded into the relevant Phase 0,
Workstream 7, and Meta tracking issues rather than implemented as drifting
repo-local policy.

### Phase 0: Contract Freeze

#### 1. Encode sealed-window acceptance as the MVP acceptance unit

**Summary**

Implement the frozen MVP acceptance rule that the canonical accepted unit is
one sealed window, not one individual contribution.

**Why**

The roadmap freezes sealed-window acceptance as the MVP contract.
Contribution-level receipts and dispositions still exist, but economic
acceptance, accepted outcomes, closeouts, and settlement happen only at the
sealed-window level.

**Scope**

- Encode sealed-window acceptance in coordinator logic.
- Keep contribution-level receipts and terminal dispositions.
- Ensure contribution-level `accepted` means "eligible for aggregation," not
  final acceptance.
- Disable contribution-level settlement for the MVP.
- Preserve the `AcceptedContribution` settlement trigger in code as a future
  path, but make it inactive for the MVP.
- Update comments and docs to reflect the frozen policy.

**Acceptance Criteria**

- No MVP flow emits a final accepted outcome or closeout for a single
  contribution.
- Sealed-window closeout is the only final acceptance path.
- Tests fail if contribution-level acceptance is treated as final economic
  acceptance.

#### 2. Encode the first supported topology as homogeneous CUDA-only windowed
data parallel training

**Summary**

Implement the frozen first topology for the MVP.

**Why**

The roadmap freezes the first real topology as homogeneous CUDA workers only,
data parallel only, one process per admitted GPU worker, fixed world size
inside a window, elastic membership only at window boundaries, one canonical
checkpoint family, one canonical environment family, and one adapter-delta
bundle contribution artifact family.

**Scope**

- Enforce homogeneous CUDA worker requirements.
- Reject unsupported topologies:
  - tensor parallel
  - pipeline parallel
  - mixed CUDA and MLX sets
  - mid-window world-size changes
  - heterogeneous worker families inside a window
- Add explicit refusal states for unsupported topology.

**Acceptance Criteria**

- MVP run manifests and runtime checks reject non-CUDA or mixed-backend
  windows.
- World size is fixed within a window.
- Elastic membership changes only happen across window boundaries.

#### 3. Implement `openagents.pylon_training_run_manifest.v1` builder, parser,
and validator

**Summary**

Add the canonical machine-consumable run manifest implementation for V1.

**Why**

The roadmap freezes UTF-8 JSON, one top-level object, schema id
`openagents.pylon_training_run_manifest.v1`, canonical `sha256` digest with
`manifest_digest` omitted or empty during hashing, additive unknown fields
ignored by parsers, and breaking changes requiring a new schema id.

**Scope**

- Manifest struct definitions.
- Builder APIs.
- Parser APIs.
- Validation rules for required fields.
- Role-conditional validation.
- Canonical digest computation.
- Stable serialization rules.
- Version guardrails for future bumps.

**Acceptance Criteria**

- Valid manifests round-trip parse and serialize.
- Digest matches canonical serialization rules.
- Invalid manifests fail with explicit validation errors.
- Unknown additive fields do not break parsing.

#### 4. Enforce role-conditional manifest requirements for worker, validator,
and recovery_source

**Summary**

Implement role-specific validation and consumption rules for manifest V1.

**Why**

The roadmap freezes these role contracts:

- `worker`: `dataset` required, `validator` omitted
- `validator`: `validator` required, `dataset` optional only if replay policy
  needs it
- `recovery_source`: `resume_from` required
- `coordinator` is a logical `Nexus` role, not a leased `Pylon` manifest role

**Scope**

- Validation logic for role-specific required and forbidden fields.
- Clear refusal messages when role-specific contract is violated.
- Unit tests per role.

**Acceptance Criteria**

- Worker manifests without `dataset` are rejected.
- Validator manifests without a `validator` section are rejected.
- Recovery-source manifests without `resume_from` are rejected.
- Coordinator manifests are not accepted as leased node runtime manifests.

#### 5. Implement manifest digest cross-check against TRN assignment receipts

**Summary**

Cross-check HTTP-delivered run manifests against the signed TRN
assignment-published receipt.

**Why**

The roadmap requires the manifest digest to appear in the corresponding
`kind:39511` assignment-published receipt via the TRN `manifest` tag so
`Pylon` can verify the delivered manifest against the signed receipt.

**Scope**

- Add digest tag generation on the coordinator side.
- Add cross-check validation on the node side.
- Refuse mismatches.
- Persist comparison results in local receipts and logs.

**Acceptance Criteria**

- Matching manifest and receipt succeeds.
- Digest mismatch causes explicit refusal.
- Tests cover success and mismatch cases.

#### 6. Implement the frozen GCS artifact backend contract and object layout

**Summary**

Implement the MVP artifact transport contract using Google Cloud Storage only.

**Why**

The roadmap freezes GCS only, no S3 alternative for the MVP, no arbitrary P2P
artifact fetch, and a fixed object layout under
`gs://<bucket>/networks/<network_id>/runs/<run_id>/windows/<window_id>/`.

**Scope**

- GCS storage client integration.
- Object layout helpers.
- Path builders for:
  - run manifest
  - latest pointer
  - checkpoint manifests
  - contribution bundles
  - proof bundles
  - validator verdicts
  - sealed-window bundles
  - score snapshots
- Validation that uploaded artifacts land in the correct layout.

**Acceptance Criteria**

- All artifact families upload to the frozen path layout.
- Path helpers are shared, not hand-built ad hoc.
- Tests cover every required object path.

#### 7. Enforce the frozen artifact digest policy and locator tagging rules

**Summary**

Implement the artifact digest rules and TRN locator tagging contract.

**Why**

The roadmap freezes `sha256` over raw bytes for uploaded objects, `sha256` over
canonical JSON UTF-8 for JSON manifests and receipts, TRN artifact locators
carrying file digest in `x`, and manifest-backed locators carrying manifest
digest in `manifest`.

**Scope**

- Shared digest helpers.
- JSON canonicalization helpers.
- Locator tag population.
- Validation on publish and ingest.
- Digest mismatch refusal flows.

**Acceptance Criteria**

- Every published artifact locator includes the required digests.
- Digest mismatches are rejected explicitly.
- Tests cover byte objects and canonical JSON objects.

#### 8. Implement ADC-only credential resolution and redact raw credentials
from state

**Summary**

Implement the frozen credential model for artifact access.

**Why**

The roadmap freezes Google Application Default Credentials only, resolution of
`GOOGLE_APPLICATION_CREDENTIALS` first with fallback to instance metadata,
persistence of only the credential-source name, and refusal to persist raw
service-account JSON, tokens, or secrets.

**Scope**

- ADC resolution helpers.
- Credential-source recording.
- Redaction rules.
- Validation that secrets do not leak into ledgers, manifests, TRN events, or
  receipts.

**Acceptance Criteria**

- ADC resolution works in local and cloud environments.
- Only the credential-source name is persisted.
- Tests assert that secrets are never written to retained state.

#### 9. Enforce terminal upload receipt semantics for complete artifact bundles
only

**Summary**

Prevent terminal upload receipts for partial artifact bundles.

**Why**

The roadmap freezes that `Pylon` may not emit terminal `artifact_uploaded`
until every required object is uploaded and digests match, and `Nexus` may not
publish `kind:39520` as stored or accepted until bundle validation succeeds.

**Scope**

- Bundle completeness checks.
- Upload state machine with staged versus terminal states.
- Coordinator-side bundle validation.
- Partial bundle refusal handling.

**Acceptance Criteria**

- Partial uploads do not emit terminal success.
- Incomplete bundles stay staged.
- Accepted locators only appear after full validation.

#### 10. Implement validator policy v1 exactly as frozen

**Summary**

Implement the frozen validator policy for the MVP.

**Why**

The roadmap freezes:

- `minimum_validator_count = 1`
- `escalation_validator_count = 2`
- one aggregate validator replay per sealed window
- sampled contribution replay per sealed window
- deterministic sample sizing and selection
- escalation and retry rules
- held windows on unresolved disagreement or failure

**Scope**

- Sample size computation.
- Deterministic sampling set construction.
- Aggregate validator scheduling.
- Escalation validator scheduling.
- Retry logic.
- Held challenge and window states.
- Exact frozen verdict semantics.

**Acceptance Criteria**

- Sampling matches the frozen formula exactly.
- Escalation triggers only under the frozen conditions.
- Held states block acceptance.
- Tests lock the policy against silent drift.

#### 11. Implement failure ownership rules between Pylon and Nexus

**Summary**

Encode the frozen authority boundary for operational failures.

**Why**

The roadmap freezes `Pylon` as authoritative for local process, file, upload,
and drain-intent state, and `Nexus` as authoritative for leases, assignment
freshness, window state, accepted outcomes, closeouts, and reputation. It also
freezes specific edge-case handling for stale upload, partial checkpoint
publish, drain during seal, crash-before-upload, and TRN publication failure.

**Scope**

- Failure ownership docs in code.
- Edge-case handlers.
- State transition enforcement.
- Tests for each frozen scenario.

**Acceptance Criteria**

- Edge cases resolve per the frozen rules.
- Nodes cannot self-promote windows or accepted outcomes.
- Stale uploads are ignored for acceptance purposes.

#### 12. Implement reputation policy v1 and scheduler projection

**Summary**

Implement authoritative `NIP-32` label publication and scheduler-facing
projection.

**Why**

The roadmap freezes:

- namespaces `trn/contributor`, `trn/validator`, `trn/build`,
  `trn/checkpoint`
- `Nexus` as the sole authority publisher
- exact mapping rules for `good`, `poor`, `quarantined`, `fraud`,
  `inconsistent`, `admitted`, `stale`, `revoked`, and `warning`
- hard-gate versus soft-preference treatment
- frozen decay windows

**Scope**

- Label builders.
- Label publication logic.
- Scheduler projection logic.
- Decay calculations.
- Hard-gate enforcement.
- Local caching and display in `Pylon`.

**Acceptance Criteria**

- Labels are published only by `Nexus`.
- Hard gates and soft preferences match policy.
- Decay behavior matches the frozen windows.
- Tests cover every label class and scheduler effect.

#### 13. Add drift tests that guard all frozen Phase 0 choices

**Summary**

Add policy and schema drift tests that fail when Phase 0 contracts change
without an intentional version bump.

**Why**

The roadmap explicitly calls for tests that fail if later changes drift from
the frozen Phase 0 choices without an intentional schema or policy bump.

**Scope**

- Snapshot tests.
- Schema id and version tests.
- Validator policy lock tests.
- Topology restriction tests.
- Artifact layout lock tests.
- Reputation policy lock tests.

**Acceptance Criteria**

- Any incompatible change fails CI until a version bump and intentional updates
  are made.

### Workstream 2: Psionic Runtime

#### 14. Add a stable machine-consumable `psionic-train` entrypoint

**Summary**

Add a stable runtime entrypoint that `Pylon` can invoke programmatically.

**Why**

The roadmap requires a machine-consumable entrypoint and explicitly says to
stop relying on the human-oriented `./TRAIN` shell surface as the only
launcher.

**Scope**

- CLI or library entrypoint for run-manifest consumption.
- Stable invocation contract.
- Role-aware startup behavior.

**Acceptance Criteria**

- `Pylon` can launch `psionic-train` from code using a manifest path or
  equivalent input.
- No human-only shell wrapper is required.

#### 15. Define machine-stable Psionic exit codes and refusal classes

**Summary**

Implement stable exit codes and refusal classes for machine supervision.

**Why**

The roadmap calls for stable refusal classes including bad config, missing
checkpoint, stale assignment, unsupported topology, validator replay refusal,
and artifact upload failure.

**Scope**

- Exit-code enum.
- Refusal-class enum.
- Structured stderr or status-packet output.
- Supervisor-friendly documentation.

**Acceptance Criteria**

- Each refusal class maps to one stable code.
- `Pylon` can distinguish retryable versus non-retryable failures.

#### 16. Implement the cluster membership and liveness state machine

**Summary**

Add real cluster session state and liveness handling to `Psionic`.

**Why**

The roadmap calls for heartbeats, lease-renewal semantics, deathrattle or
drain, membership revision tracking, and join, rejoin, drain, failed, and
replaced worker states.

**Scope**

- Membership-state model.
- Heartbeat emission.
- Liveness expiry.
- Membership-revision receipts.
- Build-digest and signed-identity binding.

**Acceptance Criteria**

- Worker membership changes are machine-visible.
- Stale workers become explicit.
- Rejoin and replace flows work without manual metadata edits.

#### 17. Implement fast local checkpointing and durable upload plumbing

**Summary**

Build the core checkpoint path for active distributed runs.

**Why**

The roadmap requires a fast local checkpoint path, async durable upload,
checkpoint manifesting, fetch, verify, and restore logic, retention, pruning,
and recovery receipts.

**Scope**

- Local checkpoint writer.
- Checkpoint-manifest schema.
- Background upload.
- Retention and pruning.
- Restore pipeline.
- Recovery receipts.

**Acceptance Criteria**

- Checkpoints can be created, uploaded, fetched, verified, and restored.
- Restore failures emit explicit refusal state.
- Latest-pointer updates behave correctly.

#### 18. Add live checkpoint serving for late joiners and recovery sources

**Summary**

Support late join and recovery-source behavior by serving checkpoint state to
peers.

**Why**

The roadmap requires late joiner and rejoin recovery plus explicit
`recovery_source` support.

**Scope**

- Checkpoint-serving endpoint or protocol.
- Joiner fetch flow.
- `recovery_source` role support.
- Peer handoff receipts.

**Acceptance Criteria**

- Joiners can restore from a live or durable checkpoint source.
- Recovery-source nodes can serve state without being active trainers.

#### 19. Implement runtime window execution and contribution packaging

**Summary**

Add `Psionic` runtime support for windows, receipts, and contribution artifact
packaging.

**Why**

The roadmap requires runtime windows, deterministic assignment materialization,
stable contribution receipts, artifact manifests, and sealed-window summaries
matching kernel models.

**Scope**

- Runtime window object.
- Assignment materialization.
- Stable contribution ids and digests.
- Artifact-manifest generation.
- Sealed-window summary generation.

**Acceptance Criteria**

- Each contribution emits stable receipts and artifacts.
- Sealed-window summary includes counts and digests matching the kernel model.

#### 20. Add runnable validator replay mode to `psionic-train`

**Summary**

Implement validator replay execution inside `Psionic`.

**Why**

The roadmap requires a validator mode that replays assigned contributions or
checkpoint deltas, emits stable verdict receipts, produces score artifacts, and
refuses stale or mismatched inputs.

**Scope**

- Validator-mode launcher.
- Replay-input loading.
- Verdict-receipt emission.
- Score-artifact generation.
- Refusal states for missing, stale, or mismatched inputs.

**Acceptance Criteria**

- Validator mode can process both accepted and rejected cases.
- Verdict states exactly match the frozen semantics.

#### 21. Emit machine-readable run-status and window-status packets

**Summary**

Expose machine-readable runtime status surfaces for `Pylon` and `Nexus`.

**Why**

The roadmap requires one machine-readable run-status packet for `Pylon`, one
machine-readable window-status packet for `Nexus`, and explicit artifacts for
launch manifest, membership revision, checkpoint pointer, recovery receipt,
validator score receipt, sealed-window bundle, and final closeout bundle.

**Scope**

- Status-packet schema.
- Runtime emitters.
- Serialization.
- Integration with supervisor and coordinator surfaces.

**Acceptance Criteria**

- `Pylon` can ingest run-status packets.
- `Nexus` can ingest window-status packets.
- Status is sufficient for reconciliation and operator inspection.

#### 22. Enforce release identity and admitted environment matching in Psionic

**Summary**

Bind runtime execution to admitted release and environment identity.

**Why**

The roadmap requires signed node-identity reuse, runnable build stamping, and
runtime refusal when the admitted release or environment does not match the run
manifest.

**Scope**

- Build stamping.
- Runtime attestation checks.
- Environment-ref enforcement.
- Publication of software attestation and capability projection.

**Acceptance Criteria**

- Mismatched build or environment is refused before training starts.
- Runtime surfaces release id, build digest, git commit, and admitted
  environment ref.

#### 23. Add Psionic unit, integration, and failure-injection coverage

**Summary**

Cover the new runtime contracts and failure paths with tests and rehearsals.

**Why**

The roadmap requires tests for manifest parsing, role transitions, join, fail,
rejoin, drain, checkpoint upload and restore, validator replay, and multiple
injected failure scenarios.

**Scope**

- Unit tests.
- Multi-process integration tests.
- Failure-injection rehearsals.

**Acceptance Criteria**

- CI covers all listed `Psionic` scenarios.
- Failure paths produce explicit expected receipts and refusals.

### Workstream 3: Pylon Node Operator

#### 24. Replace inert training contributor defaults with real capability
detection

**Summary**

Implement real training capability detection in `Pylon`.

**Why**

The roadmap calls out the current inert default and requires actual detection
of runtime availability, backend, memory, storage, network posture,
environment refs, checkpoint families, validator policy refs, and settlement
trigger.

**Scope**

- Detect `Psionic` runtime availability.
- Detect backend and machine posture.
- Populate the training capability envelope.
- Refuse sellable training capability when admission fails.

**Acceptance Criteria**

- Node records reflect real machine capability.
- Unsupported machines do not advertise sellable training capability.

#### 25. Extend `PylonConfig` and add persisted training runtime state

**Summary**

Add training-specific config and local state storage to `Pylon`.

**Why**

The roadmap requires training config sections for networks, role claims, run
root, credential-source names, checkpoint-serve address, `Nexus` URL, relays,
validator enablement, and retention limits, plus persisted runtime state and
local caches.

**Scope**

- Config-schema changes.
- Validation.
- State store for runtime, leases, windows, and publication pointers.
- Migration strategy if needed.

**Acceptance Criteria**

- Training config is validated and persisted.
- Runtime state survives restart.

#### 26. Add supervised child-process management for `psionic-train`

**Summary**

Make `Pylon` the real admitted-node supervisor for training processes.

**Why**

The roadmap requires launch, stop, drain, restart, and status support, capture
of stdout, stderr, exit code, and heartbeat, assignment binding, prevention of
conflicting assignments, and log and failure-receipt preservation across
restart.

**Scope**

- Child-process runner.
- State machine.
- Log capture.
- Heartbeat tracking.
- Assignment-conflict prevention.

**Acceptance Criteria**

- `Pylon` can supervise a training process end to end.
- Restarts preserve logs and failure receipts.

#### 27. Implement the Pylon to Nexus training coordination client

**Summary**

Add training-aware coordinator-client code in `Pylon`.

**Why**

The roadmap requires reuse of existing routes plus new idempotent coordination
calls for node admission, run lease, heartbeat, assignment ack, drain notice,
failure notice, window progress, and checkpoint publication.

**Scope**

- HTTP-client implementation.
- Idempotency keys or equivalent handling.
- Request and response types.
- Retry policy.

**Acceptance Criteria**

- All listed coordination flows work end to end.
- Repeated calls are safe and idempotent where required.

#### 28. Implement artifact courier and checkpoint serving in Pylon

**Summary**

Add upload, download, verification, retry, garbage collection, and operator
inspection for the frozen GCS backend.

**Why**

The roadmap requires `Pylon` to own upload and download logic, digest
verification, retry and backoff, local checkpoint serving, garbage collection,
and local manifest-inspection commands.

**Scope**

- GCS upload and download logic.
- Digest verification.
- Retry and backoff.
- Local checkpoint server.
- Garbage collection.
- Operator inspection commands.

**Acceptance Criteria**

- Checkpoints, proof bundles, and scores can be uploaded, downloaded, and
  verified.
- Operators can inspect local manifests and artifact state.

#### 29. Publish live Pylon state to TRN

**Summary**

Implement node-side TRN publication from live `Pylon` state.

**Why**

The roadmap requires publication of:

- `kind:39501` node records
- `kind:39511` node-originated receipts
- `kind:39520` artifact locators
- persisted event ids and `a` references

**Scope**

- Node-record builder.
- Receipt builders.
- Artifact-locator publication.
- Persistence of publication pointers.

**Acceptance Criteria**

- Live node capability state is publishable to TRN.
- Published event references are locally persisted and queryable.

#### 30. Add training-aware closeout, settlement, and reputation ingestion to
Pylon

**Summary**

Extend local operator state to ingest training closeouts and reputation.

**Why**

The roadmap requires ingestion of contribution outcomes, accepted sealed-window
closeouts, relevant `NIP-32` labels, and refusal to auto-readvertise when
reputation or build status is downgraded.

**Scope**

- Closeout ingestion.
- Settlement projection.
- Label ingestion and caching.
- Advertisement gating.

**Acceptance Criteria**

- Local operator state reflects closeouts and labels.
- Auto-readvertisement is blocked when the frozen gating rules apply.

#### 31. Add training-aware operator surfaces in Pylon

**Summary**

Add `pylon status`, training admin endpoints, doctor checks, and manual
node-record refresh support.

**Why**

The roadmap requires operator visibility without reading raw JSON by hand and
lists specific status and admin surfaces.

**Scope**

- `pylon status`
- admin HTTP endpoints
- doctor command
- manual publish and refresh commands

**Acceptance Criteria**

- Operators can inspect current run, active window, last checkpoint, validator
  queue, recent TRN events, and recent refusals and failures.

#### 32. Add Pylon test coverage for capability, config, supervision, client
idempotency, artifacts, and TRN dedupe

**Summary**

Add comprehensive node-operator coverage for the new training functionality.

**Why**

The roadmap explicitly calls for tests for capability detection, config
validation, supervision, `Nexus` idempotency, artifact handling, and TRN
publication and deduplication.

**Scope**

- Unit tests.
- Integration tests.
- Publication-dedupe tests.

**Acceptance Criteria**

- CI covers the listed areas with deterministic assertions.

### Workstream 4: Nexus Authority and Scheduler

#### 33. Implement admitted-node registry for training nodes

**Summary**

Add an admitted training-node registry keyed by node pubkey and build identity.

**Why**

The roadmap requires tracking role claims, build digest, heartbeat, capability
envelope, storage and memory posture, last successful run, and active
reputation labels, while distinguishing raw provider presence from admitted
training presence.

**Scope**

- Registry schema.
- CRUD APIs.
- Online and offline tracking.
- Distinction between generic provider and admitted training-node presence.

**Acceptance Criteria**

- The registry can answer which admitted training nodes are eligible and
  online.

#### 34. Add training run creation and scheduler loop

**Summary**

Build the live scheduler service above existing kernel objects.

**Why**

The roadmap requires canonical run creation and binding to checkpoint family,
validator policy, benchmark package set, environment package, and artifact
roots, plus node matching, lease issuance, membership tracking, and
replacement.

**Scope**

- Scheduler service.
- Run creation.
- Lease issuance.
- Matching logic.
- Replacement logic.

**Acceptance Criteria**

- `Nexus` can create and activate one real training run and assign admitted
  nodes into it.

#### 35. Implement live window planning, sealing, and reconciliation

**Summary**

Turn `ComputeAdapterTrainingWindow` into a live coordinator loop.

**Why**

The roadmap requires deterministic window ids and assignment seeds, binding
each window to run, stage, contributor set, policy, checkpoint, and dataset
slice plan, sealing windows, reconciling dispositions, and recording canonical
summaries and outcomes.

**Scope**

- Window planner.
- Assignment-seed generation.
- Seal conditions.
- Reconciliation logic.
- Persistence into kernel routes.

**Acceptance Criteria**

- Windows can move from `planned` to `active` to `sealed` to `reconciled` with
  deterministic identifiers and state.

#### 36. Automate validator challenge scheduling, leasing, and verdict
ingestion

**Summary**

Add validator automation consistent with validator policy v1.

**Why**

The roadmap requires automatic scheduling from sealed windows or sampled
contributions, leasing to admitted validators, ingesting verdicts and scores,
promoting outcomes, and supporting retry, escalation, timeout, quarantine, and
held flows.

**Scope**

- Challenge scheduler.
- Validator-lease flow.
- Verdict ingestion.
- Policy-v1 enforcement.

**Acceptance Criteria**

- Validators are scheduled automatically.
- Window acceptance is blocked on incomplete validation or held challenges.

#### 37. Implement accepted outcomes and closeout generation for sealed windows

**Summary**

Generate accepted outcomes and closeouts from reconciled sealed windows.

**Why**

The roadmap requires accepted outcomes and closeouts for rewarded, no-reward,
held, quarantined, and refused states, linked to artifacts, verdicts, and
settlement hooks.

**Scope**

- Accepted-outcome generation.
- Closeout generation.
- Linkage to locators and verdicts.
- Settlement-eligibility computation.

**Acceptance Criteria**

- Accepted sealed windows produce the correct accepted outcomes and closeouts.
- Non-accepted states produce the correct non-reward or held, quarantined, and
  refused closeouts.

#### 38. Publish authoritative TRN state from Nexus

**Summary**

Implement coordinator-side TRN publication for network, windows, receipts,
closeouts, and score locators.

**Why**

The roadmap requires publication of:

- `kind:39500` network contracts
- `kind:39510` windows
- coordinator `kind:39511` receipts
- `kind:39530` closeouts
- `kind:39520 class=score` locators
- linkage back to kernel receipt ids and object ids

**Scope**

- Event builders.
- Publication flows.
- Kernel-object linkage.
- Persistence of publication metadata.

**Acceptance Criteria**

- Canonical coordinator state is publishable and traceable back to kernel
  truth.

#### 39. Implement reputation projection and scheduler feedback

**Summary**

Project verdicts and closeouts into labels and feed them back into scheduler
preference and admission policy.

**Why**

The roadmap requires label derivation from validator-verdict quality,
closeout state, build revocation, checkpoint warnings, and scheduler feedback
based on those labels.

**Scope**

- Projection logic.
- Publication logic.
- Scheduler integration.

**Acceptance Criteria**

- Labels affect scheduling exactly per the frozen policy.

#### 40. Add Nexus run-level operator metrics and summary views

**Summary**

Expose operator metrics and run summaries for training.

**Why**

The roadmap requires `/stats` and internal views for admitted nodes online,
active runs and windows, pending validation, open challenges, checkpoint age,
artifact failures, and payout-eligible closeouts, plus a full run-summary
surface.

**Scope**

- Metrics.
- Summary endpoint or report.
- Internal operator-view integration.

**Acceptance Criteria**

- Operators can inspect live training state without raw DB or JSON inspection.

#### 41. Persist scheduler state and support replay-safe restart recovery

**Summary**

Make `Nexus` training state survive restart safely.

**Why**

The roadmap requires persistence of leases, window state, accepted
contributions, active challenges, and rebuild of TRN publication pointers
without double-publishing logically new receipts.

**Scope**

- Durable persistence.
- Restart recovery.
- Publication-pointer rebuild.
- Runtime-state recomputation from persisted truth.

**Acceptance Criteria**

- `Nexus` can restart during an active run and recover without logical
  corruption or duplicate publication.

#### 42. Add Nexus scheduler, reconciliation, timeout, closeout, reputation,
and restart-replay tests

**Summary**

Add comprehensive coordinator-side test coverage for the training scheduler.

**Why**

The roadmap explicitly lists scheduler, window planner, reconciliation,
validator timeout and retry, closeout and reputation, and restart-replay
coverage.

**Scope**

- Unit tests.
- Integration tests.
- Restart-simulation tests.

**Acceptance Criteria**

- CI covers all listed coordinator scenarios.

### Workstream 5: TRN and Nostr Implementation

#### 43. Add typed Rust support for TRN training event kinds

**Summary**

Implement typed structs, builders, parsers, and validators for all MVP TRN
event kinds.

**Why**

The roadmap requires typed support for `39500`, `39501`, `39510`, `39511`,
`39512`, `39520`, and `39530`, plus parse, validate, normalize, actor-tag,
and score-snapshot helpers.

**Scope**

- Rust structs.
- Builders.
- Parsers.
- Validators.
- Normalizers.

**Acceptance Criteria**

- Every required TRN kind round-trips correctly and validates required tags and
  fields.

#### 44. Add mapping layers from Nexus, Pylon, and validator results into TRN
events

**Summary**

Implement mapping helpers from internal models to public TRN events.

**Why**

The roadmap requires:

- kernel objects to TRN
- `Pylon` node state to node records and receipts
- validator results to verdict events and `NIP-32` labels

**Scope**

- Mapper modules.
- Validation of mapping completeness.
- Shared tagging conventions.

**Acceptance Criteria**

- Internal state can be converted to canonical TRN events without ad hoc
  formatting.

#### 45. Implement TRN-specific `NIP-32` namespace helpers and
scheduler-facing decay and gating projection

**Summary**

Add reusable `NIP-32` helpers for the frozen TRN namespaces and their
scheduler interpretation.

**Why**

The roadmap requires TRN-specific namespace helpers plus scheduler-facing
decay and gating projection code matching reputation policy v1.

**Scope**

- Namespace helpers.
- Canonical label builders.
- Subject-tagging helpers.
- Decay and gating projection helpers.

**Acceptance Criteria**

- `NIP-32` label creation and scheduler interpretation are standardized and
  reusable.

#### 46. Add relay publication retry, dedupe, and local persistence

**Summary**

Make TRN event publication reliable across relay outages and restarts.

**Why**

The roadmap requires retry, dedupe, local persistence, per-relay outcome
tracking, catch-up after outage, and local indexes that answer "what did we
publish?" without raw relay-history searches.

**Scope**

- Retry queue.
- Dedupe rules.
- Local persistence and indexes.
- Per-relay status tracking.

**Acceptance Criteria**

- Temporary relay outage does not lose publish intent.
- Publication status is queryable locally.

#### 47. Add TRN serialization, validation, helper, and cross-process emission
tests

**Summary**

Add coverage for TRN event correctness and publication behavior.

**Why**

The roadmap explicitly requires round-trip serialization tests, required-tag
validation, actor-tag and label-helper tests, and cross-process emission tests
from `Pylon` and `Nexus`.

**Scope**

- Unit tests.
- Integration tests.
- Cross-process publish tests.

**Acceptance Criteria**

- All required event kinds and helper layers are tested in CI.

### Workstream 6: Shared Artifact, Identity, and Security Plumbing

#### 48. Freeze and implement shared artifact naming, digest, id-generation,
and assignment-seed helpers

**Summary**

Build shared utilities for the cross-repo plumbing that the roadmap calls out.

**Why**

The roadmap requires one artifact naming convention, stable digest policy,
clock and id-generation policy, minimal randomness-source policy for
deterministic assignment seeds, and dataset-identity binding.

**Scope**

- Shared naming helpers.
- Digest helpers.
- ID generators.
- Assignment-seed generation.
- Dataset-identity binding helpers.

**Acceptance Criteria**

- All repos use shared policy-conformant helpers instead of ad hoc
  implementations.

#### 49. Implement node and build identity binding, revocation handling, and
receipt-redaction policy

**Summary**

Implement shared identity and security controls for admitted training nodes.

**Why**

The roadmap requires binding nodes to pubkey, release, build, and settlement
identity, handling build or node revocation, and ensuring retained receipts do
not leak raw secrets.

**Scope**

- Identity-binding model.
- Revocation handling.
- Redaction policy and enforcement.

**Acceptance Criteria**

- Revoked nodes or builds are enforceably blocked.
- Retained receipts do not contain secrets.

### Workstream 7: Defensibility Requirements

#### 50. Enforce admitted-build, signed-identity, digest, validator-evidence,
and audit-trail requirements end to end

**Summary**

Implement and verify the minimum defensibility safeguards for the admitted-node
MVP.

**Why**

The roadmap says these are the minimum safeguards required for technical
defensibility: approved releases only, signed node identity and build
attestation, artifact digests, validator evidence before closeout, rejection of
stale or digest-mismatched work, explicit refusal states, sampled replay
cadence, and persisted audit trails.

**Scope**

- End-to-end policy enforcement.
- Audit-trail persistence.
- Refusal-path checks.
- Compliance-style validation checklist.

**Acceptance Criteria**

- No accepted closeout can occur without satisfying the frozen defensibility
  requirements.

### Workstream 8: End-to-End Rehearsals

#### 51. Build and automate the full MVP rehearsal matrix

**Summary**

Automate the end-to-end rehearsal matrix and block release until it passes.

**Why**

The roadmap explicitly says not to ship without rehearsals covering
single-node flow, multi-node flow, late join, crash and recovery, lease expiry,
upload failure, validator accepted, rejected, and timeout paths,
reconciliation, TRN outage, `Nexus` restart, `Pylon` restart, and closeout and
reputation publication.

**Scope**

- Test harnesses.
- Environment setup.
- Pass and fail criteria.
- CI or release-gate integration.

**Acceptance Criteria**

- Every rehearsal in the matrix is runnable and documented.
- MVP launch is blocked until all required rehearsals pass.

### Workstream 9: Apple Silicon and Metal Expansion

#### 52. Add admitted Apple Silicon training lane under the same manifest and
policy contracts

**Summary**

Implement the first admitted Apple expansion after the first honest CUDA MVP
run.

**Why**

The roadmap includes Apple expansion as part of the same distributed-training
roadmap, with homogeneous Apple workers only, data parallel only,
backend-homogeneous windows, and no mixed CUDA-plus-Apple execution in the
same active window.

**Scope**

- Apple training lane in `Psionic`.
- Apple environment-family binding.
- Reuse of the same manifest contract.
- Reuse of the same artifact and digest policy.

**Acceptance Criteria**

- Apple nodes can run admitted homogeneous Apple windows using the same
  control-plane contracts.

#### 53. Add Apple validator replay and checkpoint and rejoin parity

**Summary**

Support Apple-native validator replay and checkpoint restore and rejoin.

**Why**

The roadmap requires Apple validator replay on Apple, Apple checkpoint,
artifact, and score emission, and rehearsal of validator accepted cases plus
checkpoint restore and rejoin.

**Scope**

- Apple validator runtime.
- Apple checkpoint support.
- Apple restore and rejoin tests.

**Acceptance Criteria**

- Apple windows can be validated on Apple.
- Apple nodes can restore and rejoin from checkpoints.

#### 54. Make Pylon advertise and supervise Apple-capable nodes

**Summary**

Extend `Pylon` capability detection and supervision to Apple-capable nodes.

**Why**

The roadmap requires Apple Silicon capability detection, admitted environment
identity, capability publication, and supervision of Apple training runtime
through the same lifecycle used by CUDA nodes.

**Scope**

- Apple capability detection.
- Node-record publication updates.
- Supervisor support for the Apple runtime.

**Acceptance Criteria**

- Apple-capable nodes advertise and run correctly under the same control plane.

#### 55. Make Nexus schedule backend-homogeneous Apple windows beside CUDA
windows

**Summary**

Extend the scheduler to support both backend families while refusing
mixed-backend windows.

**Why**

The roadmap requires backend-specific homogeneous windows, worker and validator
family matching, identical acceptance, reconciliation, closeout, and
reputation policy across families, and refusal of accidental mixed-backend
windows until a later version explicitly enables them.

**Scope**

- Backend-family scheduling constraints.
- Validator-family matching.
- Mixed-backend refusal rules.

**Acceptance Criteria**

- CUDA and Apple nodes can coexist in one network while only being scheduled
  into backend-homogeneous windows.

#### 56. Reuse TRN event shapes for Apple-capable node and window publication

**Summary**

Ensure TRN publication remains shape-compatible while making backend family and
environment refs explicit for Apple support.

**Why**

The roadmap requires reuse of the same run, node, window, receipt, verdict,
artifact, and closeout shapes and explicit backend family and environment refs
so CUDA and Apple windows are distinguishable.

**Scope**

- Node-record updates.
- Assignment and receipt updates.
- Event-validation updates.

**Acceptance Criteria**

- Public coordination state clearly distinguishes backend family without
  needing new MVP event kinds.

#### 57. Automate the Apple rehearsal matrix and block dual-backend claims
until it passes

**Summary**

Add the Apple-specific rehearsal matrix and prevent public dual-backend claims
until it passes.

**Why**

The roadmap defines the minimum Apple rehearsal matrix and explicitly says not
to widen to mixed CUDA-plus-Apple windows until portability, replay,
scheduling, and artifact-portability claims are proven.

**Scope**

- Apple single-node dry run.
- Apple multi-node rehearsal.
- Apple validator accepted case.
- Apple checkpoint restore and rejoin drill.
- Scheduler coexistence proof.

**Acceptance Criteria**

- Apple support is not claimed until all listed rehearsals pass.

### Meta and Release Planning

#### 58. Track implementation by roadmap phase and gate launch on phase
completion

**Summary**

Create a release-tracking issue tying work to the roadmap phases.

**Why**

The roadmap defines this implementation order:

- Phase 0 Contract Freeze
- Phase 1 Psionic under Pylon supervision
- Phase 2 admitted multi-node runtime
- Phase 3 Nexus windows and validator loop
- Phase 4 TRN publication and reputation
- Phase 5 dress rehearsal and launch
- Phase 6 Apple Silicon and Metal support

**Scope**

- Checklist by phase.
- Dependencies between issues.
- Launch gates.
- Post-MVP Apple gate.

**Acceptance Criteria**

- Progress is trackable by phase.
- MVP launch is blocked until Phase 5 criteria and rehearsal requirements are
  met.

#### 59. Track explicit non-blockers so they do not derail MVP delivery

**Summary**

Create a non-blockers issue to keep post-MVP scope from leaking into
delivery-critical work.

**Why**

The roadmap explicitly marks mixed CUDA-plus-Apple windows, threshold-signed
seals, Bitcoin anchoring, permissionless admission, stake and bond mechanics,
open validator markets, slashing economics, blockchain consensus or finality,
and generalized hostile-network verifiability as non-blockers for the first
honest admitted-node run.

**Scope**

- Track deferred scope.
- Prevent priority creep.
- Reference future roadmap buckets.

**Acceptance Criteria**

- Deferred items are visible but not treated as MVP blockers.
