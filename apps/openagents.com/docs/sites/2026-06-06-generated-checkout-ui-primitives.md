# Generated Checkout UI Primitives

Issue #301 adds the generated-source contract for Site checkout UI primitives.

The implementation lives in
`workers/api/src/site-checkout-ui-primitives.ts` with tests in
`workers/api/src/site-checkout-ui-primitives.test.ts`.

## Primitive Contract

Generated static and Workers for Platforms Sites can now describe source-safe
checkout UI primitives for:

- checkout buttons;
- checkout forms;
- product cards;
- paid action prompts;
- tip affordances;
- deposit affordances;
- subscription affordances;
- success states;
- cancel states;
- entitlement states.

The contract is declarative. It references catalog refs, product/action IDs,
checkout paths, display refs, price, entitlement scope, sandbox state, the
OpenAgents product surface-hosted checkout intent endpoint, and public-safe customer data
requirement refs.

It does not store MDK credentials, raw invoices, payment preimages, wallet
state, webhook secrets, provider grants, provider payout claims, raw customer
private data, or checkout result query state.

## Catalog Derivation

`siteCheckoutUiPrimitivesFromCatalog` derives common primitives from the
versioned Site payment catalog:

- each product gets a product card, checkout button, and checkout form;
- each paid action gets a paid action prompt and checkout button;
- each contract gets success, cancel, and entitlement state primitives.

The generated primitives point at
`/api/sites/{siteId}/commerce/checkout-intents` and clean local success/cancel
paths. Agents receive metadata refs only in agent/operator projections.

## Guardrails

The decoder rejects:

- success/cancel URLs with query strings or fragments;
- checkout result state such as `checkout_id`;
- raw invoices or payment-secret-shaped values;
- customer private values such as emails;
- provider grants or wallet material;
- non-local checkout/action paths.

This keeps generated Site source deployable without smuggling hosted payment
authority into the Site bundle.

## Verification

- `bun run --cwd workers/api test -- src/site-checkout-ui-primitives.test.ts`
- `bun run --cwd workers/api test`
- `bun run --cwd workers/api typecheck`
- `bun run check:architecture`
