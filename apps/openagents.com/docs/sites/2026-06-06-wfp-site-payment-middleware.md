# WFP Site Payment Middleware

Issue #302 adds the Worker-compatible payment middleware contract for generated
Workers for Platforms Sites.

The implementation lives in `workers/api/src/site-payment-middleware.ts` with
tests in `workers/api/src/site-payment-middleware.test.ts`.

## Middleware Contract

Generated WFP Site routes can describe a protected paid action using:

- Site and Site version refs;
- catalog ref;
- action ID;
- method and local route path;
- entitlement scope;
- price;
- sandbox state;
- settlement mode;
- public-safe metadata refs.

The middleware evaluates that protected route against a Site payment catalog
record, a buyer-payment challenge, an optional entitlement, an optional hosted
checkout projection, and parsed payment headers.

## Decisions

The middleware returns one of four source-safe projections:

- `allow`: a matching active entitlement exists;
- `payment_required`: no entitlement or payment credential is present, so the
  route returns a 402-style projection with clean L402 headers;
- `entitlement_required`: a payment credential/proof is present, but OpenAgents product surface has
  not projected an active entitlement yet;
- `blocked`: the route metadata does not match the catalog or the route is
  unsafe.

Payment-required projections include a formatted `WWW-Authenticate: L402`
header value and a redacted L402 response contract. They do not expose raw
invoices, payment preimages, wallet state, MDK credentials, provider grants,
provider payout claims, customer private data, or checkout query state.

## Boundary

This slice does not upload WFP deployments, call live MDK credentials,
reconcile webhooks, grant final entitlements, or settle payouts. It establishes
the contract generated WFP Site code can call once entitlement and
reconciliation flows are wired.

## Verification

- `bun run --cwd workers/api test -- src/site-payment-middleware.test.ts`
- `bun run --cwd workers/api test`
- `bun run --cwd workers/api typecheck`
- `bun run check:architecture`
