# Calibrating Lev

**Status:** proposed. There are no Lev numbers. This page defines what has
to be measured, how, and what a measurement entitles you to claim. It is the
shortest document to write and the one most likely to be ignored, so it
states the rule first.

## The rule

**Lev does not report a probability for a question family until a
calibration map has been fitted for that family and recorded.** Without one,
the door returns the typed answer, omits `probabilities` and `confidence`,
and refuses with `uncalibrated` if the caller asked for them.

This is stricter than the rule for Jev and kev, and the reason is in
[`architecture.md`](architecture.md): those two read a number the model
computed, while every Lev number is manufactured by an estimator this
repository owns. A manufactured number with no measurement behind it is
exactly the "90%" string the whole System One proposition exists to replace.

## What a question family is

A family is a set of questions that share a question type, an option
cardinality band, a domain, and a rendering style — close enough that a
single map fitted on one applies to the others. Families are declared, not
inferred. A calibration record names its family and the questions it was
fitted on, and a caller asking a question outside every declared family gets
`uncalibrated` rather than a map borrowed from a neighbor.

Splitting families too finely makes every deployment pay for its own
labelling. Splitting too coarsely produces a map that is wrong in the middle
of its own range. Family boundaries are an empirical question the first
suites answer, not a taxonomy to settle in advance.

## Fitting a map

1. Collect labelled outcomes for the family: the state, the question, the
   answer a knowledgeable person gives, and enough items to fill the bins.
2. Split the items into calibration and evaluation partitions that never
   overlap, with the split recorded.
3. Run the estimator on the calibration partition. For L2 that is `N`
   seeded samples per item; for L3 it is one call per item.
4. Fit the map from the raw signal to a probability. A binned reliability
   table is the honest default, because it shows its own resolution; an
   isotonic fit is the alternative when the bins are too sparse. Temperature
   scaling, which is what kev uses, does not apply here: there are no logits
   to scale.
5. Score the map on the evaluation partition and record the result.

Fitting on the same items you score on produces a number that means nothing.
Kev's own card fits temperature on even-indexed records and tests on odd;
the same discipline applies and the partitions are recorded in the map.

## The suite

`crates/gym/suites/support-v2-three-way.json` holds 196 authored items across
three families — 100 routing Choices, 60 urgency Nouls, 36 severity Scores —
in three partitions by construction, with a content digest so a calibration
record can name exactly what it was fitted on. It is built by
`build_support_v2_three_way.py`, which is committed beside it, so the suite
is reproducible rather than a blob.

There are three partitions and not two because the two-partition version was
read as a development set: every tuning decision in the week to 2026-09-19
was made by looking at the one evaluation split. `calibration` fits a map,
`development` chooses between maps, and `locked` is read once through a
ledger that records the read. `crates/lev/suites/support-v2.json` is the
two-way file it was built from, and it stays for the adapter training
pipeline that reads it.

**The locked partition is clean for the base model and half spent for every
adapted door.** A lock is a property of an item, not of the file that names
it, and the two files name the same 196 items. The three adapters were
trained before the three-way partitioning existed, from the 98 calibration
items of the two-way file — and 20 of those items are what the three-way
file locks. So a confirmation run against `lev-adapted@1`, `@2`, or `@3`
would spend a partition that is half training data, and the ledger would
record a read that proves less than it appears to. Nineteen of the 39 locked
items are unseen by every door and are the only fully held-out evidence in
the repository.

Two tools read the two-way file and could have caused this again.
`training/lev-adapter/convert.py` now holds back whatever the three-way file
locks and says how many items it held back; the next adapter trains on 78
records rather than 98. `lev-band` does the same. Found while reproducing the
week's claims:
[`../gym/measurements/2026-09-19-reproducing-the-week.md`](../gym/measurements/2026-09-19-reproducing-the-week.md).

It replaced a 52-item first attempt, and the reason is the whole argument for
sizing a suite properly: **every map fitted on the small suite was refused.**
Fitting five bins on twelve items turned a raw ECE of 0.031 into 0.113. The
machinery was not wrong; there was not enough evidence per bin for it to say
anything.

Difficulty is mixed on purpose. A suite of easy items produces a
near-degenerate distribution with no range to calibrate — which is exactly
what the first behavior record found — so roughly a third of the items sit
near a boundary and some are genuinely arguable. The labels are the author's
best reading, not the only defensible one, and they are not drawn from an
external dataset. That limits what the numbers can claim and it is recorded
here rather than buried.

### Bin count follows the evidence

`Map::fit_auto` picks one bin per fifteen observations, between two and ten.
More data buys more resolution; less data buys fewer, wider bins rather than
a finer table with nothing in it.

## What to measure

### Quality and calibration

| Metric | Why it is here |
| --- | --- |
| Accuracy per source and overall | The baseline. A calibrated wrong answer is still wrong. |
| Ten-bin ECE | Comparable to kev's and to Hume's 0.0313 on hosted Jev. |
| NLL and Brier | Proper scores; Brier is the one kev's preview table reports out of domain. |
| Confident errors: share of items at `p ≥ 0.9` that are wrong | The number that decides whether a threshold can gate an action. Kev's previews sit at 5–8%, hosted Jev at 3.7%. |
| Resolution: the smallest probability difference the estimator can express | `1/N` for L2, the band count for L3. A metric nobody else needs and Lev cannot omit. |

### Mechanism

These are kev's probes, reused as behavioral tests because Lev has no
mechanism to inspect:

| Probe | Passing result |
| --- | --- |
| Isolation | A secret planted in a sibling question stays at chance; the same secret in the state is found. Fails if an implementation reuses one session across questions. |
| Order sensitivity | Argmax flips across option permutations. Kev-0.5b flips 7.4%, its 8B preview 3%, hosted Jev 0%. Expect Lev to be worse and measure how much. |
| Option-set integrity | Option text containing schema-shaped or delimiter-shaped content leaves the admitted set unchanged. Structural for Lev, so this should pass trivially; test it anyway, because it is the assumption the design rests on. |
| Estimator agreement | L3's calibrated output against L2's on the same items. Divergence past tolerance means the cheap path is not a substitute for the instrument. |

### Runtime

| Measurement | Why |
| --- | --- |
| Greedy determinism, run to run | Decides whether L1 is reproducible at all. |
| Seed reproducibility, across sessions and process restarts | Decides whether L2's recorded seeds mean anything. |
| Latency against state length and question count | Decides whether fan-out is affordable and whether per-question sessions pay the full state cost. |
| Guardrail and refusal rate on decision-shaped inputs | Decides which workloads are viable at all. |
| Context limit for a state plus one question | Sets the `branch_too_long` bound. |

## The admission gate

A fitted map does not serve because it exists. `crates/gym/gates/probability-v1.json`
requires it to beat the raw signal on items it was not fitted on. The rule was
three conditions in a function body, `calibrate::admit`, until 2026-09-19; it
is now a committed file that carries its own digest, so retuning a threshold
produces a new rule rather than rewriting the meaning of every verdict already
recorded. The conditions it inherited:

- **ECE falls by at least a tenth.** Calibration is what the map is for, and
  a marginal move is binning noise.
- **Log loss does not rise.** NLL is strictly proper and punishes confident
  errors hardest, so a map that buys calibration by hedging everything into
  the middle fails here.
- **Brier rises by no more than a tenth.** Brier is calibration and
  refinement together. A binned map cannot improve refinement — it is
  monotone in the raw signal, so it cannot re-rank items by confidence — and
  the answer it scores is fixed, so it can only lose a little to binning.

The second clause used to read "and leaves the argmax alone", which is false
about the rescaled distribution and true about the answer. The two came apart
in openagents#9438, and
[`../gym/measurements/2026-09-19-calibration-and-the-argmax.md`](../gym/measurements/2026-09-19-calibration-and-the-argmax.md)
carries the contract, the enumeration, and what the gate can and cannot see.

The Brier condition started at zero tolerance and was widened after the first
run on the 196-item suite, where maps that cut ECE from 0.157 to 0.005 were
refused over a Brier move of 0.02. That is the wrong trade, and the gate file
records the case in the bound's own provenance so the reasoning does not get
lost. Changing a gate after seeing results deserves the scrutiny it sounds
like it deserves; the defence is that the condition was wrong on its own
terms, not that it was inconvenient.

**Both margins are still tuned constants, and the measurement they were
waiting for now exists.** `min_ece_reduction` records that "nothing has
measured the variance", and the gate's `pending_measurement` names
openagents#9370, which measured accuracy only.
[`measurements/2026-09-19-calibration-variance.md`](measurements/2026-09-19-calibration-variance.md)
measures it for ECE, Brier, and log loss: eight disjoint seed blocks over
the same 98 evaluation items give standard deviations of 0.0266, 0.0119,
and 0.6428 on an unchanged base door. A tenth of an ECE near 0.12 is 0.012,
which is less than half of one such standard deviation, so the inherited
threshold is smaller than the noise it was meant to exclude. Deriving the
margins from that measurement moves the gate digest every recorded verdict
names, so it is its own change and its own re-judging of the record, not a
line edit here.

Two things changed when the rule became a file. A verdict is now three
values rather than two: a measure that moved the wrong way `failed`, a
measure that moved the right way but short of its margin is `unverifiable`,
and a split too thin to judge leaves everything downstream unjudged. And
there are two rules, because a candidate can be the better decision and the
worse probability at once — `decision-v1` reads accuracy and
`probability-v1` reads log loss and confident errors first.

## Gates

Before any Lev probability gates an action in this repository:

1. A calibration map exists for the family, fitted and scored on disjoint
   partitions.
2. The isolation probe passes on the serving implementation.
3. The confident-error rate on the evaluation partition is recorded, and the
   threshold chosen for the action is justified against it rather than
   against a cookbook default.
4. The reported resolution is at least as fine as the decision boundary the
   threshold draws. A threshold at 0.85 read off an estimator whose
   resolution is 0.0625 is not a threshold.
5. The base model signature the map was fitted against matches the one
   serving.

Gate 5 is the one that will bite, and it was unimplementable until
2026-09-19: the records carried an operating system build, which is the same
for every door on one machine, and nothing that named the model. It is
implemented now. `gym::calibrate::Record::serve_to` checks a record against
the door that is running and returns the field that refused it — the build,
the base model signature, or the adapter — and `lev-serve --calibration <dir>`
sorts a directory of records into the ones this door may serve and the ones
it may not. `GET /v1/models` publishes both lists.

**A calibration map is only valid for the base it was fitted against**, and
the base arrives with the operating system. An OS update that changes the
base signature invalidates every map, every adapter, and every threshold
derived from them. The door refuses rather than serving a stale map, and
re-measurement is a scheduled cost, not an incident.

## What is admitted today

One map: `routing`, fitted on the 40 calibration items of
`support-v2-three-way` against the base door on 2026-09-19, admitted by
`probability-v1`, and served by `lev-serve --calibration`. The run is
[`measurements/2026-09-19-three-way-first-rows.md`](measurements/2026-09-19-three-way-first-rows.md),
and the map is a constant: every calibration item landed in one bin, so it
replaces the raw signal with that bin's rate. `urgency` and `severity` are
unverifiable rather than refused — 24 and 15 fitted items against a floor of
30.

## The calibration record

Each fitted family carries one committed JSON record, written by
`gym eval --fit --records <dir>` and defined in `gym::calibrate::Record`:

- The schema tag, so a record that escapes its directory still says what it
  is.
- The family, and the language the items were written in.
- The estimator configuration: the estimator, how many draws, and which seed
  block they came from. The doors here reproduce exactly, so two blocks are
  two trials and a record that names only the estimator cannot say which one
  it saw.
- The door's name and its identity: the base model signature, the adapter
  package when one is attached, and whether any of it can be checked. A
  hosted closed model publishes nothing to check and the record says so
  rather than inventing a digest.
- The operating system build.
- The suite, its content digest, and the partition the map was fitted on.
- The gate that judged it, by id and by digest.
- The map itself: the bins, with the count behind each one.
- The scored metrics on the held-out partition, raw and mapped.
- The date, and the locked-partition reads the record rests on, by subject.
  Empty is the normal case and it is a claim the ledger can check.

A family without a record does not serve probabilities. A record whose base
signature no longer matches does not serve probabilities either, and the
refusal names the field. That is the
same honest-claims boundary the kev mesh plan draws with `evalRef`, applied
to the one thing Lev can genuinely be held to.

## What these numbers will not mean

Calibration measured on one suite says nothing about a different workflow.
Kev's own card records that temperature fitted in domain does not transfer
out of it, and hosted Jev's published ECE is a property of the items Hume
probed. A Lev map fitted on classification suites does not license a
probability on a support-triage workload; that workload fits its own map on
its own labelled outcomes.

A `confidence: 0.92` from Lev is a statistic about a distribution that an
estimator produced and a table rescaled. It is not a verified probability of
being right, and no amount of measurement in this document makes it one for
a workload it was not measured on.
