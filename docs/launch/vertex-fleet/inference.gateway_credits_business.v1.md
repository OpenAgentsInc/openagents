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

## Provenance binding follow-up (this run)

The assembler above could only link three legs a caller *asserted* belonged
together — nothing in stored state proved the `credit_to_msat` grant came from
the `card_to_credit` purchase. The bridge wrote every grant with a generic
`context_ref` of `inference:usd-credit:<userId>`, which names WHO was funded but
not WHICH Stripe checkout session funded them. This run makes a card-origin
grant **dereferenceable back to its purchase**:

- `apps/openagents.com/workers/api/src/inference/card-credit-provenance.ts`
  (+ `card-credit-provenance.test.ts`): the single source of truth for the
  card-origin grant `context_ref` format —
  `cardCreditGrantContextRef(sessionId)` →
  `inference:usd-credit:card:<sessionId>` and the inverse
  `parseCardCreditGrantContextRef`. Legacy/generic grants parse to `undefined`,
  so the change is purely additive and never misreads a non-card grant as
  card-funded.
- `usd-credit-bridge.ts`: `fundInferenceFromCredit` takes an OPTIONAL
  `sourceCheckoutSessionId`; when present the grant's `context_ref` is stamped
  with the card-origin format (else the legacy generic format is kept). Verified
  against real `node:sqlite` SQL that the stored `pay_ins.context_ref`
  round-trips back to the funding session.
- `card-credit-spend-receipt.ts`: the assembler now accepts the grant's stored
  `contextRef` and, when present, REQUIRES it to parse to the purchase's
  `sessionId` (new typed failure `provenance_mismatch`); on success the
  `credit_to_msat` chain step carries that `context_ref` as dereferenceable
  evidence of the hop-1→hop-2 binding. Omitting it preserves the prior
  caller-asserted behaviour.

This closes the *stored-linkage* gap so that once a real card purchase funds a
grant, the chain can be proven from the ledger rows themselves — it does not by
itself produce a live receipt (still secrets-gated, see below).

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
