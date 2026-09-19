# Lev

**Status:** built and measured. `crates/lev` serves the System One contract
from Apple's on-device model, and the real `crates/jev` client reaches it
with a `base_url` change — the same contract `crates/kev` serves from open
weights and TypeSafe serves from its own.

Scored against the other two on 52 authored items, evaluation split, one
client:

| Door | Accuracy | ECE | Brier | Latency |
| --- | --- | --- | --- | --- |
| jev (hosted) | 0.96 | 0.099 | 0.026 | ~220 ms |
| kev-0.5b | 0.88 | 0.176 | 0.122 | ~180 ms |
| kev-4b | 0.77 | 0.188 | 0.149 | ~1 s |
| lev (Apple, N=8) | 0.85 | **0.087** | 0.127 | ~1,600 ms |

Lev has the lowest calibration error and the second-worst Brier, which is
what honestly vague numbers look like. The Kev rows are the two smallest of
four published checkpoints on 26 evaluation items; on 157 items of
`support-v2-three-way`, `kev-8b` scores 0.879 with an ECE of 0.044 and Lev
0.783, so read this table as Lev against small Kev. See
[`../kev/measurements/2026-09-19-variant-scores.md`](../kev/measurements/2026-09-19-variant-scores.md). [`disposition.md`](disposition.md) is
where to start: what Lev is admitted for, what it is refused for, and the
ranked list of what would improve it.

[`roadmap.md`](roadmap.md) holds the proposed issue sequence for review.

The name follows the pattern: Jev is the hosted model, Kev is the open
reconstruction, Lev is the Apple one.

## What it is

One document (the *state*) plus a map of typed questions goes in, one typed
answer per question comes out, and the code that asked owns the workflow.
That contract does not care what produced the answer. Lev produces it with
the model Apple ships in every recent Apple Silicon Mac, reached through the
`FoundationModels` framework.

Three properties make it worth building:

- **The weights are already there.** Nothing downloads, nothing is pinned
  to a license boundary this repository has to carry, and nothing competes
  for memory beyond what the operating system already pays. A `kev-4b`
  worker needs about 9.6 GB resident before it answers anything. A Lev
  worker needs a Mac that is already running.
- **The answer shape is guaranteed by the runtime, not by a prompt.**
  Apple's guided generation constrains decoding to a schema, so a Choice
  over an admitted option set cannot return an option that is not in the
  set. Caller text cannot add one either. That is a stronger structural
  guarantee than kev's delimiter hardening, which detects forgery rather
  than preventing it.
- **The marginal cost is zero.** No tokens are billed, nothing leaves the
  machine, and the model is resident between requests. That is the right
  economics for a judgment that runs in front of every metered agent turn.

One property makes it hard, and the whole design turns on it:

- **Apple returns no probabilities.** The framework exposes text and typed
  structured values. It exposes no logits, no log-probabilities, and no
  hidden states. Kev works because a pointer head reads the hidden state at
  a chosen position and trains against outcomes. Nothing in Apple's public
  surface permits that. Lev has to *derive* a distribution from behavior it
  can observe, and then earn the right to call the result a probability by
  measuring it. [`architecture.md`](architecture.md) is mostly about how.

## Lev is not Jev, and it is not Kev either

Kev is a laptop-scale reconstruction whose numbers come from a head trained
with cross-entropy against labelled outcomes. Its probabilities are a
learned predictive distribution, and the mechanism that produces them is
measurable end to end. Lev's numbers cannot come from there. They come from
an estimator over observable behavior plus a calibration map fitted on
labelled data, and until that map is fitted for a question family, Lev
refuses to report a probability at all.

Expect Lev to be worse than kev at the judgment and better than kev at
everything around it: availability, cost, privacy, and startup. Which kev
decides how much worse: Lev and `kev-0.5b` are 0.070 apart on our suite,
which the suite cannot resolve, while `kev-8b` is 0.096 ahead of Lev. Whether it
is good enough for any particular workflow is a measurement, and no
measurement exists yet.

## The three implementations side by side

| | Jev | Kev | Lev |
| --- | --- | --- | --- |
| Where it runs | TypeSafe's service | this machine, our code | this machine, Apple's runtime |
| Weights | closed, hosted | open adapter and head on an open base | closed, on-device, shipped by the OS |
| Readout | direct, trained against outcomes | pointer head, cross-entropy | none; estimated from behavior |
| Question isolation | packed branches, measured | block-causal mask, measured | one session per question, by construction |
| Cost per request | metered | our hardware | none |
| Probability source | the model | the model | an estimator plus a fitted calibration map |
| Shape guarantee | API validation | delimiter hardening, probed | constrained decoding, structural |
| Status here | `crates/jev`, shipped | `crates/kev`, port in progress | proposed |

## Documents here

| Document | Holds |
| --- | --- |
| [`architecture.md`](architecture.md) | What Apple's framework gives and withholds, the three ways to derive a distribution from it, the design that follows, and the wire contract mapping. |
| [`apple-fm-surface.md`](apple-fm-surface.md) | What this workspace already established about Apple FM across `openagents` and `psionic` history: the bridge contract, the router precedent, the adapter package format, and the typed error surface. |
| [`calibration.md`](calibration.md) | The rule that no Lev probability gates an action before it is measured, the suites and gates that measure it, and the record a calibrated question family has to carry. |
| [`manifest.md`](manifest.md) | The document a release is: artifact digest, base signature, contract shapes, estimator, and the calibration records each admitted family rests on. The four serving checks read it. |
| [`mesh-plan.md`](mesh-plan.md) | How a Lev worker differs from a kev worker on the earn mesh: no artifact to verify, no packing win, and a verification floor that has to move from digests to behavior. |
| [`../../training/lev-adapter/README.md`](../../training/lev-adapter/README.md) | The adapter lane: the full training process, what to train, and what counts as success. |
| [`improvement-strategy.md`](improvement-strategy.md) | Where Lev loses, why each gap has a different cause, the engineering levers with measured returns, the adapter lane in full, and what not to bother trying. |
| [`disposition.md`](disposition.md) | Where Lev is admitted and refused, the four-way scores, and six ranked improvements with the first one built and measured. |
| [`roadmap.md`](roadmap.md) | The issue sequence, the decisions as made, and the state of each step. |
| [`measurements/`](measurements/) | The behavior record, the suite scores, and the comparison runs, with the exact commands. |

## Related

- `docs/jev/knowledge-base.md` — the System One contract, the design rules,
  and the cookbook results. Lev speaks this contract.
- `docs/kev/architecture.md` — the mechanism Lev cannot use, described
  precisely enough to explain why.
- `docs/kev/model-cards.md` — the numbers Lev is measured against.
- `crates/jev` — the client that has to work against a Lev door with only a
  `base_url` change.
