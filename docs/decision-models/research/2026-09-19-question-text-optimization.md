# Is the question text a tunable parameter?

**Status:** the lead is still open; the tool is settled. `jev-align` is real,
competently built, and its launch claim is backed by no published evidence —
while its author's own unpublished benchmark, written two days earlier, shows
the technique *lowering* Jev's average accuracy. The idea remains worth
testing. The tool is not worth adopting, and the experiment below is still
unrun.

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

## What the tool actually does

Read at `49753df`. It optimizes the `instructions` string and the `criteria`
descriptions, and **freezes the option names** — a round-trip check rejects
any candidate whose key set differs, so an optimizer cannot silently rename a
class or change the task type. That constraint is well judged and worth
copying.

It depends on published GEPA rather than vendoring it, uses both halves of
the method, and persists carefully: atomic writes, an append-only label log,
a rewind facility, and a sha256 fingerprint over candidate plus backend
stamped onto every captured production prediction. That fingerprint is the
second thing worth copying.

Its backend abstraction is one method — `evaluate_many(candidate, stories)` —
and it already runs identical candidates against three different endpoints.
**A Kev or Lev adapter would be about sixty lines.**

## Why we should not adopt it

**Its validation set is its training set.** In the optimizer:

```python
dataset=examples,
valset=examples,
```

The number shown to the user as "the score" is computed on the very labels
the reflection was driven by. The optional holdout is off by default, and
when on it reserves `max(1, round(batch_size * 0.2))` rows — **one row per
round at the default batch of five**. There is no cross-validation, no
significance test, and nothing in the codebase computes a standard error.

The single honest sentence about this is in `AGENTS.md`, the file aimed at
coding agents rather than at users:

> Do not treat a higher training score as automatic approval. The score uses
> accumulated labels and is not a held-out generalization estimate.

**Each reflective mutation is driven by a fixed five-row minibatch** that is
cached and reused for every proposal in a round. A five-row F1 moves in
increments of 0.1 to 0.2. Our own measured floor says a two-door comparison
on this suite needs 0.056 accuracy to clear two sigma; essentially every move
at that resolution is inside our noise.

**And it is expensive in a way the README does not state:** roughly 2,300 Jev
calls in the first round and 1,300 in later ones, to buy five new labels. The
pool sweeps sit outside the metric budget, so the `--max-metric-calls` flag
does not bound spend.

## The finding that matters

Two days before launching a CLI premised on Jev needing calibration, the same
author opened a still-unmerged pull request against GEPA containing a case
study with an addendum on Jev:

| | |
| --- | --- |
| Zero-shot accuracy | **80%** |
| Average post-optimization accuracy | **77%** |
| Best single transferred prompt | 86.7% |

Their own note adds that Jev *"had the best zero-shot performance across the
models benchmarked."*

**On their own data, GEPA-transferred prompts moved Jev from 80% to 77% on
average**, and only a cherry-picked best-of-eleven beat zero-shot — on the
model they rank as least in need of calibration. The benchmark is weak in its
own right (n=30, one case is 3.3 points, self-graded against private
criteria), but it is the only evidence either way, and it points against the
pitch.

That is also the one existing datapoint on the transfer question this page
opened with: a prompt optimized against other models and transferred to Jev
**lost** three points. Evidence against free transfer, from the tool's
authors.

## What we do instead

The idea survives the tool. Question text is the only lever available on a
closed hosted model, and we have never pulled it.

The right shape is not to adopt the CLI but to make question text a candidate
the Gym already judges — `crates/gym/src/ab.rs` models a door as anything
answering the contract, so a text variant is a candidate exactly as an
adapter is. Two details are worth lifting: the **frozen component set**, so
an optimizer cannot rename a class, and the **definition fingerprint**, so a
later reader can tell which text produced which number.

The experiment stays as specified above, with one number added. Our best
measured non-text lever is the Choice adapter at **+13 points, 4.7 sigma**.
That is the bar. And it comes with its own warning, which is exactly the trap
this tool falls into wholesale: the adapter's +13 points of accuracy arrived
with *worse* log loss and *more* confident errors. **Whatever text
optimization we measure gets reported on the full metric panel, not on
accuracy alone.**

One thing was already true and does not depend on any of this: we have never
optimized our question text, and we have been comparing four doors on
questions written once, by hand, without measurement. That remains a gap in
our own work.
