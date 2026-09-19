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
| The contract, the limits, the design rules | [`jev/knowledge-base.md`](jev/knowledge-base.md) |
| How a decision model works mechanically | [`kev/architecture.md`](kev/architecture.md) |
| Why Apple's runtime needs a different mechanism | [`lev/architecture.md`](lev/architecture.md) |
| What Lev is admitted and refused for | [`lev/disposition.md`](lev/disposition.md) |
| How any of these numbers were produced | [`lev/measurements/`](lev/measurements/) |

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

## Other implementations

| Document | Holds |
| --- | --- |
| [`others/2026-09-19-laya.md`](others/2026-09-19-laya.md) | Laya, an open 421M decision model targeting this contract: what it is, why its benchmark table does not support what it is used for, and the two things worth taking from it. |

## Open questions

[`research/`](research/) holds leads that have not been settled — things worth
looking into, each with what would have to be true for it to matter.
