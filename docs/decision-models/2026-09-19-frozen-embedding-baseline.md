# The cheap baseline, measured

[`choosing.md`](choosing.md) ends with an uncomfortable claim: plain logistic
regression on frozen sentence embeddings reaches 0.933 on Banking77, against a
fine-tuned encoder classifier at 0.9075 and our own kev-0.5b at 0.860. It then
says that any selection guide without that baseline is skipping the cheap
answer, and that ours would have been one.

[Issue #9377](https://github.com/OpenAgentsInc/openagents/issues/9377) proposed
building the baseline as a door behind `POST /v1/systemone`. This record does
the measurement instead, because the measurement is what decides whether the
door is worth building. If frozen embeddings plus logistic regression does not
beat what we have on our own suite, no door is needed and that is the result.

**It beats Lev and `kev-0.5b` on the one family it can serve, by more than the
floor the issue names. It does not beat hosted Jev. It refuses half the suite.
And the single largest influence on its accuracy is which encoder you
downloaded, not anything about the method.**

The Kev half of that sentence needs its checkpoint named, and the naming
costs it most of its force: `kev-0.5b` is the smallest of four published
checkpoints, and against `kev-8b` the baseline's margin on `routing` falls
below the floor. See
[the correction below](#judged-against-the-measured-floor).

## What this measures

| Fact | Value |
| --- | --- |
| Suites | `support-v2` (digest `6877c24bf261d5bd`) and `support-v2-three-way` (digest `54fbf4137c3de538`) |
| Fitted on | the `calibration` partition |
| Scored on | `evaluation` for `support-v2`, `development` for `support-v2-three-way` |
| Never read | the `locked` partition of `support-v2-three-way` |
| Families served | `routing` only, 50 items on `support-v2`, 40 on `support-v2-three-way` |
| Families refused | `urgency` (Noul) and `severity` (Score) |
| Head | multinomial logistic regression, L2, scikit-learn 1.9.1 |
| Encoders | three, each pinned to a commit, plus a TF-IDF floor |
| Machine | macOS 26.4, arm64, CPU only, Python 3.12.13 |
| Cost of one measurement | about 4 seconds, including loading the encoder |

The harness is `training/baseline/`. It is Python for the reason
[`training/README.md`](../../training/README.md) gives: the exception is per
reason rather than per convenience, and here the reason is that the frozen
encoders only ship with a Python runtime. Every run is a tracked JSON record
under `training/baseline/runs/`, carrying each scored item's distribution, so
you can check a number here without rerunning anything. The metric code is a
port of `score` in `crates/gym/src/calibrate.rs` rather than a fresh
implementation, because a baseline scored by a different scorer measures the
scorer.
`training/baseline/check_panel.py` runs that port against the fixtures from
the Rust unit tests.

To reproduce every number here:

```sh
cd training/baseline
uv venv --python 3.12 .venv
uv pip install --python .venv/bin/python numpy scikit-learn sentence-transformers
./run.sh
.venv/bin/python compare.py
```

### The encoders, and why these

| Encoder | Revision | Parameters | Dimensions | Why |
| --- | --- | --- | --- | --- |
| `sentence-transformers/all-MiniLM-L6-v2` | `1110a24` | 22,713,216 | 384 | the archetypal cheap answer — small enough to run anywhere, and the model most people mean by "just use embeddings" |
| `sentence-transformers/all-mpnet-base-v2` | `e8c3b32` | 109,486,464 | 768 | the same family's stronger member, to separate the method from the model |
| `BAAI/bge-base-en-v1.5` | `a5beb1e` | 109,482,240 | 768 | a different lineage at the same size, so the spread is not one family's quirk |
| TF-IDF, word 1-2 grams | scikit-learn | 0 | 545–659 | the floor below the floor: if word counts land in the same place, no encoder is earning its download either |

Three encoders rather than one, because reporting the best of several you tried
is a selection you have to disclose. Reporting all of them turns that selection
into a measured spread, and the spread turns out to be the most interesting
number in this record.

Each encoder sees the item state and nothing else. The question text is
constant within a family, so adding it shifts every vector the same way and
cannot help a linear head.

### How the regularization strength is chosen

Inside the fitting partition, by stratified cross-validation, over a grid from
0.01 to 10,000. The scoring partition never votes.

On forty or fifty items the cross-validated loss is nearly flat above a point,
and two defensible rules disagree about where to stop:

- **`argmin`** takes the lowest cross-validated loss. The plain answer.
- **`one-se`** takes the strongest regularization whose loss is within one
  standard error of the best. The usual answer to a flat surface.

Both are reported for every run. They disagree by up to 0.025 on accuracy and
by up to 0.145 on ECE, in opposite directions, so picking one after seeing
which flattered the result would have been the whole finding. The tables below
lead with `argmin`.

## What the baseline refuses

Of the 98 scored items on `support-v2`, the baseline answers 50 and refuses 48.
On `support-v2-three-way` it answers 40 of 78 and refuses 38. Both refusals are
typed, and they are refusals rather than failures.

**`urgency`, 30 items, Noul — `unsupported_primitive`.** A Noul is the
probability that a statement holds. A classifier fitted on yes and no labels
returns the frequency of a label in its training set, which is a different
quantity that happens to have the same type. You can print it in the `noul`
field and every caller's schema validation passes.

**`severity`, 18 items, Score — `unsupported_primitive`.** A Score is a
weighted position on an ordered rubric. Multinomial logistic regression treats
its classes as unordered labels. Nothing in it keeps level 1 between level 0
and level 2, so it can put mass on levels 0 and 2 with a trough at 1 and a
weighted mean will report 1.

This is the argument [`choosing.md`](choosing.md) makes about the field, and
the argument [`others/2026-09-19-laya.md`](others/2026-09-19-laya.md) used to
decline an external model. Applying it to our own baseline is the point of
making it: a door that satisfies the type and violates the meaning is worse
than no door, and that has to be true when the door is ours.

There is a third limit worth naming even though no suite item triggers it. A
fitted head is pinned to the labels it saw. It cannot answer a caller-supplied
option set, which is the thing Kev's pointer readout exists to do. The harness
checks for it and refuses with `option_set_drift`.

## The panel

Accuracy, ECE, Brier, log loss, and confident errors, on the winning option's
reported probability, the same reduction `crates/gym/src/calibrate.rs` makes.
`SE` is the binomial standard error of the accuracy on that many items.

### `support-v2`, evaluation split, 50 `routing` items

| Featurizer | Rule | Strength | Accuracy | SE | ECE | Brier | NLL | Confident errors |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `bge-base-en-v1.5` | argmin | 100 | **0.920** | 0.038 | 0.098 | 0.092 | 0.328 | 1 |
| `bge-base-en-v1.5` | one-se | 50 | 0.920 | 0.038 | 0.127 | 0.097 | 0.342 | 1 |
| `all-mpnet-base-v2` | argmin | 10,000 | 0.900 | 0.042 | 0.073 | 0.078 | 0.454 | 2 |
| `all-mpnet-base-v2` | one-se | 100 | 0.900 | 0.042 | 0.048 | 0.090 | 0.346 | 1 |
| `all-MiniLM-L6-v2` | argmin | 100 | 0.860 | 0.049 | 0.113 | 0.099 | 0.372 | 1 |
| `all-MiniLM-L6-v2` | one-se | 50 | 0.880 | 0.046 | 0.140 | 0.103 | 0.374 | 1 |
| TF-IDF | argmin | 10,000 | 0.580 | 0.070 | 0.231 | 0.273 | 0.835 | 3 |
| TF-IDF | one-se | 100 | 0.600 | 0.069 | 0.089 | 0.235 | 0.664 | 0 |
| The fitting set's most common label | — | — | 0.280 | 0.063 | — | — | — | — |

Bootstrap 95% intervals on the headline rows: `bge` [0.840, 0.980], `mpnet`
[0.820, 0.980], `MiniLM` [0.760, 0.940], TF-IDF [0.440, 0.720]. The best any
constant answer could do on these 50 items is 0.380, which `billing` holds.

### `support-v2-three-way`, development partition, 40 `routing` items

| Featurizer | Rule | Strength | Accuracy | SE | ECE | Brier | NLL | Confident errors |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `bge-base-en-v1.5` | argmin | 100 | **0.950** | 0.034 | 0.131 | 0.068 | 0.241 | 0 |
| `bge-base-en-v1.5` | one-se | 10 | 0.950 | 0.034 | 0.258 | 0.116 | 0.396 | 0 |
| `all-mpnet-base-v2` | argmin | 50 | 0.900 | 0.047 | 0.124 | 0.073 | 0.257 | 0 |
| `all-mpnet-base-v2` | one-se | 5 | 0.925 | 0.042 | 0.263 | 0.128 | 0.422 | 0 |
| `all-MiniLM-L6-v2` | argmin | 50 | 0.900 | 0.047 | 0.114 | 0.073 | 0.263 | 0 |
| `all-MiniLM-L6-v2` | one-se | 5 | 0.900 | 0.047 | 0.259 | 0.126 | 0.422 | 0 |
| TF-IDF | argmin | 1,000 | 0.675 | 0.074 | 0.090 | 0.206 | 0.626 | 1 |
| TF-IDF | one-se | 5 | 0.700 | 0.072 | 0.207 | 0.248 | 0.689 | 0 |
| The fitting set's most common label | — | — | 0.375 | 0.077 | — | — | — | — |

The three-way partition is the honest one for a fitted thing: its builder
records that the two-way evaluation split was read for every tuning decision
made in the week to 2026-09-19, and a split read that many times is a fitting
set wearing an evaluation set's name. The `locked` partition was not read.

### The fold seed moves it barely at all

Eight fold seeds per run, the same items and the same encoder each time:

| Run | Rule | Mean | Standard deviation | Range |
| --- | --- | --- | --- | --- |
| `support-v2`, `bge` | argmin | 0.917 | 0.0066 | 0.900 to 0.920 |
| `support-v2`, `mpnet` | argmin | 0.900 | 0.0000 | 0.900 to 0.900 |
| `support-v2`, `MiniLM` | argmin | 0.863 | 0.0066 | 0.860 to 0.880 |
| `support-v2-three-way`, `bge` | argmin | 0.938 | 0.0125 | 0.925 to 0.950 |
| `support-v2-three-way`, `mpnet` | argmin | 0.900 | 0.0000 | 0.900 to 0.900 |
| `support-v2-three-way`, `MiniLM` | argmin | 0.884 | 0.0121 | 0.875 to 0.900 |

A standard deviation of 0.012 at worst, against Lev's 0.0197 over seed blocks.
The baseline's own resampling noise is smaller than Lev's, which is what you
would expect from a method whose only randomness is which items land in which
cross-validation fold.

## Against the doors

The rows below come from the per-family table in
[`../lev/measurements/2026-09-19-suite-v2-scores.md`](../lev/measurements/2026-09-19-suite-v2-scores.md),
which scored the same 50 `routing` items of `support-v2`. `lev, calibrated` is
the admitted map — the only family map in this repository that has ever passed
the admission gate.

| Door | Accuracy | SE | ECE | Brier | NLL |
| --- | --- | --- | --- | --- | --- |
| jev (hosted) | 0.940 | 0.034 | 0.060 | 0.039 | 0.122 |
| **baseline, `bge-base-en-v1.5`** | **0.920** | 0.038 | 0.098 | 0.092 | 0.328 |
| **baseline, `all-mpnet-base-v2`** | **0.900** | 0.042 | 0.073 | 0.078 | 0.454 |
| **baseline, `all-MiniLM-L6-v2`** | **0.860** | 0.049 | 0.113 | 0.099 | 0.372 |
| lev, raw | 0.820 | 0.054 | 0.140 | 0.135 | 2.860 |
| lev, calibrated | 0.820 | 0.054 | **0.024** | 0.133 | 0.431 |
| kev-0.5b | 0.780 | 0.059 | 0.120 | 0.170 | 0.502 |
| baseline, TF-IDF | 0.580 | 0.070 | 0.231 | 0.273 | 0.835 |

Do not read a point estimate without its interval. Every accuracy in that table
carries a standard error near 0.04 to 0.06, because 50 items is 50 items.

### Judged against the measured floor

[`../lev/measurements/2026-09-19-seed-variance.md`](../lev/measurements/2026-09-19-seed-variance.md)
puts the smallest difference this suite can resolve between two doors measured
on one seed block each at **0.056 accuracy, 7.2% relative**. Two published
comparisons were withdrawn this week for not clearing it.

| Comparison | Difference | Multiples of the floor | Clears the floor |
| --- | --- | --- | --- |
| `bge` against kev-0.5b | +0.140 | 2.50 | yes |
| `mpnet` against kev-0.5b | +0.120 | 2.14 | yes |
| `bge` against lev | +0.100 | 1.79 | yes |
| `mpnet` against lev | +0.080 | 1.43 | yes |
| `MiniLM` against kev-0.5b | +0.080 | 1.43 | yes |
| `MiniLM` against jev | -0.080 | 1.43 | yes |
| `MiniLM` against lev | +0.040 | 0.71 | **no** |
| `mpnet` against jev | -0.040 | 0.71 | **no** |
| `bge` against jev | -0.020 | 0.36 | **no** |
| TF-IDF against lev | -0.240 | 4.29 | yes |

By the criterion issue #9377 asks for, the baseline beats both of our own
doors and is indistinguishable from hosted Jev.

**"Both of our own doors" means Lev and `kev-0.5b`, and `kev-0.5b` is the
smallest of four Kev checkpoints.** On 2026-09-19 all four were scored on
these items, recorded in
[`../kev/measurements/2026-09-19-variant-scores.md`](../kev/measurements/2026-09-19-variant-scores.md).
Forty of the fifty `routing` items above stayed open under the three-way
partitioning, and on those forty `kev-0.5b` scores 0.800 while `kev-8b`
scores 0.875. Against `kev-8b`, `bge`'s margin is roughly +0.045 rather than
+0.140 — 0.8 floors, which does not clear. The item sets are close but not
identical, so treat that as a caution against the claim rather than a
measurement replacing it: the baseline's win over Kev is established against
the checkpoint nobody recommends serving, and is not established against the
best one.

### The floor is not the whole interval, and it matters here

The 0.056 floor covers seed resampling on 98 items. The record that produced it
says plainly that the item set is the larger of the two intervals, and the
`routing` family is 50 items rather than 98. Adding both sides' item-sample
error and asking for two standard errors of the unpaired difference:

| Comparison | Difference | Two unpaired sigma | Clears |
| --- | --- | --- | --- |
| `bge` against kev-0.5b | +0.140 | 0.140 | at the line |
| `mpnet` against kev-0.5b | +0.120 | 0.145 | no |
| `bge` against lev | +0.100 | 0.133 | no |
| `bge` against jev | -0.020 | 0.102 | no |
| TF-IDF against jev | -0.360 | 0.155 | yes |

That reading is conservative in the other direction. Both sides answered the
*same* fifty items, so the item-sample term partly cancels and a paired test
would be tighter than this. The honest statement is that the truth sits between
the two tables, and nothing here resolves it, because we do not hold Lev's and
Kev's per-item answers next to the baseline's.

**Settling it takes one thing: running the baseline as a door in the Gym, on
the same items, in the same run.** That is the door issue #9377 asked for. It
is worth building as the instrument that closes this comparison, not as a
product.

## What the panel says that accuracy does not

**The baseline wins accuracy and loses calibration.** Against the one family
Lev is admitted for, the baseline's ECE of 0.098 is four times Lev's calibrated
0.024. Lev gets there by fitting a map on 50 calibration items and passing an
admission gate; the baseline has never been calibrated at all. Calibration is
the axis [`choosing.md`](choosing.md) argues decides the most, and on it the
baseline is mid-table: better than raw Lev (0.140) and Kev (0.120), worse than
Jev (0.060) and much worse than Lev's admitted map.

**On the other probability metrics it does better than that sounds.** Brier
0.092 against Lev's 0.133 and Kev's 0.170; log loss 0.328 against Lev's
calibrated 0.431, Kev's 0.502, and raw Lev's 2.860. It produced one confident
error in 50 items. So the baseline's probabilities are sharper and more useful
than Lev's or Kev's while being less well calibrated than a fitted map — which
is the same shape as the `urgency` refusal in the scores record, where a map
bought calibration by flattening confidence.

**The regularization strength is a calibration knob nobody calls one.** On
`support-v2-three-way`, moving from `argmin` to `one-se` leaves accuracy at
0.950 and moves ECE from 0.131 to 0.258. The model becomes underconfident:
right 95 times in 100, reporting about 0.69. Nothing about that choice looks
like a calibration decision when you make it, and it dominates the calibration
panel.

**The encoder choice is worth more than the difference being argued about.**
On the same 50 items, the same head, and the same rule, the three encoders
score 0.860, 0.900, and 0.920. That 0.060 spread is 1.07 times the floor. "Use
frozen embeddings plus logistic regression" is not a number; it is a range as
wide as the entire claim, and an artifact reporting one encoder's result as
the method's result is reporting a model choice as a method finding.

**The encoder is doing the work, not the regression.**
TF-IDF on exactly the same items, head, and rule scores 0.580 against `bge`'s
0.920 — a gap of 0.34, six times the floor. The logistic regression contributes
almost nothing that the features do not already contain. The "cheap" baseline
is cheap at fit time and it still rests on a 109M-parameter encoder someone
else trained on a large corpus. It is not a smaller claim than fine-tuning; it
is the same claim with the expensive part already paid for by somebody else.

## What this settles, and what it does not

Settled:

- **The baseline belongs in the guide.** On the family it can serve it beats
  both doors we built, by the floor the issue names. Any future door on
  `routing` has to clear about 0.92 before its complexity is justified, and
  that number costs four seconds of CPU.
- **Two published comparisons should be read differently now.** Lev's 0.82 and
  Kev's 0.78 on `routing` are not a floor anyone should feel comfortable
  building on when a frozen encoder plus a linear head reaches 0.92 on the same
  items.
- **The refusals hold.** Noul and Score are not served, not degraded, and the
  refusal is the correct behavior rather than a limitation to be engineered
  around.

Not settled:

- **Whether the win survives a paired test.** The difference against Lev is
  1.79 floors and 0.75 unpaired sigma. Only a Gym run on the same items in the
  same pass will say.
- **What a calibrated baseline scores.** Nobody has fitted a map on it. Doing
  so needs a partition the baseline has not already used to fit or select, and
  the three-way suite's `locked` split is the natural candidate, read once.
- **Anything about half the suite.** 48 of 98 scored items on `support-v2` come
  back refused. A method that answers the easiest family and declines the rest
  is not comparable to a door on a suite average, and no row here reports one.
- **Anything out of domain.** `support-v2` is one author's support-desk
  taxonomy with three stable labels. The branch of the tree that frozen
  embeddings lose is a taxonomy that changes, and this suite cannot see it.

## What to do next

1. **Build the door**, as issue #9377 proposed, and register it in the Gym.
   Its purpose is to settle the paired comparison and to sit permanently next
   to every future door as the number to beat. Serve Choice; keep the typed
   refusals for Noul and Score exactly as they are.
2. **Pin an encoder and say which.** The door has to name its encoder and
   revision in every result row, because the encoder moves the answer by more
   than the floor.
3. **Update the Banking77 table in [`choosing.md`](choosing.md)** with a
   `support-v2` row, so the guide's own suite carries the same baseline it
   tells everyone else to run.
4. **Fit a calibration map on the baseline** and take it through
   `calibrate::admit`. A baseline that wins accuracy and loses calibration is
   an argument for calibrating it, not for ignoring the loss.
