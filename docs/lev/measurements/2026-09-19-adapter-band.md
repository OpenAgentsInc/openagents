# The band adapter

Second training run. Same recipe, same suite, same 98 records — only the
objective changed. The first run trained `choice` alone; this one also trains
the ordered `certainty` band, labelled from the base model's own measured
outcomes rather than from an opinion.

Produced by `lev-band` and `lev-eval` against live hardware, evaluation split
only.

## The band, before and after

A band is useful when items in a low band are wrong more often than items in
a high band. That is a stricter test than "the field varies", because a field
that varies at random also varies.

**Base model:**

| Band | Items | Correct | Accuracy |
| --- | --- | --- | --- |
| `almost certainly not` | 16 | 15 | 0.94 |
| `unlikely` | 3 | 3 | 1.00 |
| `likely` | 77 | 61 | 0.79 |
| `almost certain` | 2 | 2 | 1.00 |

**Band adapter:**

| Band | Items | Correct | Accuracy |
| --- | --- | --- | --- |
| `unlikely` | 14 | 9 | **0.64** |
| `likely` | 45 | 37 | **0.82** |
| `almost certain` | 39 | 37 | **0.95** |

The base model's band is not weak, it is **anti-informative**. Its lowest
band scores 0.94 and its second-lowest scores 1.00, while the bulk of items —
77 of 98 — pile into `likely` at 0.79. Reading it would mislead you.

The adapter's band is monotone across all three bands it uses, with real
spread: 0.64, 0.82, 0.95. **That is a usable certainty signal, and it is the
first one Lev has had.** Trained on 98 records in about four minutes.

This closes the second acceptance criterion in
[#9363](https://github.com/OpenAgentsInc/openagents/issues/9363): the band
varies, and it separates correct from incorrect better than chance.

## What it cost

Scored with the same L2 estimator as the other doors, so the numbers compare:

| | Base | Choice adapter | Band adapter |
| --- | --- | --- | --- |
| Accuracy | 0.77 | **0.90** | 0.88 |
| ECE (raw) | 0.106 | 0.097 | 0.102 |
| Brier (raw) | 0.154 | **0.103** | 0.114 |
| Log loss (raw) | 1.952 | 2.323 | 2.601 |
| Confident errors (raw) | 6 | 8 | 9 |
| Families admitted | 1 | 2 | 2 |

Two points of accuracy and a little sharpness, given up for the band. On
98 records that is roughly what you would expect: the same capacity now has
two things to learn.

Whether the trade is worth it depends on what the caller does with the
answer. A workflow that routes on the choice alone should take the choice
adapter. A workflow that needs to know when not to trust the routing should
take the band adapter, because 0.64 against 0.95 is a real gate and two
points of accuracy is not.

## The overconfidence problem is not fixed

Log loss and confident errors are the two measures that punish confident
wrongness, and both are *worse* on the band adapter than on the choice
adapter, which was already worse than the base.

So the band is a signal the caller can read, and the *probability* is still
overconfident. Those are different things and only one of them improved. The
band tells you which answers to distrust; the distribution still reports 1.00
on some of the answers it gets wrong.

What that points at next:

- **The band belongs in the calibration map.** Right now the map is fitted on
  the L2 frequency alone. Fitting it on the pair — frequency and band
  together — is the obvious use of a signal that correlates with correctness,
  and it is a change to `calibrate.rs` rather than another training run.
- **Fewer epochs.** Four epochs on 98 records is still the prime suspect for
  the overconfidence, and `adapter-epoch2.pt` and `adapter-epoch3.pt` are
  both retained for both runs.
- **The permutation augmentation has not been trained yet.** It landed after
  this run started.

## Caveats

98 training records, 98 evaluation items, the author's labels, English, one
in-domain suite. The band's monotonicity is measured on 98 items and three
bands, so the 0.64 bucket rests on 14 of them. It is a real result and a
small one.

## Conditioning the calibration map on the band

The band is only worth training if something can use it. The obvious
consumer is the calibration map: the L2 frequency says how consistently the
model answered, the band says how reliable an answer like this is, and a
table fitted per band conditions on both.

`Map::fit_banded` fits one table per band, keeping a band's own table only
when it rests on at least fifteen observations and falling back to the
pooled table otherwise — a two-item band claiming its own probability is the
small-sample failure the admission gate exists to catch.

Fitted on the calibration split, scored on the evaluation split, band
adapter:

| Map | ECE | Brier | NLL | Confident errors |
| --- | --- | --- | --- | --- |
| raw, no map | 0.102 | 0.114 | 2.601 | 9 |
| pooled | 0.106 | 0.118 | 0.499 | 11 |
| **band-conditioned** | **0.069** | **0.104** | **0.388** | 11 |

**The pooled map does not help and the band-conditioned map does.** Pooling
leaves ECE slightly worse than the raw signal — 0.106 against 0.102 — and
would be refused. Conditioning on the band is admitted outright: ECE 0.102 to
0.069, log loss 2.601 to 0.388, Brier 0.114 to 0.104.

Log loss falling by 85% is the number that matters here, because log loss is
what punishes confident wrongness, and overconfidence was the standing
complaint against both adapters. Conditioning on the band is the first thing
that has moved it.

Two bands earned their own table, `almost certain` and `likely`. `unlikely`
held fewer than fifteen calibration items and correctly fell back to the
pool.

So the band is not decoration. It is the input that makes a Lev calibration
map work, and the case for training it does not rest on a caller reading the
band directly.

### What it did not fix

Confident errors went from nine to eleven, and the pooled map has the same
problem. A calibrated map that assigns 0.95 to the `almost certain` band will
count every wrong answer in that band as a confident error by definition.
That is the measure behaving correctly rather than the model failing, but it
means "confident errors" and "log loss" are now telling different stories and
the threshold a caller picks matters more than either number alone.

## Correction: two of the comparisons on this page are inside the noise floor

Added after `docs/lev/measurements/2026-09-19-seed-variance.md` measured what
this suite can actually detect.

Eight seed blocks over the same 98 evaluation items, same door, nothing else
changed, give an accuracy standard deviation of **0.0197**. Comparing two
doors on one block each carries both blocks' noise, so a two-sigma
difference needs **0.056 accuracy — 7.2% relative.** Below that, a
comparison is reporting which seeds it drew.

Against that floor:

| Comparison | Difference | Sigma | Reading |
| --- | --- | --- | --- |
| Base against choice adapter | 0.130 | 4.7 | real |
| Base against permutation adapter | 0.120 | 4.3 | real |
| Choice against band adapter | 0.020 | 0.7 | **inside the noise** |
| Choice against permutation adapter | 0.010 | 0.4 | **inside the noise** |
| Band against permutation adapter | 0.010 | 0.4 | **inside the noise** |

**The claim above that the band cost two points of accuracy does not hold.**
It was written as "two points of accuracy and a little sharpness, given up
for the band", and two points is 0.7 sigma. The honest statement is that the
three adapters are indistinguishable from each other on accuracy at this
suite size, and that all three are clearly better than the base.

That also means the disposition drawn from it — prefer one adapter or the
other depending on whether the caller reads the band — rested on a
difference that is not there. The preference still holds, but for the other
two reasons, which are not inside the floor:

- **The band is monotone or it is not.** 0.64/0.82/0.95 against a constant
  `likely` is a categorical difference, not a two-point one.
- **Flip rate moved 0.120 to 0.040.** That is 6 flips against 2 on 50 items.
  It has not been given its own noise measurement, and it should be: a count
  that small has a wide interval, and the honest next step is to run the
  permutation probe across seed blocks the way accuracy just was.

One more thing worth recording. **Block 0, which every published Lev number
on this page and its siblings rests on, drew 0.765 against an eight-block
mean of 0.781.** Nothing about the conclusions changes — the differences
that survive survive by four sigma — but the base model's headline number is
a slightly unlucky draw, and it was presented as the number rather than as a
draw.
