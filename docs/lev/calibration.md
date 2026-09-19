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

## The suites

Adopt kev's frozen-suite format rather than inventing one: checksummed
suites with separate training, calibration, development, and locked-test
partitions, per-record provenance, and pinned dataset revisions. Development
partitions select designs. The locked test is read once per published
candidate and the read is enforced.

Use kev's suite items where they exist. `evals/transfer-v4` covers QNLI,
SciQ, TweetEval, PAWS, MMLU, Emotion, and held-out programmatic policy
rules, and kev's leaderboard already carries kev-0.6b through kev-8b and
hosted Jev scored on those same items. Scoring Lev on the same items is what
makes the comparison in [`README.md`](README.md) real instead of rhetorical,
and it costs nothing beyond running them.

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

Gate 5 is the one that will bite. **A calibration map is only valid for the
base it was fitted against**, and the base arrives with the operating
system. An OS update that changes the base signature invalidates every map,
every adapter, and every threshold derived from them. The door refuses
rather than serving a stale map, and re-measurement is a scheduled cost,
not an incident.

## The calibration record

Each fitted family carries one committed JSON record:

- The family: question type, option cardinality band, domain, rendering.
- The estimator: L2 with `N` and the seed policy, or L3 with the band set.
- The base model signature, the OS build, and the adapter digest if one is
  attached.
- The suite reference, partition hashes, and item counts.
- The map itself: bins or the isotonic fit, with the count behind each bin.
- The scored metrics from the table above, on the evaluation partition.
- The date, and the single locked-test read if one was spent.

A family without a record does not serve probabilities. A record whose base
signature no longer matches does not serve probabilities either. That is the
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
