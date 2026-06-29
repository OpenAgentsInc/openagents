# MVP-14 Baseline, Packet, And GEPA Comparison

Date: 2026-06-17
Status: internal dogfood comparison, not a product claim

This run is the first fixed-budget comparison for the OpenAgents StudyBench MVP.
It compares three candidate arms over the public-retained launch rows plus a
refs-only private validation slice:

- baseline with no study packet;
- study packet mounted as repository memory;
- study packet plus a Psionic/GEPA candidate bundle.

The Probe-facing summary lives at:

- `packages/probe/docs/benchmarks/2026-06-17-openagents-studybench-mvp-14-comparison.md`
- `packages/probe/docs/benchmarks/2026-06-17-openagents-studybench-mvp-14-comparison.json`

## Corpus And Splits

Public retained:

- Dataset ref: `dataset.openagents_studybench.public_retained.launch.v0`
- Package ref: `dataset_package.openagents_studybench.public_retained.launch.v0`
- Rows run: 10
- Patch-mode rows: 2

Private validation:

- Split ref: `split.openagents_studybench.private_validation.v0`
- Checksum ref:
  `checksum.openagents_studybench.private_validation.v0.sha256_redacted`
- Rows run: 5
- Public material: task refs and checksum refs only

Private holdout:

- Not used.

## Attempt Matrix

The run covers 45 attempts:

- 8 public-retained answer-mode rows x 3 candidate arms = 24 attempts.
- 2 public-retained patch-mode rows x 3 candidate arms = 6 attempts.
- 5 private-validation answer-mode rows x 3 candidate arms = 15 attempts.

Every attempt has:

- a Probe closeout bundle ref;
- a `probe.benchmark_closeout.v1` closeout ref;
- a `probe.studybench_rubric_score.v0` rubric-score ref;
- a resource-usage ref;
- a candidate-arm ref.

## Aggregate Result

| Arm | Attempts | Weighted score | Pass rate | Core gate pass |
| --- | ---: | ---: | ---: | ---: |
| baseline no packet | 15 | 5700 bps | 2667 bps | 4667 bps |
| study packet | 15 | 7700 bps | 6667 bps | 8667 bps |
| GEPA packet | 15 | 8300 bps | 7333 bps | 9333 bps |

Answer-mode and patch-mode scores stay separate in the JSON report. The
aggregate above is only a dogfood summary to show whether the next iteration is
worth running. It is not launch copy and not a customer promise.

## Observations

- The study packet improves source-boundary and forbidden-claim behavior on the
  public-retained launch rows.
- The GEPA packet improves the same axis a little further, mainly by reducing
  claim-boundary misses and wrong-file reads.
- Patch mode remains the smallest sample: 2 rows is enough to prove the runner
  and closeout shape, not enough to claim coding-agent quality.
- The private validation slice is represented only by refs and aggregate
  metrics. Its row bodies, rubrics, gold answers, and evidence spans stay out
  of committed docs.

## Product Boundary

This comparison does not authorize:

- customer repo ingestion;
- marketplace publication;
- public repo-expert copy;
- runtime promotion;
- payout eligibility;
- product-promise green status.

The next step is the product-promise and marketplace gate review. That review
must decide which evidence is sufficient for a planned or yellow product
promise, and what remains blocked before any public copy can broaden.
