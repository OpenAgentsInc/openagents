# Kev vs Jev, measured side by side

**Status:** measured on this machine, 2026-09-19. `kev-latest` is the
Rust port in `crates/kev` serving `kev-0.5b` on CPU at `localhost:8009`;
`jev-latest` is TypeSafe's hosted `jev-1.13.0`. Identical request bodies
were posted to `POST /v1/systemone` on both. These are single-shot
measurements, not a suite — treat the numbers as illustration, not
benchmark.

## Results

| Case | Question | Jev | Kev | Agree? |
| --- | --- | --- | --- | --- |
| Support ticket | department (choice) | returns 0.83 | returns 0.94 | yes |
| Support ticket | escalate (noul) | 0.76 | 0.72 | yes |
| Support ticket | frustration (score) | 1.01 (p(Frustrated) 0.99) | 0.92 (0.63) | yes, softer |
| MNLI premise | entailment (choice) | entailment 0.99 | entailment 0.95 | yes |
| Banking intent | intent (choice) | card_arrival 1.00 | card_arrival 1.00 | yes |
| Mixed Yelp review | stars (score) | 2.39 (3★ mode 0.57) | 1.15 (1★ mode 0.36) | **no** |
| Rust bounds check | panics (noul) | 0.97 | 0.09 | **no** |
| Policy, in-window refund | allowed (noul) | 0.97 | 0.38 | **no** |
| Policy, 31-day refund | allowed (noul) | 0.04 | 0.51 | **no** |

Latency was 145–380ms for kev on CPU and 480–620ms for Jev over the
network; not meaningful at this scale.

## Where they agree

On the training distribution — customer-support routing, NLI,
intent classification — the port tracks Jev's argmax every time, and its
distributions are slightly sharper (a 0.5B model trained on six datasets
sees fewer hedging cues). The wire contract is identical: the unmodified
`jev` client round-trips both.

## Where they diverge, and why

**Score on mixed-sentiment text.** Jev lands the mixed review at
3 stars (expected 2.39); kev spreads mass toward the low end (expected
1.15). Score was kev-0.5b's weakest primitive in its own eval (SST-5
0.533, Yelp 0.553 accuracy) — the ordinal structure is the hardest thing
for a small head to learn, and it shows exactly here.

**Out-of-domain content.** The Rust bounds-check question is code —
nothing in kev's six training sources resembles it, so 0.09 is the
model guessing, not a judgment. Jev's 0.97 reflects a much larger,
broader-trained model. This is the small-backbone limitation operating
as designed: kev's knowledge ceiling is its backbone.

**Policy-rule transfer.** The pair `allowed within 30 days` at 25 days
vs 31 days is the sharpest result in the table: Jev cleanly separates
(0.97 vs 0.04) while kev sits near chance (0.38 vs 0.51) and cannot even
order the pair correctly. This is precisely the held-out policy-rule
transfer that kev's own release screen tests — and the reason the 4B and
8B previews (0.759/0.774 OOD vs Jev's 0.857) still fail it. A 0.5B
trained on Banking77-style classification does not learn rule
application; it learns topic classification.

## What the gap is made of

The contract and the mechanism are fully ported; the delta is entirely
in the weights:

1. **Backbone scale.** 0.5B frozen Qwen2.5 vs whatever runs Jev.
   Transfer follows scale — the family's own dev curve runs
   0.598 → 0.759 → 0.774 → 0.857 across 0.6B → 4B → 8B → Jev.
2. **Training breadth.** Six public datasets (~13.5k questions) vs
   TypeSafe's corpus. Every miss above is outside the training
   distribution.
3. **Recipe.** kev-0.5b predates the ordinal and permutation-KL loss
   terms now in `kev/train.py`; its card documents both as unapplied.

The architecture — packed prefill, block-causal isolation, pointer
readout — is confirmed reproduced: conformance to the Python reference
is ≤ 4.1e-6 on every golden fixture, so what remains between kev and
Jev is model quality, not mechanics. Closing it is a training problem
(`kev-4b`/`kev-8b` scale, broader suites, the newer losses), not a port
problem.
