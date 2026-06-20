# training.data_refinery_corpus.v1 — vertex-fleet worklog

Promise: `training.data_refinery_corpus.v1` (state: **planned** — unchanged by this work).

## 2026-06-20 update — eval-delta payment settlement computation

**Blocker advanced:** `blocker.product_promises.eval_delta_payment_missing`.

The payment policy (`apps/openagents.com/docs/2026-06-10-cs336-a4-data-refinery-payment-policy.md`,
"Eval-Delta Bonus Design") documented the bonus formula
`bonus_sats = round(clamp(delta, 0, delta_cap) * bonus_rate_sats_per_unit)`
and its anti-gaming boundaries as **prose only** — no code turned a measured
eval delta into a settlement decision. This change adds that deterministic,
fail-closed function:

- `apps/openagents.com/workers/api/src/cs336-a4-eval-delta-payment.ts`
- `apps/openagents.com/workers/api/src/cs336-a4-eval-delta-payment.test.ts` (10 tests)

`settleCs336A4EvalDeltaPayment` takes a real held-constant-trainer measurement
(filtered vs unfiltered baseline downstream eval score on the same source) plus
the producing stage's recompute-verified flag and optional operator funding
parameters, and returns either a `payable` settlement (with `settledBonusSats`)
or a `blocked` settlement carrying the documented reason + blocker refs. It
**fabricates nothing**: it never invents a delta, and the default path (no
funding parameters) returns `funding_parameters_unset`. Enforced boundaries:

- the producing stage must be `deterministic_recompute` verified;
- `delta > 0` is required (no penalty for neutral filtering, no bonus for
  regressions);
- the delta is clamped to `deltaCap` before pricing;
- funding parameters must be set and positive — unset until funding is approved.

No wallet/invoice/preimage material is accepted or emitted; the function
computes a public-safe sats amount and basis, not a payment instrument.

### What genuinely remains (blocker NOT cleared)

`eval_delta_payment_missing` stays listed: this is the settlement *computation*,
not a real payment. No fixed-trainer eval loop has produced a real eval-delta
measurement, no operator funding parameters are set, and this function is not
yet wired into A4 closeout, the `a4_eval_delta` leaderboard, or a settlement
receipt. The promise's green criterion — "at least one eval-delta payment
computed from a fixed reference model" backed by a Verified
`deterministic_recompute` shard — is unmet. `crawl_scale_corpus_missing` is
untouched.

## 2026-06-19 — corpus provenance receipt builder

**Blocker advanced:** `blocker.product_promises.corpus_provenance_receipts_missing`.

A deterministic, public-safe **corpus provenance receipt** builder for CS336 A4
refinery shards:

- `apps/openagents.com/workers/api/src/cs336-a4-provenance.ts`
- `apps/openagents.com/workers/api/src/cs336-a4-provenance.test.ts` (9 tests)

The deterministic refinery core already commits a SHA-256 digest per stage
output, but a shard that carries only a single `outputDigestRef` cannot prove
where its corpus came from or that the sequence of transforms applied to it is
internally consistent. The promise's green criterion requires "every shard
carrying source-provenance and transform digests"; this module produces exactly
that artifact:

- **Source provenance** — `sourceRef`, `snapshotRef`, `licenseRef`, and an
  `acquisitionMode` (`bounded_synthetic_corpus` today; `licensed_public_dataset`
  / `public_crawl_snapshot` reserved for forward-stability).
- **Chain-linked transform digests** — each stage's `inputDigestRef` must equal
  the prior stage's `outputDigestRef` (the first must equal the declared source
  input digest), the declared `finalOutputDigestRef` must equal the last stage's
  output, and every step's `recomputedDigestRef` must equal its committed
  `outputDigestRef` (deterministic-recompute is the verification class). All
  failures fail closed with `Cs336A4ProvenanceValidationError`.
- **Content-addressed `receiptRef`** — derived from a SHA-256 over a canonical
  receipt body, so the same provenance + chain always yield the same ref.
- **Public-safety guard** — `Cs336A4ProvenanceUnsafeMaterialError` rejects crawl
  payload, wallet, payment, and private-path material before it can be committed.

## What genuinely remains (blocker NOT cleared)

This is the receipt *format and integrity check*, not the receipts themselves.
`corpus_provenance_receipts_missing` stays listed because:

- No real refinery shard has been dispatched as paid work and closed out with a
  provenance receipt populated from actual source acquisition + recompute
  verification; this builder is not yet wired into the A4 closeout/admission path
  (`training-data-refinery.ts`) or the public projection.
- `crawl_scale_corpus_missing` and `eval_delta_payment_missing` are untouched and
  out of scope for this run.

No promise state was changed; no blocker was dropped from any tracking doc.

## 2026-06-20 live A4 admission/projection wiring

The provenance receipt is now part of the live A4 refinery evidence contract:

- `Cs336A4RefineryStageEvidence` requires `corpusProvenanceReceipt` for newly
  admitted shards.
- `admitCs336A4DataRefineryEvidence` rejects shards when the receipt final
  output digest does not match the shard `outputDigestRef`, when the transform
  chain is not linked, when a recomputed digest differs from the committed
  output, or when the receipt carries private/payment/raw-shard material.
- `publicDataRefineryProjection` exposes each shard's
  `corpusProvenanceReceiptRef`, `corpusProvenanceVerified`, and public-safe
  `corpusProvenanceReceipt` object.
- `GET /api/training/refinery/a4` now reports
  `corpusProvenanceReceiptStatus`, `corpusProvenanceReceiptRefs`, and
  `corpusProvenanceReceiptBlockerRefs` across the dashboard.

This wires the receipt shape into the admission/projection boundary. It still
does **not** clear `corpus_provenance_receipts_missing`: there is no live paid
refinery shard closeout whose deterministic-recompute verifier and provider
settlement produced one of these provenance receipts. `crawl_scale_corpus_missing`
and `eval_delta_payment_missing` are also unchanged.
