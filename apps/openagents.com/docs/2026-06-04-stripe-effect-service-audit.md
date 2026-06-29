# Stripe Effect Service Audit

Date: 2026-06-04

Scope: future Stripe-backed credit purchase, customer management, balance
reconciliation, webhook fulfillment, and billing route work in
`openagents`, with focus on the Effect service/layer boundary that should
replace the current credit-card checkout placeholder.

This is a planning and code-quality audit only. It does not change production
contracts or invariants.

## References Consulted

- Workspace and repo guidance:
  - `/Users/christopherdavid/work/AGENTS.md`
  - `/Users/christopherdavid/work/INVARIANTS.md`
  - `openagents/AGENTS.md`
  - `openagents/INVARIANTS.md`
- Prior OpenAgents product surface Effect audits:
  - `docs/2026-06-04-effect-foldkit-codebase-audit.md`
  - `docs/2026-06-04-openagents-broader-effect-refactor-audit.md`
  - `docs/2026-06-04-openagents-zero-tech-debt-caller-inventory.md`
- Current OpenAgents product surface billing docs and code:
  - `docs/2026-06-03-autopilot-billing-credits.md`
  - `workers/api/src/billing.ts`
  - `workers/api/src/billing-routes.ts`
  - `workers/api/src/operator-billing-routes.ts`
  - `workers/api/migrations/0016_billing_credits.sql`
  - `workers/api/migrations/0018_billing_out_of_credits.sql`
- Local Effect guidance:
  - `effect-solutions show services-and-layers error-handling data-modeling config testing`
- Local Stripe SDK reference:
  - `../projects/repos/stripe-node`
  - `npm view stripe version peerDependencies dependencies --json`
- Stripe documentation checked on 2026-06-04:
  - `https://docs.stripe.com/api/checkout/sessions/create`
  - `https://docs.stripe.com/checkout/fulfillment`
  - `https://docs.stripe.com/api/customers/create`
  - `https://docs.stripe.com/billing/customer`
  - `https://docs.stripe.com/invoicing/customer/balance`
  - `https://docs.stripe.com/api/customer_balance_transactions`
  - `https://docs.stripe.com/api/idempotent_requests`
  - `https://docs.stripe.com/keys`
  - `https://docs.stripe.com/webhooks/signature`

## Executive Summary

OpenAgents product surface already has a first-party USD credit ledger in D1. That ledger should
remain the product authority for Autopilot credits. Stripe should enter the
system as an external payment and customer provider wrapped by typed Effect
services, not as a new source of truth spread through route handlers.

The Stripe implementation should be a narrow vertical slice:

1. Add a `StripeConfig` service for redacted API key, webhook secret, configured
   price/package IDs, success/cancel URLs, and the pinned API version.
2. Add a `StripeClient` service that lazily constructs `stripe-node` with the
   worker fetch HTTP client, `apiVersion: "2026-05-27.dahlia"`, bounded
   retries, and no module-load secret reads.
3. Add a `StripeCustomerService` that owns customer creation, mapping, metadata
   updates, and customer retrieval.
4. Add a `StripeCheckoutService` that creates credit Checkout Sessions and
   retrieves Sessions for fulfillment.
5. Add a `StripeWebhookService` that verifies raw webhook bodies and maps
   Stripe events into domain facts.
6. Add a `BillingCreditService` or equivalent Effect service that applies
   Stripe-confirmed credits to the existing D1 ledger with idempotency.
7. Keep route modules as HTTP mappers only. They should call Effect services
   and convert tagged domain errors to responses once.

The most important design rule is that a successful Stripe payment should
create one auditable positive `billing_ledger_entries` row with a stable
idempotency key such as `billing:stripe-checkout:<checkout_session_id>`. The UI
balance remains the derived D1 ledger balance. Stripe customer balance can be
used later for invoice credits, but it is not a replacement for OpenAgents product surface's
prepaid Autopilot credit ledger.

## Current State

OpenAgents product surface currently supports:

- D1-backed billing accounts and immutable ledger entries.
- Positive credits from launch grants, coupons, and operator adjustments.
- Negative debits from SHC container time and Codex token usage.
- A live billing summary API.
- A live coupon redemption API.
- A live admin manual-credit API.
- Out-of-credits suspension and email notification support.

OpenAgents product surface does not yet charge cards. `POST /api/billing/checkout` currently returns
a placeholder payload saying credit card checkout is not wired.

This current shape is healthy in one important way: product balance is already
ledger-derived, not stored as a mutable counter. Stripe should preserve that
property.

## Stripe Model Decisions

### Product Credits Stay In D1

Autopilot credits are a product ledger. The product needs to answer:

- How much credit can this user spend on Autopilot runs?
- Which payment, coupon, operator action, token event, or container interval
  produced each ledger row?
- Has a checkout session already been fulfilled?
- Can a webhook or return-page retry run twice without duplicating credit?

D1 is the right authority for those questions because the ledger already
contains product-specific sources, run IDs, team IDs, usage cursors, and
OpenAgents sync effects. Stripe should confirm money movement, then OpenAgents product surface
should append an OpenAgents product surface ledger row.

Stripe Customer Balance is a different product. Stripe documents customer
credit balances as adjustments applied to future invoices. A negative customer
balance transaction is a credit, and a positive transaction is a debit. That is
useful if OpenAgents product surface later invoices customers through Stripe Billing, but it should
not be shown as "available Autopilot balance" unless the product intentionally
switches to invoice-backed credits.

Recommended rule:

- For prepaid Autopilot credits purchased through Checkout, write OpenAgents product surface D1
  ledger entries.
- For future invoice/subscription credits, optionally mirror invoice credit in
  Stripe Customer Balance Transactions, but keep those rows clearly separated
  from prepaid usage credits.

### One Stripe Customer Mapping Per User And Currency

Stripe customers are effectively single-currency once invoicing or credit
balance is used. OpenAgents product surface's current ledger is USD-only, so create or reuse one
Stripe Customer per OpenAgents product surface user for `usd`.

Add a mapping table in a future migration, for example:

- `user_id`
- `currency`
- `stripe_customer_id`
- `livemode`
- `email_snapshot`
- `created_at`
- `updated_at`

The D1 mapping is the authority. Stripe metadata is a recovery aid, not the
primary lookup path. Customer metadata should include the OpenAgents product surface user ID, the
environment/mode, and a short product marker. Do not store raw session cookies,
provider tokens, or internal prompt/run payloads in Stripe metadata.

### Checkout Is The Payment Surface

Credit purchase should use Checkout Sessions in `payment` mode. Do not build a
custom card form first. Checkout gives the product a hosted payment page,
localized payment method handling, and a small server-side integration.

The session creation service should:

- Require an authenticated OpenAgents product surface user.
- Resolve the configured credit package to a Stripe Price ID or server-owned
  price data.
- Ensure a Stripe Customer exists for that user.
- Create a `payment` mode Checkout Session.
- Pass `customer`, `client_reference_id`, and narrow metadata containing the
  OpenAgents product surface user ID, package ID, credit amount, currency, and environment.
- Set a clean `cancel_url` to `/billing`.
- Set a non-product callback `success_url`, such as
  `/api/billing/stripe/checkout-return?session_id={CHECKOUT_SESSION_ID}`, that
  fulfills idempotently and redirects to clean `/billing`.
- Omit `payment_method_types`. Stripe's dynamic payment method configuration
  should be controlled from Stripe settings, not hard-coded in OpenAgents product surface.
- Pass an idempotency key for the Checkout Session creation POST.

This preserves OpenAgents product surface's clean public URL invariant. Product routes such as
`/billing` must not carry `checkout`, `session_id`, or payment result state.
The callback endpoint may consume the Stripe Session ID and then redirect to a
clean first-party URL.

### Webhooks Are The Fulfillment Authority

Stripe's Checkout docs are clear that redirect-based fulfillment is not
enough. Users may pay and never reach the return page. The webhook path must be
authoritative.

Add a webhook route such as:

```text
POST /api/billing/stripe/webhook
```

The route must:

- Read the raw request body exactly once.
- Read `Stripe-Signature`.
- Verify the event with `stripe.webhooks.constructEventAsync(...)` and the
  webhook signing secret.
- Use `Stripe.createSubtleCryptoProvider()` in the Worker runtime.
- Decode only event types the product owns.
- Acknowledge quickly and schedule longer work through the Worker background
  work service if needed.

Handle at least:

- `checkout.session.completed`
- `checkout.session.async_payment_succeeded`
- `checkout.session.async_payment_failed`

The return-page callback should call the same fulfillment service as the
webhook, but it must be safe to run after the webhook, before the webhook, or
concurrently with the webhook.

## Proposed Effect Service Topology

### Config

`StripeConfig` should be the only owner of Stripe secrets and package
configuration.

```ts
class StripeConfig extends Context.Service<
  StripeConfig,
  {
    readonly apiKey: Redacted.Redacted
    readonly webhookSigningSecret: Redacted.Redacted
    readonly apiVersion: '2026-05-27.dahlia'
    readonly successUrl: string
    readonly cancelUrl: string
    readonly packages: ReadonlyMap<BillingCreditPackageId, StripeCreditPackage>
  }
>()('@openagentsinc/StripeConfig') {}
```

Use a restricted API key (`rk_`) in production if Stripe permissions allow the
required customer, checkout, and webhook retrieval operations. If an initial
secret key is needed while defining the exact restricted-key policy, that
should be recorded as launch debt with a rotation date and a restricted-key
replacement task.

### Stripe Client

`StripeClient` should hide `stripe-node` construction and expose either the raw
client to leaf services or narrow helper methods. Do not instantiate Stripe at
module load, because build/test contexts may not have secrets.

```ts
class StripeClient extends Context.Service<
  StripeClient,
  {
    readonly client: Effect.Effect<Stripe>
  }
>()('@openagentsinc/StripeClient') {}
```

The live layer should:

- Pull `StripeConfig`.
- Extract the redacted API key only inside the layer.
- Construct `new Stripe(apiKey, { apiVersion, httpClient:
  Stripe.createFetchHttpClient(), maxNetworkRetries: 2, timeout: 20_000 })`.
- Attach request/response event listeners only if they log redacted structured
  metadata such as method, path, request ID, status, elapsed time, and
  idempotency key. Never log request bodies or API keys.

`stripe-node@22.2.0` currently has no runtime dependencies, supports Node 18+,
and exports worker/workerd builds plus `createFetchHttpClient`, so the package
fits a Cloudflare Worker boundary when imported through this service.

### Customer Service

`StripeCustomerService` should own all customer management. Route handlers and
billing services should not call `stripe.customers.*` directly.

Methods:

- `ensureCustomerForUser(user: BillingUser): Effect<StripeCustomerRef, StripeCustomerError>`
- `retrieveCustomer(ref: StripeCustomerRef): Effect<StripeCustomerSnapshot, StripeCustomerError>`
- `syncCustomerEmail(input): Effect<StripeCustomerSnapshot, StripeCustomerError>`

Responsibilities:

- Read/write the D1 `stripe_customers` mapping through an Effect repository.
- Create customers with email, display name, and safe metadata.
- Use an idempotency key such as `stripe:customer:<mode>:<userId>:usd`.
- Recover from stale mappings by retrieving the customer and mapping deleted or
  missing Stripe customers into tagged errors.
- Keep user-to-customer lookup separate from webhook fulfillment so webhooks
  can operate from Checkout Session metadata even when a local user session is
  absent.

Expected tagged errors:

- `StripeCustomerMappingNotFound`
- `StripeCustomerDeleted`
- `StripeCustomerCreateFailed`
- `StripeCustomerRetrieveFailed`
- `StripeCustomerMetadataInvalid`

### Checkout Service

`StripeCheckoutService` should own Checkout Session creation and retrieval.

Methods:

- `createCreditCheckout(input): Effect<CreditCheckoutSession, StripeCheckoutError>`
- `retrieveCheckoutSession(sessionId): Effect<CheckoutSessionSnapshot, StripeCheckoutError>`
- `fulfillCheckoutSession(sessionId, source): Effect<CreditFulfillmentResult, StripeCheckoutError | BillingCreditError>`

Responsibilities:

- Validate credit package IDs against `StripeConfig.packages`.
- Ensure the Stripe Customer exists.
- Create a `payment` mode Checkout Session with a stable idempotency key.
- Persist a local `stripe_checkout_sessions` row before or immediately after
  session creation so the product can inspect abandoned sessions and reconcile
  webhooks.
- Retrieve sessions with enough expansion to validate line items or use the
  line-items endpoint if package reconciliation requires it.
- Check `payment_status` before crediting.
- Validate `amount_total`, `currency`, metadata, package ID, livemode, and
  customer ID before appending OpenAgents product surface credit.

Expected tagged errors:

- `StripeCreditPackageNotFound`
- `StripeCheckoutCreateFailed`
- `StripeCheckoutRetrieveFailed`
- `StripeCheckoutUnpaid`
- `StripeCheckoutAmountMismatch`
- `StripeCheckoutCurrencyMismatch`
- `StripeCheckoutMetadataMismatch`
- `StripeCheckoutAlreadyFulfilled`

`StripeCheckoutAlreadyFulfilled` should usually map to success at the HTTP
boundary because fulfillment is intentionally idempotent.

### Webhook Service

`StripeWebhookService` should own signature verification and event decoding.

Methods:

- `constructEvent(rawBody, signature): Effect<Stripe.Event, StripeWebhookError>`
- `processEvent(event): Effect<StripeWebhookResult, StripeWebhookError | StripeCheckoutError | BillingCreditError>`

Responsibilities:

- Keep raw body handling out of generic JSON helpers.
- Store `stripe_event_id` with a unique constraint before side effects, or in
  the same transaction as fulfillment when possible.
- Treat duplicate event IDs as successful duplicate delivery.
- Map irrelevant event types to a typed ignored result.
- Never deserialize unverified webhook JSON.

Expected tagged errors:

- `StripeWebhookSignatureMissing`
- `StripeWebhookSignatureInvalid`
- `StripeWebhookUnhandledEvent`
- `StripeWebhookDuplicateEvent`
- `StripeWebhookPayloadInvalid`

### Billing Credit Service

The existing `workers/api/src/billing.ts` should be split into an Effect
service boundary instead of growing more Promise helpers.

Methods:

- `readSummary(userId): Effect<BillingSummary, BillingError>`
- `requireMinimumRunCredits(userId): Effect<BillingSummary, InsufficientCredits | BillingError>`
- `applyManualCredit(input): Effect<BillingSummary, BillingCreditError>`
- `applyCouponCredit(input): Effect<BillingCouponResult, BillingCreditError>`
- `applyStripeCheckoutCredit(input): Effect<BillingSummary, BillingCreditError>`
- `recordCodexUsageDebit(input): Effect<void, BillingDebitError>`
- `recordContainerUsageDebit(input): Effect<void, BillingDebitError>`

`applyStripeCheckoutCredit` should write one positive ledger row with:

- source: a new explicit value such as `stripe_checkout`
- amount: positive credit cents purchased
- unit: `credit_cents`
- quantity: purchased credit cents
- idempotency key: `billing:stripe-checkout:<checkout_session_id>`
- metadata: package ID, Stripe customer ID, Checkout Session ID, PaymentIntent
  ID if present, livemode, and Stripe event ID if the webhook supplied one

It should also reactivate suspended billing accounts after applying positive
credit, matching coupons and operator credits.

## Route Shape

Recommended future routes:

```text
POST /api/billing/checkout
POST /api/billing/stripe/webhook
GET  /api/billing/stripe/checkout-return
GET  /api/billing/summary
POST /api/billing/coupons/redeem
POST /api/omni/operator/billing/credits
```

`POST /api/billing/checkout` should stop returning placeholder state and
instead return:

```json
{
  "checkoutUrl": "https://checkout.stripe.com/...",
  "billing": { "...": "current summary before redirect" }
}
```

The browser command should navigate to `checkoutUrl`. It should not store
Stripe secrets, parse webhook events, or treat redirect return state as the
crediting authority.

The webhook route should not require a browser session. It authenticates
through Stripe signature verification.

The checkout-return route should accept only the Stripe Session ID, call the
idempotent fulfillment service, then redirect to `/billing` without result
query parameters.

## Error And Response Mapping

Service methods should not return `Response`. They should return domain values
or tagged errors. A billing route mapper should convert errors to HTTP status:

- Missing browser session: `401`.
- Unknown package or malformed request: `400`.
- Stripe unavailable or API failure: `502` or `503`, depending on retryability.
- Unpaid Checkout Session on return page: clean redirect to `/billing` plus a
  refreshed summary; for API calls, `409`.
- Duplicate webhook/event/session fulfillment: `200`.
- Signature failure: `400`.

Expected failures should be `Schema.TaggedErrorClass` values. Raw
`Stripe.errors.*` should be caught at the Stripe service boundary and converted
to serializable domain errors carrying safe fields:

- Stripe request ID
- Stripe error type/code when available
- HTTP status when available
- operation name
- retryable classification

Do not store raw Stripe error payloads in D1 or logs.

## Idempotency And Reconciliation

There are three idempotency layers to preserve:

1. Stripe API idempotency for outbound POST calls.
2. Stripe event ID idempotency for webhook delivery.
3. OpenAgents product surface ledger idempotency for product credits.

Recommended keys:

- Customer create: `stripe:customer:<mode>:<userId>:usd`
- Checkout Session create:
  `stripe:checkout:<mode>:<userId>:<packageId>:<requestId>`
- Ledger credit: `billing:stripe-checkout:<checkoutSessionId>`
- Webhook event: `stripe:event:<eventId>`

Checkout Session creation should use a request-scoped UUID or server-issued
checkout attempt ID. Do not key it only by `userId` and package, because the
same user may intentionally buy the same package twice.

Fulfillment should be safe under concurrent calls:

- Start with a transaction-like D1 batch or a unique insert.
- Attempt to insert the ledger row using the Checkout Session idempotency key.
- Treat an existing ledger row as fulfilled.
- Persist fulfillment status on a local Stripe checkout/session table.
- Re-read and return the billing summary.

Add an operator reconciliation job that can:

- List recent local unfulfilled Checkout Sessions.
- Retrieve their Stripe state.
- Fulfill paid sessions.
- Mark expired/unpaid sessions.

## Schema Boundaries

Do not pass raw Stripe SDK objects into browser DTOs or long-lived domain
state. Stripe SDK types are useful at the leaf boundary, but OpenAgents product surface should
project them into small Schema classes:

- `StripeCustomerRef`
- `StripeCheckoutSessionId`
- `StripeEventId`
- `StripePaymentIntentId`
- `CreditCheckoutSession`
- `CheckoutSessionSnapshot`
- `StripeWebhookEnvelope`
- `CreditFulfillmentResult`

Use brands for Stripe IDs:

- `cus_*`
- `cs_*`
- `evt_*`
- `pi_*`
- `price_*`

Decode inbound request bodies with Schema rather than ad hoc
`Record<string, unknown>` traversal:

- credit package checkout request
- webhook processing result stored in D1
- checkout return query/session ID
- operator reconciliation request

## Security And Secrets

Stripe security should follow the existing OpenAgents product surface config boundary pattern:

- Store API keys and webhook signing secrets only in Worker secrets.
- Prefer restricted API keys for production once the exact permissions are
  known.
- Keep webhook signing secrets separate from API keys.
- Never expose secret or restricted keys to `apps/web`.
- Never include raw keys in docs, logs, D1 records, source exports, issue
  comments, or commit messages.
- Use Stripe test mode and separate webhook secrets in local/staging.
- Rotate the key after any temporary broad secret-key launch period.

Stripe metadata is not a private datastore. Store only bounded identifiers and
package facts that are safe to inspect in the Stripe Dashboard.

## Testing Plan

Add tests before switching card checkout from placeholder to live:

- `StripeCustomerService` test layer creates and reuses one customer mapping.
- `StripeCheckoutService` creates a Checkout Session with the expected
  customer, package metadata, success/cancel URLs, and no
  `payment_method_types`.
- Webhook signature tests use `stripe.webhooks.generateTestHeaderString` or a
  fake `StripeWebhookService` test layer.
- Fulfillment tests prove duplicate webhook deliveries do not duplicate ledger
  entries.
- Return-page tests prove `/api/billing/stripe/checkout-return` redirects to
  clean `/billing`.
- Failure tests cover missing signature, invalid signature, unpaid session,
  package mismatch, amount mismatch, currency mismatch, and deleted customer.
- Architecture tests keep route modules from adding new
  `Effect.promise(() => dependencies.*)` adapters.

Use `@effect/vitest` for service tests and provide test layers per test so
state does not leak.

## Implementation Sequence

1. Add database migrations for Stripe customer mappings, checkout sessions,
   event receipts, and a new `stripe_checkout` billing ledger source.
2. Add Schema models and tagged error unions for Stripe IDs, packages,
   Checkout snapshots, webhook results, and credit fulfillment.
3. Add `StripeConfig` and `StripeClient` services.
4. Add `StripeCustomerService` with a fake test layer and live layer.
5. Add `StripeCheckoutService.createCreditCheckout` while keeping the existing
   placeholder route behind a temporary feature gate if needed.
6. Add `StripeWebhookService` and the webhook route.
7. Add `BillingCreditService.applyStripeCheckoutCredit` and wire webhook
   fulfillment.
8. Add the checkout-return route that calls the same fulfillment method and
   redirects cleanly.
9. Switch `POST /api/billing/checkout` from placeholder response to returning
   the Checkout URL.
10. Remove the `credit_card_placeholder` source after caller and data evidence
    shows no production dependency remains, or keep it only as historical
    ledger data with no new writes.
11. Update `docs/2026-06-03-autopilot-billing-credits.md` after the live Stripe
    path ships.
12. Tighten the architecture guardrail budgets if this migration deletes
    Promise route adapters or raw billing helpers.

## Non-Goals

- Do not implement subscriptions in this slice.
- Do not use Stripe Customer Balance as the visible Autopilot prepaid balance.
- Do not add a custom Payment Element/card form before Checkout.
- Do not add Connect platform flows unless OpenAgents product surface is explicitly charging on
  behalf of connected accounts.
- Do not route Stripe events through browser state.
- Do not store card or payment method details in OpenAgents product surface.

## Open Questions

- Should Stripe credit packages be managed as Dashboard Prices or server-owned
  `price_data`? Dashboard Prices are cleaner for operations, while `price_data`
  keeps the first integration smaller.
- Should credit purchases be user-owned only, or should team-owned billing be
  introduced before Stripe launch?
- What is the first production restricted-key permission set? The likely
  minimum is Customers read/write, Checkout Sessions create/read, webhook event
  verification support, and possibly PaymentIntents read for reconciliation.
- Should failed asynchronous payment methods create a user-visible billing
  event or only leave the checkout attempt as failed?
- Should fulfillment enqueue a sync notification immediately after crediting,
  or is the existing billing summary refetch enough for `/billing`?

## Acceptance Criteria For The Future Implementation

- No route module imports `stripe` directly.
- No browser code imports `stripe` or sees a Stripe secret.
- All outbound Stripe POST calls use idempotency keys.
- Webhook verification uses the raw body and rejects invalid signatures.
- A paid Checkout Session creates exactly one positive D1 ledger row.
- Duplicate webhook and return-page fulfillment attempts are harmless.
- `/billing` remains a clean product URL.
- Product balance remains derived from `billing_ledger_entries`.
- Stripe errors are tagged, serializable, redacted, and mapped at the route
  boundary.
- Tests cover happy path, duplicate fulfillment, signature failure, and
  mismatch failure cases before live card checkout is enabled.
