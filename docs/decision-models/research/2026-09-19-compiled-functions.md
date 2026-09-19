# Should a compiler write the adapter?

**Status:** open. Research in progress. This page records the claim, why it
lands differently for us than the others reviewed this week, and the one
question that decides whether it matters.

## What would change if it holds

Every selection guide in this directory roots on the same question: **do you
have labelled outcomes?** Without them you cannot fine-tune and you cannot
verify, so you take a general model and start collecting.

ProgramAsWeights proposes a different root. Describe the function in English,
**compile it once** into a task-specific LoRA adapter for a shared small
base, and run it locally on a CPU with the network off.

If that works without labels, the root question changes and much of
[`../choosing.md`](../choosing.md) is wrong. If it needs labels, it is a
convenience over fine-tuning — possibly a very good one — and the tree
stands.

That is the whole investigation, and everything else is detail.

## The claim

> The task often stays fixed while the inputs keep changing. To me, this
> points toward a separation between "compilation" and "inference". [...] My
> bet is that we should train a larger model to do the first job by
> generating a smaller model for the second. [...] The compiler takes an
> English function description and generates a task-specific LoRA adapter for
> a shared 0.6B model. The resulting function can run locally on a CPU and be
> saved and composed with ordinary code.

> Why do we want to use a billion-parameter model just to classify if an
> email is urgent or not? And why would we want to send those inputs to an
> API hosted by someone else?

— Yuntian Deng. Code, weights, and paper published at
[programasweights.com](https://programasweights.com).

## Why this one lands differently

Three external claims were reviewed this week and two were settled against.
This one is worth more care, for reasons that are about our own position
rather than about its marketing.

**The observation underneath it is correct, and it describes us exactly.**
The task stays fixed while the inputs change. Our question set is written
once and sent against many states; `support-v2` is 196 states against three
fixed question families. Every door we run is doing the same fixed job
repeatedly. If that structure has a compilation half, we are squarely in it.

**We have already done both halves by hand, so we can price the claim.** We
trained a LoRA adapter for Apple's on-device model in **four minutes on 98
labelled records** and gained 13 points of accuracy at 4.7 sigma. That is the
baseline a compiler has to beat, and it is not a high bar in time — it is a
high bar in *labels*. The interesting question is not whether a compiler is
faster than four minutes. It is whether it needs the 98 records.

**The local, private, zero-marginal-cost argument is one we already made.**
It is most of why [`../lev/README.md`](../lev/README.md) exists. We know what
that path costs, because we paid it: Apple's runtime returns no logits, so
Lev's probabilities are counted from seeded samples at a resolution of `1/N`,
and its base certainty band was anti-informative until an adapter fixed it. A
shared 0.6B base you control has none of those problems — you can read its
distribution directly. On that axis the proposal is strictly better than Lev,
and it is worth saying so.

## The questions that decide it

**Does it need labelled data, and how much?** Stated first because nothing
else matters as much. A compiler that turns a description into a working
adapter with no examples is a different category of thing from one that
automates a fine-tune. Both are useful; only the first changes our tree.

**What is "compile" doing mechanically?** A hypernetwork emitting LoRA
weights directly, a meta-learned initialization, an LLM writing a training
config and running an ordinary fine-tune, or retrieval over pre-trained
adapters. The word covers all four and they are not close to equivalent.

**Compiled against fine-tuned, on the same task.** Beating a zero-shot base
is a much weaker result than it sounds, and it is the comparison most likely
to be the one published. The one we need is compiled against ordinary
supervised fine-tuning with the same data.

**Does it serve more than Choice?** Two systems reviewed this week were
described as decision models and served Choice alone. A Noul is a probability
that a statement holds; a Score is a weighted position on an *ordered*
rubric. Score is where most of the field falls away, structurally.

**Is the output calibrated, or merely normalized?** The repeated finding of
this week is that numbers summing to one are not probabilities about the
world. If a compiled adapter is trained on hard labels with no outcome
supervision, it will be confident and it will not be calibrated — which is
the failure mode every measurement here keeps pointing at.

## What we would do about it

If the labels answer is "few or none", the smallest experiment is already
specified by machinery we have: compile a function for the `routing` family,
serve it behind `POST /v1/systemone`, and score it on `support-v2` against
Jev, Kev, Lev, and the logistic-regression baseline from
[#9377](https://github.com/OpenAgentsInc/openagents/issues/9377). Judge it
against the measured floor — 0.056 accuracy for a two-door comparison — on
the full panel rather than on accuracy alone.

That is a day, and it would be the first paired measurement of a compiled
adapter against a trained one on a suite neither was built for.

If the labels answer is "the usual amount", it is a tooling improvement over
what we already do in four minutes, and the honest note is that we have the
harder half — the labels — and the compiler solves the easier one.

## The part worth agreeing with regardless

> I don't see why we are turning a classifier into another private API.

That is the same argument this directory makes for Lev and for Kev, and it is
the reason there are three doors behind one contract rather than one door.
Whatever the measurement says about this particular compiler, the position is
right.
