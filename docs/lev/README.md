# Lev

For cross-project priorities and dependencies, see the [master roadmap](../roadmap.md).

**Status:** built and measured. `crates/lev` serves the System One contract
from Apple's on-device model, and the real `crates/jev` client reaches it
with a `base_url` change — the same contract `crates/kev` serves from open
weights and TypeSafe serves from its own.

The following is the historical 52-item support-suite comparison, with
evaluation subsets and door configurations described in the retained
[suite record](measurements/2026-09-19-suite-scores.md). It is not a current
all-workload ranking:

| Door | Accuracy | ECE | Brier | Latency |
| --- | --- | --- | --- | --- |
| jev (hosted) | 0.96 | 0.099 | 0.026 | ~220 ms |
| kev-0.5b | 0.88 | 0.176 | 0.122 | ~180 ms |
| kev-4b | 0.77 | 0.188 | 0.149 | ~1 s |
| lev (Apple, N=8) | 0.85 | **0.087** | 0.127 | ~1,600 ms |

Lev has the lowest calibration error and the second-worst Brier, which is
what honestly vague numbers look like. The Kev rows cover two of four
published model sizes on 26 evaluation items; on 157 items of
`support-v2-three-way`, `kev-8b` scores 0.879 with an ECE of 0.044 and Lev
0.783, so this table does not include the strongest Kev measured here. See
[`../kev/measurements/2026-09-19-variant-scores.md`](../kev/measurements/2026-09-19-variant-scores.md). [`disposition.md`](disposition.md) is
where to start: what Lev is admitted for, what it is refused for, and the
ranked list of what would improve it.

These Kev comparisons use the historical adapters pinned in this
repository. Upstream replaced the Qwen3 adapters under the same names;
the [2026-09-20 Kev review](../kev/2026-09-20-upstream-review.md) recommends
a new 4B evaluation. The update does not change Lev's measured admission
or establish a comparison with the new weights.

[`roadmap.md`](roadmap.md) holds the issue sequence and the state of each
step.

The name follows the pattern: Jev is the hosted model, Kev is the open
reconstruction, Lev is the Apple one.

## What it is

One document (the *state*) plus a map of typed questions goes in, one typed
answer per question comes out, and the code that asked owns the workflow.
That contract does not care what produced the answer. Lev produces it through Apple's `FoundationModels` framework when the helper
reports the model available. Eligibility, Apple Intelligence enablement, and
model readiness are checked at runtime; a running Apple Silicon Mac alone is
not sufficient.

Three properties make it worth building:

- **Apple supplies the base model.** This repository does not distribute the
  base weights. The helper reports device eligibility, enablement, and readiness;
  compatible OS assets and any selected adapter must be available. Inference
  still consumes device memory, compute, energy, and time.
- **The answer shape is guaranteed by the runtime, not by a prompt.**
  Apple's guided generation constrains decoding to a schema, so a Choice
  over an admitted option set cannot return an option that is not in the
  set. Caller text cannot add one either. Kev enforces its option set
  through sanitized delimiters and a pointer readout over the supplied
  options. Both constrain answer shape; neither establishes correctness.
- **Local inference has no per-token API charge.** The local helper keeps
  inference on the device. Hardware and operating costs remain, and latency
  still matters when a judgment precedes a metered agent turn. Do not report an
  unknown total cost as zero.

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
an estimator over observable behavior and, for an admitted family, a fitted
calibration map. The current door can return explicitly uncalibrated sampling
frequencies; callers that require a matching admitted map send
`extensions.require_calibration` and receive a refusal when one is unavailable.

Both Lev and local Kev can keep requests on the machine without an API
charge. Lev depends on Apple's model availability and admitted question
families; Kev requires downloaded weights and sufficient memory. Judgment
quality depends on the checkpoint: Lev and `kev-0.5b` are 0.070 apart on
our suite, which the suite cannot resolve, while `kev-8b` is 0.096 ahead
of Lev. Whether it
is good enough for any particular workflow is a measurement;
[`disposition.md`](disposition.md) holds the ones made so far.

## The three implementations side by side

| | Jev | Kev | Lev |
| --- | --- | --- | --- |
| Where it runs | TypeSafe's service | this machine, our code | this machine, Apple's runtime |
| Weights | closed, hosted | open adapter and head on an open base | closed, on-device, shipped by the OS |
| Readout | direct, trained against outcomes | pointer head, cross-entropy | none; estimated from behavior |
| Question isolation | packed branches, measured | block-causal mask, measured | one session per question, by construction |
| Inference charging | Provider policy | Local hardware; no inference API charge | Local hardware; no inference API charge |
| Probability source | the model | the model | Sampling frequencies; an admitted map where available |
| Shape guarantee | API validation | delimiter hardening, probed | constrained decoding, structural |
| Status here | `crates/jev`, shipped | `crates/kev`, serving four checkpoints | `crates/lev`, built and measured |

## Documents here

| Document | Holds |
| --- | --- |
| [`architecture.md`](architecture.md) | What Apple's framework gives and withholds, the three ways to derive a distribution from it, the design that follows, and the wire contract mapping. |
| [`apple-fm-surface.md`](apple-fm-surface.md) | What this workspace already established about Apple FM across `openagents` and `psionic` history: the bridge contract, the router precedent, the adapter package format, and the typed error surface. |
| [`calibration.md`](calibration.md) | The rule that no Lev probability gates an action before it is measured, the suites and gates that measure it, and the record a calibrated question family has to carry. |
| [`manifest.md`](manifest.md) | The document a release is: artifact digest, base signature, contract shapes, estimator, and the calibration records each admitted family rests on. The four serving checks read it. |
| [`revocation.md`](revocation.md) | The policy snapshot, the freshness window, and the guarantee that a door which never reaches the service again stops within it. The base-signature treadmill is what needs it. |
| [`mesh-plan.md`](mesh-plan.md) | How a Lev worker differs from a kev worker on the earn mesh: no artifact to verify, no packing win, and a verification floor that has to move from digests to behavior. |
| [`../../training/lev-adapter/README.md`](../../training/lev-adapter/README.md) | The adapter lane: the full training process, what to train, and what counts as success. |
| [`improvement-strategy.md`](improvement-strategy.md) | Where Lev loses, why each gap has a different cause, the engineering levers with measured returns, the adapter lane in full, and what not to bother trying. |
| [`disposition.md`](disposition.md) | Where Lev is admitted and refused, the four-way scores, and six ranked improvements with the first one built and measured. |
| [`roadmap.md`](roadmap.md) | The issue sequence, the decisions as made, and the state of each step. |
| [`measurements/`](measurements/) | The behavior record, the suite scores, and the comparison runs, with the exact commands. |

## Related

- [The Jev knowledge base](../decision-models/jev/knowledge-base.md) — the System One contract, the design rules,
  and the cookbook results. Lev speaks this contract.
- `docs/kev/architecture.md` — the mechanism Lev cannot use, described
  precisely enough to explain why.
- `docs/kev/model-cards.md` — the numbers Lev is measured against.
- `crates/jev` — the client that has to work against a Lev door with only a
  `base_url` change.
