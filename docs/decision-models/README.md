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
| Docs | [`jev/`](jev/) | [`../kev/`](../kev/) | [`../lev/`](../lev/) |

`crates/jev` is the client for all three. A caller picks by `base_url`.
Select a model explicitly when comparing variants and record its artifact
identity. The [2026-09-20 Kev review](../kev/2026-09-20-upstream-review.md)
finds that the current upstream 0.6B, 4B, and 8B weights differ from the
ones pinned here, and recommends evaluating the new 4B on Coder's current
questions before changing the default. That [evaluation](../kev/measurements/2026-09-20-candidate-4b.md) now finds a promising shell-outcome candidate, input coverage failures for action, and failed program-selection acceptance. The default remains unchanged.

## Where to start

| You want | Read |
| --- | --- |
| To pick one for a workload | [`choosing.md`](choosing.md) |
| The contract, the limits, the design rules | [`jev/knowledge-base.md`](jev/knowledge-base.md) |
| How a decision model works mechanically | [`../kev/architecture.md`](../kev/architecture.md) |
| What the new Kev release changes here | [`../kev/2026-09-20-upstream-review.md`](../kev/2026-09-20-upstream-review.md) |
| Why Apple's runtime needs a different mechanism | [`../lev/architecture.md`](../lev/architecture.md) |
| What Lev is admitted and refused for | [`../lev/disposition.md`](../lev/disposition.md) |
| How any of these numbers were produced | [`../lev/measurements/`](../lev/measurements/) |
| Whether a Score's ordering means anything, door by door | [`2026-09-19-score-ordinality.md`](2026-09-19-score-ordinality.md) |
| What the one production caller's real workload does to all of this | [`2026-09-19-coder-turns.md`](2026-09-19-coder-turns.md) |
| How big a `coder` state may be, and what shrinking it cost | [`2026-09-20-state-budget.md`](2026-09-20-state-budget.md) |
| Whether the Lev adapters transfer beyond support items, including their refusals | [`2026-09-20-lev-domain-gap.md`](2026-09-20-lev-domain-gap.md) |
| What the newest production question costs, and what it buys | [`2026-09-19-program-selection.md`](2026-09-19-program-selection.md) |
| Whether a local door can answer that question instead of hosted Jev | [`2026-09-20-program-selection-local-doors.md`](2026-09-20-program-selection-local-doors.md) |
| Which production questions to stop asking, and why | [`2026-09-20-coder-question-baselines.md`](2026-09-20-coder-question-baselines.md) |
| Whether any rewording of `risk` beats the constant, and why none is in production | [`2026-09-20-risk-respecification.md`](2026-09-20-risk-respecification.md) |
| Whether a model should be able to say it does not know, and how that would be scored | [`abstention.md`](abstention.md) |
| How far the labels themselves can be trusted, and one door on items we did not write | [`2026-09-20-instrument-validity.md`](2026-09-20-instrument-validity.md) |

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

### The ceiling stands beside the score

An accuracy is bounded by how far two careful readers agree on the labels,
and that bound is measured, not assumed. Every record that publishes a
per-family accuracy on `support-v2` carries the family's agreement ceiling
on the same page, and a suite whose labels a second reader disputed lists
those items under a `disputes` field rather than correcting them. The
measurement, the disputed items, and one door scored on a suite nobody here
authored are in
[`2026-09-20-instrument-validity.md`](2026-09-20-instrument-validity.md).

| Family | Sample | Agreement | Wilson 95% | Cohen's kappa |
| --- | --- | --- | --- | --- |
| `routing` | 51 | 0.980 | 0.897 to 0.997 | 0.971 |
| `urgency` | 31 | 0.935 | 0.793 to 0.982 | 0.870 |
| `severity` | 18 | 1.000 | 0.824 to 1.000 | 1.000 |

The second reader was an automated session, not a person, so the table says
the labels are reproducible by a reader who did not write them and not that
they are correct. External suites carry their sources' published agreement
instead: BoolQ 0.90, MultiNLI 0.887.

## Measurements that cross the implementations

A run about one door stays with that door, as `lev/measurements/` does. A run
that compares them lives here, so it does not have to be written three times.

| Document | Holds |
| --- | --- |
| [`2026-09-19-score-ordinality.md`](2026-09-19-score-ordinality.md) | Whether a Score's `Σ i · p_i` is a position, on eight doors: monotonicity under an authored ramp, adjacent-versus-distant confusion, and bimodality. Jev and Lev hold; `kev-0.5b` does not. |
| [`2026-09-20-score-contract.md`](2026-09-20-score-contract.md) | What a Score answer means and what we score: `score` is the weighted position, every published Score number is argmax accuracy, and the tie convention — the last level listed — is stated and tested. |
| [`2026-09-19-frozen-embedding-baseline.md`](2026-09-19-frozen-embedding-baseline.md) | What frozen sentence embeddings plus logistic regression do against the doors we trained: it beats Lev and Kev on the one family it can serve, loses to hosted Jev, and refuses half the suite. |
| [`2026-09-20-frozen-embedding-door.md`](2026-09-20-frozen-embedding-door.md) | The same baseline served as a `POST /v1/systemone` door and scored by the unchanged Gym into the store: 0.95 on the 40 routing items, 38 typed refusals and zero harness losses, paired against every recorded door and read against the 0.056 floor. Clear of the small Kevs; not separable from Jev, `kev-8b`, or Lev. |
| [`2026-09-19-coder-turns.md`](2026-09-19-coder-turns.md) | `crates/coder`'s own question set, scored on real turns from recorded sessions rather than on authored support-desk items: what the workload looks like, what each question is worth against a constant, which door can afford ten-kilobyte states, and the four thresholds checked. |
| [`2026-09-20-state-budget.md`](2026-09-20-state-budget.md) | The state budget `classify::state_of` now holds to: what the 40 real turn states weigh and where the bytes are, the on-device door's refusal boundary read from retained rows, hosted Jev scored at eleven rungs of truncation with no measurable fall, and the caps derived from both. |
| [`2026-09-19-restatement-and-polarity.md`](2026-09-19-restatement-and-polarity.md) | Whether a door can repeat a fact its state asserts, on four doors over a factorial panel: hosted Jev answers all 128, `kev-4b` gives a proposition and its negation the same probability, and every local door passes delegation plans that collide. Also why the recorded call in `devin-fan-out-six` does not replay. |
| [`2026-09-20-instrument-validity.md`](2026-09-20-instrument-validity.md) | Whether the suite can be trusted before the doors are: a blind second reading of 100 `support-v2` items with kappa per family and every disagreement kept, and hosted Jev on `external-v1`, 200 BoolQ and MultiNLI items with their own crowd labels, at 0.90 and 0.81 against published ceilings of 0.90 and 0.887. |
| [`2026-09-20-coder-question-baselines.md`](2026-09-20-coder-question-baselines.md) | Each of the seven production questions against the constant that would replace it, on both open partitions of the real-turn suite: `shell_outcome` beats its constant by +0.227 twice, `action` loses to it on 32 turns, and the other five are retired. The result is `coder-turns-v2`, two questions with a suite and 76 rows, and a re-specified `risk` that waits for labels. |
| [`2026-09-20-risk-respecification.md`](2026-09-20-risk-respecification.md) | Three rewordings of the retired `risk` question and the v1 control, paired on the same 32 open items of `coder-turns-v1` under hosted Jev. The best candidate moves the door from 0.375 to 0.562 pooled and clears the floor against v1, and still sits 0.188 and 0.438 below the 0.875 constant on the two partitions. No `coder-turns-v3`; the reach rubric waits for labels. |
| [`2026-09-20-compiled-functions.md`](2026-09-20-compiled-functions.md) | What our 98 `routing` labels bought against an adapter compiled from the question text with zero labels: 0.90 against the LoRA's 0.92 on the 50 evaluation items, inside the 0.056 floor, with four inline examples adding nothing. The compiler wrote its own six labelled examples, so the guide's no-labels branch gains a leaf with three conditions rather than losing its root. |
| [`2026-09-19-program-selection.md`](2026-09-19-program-selection.md) | The question every turn now asks — which program, or none. Its baseline and headroom before its accuracy, its false positives and false negatives counted apart because they cost different amounts, and where the error mass sits. |
| [`2026-09-20-program-selection-local-doors.md`](2026-09-20-program-selection-local-doors.md) | The same 44 program-selection items asked of `kev-0.5b`, `kev-0.6b`, and `kev-4b` beside hosted Jev, in one store: accuracy, missed and spurious programs counted apart, and latency across eight blocks. `kev-4b` sits inside the accuracy floor but misses four of nine program requests; the small Kevs are below it; no local door suffices yet. |

## Other implementations

| Document | Holds |
| --- | --- |
| [`others/2026-09-19-laya.md`](others/2026-09-19-laya.md) | Laya, an open 421M decision model targeting this contract: what it is, why its benchmark table does not support what it is used for, and the two things worth taking from it. |

## Open questions

[`research/`](research/) holds leads that have not been settled — things worth
looking into, each with what would have to be true for it to matter.

[`abstention.md`](abstention.md) is a design with a recommendation and no
implementation: abstention as an additive `abstain` outcome on the answer,
emitted by the door from an input-conditioned signal and scored under a cost
matrix the workload supplies through the gate's budget. It names the
measurement that would settle it before anything is built (openagents#9383).
