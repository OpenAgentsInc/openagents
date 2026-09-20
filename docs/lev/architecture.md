# Lev architecture

**Status:** proposed. This page describes a design, not an implementation.
Where it states a fact about Apple's framework, that fact comes from code
this workspace has already built and run against the live runtime; see
[`apple-fm-surface.md`](apple-fm-surface.md) for the provenance. Where it
states a number about the model's behavior, it does not: those numbers are
what [`roadmap.md`](roadmap.md) step 1 exists to produce.

## What the framework gives you

Apple's `FoundationModels` framework exposes an on-device model through a
session object. The surface this repository needs is small:

| Capability | Shape | Matters to Lev because |
| --- | --- | --- |
| Availability | `SystemLanguageModel.default.availability`, with typed unavailable reasons | A Lev door has to refuse honestly on a machine where the model is off, ineligible, or still downloading. |
| Session | `LanguageModelSession(instructions:)`, `respond(to:)` | Instructions carry developer policy; the prompt carries the turn. The split is the injection boundary. |
| Guided generation | `@Generable` types, or a runtime `DynamicGenerationSchema` | Constrained decoding makes the answer's shape structural instead of prompted. This is the whole Choice primitive. |
| Generation options | temperature, `SamplingMode.greedy`, `SamplingMode.random` with a top-k or probability threshold and a **seed**, maximum response tokens | Seeds make a sampled estimator reproducible. Greedy makes a single-call argmax deterministic. |
| Use case | a session can declare a content-tagging use case rather than general generation | Classification is a declared use case, which is what every Lev question is. |
| Adapters | a `.fmadapter` package trained with Apple's adapter training toolkit, pinned to one base model signature | The only way to teach the model this task rather than prompt it. |
| Typed errors | context exceeded, guardrail violation, refusal, unsupported guide, decoding failure, adapter incompatible, and others | These map onto the door's refusal posture one for one. |

## What it withholds

The framework returns generated content. It does not return logits,
log-probabilities, per-token alternatives, or hidden states, and it runs no
code of ours inside the model.

That removes all three things kev's mechanism needs:

1. **Hidden states at chosen positions.** Kev reads the hidden state at
   each option's closing delimiter and at the decision token.
2. **A custom head.** Kev's pointer head is two small linear maps trained
   alongside a LoRA adapter. Apple's runtime executes the adapter; it will
   not execute a head.
3. **A distribution.** Kev's softmax over option scores *is* the answer.

So the kev port does not carry over. Packing, the block-causal mask, branch
position ids, and the pointer readout are all mechanisms for producing a
readout Lev cannot perform. What carries over is everything above the
mechanism: the request and answer shapes, the question-type mapping, the
mechanism probes reused as behavioral tests, the frozen-suite discipline,
and the rule that a probability is worth nothing until it is measured.

## Deriving a distribution

Three estimators are available. Lev uses all three, for different jobs.

### L1 — constrained argmax

One call, greedy sampling, a schema whose enum is exactly the admitted
option set. You get the choice and nothing else.

This is the cheapest honest answer and the only one that costs a single
forward pass. It cannot fill a `probabilities` map or a `confidence`, so a
Lev door serving L1 returns the typed answer with the distribution absent
and says why. The `jev` client already tolerates a missing `probabilities`
map on a Score; a Choice answer without a confidence is a contract gap that
[`roadmap.md`](roadmap.md) step 6 has to resolve explicitly rather than by
inventing a number.

### L2 — seeded sample ensemble

Run the same constrained question `N` times with random sampling at a fixed
temperature and `N` distinct seeds. The empirical frequency over the option
set is a distribution.

- It is reproducible, because the seeds are recorded.
- The seeds are a *block*, not a constant. Block `b` draws
  `b * N .. b * N + N`, so two blocks never share a seed, and block 0 draws
  the seeds every recorded number was produced with. The block exists
  because reproducibility has a cost: a seed reproduces its sample exactly,
  so rerunning the same seeds is not a second trial, and a comparison that
  reran them would be calling one measurement two pieces of evidence. A
  fresh trial is a block nobody has drawn yet. Eight blocks over one suite
  move accuracy by 2 points and the raw top share by 3, which is the floor
  under any comparison on this door:
  [`measurements/2026-09-19-seed-variance.md`](measurements/2026-09-19-seed-variance.md).
  The calibration metrics move differently and one of them moves a great
  deal — the confident-error count ran 6, 6, 10, 4 over four blocks of an
  unchanged base door, and the raw log loss with it, because most of a raw
  log loss here is the clamp on those items:
  [`measurements/2026-09-19-calibration-variance.md`](measurements/2026-09-19-calibration-variance.md).
- Its resolution is `1/N`. At `N = 16` the finest distinction the estimator
  can draw is 0.0625, which is coarser than the two decimal places the
  System One contract reports. Either the door reports at the estimator's
  resolution or it states the resolution alongside the answer. It does not
  round 0.0625 to 0.06 and let a caller read that as precision.
- It costs `N` times a single call, with no shared-prefix saving, because
  the framework exposes no cache Lev can reuse across samples.

L2 is the measurement instrument. It is how you see the model's decoding
spread at all, and it is the ground truth the cheaper estimator is fitted
against.

**L2 is not a calibrated predictive distribution, and this is the trap the
whole System One proposition warns about.** Sampling spread measures
decoding entropy. A small model can be wrong at every seed and report a
unanimous 1.00. Frequency over seeds says how consistently the model
answers, not how often it is right.

### L3 — band readout

Extend the schema with a second constrained field: an ordered enum of named
certainty bands, answered in the same call as the choice. Map bands to
numbers with a table fitted on held-out labelled outcomes.

One call, a per-item signal, and real numbers — but the numbers come from
our table, not from the model's claim about itself. That distinction is the
point. A model that emits the string "90%" has told you nothing; a model
that selects a band whose historical accuracy you have measured at 0.71 has
told you 0.71, and the 0.71 is yours.

L3 is the production path, because it costs one call. It is also the path a
trained adapter can improve, since band selection is exactly the behavior an
adapter learns from records labelled with outcomes.

### The design that follows

```text
question + state
      │
      ├─ L1 argmax ──────────────► choice, no distribution
      │
      ├─ L2 ensemble (N seeds) ──► raw frequency s
      │
      └─ L3 band (1 call) ───────► raw band s
                                      │
                                      ▼
                        calibration map g, fitted per
                        question family on labelled data
                                      │
                                      ▼
                    probabilities, confidence = (p_max − 1/K) / (1 − 1/K)
```

The estimator produces a raw signal. A fitted map turns the raw signal into
a reported probability. The confidence formula is the one TypeSafe's own
adapter uses and the one kev reproduces, computed on the calibrated
distribution rather than the raw one.

**No Lev number is a calibrated predictive probability, and the door says so
in every response.** The original rule here was stricter: omit the
distribution entirely until a map is fitted. The first measurement run
changed it, and the reasoning is worth keeping visible.

The run found no signal to fit. Sampling spread does not track difficulty —
top share sits between 0.81 and 1.00 on easy and hard items alike — and the
certainty band came back `likely` on every item including the wrong ones.
There is no dynamic range to map, so "wait for a fitted map" would have meant
"return nothing, indefinitely."

What the door does instead:

- It serves the L2 frequency in `probabilities`, and `confidence` computed
  from it, so an unmodified System One client round-trips.
- Every response carries `extensions.calibration`, and `GET /v1/models`
  repeats it: these are seeded-sampling frequencies measuring decoding
  consistency, not correctness. The behavior record shows the model holding a
  wrong answer at 0.81 as steadily as a right one.
- A caller that will not accept an uncalibrated number sends
  `extensions.require_calibration` and gets a typed `uncalibrated` refusal.

That posture is the same one kev's card and TypeSafe's own docs take — a
confidence is a statistic about a distribution's shape, not a verified
probability of being right. The difference is that kev's distribution comes
from a head trained against outcomes and Lev's comes from counting samples.
[`calibration.md`](calibration.md) defines what fitting would mean and what a
calibrated family would have to carry, and
[`measurements/2026-09-19-behavior.md`](measurements/2026-09-19-behavior.md)
records why there is nothing to fit yet.

## Question isolation without a mask

The System One contract says every question reads the same state and no
answer becomes context for another. Kev proves this with a block-causal
mask and measures it. Lev has no mask, so it gets isolation the only other
way: **one session per question, constructed fresh, never reused across
questions in a request.**

The consequence is the inverse of kev's economics. Kev computes the state
once and runs every question as a branch off it, which is why packing is
about twice as fast at 2.7 questions per request. Lev re-encodes the state
for every question. A ten-question request costs ten state encodings.

Two things follow:

- Lev's cost is linear in question count, so the System One design rule that
  says to ask every question that shares a state — including speculative
  ones — is much weaker advice here. Measure before you fan out.
- Kev's isolation probe transfers unchanged as an acceptance test. Plant a
  secret inside one question's text and ask a sibling question about it. The
  sibling must stay at chance. Move the secret into the state and the same
  question must find it. A Lev implementation that reuses one session across
  questions fails this, which is exactly what the test is for.

Whether the framework recomputes the shared prefix on every session, or
caches something Lev benefits from, is not observable from outside. It is
observable as latency, so it belongs in the step 1 measurement record rather
than in this paragraph as a guess.

## The wire contract

Lev serves the same `POST /v1/systemone` contract as Jev and kev, so the
`jev` client reaches it with a `base_url` change and no other edit. The
question types map onto guided generation:

| Question type | Schema | Answer derived from the distribution |
| --- | --- | --- |
| `noul` | a two-value enum, `no` and `yes`, with the criteria text as the value descriptions | `noul = p(yes)`, `selected` names the option the estimator's raw distribution picked |
| `choice` | one enum value per criteria key, described by `name` or `name: description` | `choice = argmax` of the **raw** estimator distribution, `probabilities` by option key, `confidence = (p_choice − 1/K) / (1 − 1/K)`, floored at zero |
| `score` | one enum value per ordered level description | `score = Σ k · p[k]`, `selected` names the level the estimator's raw distribution picked, `legend` maps indices to level text, `probabilities` by index |

That table is deliberately identical to kev's but for `selected`. Two
implementations of one contract should agree on what a question means even
when they disagree on how to answer it.

The "raw" qualifications — and `selected` on the other two rows — are the
one place the implementations can come apart, and only when a calibration
map is serving. A map rescales the probability of the answer the estimator
chose and never picks a different one, so a rescale that leaves a runner-up
holding the larger share changes the number beside the answer rather than
the answer — and then `choice` or `selected` is the only place the answer
survives the wire, because `noul` can sit below one half and a runner-up
level can lead `probabilities`. kev applies no map, so its argmax is always
the raw one and it writes no `selected`.
[`../gym/measurements/2026-09-19-calibration-and-the-argmax.md`](../gym/measurements/2026-09-19-calibration-and-the-argmax.md)
carries the contract and the enumeration behind it.

Question ids stay caller-side and never reach the model, the same as both
other implementations. Structured `state`, `instructions`, and criteria
values flatten into labelled text through one renderer that training and
serving share.

Lev adds one response extension, opt-in and additive so an unmodified client
still works: `extensions.estimator: true` asks the answer to carry which
estimator ran, `N` and the seeds for L2, the selected band for L3, the
calibration record the map came from, and the base model signature the
answer was produced against.

## Untrusted state, guardrails, and refusals

The state is data, never instruction. Apple's session splits developer
instructions from the turn prompt and treats instructions as the higher
authority, so **the question policy goes in the instructions and the state
goes in the prompt.** Putting caller-supplied state into the instructions
would elevate whatever an attacker wrote into it. That choice also rules out
priming one session with the state and reusing it, which is consistent with
the isolation rule above.

What Lev cannot do is fence the state. The usual defense — wrap untrusted
input in delimiters and tell the model to ignore instructions inside them —
is refused by Apple's guardrails. Measured on one benign item at fixed seeds:
`<state>` tags plus an ignore-instructions line refused on every draw, bare
`<state>` tags with no such line still refused, a triple-quoted block
refused, and a plain `STATE` label answered every time. The guardrail reads
fencing as adversarial framing regardless of content, so the instructions
boundary is the only marking Lev has. It is weaker than fencing, and this is
the runtime forcing a tradeoff rather than anyone choosing one. See
[`measurements/2026-09-19-comparison.md`](measurements/2026-09-19-comparison.md).

Constrained decoding covers the failure kev hardens against by hand. Option
text containing fake delimiters cannot add an option to a Lev question,
because the option set lives in the schema rather than in the rendered text.
Injection can still change *which* admitted option the model picks, which is
a judgment failure rather than a structural one, and it is what the adapter's
system policy and the calibration measurements have to account for.

Apple's guardrails are not ours and they fire on content we may consider
ordinary. A guardrail violation or an explicit refusal is a real answer about
the machine's state and the door passes it through as a typed refusal rather
than converting it into a neutral judgment. The practical limit this creates
is worth stating plainly: **a moderation, safety, or abuse workload is a poor
fit for Lev**, because the inputs most worth judging are the ones most likely
to trip the guardrail. Declaring the session's content-tagging use case is
the mitigation the framework offers, and how far it goes is a step 1
measurement.

The refusal set maps onto the framework's typed errors:

| Refusal | Raised when |
| --- | --- |
| `model_unavailable` | Apple Intelligence is off, the device is ineligible, or the model is still preparing |
| `uncalibrated` | no fitted calibration map covers this question family and the caller asked for probabilities |
| `too_many_options` | the option set exceeds the contract bound or what the schema admits |
| `branch_too_long` | the state and question exceed the runtime's context window |
| `guardrail` | Apple's guardrails blocked generation, or the model refused |
| `unsupported_guide` | the runtime rejected the generated schema |
| `adapter_incompatible` | the attached adapter's base signature does not match the running base |
| `invalid_request` | the request fails contract validation before any call |

## Adapters, and the thing that breaks them

An adapter is the only way to teach this model the task instead of
describing it. Apple's adapter training toolkit trains a LoRA adapter and
exports a `.fmadapter` package; `apple-fm-surface.md` records the package
format, the dataset shape, and the fact that `psionic` already implements a
Rust-native reader, writer, and validator for it.

The operational fact that matters more than any of that: **a `.fmadapter`
package is pinned to one base model signature, and the base ships with the
operating system.** An OS update that changes the base invalidates every
adapter trained against the old one. Nothing in this repository controls that
schedule. A Lev deployment therefore has to pin the signature, refuse on
mismatch rather than silently running an incompatible adapter, and treat
retraining as a recurring cost rather than a one-time one. That is the
opposite of kev, where the base is a file we hold.

Whether Lev ships an adapter at all in a first version, or stays on the base
model with prompted questions, is a decision in [`roadmap.md`](roadmap.md)
rather than an assumption here.

## What Lev will not do

- Return a probability the model computed. Every number Lev reports is
  produced by an estimator and a fitted map that this repository owns and
  can show you.
- Beat kev or Jev on judgment quality. It has a smaller model, no readout,
  and no control over training the base.
- Run anywhere but recent Apple Silicon with Apple Intelligence enabled.
- Carry a paid inference claim before somebody reads Apple's terms for
  serving the on-device model to third parties. That read is a decision in
  the roadmap, not a detail.
