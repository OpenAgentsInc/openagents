# Does a Score's ordering mean anything?

A Score answer is `score = Σ i · p_i`, a probability-weighted position on an
ordered rubric. The number carries information only if the ordering does. A
model that learns its levels as independent labels has no mechanism enforcing
it: mass can land on levels 0 and 4 with a trough at 2, and the weighted mean
will report 2, a level no part of the distribution supports.

Research this week found two external decision models in exactly that
position — one trained with per-label binary cross-entropy that shuffles
candidate order every step, one averaging over arbitrary vocabulary tokens —
and closed with the line this record answers: *we should hold our own Score
numbers to the same complaint*.

The answer, measured on eight doors:

- **Hosted Jev's ordering is exact.** A perfect walk on every ramp, every
  error adjacent, no bimodal distribution, and not one item where the
  weighted mean disagrees with the level the door picked.
- **Lev's ordering is sound**, on all three Lev doors reached, even though its
  levels are stringified indices in a constrained schema. Its errors cluster
  next to the truth far above chance, and no distribution is bimodal. Its own
  mean and argmax disagree on 10 of 96 items, but seven of those are exact
  ties on an 8-sample grid rather than troughs.
- **`kev-0.5b`'s ordering is weak, and its weighted mean is not usable.**
  Twenty-six of 60 ramp distributions are bimodal. On 28 of 96 items the mean
  lands in a trough. On 47 of 96 — **half the probe** — the number a caller
  reads is not the level the model picked. Its mean score is not even
  monotone in the true level at the bottom of the scale.
- **It improves with base capacity, which is the tell.** `kev-4b` and
  `kev-8b` walk the ramp cleanly where `kev-0.5b` scatters. Nothing in the
  released training recipe teaches the ordering, so what ordering exists is
  read off the rubric text by the backbone, and a bigger backbone reads it
  better.

Produced by [`training/score-probe/`](../../training/score-probe/) on
2026-09-19, against live doors over HTTP. Ninety-six items per door, one
question per request, so no answer depends on what was packed beside it. Raw
answers are committed in `training/score-probe/results/`.

## The instrument

| Family | Items | Levels | Source |
| --- | --- | --- | --- |
| `ramp` | 60 | 5 | `training/score-probe/ramps.json`, authored for this probe |
| `severity` | 36 | 3 | the `severity` family of `crates/lev/suites/support-v2.json`, digest `6877c24bf261d5bd`, both splits |

A ramp is the part that had to be built. Twelve ramps each fix a subject —
the export button, mobile sync, the invoice PDF — and walk the true level
from 0 to 4, changing only the clause that carries the severity and holding
the rubric, the instruction, and the level order fixed. The labels are the
author's reading, chosen so the walk is as unarguable as a written scenario
allows.

The `severity` family is not a ramp: its items are independent judgments. It
is here because it is the rubric this repository already publishes numbers
against, so any correction has to land on the same items.

Three rules, fixed in `analyze.py` before any door was called:

**Monotonicity.** Within a ramp, take every pair of levels `i < j` and ask
whether the reported score is greater at `j`. Ties count as half. Twelve
ramps give 120 pairs, and `tau = 2 · concordant − 1`: 1.0 is a perfect walk,
0.0 is a coin toss.

**Confusion.** When the argmax level is wrong, does the error land on a
neighbor? Reported against the null that spreads the error uniformly over the
other levels, computed from the true levels the items actually carry. Beating
the null is the evidence. Sitting on it is the absence of evidence.

**Bimodality.** A level is a peak when it is a strict local maximum carrying
at least 0.15. A distribution is bimodal when it has two or more peaks and
the lowest level between the two largest carries at most 0.6 of the smaller
peak. Separately, the mean is *unsupported* when the level the score rounds
to carries less probability than some level below it and less than some level
above it — the weighted mean lands in the trough.

One statistic was added after the rules were fixed, because it turned out to
be the most readable of the lot: **how often the level the score rounds to is
not the level the door picked.** It is a consequence of the other three
rather than independent evidence, and it is reported as such.

Halves round up. Every door rounds the `score` field to two decimal places,
so a score of exactly 1.50 is common, and Python's default ties-to-even rule
would send 1.50 to level 2 and 2.50 back to level 2. Rounding halves up moved
two Lev counts by one item each and changed nothing else.

## The three statistics, per door

Monotonicity is from the 60 ramp items. Confusion and bimodality pool all 96.
Intervals are 95% Wilson.

| Door | tau | Steps up | Accuracy | Adjacent (null) | Bimodal | Unsupported mean | Mean ≠ argmax |
| --- | --- | --- | --- | --- | --- | --- | --- |
| hosted Jev | **+1.00** | 48/48 | 0.96 | 1.00 (0.75) | **0** [0.00, 0.04] | **0** [0.00, 0.04] | **0** [0.00, 0.04] |
| `lev-base` | +0.98 | 46/48 | 0.75 | **0.92** (0.44) | **0** [0.00, 0.04] | **0** [0.00, 0.04] | 10 [0.06, 0.18] |
| `lev-v1` | +0.91 | 41/48 | 0.71 | 0.86 (0.49) | 1 [0.00, 0.06] | 1 [0.00, 0.06] | 4 [0.02, 0.10] |
| `lev-band` | +0.86 | 40/48 | 0.82 | 0.76 (0.50) | **0** [0.00, 0.04] | **0** [0.00, 0.04] | **0** [0.00, 0.04] |
| `kev-8b` | +0.93 | 44/48 | 0.86 | 0.62 (0.62) | 5 [0.02, 0.12] | 8 [0.04, 0.16] | 17 [0.11, 0.27] |
| `kev-4b` | +0.98 | 47/48 | 0.78 | 0.71 (0.67) | 10 [0.06, 0.18] | 11 [0.07, 0.19] | 24 [0.17, 0.35] |
| `kev-0.6b` | +0.78 | 37/48 | 0.51 | 0.60 (0.56) | 14 [0.09, 0.23] | 10 [0.06, 0.18] | 30 [0.23, 0.41] |
| **`kev-0.5b`** | **+0.78** | 37/48 | 0.60 | 0.63 (0.50) | **26** [0.19, 0.37] | **28** [0.21, 0.39] | **47** [0.39, 0.59] |

Counts are out of 96 and the intervals are wide: 26 of 96 is anywhere from a
fifth to well over a third of the items. Every count in that table should be
read as evidence of a rate, not as a rate.

`lev-perm`, the adapter trained with shuffled Choice order, is the one door
of nine not measured: its Swift helper had closed its input by the time the
run reached it, and restarting another lane's server was not this probe's to
do. Its converter leaves Score levels alone by design, so it is not a
separate suspect for this question.

## Monotonicity: the mean score at each true level

The pair statistic hides where a door breaks. The mean reported score at each
true level does not.

| Door | 0 | 1 | 2 | 3 | 4 |
| --- | --- | --- | --- | --- | --- |
| hosted Jev | 0.02 | 0.89 | 1.88 | 2.92 | 4.00 |
| `lev-base` | 0.23 | 1.00 | 1.75 | 2.48 | 3.61 |
| `kev-8b` | 0.16 | 0.84 | 1.62 | 2.57 | 3.71 |
| `kev-4b` | 0.03 | 0.71 | 1.72 | 2.46 | 3.76 |
| `kev-0.6b` | 1.70 | **1.45** | 2.50 | 3.06 | 3.88 |
| `kev-0.5b` | 1.44 | **1.35** | 2.08 | 2.28 | 3.93 |

Both small kev checkpoints report a *lower* mean for level 1 than for level
0. A monotone reading of those numbers is impossible: whatever the score
means, it does not mean "further along the rubric."

The same thing happens on the three-level `severity` family, and worse:

| Door | 0 | 1 | 2 | Range across all 36 items |
| --- | --- | --- | --- | --- |
| hosted Jev | 0.03 | 0.99 | 1.91 | 0.00 – 2.00 |
| `lev-base` | 0.66 | 1.21 | 1.88 | 0.00 – 2.00 |
| `kev-8b` | 0.28 | 1.03 | 1.81 | 0.07 – 1.99 |
| `kev-0.5b` | **1.13** | **1.02** | 1.50 | **0.60 – 1.83** |

On the rubric this repository publishes Score-item numbers against,
`kev-0.5b` never reports a score below 0.60 or above 1.83. Items whose true
level is 0 and items whose true level is 2 both come back near the middle.
The scale is there in the answer and almost entirely unused.

## Confusion: kev's errors do not cluster near the truth

An ordered model errs to a neighbor. Against the uniform-error null:

| Door | Errors | Adjacent | 95% interval | Null |
| --- | --- | --- | --- | --- |
| `lev-base` | 24 | 0.92 | [0.74, 0.98] | 0.44 |
| `lev-v1` | 28 | 0.86 | [0.69, 0.94] | 0.49 |
| `lev-band` | 17 | 0.76 | [0.53, 0.90] | 0.50 |
| hosted Jev | 4 | 1.00 | [0.51, 1.00] | 0.75 |
| `kev-4b` | 21 | 0.71 | [0.50, 0.86] | 0.67 |
| `kev-0.5b` | 38 | 0.63 | [0.47, 0.77] | 0.50 |
| `kev-0.6b` | 47 | 0.60 | [0.45, 0.72] | 0.56 |
| `kev-8b` | 13 | 0.62 | [0.36, 0.82] | 0.62 |

Lev separates from its null by a margin no interval on 24 errors can
explain away, and the mean error distance says the same thing: 1.08 levels
against a null of 1.89. **No kev door does.** Every kev interval covers its
own null, `kev-8b` sits exactly on it, and the mean error distance for
`kev-8b` is 1.46 against a null of 1.44.

Hosted Jev makes only four errors on 96 items, all adjacent. Four errors
cannot carry an interval worth reporting, and the honest statement is that
Jev leaves almost nothing to measure here.

The dissociation inside kev is the interesting part. `kev-4b` and `kev-8b`
walk the ramp almost perfectly in the mean while their errors scatter like an
unordered model's. A broad distribution can have a mean that tracks the truth
and an argmax that jumps, and that is what these two are doing.

## Bimodality: where the mean reports a level nothing supports

Four `kev-0.5b` answers, straight from `results/kev-0.5b.jsonl`:

| Item | True level | Reported score | Distribution |
| --- | --- | --- | --- |
| `ramp/export-button/0` | 0 | 1.50 | 0.24, **0.35**, 0.12, **0.28**, 0.02 |
| `ramp/report-chart/3` | 3 | 1.50 | **0.51**, 0.06, 0.03, **0.22**, 0.18 |
| `ramp/notification-bell/3` | 3 | 1.64 | **0.45**, 0.04, 0.08, **0.31**, 0.13 |
| `severity/008` | 0 | 0.99 | **0.35**, 0.31, **0.34** |

Read the first row as a caller would. The score says 1.5, halfway between
"Minor" and "Impaired". The distribution says the model is torn between
"Minor" at 0.35 and "Severe" at 0.28, and thinks "Impaired" is the least
likely level of the middle three at 0.12. The number reported is the one
level the model ruled out.

`severity/008` is the same failure on the shipped rubric: 0.35 on
"Cosmetic", 0.34 on "Blocking", 0.31 in the trough between them, and a
reported score of 0.99 — "Impaired; there is a workaround" — which is the
level with the least support of the three.

Twenty-six of 60 ramp distributions are bimodal under the stated rule, and 28
of 96 items overall put the mean in a trough. For comparison, hosted Jev and
two of three Lev doors produce zero of each.

## Lev's disagreements are ties, not troughs

`lev-base` reports a mean that disagrees with its own argmax on 10 of 96
items, which is not zero and should not be read as the same failure as kev's.
Seven of the ten are exact ties on the sampling grid: with 8 samples a
probability is a multiple of 0.125, and a 4–4 split gives two levels 0.5 each
and a mean exactly between them. On `ramp/search-box/3` the distribution is
`0, 0, 0.5, 0.5, 0` and the score is 2.50. Nothing is in a trough; the model
is evenly divided, and the mean says so more faithfully than either argmax
does.

The other three are genuine: on `severity/007` the distribution is
`0.5, 0.375, 0.125` and the score is 0.62, which rounds to a level carrying
0.375 rather than the 0.5 the door picked. That is the mean and the mode
parting company on a skew, and it is the harm the weighted mean is capable of
on a door whose ordering is otherwise sound.

**A tie also makes "the argmax" a convention rather than a fact.** Nine
`lev-base` items have a tied maximum. `analyze.py` takes the first of them,
and `lev_eval`'s `max_by` takes the last, which moves `lev-base`'s accuracy
between 0.750 and 0.729. That spread is inside the 0.056 floor and changes
nothing here, but the accuracy column for a Lev door is a convention-dependent
number and this record should not pretend otherwise. No kev door and not Jev
has more than one tied item, and none of their accuracies move at all.

## What this means for each door

### `kev-0.5b`: serve the argmax, not the mean

The mechanism explains the measurement. In upstream's `kev/train.py`, a
question's loss is `cross_entropy(logits, label)`, plus a ranked probability
score for Score questions **only when `--ord_w > 0`**. The released
`kev-0.5b` predates that term, as its own card records. Plain cross-entropy
is exactly indifferent between putting the wrong mass one level away and four
levels away, so nothing in that run taught the head an ordering.

What it did have is the rubric text, rendered in level order, read by a
frozen Qwen2.5-0.5B. That is the whole of the ordering signal, and 0.5B of
backbone reads it poorly. `kev-4b` and `kev-8b`, trained the same way on
larger bases, read it well — which is consistent with the ordering being the
backbone's, not the recipe's.

**A `score` from `kev-0.5b` should not be read as a position.** On just
under half the items it disagrees with the level the model picked, and on
more than a quarter it names a level in a trough.

The argmax is what remains, and it is not blameless: this checkpoint's argmax
errors land on a neighbor 63% of the time against a null of 50%, an interval
that covers the null. But an argmax makes no claim about distance, so a
failed ordering does not damage it — it stays a level, right 0.60 of the time
on this material, which is a number the suite already reports. The weighted
mean does make that claim, and this checkpoint cannot support it. Serving the
argmax and refusing the weighted mean is the honest answer for this door.

This record does not change `crates/kev`. Its server is a conformance port of
an upstream contract that returns `score`, and dropping the field would
break that conformance; the change belongs in a decision about what our own
doors promise, not in a quiet edit to a port. What changes today is the
documentation, so nobody reads `score` from this checkpoint as a measured
position.

### Lev: the ordering holds, and the published numbers stand

Lev's Score levels are stringified indices in a constrained schema, which was
a fair reason to suspect it. The measurement clears it. `lev-base` walks the
ramp at tau +0.98 with a mean per-ramp Spearman of 0.996, errs to a
neighbor 92% of the time against a 0.44 null, and produces no bimodal
distribution on 96 items.

Two details explain why. The compiled question tells the model *"Pick the one
level that fits the state. The levels are ordered"*, and
`training/lev-adapter/convert.py` deliberately never permutes Score levels —
it permutes Choice options only, on the stated ground that a Score's ordered
levels carry meaning in their order. Whatever else Lev estimates rather than
reads, the ordering is a property of a model that understands the rubric as
English, and it survived adapter training intact.

The Score-item numbers in `docs/lev/measurements/` are scored on the argmax
level, not on the weighted mean: `lev_eval` takes the highest-probability
level as the door's answer. **Nothing in those records rests on the statistic
this probe questions, and nothing is withdrawn.**

### Hosted Jev: nothing to withdraw, and nothing much to measure

A perfect walk, four adjacent errors on 96 items, no bimodality, and no item
where the mean and the argmax disagree. Jev's Score is a position in the
sense the contract claims.

That is the strongest result in the table and also the least informative one:
a door that is right 96% of the time on this material leaves almost no errors
to characterize. The finding is that this probe cannot break Jev's Score, not
that Jev's Score cannot be broken.

## The floor, and what the counts can bear

[`2026-09-19-seed-variance.md`](../lev/measurements/2026-09-19-seed-variance.md)
puts the noise floor for a two-door accuracy comparison on this suite at
0.056. That floor is Lev's sampling noise, so it bounds what can be claimed
about Lev and about any accuracy difference, and it does not apply to kev,
whose readout is a deterministic forward pass with no seed. Read against it:

- **The `kev-0.5b` findings clear anything the floor could explain.** A tau
  of +0.78 against +1.00, a mean that falls between true level 0 and true
  level 1, and 26 bimodal distributions are shape facts about deterministic
  answers, not a two-door accuracy difference near a threshold.
- **The gaps within the Lev family — 0.98, 0.91, and 0.86 — are not a
  ranking.** Three doors measured once each on 60 ramp items, on a suite
  whose accuracy floor is 0.056, cannot separate that finely. All three say
  the same thing, which is that Lev's ordering holds.
- **Every count here is a count on 96 items, with the interval to match.**
  The distinction the record leans on is 26 against 0, not 10 against 5.

No door was asked the same item twice, so none of these numbers carries a
repeat interval of its own. Lev's answers are seeded-sampling frequencies
over 8 samples, so its probabilities come in steps of 0.125, and a peak
threshold of 0.15 is coarse against that grid. Lev's zero bimodal count is
therefore a weaker "no" than Jev's, and it is not an impossibility proof.

## What would change the reading

1. **A `kev-0.5b` retrained with `--ord_w`.** The upstream term exists and
   this probe is the test it should be held to. If the bimodality count on
   the ramp falls from 26 toward Jev's 0 and the mean at level 1 rises above
   the mean at level 0, the ordering came from the loss and the released
   checkpoint's gap is exactly what it looks like.
2. **A second author's ramps.** Twelve ramps written by one person, with one
   person's labels, is the weakest part of this instrument. The failure it
   found is a property of the distributions rather than of the labels, which
   limits the damage, but a second set would settle it.
3. **A permutation control.** The ramp presents levels in order every time.
   Whether a door's monotonicity survives reversing the rubric is a separate
   question, and a fair one to ask of Lev, whose adapter was measured
   learning position alongside judgment on Choice questions.
4. **This probe as a gate.** Bimodality rate and mean-versus-argmax
   disagreement are cheap, computed from answers already collected, and
   diagnose a failure accuracy cannot see. If they earn a place, they belong
   in `crates/gym` in Rust, not in a Python probe.

## What it cost

Eight doors, 96 items each, 768 requests. The seven local doors ran on this
machine at no cost: about 25 seconds for `kev-0.5b`, 385 for `kev-8b`, and
186 to 458 seconds for each Lev door.

Hosted Jev is metered. This run made 96 calls, 34,901 input and 1,632 output
tokens, in 51 seconds of wall time. A first pass of the same 96 items was
discarded and rerun after three words in the items were corrected to American
spelling, so the probe spent 192 hosted calls in total. The only price on
record in this repository is the cookbook's $0.000043 for one 14-question
call, which puts the whole run well under a cent at that order; the exact
charge is on TypeSafe's meter, not ours.
