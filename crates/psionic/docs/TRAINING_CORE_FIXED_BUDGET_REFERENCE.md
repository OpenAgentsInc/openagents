# Training Core Fixed-Budget Reference Loop

> Status: canonical `#3564` training-core record, updated 2026-03-14 after
> landing the runnable harness in
> `scripts/release/check-psionic-training-core.sh`.

This document records the first real Rust-native training-core slice inside
`psionic-train`.

It is intentionally a bounded reference path, not a fake claim that the entire
distributed train system is complete.

## What Landed

The issue landed a typed fixed-budget training loop with:

- explicit parameter-group classes
- per-group optimizer config and mutable optimizer state ownership
- explicit optimizer-state residency policy for active-step versus idle posture
- machine-legible step telemetry for gradient, update, and parameter norms
- visible inner-step, window, and cadence scheduling
- checkpoint-anchored restore lineage from `TrainingSessionState`

The current step path still uses explicit gradient batches. That remains a good
fit for a bounded reference harness, but it no longer means the gradient or
optimizer story is trainer-private: `psionic-ir::autodiff` and
`psionic-train::optimizer` now own reusable substrate underneath this loop.

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-training-core.sh
```

## Workload Shape

The current reference workload is deliberately small and deterministic:

1. define typed parameter groups using `TensorSpec` plus host-visible `f32`
   payloads
2. attach per-group optimizer config and residency policy
3. restore from the latest durable checkpoint when available, or start cold
4. apply one or more explicit gradient batches through the fixed-budget loop
5. emit typed step receipts and a final run summary

## Pass Criteria

The reference loop is green only if all of the following are true:

- at least one training run can execute a typed fixed-budget step loop inside
  `psionic-train`
- at least one parameter group uses nontrivial optimizer state rather than
  stateless updates only
- step receipts surface:
  - gradient norm
  - clipped gradient norm
  - update norm
  - parameter norm
  - window and cadence schedule identity
- optimizer-state residency transitions are explicit in the receipt rather than
  hidden in logs
- checkpoint restore lineage is typed and survives into the step receipt when
  the run bootstraps from durable checkpoint truth

## Expected Signals

The current harness should prove:

- `FixedBudgetTrainingRun` can execute to budget
- `TrainingSessionState::restore_fixed_budget_run` can anchor a run to the
  latest durable checkpoint plus manifest
- `TrainingGroupTelemetry` surfaces norm posture for each group
- `OptimizerResidencyTransition` records step-prefetch and post-step offload
  behavior when the group policy requires it
- `TrainingRunSummary` reports final parameter norms and whether the budget was
  reached

## Current Limitations

This issue intentionally does not claim:

- broad autodiff coverage across every future backend-extension and training op
- distributed optimizer or memory-sharding runtime completeness
- rollout artifacts, orchestrator control, or RL freshness policy
- full environment or eval runtime integration
- production benchmarking or hardening

The current step path is an explicit-gradient reference path over `f32`
payloads. That is enough to make trainer-step truth, optimizer ownership,
residency policy, telemetry, and checkpoint restore real without pretending the
broader train-system issues are already solved.
