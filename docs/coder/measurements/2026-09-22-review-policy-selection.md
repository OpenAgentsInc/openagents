# Review policy selection on development and held-out halves

This doc finishes the evaluation half of openagents#9485: policy
settings selected on development data and confirmed on held-out data,
plus the calibration and risk-coverage accounting the acceptance item
asks for. It reuses the retained records of
[`2026-09-21-review-policy-eval.md`](2026-09-21-review-policy-eval.md) —
the same twenty-item `review-policy-eval-v1` panel
(`0c06abfb7635a9a0f9fa5444d88e779d823d9204998203009b7d3ca4705c9c0d`),
the same `kev-0.5b` primary and reviewer, the same two runs. No new
model calls were made; every number below is recomputed from
[`2026-09-21-review-policy-eval.results.json`](2026-09-21-review-policy-eval.results.json)
and the raw exchange.

## Split

Alternating suite order: even-indexed items are the development half,
odd-indexed the held-out half — deterministic, fixed before any number
was computed, and mixed across noul, choice, and score kinds. Ten
items per half; the halves share the annotator, the model, and the
policy digests, so the only thing that differs is which items answered.

## What the development half selects

Two policy choices were on the table: a review trigger (`uncertain` at
a declared cut, `keep-original` on reviewer failure) and an abstention
floor on primary evidence.

**Review trigger.** The development half's six primary errors sat at
evidence 0.49–0.92; five carried the `uncertain` flag and would have
dispatched a re-judgment at any cut at or above 0.8. On the retained
records every one of the sixteen reviewed units — development and
held-out alike — returned the same selection the primary gave. A
review pass under this pairing spends a second dispatch and changes no
answer, so the development half selects **no review**: strict primary
operation, which the policy format already supports by omitting the
review block. This is a selection about this reviewer/model pairing,
not about the mechanism — the pass dispatches, records, and reports
correctly, and a distinct reviewer, question, or evidence source was
never varied.

**Abstention floor.** Risk-coverage on the development half:

| Floor | Coverage | Errors kept | Error rate |
| --- | --- | --- | --- |
| 0.00 | 10/10 | 6 | 0.60 |
| 0.50 | 9/10 | 5 | 0.56 |
| 0.60 | 8/10 | 5 | 0.62 |
| 0.70 | 7/10 | 4 | 0.57 |
| 0.80 | 3/10 | 1 | 0.33 |
| 0.90 | 3/10 | 1 | 0.33 |
| 0.95 | 2/10 | 0 | 0.00 |

Every floor that removes errors removes more correct answers than
errors: at 0.80 the floor discards seven of ten items to keep one
remaining error out. The development half nominates **no floor** as a
declared quality setting.

## Held-out confirmation

| Floor | Coverage | Errors kept | Error rate |
| --- | --- | --- | --- |
| 0.00 | 10/10 | 2 | 0.20 |
| 0.50 | 9/10 | 1 | 0.11 |
| 0.60 | 8/10 | 1 | 0.12 |
| 0.70 | 5/10 | 0 | 0.00 |
| 0.80 | 3/10 | 0 | 0.00 |
| 0.90 | 1/10 | 0 | 0.00 |
| 0.95 | 0/10 | 0 | — |

Held-out confirms the selection in both directions. The two held-out
errors (evidence 0.65 and 0.41) sit below every floor at or above
0.7 — but confirming the nominated setting needs the cheap direction,
and the cheap direction is what fails: a floor the development data
would justify only by discarding most coverage discards it there too
(at 0.7, five of ten held-out items would be dropped to save two
errors). Review on held-out: nine of ten units flagged, nine reviewed,
zero changed — the same-model re-judgment agrees with itself there as
it did in development.

## Calibration

Primary evidence against correctness, all twenty cases:

| Evidence bucket | Cases | Accuracy | Mean evidence |
| --- | --- | --- | --- |
| [0, 0.6) | 4 | 0.50 | 0.48 |
| [0.6, 0.8) | 10 | 0.50 | 0.70 |
| [0.8, 0.9) | 2 | 1.00 | 0.85 |
| [0.9, 1] | 4 | 0.75 | 0.94 |

Four of twenty cases sit at or above 0.9 evidence and one of them is
wrong — the highest bucket is not a safe region, and no bucket's
observed accuracy reaches its mean evidence. Evidence on this model
and panel is not calibrated, and nothing here presents it as a
correctness probability.

## The `uncertain` flag's semantics, settled

The earlier doc noted a one-case disagreement at evidence 0.90. The
server's flag is computed on **top distribution mass** — for binary,
`max(p, 1−p)`; for single-label and score, the largest label/level
probability — strictly below the declared cut, never on the answer's
`confidence` field. The driver's recompute used `confidence` for score
units, which for `score-severity-intermittent-borderline` is 0.90 while
its top level mass is 0.71. Same cutoff, different field; the server's
rule is the contract, and the retained raw records reproduce all
sixteen flags on it exactly.

## Failure and budget accounting

Across both runs: 40 HTTP calls, 16 primary-plus-review dispatches,
zero refusals, zero transport failures, zero reviewer failures, zero
fallbacks — so `on_failure`, exhaustion, and fallback paths are
covered by the unit suite rather than this panel. Where the server did
have failure surface to report, it reported it in shape: each reviewed
unit carries `review_status`, a `review` record with reason, attempt
id, model and artifact digest, latency, and usage, an `attempts` chain
naming primary and review roles with sealed receipts, and
`final_source`; unreviewed units report `not-reviewed` rather than
borrowing a review they never ran.

## Cost and net quality

The backend is unmetered: spend is recorded as `unknown` in the
results, never estimated or zeroed. Measured cost is tokens and time —
3 652 in / 2 952 out for the baseline run plus 1 458 in / 1 112 out for
the review pass, and +71 percent call time (375.5 s to 641.4 s). Net
quality under the declared review policy is unchanged at any
confidence presentation: 12/20 correct before and after. The honest
summary for a buyer of this feature is that uncertainty-targeted
review is implemented, instrumented, and selectable — and on this
panel it buys coverage over flagged cases at known latency cost and
zero measured rescue.

## Limits

Twenty items, one model, one annotator, one reviewer pairing. A
selection of *no review* under same-model re-judgment says nothing
about what a distinct reviewer recovers; the mechanism's rescue value
is unbounded by this panel, in both directions. Calibration buckets of
two and four cases are descriptive, not fitted. Downstream task
quality from reviewed answers remains unmeasured.
