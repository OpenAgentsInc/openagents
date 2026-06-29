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

## Single-model retrieve dispatcher wiring (this run)

Blocker advanced: `public_paid_model_gateway_missing` — *partially advanced*
(left listed; see "What remains" below).

The `handleModelRetrieve` handler above was built and fully tested but
**unreachable in prod**: the exact-route registry is exact-match only, so
`GET /v1/models/{model}` fell through to a 404. This run wires it in, closing the
"remaining step" noted directly above:

- `inference/models-routes.ts`: `routeModelRetrieveRequest` — a prefix
  dispatcher mirroring `routeCloudCodingSessionRequest`. It matches only the
  path-param `/v1/models/{model}` (the LIST `/v1/models` stays an exact route),
  decodes the model id, falls through (`undefined`) for the bare list path /
  trailing-slash / nested paths (every served model id is a slash-free canonical
  slug), and otherwise hands off to `handleModelRetrieve`. The INERT gate +
  method check live in the handler, so a matching path with the gateway off
  still returns the typed inert 404 rather than a router fall-through.
- `worker-routes.ts`: new `routeModelRetrieveRequest` dependency + dispatch call
  in `makeWorkerRouteRequest`, placed after the exact-route pass so the LIST
  route keeps priority.
- `index.ts`: wires the dependency, gated by the SAME `INFERENCE_GATEWAY_ENABLED`
  flag; public + unauthenticated (published price/policy only).
- `models-routes.test.ts`: +8 dispatcher tests (16 total pass) covering match /
  no-match / decode / unknown-model / inert-off behaviour.
- `scripts/check-zero-debt-architecture.mjs`: Worker Response-return-surface
  budget ratcheted 94 → 95 with a justification comment, following the
  established per-surface ratchet pattern.

`GET /v1/models/{model}` is now a live (flag-gated) route: off-the-shelf OpenAI
clients can resolve a single model and a credits customer can read one model's
published price before funding a balance. This does NOT by itself let a customer
buy inference — the paid-credits funding loop is still secrets-gated — so the
blocker stays listed. No promise state changed; any future green flip remains
receipt-first and owner-signed.

## Pre-purchase cost estimator (this run)

Blocker advanced: `public_paid_model_gateway_missing` — *partially advanced*
(left listed; see "What remains" below).

The gateway now PUBLISHES per-1M-token prices, but a credits customer who wants
to "spend deliberately" still had to do the arithmetic by hand to answer the
question that actually gates a funding decision: "for THIS model and THIS many
tokens, on THIS funding rail, how many credits will it cost?" This run adds the
single pure answer to that question:

- `apps/openagents.com/workers/api/src/inference/cost-estimate.ts`
  (+ `cost-estimate.test.ts`): `estimateRequestCost` — a PURE pre-purchase
  estimator that reuses `priceRequest` (the EXACT pricing engine the metering
  hook charges against), fed with the customer's ESTIMATED token counts. It
  returns the customer-facing charge in USD, the legible credit unit, AND the
  integer msat the metering hook would actually decrement (`usdToMsatCeil`, the
  same ceiling the ledger uses), so an estimate cannot drift from the billed
  charge (a test asserts equality against `priceRequest`/`sellPricePerMtok`).
  It surfaces the exact Bitcoin-rail saving versus card (the on-brand pull onto
  Bitcoin), flags free-tier-eligible models while still quoting the PAID price
  (conservative planning for when the free pool is exhausted), flags unknown
  models, and clamps negative/fractional/NaN token inputs to non-negative ints.

RECEIPT-FIRST DISCIPLINE PRESERVED: the output carries `isEstimate: true`, moves
no money, writes no ledger row, and OMITS our cost basis / margin (`costUsd`) so
the estimate never leaks unit economics — only the customer-facing charge already
implied by the published catalog price. 11 tests pass.

No route is wired yet (kept to the smallest self-contained unit; the estimator is
the reusable core a future `POST /v1/quote`-style surface or price page reads).
This does NOT by itself let a customer buy inference — the paid-credits funding
loop is still secrets-gated — so the blocker stays listed. No promise state
changed; any future green flip remains receipt-first and owner-signed.

## `POST /v1/quote` route (this run)

Blocker advanced: `public_paid_model_gateway_missing` — *partially advanced*
(left listed; see "What remains" below).

The previous run built the pure pre-purchase estimator (`cost-estimate.ts`) but
left it un-wired: "No route is wired yet … the estimator is the reusable core a
future `POST /v1/quote`-style surface or price page reads." This run adds exactly
that surface — the thin, callable HTTP route over the existing estimator so a
credits customer can actually GET a quote before funding a balance:

- `apps/openagents.com/workers/api/src/inference/quote-routes.ts`
  (+ `quote-routes.test.ts`): `handleQuote` — `POST /v1/quote`. It validates the
  body (Effect Schema; `model` + `promptTokens` + `completionTokens` required,
  `fundingKind` constrained to `card | bitcoin` and defaulted to the conservative
  card rail, `cachedPromptTokens`/`batch` optional) and delegates to
  `estimateRequestCost`, returning the exact credit/USD/msat charge the metering
  hook WOULD settle, with `isEstimate: true`. 9 tests pass (inert 404, 405, 400
  invalid-json / invalid-request / out-of-range fundingKind, byte-for-byte parity
  with the pure estimator, card default, Bitcoin-saving surface, sloppy-token
  clamp).
- `index.ts`: registers `/v1/quote`, gated by the SAME `INFERENCE_GATEWAY_ENABLED`
  flag as `/v1/chat/completions` and `/v1/models`. Public + unauthenticated like
  `/v1/models`: it reads only published catalog prices (the estimator omits our
  cost basis / margin), moves no money, and writes no ledger row.

The route is a thin pass-through — no explicit `Response`-typed surface — so the
zero-debt Worker Response-return budget is UNCHANGED (96/96).

RECEIPT-FIRST DISCIPLINE PRESERVED: a quote is an estimate, never a receipt; the
real charge is still metered receipt-first from the provider's actual `usage`
object. This does NOT by itself let a customer buy inference — the paid-credits
funding loop is still secrets-gated — so the blocker stays listed. No promise
state changed; any future green flip remains receipt-first and owner-signed.

## Budget (affordability) quote — `/v1/quote` inverse mode (this run)

Blocker advanced: `public_paid_model_gateway_missing` — *partially advanced*
(left listed; see "What remains" below).

The gateway could answer the FORWARD funding question ("for THIS model and THIS
many tokens, how many credits will it cost?", `/v1/quote` token mode) but not the
INVERSE one that actually sizes a top-up: "if I fund N credits for THIS model on
THIS rail, how many requests of my typical shape can I run, and how many tokens is
that?" This run adds that:

- `apps/openagents.com/workers/api/src/inference/budget-estimate.ts`
  (+ `budget-estimate.test.ts`): `estimateBudgetCapacity` — a PURE affordability
  estimator. It prices ONE representative request through the SAME
  `estimateRequestCost` the forward quote uses (which reuses `priceRequest`, the
  exact engine the metering hook bills with), then floors the credit budget by
  that per-request cost. So an affordability estimate can never disagree with the
  per-request quote nor the eventual billed charge (a test asserts the embedded
  `perRequest` equals the forward estimate byte-for-byte). It reports affordable
  whole-request count, total tokens, spent/leftover credits (reconciling to the
  budget), the budget as spendable msat at the SAME `usdCentsToMsatFloor` rate the
  bridge grants with (never overstating balance), surfaces the Bitcoin-rail
  advantage (more requests per credit), flags the degenerate zero-cost shape as
  `affordableRequestsUnbounded`, and clamps negative/NaN budgets to zero. 11 tests.
- `quote-routes.ts` (+ `quote-routes.test.ts`): `POST /v1/quote` gains an
  ADDITIVE optional `budgetCredits` field. When present it returns the budget
  estimate (embedding the per-request `CostEstimate` under `perRequest`); when
  omitted it returns the per-request quote EXACTLY as before (backward-compatible —
  a test asserts the prior shape and that `affordableRequests` is absent). Thin
  pass-through, so the zero-debt Worker Response-return budget is UNCHANGED.
- `index.ts`: route comment updated to document the additive budget mode (same
  `INFERENCE_GATEWAY_ENABLED` gate, public + unauthenticated, public-safe).

RECEIPT-FIRST DISCIPLINE PRESERVED: a budget quote is an estimate
(`isEstimate: true`), never a receipt or a grant; it moves no money and writes no
ledger row. This does NOT by itself let a customer buy inference — the
paid-credits funding loop is still secrets-gated — so the blocker stays listed. No
promise state changed; any future green flip remains receipt-first and
owner-signed.

## Receipt resolver seam — assembler → resolvable surface (this run)

Blocker advanced: `inference_card_credit_inference_spend_receipt_missing` —
*partially advanced* (left listed; see "What remains" below).

`assembleCardCreditSpendReceipt` is a PURE linker that needs all three resolved
ledger legs handed to it; nothing turned a single Stripe checkout session id into
those legs by READING stored ledger state, so the assembler could not back a
resolvable surface. The "What remains" note below flagged exactly this
("Wiring the assembler into a resolvable `GET` endpoint …"). This run builds the
seam between them:

- `apps/openagents.com/workers/api/src/inference/card-credit-spend-receipt-resolver.ts`
  (+ `card-credit-spend-receipt-resolver.test.ts`):
  `resolveCardCreditSpendReceipt(sessionId, readers)` — reads the three legs via
  injected reader seams (`readPurchaseLeg` / `readGrantLeg` / `readSpendLeg`,
  which the Worker wires to the real D1 reads; tests inject fixtures), in chain
  order, short-circuiting on the first unsettled hop. It reports HONESTLY:
  `blank_session` (nothing to resolve), `pending` with the first `missing` hop
  (the EXPECTED state until the paid loop runs end to end — a card purchase
  exists before the bridge grant, which exists before any metered spend),
  `invalid` carrying the assembler's typed conservation/provenance failure
  (legs present but the chain lies), or `ok` with the assembled dereferenceable
  receipt. PURE apart from the injected readers (no D1, clock, network, or
  secrets); adds no ledger writes and moves no money. 8 tests pass (blank
  short-circuits all reads, full resolve, each `pending` hop, missing-grant never
  reads spend, `spend_exceeds_grant` → invalid, cross-session `context_ref` →
  `provenance_mismatch`).

This makes a `pending` vs `invalid` vs `ok` chain a typed, route-mappable
outcome, so a future flag-gated `GET` receipt route is a thin pass-through over
this resolver. It does NOT by itself produce a live receipt: the readers have no
real legs to read until the paid loop is collectable (still secrets-gated), so
the blocker stays listed. No promise state changed; any future green flip remains
receipt-first and owner-signed.

## What remains (both blockers, unchanged)

- `inference_card_credit_inference_spend_receipt_missing`: still no real
  card→credit purchase on prod (no Stripe secrets), so no dereferenceable
  end-to-end card→credit→inference-spend receipt exists yet. The assembler,
  provenance binding, and now the resolver seam are wired in source and waiting
  on a real upstream purchase; the remaining step is binding the resolver's three
  reader seams to live D1 reads behind a flag-gated, owner-authenticated `GET`
  receipt route (the resolver makes that route a thin pass-through).
- `public_paid_model_gateway_missing`: discovery (`/v1/models`,
  `/v1/models/{model}`) and now pre-purchase quoting (`/v1/quote`) are live
  (flag-gated), but a customer still cannot FUND a balance with a card or Bitcoin
  in prod, so the paid gateway loop is not closed.
