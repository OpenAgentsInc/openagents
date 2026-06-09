# Generated Site Checkout Reconciliation Smoke

Date: 2026-06-07
Issue: #457 / `OPENAGENTS-SITES-MDK-LIVE-004`

## Summary

OpenAgents product surface now has deterministic generated-Site smoke coverage for the
checkout-to-reconciliation boundary.

The smoke uses the generated-Site fixture from #454:

- Site: `site_payment_smoke`
- Version: `version_site_payment_smoke_v1`
- Human checkout product: `human_brief_checkout`
- Checkout path: `/checkout/brief`
- Clean success path: `/checkout/complete`
- Clean status path:
  `/api/sites/site_payment_smoke/commerce/checkout-returns/{checkoutIntentRef}/status`

The modeled provider event source is MDK dashboard Standard Webhooks. The test
signs a deterministic `checkout.completed` event with the Standard Webhooks
header family, posts it through OpenAgents product surface's Site commerce webhook route, and then
replays the same event to prove duplicate handling.

## What It Proves

The smoke proves:

- the generated Site checkout starts in checkout-created, unpaid state;
- browser return/status reads do not create receipts or entitlements;
- the exact modeled source is dashboard Standard Webhooks, not daemon invoice
  HMAC or SDK node-control callbacks;
- the webhook route verifies the modeled event source before mutating state;
- a payment-received event updates the checkout intent to `payment_received`;
- the buyer payment ledger receives exactly one receipt, one entitlement, and
  one reconciliation event;
- replaying the same provider event reports `replayed` and does not duplicate
  durable receipt, entitlement, or reconciliation records;
- the clean success return projects an active entitlement after verified
  reconciliation;
- the public payment-proof route projects `verified_entitlement`; and
- public evidence omits raw invoices, payment hashes, preimages, MDK
  credentials, wallet material, customer private values, and webhook secret
  material.

Issue #557 records this as checkout evidence only through the generated Site
`paymentLaunchGate`. The smoke may expose a receipt bundle, but it must not
claim payout settlement unless settlement receipt refs are present.

## What It Does Not Prove

This is deterministic contract smoke evidence. It does not prove:

- a live MDK checkout was created;
- a live MDK dashboard callback reached production;
- a live invoice was minted;
- bitcoin moved;
- the agent-paid L402 proof route performed live proof verification;
- accepted work became payout eligible; or
- Pylon/Nexus/Treasury settlement occurred.

Live provider evidence still requires configuring the actual MDK-compatible
checkout route or sidecar, setting the correct webhook source and secret in
the Worker environment, running an operator-approved live or sandbox callback,
and recording only public-safe evidence refs.

## State Separation

The smoke preserves the required state separation:

| State            | Authority                                                                                                   |
| ---------------- | ----------------------------------------------------------------------------------------------------------- |
| Checkout created | A buyer has an issued checkout challenge and hosted checkout ref. This is not payment proof.                |
| Browser returned | The first-party return route can project status from server-side state. This is not payment proof.          |
| Payment verified | A signed, source-specific provider event matched a stored checkout and created receipt/entitlement records. |
| Payout eligible  | Not granted by this smoke. Accepted-work payout requires separate Nexus/Treasury gates and receipts.        |
| Payout settled   | Not granted without explicit settlement receipt refs.                                                       |

## Verification

Run:

```bash
bun run --cwd workers/api test -- src/site-commerce-routes.test.ts src/site-mdk-webhooks.test.ts
bun run --cwd workers/api test -- src/site-checkout-return.test.ts src/site-mdk-reconciliation.test.ts src/site-payment-proof.test.ts
bun run --cwd workers/api test -- src/redaction-regression.test.ts
```

The #457 route smoke is the
`runs generated Site checkout reconciliation through exact-source MDK webhook smoke`
case in `site-commerce-routes.test.ts`.

No live MDK account, funded wallet, live callback, deployed Worker secret, or
real bitcoin movement is required for this deterministic smoke.
