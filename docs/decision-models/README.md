# Decision models

One document — the *state* — plus a map of typed questions goes in. One typed
answer per question comes out, with probabilities. No text is generated.

That contract is TypeSafe's `POST /v1/systemone`, and this directory holds
everything about it: the three implementations this repository maintains, the
others worth knowing about, and the open questions.

The name is deliberate. These are not classifiers. Kev's readout is a pointer
over options the caller supplies per request rather than a fixed head over
learned classes, a Noul returns a probability rather than a label, and a Score
returns a weighted position on an ordered rubric. Calling the family
"classifiers" would describe one third of it.

## The three primitives

| Type | Asks | Answer |
| --- | --- | --- |
| `Noul` | Is this true? | `noul`, a probability of yes from 0 to 1 |
| `Choice` | Which one of these named options? | `choice`, `confidence`, `probabilities` |
| `Score` | Which level on this ordered rubric? | `score`, `confidence`, `legend`, `probabilities` |

Every question in a request reads the same state and is answered on its own.
No answer becomes context for another.

## The three implementations

| | Jev | Kev | Lev |
| --- | --- | --- | --- |
| Where it runs | TypeSafe's service | this machine, our code | this machine, Apple's runtime |
| Weights | closed, hosted | open adapter and head on an open base | closed, on-device, shipped by the OS |
| Readout | direct, trained against outcomes | pointer head, cross-entropy | none; estimated from behaviour |
| Cost per request | metered | our hardware | none |
| Docs | [`jev/`](jev/) | [`kev/`](kev/) | [`lev/`](lev/) |

`crates/jev` is the client for all three. A caller picks by `base_url`.

## Where to start

| You want | Read |
| --- | --- |
| To pick one for a workload | [`choosing.md`](choosing.md) |
| The contract, the limits, the design rules | [`jev/knowledge-base.md`](jev/knowledge-base.md) |
| How a decision model works mechanically | [`kev/architecture.md`](kev/architecture.md) |
| Why Apple's runtime needs a different mechanism | [`lev/architecture.md`](lev/architecture.md) |
| What Lev is admitted and refused for | [`lev/disposition.md`](lev/disposition.md) |
| How any of these numbers were produced | [`lev/measurements/`](lev/measurements/) |
| Whether a Score's ordering means anything, door by door | [`2026-09-19-score-ordinality.md`](2026-09-19-score-ordinality.md) |
| What the one production caller's real workload does to all of this | [`2026-09-19-coder-turns.md`](2026-09-19-coder-turns.md) |
| What the newest production question costs, and what it buys | [`2026-09-19-program-selection.md`](2026-09-19-program-selection.md) |

## The rule this directory keeps

A probability is worth nothing until it is measured against outcomes, and
"measured" means on items the model did not see, under a rule fixed before
the numbers arrived.

Every document here is expected to say which of its claims are measured,
which are inferred, and which are neither. Where a number has been withdrawn
because it did not survive its own noise floor, the withdrawal stays on the
page next to the claim rather than replacing it quietly.

The machinery that enforces this lives in `crates/gym` and is surveyed in
[`../gym.md`](../gym.md).

## Measurements that cross the implementations

A run about one door stays with that door, as `lev/measurements/` does. A run
that compares them lives here, so it does not have to be written three times.

| Document | Holds |
| --- | --- |
| [`2026-09-19-score-ordinality.md`](2026-09-19-score-ordinality.md) | Whether a Score's `Σ i · p_i` is a position, on eight doors: monotonicity under an authored ramp, adjacent-versus-distant confusion, and bimodality. Jev and Lev hold; `kev-0.5b` does not. |
| [`2026-09-19-frozen-embedding-baseline.md`](2026-09-19-frozen-embedding-baseline.md) | What frozen sentence embeddings plus logistic regression do against the doors we trained: it beats Lev and Kev on the one family it can serve, loses to hosted Jev, and refuses half the suite. |
| [`2026-09-19-coder-turns.md`](2026-09-19-coder-turns.md) | `crates/coder`'s own question set, scored on real turns from recorded sessions rather than on authored support-desk items: what the workload looks like, what each question is worth against a constant, which door can afford ten-kilobyte states, and the four thresholds checked. |
| [`2026-09-19-restatement-and-polarity.md`](2026-09-19-restatement-and-polarity.md) | Whether a door can repeat a fact its state asserts, on four doors over a factorial panel: hosted Jev answers all 128, `kev-4b` gives a proposition and its negation the same probability, and every local door passes delegation plans that collide. Also why the recorded call in `devin-fan-out-six` does not replay. |
| [`2026-09-19-program-selection.md`](2026-09-19-program-selection.md) | The question every turn now asks — which program, or none. Its baseline and headroom before its accuracy, its false positives and false negatives counted apart because they cost different amounts, and where the error mass sits. |

## Other implementations

| Document | Holds |
| --- | --- |
| [`others/2026-09-19-laya.md`](others/2026-09-19-laya.md) | Laya, an open 421M decision model targeting this contract: what it is, why its benchmark table does not support what it is used for, and the two things worth taking from it. |

## Open questions

[`research/`](research/) holds leads that have not been settled — things worth
looking into, each with what would have to be true for it to matter.
