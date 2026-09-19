# How to make Lev better

**Status:** strategy, written against measured numbers. Every claim about
current behavior links to a run in [`measurements/`](measurements/).
[`disposition.md`](disposition.md) says what Lev is good for today; this page
says what to do about what it is not.

## Where Lev actually loses

Scored on 52 authored items, evaluation split, one `crates/jev` client:

| Door | Accuracy | ECE | Brier | NLL | Latency |
| --- | --- | --- | --- | --- | --- |
| jev (hosted) | 0.96 | 0.099 | 0.026 | 0.117 | ~220 ms |
| kev-0.5b | 0.88 | 0.176 | 0.122 | 0.388 | ~180 ms |
| kev-4b | 0.77 | 0.188 | 0.149 | 0.464 | ~1 s |
| lev (N=8, 4 helpers) | 0.85 | 0.087 | 0.127 | 0.361 | ~1,600 ms |

Three gaps, and they have different causes, so they need different fixes:

1. **Sharpness.** Brier 0.127 against Jev's 0.026. Lev's numbers are vague.
   Its low ECE says the vagueness is honest, not that the numbers are good.
2. **Accuracy.** 0.85 against 0.96, on a task the base model was never
   trained for.
3. **Latency.** ~1.6 s against ~220 ms, because a distribution costs `N`
   forward passes rather than one.

A fourth is not visible in the table and matters more than it looks:
**order sensitivity**. Reversing an option list flips one greedy answer in
eight, against kev-0.5b's 7.4% over four permutations and hosted Jev's zero
observed flips.

## The shape of the problem

Every one of those gaps traces to the same root. Apple's runtime returns a
selected option and nothing else — no logits, no log-probabilities, no
hidden states. Jev and Kev both read a distribution the model computed. Lev
counts samples.

That has three consequences worth internalizing before choosing a lever:

- **Resolution is `1/N`.** An eight-sample estimate cannot express a
  difference finer than 0.125, which is coarser than the two decimals the
  contract reports. Sharpness is bounded by sample count, and sample count is
  bounded by latency.
- **Sampling entropy is not predictive uncertainty.** The behavior record
  shows the model holding a wrong answer at 0.81 as steadily as a right one.
  Drawing more samples measures the model's consistency more precisely; it
  does not make the model more right.
- **The only way to change what the distribution *means* is to change the
  model.** Which is the adapter.

So the levers split cleanly: engineering levers move latency and variance,
and exactly one lever — the adapter — moves accuracy and the meaning of the
numbers.

## Engineering levers

### 1. Concurrent sampling — built, and mostly spent

`bridge::Pool` runs `k` helper processes and `estimator::l2_pool` spreads an
ensemble's draws across them. Sound because sessions are independent by
construction, which is the same property that gives question isolation;
`tests/pool.rs` proves the same seeds give the same distribution rather than
assuming it.

| Helpers | Wall clock, 8 samples |
| --- | --- |
| 1 | 2,365 ms |
| 4 | 1,560 ms |

**1.5x, not 4x.** The helper processes are genuinely concurrent, so what
still serializes is inside Apple's runtime: the on-device model is one shared
resource and four callers queue for it. Concurrency is capped by the device.
Raising `k` past 4 is not worth measuring again without a reason.

*Return: spent. Cost: paid. Status: done.*

### 2. Serve L1 by default

Greedy decoding is deterministic — eight calls, one answer — and costs one
forward pass at ~300 ms. Most routing callers never read `probabilities`.

The contract makes this awkward rather than free: `jev::NoulAnswer` carries
only a probability and both other answer types require a `confidence`, so a
no-distribution estimator cannot fill the contract. The path is an explicit
opt-in — a request extension, or a distinct model id such as `lev-fast` —
that returns the typed choice with a degenerate distribution clearly marked,
rather than a silent default that hands a caller a 1.00 it will misread.

*Return: 5x on latency for the majority of calls. Cost: one contract decision
and a day. Status: the largest unstarted lever.*

### 3. Adaptive sample count

Nothing requires `N` to be fixed. Draw four; if they agree unanimously, stop;
if they split, draw four more. Most items in the suite are unanimous at four,
so the expected cost falls toward the floor while contested items keep their
resolution.

This is strictly better than a fixed `N` as long as the stopping rule is
recorded with the estimate, because the resolution then varies per answer and
the response has to say so.

*Return: most of lever 2's benefit without a contract change. Cost: small.
Status: unstarted, and probably the best return per hour on this page.*

### 4. Permutation averaging

Order sensitivity is a real error source: one flip in eight. Averaging an
estimate over `p` option orders removes most of it at `p` times the calls,
which composes with lever 1 and fights lever 3.

Kev gets order invariance structurally, from an `option_isolation` variant
that gives each option its own sub-branch with shared position ids. Lev has
no mask to exploit and has to pay in calls.

*Return: a measurable accuracy gain on multi-option questions. Cost: linear
in `p`. Status: unstarted; do it after 3, and measure whether the accuracy
gain survives the latency cost.*

### 5. Grow the suite until calibration is possible

The calibration machinery is built and correct, and it refuses every map:

| Family | Fitted on | Raw ECE | Mapped ECE | Verdict |
| --- | --- | --- | --- | --- |
| routing | 12 | 0.031 | 0.113 | refused; raw was already better |
| severity | 6 | 0.188 | 0.132 | refused; below the 8-item floor |
| urgency | 8 | 0.219 | 0.201 | refused; Brier got worse |

`routing` going from 0.031 to 0.113 is the whole argument for the gate:
fitting five bins on twelve items ruins a good signal. Hosted Jev's map
passes the same gate on the same suite (0.099 to 0.083), so the bar is real
and reachable.

A five-bin table wants roughly 30 items per bin to say anything, which is
150 to 200 items per family rather than 12. That is authoring work, not
research, and it is the only thing between the existing machinery and a
calibrated Lev.

*Return: moves Lev from "typed choice, approximate confidence" to "typed
choice, measured confidence", which unlocks every workload currently refused
because the probability gates an action. Cost: authoring 400+ labelled items.
Status: the highest-value non-model work.*

## The model lever: an adapter

Everything above makes Lev faster, steadier, or better understood. None of it
makes the model better at the judgment. Only an adapter does.

### Why it is the only route

Accuracy of 0.85 against 0.96 is not a prompting deficit. Neither is the
constant certainty band: every item in the behavior record came back
`likely`, including the wrong ones, so there is no band signal to calibrate
and no prompt that creates one. Both need weights trained against labelled
outcomes.

Apple's framework admits exactly one way to do that: a LoRA adapter trained
with Apple's adapter training toolkit and exported as a `.fmadapter`
package, attached to a session at run time.

### What the work is

The format and the data shape are already frozen in this workspace, from the
specs `psionic` wrote against toolkit v26.0.0:

- A `.fmadapter` package is a directory holding `metadata.json` and
  `adapter_weights.bin`, optionally a complete `draft.mil` plus
  `draft_weights.bin` pair. The weights file is a Core ML blob-storage
  container — 64-byte file header, 64-byte-aligned tensor records each
  beginning with the magic `0xdeadbeef`, fp16 little-endian payloads — so a
  package with correct values and the wrong container layout is rejected.
- Required metadata is `adapterIdentifier`, `baseModelSignature` (40
  lowercase hex), and `loraRank`.
- Training data is JSON Lines, one message array per line, roles `system`,
  `user`, and `assistant`, at most one system message and first if present,
  `response_format` admitted only on user messages and only for
  guided-generation records, assistant last.

That last line is what makes this tractable: **our suite is already in the
right shape.** Each item is a state, a question, and a labelled answer, and
the guided-generation `response_format` is exactly the constrained enum the
schema compiler already emits. The conversion is mechanical.

### What to train it to do

Two objectives, and the second is the one worth having:

1. **Pick the right option.** Ordinary supervised fine-tuning on
   `(state, question) -> correct option`. Moves accuracy.
2. **Report a truthful certainty band.** Train the band field against
   *outcomes* rather than against a label: an item the base model gets right
   is labelled `likely` or `almost certain`, one it gets wrong is labelled
   `unlikely` or `even odds`, with the band assigned from measured accuracy
   in that region rather than from an opinion. That is the closest thing to a
   proper scoring rule this architecture admits, and it is what turns L3 from
   a constant into a signal.

Objective 2 is the interesting one because it addresses the *meaning* of the
numbers, not just their accuracy. A model that reliably says "even odds" when
it is about to be wrong is worth more to a workflow than a model that is two
points more accurate.

### What stands in the way

- **The toolkit is an external dependency and is not on this machine.**
  It is distributed by Apple to developers, not from a package index. Nothing
  in the adapter lane starts until it is obtained.
- **Vendoring it into this repository is a licensing question**, not a
  convenience one. This repo is open source; Apple's toolkit is not
  redistributable on those terms. The correct structure is our training
  harness in-repo and the toolkit fetched by the operator into an ignored
  directory.
- **An adapter is pinned to one base model signature, and the base ships
  with the operating system.** An OS update that changes the base invalidates
  every adapter trained against the old one, along with every calibration map
  fitted with it. Nothing here controls that schedule. The adapter lane is a
  recurring cost, not a one-time one, and the door has to refuse on signature
  mismatch rather than silently serving an incompatible pairing.

### What success looks like

Not "it trained." The gate is the same one the calibration map has to pass:
beat the base model on items it was not trained on.

- Accuracy on the evaluation split, base against adapted, same suite, same
  estimator.
- Band signal: the certainty field must *vary*, and its bins must separate
  correct from incorrect at a rate better than chance.
- A calibration map fitted on the adapted model that passes
  `calibrate::admit` — which the base model's never has.
- Order sensitivity re-measured, since fine-tuning often reduces it.

## What not to do

- **Do not chase Jev's Brier.** Every Lev number is counted at resolution
  `1/N`. Matching 0.026 would take a sample count that destroys the latency
  that makes Lev worth having. Lev's position is a free, private,
  shape-guaranteed decision with an approximate confidence; Jev is what you
  call when the number has to be sharp.
- **Do not fence the state.** Apple's guardrails refuse delimiter-wrapped
  states regardless of content — `<state>` tags, bare tags, and triple quotes
  all refused a benign item that a plain label answered. The standard
  injection defense is unavailable, and re-introducing it costs availability
  for no security gain.
- **Do not add prompting layers.** The gap is in the weights. More
  instructions cost context and guardrail risk, and the behavior record shows
  the failures are not instruction-following failures.
- **Do not raise the helper pool past four** without new evidence. The device
  is the bottleneck.

## Sequence

1. Adaptive sample count (lever 3) — best return per hour, no contract
   change.
2. L1 as an explicit fast path (lever 2) — one contract decision.
3. Suite growth to 150–200 items per family (lever 5) — unlocks calibration.
4. Adapter, once the toolkit is obtained — the only accuracy lever, and the
   only route to a band signal.
5. Permutation averaging (lever 4) — last, and only if its accuracy gain
   survives its latency cost.

Levers 1 through 3 are ours. Lever 4 depends on Apple. That division is worth
remembering whenever the roadmap slips.
