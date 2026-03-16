# ARC-ML Practice Log
Status: bounded implementation log
Date: 2026-03-16

## Why This Exists

Real ARC-AGI-3 dataset access is still absent in the retained repo, but the
next honest ARC step after the interactive runtime substrate is not "wait
forever". It is to practice the evaluator-first ARC-ML layer against synthetic
ARC-AGI-3-style attempts while keeping all contracts explicit.

This log records that bounded step.

## What Landed

- `crates/arc/ml` now exists as package `arc-ml`.
- The first landed slice is evaluator-first, not training-first.
- `arc-ml` can evaluate synthetic ARC-AGI-3-style attempt suites built from
  typed `ArcRecording` values.
- Each attempt is scored through `arc-benchmark`, so score truth stays in the
  benchmark crate rather than drifting into `arc-ml`.
- Each evaluated attempt retains the full `ArcInteractiveRunReport`.
- Suite reports aggregate `pass@k` over the scored attempts.

## What Counts As Synthetic Practice

The current practice corpus is rooted in owned engine fixtures, not in real ARC
Prize held-out data.

Current fixture:

- `crates/arc/ml/fixtures/interactive_practice_suite.json`

The fixture describes:

- one mixed case with one successful completion and two failed attempts
- one failure-only case
- requested `k` values of `1`, `2`, and `4`

The tests replay those action traces through `arc-engine`, then hand the
resulting `ArcRecording` values to `arc-ml` for scoring and aggregation.

## Verification

The bounded evaluator-first checks are:

```bash
cargo test -p arc-ml interactive_practice_suite_scores_synthetic_attempts_and_aggregates_pass_at_k -- --nocapture
cargo test -p arc-ml pass_at_k_estimator_matches_reference_probability -- --nocapture
cargo test -p arc-ml interactive_practice_suite_refuses_attempt_task_mismatch -- --nocapture
```

## What This Does Not Claim

This does not claim:

- real ARC-AGI-3 dataset access
- learned-model parity
- ARC-ML training parity
- HRM readiness

Those remain later Epic 5 work and stay gated on the Psionic roadmap items
called out in `crates/arc/docs/ROADMAP.md`.
