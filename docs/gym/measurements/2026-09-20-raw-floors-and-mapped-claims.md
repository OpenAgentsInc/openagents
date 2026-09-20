# The raw floors and mapped claims, re-derived

openagents#9419 settled the calibration contract — a map rescales the
probability of a fixed selected answer and never replaces it — and required
a review of everything the settlement could have touched. Three deliverables
from that issue live here: the re-derived spreads, the two-sigma comparison
quantities kept distinct from them, and an inventory of the mapped claims identified below, labelled unchanged,
changed, withdrawn, or unverifiable.

Nothing was rerun against a door. Every number below is recomputed from the
committed draws, rows, and records by
[`crates/gym/tests/rederived_floors.rs`](../../../crates/gym/tests/rederived_floors.rs):

```text
cargo +1.97.1 test -p gym --test rederived_floors -- --nocapture
```

## What the raw path is

`gym spread` scores each seed block's `Draw::observation` — the raw signal,
a top frequency paired with its outcome — directly. No `Map` is applied on
that path and `eval::mapped_observations` is not reachable from it. So the
audit finding that moved `mapped_observations` could not have moved the
floors. That is an argument about reachability; the test file checks it by
recomputing.

## The raw spreads, re-derived

All eight `lev-base` evaluation blocks of
`crates/gym/results/support-v2-calibration-blocks.jsonl`, scored through
`calibrate::score`. Every per-block row and every spread reproduces the
published record at its printed precision:

| Metric | Published sd | Re-derived | Status |
| --- | --- | --- | --- |
| accuracy | 0.0197 | reproduces | unchanged |
| ECE | 0.0266 | reproduces | unchanged |
| Brier | 0.0119 | reproduces | unchanged |
| log loss | 0.6428 | reproduces | unchanged |
| confident errors | 2.4928 | reproduces | unchanged |

## The two-sigma bounds, distinct from the spreads

The adopted values above are raw block standard deviations. The bound a
comparison has to clear is `sigmas · sd · √(1/b + 1/c)` — at two sigma with
one block on each side, `sd · 2√2`, a different and larger number. Both are
re-derived:

| Metric | Raw sd | Two-sigma bound |
| --- | --- | --- |
| accuracy | 0.0197 | 0.056 |
| ECE | 0.0266 | 0.075 |
| Brier | 0.0119 | 0.034 |
| log loss | 0.6428 | 1.818 |
| confident errors | 2.4928 | 7.05 — published as "seven" |

These are the same numbers `ab::Rule::v2` derived its floors from, and the
bounds were already stated this way in
[`../../lev/measurements/2026-09-19-seed-variance.md`](../../lev/measurements/2026-09-19-seed-variance.md)
and the calibration-variance record; what the issue fixed is the confusion
of naming the spreads themselves "thresholds". No numeric basis changed, so
no gate or rule digest moves.

## The mapped claims, re-derived

The mapped claims listed below are reproduced as follows: a map fitted on block 0's calibration draws, applied
with `Map::apply` to each held-out observation's raw signal, the outcome
carried unchanged. That path never touched `mapped_observations`. The claims
covered are the ones listed below — the mapped numbers of
[`../../lev/measurements/2026-09-19-calibration-variance.md`](../../lev/measurements/2026-09-19-calibration-variance.md),
with the six calibration records reviewed separately below. This is a finite
inventory of named artifacts, not proof about unretained history. These
listed values reproduce:

| Claim | Source | Re-derived | Status |
| --- | --- | --- | --- |
| `lev-base` suite pooled map, block 0: ECE 0.071, Brier 0.157, log loss 0.481, 0 confident errors | draws, block-0 calibration (98 items) | reproduces | unchanged |
| `lev-base` suite mapped log loss: mean 0.477, sd 0.0182 | same | reproduces | unchanged |
| `lev-base` routing pooled map, block 0: ECE 0.024, Brier 0.133, log loss 0.431 | draws, block-0 calibration (50 items) | reproduces | unchanged |
| `lev-base` routing mapped log loss: mean 0.433, sd 0.0188 | same | reproduces | unchanged |
| `lev-base` urgency pooled map, block 0: ECE 0.025, Brier 0.196, log loss 0.582; mapped Brier mean 0.185 against raw 0.182 | draws, block-0 calibration (30 items) | reproduces | unchanged |
| `lev-base` severity pooled map, block 0: ECE 0.120, Brier 0.215, log loss 0.639 | draws, block-0 calibration (18 items) | reproduces | unchanged |
| `lev-band` band-conditioned map, block 0: ECE 0.069, Brier 0.104, log loss 0.388 against raw 2.601 and pooled 0.499 | draws, block-0 calibration (98 items), band tables `almost certain` and `likely` | reproduces | unchanged |
| `lev-band` band-conditioned log loss: mean 0.409, sd 0.0204 | same | reproduces | unchanged |
| mapped accuracy equals raw accuracy on every block, both doors | the fixed-answer contract | checked per block, not just on the means | unchanged |

The two paths a mapped number can be produced by agree where they overlap:
`Map::apply` on a draw's top frequency and `mapped_observations` reading the
selected option's rescaled probability compute the same value, because the
rescale hands the selected option exactly the calibrated reading of its raw
signal. A test asserts the two give identical observations over the
committed `lev-base` routing rows.

## The six committed records, regenerated

Every record under `crates/lev/calibration/` is regenerated from
`crates/gym/results/support-v2-three-way.jsonl`: the map refitted on the
rows of its recorded partition, scored on the rest through the post-fix
`mapped_observations`, judged by the committed `probability-v1` gate. The
test reads the file through `Store::verified_rows`, so the receipt chain,
the schema allowlist, and every line's parse are checked before a number is
computed; the rows are then checked against the committed three-way suite —
suite digest, item identity, partition, family, label provenance, per-door
coverage of the open partitions, and one estimator and question set per
door — before a record's fit runs.

| Record | fitted_on | Verdict | Status |
| --- | --- | --- | --- |
| `lev-base/routing` | 40 | passed | unchanged — counts, map, verdict, and provenance identical; float metrics identical or within one representable step |
| `lev-base/urgency` | 24 | unverifiable: `fitted_on>=30` | unchanged |
| `lev-base/severity` | 15 | unverifiable: `fitted_on>=30` | unchanged |
| `lev-adapted@1/routing` | 40 | passed | unchanged — counts, map, verdict, and provenance identical; mapped Brier differs by 6.9e-18, one representable step |
| `lev-adapted@1/urgency` | 24 | unverifiable: `fitted_on>=30` | unchanged |
| `lev-adapted@1/severity` | 15 | unverifiable: `fitted_on>=30` | unchanged — raw ECE differs by 2.8e-17, one representable step |

Eight float fields differ by one representable step in this re-derivation:

| Record | Field | Stored | Recomputed |
| --- | --- | --- | --- |
| `lev-adapted@1/routing` | mapped Brier | 0.04762046400951813 | 0.047620464009518124 |
| `lev-adapted@1/severity` | raw ECE | 0.24999999999999992 | 0.24999999999999994 |
| `lev-base/routing` | mapped ECE | 0.10670731707317029 | 0.10670731707317027 |
| `lev-base/severity` | raw ECE | 0.11607142857142856 | 0.11607142857142855 |
| `lev-base/severity` | raw log loss | 0.4929959869784256 | 0.49299598697842556 |
| `lev-base/severity` | mapped ECE | 0.221938775510204 | 0.22193877551020402 |
| `lev-base/urgency` | raw Brier | 0.20052083333333337 | 0.20052083333333334 |
| `lev-base/urgency` | mapped Brier | 0.2076 | 0.20759999999999998 |

The cause of these arithmetic differences is not established. Evaluation
order, parsing, and the write-time toolchain are possible contributors. The
test compares floating-point metrics within `1e-15` and prints every nonzero
difference; it checks item counts, confident-error counts, fitted tables,
provenance, and verdicts exactly. All six recorded verdicts are unchanged.
The separate selected-answer tests and retained-map enumeration establish
the selection behavior; a small aggregate difference alone would not prove
that no answer changed.

The post-fix `mapped_observations` reading the selected option rather than
the rescaled maximum changes a mapped value only where a map moves an
argmax, and no committed record's map moves the argmax of any committed row
— `crates/gym/tests/winner_inversion.rs` enumerates all 14,082 pairs.

## What changed elsewhere

One stale claim was found and corrected:
[`../../lev/calibration.md`](../../lev/calibration.md) quoted a preliminary
four-block run — ECE 0.0207, Brier 0.0099, log loss 0.6605 — where it now
cites the published eight-block spreads.

Claims that rest on the retired `support-v1` suite — the 0.361 log loss
quoted in `docs/lev/README.md` and `docs/lev/disposition.md`, and the
preliminary four-block numbers `docs/lev/calibration.md` used to carry —
are **unverifiable**: that suite's draws and rows are not retained in a form
this test can score, so the numbers stand as recorded history and are not
claimed here. They were flagged the same way in the calibration-variance
record.

## Digests

No gate semantic or numeric basis changed: `probability-v1`'s digest stands,
the recorded verdicts regenerate verbatim under it, and `ab::Rule::v2`'s
derived bounds name the same floors. New measurement rows use `eval_row.v2` because the selected-answer
probability has a different meaning from v1's distribution maximum. Existing
rows and receipts remain v1 and are not rewritten. This input-schema change
does not change the gate's criteria or thresholds.
