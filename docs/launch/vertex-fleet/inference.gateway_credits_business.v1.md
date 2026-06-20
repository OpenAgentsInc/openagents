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

## Public paid-model gateway surface (this run)

Blocker advanced: `public_paid_model_gateway_missing` — *partially advanced*
(left listed; see "What remains" below).

The OpenAI-compatible gateway accepted requests (`POST /v1/chat/completions`)
but had **no public discovery + price surface**: nothing told a client which
models the gateway serves or what each PAID model costs. An OpenAI-compatible
gateway is expected to answer `GET /v1/models`, and a credits business needs a
published price per model before a customer can fund a balance and spend it
deliberately. This run adds that surface:

- `apps/openagents.com/workers/api/src/inference/model-catalog.ts`
  (+ `model-catalog.test.ts`): a PURE catalog builder over the existing
  `MODEL_PRICING_TABLE`. For every served model it publishes the supply lane,
  the per-1M-token sell price (USD + credits, in both input/cached/output
  dimensions), the published multiplier, the free-tier-eligibility flag (single
  source of truth reused from `FREE_ELIGIBLE_MODEL_CLASSES`), and the cost-basis
  provenance (`verified` for the real Fireworks rates vs `list_placeholder` for
  the Vertex Claude/Gemini billing TODO). Prices are derived from the SAME table
  the metering hook charges against, and a test asserts they equal
  `sellPricePerMtok`, so the **published price can never drift from the billed
  price**. `toOpenAiModelsResponse` projects this into the OpenAI `/v1/models`
  list shape (standard fields + `oa_*` extensions; clients ignore unknowns).
- `apps/openagents.com/workers/api/src/inference/models-routes.ts`
  (+ `models-routes.test.ts`): `handleModelsList`, a flag-gated GET handler that
  404s when the gateway is off (same inert posture as `/v1/chat/completions`),
  405s on non-GET, and otherwise serves the catalog. Public + unauthenticated
  (pre-purchase discovery, public-safe: published sell prices only — no prompts,
  credentials, or balances).
- `index.ts`: registers `GET /v1/models`, gated by the SAME
  `INFERENCE_GATEWAY_ENABLED` flag, alongside the existing chat-completions route.

This makes the paid model gateway publicly **discoverable and priced** — the
catalog every paid client and price page reads. It does NOT by itself let a
customer buy inference: the blocker stays listed because the paid-credits funding
loop above is still secrets-gated, so no published-priced model can yet be paid
for end to end. No promise state changed; any future green flip remains
receipt-first and owner-signed.

## OpenAI-compatible single-model retrieve (this run)

Blocker advanced: `public_paid_model_gateway_missing` — *partially advanced*
(left listed; see "What remains" below).

The gateway answered `GET /v1/models` (the list) but not the OpenAI standard
`GET /v1/models/{model}` (retrieve one). Off-the-shelf OpenAI clients call the
retrieve to verify a model exists and read its price before use, and a credits
customer wants to resolve a single model's published price before funding a
balance. This run adds that surface:

- `model-catalog.ts`: `findModelCatalogEntry(modelId, margin?)` (single source
  of model lookup; blank/unknown id → `undefined`) and `toOpenAiModelObject`
  (the single source of the OpenAI model-object shape, now reused by BOTH the
  list and retrieve projections so they can never disagree on a model's price or
  policy). A test asserts the retrieve entry equals the entry the list catalog
  publishes (no divergence) and that the single-model projection equals the
  per-model object the list emits.
- `models-routes.ts`: `handleModelRetrieve(request, modelId, deps)` — same inert
  posture as the list (404 when the gateway is off, 405 on non-GET), public +
  unauthenticated (published price/policy only), and an unknown/blank model id
  returns OpenAI's standard `model_not_found` error shape (`error.code`,
  `error.type: invalid_request_error`, `error.param: model`) so clients surface
  it correctly. Prices are the SAME pricing-table-derived sell rate the metering
  hook charges, so the resolved price cannot drift from the billed price.

Dispatch wiring is intentionally NOT added yet, mirroring the established repo
pattern for `GET /v1/fine_tuning/jobs/:jobId` and `GET /v1/sandboxes/:id`
(`handleFineTuningJobGet` / `handleSandboxGet`): those `/:id` lifecycle handlers
were landed built + fully tested ahead of their dispatcher path-param wiring.
The handler + lookup + projection + tests are review-stable here; the remaining
step is to register a `/v1/models/{model}` path-param route in the worker
dispatcher (the exact-route registry is exact-match only). This does NOT by
itself let a customer buy inference, so the blocker stays listed.
