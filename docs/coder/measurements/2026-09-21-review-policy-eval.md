# Review policy on a labeled panel

On 2026-09-21, a locally installed registry and gateway fronted a live
`kev-serve` backend (kev-0.5b, shared capacity) and ran the twenty-item
`review-policy-eval-v1` panel through `POST /v1/classify` twice: once
under the plain selection policy and once under the same policy plus a
declared review block (`trigger: uncertain`, `on_failure: keep-original`,
one item and one attempt per call, a 30 s review deadline) at an
`uncertain_below` cut of 0.9. The review mechanism worked end to end —
sixteen dispatches, sixteen answered re-judgments, sealed receipts —
and changed nothing: the reviewer agreed with the primary on every
reviewed case, so zero errors were rescued and zero answers regressed
for a 71 percent increase in call time. This is the evaluation half of
openagents#9485.

## What was asked

| Artifact | Digest |
| --- | --- |
| `review-policy-eval-v1` suite | `0c06abfb7635a9a0f9fa5444d88e779d823d9204998203009b7d3ca4705c9c0d` |
| review policy | `sha256:9db6ebdb4cd062dce588c001cd90ad032ece5b06b919de7d34ab34dbf617cbef` |
| baseline policy | `sha256:d93a719bad77771ea257d9c42fdcbb5d1175f74dc52a521fe875b528b333b20a` |
| review block | `sha256:d37feb142e06b74021b1892ddc8fd76ac9a3eed1aa1f719c6ef4225466f328e0` |

The suite is
[`crates/gym/suites/review-policy-eval-v1.json`](../../../crates/gym/suites/review-policy-eval-v1.json):
twenty support-desk items across noul (7), choice (8), and score (5)
kinds, partitioned answerable / ambiguous / near-threshold with
author-labeled expected answers. The driver is
[`crates/gym/suites/eval_review_policy_v1.py`](../../../crates/gym/suites/eval_review_policy_v1.py);
it sends each item once per policy and writes the raw exchange at
[`2026-09-21-review-policy-eval.raw.jsonl`](2026-09-21-review-policy-eval.raw.jsonl)
and the scored report at
[`2026-09-21-review-policy-eval.results.json`](2026-09-21-review-policy-eval.results.json).
The gateway ran a manifest bound to the backend's published model card,
with classify limits set to product maximums for the local run.

## Results

| Measure | Value |
| --- | --- |
| Cases | 20 (answered 20/20 under both policies) |
| Primary correct | 12/20 (error rate 0.40) |
| Final correct after review | 12/20 (error rate 0.40) |
| Review triggered (`uncertain` at cut 0.9) | 16/20 |
| Review answered | 16/16 |
| Review changed | 0 |
| Rescued | 0 |
| Regressed | 0 |
| Confident errors at the declared cut | 1 |
| Fallback dispatches | 0 (never triggered) |
| Refusals, transport errors | 0 |
| Calls | 40 HTTP (20 primary + 20 review-run; 16 review dispatches inside) |
| Call time | baseline 375.5 s, review run 641.4 s (+71%); wall 1017 s |
| Tokens | 3 652 in / 2 952 out baseline; 1 458 in / 1 112 out for reviews |
| Cost | unmetered — spend recorded as unknown |

Latency is kev-0.5b on CPU, roughly 14 s per call and 29 s per
review-wrapped call; it measures the mechanism's overhead on this
backend, not a serving bound.

## Threshold sensitivity

Recomputed from the retained primary evidence without further calls —
noul distance to the selected side, choice and score `confidence`:

| Cut | Reviewed | Wrong among reviewed | Confident errors |
| --- | --- | --- | --- |
| 0.5 | 2 | 0 | 8 |
| 0.6 | 5 | 2 | 6 |
| 0.7 | 6 | 2 | 6 |
| 0.8 | 11 | 5 | 3 |
| 0.9 | 15 | 7 | 1 |
| 0.95 | 18 | 8 | 0 |

The gateway flagged sixteen units uncertain at 0.9; the table's
recompute flags fifteen. The one-case difference is the score unit at
exactly 0.90, which the server's flag rule counts as uncertain and the
driver's strict `<` does not — worth settling before the next run.

## Findings

- **Same-model review agrees with itself.** Sixteen re-judgments, all
  with the same model and the same question, changed zero answers. On
  this suite the pass bought coverage over the flagged cases but no
  rescue — a reviewer's value has to come from a different question, a
  different model, or added evidence, none of which this panel varies.
- **The mechanism itself is sound.** Every reviewed unit carried its
  review attempt receipt (attempt id, model, artifact digest, latency,
  usage), `on_failure` never had to fire, and the uncertain trigger
  matched the evidence distribution.
- **Errors cluster where evidence is middling.** Six of the eight
  primary errors sat in the 0.47–0.90 evidence band — exactly the
  region a cut can reach — while three were confident (0.90–0.97),
  including the one confident error at the declared cut.
- **Score mode was the weakest kind** (3 of 5 wrong), consistent with
  kev-0.5b's size rather than a policy defect; both borderline
  sentiment items stayed correct.

## Limits

Twenty items, one model, one annotator, one cut — a mechanism smoke,
not a quality map. The zero-rescue result says the *same-question,
same-model* review shape cannot rescue errors on this panel; it does
not bound what a distinct reviewer could recover. Fallback and refusal
paths were never exercised: the suite produced no no-match-with-fallback
or refused cases under these policies. Downstream task quality from
reviewed answers remains unmeasured, and the driver's uncertain rule
and the server's differ on the 0.90 boundary by one case.
