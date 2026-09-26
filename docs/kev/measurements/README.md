# Kev measurement records

These are dated observations for exact checkpoint bytes, numerical settings,
workloads, and hosts. A replaced Hub name is not the same artifact. Start with
the [Kev overview](../README.md), [artifact locks](../artifacts.md), and
[serving reconciliation](2026-09-22-serving-reconciliation.md).

| Record | Scope and decision |
| --- | --- |
| [Historical variant scores](2026-09-19-variant-scores.md) | Four original checkpoints on one support suite; not replacement-weight scores. |
| [Replacement 4B](2026-09-20-candidate-4b.md) | Separate pinned candidate, reference conformance, 461 open workload items, and refusal coverage. |
| [Merge precision](2026-09-20-merge-precision.md) | fp32 LoRA merging before cast and loading-memory observations. |
| [Metal attention](2026-09-20-metal-attention.md) | Experimental SDPA and padding; the bf16 changed winner prevents treating it as numerically identical. |
| [Per-variant admission](2026-09-20-per-variant-admission.md) | Working-memory estimate checked against an actual CPU forward. |
| [Program-selection latency](2026-09-20-program-selection-latency.md) | Eight passes per checkpoint with retained workload and host conditions. |
| [Real weights and domain gap](2026-09-20-real-weights-and-domain-gap.md) | Two small checkpoints on support and public-label workloads; training-source overlap is disclosed. |
| [State reuse](2026-09-20-state-reuse.md) | Archived-input audit found no exact repeated states; no production cache-hit claim. |
| [Training decision](2026-09-20-training-decision.md) | Training deferred pending better inputs and independent labels; no training ran. |
| [Serving reconciliation](2026-09-22-serving-reconciliation.md) | Actual Metal inference, CPU timeout failures, and feature builds separated. |

Related records are the [quiet CPU timing](../../gym/measurements/2026-09-20-kev-quiet-latency.md),
[single 4B Metal pass](../../gym/measurements/2026-09-20-kev-4b-metal-pass.md),
and [Apple serving matrix](../../lev/measurements/2026-09-20-apple-serving-matrix.md).
A single pass is not a multi-block timing floor. Missing hardware measurements
stay unmeasured even when a tracking issue is closed.

The old verification commands remain as provenance. New checks follow the
[current policy](../../verification.md); weight-backed conformance requires
`KEV_CONFORMANCE=1` and the exact artifacts.
