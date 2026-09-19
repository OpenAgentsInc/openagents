# Is the question text a tunable parameter?

**Status:** open. Research in progress; this page records the claim and what
would have to be true for it to change what we build.

## What would change if it holds

Every lever this repository has pulled so far changes the *model*: a LoRA
adapter, a training objective, an augmentation, a calibration map fitted
afterwards. None of them is available on hosted Jev, whose weights are
closed.

But all three doors take the same request, and two of its fields are text the
caller writes: `instructions`, which holds the judgment, and `criteria`,
which holds the answer space. **On Jev that text is the only thing a caller
can change.** If optimizing it produces real gains, it is the only lever on
the hosted door — and it is a lever we have never pulled on any door.

There is a second, sharper question behind it. The three doors share one
contract, so question text optimized against one can be sent to the others
unchanged. Whether a gain transfers is directly testable, and the answer
matters either way:

- **If it transfers**, question text is a property of the *task* and worth
  optimizing once, centrally, for all three.
- **If it does not**, question text is a property of the *model*, and every
  door needs its own — which would also mean any published comparison
  between doors is partly a comparison of how well each one's questions were
  written. Our four-way table used one question set for all four. That is
  fair in the sense that nothing was tuned for anyone, and unfair in the
  sense that it may not be equally natural to each.

## The claim

> Jev is cool, but like any foundation model it needs to be calibrated to
> your decision criteria.
>
> We launched jev-align: an open-source CLI to quickly teach Jev what good
> and bad looks like using GEPA.

— Seth Kimmel, Sutro. The tool is `github.com/sutro-sh/jev-align`.

GEPA is reflective prompt evolution with Pareto-frontier selection: propose
mutations to a text parameter, score them, keep a frontier rather than a
single best. This workspace already carries it as reference material at
`~/work/projects/repos/gepa`, and an earlier generation of our benchmark
harness had a lane for exactly this — optimizing staged text surfaces against
a development suite. That lane is recorded history rather than something
running today.

## Why the framing is right, and where it gets dangerous

The framing is correct and worth saying plainly: a decision model is not
calibrated to *your* criteria out of the box. TypeSafe's own documentation
says as much — it warns that agents write weak questions and expects a person
to edit them, and every threshold in the cookbooks is offered as an example
to re-measure rather than a value to adopt.

The danger is that optimizing text against a labelled set is fitting to that
set, and text has a great many degrees of freedom. A question reworded until
it scores well on fifty items has been fitted to fifty items, and nothing
about the process announces that.

This repository has just spent a day learning that lesson from the other
direction. A measured noise floor says a two-door accuracy comparison on our
suite needs **0.056 — 7.2% relative** to clear two sigma, and applying it
retired two published findings. An optimizer that reports a 3% gain on a
development split would be reporting which items it drew.

So the questions that decide this are less about GEPA than about discipline:

1. **What exactly is optimized** — the instruction, the criteria
   descriptions, the option names, or a preamble? Each has different
   portability. Option *names* are answer-space identity and changing them
   changes what the caller gets back.
2. **What is the objective, and on what data?** Does it hold anything out, or
   does it optimize and report on the same items?
3. **Does the output carry provenance?** A question set that scores well is
   an artifact; without a digest and a record of what produced it, a later
   reader cannot tell which text made which number. This is the same problem
   the Gym exists to solve for models, applied to text.
4. **Does a gain survive a noise floor at all?** Not "is it positive" but "is
   it larger than the spread across resamples of the same suite".

## What we would do with it

If the technique is sound, the right shape is not to adopt the CLI but to
make question text a candidate the Gym already knows how to judge. The
machinery is built: a suite with a locked partition read once, a
receipt-chained store, gates that carry their own digest, and an A/B with
confirm-on-the-metric-you-won-on. A reworded question is a candidate like any
other, and it should have to clear the same bar as a trained adapter.

That also makes the transfer question a one-command experiment: optimize
against one door, then score the resulting question set on all three and see
whether the gain follows the text or stays with the model.

The smallest honest experiment:

1. Take one family — `routing` is the largest and the only one with an
   admitted calibration map.
2. Optimize its question text against the **development** partition only.
3. Score the original and the optimized text on the **calibration**
   partition, both doors, through the store.
4. Report against the measured floor. If the gain is under 0.056 accuracy,
   it is not a gain.
5. Spend the locked partition only if steps 3 and 4 say there is something
   worth confirming.

## Where this page goes next

Research is under way on what `jev-align` actually optimizes, what it
measures, and whether its evidence is a committed evaluation or a README
example. When it returns, this page either becomes a finding or is closed
with the reason.

One thing is already clear and does not depend on the research: **we have
never optimized our question text, and we have been comparing four doors on
questions written once, by hand, without measurement.** That is a gap in our
own work regardless of whether this particular tool is any good.
