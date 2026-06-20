# inference.gateway_credits_business.v1 — fleet build note

State: **red** (unchanged — no green flip; this note changes no promise state).

## Blocker advanced

`blocker.product_promises.inference_card_credit_inference_spend_receipt_missing`
— *partially advanced* (left listed; see "What remains").

## What was built

A pure, dereferenceable **card → credit → inference-spend chain receipt
assembler**:

- `apps/openagents.com/workers/api/src/inference/card-credit-spend-receipt.ts`
- `apps/openagents.com/workers/api/src/inference/card-credit-spend-receipt.test.ts`

The paid-credits loop already emits a receipt ref at **each** of its three hops,
but nothing linked them into one dereferenceable artifact. The three hops and
their existing refs:

1. **card → USD credit** (Stripe checkout fulfillment, `billing.ts`):
   USD ledger key `billing:stripe-checkout:<sessionId>`, evidence ref
   `evidence.stripe_checkout_paid.<sessionId>`.
2. **USD credit → msat** (the USD→msat bridge, `usd-credit-bridge.ts`):
   grant receipt ref `receipt.inference.usd_credit_grant.<grantRef>`.
3. **msat → inference** (the metering hook, `metering-hook.ts`):
   charge receipt ref `receipt.inference.charge.<requestId>`.

`assembleCardCreditSpendReceipt` is the single pure linker over those refs. It
mints the end-to-end ref `receipt.inference.card_credit_spend.<sessionId>`,
references the SAME refs the real ledger writes emit (no new ledger writes), and
enforces the conservation invariants that make the chain honest:

- granted USD never exceeds purchased USD,
- granted msat equals the shared `usdCentsToMsatFloor` conversion of granted
  cents (so the receipt cannot overstate spendable balance),
- metered spend never exceeds the credit this purchase funded.

It returns a typed failure (never throws) on any violation, so a caller can
refuse to publish a dishonest receipt. Pure: no D1, clock, network, or secrets;
no payment material in the output (refs, token counts, and owner-visible amounts
only). 9 tests pass.

## What remains (blocker NOT cleared)

The receipt FORMAT + linker now exist and are review-stable, but the blocker
stays listed because a *real dereferenceable receipt instance* still cannot be
produced in prod:

- prod Stripe secrets (`STRIPE_API_KEY`, `STRIPE_WEBHOOK_SIGNING_SECRET`,
  `STRIPE_CREDIT_PACKAGES_JSON`) are not set, so hop 1 cannot collect money;
- with no real card→credit purchase, the USD→msat bridge (hop 2) has nothing to
  bridge;
- therefore no funded metered inference request (hop 3) has settled against a
  card-origin grant.

Wiring the assembler into a resolvable `GET` endpoint and producing the first
real instance is the follow-up once the paid loop is collectable. No promise
state changed; any future green flip remains receipt-first and owner-signed.
