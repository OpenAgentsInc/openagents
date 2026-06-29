# OpenAgents StudyBench MVP-14 Comparison

Date: 2026-06-17
Status: recorded public-safe MVP comparison, not a product claim

This note records the first fixed-budget OpenAgents StudyBench comparison for
issue #5296. It uses the current Probe StudyBench answer and patch runner
contracts, with manual/judge-supplied claim-score refs and Probe closeout refs.

Machine-readable summary:

- `2026-06-17-openagents-studybench-mvp-14-comparison.json`

## Candidate Arms

1. `candidate_arm.openagents_studybench.baseline_no_packet.v0`
   - no study packet mounted
   - no GEPA candidate bundle
2. `candidate_arm.openagents_studybench.study_packet.v0`
   - `study_packet.openagents.launch.v0` mounted as refs-only repository memory
   - no GEPA candidate bundle
3. `candidate_arm.openagents_studybench.gepa_packet.v0`
   - `study_packet.openagents.launch.v0` mounted
   - `psionic_gepa_candidate.openagents_studybench.mvp_14.v0` selected as a
     candidate bundle

## Rows And Modes

- Public retained: 10 rows from
  `dataset.openagents_studybench.public_retained.launch.v0`.
- Private validation: 5 rows by refs only from
  `split.openagents_studybench.private_validation.v0`.
- Private holdout: not used.
- Answer mode: 8 public-retained rows and 5 private-validation rows per arm.
- Patch mode: 2 public-retained rows per arm.

The private validation rows are not committed. The public summary carries only
task refs and a checksum ref.

## Coverage

- Expected attempts: 45.
- Observed attempts: 45.
- Missing closeout bundle refs: none.
- Missing Probe closeout refs: none.
- Missing rubric score refs: none.

Each attempt maps to refs using the patterns in the JSON report. The referenced
closeout bundles use the normal Probe closeout contract plus StudyBench task and
rubric-score refs. This report does not embed raw private rows, hidden rubrics,
hidden gold answers, private customer source, or raw repo archives.

## Results

| Arm | Attempts | Weighted score | Pass rate | Core gate pass |
| --- | ---: | ---: | ---: | ---: |
| baseline no packet | 15 | 5700 bps | 2667 bps | 4667 bps |
| study packet | 15 | 7700 bps | 6667 bps | 8667 bps |
| GEPA packet | 15 | 8300 bps | 7333 bps | 9333 bps |

Patch-mode rows are reported separately from answer-mode rows in the JSON
metrics. Patch-mode carries test pass rate and tool budget use; answer-mode
keeps those fields null or zero because no repo patch is allowed.

## Boundary

This is internal dogfood evidence. It does not claim customer availability,
marketplace readiness, product readiness, payout eligibility, runtime promotion,
or a trained repo-expert model.

The next gate is product-promise and marketplace review, not public copy.
