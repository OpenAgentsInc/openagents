# Hosted Site Checkout Intent API

Issue #300 upgrades the first hosted Site checkout intent route from a
caller-supplied stub into a catalog-backed contract.

The implementation lives in `workers/api/src/site-commerce-routes.ts` with
coverage in `workers/api/src/site-commerce-routes.test.ts`.

## Endpoint

`POST /api/sites/{siteId}/commerce/checkout-intents`

Every write requires an `Idempotency-Key` header. Repeating the same request and
key returns deterministic safe refs.

The request identifies a generated Site payment catalog item:

- `siteVersionId`;
- `itemKind`, either `product` or `paid_action`;
- `productId` for products or `actionId` for paid actions;
- optional `catalogRef`;
- optional `expectedPrice` as a stale-price guard;
- `customerDataRefs`, which are public-safe requirement refs, not customer
  private values;
- clean success and cancel return paths.

The route validates active catalog membership, price match, settlement metadata,
sandbox state, required customer data refs, and clean checkout paths before it
creates a checkout intent projection.

## Hosted Boundary

This slice does not call live MDK credentials. It derives a
`BuyerPaymentChallengeRecord`, converts the Site catalog item to the paid
endpoint product catalog shape, builds an `OpenAgentsHostedMdkCheckoutRequest`,
and sends it to the fake hosted MDK client.

That keeps the route aligned with the future buyer-payment ledger and hosted
MDK client without granting entitlements, reconciling webhooks, settling
payouts, or exposing merchant credentials.

## Response Boundary

The public response includes:

- checkout intent ref;
- hosted checkout URL ref;
- redacted hosted checkout projection;
- redacted buyer payment challenge projection;
- deterministic idempotency marker;
- redaction claims.

It must not expose raw invoices, payment preimages, wallet state, MDK
credentials, provider grants, provider payout claims, raw customer private data,
or checkout query state.

## Verification

- `bun run --cwd workers/api test -- src/site-commerce-routes.test.ts`
- `bun run --cwd workers/api test`
- `bun run --cwd workers/api typecheck`
- `bun run check:architecture`
