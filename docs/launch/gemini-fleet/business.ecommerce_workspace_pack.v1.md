# business.ecommerce_workspace_pack.v1 — launch note (gemini-fleet)

Promise state: **yellow** (unchanged by this work — no green flip).

## What this run built

A dereferenceable **claim-upgrade projection and gate endpoint** for the e-commerce first-paid delivery receipt. This completes the wiring of the e-commerce delivery receipt into the `proof.claim_upgrade_receipts.v1` contract.

- `apps/openagents.com/workers/api/src/ecommerce-campaign-receipt-routes.ts`
  - Updated the existing `/api/public/ecommerce-campaign/receipts` GET route to accept a `?view=paid-delivery-claims` query parameter.
  - This path queries the injected `EcommerceCampaignPaidDeliveryClaimStore` and returns the output of `projectEcommerceCampaignPaidDeliveryClaims`.
- `apps/openagents.com/workers/api/src/ecommerce-campaign-receipt-routes.test.ts`
  - Added a test verifying the `?view=paid-delivery-claims` path correctly returns the empty projection (since no receipts exist yet) with a `yellow` state.
- `apps/openagents.com/workers/api/src/index.ts`
  - Wired `emptyEcommerceCampaignPaidDeliveryClaimStore` into the dependencies of `makeEcommerceCampaignReceiptRoutes`.

## Which blocker this advances

`blocker.product_promises.ecommerce_pack_first_paid_delivery_receipt_missing`
— **partially.** This fully supplies step 2 ("Wiring the stored instance into a receipt-first claim upgrade per proof.claim_upgrade_receipts.v1") by adding the missing API route surface. The projection reports `totals.substantiatedCount = 0` correctly.

## What remains for green

1. Emitting and storing an instance of this receipt from a live seeded workspace.
2. Delivering a self-serve vertical pack to clear the self-serve blocker.
