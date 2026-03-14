# Train Off-Policy Budget Reference

> Status: canonical `#3574` rollout-admission record, updated 2026-03-14
> after landing the runnable harness in
> `scripts/release/check-psionic-train-off-policy-budget.sh`.

This document records the first bounded stale-rollout admission layer in
`psionic-train`.

## What Landed

The issue widened the `orchestrator` module with:

- `TrainingOffPolicyBudget` over revision drift, policy age, rollout age, and
  quarantine thresholds
- `RolloutAdmissionReceipt` with typed accepted exact, accepted off-policy,
  quarantined, and discarded outcomes
- `RolloutAdmissionSignal` so freshness and drift violations are machine
  readable rather than only visible in logs
- `RolloutIngestionTelemetry` on each orchestrator window for accepted,
  quarantined, and discarded rollout and token counts
- retained quarantined rollout artifacts plus discard receipts so later
  validator or operator review has typed state to inspect

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-train-off-policy-budget.sh
```

## Workload Shape

The current reference path proves one bounded but real stale-rollout control
workload:

1. plan and activate a training window with an explicit target policy revision
2. submit an exact-policy rollout and admit it directly into trainer-batch
   eligible state
3. submit an older but still budgeted rollout and admit it as bounded
   off-policy work
4. submit a rollout outside the direct-accept budget but still inside the
   quarantine budget and retain it out of trainer batches
5. submit a rollout beyond the quarantine budget and emit a discard receipt
6. preserve accepted, quarantined, and discarded counts as typed window
   telemetry

## Pass Criteria

The off-policy layer is green only if all of the following are true:

- bounded off-policy rules are explicit typed state on the orchestrator rather
  than a hidden trainer-side heuristic
- every non-error rollout submission returns a typed receipt that explains the
  outcome
- stale or mismatched rollouts are separated into quarantined versus discarded
  state
- trainer-batch assembly still draws only from accepted rollout artifacts

## Expected Signals

The current harness should prove:

- exact-policy and bounded off-policy rollouts can both be accepted
- rollout age and revision drift are surfaced as typed admission signals
- quarantine and discard outcomes increment separate token and rollout counters
- stale rollouts no longer disappear into silent filtering

## Current Limitations

This issue intentionally does not claim:

- validator-owned rollout verification bundles
- sampled adjudication or penalty policy
- train-wide durable receipt families beyond rollout admission

Worker-heartbeat, claim, and upload protocol completion now live in the
follow-on record `TRAIN_ROLLOUT_WORKER_PROTOCOL_REFERENCE.md`. This issue makes
bounded stale-rollout truth real first.
