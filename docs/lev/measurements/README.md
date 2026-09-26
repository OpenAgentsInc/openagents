# Lev measurement records

Each record describes an Apple runtime, base signature, estimator, adapter,
suite, and observation date. Read the [current disposition](../disposition.md)
for workload admission and the [roadmap reconciliation](../roadmap.md) for issue
status. A trained adapter or completed experiment does not grant calibrated
probabilities on a new workload.

## Behavior, variance, and calibration

| Record | Scope |
| --- | --- |
| [Base behavior](2026-09-19-behavior.md) | Greedy behavior, sampling, guardrails, and typed output on the recorded device. |
| [Seed variance](2026-09-19-seed-variance.md) | Eight-sample estimates across disjoint seed blocks. |
| [Calibration variance](2026-09-19-calibration-variance.md) | Metric spread from retained draws, without another model call. |
| [Option-order variance](2026-09-19-flip-rate-variance.md) | Flip-rate noise and withdrawn improvements below the measured floor. |
| [Live admission](2026-09-20-admission-live.md) | Actual FoundationModels startup probe and retained build failure. |

## Workload and adapter observations

| Record | Scope |
| --- | --- |
| [Support v1](2026-09-19-suite-scores.md) | The original 52 authored items with separate calibration and evaluation halves. |
| [Support v2](2026-09-19-suite-v2-scores.md) | Larger authored suite and label-agreement caveat. |
| [Choice adapter](2026-09-19-adapter-v1.md) | First adapter's support-suite results, not unseen-workload evidence. |
| [Band adapter](2026-09-19-adapter-band.md) | Learned band signal; interpret beside subsequent variance findings. |
| [Three-way first rows](2026-09-19-three-way-first-rows.md) | First receipt-chained 157-item run. |
| [Eight-item comparison](2026-09-19-comparison.md) | Same client and questions across three doors; a small behavior comparison. |
| [Domain gaps](../../decision-models/measurements/2026-09-20-lev-domain-gap.md) | Choice, band, and permutation coding outcomes plus base public-label results, with refusals retained. |
| [State budget](../../decision-models/measurements/2026-09-20-state-budget.md) | Paired base and choice eleven-rung comparison; failed attempt and recovery are separate. |

## Verification records

| Record | Scope |
| --- | --- |
| [Apple serving matrix](2026-09-20-apple-serving-matrix.md) | Lev passed; the combined Kev command failed on a CPU deadline despite completed Metal conformance. |
| [Workspace gate failure](2026-09-20-final-workspace-gate.md) | Original failed feature-enabled Coder tests and complete log; later phases were not reached. |
| [Serving reconciliation](../../kev/measurements/2026-09-22-serving-reconciliation.md) | Later disposition of serving failures and actual backend coverage. |

Retain the historical failures and their original issue status as dated
observations. Current instructions live in [verification](../../verification.md);
a docs-only update does not require inference, model measurements, or a workspace
gate.
