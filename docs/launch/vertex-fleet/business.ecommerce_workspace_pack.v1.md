# business.ecommerce_workspace_pack.v1 — launch note

Promise state: **yellow** (unchanged by this work — no green flip).

## What this run built

A dereferenceable **first-paid delivery-receipt format + verifier** for the
e-commerce inventory-aware ad-campaign work item produced by the
`forge.template.ecommerce.inventory_campaign.v1` prefilled workspace.

- `apps/openagents.com/workers/api/src/ecommerce-campaign-delivery-receipt.ts`
  - `EcommerceCampaignDeliveryReceipt` schema: template ref, work-item ref,
    outcome kind, delivery stage, honesty evidence state, the five authority
    gates, outstanding blockers, the separated measurement contract (published
    artifact refs, spend cap, observed spend, stats window, attribution caveat,
    stockout follow-up), paid settlement, and a freshness timestamp.
  - `buildEcommerceCampaignDeliveryReceipt(...)`: deterministic, pure builder
    that enforces the hard authority invariant by construction — it **throws**
    if a receipt would carry published artifacts or observed spend while any
    authority gate is blocked, or if observed spend exceeds the accepted spend
    cap. `noAutoPublish` / `noAutoSpend` are always `true`.
  - `verifyEcommerceCampaignPaidDelivery(...)`: returns the reasons a receipt
    does NOT yet qualify as an evidenced first **paid** delivery (empty list ==
    qualifies). This is the gate a future green flip would consult.
- `apps/openagents.com/workers/api/src/ecommerce-campaign-delivery-receipt.test.ts`
  - 7 tests covering blocked drafts, reviewed-but-unpaid drafts, a fully gated +
    reviewed + paid delivery that verifies clean, and the rejection invariants
    (publish/spend while blocked, over-cap spend, missing payment ref).

## Which blocker this advances

`blocker.product_promises.ecommerce_pack_first_paid_delivery_receipt_missing`
— **partially.** This supplies the receipt schema and the honest verification
machinery a first paid e-commerce delivery requires, with the no-auto-publish /
no-auto-spend authority invariant baked in. It does **not** clear the blocker:
no real first paid e-commerce work-item delivery has occurred, so there is no
real receipt instance to dereference yet, and no receipt-first claim upgrade
per `proof.claim_upgrade_receipts.v1`. The blocker stays listed.

`blocker.product_promises.ecommerce_pack_self_serve_missing` — untouched
(out of scope for this run; the pack remains operator-assisted).

## What remains for green

1. Run a real first paid e-commerce work item through the seeded workspace and
   emit an instance of this receipt with `verifyEcommerceCampaignPaidDelivery`
   returning `[]`.
2. Persist/serve that receipt instance at a dereferenceable public route and
   wire it into a receipt-first claim upgrade per `proof.claim_upgrade_receipts.v1`.
3. Deliver a self-serve vertical pack to clear the self-serve blocker.
