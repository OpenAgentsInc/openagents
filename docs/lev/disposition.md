# Lev: what it is good for, and how to make it better

**Status:** the disposition #9354 asked for, written against measured
numbers. Everything here comes from `docs/lev/measurements/`, produced by
`lev-eval` and `lev-compare` against live doors on one machine. Both
commands are now `gym eval` and `gym compare`; the numbers below are the ones
they produced under their old names.

## The four-way table

**Superseded on 2026-09-19 for every Kev row.** The table below scored two
Kev checkpoints on 26 evaluation items, which cannot separate two doors at
this suite's noise floor, and the two checkpoints it left out are the two
that matter. All four Kev checkpoints are now scored on 157 items of
`support-v2-three-way`, recorded as rows, in
[`../kev/measurements/2026-09-19-variant-scores.md`](../kev/measurements/2026-09-19-variant-scores.md).
Read that record for any claim about Kev. The table here stays because the
Lev and Jev rows are still the ones this page's disposition rests on.

52 authored items in `crates/lev/suites/support-v1.json` — 24 routing
Choices, 16 urgency Nouls, 12 severity Scores — split evenly into
calibration and evaluation partitions by construction. Scored on the
evaluation partition only. One `crates/jev` client, four base URLs.

| Door | Accuracy | ECE | Brier | NLL | Confident errors | Median latency |
| --- | --- | --- | --- | --- | --- | --- |
| jev (hosted) | **0.96** | 0.099 | **0.026** | **0.117** | 0 | ~250 ms |
| kev-0.5b | 0.88 | 0.176 | 0.122 | 0.388 | 0 | ~180 ms |
| kev-4b | 0.77 | 0.188 | 0.149 | 0.464 | 1 | ~1 s |
| lev (Apple, N=8) | 0.85 | **0.087** | 0.127 | 0.361 | 0 | ~2,100 ms |

Twenty-six evaluation items is a small number and these are the author's
labels, so read the table as a working instrument rather than a benchmark.
Three things in it are still worth saying.

**Jev wins on sharpness, and it is not close.** A Brier of 0.026 against
Lev's 0.127 is the whole story: Jev is both right more often and willing to
commit. That is what a readout trained against outcomes buys.

**Lev has the lowest calibration error in the table**, including against
hosted Jev. That is a genuine result and an easy one to over-read. Low ECE
with a high Brier means Lev's numbers are honestly vague: it hedges, and the
hedging happens to match how often it is right. Jev's higher ECE comes from
being confidently right, which is a better failure mode to have.

**"Lev beats kev-4b outright on this suite" — withdrawn on 2026-09-19.** The
claim rested on 0.85 against 0.77 over 26 items, a difference the suite
cannot resolve. On 157 items of `support-v2-three-way` the two doors are
0.783 and 0.745, a difference of 0.038 against a floor of 0.056, and a
paired test over the items only one of them answered correctly puts it at
*p* = 0.41. Lev and `kev-4b` are indistinguishable on our own suite.

What survives the withdrawal is the weaker reading the claim was reaching
for: on out-of-domain support text a model with no task-specific training
stays level with a small one that has some. It does not stay level with a
large one.

## The best Kev, which is not the one this page measured

| Door | Accuracy | ECE | Brier | NLL | Confident errors |
| --- | --- | --- | --- | --- | --- |
| `kev-8b` | **0.879** | **0.044** | **0.086** | **0.279** | **1** |
| `lev-base` | 0.783 | 0.107 | 0.161 | 2.353 | 12 |
| `kev-4b` | 0.745 | 0.081 | 0.148 | 0.592 | 5 |
| `kev-0.5b` | 0.713 | 0.074 | 0.186 | 0.548 | 3 |
| `kev-0.6b` | 0.675 | 0.141 | 0.181 | 0.522 | 4 |

157 items of `support-v2-three-way` at digest `54fbf4137c3de538`, both open
partitions pooled, every row in
`crates/gym/results/support-v2-three-way.jsonl`. Hosted Jev has no rows on
this suite; the record named above says why and what follows from it.

`kev-8b` beats `kev-0.5b` by 0.166, three times the floor. It leads
`lev-base` by 0.096, which is 1.7 floors and therefore not established by
this suite, and it leads on every other column of the panel as well, where
no floor has been measured at all.

The consequence for this page is narrower than the numbers look. The Kev
door Lev gets compared against is a choice, that choice has been the
smallest checkpoint, and against the largest one Lev leads nothing. What Lev
is admitted and refused for below does not change, because none of it rests
on beating Kev. Latency is the axis that might still favor the small doors,
and the run that produced these rows was too contended to measure it.

**Every column in that table is a column `decision-v1` or `probability-v1`
judges, and the latency column is not one of them.**
[`docs/gym/measurements/2026-09-19-deployment-ranking.md`](../gym/measurements/2026-09-19-deployment-ranking.md)
re-reads the table through `deployment-v1`, which judges latency, cost, and
refusal rate against ceilings the caller states. Two things it finds are
about this table rather than about the doors: the ranking inverts between Lev
and kev-4b once time is a criterion, and not one of the four doors has a p95
on record, because the median is the statistic that hides the tail a caller
waits through.

## Where Lev should be used today

Admitted:

- **Routing and triage inside an application**, where a wrong answer costs a
  re-route rather than a person. Lev scored 1.00 on the routing family with a
  raw ECE of 0.031.
- **Anything that must not leave the machine.** Nothing is sent anywhere.
- **Anything that must not be billed.** There is no marginal cost, so a
  judgment can run in front of every metered call.
- **Shape-guaranteed decisions.** Constrained decoding means the answer is
  always an admitted option. A caller never parses prose and never handles an
  option it did not offer.

Refused:

- **Anything where the probability gates the action.** No calibration map is
  admitted; see below.
- **Moderation, safety, and abuse review.** Apple's guardrails fire on the
  inputs most worth judging, and they refuse rather than answer.
- **Latency-sensitive paths that need a distribution.** Eight samples is two
  seconds.
- **Anything needing a state fenced as untrusted data.** The guardrails
  refuse delimiter-wrapped states outright, so the usual injection defense is
  unavailable.

## Why no calibration map is admitted

**Superseded on 2026-09-19.** One is: `routing` on the three-way suite, fitted
on 40 items and served by a Lev door. See
[`measurements/2026-09-19-three-way-first-rows.md`](measurements/2026-09-19-three-way-first-rows.md).
The section below records why none was, on the 52-item suite.

The machinery is built and works: a binned reliability table with Jeffreys
smoothing, a per-family fit on the calibration split, scoring on the
evaluation split, and a record per family with its verdict. What it produced:

| Family | Fitted on | Raw ECE | Mapped ECE | Verdict |
| --- | --- | --- | --- | --- |
| `routing` | 12 | 0.031 | 0.113 | refused; the raw signal was already better |
| `severity` | 6 | 0.188 | 0.132 | refused; fitted on 6 items, below the floor of 8 |
| `urgency` | 8 | 0.219 | 0.201 | refused; Brier got worse |

The gate, now `crates/gym/gates/probability-v1.json` and then
`calibrate::admit`, requires a map to beat the raw signal on items
it was not fitted on. None does, at this suite size. That is the correct
outcome rather than a disappointing one: fitting a five-bin table on a dozen
items is how a good signal gets made worse, which `routing` demonstrates by
going from 0.031 to 0.113.

Hosted Jev's map, by contrast, **is** admitted — 0.099 to 0.083 — which is a
useful sanity check that the gate is not simply impossible to pass.

## How to make Lev better, in order of expected return

### 1. Sample concurrently — done, and it returns less than it should

**Built.** `bridge::Pool` runs `k` helpers and `estimator::l2_pool` spreads
the draws over them. Sessions are already independent by construction, which
is the same property that gives question isolation, so the pool changes the
wall clock and not the answer — `tests/pool.rs` checks that the same seeds
produce the same distribution rather than assuming it.

Measured, eight samples on one question:

| Helpers | Wall clock |
| --- | --- |
| 1 | 2,365 ms |
| 4 | 1,560 ms |

A pool of four returns about 1.5x, not 4x, and the shortfall is the
interesting part. The helper processes are genuinely concurrent, so the
serialization that remains is inside Apple's runtime: the on-device model is
one shared resource and four callers queue for it. **Concurrency is bounded
by the device, not by the bridge**, which caps what this lever can ever
return and moves the remaining latency work to drawing fewer samples rather
than drawing them faster.

End to end on the eight-item comparison, a pooled door runs 1,615 ms against
2,082 ms serial, with hosted Jev at 219 ms.

### 2. Serve L1 by default and L2 only on request

Now the largest remaining latency lever, given that concurrency is capped by
the device.

Greedy decoding is deterministic and costs one call. A caller that wants a
typed choice and no distribution can have it at ~300 ms. The contract makes
this awkward — `jev::NoulAnswer` carries only a probability — so it needs an
explicit opt-in rather than a silent default, but most routing callers do not
read `probabilities` at all.

### 3. Grow the suite to the size calibration needs

The gate refused every map on 6 to 12 fitted items. A five-bin table wants on
the order of 30 items per bin to say anything, which means roughly 150 to 200
items per family rather than 12. That is authoring work, not research, and it
is the only thing standing between the existing machinery and an admitted
map.

### 4. Reduce order sensitivity by averaging over permutations

Reversing the option list flips one greedy answer in eight. Averaging an
estimate over `p` option orders removes most of that at `p` times the cost,
which composes with the concurrency work above. Kev gets this structurally
from an `option_isolation` variant; Lev has to pay for it.

**Measured more carefully on 2026-09-19.** One in eight was one pair of
option orders on one item set. Over all fifteen pairs a three-option Choice
admits, the base door flips 0.112 of its answers, plus or minus 0.069 at two
sigma, and the pairs themselves run from 0.025 to 0.175. Read the rate as
about one in nine and read any *comparison* of two flip rates against
[`measurements/2026-09-19-flip-rate-variance.md`](measurements/2026-09-19-flip-rate-variance.md)
first: no two-door difference measured on this suite has cleared its own
floor yet, including the one that was published for the permutation-augmented
adapter. The lever is still worth pulling. What it buys has not been
measured.

### 5. Train an adapter — the only route to accuracy

Accuracy of 0.85 against Jev's 0.96 is not a prompting problem. The certainty
band is constant on the base model, so there is also no band signal to
calibrate. Both need an adapter trained against labelled outcomes, which
needs Apple's adapter training toolkit. That is #9353, and it carries the
retraining treadmill Apple's release schedule imposes.

### 6. Do not chase Jev on sharpness

Lev cannot read a distribution out of the model. Every number it reports is
counted from samples, and the resolution of an `N`-sample estimate is `1/N`.
Matching Jev's Brier would take a sample count that destroys the latency
advantage that makes Lev worth having. The honest position is that Lev is a
free, private, shape-guaranteed decision with an approximate confidence, and
that Jev is what you call when the number has to be sharp.

## What would change this assessment

- An admitted calibration map on a suite of the right size would move Lev
  from "typed choice, approximate confidence" to "typed choice, measured
  confidence" and unlock the workloads listed as refused.
- An adapter that closes the accuracy gap would make the comparison a real
  contest rather than a demonstration.
- A guardrail posture that tolerates fenced states would restore the standard
  injection defense and open the moderation workloads.

None of the three is in this repository's control except the first.
