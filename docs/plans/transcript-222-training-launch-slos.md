# Transcript 222 Training Launch SLOs

Status: active  
Date: 2026-04-12

This document freezes the launch SLO package for the Transcript 222
launch-hardening program.

It does not build new dashboards. That work is tracked separately. This
document defines the metrics, thresholds, and stop-ship rules that existing
Nexus, treasury, public-stats, and runtime surfaces must satisfy before crowd
expansion.

## Scope

This SLO package governs:

- Nexus control-plane health for training rollout and stats
- assignment issuance and assignment materialization
- validator completion and closeout timeliness
- accepted-work payout timeliness and ledger cleanliness
- authoritative and public-facing stats freshness
- rollout-policy integrity during canary and broad expansion

This document is intentionally narrower than a full observability plan.
Dashboard layout, alert routing, and public drilldowns belong to the next
launch-hardening issue.

## Authoritative Measurement Surfaces

These are the current source-of-truth surfaces for launch SLOs:

1. `scripts/deploy/nexus/04-verify-gates.sh`
   - authoritative deploy receipt for `/healthz`, `/api/stats`,
     `/api/training/rollout`, `/v1/treasury/status`, tail latency, treasury
     freshness, and rollout-policy capture
2. `apps/nexus-control/src/lib.rs`
   - `training_operator_summary_snapshot`
   - `training_public_stats_snapshot`
   - `training_rollout_policy_snapshot`
   - accepted closeout and payout-projection metadata on training outcomes
3. `/api/stats`
   - authoritative public training summary for assigned, accepted, payout, run,
     and checkpoint-age truth
4. `/api/training/rollout`
   - authoritative rollout revision, pause state, gates, cohorts, and blocked
     release/build breakers
5. `/api/kernel/compute/training/artifacts/:artifact_id/resolver`
   and `/api/kernel/compute/training/artifacts/:artifact_id/signed-access`
   - authoritative artifact resolver and signed-URL issuance surfaces
6. `/v1/treasury/status`
   - authoritative payout cadence, pending/failed/skipped state, and training
     payout reconciliation status
7. Existing retained runtime receipts and failure uploads from the zero-touch
   assignment/materialization path
   - authoritative source for assignment materialization and artifact-fetch
     failures until the dedicated dashboard issue lands

Where a metric is not yet emitted as one first-class dashboard counter, the SLO
still applies. Operators should derive the measurement from the retained
receipts and authoritative endpoints above rather than inventing a second path.

## SLO Package

### 1. Control-Plane Availability and Latency

Measurement:

- `/healthz`
- `/api/stats`
- `/api/training/rollout`
- `/v1/treasury/status`

Launch target:

- `/healthz` p95 <= `1000 ms`, p99 <= `2000 ms`
- `/api/stats` p95 <= `1000 ms`, p99 <= `2000 ms`
- `/api/training/rollout` request latency <= `1000 ms`
- `/v1/treasury/status` request latency <= `1000 ms`

Current implementation note:

- these are already the default hard gates in
  `scripts/deploy/nexus/04-verify-gates.sh`

Breach policy:

- any hard breach blocks rollout widening immediately
- any sustained breach over two consecutive verification windows is
  `train.control.relay_degraded`

### 2. Assignment Success

Definition:

- an assignment succeeds when an admitted, rollout-eligible node can obtain a
  valid lease for its work class and the lease does not expire unused

Authoritative source:

- scheduler lease claims and assignment state inside `nexus-control`
- retained assignment and failure receipts from the automatic Pylon path

Launch target:

- canary: rolling 15-minute assignment success >= `99%`
- broad: rolling 15-minute assignment success >= `97%`
- no active run may go two consecutive lease windows without a successful new
  lease for an otherwise eligible cohort

Failure budget notes:

- rollout mismatch, blocked release/build, and explicit pause states are not
  counted as healthy success
- expired leases count against the budget unless the rollout was explicitly
  paused first

### 3. Artifact Resolver Latency and Assignment Materialization Success

Definition:

- resolver latency covers the logical artifact lookup and signed-access
  issuance path
- materialization success covers whether a leased assignment fetched its
  declared artifacts and reached runtime start without an artifact-side failure

Authoritative source:

- `/api/kernel/compute/training/artifacts/:artifact_id/resolver`
- `/api/kernel/compute/training/artifacts/:artifact_id/signed-access`
- retained runtime receipt uploads and assignment-failure reports

Launch target:

- resolver lookup latency p95 <= `750 ms`
- signed-access issuance latency p95 <= `1000 ms`
- canary assignment materialization success >= `99%`
- broad assignment materialization success >= `98%`
- no cohort-wide signing failure or bucket-path misconfiguration may remain
  open during rollout expansion

Current implementation note:

- `training_artifact_failures_open` already exists on the public training state
  surface, but it is not yet backed by a real backlog counter; until the
  dashboard issue lands, operators should use retained assignment/runtime
  failure receipts as the authoritative source

### 4. Validator Completion

Definition:

- a window completes validation when it reaches its required distinct validator
  count before the validator challenge budget is exhausted

Authoritative source:

- per-window validator audit metadata in `nexus-control`
- `validator_challenges_open`
- `validator_challenges_queued`
- `required_validator_count`
- `distinct_validator_count`
- `last_validator_finalized_at_ms`

Launch target:

- canary: `99%` of sealed windows satisfy validator requirements within one
  challenge window
- broad: `95%` of sealed windows satisfy validator requirements within one
  challenge window
- no window may remain in pending validation for more than `2x` its configured
  challenge budget without an active incident

### 5. Closeout Latency

Definition:

- closeout latency is the interval from `sealed_at_ms` on a training window to
  the accepted terminal closeout timestamp `accepted_at_ms`

Authoritative source:

- training window metadata (`sealed_at_ms`)
- accepted outcome metadata (`accepted_at_ms`, `closeout_status`)

Launch target:

- p95 closeout latency <= `max(challenge_window_ms + 300000, 900000)`
- no payout-eligible sealed window may exceed `2x` its closeout budget without
  an active incident and rollout pause on the affected scope

Interpretation:

- held, quarantined, and refused outcomes still count as terminal closeout
  states for latency measurement
- only payout-eligible windows participate in the payout-latency SLO below

### 6. Payout Latency and Ledger Cleanliness

Definition:

- dispatch latency measures time from payout-eligible closeout acceptance to the
  queued/dispatched treasury payout record
- confirmation latency measures time from accepted closeout to wallet-confirmed
  payout evidence

Authoritative source:

- `/v1/treasury/status`
- training payout ledger state in treasury
- accepted outcome timestamps from `nexus-control`

Launch target:

- dispatch latency p95 <= `max(5 * nexus_treasury_payout_interval_seconds, 300 seconds)`
- no payout-eligible accepted-work payout may sit in
  `attention_required` reconciliation status during active rollout expansion
- rolling average confirmation latency <=
  `max(15 * nexus_treasury_payout_interval_seconds, 1800 seconds)`
- failed or skipped accepted-work payouts must remain `0` over the active
  canary cohort unless the rollout is paused and the incident is open

Implementation note:

- this follows the same shape as the existing desktop earnings scoreboard
  metrics: success ratio plus confirmation latency, rather than inventing a
  second payout truth contract

### 7. Public-State Freshness and Drift

Definition:

- authoritative freshness is the age of the current Nexus-generated public
  training snapshot
- public drift is any unexplained mismatch between authoritative Nexus counters
  and the public-facing rendered snapshot

Authoritative source:

- `/api/stats`
- `training_public_state`
- generated timestamp on the authoritative snapshot
- the public-site cached snapshot that consumes the same broadcast/state path

Launch target:

- authoritative Nexus public-stats freshness <= `15000 ms`
- public mirrored snapshot freshness <= `120000 ms`
- no unresolved drift is permitted for:
  - assigned contributors
  - accepted contributors
  - model-progress contributors
  - payout-class totals
  - active run/window identity

Current implementation note:

- the authoritative freshness budget is already enforced in deploy verification
- public consumer drift detection is frozen here as a launch requirement and is
  operationalized in the dashboard/alerting issue rather than here

### 8. Rollout-Policy Integrity

Definition:

- the active training rollout policy must remain observable and auditable while
  expansion is in progress

Authoritative source:

- `/api/training/rollout`
- deploy receipts that capture the rollout-policy snapshot

Launch target:

- every deploy receipt must capture the active rollout revision
- rollout widening is forbidden while `pause_new_leases=true`
- rollout widening is forbidden while the active release id or build digest is
  on the blocked list for the targeted cohort
- canary and beta cohort definitions must resolve to real admitted nodes before
  expansion proceeds

## Go/No-Go Policy

Before `#4318` small-cohort canary crowd rehearsal:

- all hard latency and freshness gates are green for 24 continuous hours
- no Severity 1 or Severity 2 training incident is open
- payout reconciliation is not in `attention_required`
- the active rollout policy matches the intended canary cohort and release/build
  posture

Before `#4319` widened crowd rehearsal:

- all of the above remain true
- validator completion and closeout latency stay inside budget across the canary
  cohort
- public-state drift remains resolved across the authoritative and mirrored
  stats paths

## Relationship to the Next Issue

This document defines the thresholds. It does not decide where the charts live.

The next launch-hardening issue is responsible for:

- dashboard packets and alert views
- alert routing and drilldown
- public-state drift visualizations
- resolver backlog charts

Those surfaces must use this document as their threshold source rather than
inventing a second SLO package.
