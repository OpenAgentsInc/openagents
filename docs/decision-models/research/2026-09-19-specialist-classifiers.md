# Does a fine-tuned specialist make a general decision model pointless?

**Status:** open, but the central claim is one we have already tested from
the other side. Research is under way on the specific model; this page
records what we know independently, which is more than the claim's framing
suggests.

## What would change if it holds

The argument is that a small open model, fine-tuned for an hour on one task,
beats hosted Jev on that task while running locally and faster — and that
most teams shipping to production will therefore fine-tune rather than call a
general endpoint.

If that is right in the strong form, a general decision model is a
convenience for prototypes and the product is a fine-tuning pipeline. If it
is right only in the weak form — a specialist wins on its own task and loses
elsewhere — then the interesting question is not *whether* to specialize but
*when*, and that is a measurement per workload rather than a position.

## The claim

> Jev is great at zero-shot classification, but specialist classifiers will
> dominate commercial use cases. [...] I tuned GLiNER 2.5 on a task in 51
> minutes yesterday and it crushes Jev. And it's local. And 8.8x faster.

> Default GLiNER 2.5 trailed Jev by ~9pp, despite being 10x faster. After
> fine-tuning GLiNER 2.5 on ~10k training samples (about 51 minutes on my M4
> Max), I retested. Fine-tuned GLiNER 2.5 gained ~18pp of classifying
> accuracy on the task, beating Jev by ~9pp.

— Josh Kuechly, on Banking77. A second claim cites `trycua` scoring 99.7%
against hosted Jev's 83.6% on a form-filling eval.

## We have independent evidence for the weak form

This is not a new experiment for us. **Kev** is a 0.5B model — a LoRA adapter
and a trained pointer head on a frozen Qwen2.5-0.5B — and Banking77 was in
its training mix at 1,500 records.

From [`../kev/model-cards.md`](../kev/model-cards.md):

| | Banking77 accuracy |
| --- | --- |
| kev-0.5b, held-out split | **0.860** |
| hosted Jev, published figure | 0.870 |

A half-billion-parameter model trained on the task roughly matches the hosted
generalist on it. So the claim's direction is right, and we did not need
GLiNER to learn it.

The same card says what it costs, in the same table it reports the win:

> In-distribution only — every number above is a held-out split of training
> sources. 0.5B of knowledge: BoolQ 0.75 and MNLI 0.75 are far below the
> state of the art.

And the preview family's out-of-domain numbers trail hosted Jev by 8 to 26
points depending on checkpoint size. **The specialist wins on its task and
loses everywhere else**, by a margin large enough that the trade is the whole
decision.

There is a third data point worth putting beside these, because it is the
same benchmark failing a different way. [Laya](../others/2026-09-19-laya.md),
an open 421M decision model, scores **0.425** on Banking77 against Jev's
0.870 — and diagnoses it correctly as a context budget ceiling rather than a
capability gap: 77 option descriptions do not fit in the 192 tokens its head
reserves. Banking77 punishes a small context as hard as it rewards
fine-tuning.

## What the claim does not address, and what the research is for

**Is GLiNER a decision model at all?** Its lineage is span-based named-entity
recognition — a bi-encoder scoring spans against type descriptions. That
makes it a strong *classifier*, which is one of the three primitives. A Noul
returns a probability that a statement holds; a Score returns a weighted
position on an *ordered* rubric where the ordering has to mean something. If
GLiNER does neither, then "beats Jev" is a claim about one third of the
contract, and the honest comparison is against a classifier rather than
against a decision model.

**Are the numbers probabilities or scores?** Accuracy is the easy half. The
harder question is whether the outputs are calibrated — whether an answer at
0.9 is right about nine times in ten — because that is what lets code route
on a threshold instead of just taking the argmax. Nothing in the claim
mentions calibration, and a margin passed through a sigmoid is not a
probability about the world.

**Do the options interact?** Hosted Jev demonstrably lets them: appending an
irrelevant option moves the top-two log-odds, and a reference card placed
after the candidates changes which earlier option wins. A model that scores
each label independently and normalizes afterwards cannot do that, which
matters for "none of the above", for options that qualify each other, and for
anything listwise.

**Who measured which side?** The comparison is a self-run fine-tune against a
hosted figure. Ours has the same shape and the same weakness — kev-0.5b's
0.860 is ours and Jev's 0.870 is quoted. Neither is a paired measurement, and
both should be read as indicative.

## What we would do about it

The shape this repository already believes in is that the question is per
workload, not per model — which is why the comparison harness takes
`--door name=url` and scores whatever answers the contract.

If GLiNER is worth anything to us, it is as a fourth door: wrap it behind
`POST /v1/systemone`, score it on the same suite as the other three, and let
the gate say whether it wins. That is a day of work, and the result would be
real in a way that none of these posts are, because it would be the first
paired measurement anyone has run.

The part we should take seriously regardless of GLiNER's merits is the
economics. **Fifty-one minutes on a laptop** is the number that matters in
the claim, and it is consistent with our own experience: Lev's adapter
trained in four minutes on 98 records and gained 13 points of accuracy. If a
specialist is an hour away, then "which door" stops being an architecture
decision and becomes a scheduling one — and the thing that decides it is a
measurement plane that can score a new door on a fixed suite without anyone
rewriting a benchmark. That is what `crates/gym` is.

## The honest counter-argument to our own position

Kev exists because a general open decision model seemed worth having. If the
market goes the way this claim says, most of Kev's value is in its *port* —
the packed prefill, the isolation mask, the conformance suite — rather than
in the checkpoint, and the same machinery would serve a fine-tuned specialist
just as well.

That would not make the work wasted, but it would change what it is for, and
a research page that did not say so would be flattering us.
