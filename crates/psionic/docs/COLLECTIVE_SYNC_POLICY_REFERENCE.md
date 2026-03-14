# Collective Sync Policy Reference

> Status: canonical `#3571` collective-sync record, updated 2026-03-14 after
> landing the runnable harness in
> `scripts/release/check-psionic-collective-sync.sh`.

This document records the first explicit Psionic collective sync planner.

## What Landed

The issue extended `psionic-collectives` with:

- `CollectiveTransportFeedback` for mesh-wide bandwidth, latency, and stream
  pressure observations
- `CollectiveSyncCadencePolicy` for healthy versus degraded global-sync
  intervals, local/global quantization posture, and transport thresholds
- `CollectiveSyncExecutionPlan` and `CollectiveSyncStage` for explicit
  local-group versus full-mesh sync staging
- `CollectiveSyncCadenceReceipt` for machine-legible cadence, next-global-step,
  selected quantization, degraded-transport posture, and stable receipt digest
- `CollectiveReplanTrigger` for mesh-revision, bandwidth, latency, stream, and
  quantization fallback reasons

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-collective-sync.sh
```

## Workload Shape

The current reference path proves one bounded but real sync-control workload:

1. observe an elastic mesh
2. record benchmark-approved quantization for a collective path
3. record mesh-wide transport feedback
4. decide whether transport is healthy or degraded under explicit policy
5. plan local subgroup sync, full-mesh sync, or both for one trainer step
6. emit a typed cadence receipt that preserves the replan reasons

## Pass Criteria

The collective-sync layer is green only if all of the following are true:

- local-group and global-mesh sync are explicit stages, not hidden optimizer
  behavior
- transport feedback changes cadence under typed policy rather than ad hoc
  conditionals
- replan triggers are machine-legible and stable enough for later operator or
  validator inspection
- quantized global sync is still benchmark-gated even when the cadence planner
  is making fallback decisions
- mesh revision changes can force a visible replan on the next sync step

## Expected Signals

The current harness should prove:

- degraded bandwidth or latency can defer the next global sync and run only
  local subgroup stages
- cadence interval expiry can trigger a local-then-global plan
- mesh revision changes are surfaced as typed replan triggers
- missing quantization approval is recorded as a visible fallback reason rather
  than disappearing into a silent downgrade

## Current Limitations

This issue intentionally does not claim:

- distributed optimizer state integration
- parameter-shard accounting or optimizer-shard cadence
- NIC-aware topology placement beyond the current subgroup heuristic
- transport observations imported directly from `psionic-net` session snapshots
- validator-owned acceptance thresholds for collective throughput

Those remain later issues. This issue makes local/global sync cadence,
transport-feedback replanning, and quantized policy surfaces real first.
