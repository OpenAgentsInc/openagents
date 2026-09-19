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
opens.

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
[zero non-test callers](research/2026-09-19-specialist-classifiers.md), one
[reported an ECE that was refit on its own held-out distribution](others/2026-09-19-laya.md)
against an out-of-box competitor, and one measured it honestly and
[lost on it](research/2026-09-19-inference-side-scoring.md).

We learned this expensively in our own work: conditioning a calibration map
on a trained certainty band cut log loss from 2.601 to 0.388, where the
pooled map did not help at all. Nothing in the tree would tell you that
problem exists.

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

Before adopting anything, ask which primitives it actually serves. "Beats
Jev" has meant "beats Jev at Choice" in every claim reviewed this week.

### Two branches are missing

**Does the state fit?** An open 421M model scored 0.425 on Banking77 against
hosted Jev's 0.87 — not from weakness but because 77 option descriptions do
not fit the token budget its head reserves. Check the context arithmetic
before comparing quality.

**Must it stay on the machine?** Free, private, and always-resident is a
category with no leaf. It is also the category where you give up reading a
probability and start estimating one, which changes everything downstream.

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
                                   |          |
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
  probabilities rather than read them, at a resolution set by how many
  samples you can afford.
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
| kev-0.5b | 0.860 | 1,500 | **150 held-out items**; standard error ≈ 0.028 |
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
| [`research/2026-09-19-specialist-classifiers.md`](research/2026-09-19-specialist-classifiers.md) | Why an encoder classifier cannot serve Score, and what happened to the specialist claims under inspection |
| [`research/2026-09-19-inference-side-scoring.md`](research/2026-09-19-inference-side-scoring.md) | Whether an inference engine's scoring endpoint is a shortcut to a decision model |
| [`research/2026-09-19-question-text-optimization.md`](research/2026-09-19-question-text-optimization.md) | The one lever available on a closed hosted model |
| [`lev/disposition.md`](lev/disposition.md) | A worked example of admitting and refusing one model per workload |
| [`../gym.md`](../gym.md) | The machinery that decides any of this on your own data |
