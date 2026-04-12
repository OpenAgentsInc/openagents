# Transcript 222 Training Incident Taxonomy

Status: active  
Date: 2026-04-12

This document freezes the operator incident taxonomy for the Transcript 222
launch-hardening program.

Its job is simple: when a launch SLO breaks, operators should not improvise the
incident name, severity, or containment path. The taxonomy below defines the
classes that block rollout expansion before crowd runs.

This taxonomy complements
[`transcript-222-training-launch-slos.md`](./transcript-222-training-launch-slos.md).

## Severity Model

### Severity 1

- stop all new leases immediately
- pause public expansion claims
- contain across all rollout channels until the fault is understood

### Severity 2

- pause the affected work class, backend family, release, or cohort
- do not widen rollout
- continue only on unaffected scopes if the blast radius is genuinely bounded

### Severity 3

- degraded behavior inside an active canary or beta scope
- rollout stays frozen for the affected scope until the issue is resolved

### Severity 4

- operator attention required, but no current evidence that assignment,
  validation, closeout, payout, or public truth is compromised

## Required Incident Packet

Every training incident should capture at least:

- incident class
- severity
- detected-at timestamp
- rollout revision
- affected training run ids and window ids
- affected work classes, backend families, release ids, or build digests
- primary evidence links
- containment actions taken
- whether public claims are frozen

## Incident Classes

### `train.control.relay_degraded`

Default severity: `1`

Trigger:

- `/healthz`, `/api/stats`, `/api/training/rollout`, or `/v1/treasury/status`
  breaches the frozen launch budget
- deploy verification cannot capture one of the required authoritative
  snapshots

Primary evidence:

- deploy receipt gate failures
- local-origin probe latency samples

Immediate containment:

1. set `pause_new_leases=true`
2. stop rollout widening
3. retain the failing deploy receipt and VM-local logs

### `train.rollout.policy_misconfigured`

Default severity: `2`

Trigger:

- rollout revision does not match the intended operator change
- wrong cohort receives traffic
- blocked release/build breakers are missing or ignored
- rollout widening proceeds while the policy is paused

Primary evidence:

- `/api/training/rollout`
- deploy receipt rollout snapshot

Immediate containment:

1. pause new leases
2. restore the intended rollout policy
3. verify cohort matching against admitted-node reality before resuming

### `train.scheduler.assignment_stall`

Default severity: `2`

Trigger:

- assignment success falls below the frozen floor
- eligible nodes repeatedly hit lease starvation or expiry
- active runs stop issuing successful leases across consecutive lease windows

Primary evidence:

- scheduler assignment state
- retained assignment-failure receipts
- admitted-node versus assigned-node deltas

Immediate containment:

1. pause the affected work class or cohort
2. preserve assignment and failure receipts
3. confirm whether the stall is rollout-gate, capability, or scheduler state

### `train.artifact.resolver_or_materialization_failure`

Default severity: `2`

Trigger:

- resolver or signed-access latency breaches the frozen budget
- signed URL issuance fails for a live cohort
- leased assignments cannot materialize required artifacts consistently

Primary evidence:

- artifact resolver and signed-access endpoint responses
- retained runtime fetch/materialization failure receipts

Immediate containment:

1. pause the affected work class or release
2. preserve resolver payloads and runtime failure evidence
3. verify bucket layout, object identity, and signing credentials before resume

### `train.validation.backlog`

Default severity: `2`

Trigger:

- sealed windows fail validator completion inside the allowed challenge budget
- queued/open validator challenges grow without draining
- windows remain pending validation longer than `2x` the configured challenge
  window

Primary evidence:

- per-window validator audit
- validator challenge queue/open counters

Immediate containment:

1. freeze rollout widening
2. preserve window ids, challenge ids, and validator pool references
3. increase validation focus before allowing new windows to pile up

### `train.closeout.stall`

Default severity: `2`

Trigger:

- sealed windows miss the frozen closeout-latency budget
- payout-eligible windows remain non-terminal past `2x` their closeout budget

Primary evidence:

- `sealed_at_ms`
- accepted outcome `accepted_at_ms`
- closeout status on retained outcomes

Immediate containment:

1. pause the affected training run or work class
2. preserve closeout outcome evidence and validator audit state
3. do not widen rollout until terminal closeout recovers

### `train.payout.backlog_or_reconciliation`

Default severity: `1` for accepted-work backlog, `2` otherwise

Trigger:

- payout-eligible accepted work misses the frozen payout-latency budget
- accepted-work payouts enter `failed`, `skipped`, or
  `attention_required` reconciliation state
- payout backlog grows while rollout expansion remains enabled

Primary evidence:

- `/v1/treasury/status`
- training payout ledger summary
- accepted outcome payout metadata

Immediate containment:

1. stop rollout widening immediately
2. preserve treasury status snapshot and payout ledger state
3. reconcile payout targets, queue state, and wallet evidence before resume

### `train.public_state.stale_or_drifted`

Default severity: `2`

Trigger:

- authoritative `/api/stats` freshness exceeds the frozen budget
- public mirrored snapshot age exceeds the frozen budget
- public assigned/accepted/payout counters drift from authoritative Nexus truth

Primary evidence:

- `/api/stats`
- rendered public snapshot timestamp
- public-site snapshot payload consuming the Nexus broadcast/state path

Immediate containment:

1. freeze public claim widening
2. preserve authoritative and mirrored snapshots
3. correct the stale or drifted consumer before expanding the audience

### `train.identity_or_replay_integrity`

Default severity: `2`

Trigger:

- duplicate ids, stale manifests, clock skew, or replay-suppression failures
  threaten closeout or payout truth

Primary evidence:

- retained receipts
- manifest digests
- idempotency/identity records

Immediate containment:

1. pause the affected release or build
2. preserve the conflicting receipts and manifests
3. verify time, identity, and replay controls before resuming

This class stays aligned with the dedicated reproducibility-hardening issue. It
is still part of launch ops because any unresolved integrity drift invalidates
accepted-work and payout claims.

## Exit Criteria

An incident may be closed only when:

- the triggering evidence is retained
- the containment action is recorded
- the affected SLO is back inside its frozen budget
- rollout pause and blocker state has been reviewed explicitly
- public claims are unfrozen only if authoritative and mirrored truth agree

## Relationship to the Next Issue

This taxonomy defines the incident names and containment rules. The next
launch-hardening issue is responsible for dashboards, alert routing, and the
single operator surface that renders these incidents live.
