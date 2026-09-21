# Choosing a decision model

A selection guide, written because four external artifacts in one week made
the same category error and one of them drew a tree worth arguing with.

Everything here is grounded in measurements recorded elsewhere in this
directory. Where a number has an interval, the interval is given, because
several of the comparisons that prompted this page turn out to be narrower
than the noise around them.

## The tree people are drawing

A widely shared version:

```text
                      NEED A DECISION
                            |
          +-----------------+-----------------+
    taxonomy stable?                  taxonomy dynamic?
          |                                   |
    lots of labels?                 simple bounded decision?
      |        |                        |            |
    YES       NO                      YES           NO
      |        |                        |            |
    BERT   EmbeddingGemma              JEV        Gemma 4
  cheapest   flexible/local        zero-shot     reasoning
  at scale     semantic             decision    + generation
```

With the summary: use a decision model for instant classification, and once
you have the data and need scale, switch to a fine-tuned encoder.

**The direction is right and we have our own evidence for it.** A small model
trained on a task lands near a hosted generalist on that task. What the tree
gets wrong is its root, its omissions, and one label.

## Four corrections

### The root question is not the taxonomy

It is **do you have labelled outcomes?**

Fine-tuning needs labels. But so does *knowing whether the zero-shot option
is working* — and the tree implicitly treats the general model as the
no-labels branch. It is, for getting started. The moment you care whether it
is right, you need labels anyway, and once you have them every other branch
opens. The no-labels branch has a second leaf since
[`2026-09-20-compiled-functions.md`](../measurements/2026-09-20-compiled-functions.md): an
adapter compiled from the question text alone scored 0.90 on `routing`
against 0.92 for the LoRA we trained on 98 labels, inside the 0.056 floor.
Take that leaf only on three conditions, each measured there. You accept a
frontier model's guess at what your labels would be, because the compiler
wrote six of its own before it mapped the adapter, and on a family whose
labels encode an outcome only you can see it has nothing to guess from. The
task is Choice, because the compiled function returns a bare string. And no
probability gates an action, because a bare string has no calibration to
measure or refuse. What our 98 labels bought on `routing` was not accuracy;
it was a distribution that passes the gate.

So the honest root demotes "taxonomy stable" to second place, where it
belongs: it decides whether you can train *repeatedly*, not whether you can
measure at all.

### Calibration appears nowhere, and it is the axis that decides the most

Every leaf in that tree is justified by cost, flexibility, or capability.
None by whether the number it returns means anything.

That is the difference between code that can route on a threshold and code
that can only take the argmax. A BERT softmax is not calibrated by default,
and neither is anything else. Of the artifacts reviewed this week, one had
calibration machinery in the repository with
[zero non-test callers](../research/2026-09-19-specialist-classifiers.md), one
[reported an ECE that was refit on its own held-out distribution](../others/2026-09-19-laya.md)
against an out-of-box competitor, and one measured it honestly and
[lost on it](../research/2026-09-19-inference-side-scoring.md).

We learned this expensively in our own work: conditioning a calibration map
on a trained certainty band cut log loss from 2.601 to 0.388, where the
pooled map did not help at all. Nothing in the tree would tell you that
problem exists.

And the cheap baseline at the bottom of this page demonstrates the split
directly. On our own `routing` family it **beats Lev by ten points of
accuracy and is four times worse calibrated** — ECE 0.098 against the 0.024
of Lev's one admitted map. Sharper, less trustworthy. A tree that ranks by
accuracy picks it; a caller that routes on a threshold should not.

### "Decision" is not "classification"

The tree picks a label at every leaf. The contract has three primitives and
only one of them is classification:

| Primitive | What it returns | Who can serve it |
| --- | --- | --- |
| Choice | one of N named options, with a distribution | almost everything |
| Noul | a probability that a statement holds | anything that can produce a calibrated scalar |
| Score | a weighted position on an *ordered* rubric | very little |

Score is where most of the field falls away, and it falls away structurally
rather than by quality. A model whose levels are arbitrary label tokens has
no mechanism keeping the ordering meaningful — it can put mass on levels 0
and 4 with a trough at 2, and a weighted mean will report 2.

That is measured, not hypothetical, and it is measured on one of ours:
`kev-0.5b` does it on 26 of 60 ramp items, where hosted Jev and Lev do it on
none. Ask which primitives a door serves, then ask whether its Score has been
held to a ramp —
[`2026-09-19-score-ordinality.md`](../measurements/2026-09-19-score-ordinality.md)
is the test.

Before adopting anything, ask which primitives it actually serves. "Beats
Jev" has meant "beats Jev at Choice" in every claim reviewed this week — and
it means the same for our own cheap baseline, which **declines 48 of the 98
items it is handed**, typed as `unsupported_primitive`, because `urgency` is
a Noul and `severity` is a Score. That is not a gap in the implementation. A
logistic head over frozen embeddings has nowhere to put an ordered rubric.

Five artifacts reviewed this week serve Choice alone. The pattern is strong
enough to state as a rule: **cheap means Choice.** Anything that serves Noul
or Score is either a decision model or a general model, and the price
difference is the reason this page exists.

### Two branches are missing

**Does the state fit?** An open 421M model scored 0.425 on Banking77 against
hosted Jev's 0.87 — not from weakness but because 77 option descriptions do
not fit the token budget its head reserves. Check the context arithmetic
before comparing quality.

**Must it stay on the machine?** Local execution is a constraint the tree
omits. Kev can return a trained distribution from open weights. Lev uses
Apple's model and estimates a distribution from observable behavior. Both
avoid an API charge, but their memory, availability, and calibration
requirements differ.

## The tree we would draw

```text
                    NEED A DECISION
                          |
              Do you have labelled outcomes?
                 |                     |
                NO                    YES
                 |                     |
      A general model, and       Which primitives?
      your first task is to        |          |
      get labels. Treat every   Choice     Noul or
      number as unverified.      only       Score
      Or a compiled adapter,       |          |
      if you accept a guessed      |          |
      label, Choice only, and      |          |
      no probability gates.        |          |
                          Everything is   A decision
                          available.      model. Most
                          Try frozen      classifiers
                          embeddings +    cannot serve
                          logistic        Score at all.
                          regression      |
                          first.          |
                                   \      /
                                    \    /
                          Does a probability gate an action?
                             |                        |
                            NO                       YES
                             |                        |
                   Accuracy is the only      Calibration on your own
                   axis. Cheapest            data, measured against a
                   accurate thing wins.      fixed rule. This is a
                             |               bigger project than
                             |               choosing a model.
                              \             /
                               \           /
                          Is the taxonomy stable?
                             |            |
                           YES           NO
                             |            |
                    Fine-tune something  Zero-shot generalist;
                    small. It will win   you cannot retrain per
                    on its task and      taxonomy.
                    lose elsewhere.
```

Overriding constraints, checked first because they eliminate branches:

- **Must stay on device, or cost nothing per call.** You will estimate
  probabilities with Lev's estimators, or read a trained distribution from
  local Kev. Both require workload measurements; local execution does not
  imply that probabilities must be estimated from generated answers.
- **The state will not fit.** Count tokens for the state plus the full option
  set before comparing anything.
- **Content that trips guardrails.** Moderation, safety, and abuse review are
  the workloads whose most interesting inputs are the ones most likely to be
  refused. Measure the refusal rate before assuming throughput.

## The reframe that matters

**The axis is not architecture. It is whether you fine-tuned, and whether you
measured.**

The tree's `lots of labels → BERT` leaf is not winning because it is BERT.
The same data in a small *decision* model lands in the same place and keeps
Noul and Score, which an encoder classifier cannot. Read that leaf as
"anything small you fine-tuned" and the tree gets both more accurate and more
useful.

Our own evidence, with its intervals, on Banking77:

| | Accuracy | Task records | Note |
| --- | --- | --- | --- |
| kev-0.5b | 0.860 | 1,500 | **150 held-out items**; standard error ≈ 0.028. The *smallest* of four checkpoints — see below |
| A fine-tuned encoder classifier | 0.9075 | 10,003 | self-reported, no published artifact |
| Frozen embeddings + logistic regression | 0.933 | 10,003 | independent |
| Supervised state of the art | ~0.94 | 10,003 | |
| Hosted Jev | 0.763–0.870 | zero-shot | **a range of independent measurements**, not one number |

Two things follow, and the second is the uncomfortable one.

A 0.5B full-contract decision model reaches 0.860 on **1,500** records where
an encoder classifier takes **10,003** to reach 0.9075. Per sample, the
decision model is the more efficient specialist.

And **plain logistic regression on frozen embeddings beats both.** It cannot
do Noul or Score, it cannot handle a dynamic taxonomy, and it is the thing to
try first anyway when the task is Choice with stable labels. Any selection
guide that does not have it as a baseline — including the tree above, and
including most of what we have built — is skipping the cheap answer.

We have since run it on our own data rather than citing someone else's, and
it holds. On the 50 `routing` items of `support-v2`, same split as every
published row:

| Door | Accuracy | SE | ECE | Serves |
| --- | --- | --- | --- | --- |
| hosted Jev | 0.940 | 0.034 | 0.060 | all three |
| **frozen `bge-base-en-v1.5` + logistic regression** | **0.920** | 0.038 | 0.098 | Choice only |
| frozen `all-mpnet-base-v2` | 0.900 | 0.042 | 0.073 | Choice only |
| frozen `all-MiniLM-L6-v2` | 0.860 | 0.049 | 0.113 | Choice only |
| Lev, calibrated | 0.820 | 0.054 | **0.024** | all three |
| kev-0.5b (smallest of four) | 0.780 | 0.059 | 0.120 | all three |
| TF-IDF + logistic regression | 0.580 | 0.070 | 0.231 | Choice only |
| most common label | 0.280 | 0.063 | — | — |

Four qualifications, and they matter as much as the ranking.

**The interval is wider than the floor suggests.** Our 0.056 figure covers
seed resampling on 98 items; `routing` is 50. Ask for two *unpaired* sigma on
an item sample and none of the wins over Lev clear. But both sides answered
the same fifty items, so a paired test would be tighter than that, and we do
not hold the per-item answers needed to run one. The truth is between the two
readings, and [#9377](https://github.com/OpenAgentsInc/openagents/issues/9377)
stays open to build the door that would settle it.

**"Frozen embeddings" is a range, not a number.** Swapping the encoder moved
accuracy 0.860 → 0.920 on identical items — a 0.060 spread, slightly *larger*
than the noise floor. The choice of downloaded encoder matters more than
anything else in the method, which also qualifies the 0.933 Banking77 row
above: it is one encoder's result, reported as the technique's.

**The work is in the encoder, not the regression.** TF-IDF with the same head
scores 0.580, six floors below. So the cheap baseline is cheap at *fit* time
and still rests on a 109M-parameter model someone else trained. It is a good
deal, not a free one, and the distinction matters when the constraint is what
runs on the device rather than what costs money to train.

The Kev comparisons in this section refer to the historical fixture
checkpoints. Upstream has replaced all three Qwen3 adapters under the same
names. The [2026-09-20 review](../../kev/2026-09-20-upstream-review.md)
recommends measuring the new 4B before carrying these conclusions forward.

**And the Kev row above is the weakest of four checkpoints, which costs this
comparison most of its force.** `kev-0.5b` is the smallest published
checkpoint, and it is the only one we had measured when this table was
written. On the 157 items of `support-v2-three-way`, `kev-8b` scores **0.879
with an ECE of 0.044**, leading the measured local panel. Hosted Jev has no
rows on that suite, so its calibration cannot be ranked on these items.
Against `kev-8b` on shared items the baseline's margin
falls from about +0.140 to roughly **+0.045, which is below the floor.**

So the honest statement is narrower than the one this section opened with:
**frozen embeddings plus logistic regression beats the models we happened to
have running, not the best model we own.** It is still the right first thing
to try, for the reasons above, and it is no longer evidence that a trained
decision model is not worth the trouble.

The same measurement retired a comparison that had been published here: *Lev
beats kev-4b outright* is withdrawn at 0.038 against a 0.056 floor, paired
*p* = 0.41. The historical `kev-4b` also did not establish an accuracy
advantage over `kev-0.5b` (+0.032, *p* = 0.55). That result does not show
equivalence and does not test the new 4B. Among the checkpoints measured
then, the larger improvement came from 4B to 8B. A card's serving
recommendation still needs measurement on the intended workload.

## What the general model is actually for

Not "instant classification." That undersells it in one way and oversells it
in another.

It is for **the period before you have labels**, for **taxonomies that change
faster than you can retrain**, and for **the long tail of one-off judgments**
that never justify a training run. It is also, on the evidence gathered this
week, the thing that still wins out of domain by a wide margin: small
specialists collapse at the distribution edge, and the ones measured this
week collapsed *confidently* — one emitting a default answer on 36 of 41
out-of-catalogue items at mean confidence 0.974, where the general model
scored 90.2% on the same items.

That failure mode is the real argument for a general model, and no tree drawn
this week has it as a branch.

## Further reading

| Document | Holds |
| --- | --- |
| [`research/2026-09-19-specialist-classifiers.md`](../research/2026-09-19-specialist-classifiers.md) | Why an encoder classifier cannot serve Score, and what happened to the specialist claims under inspection |
| [`research/2026-09-19-inference-side-scoring.md`](../research/2026-09-19-inference-side-scoring.md) | Whether an inference engine's scoring endpoint is a shortcut to a decision model |
| [`research/2026-09-19-question-text-optimization.md`](../research/2026-09-19-question-text-optimization.md) | The one lever available on a closed hosted model |
| [`2026-09-19-frozen-embedding-baseline.md`](../measurements/2026-09-19-frozen-embedding-baseline.md) | The cheap baseline measured on our own suite, with its intervals and its refusals |
| [`research/2026-09-19-compiled-functions.md`](../research/2026-09-19-compiled-functions.md) | Whether a compiler can produce a working adapter without labels, which would move this page's root |
| [`2026-09-20-compiled-functions.md`](../measurements/2026-09-20-compiled-functions.md) | The measurement: a zero-label compile lands inside the floor of our 98-label LoRA on `routing`, and what that does and does not earn |
| [`lev/disposition.md`](../../lev/disposition.md) | A worked example of admitting and refusing one model per workload |
| [`../gym.md`](../../gym.md) | The machinery that decides any of this on your own data |
