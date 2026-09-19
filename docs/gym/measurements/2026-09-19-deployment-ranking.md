# The four-way table, re-read with time and money as criteria

[`docs/lev/disposition.md`](../../lev/disposition.md) ranks four doors on
accuracy, ECE, Brier, log loss, and confident errors. `decision-v1` and
`probability-v1` judge those columns. Neither judges time or money, so
neither would refuse a door that wins two points of accuracy and costs 1.4
seconds a turn.

`deployment-v1` judges the other three columns: the latency percentile, the
price of a decision, and the share of the workload a door declines. This is
the four-way table read through it, and the answer to the question
openagents#9382 exists to ask — **does the ranking change when time is a
criterion?**

The short answer: **yes at one pair, and no gate can certify it today**, for
two reasons that are independent of each other. The record kept a median
where the criterion needs a tail, and the floor that would band a tail is
unmeasured.

## The table as recorded

52 authored items, scored on the 26-item evaluation partition, one `jev`
client and four base URLs.

| Door | Accuracy | Accuracy rank | Median latency | p95 on record | Cost lane |
| --- | --- | --- | --- | --- | --- |
| jev (hosted) | 0.96 | 1 | ~250 ms | none | metered |
| kev-0.5b | 0.88 | 2 | ~180 ms | none | `unmetered_local_lane` |
| lev (Apple, N=8) | 0.85 | 3 | ~2,100 ms | none | `unmetered_local_lane` |
| kev-4b | 0.77 | 4 | ~1,000 ms | none | `unmetered_local_lane` |

Ranked by the recorded median instead, the order is kev-0.5b, jev, kev-4b,
lev.

## Where the two orders disagree

**Lev against kev-4b.** Accuracy puts Lev eight points ahead. The recorded
median puts kev-4b ahead by more than a factor of two, and Lev's cost is
structural rather than incidental: it draws eight samples across four
helpers, and the isolation rule makes every draw pay the state cost again.
Any stated ceiling between those two medians separates the pair in the
opposite order from accuracy. That is the ranking change the issue asks
about.

**Jev against kev-0.5b** does not invert on time — 250 ms against 180 ms is
inside the noise of anything measured here — but it inverts on money in a way
worth stating, and the direction is the opposite of the intuitive one. See
below.

Nothing else moves. Jev leads on accuracy and is second on median latency;
kev-4b is last on accuracy and third on latency.

## What the gate actually says

Judged by `deployment-v1` against a router's ceilings, stated by the caller —
300 ms at p95, $0.0001 per decision, 2% of the workload declined — against
kev-0.5b as the incumbent. Verbatim from the gate:

```text
### jev (hosted) -> unverifiable
- timed_calls>=20 [passed] timed_calls is 26, at or above the floor of 20
- latency_p95_within_budget [unverifiable] the run recorded no p95; a median
  is not an answer to a tail question, and the tail is what a caller waits
  through
- cost_per_decision_within_budget [passed] $0.000018 per decision against the
  router ceiling of $0.000100, which is $1.82 against $10.00 per 100,000
  decisions
- refusal_rate_within_budget [unverifiable] 0 of 26 calls declined, 0.0%,
  against the router ceiling of 2.0%, give or take 5.5 points at 2.0 binomial
  standard errors of that ceiling

### lev (N=8) -> unverifiable
- latency_p95_within_budget [unverifiable] the run recorded no p95; a median
  is not an answer to a tail question, and the tail is what a caller waits
  through
- cost_per_decision_within_budget [unverifiable] the lane is
  unmetered_local_lane: it runs on hardware we own and nothing meters it. An
  unmetered lane has no price to compare with the router ceiling of $0.000100
  per decision, and the absence of a meter is not a price of zero
```

Four findings come out of that, and three of them are about the record rather
than about the doors.

**Not one of the four doors has a p95 on record.** The table kept a median,
and a median is the statistic that hides the tail. A router does not wait for
the median call; it waits for the slow one, and a door whose median is 180 ms
and whose p95 is 1,200 ms is a different product from one whose p95 is 240
ms. The comparison cannot be re-ranked on the criterion that decides it,
because the criterion was never recorded. That is not a defect of the gate.
It is the gate reporting what the record left out.

**One door's per-call latencies survive anywhere in this repository.**
`crates/gym/results/support-v2-three-way.jsonl` holds 157 timed calls for
`lev-base` on the 157-item `support-v2-three-way` suite: p50 1,494 ms, p95
1,612 ms, no refusals. That is a different suite from the one the four-way
table used, so it does not join the table — but it is the shape a row store
gives you for free, and it is the reason `gym eval` records `latency_ms` per
row rather than a summary. The four-way table was a paste; the rows are
evidence.

**The gate's cost column refuses the free-looking doors, not the billed one.**
Hosted Jev is the only door whose price can be checked at all, and at $1.82
per 100,000 decisions it clears a $10.00 ceiling comfortably. The three local
doors report `unmetered_local_lane` and come back `unverifiable` — not because
they are expensive but because nobody priced them. Recording them as zero
would clear every price ceiling ever stated, which is the same lie as
recording an unmeasured metric as zero, and `an_unmetered_lane_is_not_a_price_of_zero`
in `crates/gym/src/gate.rs` pins the difference: the same door written as
`Cost::Metered { usd_per_decision: 0.0 }` passes.

**A 2% refusal ceiling cannot be checked on 26 calls.** Two binomial standard
errors of a 2% rate over 26 calls is 5.5 points, so a door that declined
nothing is still `unverifiable` against that ceiling. On the 157-call store
the same band is 2.2 points. A refusal ceiling this tight needs a workload of
a few hundred calls before anyone can say it held.

## What is blocking the verdict, and what is not

Two separate things stop the gate from ranking these doors on time, and they
have different fixes.

| Blocker | Fix | Owner |
| --- | --- | --- |
| No p95 on record for any door in the table | Re-run the four-way comparison through `gym eval --record`, which keeps a `latency_ms` on every row | this repository, any quiet hour |
| No measured block-to-block spread for a latency percentile | One `gym latency` sweep on an uncontended machine | [`2026-09-19-latency-noise-floor.md`](2026-09-19-latency-noise-floor.md) |

The second one is why this document does not publish a number. The sweep that
tried to measure the floor ran while nine agents were working in this
repository, and it measured them: the median climbed in every one of eight
blocks and one block's p95 came back four times its neighbours'. A floor
derived from that afternoon, baked into a gate digest, would be wrong by
construction. `deployment-v1` records it as `unmeasured` with
`pending_measurement` inside the digest instead, so filling it produces
`deployment-v2` and leaves the verdicts recorded under `deployment-v1`
readable as what they were.

## The answer

Does the ranking change when time is a criterion? **Between Lev and kev-4b,
on the medians we have, yes.** Accuracy puts Lev third and kev-4b fourth;
time reverses them, and for a router in front of every agent turn the
reversal is the one that matters, because a door that answers in two seconds
is not a router whatever it scores.

What this work changes is not that claim, which was already readable off the
table in prose. It is that the claim now has a rule that can refuse it, a
rule whose ceilings come from the workload rather than from a constant, and a
rule that says out loud which of its own inputs are missing. Today it says
`unverifiable` four times out of four on latency. That is the correct answer
to a question asked of a record that kept the wrong statistic, on a machine
that could not be timed.
