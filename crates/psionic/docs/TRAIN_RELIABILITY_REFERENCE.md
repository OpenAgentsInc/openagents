# Train Reliability Reference

> Status: canonical `PSI-287` / `#3592` reference record, updated 2026-03-14
> after landing the train reliability suite in
> `crates/psionic/psionic-train/src/reliability.rs`.

This document records the first explicit chaos and failure-injection suite for
train workloads inside Psionic.

## Canonical Runner

Run the reliability harness from the repo root:

```bash
scripts/release/check-psionic-train-reliability.sh
```

## What Landed

`psionic-train` now owns a reliability harness that runs typed scenario drills
over checkpoint recovery, collectives, rollout admission, validator verdicts,
and orchestrator restart.

The new typed surfaces include:

- `TrainReliabilityScenarioKind`
- `TrainReliabilityScenarioSpec`
- `TrainReliabilitySignal`
- `TrainReliabilityScenarioReceipt`
- `TrainReliabilitySuiteReceipt`
- `TrainReliabilityHarness`

## What The Contract Makes Explicit

The reliability suite now makes these train-side failure classes
machine-legible:

- topology churn under checkpoint-backed elastic recovery
- degraded transport under collective cadence fallback
- stale-weight flood containment at rollout admission
- checkpoint corruption or stale-pointer fallback
- validator stress with mixed accepted, normalized, and rejected outcomes
- orchestrator state restore after restart

## Pass Criteria

The reliability suite is green only if all of the following remain true:

- topology churn emits a recovery plan rather than dropping state on the floor
- degraded transport moves collective cadence onto a safe fallback path
- stale flood scenarios do not produce accepted rollouts
- checkpoint corruption falls back to a durable manifest source
- validator stress produces mixed outcome classes instead of collapsing into one
- orchestrator state can round-trip and resume trainer-batch control after restart

## Current Limits

This issue does not claim that train reliability is complete. It does not yet
implement:

- cluster-wide fault injection across real network transports
- long-running soak tests over external storage or validator services
- benchmark-driven pass/fail thresholds
- production incident automation or remediation playbooks

What it does do is give Psionic one Rust-owned reliability suite for topology,
checkpoint, validator, and orchestrator failure classes, with typed receipts
for every scenario in the reference program.
