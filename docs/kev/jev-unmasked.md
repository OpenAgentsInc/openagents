# Jev's Architecture Unmasked, digested

**Status:** a reading of an external document — Archer Hume's
[Jev's Architecture Unmasked](https://archerhume.com/posts/jevs-architecture-unmasked)
(2026-09-17). Hume probed the hosted Jev (`jev-1.13.0`) with roughly 10,000
instrumented API calls to reconstruct its architecture. Kev implements the
reconstruction; this page separates what the probes measured from what
remains inference.

## The claim under test

An ordinary LLM asked for a confidence generates the string "90%" — the
probability of producing those tokens is not a 0.9 probability of being
right. TypeSafe's stated proposition for Jev is to keep a pretrained
transformer's knowledge but replace generated confidence claims with
probabilities read directly from internal representations, trained against
outcomes. Hume's question: what is the actual architecture, and which of
the popular guesses (a JSON classifier, a diffusion model, a wrapper on a
chat model) survive contact with the API?

## The seven components

### 1. Inference ends with a readout, not a decode loop

Published: TypeSafe says Jev "outputs all probabilities in parallel instead
of autoregressively generating by token."

Observed: the `output_tokens` field is accounting, not measurement. Its
count includes each question's identifier — text the docs state the model
never sees — and a 200-option answer costing 1,911 output tokens returned
as fast as a two-option answer. Latency grows with input length, not output
length. The tokenizer behind the count matches none of 192 public
tokenizers but matches Jev's own input counter. Whatever produces the
number runs after inference, on the serialized response.

### 2. Shared state, isolated questions

Observed: input token accounting is exactly additive — a common prefix plus
per-question suffixes. The decisive intervention is a planted secret: a
code declared inside a sibling question's text stays invisible to the
target question (`p ≈ 0.00`), while the same declaration moved into the
state reads `p ≈ 0.90–0.92`. Questions see the shared evidence and cannot
see each other.

Observed limits corroborate the packing: each branch (state plus one
question) caps near 32,768 tokens, and the whole request near 65,536 with
the state counted once — a single packed sequence of up to 2^16 where each
branch keeps a 2^15 window. Serving time stays nearly flat to ~100
questions per request, and question text costs roughly twice state text
token-for-token — consistent with computing the state once and batching
branch work. Hydragen and DeFT are the published prior art for
shared-prefix / tree-structured attention; Hume cites them as feasibility,
not as evidence of the implementation.

### 3. A causal backbone

Inferred, not measured: the probes cannot distinguish a causal decoder
from a bidirectional encoder. Hume argues causal anyway: Jev's knowledge
breadth (84.6% MMLU-Pro) implies frontier-scale pretraining, every model at
that scale is a causal decoder, and giving up causal structure forfeits
shared-prefix caching. The tokenizer fingerprint is a red herring for base
identification — closest public match is Qwen (348/415 probes), which rules
out an unchanged public tokenizer, not a public base.

### 4. Options interact before the choice

Observed: within one question, the alternatives are read together. Two
experiments:

- **Reference card.** An option's correctness depends on a reference card
  placed among the options. With the card last, Jev answers 16/16; first or
  middle, probability scatters around 0.5. Switching only the card's value
  flips which earlier option wins — proof the decision can use information
  placed after the candidates, and that earlier options' scores respond to
  later context.
- **IIA violation.** Appending an irrelevant option (`weather`) to a
  four-option question shifted the top-two log-odds from +0.38 to +0.11 —
  replicated across ten randomized blocks (mean −0.28, interval
  approximately −0.36 to −0.19). Fixed independent logits under a shared
  softmax cannot do that; a readout over the whole list can.

Two readout designs fit: a final-position head scoring numbered option
slots, or a pointer-style scorer comparing the decision representation
against each option's own hidden state. The 255-option API cap suits a
256-slot head but is enforced by validation, not the model; at 200 options
a copied answer scores 1.00 at every position without error spill, which
suits a pointer. Neither is decisive. Separately observed: fake options
injected through text never displace real ones — option boundaries are
marked in a way input text cannot forge. And order sensitivity is real:
reversing options moved one classification from 0.84–0.89 to 0.93–0.96.

### 5. Train the distribution; compute confidence in application code

Published: TypeSafe calls its post-training method Reinforcement Learning
for Calibrated Decisions (RLCD). The recipe is unpublished; the natural
objectives are proper scoring rules (log loss, Brier), where reporting the
true conditional distribution minimizes expected loss.

Observed: on 1,200 MMLU items, ten-bin ECE was 0.0313 with most predictions
near certainty. On freshly generated math, confidence tracked difficulty
(86.7% accurate / mean p 0.83 on multiplication; 32% / 0.30 on two-step
word problems). And the `confidence` field itself is arithmetic — the
official adapter computes `(p_max − 1/K) / (1 − 1/K)`, a measure of how far
the leading answer stands above uniform, not a second learned estimate.
Hume's caution carries through to kev's own docs: properness is an
incentive, not a deployment guarantee; calibration claims transfer only
within measured distributions.

### 6. Sparse capacity

Inferred: likely a sparse mixture-of-experts backbone. Jev processed ~30k
tokens in ~160 ms, which fits a MoE with ~10B active parameters better than
a dense 70B. Prefill-only serving removes MoE's usual costs (no decode loop
where bandwidth dominates, no long-lived KV cache competing with expert
weights). Hume flags this as the least certain component; nothing else in
the reconstruction depends on it — kev's dense Qwen bases prove the point.

### 7. Branches schedule as a batch, not a conversation

Observed: repeated identical requests differ slightly — including duplicate
questions inside one request — so API-level determinism should not be
assumed. That is consistent with numerical kernels, dynamic batching, or
routing, and does not imply sampling. The structural fact that matters:
there is no dependency chain between answers. If a workflow needs a later
question to consume an earlier answer, application code adds a second
request; the model does not do it for you.

## What would change the author's mind

Hume lists the reconstruction's commitments in order of confidence: direct
probability outputs (publicly described), question isolation and
option-order effects (observed), KV sharing and causal attention
(inferred), slot-vs-pointer readout (unresolved), sparse experts
(unverifiable from outside). The reference-card and fake-option results are
the settled ones; a 200-option middling-difficulty task could separate slot
head from pointer scorer; confirming MoE would take disclosure.

## Methods

17 September 2026, `jev-1.13.0`, one early-access account, one service
region. 1,029 instrumented probe records, 6,800 benchmark records, plus
follow-up studies (146 relational/option-interaction requests, 445
tokenizer fingerprints, latency sweeps). Latency comes from the
`x-envoy-upstream-service-time` header — upstream durations, not model
benchmarks. Probabilities return at two-decimal precision. A downloadable
evidence bundle records every observation; the essay links exact request
payloads per claim.

## Where kev lands relative to it

| Inferred Jev property | Kev's implementation |
| --- | --- |
| Direct readout, no decode | Pointer head + softmax, prefill-only — matches |
| Shared state, isolated branches | Block-causal mask; isolation measured (p 0.03 vs 0.99), packed = separate to 4e-6 — matches |
| Causal backbone | Qwen2.5/Qwen3 dense bases — matches the letter, not the scale |
| Listwise option readout | Pointer scorer over `</opt>` states, `<decide>` last — one of the two candidate designs |
| Proper-scoring-rule training | Cross-entropy (a proper rule); not RLCD — the recipe is unknown anyway |
| Sparse MoE capacity | Dense 0.5B–8B — deliberately not attempted |
| Batched branches | One packed sequence per request; no cross-request batching yet |
