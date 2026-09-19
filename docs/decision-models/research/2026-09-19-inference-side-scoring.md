# Can an inference engine's scoring endpoint replace a trained readout?

**Status:** open. Research in progress; this page records the claim and what
would have to be true for it to change what we build.

## What would change if it holds

A third mechanism for producing option probabilities, alongside the two this
repository already has:

- **Kev** trains a pointer head against labelled outcomes and reads the
  hidden state at each option's closing delimiter. Getting there took an
  adapter, a head, a frozen suite, and a conformance port.
- **Lev** has no readout at all — Apple's runtime exposes no logits — so its
  probabilities are frequencies counted over seeded samples and then
  calibrated. That costs `N` forward passes per question and its resolution
  is `1/N`.
- **This** would need neither. Point a scoring endpoint at any open model,
  hand it a candidate set, and read probabilities back.

If it works and the numbers are usable, it is cheaper than both, and the
honest conclusion would be that a large part of Kev's port bought something
obtainable another way. That is a result worth having either way.

## The claim

Quoted from the post that prompted this:

> sglang (an inference engine) offers a scoring endpoint in addition to the
> normal generation one. in scoring mode, given an input & set of possible
> answers, it forces the model to produce probabilities for each one.
>
> getting the above behavior instead of streamed output is as simple as using
> sglang's `/v1/score` endpoint instead of `/generate`. there's just one other
> trick required.
>
> for deepseek, you have to add a closing think tag before the response. this
> forces a direct answer instead of a reasoning trace.
>
> dsv4.1 flash is not as good as jev, but if we had enough spare compute to
> experiment with this same approach for a larger model then i think the
> decision quality would be at least as good, if not better.

An endpoint called `deepseek-v4.1-flash-jev` is offered as the demonstration,
and `github.com/skeptrunedev/jev-recruiter` as an application of it.

## What has to be true

Four questions decide whether this is a door, a technique, or a footnote.
They are ordered by how much they would change.

### 1. Is it calibrated, or only normalized?

Numbers that sum to one are not probabilities about the world. Kev's come
from a head trained with cross-entropy against labelled outcomes; that
training is what makes them predictive rather than merely tidy. A scoring
endpoint returns token likelihoods under the model's own distribution, which
is a different quantity.

The measurement that settles it is the one this repository already runs: ECE,
Brier, and log loss on held-out items, against a fixed gate. Until that
exists, "produces probabilities" is a statement about the output's shape.

### 2. Does it see the options together, or score them independently?

This is the sharpest mechanistic question, and the answer is not in the post.

Hume's probes of hosted Jev found options interacting: appending an
irrelevant option moved the top-two log-odds, and a reference card placed
*after* the candidates changed which earlier option won. Fixed independent
logits under a shared softmax cannot do that. Kev reproduces the behaviour
because its decision token sits after every option, so the readout sees the
whole list.

If a scoring endpoint scores each candidate independently and normalizes
afterwards, it is a different mechanism wearing the same interface — and
"none of the above", listwise effects, and anything where options qualify
each other would behave differently. That difference is testable with the
probes already in `../kev/architecture.md`.

### 3. Does it carry the other two primitives?

Choice maps onto scoring naturally: candidates are options. Noul is
expressible as two candidates. Score is the hard one — a weighted mean over
ordered levels needs the ordering to mean something, and a set of
independently scored strings has no ordering.

### 4. What does the think-tag trick generalize to?

Forcing a closing think tag before the response suppresses a reasoning trace.
That is a property of one model family's prompt format, not of scoring. Any
approach that needs a per-model incantation to stop the model reasoning has a
portability cost that should be named rather than absorbed.

## The second claim in the post

The author also argues decision models eliminate prospecting and sourcing
work in recruiting. That is a market claim rather than a technical one and it
is not what this page is about — but it is the kind of claim worth testing
against the same bar as any other: on labelled outcomes, on items the model
did not see. `jev-recruiter` is the artifact to read for whether anyone has.

## How this gets settled

Research is under way. When it returns, this page either becomes a finding
with numbers or is marked closed with the reason. If it warrants a door, the
work belongs behind `POST /v1/systemone` like everything else, and it gets
scored by `crates/gym` on the same suite as Jev, Kev, and Lev — which is
precisely the situation the Gym was built for.
