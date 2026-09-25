# Fitting Jev decision settings on recorded answers

2026-09-25. Issue
[#9659](https://github.com/OpenAgentsInc/openagents/issues/9659), after
DSPy 3.4.0's ReAnchor (stanfordnlp/dspy#10475, MIT), reimplemented in Rust.

**Fitting didn't establish a better setting on either component's own
objective.** On the stall detector, the fit moved `stall.progress` from 0.5
to 0.97, which makes the detector call every checkpoint the code suspects.
On the evaluation tasks, recall rose from 30 of 85 stalls (35%, 26–46%) to
45 of 85 (53%, 42–63%), and precision went from 86% to 83%. The primary
metric's interval includes zero, though: −0.61 to +0.33. That's the earlier
[stall-detection](2026-09-25-stall-detection.md) finding, rediscovered by
the fit: Jev's progress gate costs recall and adds no precision. On the
truthful checks' corroboration threshold, no candidate passed the held-out
check, so `checks.verdict.admission` stays at 0.8. Every run used recorded
answers only, with no Jev call, and no policy changed.

## What was built

- **The fit** (`crates/coder-one/src/study/reanchor.rs`). For each setting
  and each of its numbers, in order:
  1. Candidates are the midpoints between neighboring observed values. For
     a Noul threshold, the values are the probabilities of yes, with 0 and 1
     as the outer neighbors. For a Score cut, they're the answers'
     probability-weighted mean levels between the neighboring cuts. For a
     Choice weight, they're the points where an answer's pick flips to or
     from the option, on a log scale from 1/16 to 16. Each number gets at
     most 40 candidates, evenly spaced by rank.
  2. Every candidate is scored on the fit partition with the other settings
     held. The best score wins. A tie goes to the candidate in the widest
     gap between observed values, then to the lowest value.
  3. The winner replaces the current value only when it scores strictly
     better on the fit partition and passes a 5-fold held-out check.
     Otherwise the current value stays.
- **The held-out check.** The fit partition's groups (trials or tasks) are
  ordered by the SHA-256 of `openagents.decision-fit.fold:` and the group's
  name, then dealt into 5 folds, so a group never sits on both sides. The
  selection reruns without each fold and predicts that fold. The pooled
  predictions on the left-out folds must score strictly better than the
  current value's on the same examples.
- **The study** (`crates/coder-one/src/study/decisions.rs`),
  `coder-one study run decision-fit`. It reads examples from one of three
  sources:
  - `--component control.stall`: the stall-detection checkpoints and
    their recorded Jev answers.
  - `--component checks.verdict`: the truthful-check rows and their
    recorded report answers.
  - `--questions FILE --examples FILE`: any question set, with one
    recorded answer and its right decision per line.

  It fits on the fit partition only, writes the settings down, and then
  scores the evaluation partition before and after, with a bootstrap over
  whole tasks. The metric is `accuracy`, `f1`, or
  `recall-at-precision:FLOOR`.
- **The output format** is issue #9660's. `decisions.json` holds the
  settings the fit moved, by name, as `decision` blocks, and the digest
  that `policy.jev.decision` would record if they were in effect. A
  question-set run also writes a copy of the set with each fitted block
  beside its question's wording, checked with `Decision::validate`. The
  input set is never edited.
- **`checks::verdict::corroborated_at`** takes the corroboration threshold
  as an argument, so the fit can score another value on the recorded
  rows. `corroborated` calls it with the setting in effect, so no verdict
  changes.

The fold rule lives with the fit in `crates/coder-one`, not in
`crates/gym`: Coder One doesn't depend on the Gym, and the rule is a few
lines that only this study uses.

## How it was measured

The [protocol](../../bench/terminal-bench/experiments/2026-09-25-decision-fit/protocol.md)
fixed the components, the settings, the metrics, and the reporting before
the fit ran on any retained record. Both splits are the ones earlier
studies froze, reused unchanged.

| Component | Examples | Fit partition | Evaluation partition | Folds by |
| --- | --- | --- | --- | --- |
| `control.stall` (#9627) | Labeled checkpoints with recorded `progress` and `repeating` answers | 3 calibration tasks, 327 checkpoints, 49 trials | 7 evaluation tasks, 102 checkpoints | Trial |
| `checks.verdict` (#9584) | `fixtures/truth/rows.jsonl`, with recorded report answers and the verifier's reward | 132 calibration rows | 185 held-out rows, 32 tasks | Task |

- **Settings.** The stall detector calls a suspect checkpoint a stall when
  Jev's `progress` is below `stall.progress` or `repeating` is at least
  `stall.repeating`; both are 0.5. The corroborated verdict keeps a
  failure call when the detector found an admission or `admits_unmet` is
  at least `checks.verdict.admission`, which is 0.8. A checkpoint the code
  doesn't suspect, and a verdict that isn't a failure call, read no
  setting, so their answers don't make candidates.
- **Primary metric.** Each component's own threshold rule: recall when
  precision reaches a floor (0.80 for stalls, 0.90 for failures), and
  precision minus 1 below it, so any setting that reaches the floor beats
  any that doesn't.
- **Secondary metric.** F1 of the positive label, reported for reading
  only.
- **Intervals.** 95% percentile intervals from 10,000 resamples of whole
  tasks (seed 9659), and 95% Wilson intervals on precision and recall.

## Results

### Stall detector, primary metric: recall at 80% precision

| Step | Candidates | Fit partition | Held-out check | Outcome |
| --- | ---: | --- | --- | --- |
| `stall.progress` | 40 | 0.370 to 0.505 | 0.370 to 0.500 | Moved to 0.97 |
| `stall.repeating` | 40 | 0.505 to 0.505 | Not run | Kept at 0.5 |

| Partition | Setting | Calls | Precision | Recall | Metric (interval) |
| --- | --- | ---: | --- | --- | --- |
| Fit | 0.5 (now) | 88 | 80 of 88, 91% (83–95%) | 80 of 216, 37% (31–44%) | 0.370 |
| Fit | 0.97 (fitted) | 121 | 109 of 121, 90% (84–94%) | 109 of 216, 50% (44–57%) | 0.505 |
| Evaluation | 0.5 (now) | 35 | 30 of 35, 86% (71–94%) | 30 of 85, 35% (26–46%) | 0.353 (−0.33 to 0.48) |
| Evaluation | 0.97 (fitted) | 54 | 45 of 54, 83% (71–91%) | 45 of 85, 53% (42–63%) | 0.529 (−0.36 to 0.79) |

The evaluation difference is +0.18, with an interval from −0.61 to
+0.33. The metric jumps by a whole point when a resample's precision
crosses 0.80, so the interval is wide. By the protocol's rule, the fitted
setting doesn't beat the hard-coded one.

At 0.97, the fitted detector makes the same calls as the code's suspect
call alone: 121 on the fit partition and 54 on evaluation, the counts the
[stall-detection write-up](2026-09-25-stall-detection.md) reports for
code suspect. The fit found, on its own, the change that write-up
recommends. Evaluation precision still sits near its base rate: 83% of
evaluation checkpoints are stalls, because no evaluation trial passed.

### Truthful checks, primary metric: recall at 90% precision

| Step | Candidates | Fit partition | Held-out check | Outcome |
| --- | ---: | --- | --- | --- |
| `checks.verdict.admission` | 25 | 0.387 to 0.435 | 0.387 to −0.161 | Kept at 0.8 |

Without their own rows, two of the five folds chose 0.53 and 0.545. On
the left-out rows, their added failure calls brought pooled precision to
84%, below the 90% floor. So the check refused the candidate, and 0.8
stays: 24 of 26 calls right on the
fit partition, and on the held-out rows 13 of 20 (65%, 43–82%), catching
13 of 60 failures (22%, 13–34%). Neither setting reaches 90% precision on
the held-out rows.

### Secondary metric: F1, for reading only

| Component | Fitted | Evaluation F1, now to fitted | Difference (interval) | Precision, now to fitted | Recall, now to fitted |
| --- | --- | --- | --- | --- | --- |
| Stall detector | `stall.progress` 0.935 | 0.500 to 0.648 | +0.148 (+0.014 to +0.243) | 86% to 83% | 35% to 53% |
| Truthful checks | `checks.verdict.admission` 0.22 | 0.325 to 0.454 | +0.129 (+0.044 to +0.222) | 65% to 59% | 22% to 37% |

Under F1, both settings move, and both evaluation intervals exclude zero.
F1 trades precision for recall, which neither component's protocol
chose, so these runs propose nothing. They show that the fit and the
held-out check move a setting when the metric rewards it, and that the
answer depends on the metric you state before fitting.

## Negative results

- **No fitted setting beats a hard-coded one on its own objective.** The
  stall detector's interval includes zero, and the truthful check's
  candidate failed the held-out check.
- **Both evaluation partitions were read before.** The studies that froze
  the splits reported on them, so this is reused validation, not an
  untouched confirmation.
- **A floor metric makes wide intervals.** Recall at a precision floor
  changes by a whole point when a resample crosses the floor. A smooth
  metric, such as F1 or a cost-weighted error, would give tighter
  intervals, but it has to be the question the component asks.
- **The candidate cap limits resolution.** Jev returns two decimals, so a
  setting can have up to 101 observed values, and 40 candidates spaced by
  rank skip some gaps. The synthetic tests use fewer values than the cap.

## What changed, and what didn't

- Fitted settings are proposals under
  `bench/terminal-bench/experiments/2026-09-25-decision-fit/records/`,
  one directory per run: `study.json` (the plan and its digest),
  `steps.jsonl`, `decisions.json`, and `result.json`.
- No manifest, question set, or constant changed. The stall thresholds
  aren't named settings in `coder_one::decision` yet; enacting a fitted
  value means naming them there or changing `stall::FROZEN`, with a
  matched run first.

## Tests

`cargo test -p coder-one --lib study::` runs synthetic recorded answers
where the right setting is known:

- A threshold moves to the gap the labels put it in (0.6875 for labels at
  0.7 on a grid of 32 probabilities), and the held-out check passes.
- With coin-flip labels, a candidate beats 0.5 on the fit partition, fails
  the held-out check, and 0.5 stays.
- A threshold that's already right is kept.
- A tie goes to the widest gap.
- A Choice weight tilts the pick to where the labels put it, and Score cuts
  move to the right means.
- Folds keep groups whole and don't depend on input order.
- The metrics, the task bootstrap, and reading answers as Jev returns them.
- At the settings in effect, the stall rule reproduces the protocol's 88
  calls with 80 right, and the verdict rule matches `corroborated` on all
  317 rows.
- A question-set run writes fitted blocks to a copy and leaves the input
  set unchanged.

## Replay

Each command reads only retained files, makes no network call, and
reproduces the records in a few seconds. Only the wall time in
`result.json` differs between runs.

```sh
cargo run -q -p coder-one -- study run decision-fit --component control.stall \
  --out bench/terminal-bench/experiments/2026-09-25-decision-fit/records
cargo run -q -p coder-one -- study run decision-fit --component checks.verdict \
  --out bench/terminal-bench/experiments/2026-09-25-decision-fit/records
cargo run -q -p coder-one -- study run decision-fit --component control.stall --metric f1 \
  --out bench/terminal-bench/experiments/2026-09-25-decision-fit/records
cargo run -q -p coder-one -- study run decision-fit --component checks.verdict --metric f1 \
  --out bench/terminal-bench/experiments/2026-09-25-decision-fit/records
```

To fit a question set of your own, write one JSON line per recorded
answer with `id`, `group`, an optional `task`, `partition` (`fit` or
`evaluation`), `question`, `answer` as Jev returns it, and `label`, then
run:

```sh
cargo run -q -p coder-one -- study run decision-fit \
  --questions questions/SET.json --examples EXAMPLES.jsonl --metric accuracy
```
