# business.ecommerce_workspace_pack.v1 — launch note (gemini-fleet)

Promise state: **yellow** (unchanged by this work — no green flip).

## What this run built

A dereferenceable **claim-upgrade projection and gate** for the e-commerce first-paid delivery receipt. This wires the e-commerce delivery receipt into the `proof.claim_upgrade_receipts.v1` contract.

- `apps/openagents.com/workers/api/src/ecommerce-campaign-claim-upgrade.ts`
  - `EcommerceCampaignPaidDeliveryClaim` schema and gate structure: assesses whether a published e-commerce receipt substantiates a first-paid delivery under `business.ecommerce_workspace_pack.v1`.
  - `assessEcommerceCampaignPaidDeliveryClaim(...)`: evaluates the receipt against `verifyEcommerceCampaignPaidDelivery` and an `ownerSignOffRef`. It is PURE, never flipped a promise state, and returns `yellow`.
  - `projectEcommerceCampaignPaidDeliveryClaims(...)`: aggregates these claims.
- `apps/openagents.com/workers/api/src/ecommerce-campaign-claim-upgrade.test.ts`
  - 5 tests verifying that fully-gated receipts pass, missing sign-offs fail, unverified receipts fail, and projections correctly summarize counts without clearing blockers prematurely.

## Which blocker this advances

`blocker.product_promises.ecommerce_pack_first_paid_delivery_receipt_missing`
— **partially.** This supplies step 2 ("Wiring the stored instance into a receipt-first claim upgrade per proof.claim_upgrade_receipts.v1"). It does **not** clear the blocker because the system requires a real published instance and owner sign-off.

## What remains for green

1. Emitting and storing an instance of this receipt from a live seeded workspace.
2. Delivering a self-serve vertical pack to clear the self-serve blocker.
