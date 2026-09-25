# Decision-settings fit on recorded answers: protocol

Issue [#9659](https://github.com/OpenAgentsInc/openagents/issues/9659).
Written on 2026-09-25, after the fitting code and its synthetic tests, and
before the fit ran on any retained record. Nothing below changes after that.

## What is fitted

`coder-one study run decision-fit` fits Jev decision settings, the numbers
that turn a recorded answer into an action, without changing any question's
wording and without a Jev call. The algorithm is in
`crates/coder-one/src/study/reanchor.rs`, reimplemented from DSPy 3.4.0's
ReAnchor (stanfordnlp/dspy#10475, MIT):

- Candidates are the midpoints between neighboring observed probabilities,
  with 0 and 1 as the outer neighbors, at most 40 per setting, evenly spaced
  by rank.
- A candidate replaces the current value only when it scores strictly
  better on the fit partition and passes a 5-fold held-out check: the fit
  partition's groups are dealt into 5 folds by SHA-256 of their names, the
  selection reruns without each fold, and the pooled predictions on the
  left-out folds must score strictly better than the current value's.
- A tie goes to the candidate in the widest gap between observed values.
- One pass over the settings, in the order below.

## Components and splits

Both splits were frozen by earlier studies and are reused unchanged.

1. **`control.stall`** (#9627). Examples: the 429 labeled checkpoints in
   `2026-09-25-stall-detection/records/`, with the Jev answers each row
   carries (replayed from `jev-recorded.json`, 0 misses). Settings, in order:
   `stall.progress` (a suspect checkpoint stalls when `progress` is below
   it; now 0.5) and `stall.repeating` (it stalls when `repeating` is at or
   above it; now 0.5). Only suspect checkpoints read either. Fit partition:
   the 3 calibration tasks, 327 checkpoints, folds by trial. Evaluation:
   the 7 evaluation tasks, 102 checkpoints.
2. **`checks.verdict`** (#9584). Examples: the 317 rows of
   `crates/coder-one/fixtures/truth/rows.jsonl`, with their recorded report
   answers. Setting: `checks.verdict.admission`, the `admits_unmet`
   probability at which a failure call is corroborated (now 0.8). Only a
   failure call without a detected admission reads it. Fit partition: the
   132 calibration rows, folds by task. Evaluation: the 185 held-out rows.

## Metrics, fixed now

The primary metric of each component is the rule its own protocol chose
thresholds by, so a fitted value answers the question that picked the
current one:

- `control.stall`: `recall-at-precision:0.8`, stall positive. Recall of
  stalls when precision is at least 0.80; any setting below the floor
  scores its precision minus 1.
- `checks.verdict`: `recall-at-precision:0.9`, failure positive.

A secondary run of each uses F1 of the positive label, reported for
reading; it decides nothing.

## Reporting

For each run: the fitted settings, each step's candidates, training and
held-out scores, and whether it was accepted; the metric before and after
on the fit partition and on the evaluation partition; and 95% percentile
intervals from a bootstrap that resamples whole tasks (10,000 samples,
seed 9659), for each score and for the difference. Precision and recall
carry 95% Wilson intervals. A fitted setting "beats the hard-coded
setting" only when the evaluation difference's interval excludes zero.

Both evaluation partitions were read by the studies that froze them, so
this is reused validation, not an untouched confirmation.

## Budget

No Jev call, no Luna session, and no Terminal-Bench trial. Fitted settings
are written as proposals under this directory; no policy changes.
