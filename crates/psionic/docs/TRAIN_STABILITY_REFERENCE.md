# Train Stability Reference

> Status: canonical `#3582` train-stability record, updated 2026-03-14
> after landing the runnable harness in
> `scripts/release/check-psionic-train-stability.sh`.

This document records the first Psionic-native train safety controller.

## What Landed

The issue widened `psionic-train` with:

- aggregated instability telemetry derived from trainer-step receipts and
  rollout-ingestion telemetry
- digest-bound instability policy with threshold rules and risky-optimization
  rules
- explicit signal receipts plus risky-optimization receipts
- final `continue`, `quarantine`, or `halt` verdicts that higher layers can
  consume directly

This is the first typed safety layer over the train substrate. It does not yet
claim operator UI or authority publication.

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-train-stability.sh
```

## Reference Flow

The current reference path proves:

1. derive instability telemetry from existing trainer-step and rollout-window
   surfaces
2. attach extra operational counters such as topology churn and failure rates
3. evaluate the telemetry against a digest-bound policy
4. evaluate requested risky optimizations against explicit policy rules
5. emit a final typed verdict

## Pass Criteria

The stability layer is green only if all of the following are true:

- telemetry is derived from existing train receipts rather than hidden in logs
- stale-rollout drop rate, gradient norms, and clipping ratios are available to
  policy
- risky runtime optimizations are explicit policy inputs
- verdicts are machine-legible and severity-ordered

## Expected Signals

The current harness should prove:

- rollout-drop pressure can trigger quarantine without halting the trainer
- high gradient norms can trigger halt
- ungated risky optimizations default to a blocking verdict
- verdicts keep both signal receipts and optimization receipts

## Current Limitations

This issue intentionally does not claim:

- desktop/operator presentation of safety verdicts
- automatic remediation loops after halt or quarantine
- authority publication of train safety receipts
- adaptive thresholds learned from historical runs
