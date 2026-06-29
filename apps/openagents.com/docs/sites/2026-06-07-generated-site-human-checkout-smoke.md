# Generated Site Human Checkout Smoke

Date: 2026-06-07
Issue: #455 / `OPENAGENTS-SITES-MDK-LIVE-002`

## Summary

OpenAgents product surface now has route-level smoke coverage proving the generated-Site payment
fixture from #454 can initiate a human checkout through the real Site commerce
API boundary.

The smoke uses the deterministic generated-Site catalog from
`workers/api/src/generated-site-payment-smoke-fixture.ts` and calls:

1. `GET /api/sites/site_payment_smoke/commerce/discovery`
2. `POST /api/sites/site_payment_smoke/commerce/checkout-intents`
3. `GET /api/sites/site_payment_smoke/commerce/checkout-returns/{checkoutIntentRef}/status`

This proves the generated fixture can drive checkout discovery, checkout
intent creation, durable challenge/intent persistence, and clean return-status
projection without bypassing OpenAgents product surface's route layer.

## What It Proves

The smoke proves:

- the generated Site fixture catalog is accepted by the Site commerce API;
- the generated checkout helper body can create a checkout intent;
- the checkout intent store receives a checkout intent record;
- the buyer payment ledger receives an issued checkout challenge;
- the public checkout response is redacted;
- the clean status return does not require provider query state;
- the return status remains unpaid until verified reconciliation occurs.

## What It Does Not Prove

The smoke does not prove:

- a live MDK checkout was created;
- an invoice was minted;
- bitcoin moved;
- a provider webhook was verified;
- a receipt or entitlement was issued;
- accepted-work payout or settlement authority exists.

Those are covered by generated Site MDK live-smoke issues:

- #456 for the agent-paid L402 action path, now covered as a registered-agent
  contract smoke;
- #457 for checkout-return and provider reconciliation evidence, now covered
  by a deterministic dashboard Standard Webhooks smoke;
- #458 for public-safe runbook and evidence publishing.

## Verification

Run:

```bash
bun run --cwd workers/api test -- src/site-commerce-routes.test.ts src/site-checkout-return.test.ts src/site-mdk-reconciliation.test.ts
```

The #455 route smoke is the
`runs generated Site human checkout fixture through commerce APIs without payment verification`
case in `site-commerce-routes.test.ts`.

No live MDK account, funded wallet, or deployed Worker secret is required.
