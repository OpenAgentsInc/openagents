# Calibration and the argmax

Two documents on `main` said opposite things about
[`Map::apply_distribution`](../../../crates/gym/src/calibrate.rs), and
openagents#9438 asked for a test rather than a third reading.

- The codebase audit at `a43033cd`, finding A05, said a map **can** change
  which option wins, and reproduced it: `{yes: 0.8, no: 0.2}` rescaled to
  `{yes: 0.25, no: 0.75}` while `eval::mapped_observations` kept the original
  `correct: false`.
- The panel record from openagents#9414 at `2e8fe19c49` said the opposite:
  "structurally `Map::apply_distribution` never changes the winner."

The audit is right about the arithmetic. The record is right about what the
harness measures, and wrong about why. Three questions come apart, and this
page answers each of them.

The enumeration lives in
[`crates/gym/tests/winner_inversion.rs`](../../../crates/gym/tests/winner_inversion.rs)
and runs with the rest of the suite:

```text
cargo +1.97.1 test -p gym --test winner_inversion -- --nocapture
```

## 1. A rescale moves the argmax, and here is exactly when

`rescale` gives the selected option its calibrated probability `c` and hands
each loser `(1 - c) · v / rest`, where `rest` is every losing share together.
The largest loser `m` therefore lands at `(1 - c) · m / rest`, and it passes
the selected option exactly when:

```text
c < m / (m + rest)
```

Because `m` never exceeds `rest`, that threshold never exceeds one half. A
map that reads every raw signal at or above 0.5 cannot move an argmax
whatever the distribution looks like. A map with a bin below 0.5 can, and
[`Map::fit`](../../../crates/gym/src/calibrate.rs) produces such a bin
whenever fewer than half the observations in it were right: with Jeffreys
smoothing, `(correct + 0.5) / (count + 1)` falls below one half exactly when
`correct < count / 2`.

Two edges are worth naming. Where the estimator was unanimous there is no
`rest` to share in proportion, the remainder spreads evenly, and the
threshold is `1 / k` for `k` options. On the threshold itself the two land on
the same number, and `max_by` returns the last of equal maxima — so which
option is largest is decided by the order the estimator listed them in rather
than by any probability.

A sweep over nine distributions and 101 calibrated values checks the
predicate against the behaviour on every pair.

## 2. The gate would admit such a map

A [`Comparison`](../../../crates/gym/src/gate.rs) carries scores and an item
count. It carries nothing about the fitted table, so no criterion in
`probability-v1` reads one, and `accuracy_does_not_fall` cannot move because
the answer being scored is fixed on both sides.

The test builds the case rather than arguing it. Forty binary items, the
estimator unanimous on every one and right on ten: the fitted table reads
that signal at 0.256, which is a large calibration win and low enough to move
an argmax on any binary distribution.

| Measure | Raw | Through the map |
| --- | --- | --- |
| Accuracy | 0.250 | 0.250 |
| ECE | 0.750 | 0.006 |
| Brier | 0.750 | 0.188 |
| Log loss | 20.723 | 0.562 |
| Confident errors | 30 | 0 |

`probability-v1` passes it. So the answer to "is this unreachable through the
gate?" is no: the gate is not what stops it.

## 3. Nothing committed reaches it

Every record under `crates/lev/calibration/` against every scored row under
`crates/gym/results/` is **14,082 map-and-row pairs, and no argmax moves.**
The lowest fitted value in any committed record is 0.500, in
`lev-base/severity.json`, which the gate refused for its item count, and 0.5
is the boundary rather than past it.

Refitting instead of reading — for every door and family the store holds on
both sides of a partition, fit the map the harness would fit and judge it
with the committed rule — gives the same answer where it counts:

| Group | Fitted on | Lowest bin | Argmax moves | Verdict |
| --- | --- | --- | --- | --- |
| `support-v2-three-way` `lev-base` `routing` | 40 | 0.768 | 0 | passed |
| `support-v2-three-way` `lev-adapted@1` `routing` | 40 | 0.939 | 0 | passed |
| `support-v2-three-way` `kev-4b` `routing` | 40 | 0.250 | 2 | failed: `brier_stays_within_tolerance` |
| `support-v2-three-way` `kev-0.6b` `severity` | 15 | 0.333 | 14 | unverifiable: `fitted_on>=30` |
| `restatement-v1` `kev-4b` `writes_none` | 8 | 0.000 | 8 | unverifiable: `fitted_on>=30` |

The two passing rows reproduce the two committed admitted records verbatim,
verdict line included. Everything that moves an argmax is refused, and the
third row is the one to watch: `kev-4b` on `routing` clears the 30-item floor
with 40 items, reads its top bin at 0.250, moves two argmaxes out of 40, and
is refused for a Brier move of 0.143 to 0.193 against a ceiling of 0.157 —
which has nothing to do with the map's shape. One different number in an
unrelated column and it is admitted.

That row is also the shape of door this is most likely to arrive from. A door
that answers a question's negation backwards is confidently anti-correlated
with the truth, which is exactly the failure openagents#9414's panel found on
`kev-4b`, and a map fitted on that family is sharply inverting by
construction.

## The contract, and what changed

A map is fitted on observations whose `correct` means "the estimator's choice
was the labelled answer", so a fitted bin estimates how often that choice is
right and estimates nothing about which other option would be right instead.
The remainder is spread in the estimator's own proportions, which is a
display convention rather than a fitted quantity. Reading a new answer out of
it reads a prediction from a number nobody fitted.

So this repository holds the first of the two contracts openagents#9438 named:
**calibrate confidence in a fixed selected answer.** The selected option is
the estimator's argmax, a map rescales its probability, and a calibrated
probability below one half is the same answer reported as more likely wrong
than right — which is what a caller's refusal threshold is for.

The second contract, letting the predictor change and recomputing correctness
against retained labels, is not available over this store.
[`Row`](../../../crates/gym/src/row.rs) carries `correct` and no label, so
nothing in a row can say whether the runner-up was the answer. A block draw
carries `truth`; an evaluation row does not.

Two consumers were reading the rescaled distribution's own argmax, and both
now name the selected option:

- `eval::mapped_observations` took `mapped.values().max()` and paired it with
  `row.correct`. Where a map moves an argmax those are different options, and
  the recorded observation charged the selected answer with a probability the
  door never claimed for it. On the audit's own construction it recorded
  `(0.75, false)` — a confident error — where the map's claim about that
  answer was 0.25. It now reads the selected option's probability.
- `lev::serve` derived a Choice's `choice` and `confidence` from the rescaled
  distribution, so a moved argmax would have been served as a different
  answer than the estimator chose. The door now reads the selected option
  from the raw distribution before the map is applied, and
  `estimator::confidence_in` reports sharpness on the answer rather than on
  whichever option leads.

A Noul is untouched, and deliberately. It answers with `p(yes)` rather than
with a choice, so there is no selected answer to hold fixed: the number is
the answer, and a rescale of a two-option distribution is a correct posterior
over it. A caller that wants a yes or a no reads it against its own
threshold, which is the same thing it does without a map.

No committed number moves, because nothing committed reaches the case. The
two refitted verdicts above are computed after the change and match the
records fitted before it.

## What openagents#9376's floors inherit

Nothing, and not for the reason the issue expected. The floors — ECE 0.0266,
Brier 0.0119, log loss 0.6428, from
[`2026-09-19-calibration-variance.md`](2026-09-19-calibration-variance.md) —
are spreads of an unchanged door across disjoint seed blocks, scored by
`calibrate::score` over `Draws::observations`. Those are raw block draws
carrying a top frequency and an outcome, with no distribution in them, so no
map is applied and `mapped_observations` is not on that path at all.
