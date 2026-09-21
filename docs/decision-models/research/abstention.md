# Abstention: should a model be able to say it does not know?

openagents#9383 asked four questions and wanted a design with a
recommendation, not an implementation. This document is that design. It
answers the four questions, recommends one shape, says how a door that
abstains would be scored, and says what the shape would break. Nothing here
has been built, and no door has been measured for abstention yet; the
section at the end says what to measure first.

Every number in this document is quoted from a record already in the
repository, and the record is named beside it. The two cost worked
examples are arithmetic over one published table, marked as such.

## The finding this answers

The failure mode every measurement points at is not inaccuracy. It is being
wrong at high confidence:

- Lev's base model holds a wrong answer at 0.81 as steadily as a right one
  ([`../lev/measurements/2026-09-19-behavior.md`](../../lev/measurements/2026-09-19-behavior.md)).
- Its choice adapter gained 13 points of accuracy, 0.77 to 0.90, and made
  both measures that punish confident wrongness worse: log loss 1.952 to
  2.323, confident errors six to eight
  ([`../lev/measurements/2026-09-19-adapter-v1.md`](../../lev/measurements/2026-09-19-adapter-v1.md)).
- An external specialist scored 29.3% off-catalogue while emitting its
  default answer on 36 of 41 items at mean confidence 0.974
  ([`research/2026-09-19-specialist-classifiers.md`](2026-09-19-specialist-classifiers.md)).
- Hosted Jev's edge over an open alternative was sharpest in the tail:
  wrong once in 366 answers claimed at 99% or above, against 32 in 1,610
  ([`research/2026-09-19-inference-side-scoring.md`](2026-09-19-inference-side-scoring.md)).
- An independent evaluator found that no confidence threshold recovered a
  compiled adapter's misses, because the misses sat at 0.97 to 1.00, while
  an explicit `unsure` class reached 98.9% on the cases it decided, n=100
  ([`research/2026-09-19-compiled-functions.md`](2026-09-19-compiled-functions.md)).

And one measurement says a usable "do not trust this one" signal is
buildable here: trained against the base model's own outcomes, Lev's
certainty band went from anti-informative, its lowest band scoring 0.94
and its bulk band 0.79, to monotone at 0.64 / 0.82 / 0.95, from 98 records
and four minutes of training
([`../lev/measurements/2026-09-19-adapter-band.md`](../../lev/measurements/2026-09-19-adapter-band.md)).

The contract has no way to say "I do not know." A Choice names an option, a
Noul returns a number, a Score returns a position. `lev/disposition.md`
tells a caller to route below a threshold to a person, and that is a
convention in the caller, not a property the door is scored on. A door
that abstains well cannot score better today than one that guesses.

## Recommendation in one paragraph

Abstention is an **outcome**, not an option: a per-answer, additive
`abstain` field on the response, beside the answer rather than in its
option list, that says "do not act on this answer." The door emits it from
an input-conditioned certainty signal it already has — Lev's band, or a
calibrated probability where that is all a door has — under a rule the
door commits to in its manifest, so the signal can see the input and the
caller cannot move it after the fact. The gym records it on the row and
scores it under a **cost matrix the workload supplies** through the gate's
existing `Budget`, so an abstaining door beats a guessing one only where
the workload says an escalation is cheaper than a wrong action. The caller
still owns what happens next; the caller does not own whether the model
is scored on it.

## The four questions

### Is abstention an option or an outcome?

An outcome. The two statements the issue separates are separate:

- `other` or `none` in a Choice says **none of these options fits the
  state**. It is a claim about the state, it belongs in the option list,
  the label can be right or wrong, and probability mass on it is
  probability mass about the state. The knowledge base's rule to include a
  no-match outcome stands, and the suites here already follow it.
- Abstention says **I cannot tell, or you should not act on what I would
  say**. It is a claim about the model. It has no truth label in the
  suite: an item is `routing: billing`, never `routing: unsure`. Putting
  it in the option list makes it compete with the real options for mass,
  makes the reported probabilities no longer about the state, and, as the
  issue says, is how an `other` class becomes a dumping ground.

The external `unsure` result is the strongest evidence for a named option
and it does not move this answer. That adapter returned a bare label with
no distribution, so a class was the only place an abstention could live.
Every door here returns a distribution, and Lev returns a band beside it.
Where the model has a channel outside the answer, use it; a named option
is the fallback for a model that has none. The comment on the issue asks
that the named option be measured against the outcome rather than assumed
redundant, and the last section keeps that.

### Is it a separate head?

Yes, in the sense that matters: the abstention signal lives **outside the
answer distribution**, conditioned on the input, emitted by the door. No,
in the sense of a new trained head with its own cost-weighted loss the way
Laya's `act_probability` is trained. Lev already has the head. The band
adapter's certainty band is monotone in outcome, 0.64 / 0.82 / 0.95, and
`Map::fit_banded` already conditions the calibration map on it, taking
log loss from 2.601 to 0.388 where the pooled map did nothing. That is an
abstention signal wearing different clothes, as the issue suspected. The
recommendation is to make it first-class rather than to train another.

What a door emits `abstain` from is the door's business and its
manifest's declaration:

| Door | Signal | Where the rule lives |
| --- | --- | --- |
| `lev-adapted` band doors | The certainty band. `abstain` when the band is at or below a level the manifest names. | The Lev manifest, beside `evalRef`. |
| `lev-base`, `kev-*` | The served, calibrated probability of the selected answer, under the admitted map. `abstain` below a threshold the manifest names; never on an uncalibrated frequency. | The door's manifest; refused as `Uncalibrated` where no map is admitted, as today. |
| Hosted Jev | Nothing. The door emits no `abstain`; the caller's threshold on `confidence` is what it is, and the door is scored as always acting. | Nowhere; the gym records `abstain: false`. |

The difference between the first row and a caller's threshold is the one
the issue's comment names: a band conditions on the input, so it can mark
an answer the probability is sure about. A threshold on the probability
cannot, and the external evidence is that this fails exactly where it is
needed. The second row is the honestly weaker signal, and it is listed so
the measurement can show how much weaker.

Laya's costs, `escalate: 0.5` and `wrong_act: 3.0`, are the right shape
and the wrong place. They are workload facts, so they go in the gate's
budget, not in a training objective. A door trained against one cost
ratio is admitted for one workload.

### How is it scored?

By expected cost per item, under a cost matrix the workload supplies, with
abstentions in the denominator. Three costs, all in the workload's own
unit:

| Outcome | Cost |
| --- | --- |
| Acted, answer correct | 0 |
| Acted, answer wrong | `wrong_act` |
| Abstained, either way | `abstain` |

The door's score is the mean cost over every item in the partition. It is
judged against two constants that every workload has for free, and it has
to beat both by more than the suite's noise floor:

- **Always act**: the door's own error rate times `wrong_act`. A door whose
  abstentions do not remove wrong answers loses to this.
- **Always abstain**: `abstain`. A door that abstains on everything is
  perfectly safe and ties this, and so is never admitted for abstaining.

A door is admitted for abstaining well when its mean cost is below both
constants by at least the two-sigma difference the suite can detect, and
the record prints **coverage and accuracy-given-coverage together**, never
one alone. Accuracy on decided cases is the number that always looks good;
98.9% on decided cases, on its own, is exactly the claim this rule refuses
to read.

The ratio `abstain / wrong_act` is the break-even: an abstention pays for
itself when the answer it withdrew was wrong with probability at least
that ratio. Under Laya's costs that is one in six; a band whose lowest
level is wrong more than one time in six is worth abstaining on, and one
that is wrong less often is not. That is why the costs are a gate input:
the same door and the same band admit under a triage workload and refuse
under a workload where an escalation is as dear as a mistake.

**Two worked examples, from one published table.** The band adapter's
band table in `adapter-band.md`, 98 evaluation items, `unlikely` 14 items
with 9 correct, `likely` 45 with 37, `almost certain` 39 with 37: 83
correct, 15 wrong. Suppose the door abstains on `unlikely`. Coverage is
84 of 98; accuracy given coverage is 74 of 84, 0.881, against 83 of 98,
0.847, over all items. The table's own totals are used here rather than
the page's 0.88 headline, which comes from a different scoring pass on the
same page.

| Costs (`abstain`, `wrong_act`) | Always act | Abstain on `unlikely` | Always abstain | Verdict |
| --- | --- | --- | --- | --- |
| 0.5, 3.0 (Laya's) | 45 / 98 = 0.459 | (10 × 3.0 + 14 × 0.5) / 98 = 0.378 | 0.500 | beats both |
| 1.0, 1.0 | 15 / 98 = 0.153 | (10 + 14) / 98 = 0.245 | 1.000 | loses to always act |

Same door, same band, opposite verdicts, decided by the workload. Neither
row is a measurement of an abstaining door: no door has emitted `abstain`
yet, the `unlikely` bucket rests on 14 items, and the suite's two-sigma
floor is 0.056 accuracy, 7.2% relative
([`../lev/measurements/2026-09-19-seed-variance.md`](../../lev/measurements/2026-09-19-seed-variance.md)).
They show the shape of the rule, not a result under it.

In `crates/gym` this is:

- Two optional fields on `gate::Budget`, `cost_wrong_act` and
  `cost_abstain`, with a `costs_source` beside them. `Budget` is the
  caller's workload statement and sits outside the rule digest, so adding
  them re-judges nothing and orphans no row. Where either is absent the
  abstention criterion is `Unverifiable`, as a missing latency ceiling is
  today.
- One additive field on `row::Row`, `abstained: bool`, default `false`.
  An abstained row keeps `answered: true`, its `selected`, its
  `distribution`, and its `raw_top`, so every existing metric still reads
  it: accuracy, ECE, Brier, and log loss are computed over all rows as
  now, and coverage and accuracy-given-coverage are computed beside them.
  `Row::correct` is unchanged; the door still said something, and whether
  it was right is still a fact.
- A refusal stays a refusal. `refusal` is the door failing to answer and
  is priced by `max_refusal_rate`; `abstained` is the model answering and
  saying not to act. A row carries at most one of them, and `Row::check`
  enforces it as it enforces `answered` against `refusal` today.

### Does it belong in the contract?

As an **additive response extension**, not a fourth primitive and not a
new field on the request. On a Choice, Noul, or Score answer:

```json
{
  "choice": "billing",
  "confidence": 0.71,
  "probabilities": { "billing": 0.71, "shipping": 0.29 },
  "abstain": true,
  "abstain_basis": "band:unlikely"
}
```

`abstain` is absent from a door that has not adopted it, and absent reads
as `false`. `abstain_basis` is a short label the door's manifest defines,
so a row can say which signal spoke without the reader loading the
manifest. The answer stays: an abstaining door still tells the caller what
it would have said, so a caller that escalates can hand the person a
suggestion.

The other three options the issue lists, and why not:

- **A `refusal` outcome the door already types.** Reuse would be free and
  wrong. Refusals mean the door could not answer — off, uncalibrated,
  guardrailed — and the gym counts them against a workload's refusal
  ceiling. An abstention is an answer with a warning on it; scoring it as a
  failure to answer hides the one thing worth scoring, whether the warning
  was right.
- **A fourth field on the request or a new primitive.** Breaks every client,
  including `crates/jev`, for nothing the response field does not give.
- **Leave it to the caller and do not score it.** This is where we are, and
  it is the outcome the issue wanted written down if it were right. It is
  not, for one reason: the caller's threshold is applied to a number the
  model reports after the fact, and the evidence is that the failures sit
  where that number is highest. A caller can still own the decision; only
  a door can own the signal, and only the gym can say whether the signal
  was any good.

## What it would break

**Nothing that reads the wire today.** `crates/jev`'s answer types
deserialize with serde's default of ignoring unknown fields, so a response
that carries `abstain` parses in every current client, and a client that
does not read it behaves as it does now. That is the compatibility
argument for the field and also its cost: **a caller that ignores
`abstain` acts on an answer the door told it not to act on**, and nothing
in the transport stops it. The gym record for such a door must say so.

**The row schema, additively.** `abstained` defaults to `false` on every
row written before it exists. Rows are pinned by suite, question, and gate
digests, none of which change. `Row::check` grows one rule.

**The gate's budget, additively.** Two optional costs and their source.
The budget is outside the rule digest; gate identities and every recorded
verdict stand.

**The reading of a headline accuracy.** Once a door can abstain, its
accuracy over decided items is not comparable to a door's accuracy over
all items, and a table that prints one beside the other misleads. Every
record for an abstaining door prints coverage and accuracy-given-coverage
together, and the expected cost under named costs, or it prints the
all-items accuracy the way records do now.

**Lev's manifest and the band doors.** A band door that emits `abstain`
declares the band level it abstains at, and a change to that level is a
new door version, as a change to `evalRef` is. The band adapter today
admits nothing and serves uncalibrated frequencies; an abstention drawn
from the band is admissible before the probability is, because the band's
monotonicity is measured and the frequency's calibration is not, and the
disposition should say that in those words.

**Suites do not change.** No `unsure` label is added to any item. The
locked partitions stay what they are, and `support-v2-unseen` is the one
that can confirm an adapted door
([`../gym/measurements/2026-09-20-relocking-support-v2.md`](../../gym/measurements/2026-09-20-relocking-support-v2.md)).

## What to measure before building it

The issue's comment asked that a named `unsure` option be measured against
an outcome rather than assumed redundant. The comparison, on the
`development` partition of `support-v2-unseen`, never the locked one:

1. **Band-derived abstention**: the band adapter, `abstain` on
   `unlikely`, then on `unlikely` or `likely`.
2. **Probability-threshold abstention**: `lev-base@1` under its admitted
   `routing` map, `abstain` below a sweep of thresholds on the served
   probability.
3. **A named option**: the same items with `unsure` appended to every
   option list, scored with `unsure` read as an abstention and the label
   left as it is.

Each reports coverage, accuracy-given-coverage, and expected cost under at
least two cost pairs, one of them Laya's, beside the always-act and
always-abstain constants. The win, if there is one, has to clear 0.056
accuracy or the equivalent in cost, and the record says which pairs it
clears and which it does not.

This needs Apple hardware for the Lev doors. A Linux CPU box can run the
`kev` row of the probability-threshold arm and the always-act constants,
and nothing else here.

## What this document is

Measured: every number in "The finding this answers" and the band table,
each from the named record. Derived: the two cost rows, arithmetic over
that table, and the break-even ratio. Neither: the recommendation, which
is an argument, and the compatibility claim about `crates/jev`, which is
read from the source and would be tested by the first response that
carries the field.
