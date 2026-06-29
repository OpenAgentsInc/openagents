# Site Payment Product And Action Catalog

Issue #299 adds the versioned Site payment catalog contract for generated
Sites.

The implementation lives in `workers/api/src/site-payment-catalog.ts` with
tests in `workers/api/src/site-payment-catalog.test.ts`. The replay-safe D1
contract starts in `workers/api/migrations/0115_site_payment_catalog.sql`.

## What It Stores

The catalog records generated checkout products and paid actions outside the
generated Site source. Each item is linked to:

- Site and Site version refs;
- optional deployment, customer order, workroom, manifest, and source digest
  refs;
- product or paid-action IDs;
- price asset, denomination, and minor-unit amount;
- entitlement scope;
- settlement mode;
- checkout path;
- public projection state;
- sandbox state;
- agent-readable metadata refs.

Catalog records are not payment receipts and do not grant entitlements by
themselves. They are the stable source of product/action truth used by later
checkout intent, L402, reconciliation, and proof layers.

## Projection Boundary

The catalog supports public, customer, agent, and operator projections:

- public projections show active non-hidden items and omit metadata and
  operator refs;
- customer projections can show reviewable products/actions without operator
  refs;
- agent projections include agent-readable metadata refs when the item is
  marked agent-readable;
- operator projections include sanitized linkage refs for deployment, manifest,
  order, workroom, and source digest inspection.

All projections reject raw customer values, raw invoices, payment hashes,
preimages, wallet state, MDK credentials, provider grants, private runner
payloads, checkout query state, and secret-shaped refs.

## Payment Contract Integration

The catalog converts Site products and paid actions into
`OpenAgentsPaidEndpointProductRecord` values:

- checkout products bind as `site_checkout`;
- paid actions bind as `site_paid_action`;
- both use the `site_checkout` payment-policy surface until a separate
  surface is required;
- entitlements remain `resource` records scoped to the Site/version/item.

The module also defines `OpenAgentsSitePaymentCatalogHostedCheckoutPlan`, which
pairs a catalog record with an `OpenAgentsHostedMdkCheckoutRequest`. That keeps
hosted checkout intent work type-aligned without processing live payments in
this slice.

## Verification

- `bun run --cwd workers/api test -- src/site-payment-catalog.test.ts`
- `bun run --cwd workers/api test`
- `bun run --cwd workers/api typecheck`
- `bun run check:architecture`
