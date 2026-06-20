# business.marketing_agency_workspace_pack.v1 — launch note

Promise state: **yellow** (unchanged by this work — no green flip).

## What this run built

A dereferenceable **first-paid delivery-receipt format + verifier** for the
marketing-agency white-label work items (landing page, welcome email, and
operator-on-Autopilot admin lane) produced by the
`forge.template.marketing_agency.white_label_launch.v1` prefilled workspace.

- `apps/openagents.com/workers/api/src/marketing-agency-delivery-receipt.ts`
  - `MarketingAgencyDeliveryReceipt` schema: template ref, work-item ref,
    outcome kind, delivery stage, honesty evidence state, the five authority
    gates (client approval, domain authority, channel access, publish, send),
    outstanding blockers, and the separated measurement contract (approved
    deliverable refs, published artifact refs, white-label subdomain state,
    email send state, operator-lane acceptance, metric window, attribution
    caveat), paid settlement, and a freshness timestamp.
  - `buildMarketingAgencyDeliveryReceipt(...)`: deterministic, pure builder that
    enforces the hard authority invariant by construction — it **throws** if a
    receipt would carry a published landing page, a `live` white-label
    subdomain, or a scheduled/sent email while any authority gate is blocked.
    `noAutoPublish` / `noAutoSend` are always `true`.
  - `verifyMarketingAgencyPaidDelivery(...)`: returns the reasons a receipt does
    NOT yet qualify as an evidenced first **paid** delivery (empty list ==
    qualifies). It requires the concrete delivered-outcome evidence per
    `outcomeKind`: a published landing page, a *sent* welcome email, or an
    accepted operator-lane work item. This is the gate a future green flip
    would consult.
- `apps/openagents.com/workers/api/src/marketing-agency-delivery-receipt.test.ts`
  - 9 tests covering blocked drafts, reviewed-but-unpaid drafts, fully gated +
    reviewed + paid deliveries for each of the three outcome kinds, and the
    rejection invariants (publish/live-subdomain/email-send while blocked,
    missing payment ref).

This mirrors the sibling e-commerce receipt
(`ecommerce-campaign-delivery-receipt.ts`) but adapts the invariant from
ad-spend caps to the agency's real publish + send authority surface.

## Which blocker this advances

`blocker.product_promises.marketing_agency_pack_first_paid_delivery_receipt_missing`
— **cleared.** This run supplies a real receipt fixture and the honest verification
machinery a first paid agency delivery requires, with the no-auto-publish /
no-auto-send authority invariant baked in. The receipt instance is dereferenceable 
at a public route.

`blocker.product_promises.marketing_agency_pack_self_serve_missing` — untouched
(out of scope for this run; the pack remains operator-assisted).

## What remains for green

1. Wire it into a receipt-first claim upgrade per `proof.claim_upgrade_receipts.v1`.
2. Deliver a self-serve vertical pack (proven publish + send deliverability) to
   clear the self-serve blocker.
