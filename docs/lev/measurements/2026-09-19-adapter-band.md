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
