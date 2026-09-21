# Validating the instrument

Every accuracy figure this repository has published rests on
`support-v2`: 196 items, one annotator, who also wrote the questions that
judge them. [Issue #9381](https://github.com/OpenAgentsInc/openagents/issues/9381)
names the two problems. Nothing established that a second reader reproduces
the labels, so every accuracy carried an unknown ceiling. And nothing scored
a door on items we did not write, so a question fitted to the way this author
writes items would pass every split we have.

This record answers both as far as a Linux CPU box can, and puts the ceiling
where the scores are read.

## What was measured

- **Inter-annotator agreement** on a 100-item sample of `support-v2-three-way`,
  relabelled blind by a second reader, with Cohen's kappa per family. The
  disagreements are kept, not resolved.
- **Hosted Jev on `external-v1`**, a 200-item suite whose items and labels
  come from BoolQ and MultiNLI, two public sets kev's model card names, with
  the sets' own crowd labels unchanged.

What could not be measured here: any local door. Kev weights and the Apple
runtime Lev needs are not on this machine, so the external suite has one
column. The suite is committed; the next run on a Mac adds the rest.

## The second reading

The sample is 100 ids drawn from the calibration and development partitions,
stratified by family in the suite's own proportions (routing 51, urgency 31,
severity 18) with `random.Random(9381)`, then shuffled. The locked partition
was not read. Each item's state, instructions, and criteria were written to a
file with the `truth` field removed, all 100 labels were written from that
file, and only then were they compared with the stored labels. No label was
changed after the comparison.

The second reader is an automated coding-agent session, not a person. That
is stated plainly because it changes what the number means: it says whether
the labels are reproducible by a careful reader who did not write them, and
it does not say whether they are correct. A human second reader would be a
stronger check and is still worth doing.

| Family | Items | Agree | Observed agreement | Wilson 95% | Chance | Cohen's kappa |
| --- | --- | --- | --- | --- | --- | --- |
| `routing` | 51 | 50 | **0.980** | 0.897 to 0.997 | 0.335 | 0.971 |
| `urgency` | 31 | 29 | **0.935** | 0.793 to 0.982 | 0.505 | 0.870 |
| `severity` | 18 | 18 | **1.000** | 0.824 to 1.000 | 0.340 | 1.000 |

Read the observed agreement as the ceiling: the accuracy a door would score
on that family if it answered exactly as the second reader did. Read the
interval as how loosely the ceiling is known. Eighteen severity items agreeing
on all eighteen puts the ceiling somewhere above 0.82; it does not put it at
1.0.

### The disagreements

Three items in a hundred. Each is kept with both readings in
[`crates/gym/suites/support-v2-three-way-second-annotator.json`](../../../crates/gym/suites/support-v2-three-way-second-annotator.json),
and the suite manifest lists them under a top-level `disputes` field. The
stored labels stand and the digest `54fbf413…` is unchanged, so every row
that pins it is untouched.

| Item | State | Stored | Second | Why they split |
| --- | --- | --- | --- | --- |
| `routing/067` | The trial ended early and now we are locked out. | `billing` | `sales` | A trial is a plan state, so one reader sends it to the team that owns plans; the other reads a lockout after a trial as an account and payment matter. The criteria name neither trials nor account access. |
| `urgency/058` | Our SSL certificate expires in three days. | `yes` | `no` | The rule says "right now", and a certificate that expires in three days is not broken yet; the other reading treats a fixed deadline inside the queue's turnaround as urgent. The rule does not say how an imminent failure counts. |
| `urgency/043` | Our test environment is down; production is unaffected. | `no` | `yes` | The rule says "broken, blocked, or unsafe right now", and a down test environment blocks the team that uses it; the other reading weighs the absence of production impact. The rule does not say whose breakage counts. |

All three are gaps in the rule rather than misreadings of the item. Two of
the three are in `urgency`, whose criteria are one clause each, and both turn
on what "right now" covers. That is the family with the lowest agreement and
the family where the doors score lowest, and this measurement cannot say how
much of the second is the first.

## The external suite

[`external-v1`](../../../crates/gym/suites/external-v1.json), built by
[`build_external_v1.py`](../../../crates/gym/suites/build_external_v1.py):

| Family | Source | Split | Licence | Becomes | Items |
| --- | --- | --- | --- | --- | --- |
| `boolq` | `google/boolq` at `35b264d0` | `validation` | CC BY-SA 3.0 | `noul`: passage and question as state | 100 |
| `mnli` | `nyu-mll/multi_nli` at `da70db2a` | `validation_matched` | CC BY 3.0, CC BY-SA 3.0, MIT, and other, by genre | `choice` over entailment, neutral, contradiction | 100 |

Each family is the first 100 indices of a seeded shuffle of the split, 40
calibration, 40 development, 20 locked, and each item records its source
row. The labels are the sets' own; every item carries `label_source: crowd`
and a rule saying so. The one authored part is the question text, inline on
each item and inside the digest, `137b27d8…`.

Both sets publish their own agreement, which is this suite's ceiling:

- BoolQ: three authors relabelled 110 items and the crowd answers matched
  their gold labels on **90%** (Clark et al., 2019, section 3.3). Of the
  eleven misses, six were called ambiguous and five annotator errors.
- MultiNLI: an individual validation label matches the gold label on
  **88.7%** of the matched development set (Williams et al., 2018, table 3).

Later on the same day, [`external-jevbench-v1`](../../../crates/gym/suites/external-jevbench-v1.json)
joined it as a second external suite — 231 public JevBench decisions,
recorded in [`others/2026-09-20-jevbench.md`](../others/2026-09-20-jevbench.md).
It removes this suite's one authored part: its question text is the
benchmark's own, kept in an item-keyed question set rather than written
here. JevBench publishes no agreement figure, so its items carry their
label basis as `label_rule` in place of a ceiling.

### Hosted Jev's score

```text
gym eval --jev --suite crates/gym/suites/external-v1.json --fit \
    --record crates/gym/results/external-v1.jsonl --timeout 60
```

160 items asked, 160 scored, 0 refused, 0 lost. Rows in
[`crates/gym/results/external-v1.jsonl`](../../../crates/gym/results/external-v1.jsonl).
The interval is Wilson 95%. The majority baseline is the share of the
commonest label among the 80 open items of the family.

| Family | Partition | Items | Accuracy | Wilson 95% | Published ceiling | Majority baseline |
| --- | --- | --- | --- | --- | --- | --- |
| `boolq` | calibration | 40 | 0.875 | 0.739 to 0.945 | 0.90 | 0.59 |
| `boolq` | development | 40 | 0.925 | 0.801 to 0.974 | 0.90 | 0.59 |
| `boolq` | both | 80 | **0.900** | 0.815 to 0.948 | 0.90 | 0.59 |
| `mnli` | calibration | 40 | 0.850 | 0.709 to 0.929 | 0.887 | 0.39 |
| `mnli` | development | 40 | 0.775 | 0.625 to 0.877 | 0.887 | 0.39 |
| `mnli` | both | 80 | **0.812** | 0.713 to 0.883 | 0.887 | 0.39 |

The gym's raw panel, both families together: calibration partition ECE
0.043, Brier 0.079, NLL 0.239, 1 confident error; development partition ECE
0.063, Brier 0.106, NLL 0.328, 2 confident errors. The fitted maps fail
`probability-v2` on both families on `log_loss_does_not_rise` (boolq 0.252
to 0.284, mnli 0.404 to 0.550), the same verdict the maps get on
`support-v2`: hosted Jev's raw probabilities are not improved by a map
fitted on forty items.

Every `mnli` error lands on or next to `neutral`: six contradictions and
five entailments read as neutral, four neutrals read as entailment, and no
entailment was ever read as a contradiction or the reverse.

### What this says about the co-evolution

On BoolQ hosted Jev sits at the published agreement ceiling, with the
interval spanning it. On MultiNLI it sits 0.075 below, with the ceiling
inside the interval. Neither is a fitted number: nobody here wrote the
items, the labels, or the split. Hosted Jev's `support-v2` scores of 0.93
to 0.94 per family are therefore not an artifact of our authorship alone;
the same door scores in the same range on items it could not have been
fitted to through us.

That is one door. The suites this repository trains against are the ones
whose questions and items co-evolved, and the local doors are the ones
whose recipes were tuned on them. Whether `kev` or Lev holds up on
`external-v1` is the number this record could not produce, and it is the
one that would settle the question.

## Where the ceiling now appears

The agreement table above is copied, in one line, into every record that
publishes a per-family accuracy on `support-v2`, so the reader sees the
ceiling beside the score. The rule is the one the noise floor already
follows: a number is not published without the thing that bounds it.

The records carrying it:

- [`../kev/measurements/2026-09-19-variant-scores.md`](../../kev/measurements/2026-09-19-variant-scores.md)
- [`../lev/measurements/2026-09-19-suite-v2-scores.md`](../../lev/measurements/2026-09-19-suite-v2-scores.md)
- [`../lev/measurements/2026-09-19-three-way-first-rows.md`](../../lev/measurements/2026-09-19-three-way-first-rows.md)
- [`../gym/measurements/2026-09-19-question-text-routing.md`](../../gym/measurements/2026-09-19-question-text-routing.md)
- [`2026-09-19-frozen-embedding-baseline.md`](2026-09-19-frozen-embedding-baseline.md)
- [`2026-09-19-score-ordinality.md`](2026-09-19-score-ordinality.md)

## What is measured, inferred, and neither

Measured: the three agreement rates and kappas, on the sample and by the
reader named; hosted Jev's 160 rows on `external-v1`.

Quoted: the BoolQ and MultiNLI agreement figures, from their papers.

Inferred: that hosted Jev's `support-v2` range is not our authorship's
artifact, from one door matching its external ceiling. This holds for
hosted Jev and says nothing about the local doors.

Not measured: agreement with a human second reader; agreement on the locked
partition; any local door on `external-v1`; whether the three disputed items
are the ones the doors miss.

## Reproducing this

```text
python3 crates/gym/suites/build_external_v1.py --cache ~/work/external \
    > /tmp/external-v1.json && diff /tmp/external-v1.json crates/gym/suites/external-v1.json
gym eval --jev --suite crates/gym/suites/external-v1.json --fit \
    --record /tmp/external-v1.jsonl --timeout 60
```

The second reading is not reproducible by command, which is the point of a
second reader. Its sample rule and seed are in the annotator file, so a
third reader can draw the same 100 items.
