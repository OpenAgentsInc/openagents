# `Psionic Architecture Explainer` Cluster Bring-Up Runbook

> Status: canonical `#3661` runbook, added 2026-03-15 after reviewing
> `docs/MVP.md`, `docs/OWNERSHIP.md`,
> `crates/psionic/docs/TRAIN_SYSTEM.md`,
> `crates/psionic/docs/CLUSTER_VALIDATION_RUNBOOK.md`,
> `crates/psionic/docs/ROADMAP_CLUSTER.md`,
> `crates/psionic/psionic-train/src/adapter_cluster.rs`, and
> `crates/psionic/psionic-train/src/adapter_reference_program.rs`.

This runbook is the operator guide for the first truthful multi-device
clustered attempt around the `Psionic architecture explainer` adapter path.

It is intentionally narrower than a product launch guide. It exists so the
first clustered attempt can be rehearsed honestly against the repo's current
implementation state.

## What This Runbook Covers

- the recommended first topology for a clustered attempt
- the safer homogeneous-lab path
- the experimental mixed Apple Metal plus NVIDIA path
- network and artifact-staging expectations
- the exact Psionic cluster, datastream, window, and worker surfaces that
  should be exercised first
- the receipts and failure signals operators should watch
- how to react when workers fall behind, disappear, or disagree on artifacts

## What This Runbook Does Not Claim

This document must not be read as a claim that live multi-device Apple adapter
training is already complete.

Current truthful state:

- single-host Apple adapter training plus `.fmadapter` runtime validation is
  real through the app-owned operator flow and the Apple FM bridge
- `psionic-train` already owns real cluster-backed adapter coordinator,
  worker-protocol, artifact-staging, validator, and promotion contracts
- the repo already has deterministic cluster and decentralized-adapter
  reference harnesses

Current non-claims:

- no shipped claim that a live `psionic-cluster` multi-node Apple training run
  is already production-ready
- no shipped claim that Apple adapter gradients are already exchanged through
  real collectives across multiple machines
- no shipped claim that an Apple plus NVIDIA mixed cluster can already train
  one Apple-valid adapter end to end

The first clustered bring-up is therefore a rehearsal and validation exercise,
not a market-ready training product.

## Three Postures To Keep Separate

### 1. Current Live Product Truth

This is the path the repo can already claim:

- one Apple host
- repo-owned dataset, environment, eval, and authority flow
- app-owned training launch or export or accept operator flow
- bridge-backed Apple runtime smoke and acceptance

### 2. Current Cluster Rehearsal Truth

This is the path this runbook focuses on:

- multiple machines on a trusted network
- `psionic-cluster` membership and topology truth
- `psionic-train` adapter contributor selection, window planning, worker
  protocol, artifact receipts, validator dispositions, and promotion receipts
- deterministic harness-backed validation before any broader claim

### 3. Later Live Cluster Ambition

This remains later work:

- real multi-machine adapter execution for the Apple lane
- collective-backed gradient exchange
- real heterogeneous Apple plus NVIDIA execution for one coherent training job
- broader operator and product surfaces above the reusable crates

## Recommended First Topologies

## Preferred Lower-Risk Topology

Use a small homogeneous Apple lab cluster first when possible:

- `1` Apple Silicon coordinator host
- `2` Apple Silicon executor hosts
- same repo commit on every machine
- same trusted local or lab network
- same cluster namespace and admission token
- same frozen dataset and benchmark digests
- one coordinator-only node for planning, artifact authority, final export, and
  runtime validation
- executor-only nodes for contribution windows

Why this is the preferred first topology:

- it keeps backend eligibility uniform for the Apple adapter lane
- it reduces receipt ambiguity when a worker is excluded or downgraded
- it keeps final `.fmadapter` export and live Apple runtime validation on the
  same platform family that will actually load the package

## Experimental Mixed Apple Metal Plus NVIDIA Topology

Use this only as an explicitly experimental bring-up path tracked alongside
`#3662`:

- `1` Apple Silicon host as coordinator, export host, and Apple runtime
  validation host
- `1` optional additional Apple Silicon host as the first Apple executor
- `1` NVIDIA Linux host as a mixed-role executor or artifact contributor
- same trusted local or lab network
- explicit worker capability declarations and stable replay identity

The honest current role split for that heterogeneous path is:

- Apple host:
  - cluster coordinator
  - final Apple runtime-validation host
  - final `.fmadapter` export and attach host
  - optional Apple executor
- NVIDIA host:
  - open-backend executor for the non-Apple reference lane
  - mixed-hardware artifact and receipt contributor
  - transport, staging, churn, and validator-path stress participant

What this mixed path is good for today:

- proving that cluster membership, contributor selection, datastream staging,
  replay identity, validator receipts, and failure handling remain truthful on
  mixed hardware

What this mixed path is not yet good for today:

- claiming one NVIDIA worker is already contributing Apple-valid adapter
  gradients into the final Apple package

## Hardware And Host Prerequisites

Every participating host should have:

- the same checked-out repo commit
- Rust toolchain sufficient to run the cited harnesses
- healthy local disk for dataset and artifact staging
- stable clocks good enough for receipt ordering
- a low-jitter trusted network path to every other host

Additional Apple host requirements:

- Apple Silicon
- Apple Intelligence enabled
- working Swift toolchain
- successful local bridge build and health check:

```bash
cd swift/foundation-bridge && ./build.sh
./bin/foundation-bridge
curl -s http://127.0.0.1:11435/health
```

Additional NVIDIA host requirements:

- supported Linux plus usable NVIDIA driver or CUDA runtime for the open lane
- no claim that the NVIDIA host can validate or attach Apple adapters

## Frozen Inputs Before Any Cluster Attempt

Do not start a clustered attempt until the following are frozen and copied into
operator notes:

- target contract: `Psionic architecture explainer`
- dataset identity and version
- train, held-out, and benchmark split digests
- environment package key and version
- benchmark package key
- policy family and checkpoint family identifiers
- expected Apple base-model compatibility anchor
- cluster namespace and admission token

Why this matters:

- cluster receipts are only comparable if every host is participating in the
  same run definition
- mixed or stale manifests turn worker disagreement into operator confusion

## Network And Artifact-Staging Expectations

The first clustered attempt should assume:

- trusted LAN or trusted lab network only
- no internet-wide discovery posture
- explicit cluster namespace and admission configuration
- explicit artifact staging and replay identity
- one coordinator-visible source of truth for manifests, receipts, and final
  promotion state

Operator rules:

- do not stage critical artifacts with ad hoc `scp` or hand-copied temporary
  files once the attempt begins
- do use Psionic datastream or artifact-manifest surfaces as the durable record
  for dataset slices, contribution uploads, and promoted checkpoints
- keep the Apple `.fmadapter` export on the Apple coordinator host that will
  also perform runtime validation

## First Bring-Up Sequence

### 1. Validate Cluster Transport And Ordered-State Truth First

Run the canonical cluster validation gates from the repo root before any
training attempt:

```bash
cargo test -p psionic-cluster --test local_cluster_transport
cargo test -p psionic-cluster --test cluster_validation_matrix
cargo test -p psionic-cluster
```

If these fail, stop. Do not reinterpret a transport or admission failure as a
training issue.

### 2. Validate Adapter Cluster Selection And Churn Handling

Run the adapter cluster harnesses:

```bash
cargo test -p psionic-train adapter_cluster -- --nocapture
scripts/release/check-psionic-decentralized-adapter-reference-program.sh
```

What these prove before live cluster work:

- contributor eligibility is derived from cluster membership plus telemetry
- deterministic worker ranking and selection are stable
- window replanning under churn is explicit
- stale uploads, manifest corruption, and replay-missing cases are rejected

### 3. Build The First Coordinator-Only Operator Posture

The coordinator host should be treated as the control and truth anchor for the
first attempt:

- coordinator-only cluster role when possible
- final export and runtime validation host for the Apple lane
- durable collector of membership, plan, artifact, security, validator, and
  promotion receipts

The coordinator should not also be the only required executor unless hardware
constraints force it.

### 4. Admit Executors Conservatively

For the first attempt, select executors only when they satisfy the policy
reflected in `AdapterContributorCapabilityPolicy`:

- matching backend label
- sufficient free memory
- visible accelerator when required
- stable node posture
- non-degraded backend unless degraded mode is explicitly allowed

The current default Apple policy inside `adapter_cluster.rs` expects the
backend label:

- `apple.foundation_models.adapter_train`

Do not manually override contributor selection in operator notes and then call
the result "cluster-backed truth." If the policy needs to change, change the
policy and capture the new receipts.

### 5. Rehearse One Two-Window Adapter Cycle Before Widening Scope

The first truthful clustered attempt should look like the existing
decentralized-adapter reference program:

- observe cluster state
- produce a membership receipt
- plan one window
- activate workers
- collect progress and upload receipts
- validate or quarantine contributions
- seal and aggregate
- re-observe cluster state after churn
- plan the next window
- repeat

Only after that sequence is stable should operators try to widen worker count,
artifact volume, or benchmark scope.

## Receipts And Telemetry To Watch

At minimum, operators should surface and preserve:

- `AdapterClusterMembershipReceipt`
- `AdapterClusterWindowPlanReceipt`
- `AdapterContributionArtifactReceipt`
- `AdapterContributionSecurityReceipt`
- `AdapterWindowCheckpointReceipt`
- `AdapterPolicyPromotionReceipt`
- benchmark aggregation summaries
- held-out eval summaries
- Apple bridge runtime-smoke receipts on the coordinator host

Operator interpretation:

- membership receipt answers "who was eligible and why"
- window-plan receipt answers "who was selected and for which slices"
- artifact and security receipts answer "what was uploaded and whether it can
  be trusted"
- promotion receipts answer "whether the window actually advanced policy
  state"

## What To Do When Workers Fall Behind Or Disappear

Use the typed contributor posture instead of ad hoc judgment.

If a worker is:

- `joining`
  - keep it out of selected contributors for the current window
- `draining`
  - do not assign fresh work; finish or explicitly replace its current window
    posture
- `offline`
  - treat any incomplete contribution as replay-required or incomplete rather
    than silently accepted

If a worker shows:

- `backend_unavailable`
  - remove it from contributor selection immediately
- `backend_degraded`
  - keep it excluded unless the policy explicitly allows degraded backends
- `insufficient_free_memory`
  - move it to standby instead of forcing selection
- `node_unstable`
  - prefer reselection over hoping the node stabilizes mid-window

The first operator reflex should be:

- capture the membership receipt
- capture the latest window-plan and upload receipts
- re-run selection against current cluster truth
- prefer explicit replanning over manual patch-up

## What To Do When Artifacts Disagree

Treat artifact disagreement as a validator and provenance problem, not as a
mere networking nuisance.

Operator rules:

- never merge conflicting artifacts by hand
- preserve all competing manifests and checksums
- require validator disposition to remain explicit: accepted, quarantined,
  rejected, or replay-required
- do not promote policy if artifact lineage or replay proof is ambiguous

This is especially important on the mixed Apple plus NVIDIA path, where mixed
hardware is useful for stressing staging and provenance, but not yet a basis
for claiming one unified Apple-valid training result.

## Honest Success Criteria For The First Clustered Attempt

Call the first clustered attempt successful only if all of the following are
true:

- cluster transport and admission validation passed
- adapter cluster harness and reference-program validation passed
- multiple devices participated in one explicit two-window rehearsal
- churn or disagreement produced explicit receipts rather than silent success
- the Apple coordinator host still owns final runtime-validation truth for the
  Apple lane
- the final report states clearly whether the attempt was:
  - a cluster rehearsal only
  - an open-backend mixed-hardware proof
  - or a later true live clustered Apple execution attempt

## Escalation Rules

Stop the clustered attempt and fall back to single-host truth when:

- cluster admission or ordered-state checks fail
- contributor eligibility is not reproducible from receipts
- artifacts disagree and validator disposition is unclear
- the Apple coordinator host cannot perform final bridge validation
- operators are tempted to claim mixed Apple plus NVIDIA contribution to an
  Apple-valid package without machine-checkable evidence

## Related References

- `crates/psionic/docs/TRAIN_SYSTEM.md`
- `crates/psionic/docs/CLUSTER_VALIDATION_RUNBOOK.md`
- `crates/psionic/docs/ROADMAP_CLUSTER.md`
- `scripts/release/check-psionic-decentralized-adapter-reference-program.sh`
- `crates/psionic/psionic-train/src/adapter_cluster.rs`
- `crates/psionic/psionic-train/src/adapter_reference_program.rs`
