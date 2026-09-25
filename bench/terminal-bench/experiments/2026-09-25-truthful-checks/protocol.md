# Truthful checks: corroboration and Microluna coverage

Frozen before this iteration reads comparison outcomes or new report answers.
Source: `7c5b53f41401e1da5ddfc3b04b9fcb9631d42c93`. Issue #9584.

## Rule

Keep the existing fitted logistic model and pass cutoff. Call a candidate failed
only when the existing model calls it failed and either the report detector finds
an admission or Jev's `admits_unmet` probability is at least 0.8. Otherwise abstain.
No model, task name, version, official verifier output, or reward is a feature.
The existing five report questions and their recorded answers remain unchanged.

The threshold was chosen using only the 132 historical calibration rows: among
0.5, 0.7, 0.8, 0.9, and 0.95, choose the highest recall reaching 90% empirical fail
precision. At 0.8, 24 of 26 fail calls are correct, catching 24 of 62 failures.
These are selection results, not validation. Do not tune the rule again in this
iteration after viewing comparison outcomes.

## Comparisons

1. Replay all 317 original rows. Compare the frozen rule with both original
   scenario checks and the original combined verdict. Report exact denominators,
   Wilson intervals, and paired task-bootstrap intervals for precision and recall
   differences (10,000 samples, seed 9584). Repeated trials are clustered by task.
   The old held-out partition was inspected in the original study and is reused
   validation, not pristine confirmation. Keep its original split unchanged.
2. Load the 18 already published Microluna v12, evidence-v1, and v13-retained trials.
   Recover the selected candidate's report and self-score, rather than a discarded
   review's report. Ask the unchanged report questions once and retain responses.
   These two tasks were studied repeatedly: this is development evidence, with no
   held-out claim. Keep related candidates in the task's existing partition.
3. A later untouched task-group confirmation remains necessary if intervals do not
   establish improvements in both precision and recall. Do not label the other
   agent's ongoing v15 run untouched merely because this iteration did not read it.
   Do not consume it for tuning or start competing live benchmark trials.

Missing reports, ambiguous candidate selection, invalid scores, API failures, and
missing labels remain explicit unknowns. Never interpret absence as passing.
Only the selected final candidate receives the official final-trial label; no
intermediate candidate inherits it.

## Operational scope

Add Microluna evidence to the truth CLI and Gym view. Preserve the legacy verdict
as a comparison signal and state the population behind any reported precision.
Test selection/restoration, missing evidence, task aliases, invalid probabilities,
and replay reproducibility. Keep experimental verification off by default.
