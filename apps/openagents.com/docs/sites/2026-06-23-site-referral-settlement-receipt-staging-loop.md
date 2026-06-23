# Site referral settlement-receipt staging loop (DE-1 / #5524)

Promise: `sites.referral_bitcoin_stream.v1` (yellow).
EPIC: DE-1 `#5524` — Revenue Loop (Referral · Payments · Credits).
Registry pass: `2026-06-23.1`.

## What this advances

It clears the stale blocker
`blocker.product_promises.referral_settlement_receipts_missing` by proving the
**public settled referral-payout receipt surface is genuinely dereferenceable
end to end** — through a closed loop that runs the real dispatch path against a
real ledger and dereferences the produced receipt through the real public
receipt store, **without moving any money**.

It does **not** flip the promise green. The one remaining blocker,
`referral_first_real_payout_pending`, is owner-gated (a real settled Bitcoin
payout over the live hosted-MDK rail). That step is precisely flagged below.

## The gap that existed

The RL-1 rail (#5458) was already wired and money-safe, but its two halves were
only ever tested in isolation:

- `site-referral-payout-wire.test.ts` proved the dispatcher settles a row
  through a **mock** adapter.
- `public-site-referral-payout-receipt-routes.test.ts` proved the public route
  serves a settled receipt from a **mock** store.

Nothing connected the two. No test ran
**feed → dispatch → a real settled D1 row → and then dereferenced the receipt
the dispatch produced through the real public receipt store**. That missing
closed loop *was* the "settlement receipts missing" blocker: the settlement
receipt surface had never been proven dereferenceable against a real settled row
produced by the real dispatch path.

## What was built

### `site-referral-payout-staging-adapter.ts`

A staging/test-mode sibling of the production hosted-MDK referral payout adapter
(`site-referral-payout-adapter.ts`). It satisfies the **same**
`ReferralPayoutAdapter` contract the dispatcher
(`dispatchReferralPayoutSettlement`) invokes, so a staging-test settlement walks
the **same** idempotent, readiness-gated, asset-boundary-enforced
`approved → dispatched → settled` path and lands in the **same** ledger.

Money-safety, by construction:

- No wallet client, no destination resolver, no rail call. It cannot send
  Bitcoin even if reached on a production path.
- Gated behind an explicit `enabled` flag that **defaults OFF**. When disabled
  it **fails closed (throws)**, so the dispatcher records **no** settled state —
  the same fail-closed posture as the unconfigured production adapter.
- Produces a deterministic, public-safe
  `receipt.site_referral_payout.staging_test.{sha}` ref (derived from
  `payoutRef:amountSats`), so a retried dispatch maps to the same ref
  (idempotent), and the `staging_test` rail marker makes the public projection
  label it honestly — never as a real hosted-MDK Bitcoin payout.

### `site-referral-payout-receipt-loop.test.ts`

The closed-loop end-to-end proof (5 tests):

1. **feed → dispatch (staging adapter) → settled D1 row → public receipt
   dereferences.** A referred + paid Bitcoin event records an eligible row; the
   staging-test dispatch (readiness gate armed for staging) settles it; then
   `makeD1SiteReferralPayoutReceiptStore` resolves the produced receipt as a
   `staging_test` settlement with `amountSats`, `attributionLinked`,
   `qualifyingEventKind`, policy/caveat refs, and live-at-read staleness — and
   leaks no private payout material.
2. **Idempotent re-drive** settles at most once (exactly one settled row), and
   the same receipt still dereferences.
3. **Fail-safe:** a *disabled* staging adapter records no settled state, so
   nothing dereferences.
4. The disabled adapter throws the tagged fail-closed error.
5. The staging receipt ref is deterministic and public-safe.

## Dereferenceable receipt (the proof)

Running the loop produces a real settled ledger row whose evidence ref is a
`receipt.site_referral_payout.staging_test.*` ref, and the public receipt store
resolves it to:

```
{
  "amountSats": 125,
  "attributionLinked": true,
  "qualifyingEventKind": "forum_tip_paid",
  "receiptRef": "receipt.site_referral_payout.staging_test.<sha>",
  "resolution": { "settlementRail": "staging_test", "state": "settled", "status": "ok" },
  "schemaVersion": "openagents.site_referral_payout_receipt.v1",
  "evidenceRefs": ["receipt.site_referral_payout.staging_test.<sha>", ...],
  "staleness": { "composition": "live_at_read", "contractVersion": "projection_staleness.v1" }
}
```

In production this is served at
`GET /api/public/site-referral-payout-receipts/{receiptRef}` by the same store.

## The exact remaining owner step (to reach green)

The rail is fully built and proven. The **only** engineering-complete gate left
is `referral_first_real_payout_pending`, and turning it green is an owner
arm/flip, not engineering:

1. **Arm the live MDK payout mode** so
   `hostedMdkDirectPayoutDisabledGate()` → `livePayoutClaimAllowed: true`
   (i.e. `hostedProgrammaticPayoutsEnabled` + `hostedFundedKeyVerified`,
   per `mdk-payout-mode-gate.ts`, the #5512 boundary).
2. **Configure the production referral payout adapter** in `index.ts`
   (`referralPayoutSettlementAdapter`) with a funded hosted-MDK programmatic
   payout `client` and a real `resolveDestination` for the referrer's
   registered, reusable payout target (BOLT12 offer / LN address). Both are
   currently `null` (fail-closed).
3. **Drive one real referred + paid Bitcoin-revenue event** through the live
   path so the dispatch settles over the `hosted_mdk` rail (not `staging_test`),
   producing a real settled receipt at
   `GET /api/public/site-referral-payout-receipts/{receiptRef}`.
4. **Record the owner-signed green-flip transition receipt** per
   `proof.claim_upgrade_receipts.v1`.

Until then the promise stays yellow and no real Bitcoin moves.
