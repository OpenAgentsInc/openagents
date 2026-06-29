# Paid Endpoint Product Catalog

Issue: #290 / OPENAGENTS-H-002

Date: 2026-06-06

## Purpose

Recoverable economic limits need stable product references before OpenAgents can
issue credits, hosted checkout intents, Lightning/MDK invoices, or L402
challenges. Generated Sites and agents should not embed ad hoc prices or route
logic. They should point at a catalog product that describes what is being paid
for, what entitlement it grants, and what spend-cap hints apply.

The implementation in `workers/api/src/paid-endpoint-product-catalog.ts` defines
that catalog contract without creating invoices, debiting credits, or settling
provider payouts.

## Product Record

Each product record includes:

- `productId`: stable lowercase catalog ID.
- `surface`: one of the shared payment-policy surfaces such as `agent_api`,
  `forum_paid_action`, `runner`, or `site_checkout`.
- `binding`: the endpoint or action the product unlocks.
- `price`: asset, denomination, and positive integer amount in minor units.
- `entitlement`: duration, quota, resource, or duration-plus-quota scope refs.
- `spendCapHintRefs`: public-safe hints agents can compare against local spend
  caps before paying.
- `publicAgentDocRefs`: public-safe documentation refs for agent clients.
- `status`: `draft`, `active`, `paused`, or `retired`.
- `projectionPolicy`: whether the product is visible publicly, to agents, to
  customers, or only to operators.
- `internalEconomicsRefs`, `providerBindingRefs`, and `operatorNoteRefs` for
  operator-only reasoning.

The catalog currently supports products for:

- agent API endpoints;
- Forum paid actions;
- Site checkout and Site paid actions;
- runner recovery products.

## Validation Rules

The decoder rejects:

- unstable product IDs, resource refs, action refs, and entitlement refs;
- duplicate product IDs;
- non-positive or non-integer prices;
- price-denomination mismatches;
- endpoint bindings without a method and path;
- action bindings without an action ref;
- paths with query strings or fragments;
- secret-shaped keys or values, raw invoices, raw payment material, wallet
  material, preimages, MDK credentials, provider grants/tokens, customer emails,
  raw prompts, raw runner logs, source archives, and other private-material
  shapes already rejected by the runner gateway private-material guard.

The bitcoin price path uses `bitcoin_millisatoshi` as a denomination label
where precision matters, while public docs should generally say bitcoin unless
the exact unit is relevant.

## Projection Boundary

`projectOpenAgentsPaidEndpointCatalog` and
`projectOpenAgentsPaidEndpointProduct` produce public, agent, customer, or
operator projections.

Public, agent, and customer projections:

- exclude `operator_only` products;
- remove `internalEconomicsRefs`;
- remove `providerBindingRefs`;
- remove `operatorNoteRefs`;
- keep only safe product IDs, route/action bindings, prices, entitlement refs,
  spend-cap hints, status, and docs refs.

Operator projections can include safe internal economics, provider binding, and
operator-note refs. They still do not allow raw provider credentials, private
payment material, customer data, raw prompts, or wallet material.

## Non-Goals

This slice does not:

- persist catalog records in D1;
- create checkout intents;
- create payment challenges;
- mint or verify L402 credentials;
- debit credits;
- grant entitlements;
- settle provider payouts.

Those are covered by #291 and #292 plus the later Epic H issues.

## Verification

- `bun run --cwd workers/api test -- src/paid-endpoint-product-catalog.test.ts`
- `bun run --cwd workers/api typecheck`
