# Specialist classifiers: settled, including against our own number

**Status:** settled. The direction of the claim is right and we had
independent evidence for it already. The specific claims do not hold, the
model cannot serve the contract, and the investigation corrected a number on
our own side.

## The claim

> Jev is great at zero-shot classification, but specialist classifiers will
> dominate commercial use cases. [...] I tuned GLiNER 2.5 on a task in 51
> minutes yesterday and it crushes Jev. And it's local. And 8.8x faster.

With a second claim that `trycua` "tuned a tiny model" scoring 99.7% against
hosted Jev's 83.6% on a form-filling evaluation.

## GLiNER 2.5 cannot serve the contract

This is the finding that settles it, and it is structural rather than a
matter of quality.

`fastino/gliner2.5-base-v1` is DeBERTa-v3-base, 194M parameters, Apache-2.0.
Labels are injected into the prompt as tokens and **one shared MLP maps each
label's own embedding to one scalar** (`gliner2/classification/scoring.py:204`).

| Primitive | Served? |
| --- | --- |
| Choice | yes |
| Noul | shape available, not a probability — see calibration below |
| Score | **no** |

Score is the hard blocker. `ordinal()` exists, but the ordering is used only
for feasibility constraints and for **reporting the argmax's index**
(`gliner2/classification/result.py:133`). With levels calm, frustrated, very
frustrated it can return 0, 1, or 2. It can never return 1.4. Grepping the
repository for any expected-value computation returns nothing.

You could compute `Σ i · p_i` yourself. You should not: the training loss is
**independent per-label binary cross-entropy**, and candidate order is
**shuffled every training step by default**
(`gliner2/processor.py:252,787`). The model is trained toward permutation
invariance over the candidate list, so nothing teaches it that "very
frustrated" is further along the same axis than "frustrated". A weighted mean
over those logits would be a number with no metric content.

That shuffling also settles the option-interaction question, and not the way
either side assumed. Options **do** co-encode — all labels sit in one
sequence and the repository warns in its own source that narrowing the task
set *"changes the encoder input, so the scores change. This is a different
measurement, not a filter."* But the interaction is an incidental encoder
artifact that per-label BCE and label shuffling actively push against. Hosted
Jev's measured behaviour — a reference card placed after the candidates
changing which earlier option wins — is positional and order-sensitive, which
is precisely what this training suppresses.

**Calibration is scaffolding.** `fit_binary_temperature` and
`expected_calibration_error` exist in the repository and have **zero
non-test callers**. No Brier, no reliability diagram. The paper contains zero
instances of the string "calibrat". Probabilities are a softmax or sigmoid
applied post-hoc to independently trained binary margins; the code's own
docstring says `probability` is "presentation" and `utility` is "the
objective".

## The Banking77 numbers do not say what the post says

The chart's actual figures: fine-tuned GLiNER **90.75**, hosted Jev
**81.01**, base GLiNER **72.21**.

**That 81.01 is his own single measurement, not a published figure.**
TypeSafe has published no Banking77 number. Independent published
measurements of Jev on this benchmark span roughly **0.763 to 0.870**. So the
9.7-point margin is narrower than the spread between existing measurements of
the thing being beaten, and label-set encoding alone is known to swing Jev
about 6 points here.

**90.75 on the full 10,003-example train split is a weak result, not a
strong one.** Banking77 supervised state of the art is about 93.7–94%, and an
independent run of frozen embeddings plus plain logistic regression reaches
**0.933** on the same split. The "crushes Jev" fine-tune plausibly
underperforms a logistic-regression head.

Banking77 has no validation split, so any epoch, threshold, or learning-rate
selection was done on test or done blind. The post says neither, and there is
no published code, logs, or artifact — the claim exists as two posts and one
self-made image.

**Speed is contradicted.** The chart itself labels the comparison *"local
GLiNER vs Jev API"*, which is local inference against an internet round trip.
An independent benchmark of the same model on the same chip — an M4 Max —
measured GLiNER at ~296 ms against Jev's ~246 ms at 72 labels: **Jev
faster**. GLiNER's latency scales with label count, and Banking77's 77 labels
is its worst case. The paper's own table shows 130 ms at 5 labels rising to
208 ms at 50.

## The `trycua` claim is a replication of our own finding

It is not a fine-tune. It is a **706K-parameter byte-level transformer
trained from scratch** on ~150k synthetic examples, so it is not evidence
that fine-tuning is cheap.

Its own model card concedes the comparison is uneven — the model was trained
on the no-op convention hosted Jev was not, and the no-op sub-score is
exactly where Jev loses most. Jev scores **96%** on the decisions requiring
actual judgment.

Then the part worth reading twice. On held-out items whose field labels fall
outside its 55-concept catalogue, it scores **29.3% against 97.5%
in-distribution** — and it does not abstain. It emits "skip" on 36 of 41
held-out items **with mean confidence 0.974**. On those same items,
`jev-latest` scored **90.2%**.

In-distribution parity, out-of-domain collapse, and confident-wrong failure
instead of abstention. That is exactly what [`../kev/model-cards.md`](../kev/model-cards.md)
already records about small specialists, arrived at independently by someone
selling the opposite conclusion. It is the strongest external corroboration
of our own position that turned up today.

## The correction to our side

Our earlier reading of this compared Kev's Banking77 accuracy of **0.860**
against "hosted Jev's published 0.870". Two problems with that, both ours:

1. **There is no published figure.** 0.870 is one third-party measurement
   inside a 0.763–0.870 spread. Quoting the top of a range as *the* number
   flattered the comparison in Jev's favour, and a different quote would have
   made Kev look better. Neither is honest; the range is the fact.
2. **Kev's 0.860 is 150 items**, not the official 3,080-item test split — its
   card says 150 records per source. A 150-item accuracy has a standard error
   near 0.028, so 0.860 and 0.870 are indistinguishable, and so are 0.860 and
   Josh's 0.8101.

Both numbers in our headline comparison were less solid than the comparison
implied. The direction of the finding survives — a small specialist trained
on the task lands near a hosted generalist on it — but the precision does
not, and neither number should be quoted again without its interval.

Worth noting what that does to the specialist argument overall: Kev reaches
0.860 on **1,500** Banking77 records against Josh's 90.75 on **10,003**. Per
sample, the full-contract decision model is the more efficient specialist —
which is a better line than accuracy, where he wins and where a
logistic-regression baseline at 0.933 embarrasses us both.

## What to do

**Not a door.** A door has to serve all three primitives. This one serves
Choice, fakes Noul, and cannot serve Score at all. Building a door that
degrades Score to "nearest rung, integer only" would satisfy the type and
violate the meaning, which is worse than having no door.

Specifically it could not answer: any Score needing a value between rungs;
any Noul whose number is consumed as a probability rather than compared to a
tuned constant; anything order-sensitive across the candidate set; and
anything needing honest out-of-domain abstention.

**One idea worth borrowing: co-encoding the whole option set in a single
pass.** `[P] task ([L] ℓ1 [L] ℓ2 …) [SEP] text` with per-label readout is a
clean way to get an arbitrary caller-supplied option set into an encoder, and
the paper's numbers show the payoff against per-label passes — 208 ms at 50
labels where a per-label encoder takes 16,897 ms. Borrow the layout. Do not
borrow the per-label BCE loss or the label shuffling; those are precisely
what destroy the properties we need.

**One cheap experiment, about twenty minutes:** score a fixed text against
`[A, B]`, then against `[A, B, Z]` for an irrelevant `Z`, and compare
`logit(A) − logit(B)`. Then permute `[A, B]` to `[B, A]`. The first measures
whether options interact at all; the second measures how completely the
shuffle-training suppressed it. The repository's own warning predicts the
first moves.

## Sources

[fastino-ai/GLiNER2](https://github.com/fastino-ai/GLiNER2) at `d7c7274` ·
[arXiv:2507.18546](https://arxiv.org/abs/2507.18546) ·
[fastino/gliner2.5-base-v1](https://huggingface.co/fastino/gliner2.5-base-v1) ·
[cua-ai/cua-s1-forms](https://huggingface.co/cua-ai/cua-s1-forms) ·
[trycua/cua#3978](https://github.com/trycua/cua/issues/3978) ·
[PolyAI/banking77](https://huggingface.co/datasets/PolyAI/banking77) ·
[AbdelStark/jev-benchmarks](https://github.com/AbdelStark/jev-benchmarks) ·
[ickma2311/jev-baselines-eval](https://github.com/ickma2311/jev-baselines-eval)
