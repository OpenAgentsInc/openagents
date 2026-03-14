# Train Scheduling and Accounting Reference

> Status: canonical `PSI-286` / `#3591` reference record, updated 2026-03-14
> after landing the train scheduling/accounting layer in
> `crates/psionic/psionic-train/src/scheduling_accounting.rs`.

This document records the first explicit scheduling, preemption, and
cost-attribution contract for train workloads inside Psionic.

## Canonical Runner

Run the scheduling/accounting harness from the repo root:

```bash
scripts/release/check-psionic-train-scheduling-accounting.sh
```

## What Landed

`psionic-train` now owns a controller that admits, queues, preempts, completes,
and accounts for trainer, rollout, eval, sandbox, and validator workloads over
typed runtime work items.

The new typed surfaces include:

- `TrainBudgetCap`
- `TrainQueueClass`
- `TrainQueuePolicy`
- `TrainPreemptionMode`
- `TrainRoleCostRate`
- `TrainScheduledWorkload`
- `TrainAdmissionReceipt`
- `TrainCompletionReceipt`
- `TrainSchedulingAccountingSnapshot`
- `TrainSchedulingAccountingController`

## What The Contract Makes Explicit

The scheduling/accounting layer now makes these train-side operator seams
machine-legible:

- global budget caps for active work units, bytes, and cost
- explicit queue priorities and preemption posture
- runtime dispatch plans attached to workload admission
- role-aware cost rates across trainer, rollout, eval, sandbox, and validator
  work
- environment-scoped cost attribution
- validator-scoped cost attribution
- queue draining after completion instead of hidden retry loops

## Pass Criteria

The scheduling/accounting layer is green only if all of the following remain
true:

- higher-priority validator or eval work can preempt lower-priority sandbox or
  background work when the active budget is saturated
- queued workloads become active only through typed completion-driven state
  transitions
- environment and validator summaries reflect completed actual cost rather than
  only admission-time estimates
- constructors for trainer, rollout, eval, sandbox, and validator workloads
  preserve role and provenance truth

## Current Limits

This issue does not claim that train economics are complete. It does not yet
implement:

- market pricing or settlement authority
- cluster-wide multi-host budget arbitration
- capital-aware validator procurement
- benchmark-driven performance acceptance thresholds

What it does do is give Psionic one Rust-owned scheduling/accounting surface
for budget caps, queue classes, preemption, queue draining, environment cost
attribution, and validator cost visibility.
