# marketplace.monetize_any_layer_with_referral.v1 — resale receipt seam

**Promise state:** `planned` (UNCHANGED — nothing here flips it).

## Blocker advanced

`blocker.product_promises.monetize_any_layer_resale_receipt_missing`

The monetize-any-layer scaffold already had two halves but no single
dereferenceable artifact tying a resale-with-referral event together:

- `marketplace-monetize-any-layer.ts` builds the per-layer **offer** + the pure
  accrual **plan**.
- `marketplace-monetize-any-layer-accrual.ts` feeds an authorized plan into the
  ONE cross-category referral ledger and returns a tagged accrual **result**.

Nothing projected those into ONE dereferenceable resale receipt, so the
"resale receipt missing" blocker had no artifact behind it.

## What was built

- `apps/openagents.com/workers/api/src/marketplace-monetize-any-layer-receipt.ts`
  — a PURE, INERT receipt builder. `buildMonetizeLayerResaleReceipt` reconciles a
  per-layer offer against the accrual result's plan (matching layer + sellerRef,
  reconciling qualifying sats = metered msat -> whole sats) and emits ONE
  `MonetizeLayerResaleReceipt`: a deterministic `receiptRef`, the offer, the
  metered spend event, the no-resale guard posture, and a typed
  `referralOutcome` (`disabled` / `unauthorized` / `no_attribution` /
  `self_attribution` / `zero_referrer_share` / `boundary_refused` /
  `invalid_input` / `recorded`). The `recorded` arm surfaces the ledger row's
  `payoutRef`, `qualifyingEventRef`, `state`, and accrued sats. A public-safe
  `monetizeLayerResaleReceiptProjection` drops amounts/keys.
- `apps/openagents.com/workers/api/src/marketplace-monetize-any-layer-receipt.test.ts`
  — 6 tests covering disabled / recorded / unauthorized outcomes, the
  offer-mismatch and empty-event-id guards, and the projection.

The receipt is honestly `settled: false`, `inert: true`, and carries the
blocker ref in `unclearedBlockerRefs` — a reconciled receipt SHAPE over an
inert accrual (eligibility at most) is NOT a settled resale receipt.

## What genuinely remains

- The blocker is **advanced, not cleared** — left in `blockerRefs`. A real
  green needs a settled revshare row whose receipt dereferences a settled
  payout, owner-signed per `proof.claim_upgrade_receipts.v1` and with demand
  provenance per `proof.demand_provenance.v1`.
- Sibling blockers `monetize_any_layer_access_product_unbuilt` and
  `monetize_any_layer_referral_accrual_unbuilt` are out of scope for this run.
- No wallet/settlement dispatch is wired; settlement stays on the
  readiness-gated, owner-armed rail.
