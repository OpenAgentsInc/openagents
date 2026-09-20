# Kev vs Jev, measured side by side

**Status:** measured on this machine, 2026-09-19, updated for the full
variant family. All four kev checkpoints run in the Rust port in
`crates/kev`, served by one `kev-serve --bundle-dir` process on CPU fp32
at `localhost:8009`; `jev-latest` is TypeSafe's hosted `jev-1.13.0`.
Identical request bodies were posted to `POST /v1/systemone` on both.
These are single-shot measurements, not a suite — treat the numbers as
illustration, not benchmark. The request texts are the same cases as the
original 0.5b run, reconstructed; small wording differences shift the
absolute numbers but not the shape of the result.

The Kev rows describe the historical adapters pinned by the fixture
manifests. They do not measure the replacement Qwen3 weights now on the
Hub. See the [2026-09-20 review](2026-09-20-upstream-review.md) and
[current upstream results](model-cards.md#current-upstream-checkpoints-reviewed-2026-09-20).

## Per-variant results

| Case | kev-0.5b | kev-0.6b | kev-4b | kev-8b | Jev |
| --- | --- | --- | --- | --- | --- |
| Support routing (choice) | returns 0.95 | returns 0.75 | returns 0.89 | returns 0.96 | returns 0.85 |
| Escalation (noul) | 0.75 | 0.40 | 0.54 | 0.91 | 0.72 |
| Frustration (score 0–2) | 0.99 | 1.24 | 1.24 | 1.74 | 1.05 |
| MNLI contradiction | contra 0.56 | contra 0.82 | contra 1.00 | contra 1.00 | contra 0.98 |
| Banking intent | card_arrival 1.00 | card_arrival 1.00 | card_arrival 1.00 | card_arrival 1.00 | card_arrival 1.00 |
| Mixed Yelp review (score 0–4) | 1.23 (1★ 0.51) | 2.26 (2★ 0.33) | 1.99 (2★ 0.58) | 1.66 (2★ 0.64) | 2.06 (2★ 0.88) |
| Rust `v[5]` panics? (noul) | 0.17 | 0.07 | 0.99 | 1.00 | 0.97 |
| Refund allowed at 25 days | 0.95 | 0.09 | 1.00 | 1.00 | 0.97 |
| Refund allowed at 31 days | 0.97 | 0.03 | 0.00 | 0.00 | 0.04 |

## What scale closed

**Policy-rule transfer — the sharpest gap, now closed at 4B.** The
25-day/31-day refund pair was kev-0.5b's worst failure (0.38 vs 0.51,
unordered near chance). kev-4b and kev-8b separate the pair cleanly at
1.00 vs 0.00, matching Jev's 0.97 vs 0.04. The held-out rule-application
screen that every upstream preview fails on suite metrics still shows up
correctly at this prompt level.

**Out-of-domain code.** `v[5]` on a 3-element vector: the small variants
guess (0.17, 0.07); kev-4b and kev-8b answer 0.99 and 1.00 against Jev's
0.97. Backbone knowledge, not mechanism, was the limit — and the Qwen3
bases carry it.

**Score ordinality — partially closed.** The mixed review should land
2–3 stars. kev-0.5b smears to 1★; the Qwen3 variants put mode at 2★ with
expected values 1.66–2.26 vs Jev's 2.06. kev-0.6b is numerically closest
but flattest; the family's mode agrees with Jev's, the confidence
ordering does not.

**In-distribution tasks hold everywhere.** Routing, intent, NLI: every
variant picks Jev's argmax. kev-8b is the sharpest (0.96–1.00 margins);
kev-0.6b is the softest but never wrong on argmax in this battery.

## What did not close

- **Calibration.** kev-8b overshoots on `frustration` (1.74 vs Jev's
  1.05 on a 0–2 legend) and `escalate` (0.91 vs 0.72). Upstream reports
  out-of-domain calibration metrics too, but a temperature fitted
  in-domain does not establish calibration for a new workload.
- **Mechanism probe weakness at 4b.** `state_in_state` isolation is
  0.63 for kev-4b (vs 0.997 at 0.5b, 0.85 at 8b). That is weaker use of
  evidence in the shared state, not evidence of sibling-question leakage.
  The sibling and absent conditions agree; the port reproduces the
  checkpoint's behavior.

## The remaining gap

The contract, mechanism, and now the full checkpoint family are ported
with per-variant conformance ≤ 4.1e-6 against the Python reference. What
separates kev from Jev is no longer mechanical and, at 4B+, no longer
primarily about these prompts either:

1. **Training breadth.** The 0.5B used six public datasets; the pinned
   Qwen3 adapters used broader suites. The historical 8B's upstream OOD
   development score was 0.774 against Jev's 0.857. The new 8B records
   0.796, which has not been measured on this page's requests.
2. **Calibration for transfer.** kev's temperature is fitted on its own
   dev distribution.
3. **The release screen.** The historical Qwen3 previews did not clear
   upstream's declared transfer screen. The new 4B clears the individual
   research checks, but not the policy-pair threshold across all recipe
   seeds. Neither result establishes suitability for a Coder workload.

For local routing and intent workloads, kev-4b on this machine agrees with
Jev's argmax on every in-distribution case tested, and kev-8b is the
sharpest. The ~0.8 s and ~1.5 s figures this page used to quote came from
these nine short requests. A suite item carries a full support message and a
full option set and costs more;
[`measurements/2026-09-19-variant-scores.md`](measurements/2026-09-19-variant-scores.md)
records what 157 of them cost, and also why that run cannot yet settle a
latency comparison between the variants.
