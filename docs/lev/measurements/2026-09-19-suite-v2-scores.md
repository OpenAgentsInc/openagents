# Suite scores: `support-v2`

196 items, 98 in the calibration split and 98 in the evaluation split, digest `6877c24bf261d5bd`.

Support-desk judgments authored in this repository. Three families with difficulty mixed on purpose: clear cases, near-boundary cases, and some that are genuinely arguable. Labels are the author's best reading, not the only defensible one, and they are not drawn from an external dataset. Sized so a calibration map has enough evidence per bin to be worth fitting.

**Agreement ceiling, added 2026-09-20.** A second reader, labelling blind, agreed with the stored labels on `routing` 50/51 (0.980, kappa 0.971), `urgency` 29/31 (0.935, kappa 0.870), and `severity` 18/18 (1.000, kappa 1.000), on a 100-item sample of these same 196 items. Read each family accuracy against that ceiling, not against 1.0; the intervals and the three disputed items are in [`../decision-models/2026-09-20-instrument-validity.md`](../decision-models/2026-09-20-instrument-validity.md).

## jev (hosted)

| Set | Accuracy | ECE | Brier | NLL | Confident errors | Items |
| --- | --- | --- | --- | --- | --- | --- |
| evaluation, raw | 0.94 | 0.078 | 0.044 | 0.158 | 0 | 98 |
| evaluation, admitted maps only | 0.94 | 0.078 | 0.044 | 0.158 | 0 | 98 |

| Family | Fitted on | Raw ECE | Mapped ECE | Raw NLL | Mapped NLL | Raw Brier | Mapped Brier | Accuracy | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `routing` | 50 | 0.060 | 0.020 | 0.122 | 0.214 | 0.039 | 0.055 | 0.94 | refused: ECE improved 0.060 to 0.020 but log loss rose 0.122 to 0.214, so the map bought calibration by hedging |
| `severity` | 18 | 0.050 | 0.023 | 0.127 | 0.219 | 0.034 | 0.053 | 0.94 | refused: ECE improved 0.050 to 0.023 but log loss rose 0.127 to 0.219, so the map bought calibration by hedging |
| `urgency` | 30 | 0.167 | 0.014 | 0.238 | 0.246 | 0.059 | 0.062 | 0.93 | refused: ECE improved 0.167 to 0.014 but log loss rose 0.238 to 0.246, so the map bought calibration by hedging |

## kev-0.5b

This is the smallest of kev's four published checkpoints and the one its own
card says not to serve. It is the only Kev door this run asked. All four are
scored on the same items in
[`../../kev/measurements/2026-09-19-variant-scores.md`](../../kev/measurements/2026-09-19-variant-scores.md),
where `kev-8b` reaches 0.879.

| Set | Accuracy | ECE | Brier | NLL | Confident errors | Items |
| --- | --- | --- | --- | --- | --- | --- |
| evaluation, raw | 0.72 | 0.092 | 0.188 | 0.561 | 2 | 98 |
| evaluation, admitted maps only | 0.72 | 0.092 | 0.188 | 0.561 | 2 | 98 |

| Family | Fitted on | Raw ECE | Mapped ECE | Raw NLL | Mapped NLL | Raw Brier | Mapped Brier | Accuracy | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `routing` | 50 | 0.120 | 0.046 | 0.502 | 0.525 | 0.170 | 0.172 | 0.78 | refused: ECE improved 0.120 to 0.046 but log loss rose 0.502 to 0.525, so the map bought calibration by hedging |
| `severity` | 18 | 0.187 | 0.171 | 0.625 | 0.671 | 0.218 | 0.239 | 0.67 | refused: ECE 0.187 to 0.171, short of the 10% reduction a map has to earn |
| `urgency` | 30 | 0.157 | 0.005 | 0.622 | 0.637 | 0.199 | 0.222 | 0.67 | refused: ECE improved 0.157 to 0.005 but log loss rose 0.622 to 0.637, so the map bought calibration by hedging |

## lev

| Set | Accuracy | ECE | Brier | NLL | Confident errors | Items |
| --- | --- | --- | --- | --- | --- | --- |
| evaluation, raw | 0.77 | 0.106 | 0.154 | 1.952 | 6 | 98 |
| evaluation, admitted maps only | 0.77 | 0.065 | 0.153 | 0.713 | 1 | 98 |

| Family | Fitted on | Raw ECE | Mapped ECE | Raw NLL | Mapped NLL | Raw Brier | Mapped Brier | Accuracy | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `routing` | 50 | 0.140 | 0.024 | 2.860 | 0.431 | 0.135 | 0.133 | 0.82 | admitted: ECE 0.140 to 0.024, log loss 2.860 to 0.431, Brier 0.135 to 0.133 on held-out items |
| `severity` | 18 | 0.097 | 0.126 | 0.487 | 0.641 | 0.172 | 0.216 | 0.67 | refused: ECE 0.097 to 0.126, short of the 10% reduction a map has to earn |
| `urgency` | 30 | 0.129 | 0.025 | 1.319 | 0.582 | 0.173 | 0.196 | 0.73 | refused: ECE improved 0.129 to 0.025 but Brier rose 0.173 to 0.196, past the 10% the binning is allowed to cost |


## How the `severity` family is scored

`severity` holds Score questions, and `lev_eval` scores one by the level with
the greatest probability, not by the weighted mean the door also returns.
Every `severity` number on this page is an argmax number.

That distinction was checked on 2026-09-19, after a probe asked whether a
Score's ordering means anything at all on each door:
[`2026-09-19-score-ordinality.md`](../../decision-models/2026-09-19-score-ordinality.md).
Lev's ordering held — tau +0.98 on a twelve-ramp walk, 92% of errors landing
on an adjacent level against a 0.44 null, and no bimodal distribution on 96
items. **Nothing on this page is withdrawn.** `kev-0.5b`'s weighted mean did
not survive the same probe, which is recorded on its card rather than here,
because its `severity` accuracy above is also an argmax number and stands.

## What this run establishes

Written after the run, against the numbers above.

**Lev has a calibrated question family for the first time.** The `routing`
map cleared the admission gate: ECE 0.140 to 0.024, log loss 2.860 to 0.431,
Brier 0.135 to 0.133 — better calibrated, far better on log loss, and no
sharpness lost. Fitted on 50 items, scored on 48 it never saw.

Across the whole evaluation split, with only admitted maps applied:

| | Raw | Calibrated |
| --- | --- | --- |
| ECE | 0.106 | **0.065** |
| Log loss | 1.952 | **0.713** |
| Brier | 0.154 | 0.153 |
| Confident errors | 6 | **1** |

Five of six confident errors gone is the number that matters for a workflow.
A confident error is the failure a threshold cannot catch, and it fell
because the map stopped the estimator reporting 1.00 on items a 50-item bin
says are right about four times in five.

**Two families still fail, for opposite reasons.** `severity` was fitted on
18 items and its map made calibration worse, which is the small-sample
failure the first suite showed throughout — it needs more items, not a better
method. `urgency` is the interesting one: its map cut ECE from 0.129 to
0.025, a fivefold improvement, and was refused anyway because Brier rose from
0.173 to 0.196, past the tenth the gate allows binning to cost.

That refusal is worth sitting with. The gate was already widened once, after
it rejected maps on a 0.02 Brier move, and widening it a second time to
admit this one would be moving the goalposts rather than fixing a rule. The
honest reading is that `urgency`'s map buys calibration by flattening
confident answers, and the Brier rise is the cost of that flattening showing
up where ECE cannot see it. The fix is more items, which is the same fix
`severity` needs.

**The harder suite separated the doors properly.** On the 52-item suite Lev
scored 0.85 and kev-0.5b 0.88. On 196 items with a third of them near a
boundary, Lev is 0.77 and kev-0.5b 0.72, against hosted Jev's 0.94. The
earlier numbers were compressed by easy items.

**Correction: it separated Jev from the other two, and it did not separate
those two from each other.** Added on 2026-09-19 after the three numbers in
that sentence were re-derived from rows in
[`../../gym/measurements/2026-09-19-reproducing-the-week.md`](../../gym/measurements/2026-09-19-reproducing-the-week.md).
All three reproduce: on the 79 items of this evaluation split that the
three-way suite leaves open, hosted Jev scores 0.949, `kev-0.5b` 0.722, and
`lev-base` 0.772. But Lev over `kev-0.5b` is 0.050 against a floor of 0.056,
and the two doors disagree on 20 items, 12 to 8 — a paired exact test puts
that at *p* = 0.50. On all 157 open items the gap is 0.070 at *p* = 0.13.
**Lev and `kev-0.5b` are indistinguishable on this suite**, and the sentence
above reads a five-point difference as a separation. Hosted Jev over Lev is
0.177 at *p* = 0.003, which is the separation this suite did buy.

That separation is between Lev and the *smallest* Kev checkpoint, and it does
not survive being asked of the others: on the three-way partitioning of these
same items, `kev-8b` scores 0.879 and Lev 0.783. Nothing about Lev's number
changes; the Kev column was reporting one checkpoint out of four.

**Hosted Jev's maps are all refused**, and that is a compliment to Jev rather
than a failure of the gate: at a raw ECE of 0.078 there is not a tenth of
improvement left for a binned table to find.

### What changes in the product

`routing` — the largest family, and the one Lev is actually admitted for —
now has a fitted map with a record naming the suite digest, the base
signature, and the scores on held-out items. A door serving that family can
report a probability with something behind it. `severity` and `urgency`
continue to refuse with `uncalibrated`, which is now a per-family answer
rather than a blanket one.
