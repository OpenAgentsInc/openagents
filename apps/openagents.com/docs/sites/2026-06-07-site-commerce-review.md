# Site Commerce Review

Date: 2026-06-07
Issue: #440

## Summary

OpenAgents product surface now has a Site commerce review layer for generated checkout products and
paid actions.

The public-safe read route is:

```text
GET /api/sites/{siteId}/commerce/review
```

The operator-gated write route is:

```text
POST /api/sites/{siteId}/commerce/review-decisions
```

The read projection is designed for Site builder, operator, and agent review
flows. It shows generated Site commerce items before a Site version is saved or
deployed, without putting MDK credentials or payment material into generated
source.

## Review Projection

The projection includes:

- checkout products and paid actions from the Site payment catalog;
- catalog refs, product/action refs, checkout paths, prices, and denomination
  refs;
- customer-data requirement refs and label refs, not customer values;
- entitlement scope and spend-cap hint refs;
- sandbox or live-provider candidate classification;
- source-safe checkout UI primitive refs derived from the generated-source
  primitive contract;
- current review state: needs review, accepted, held, rejected, or needs
  customer input.

The projection also states that review decisions do not create:

- payment authority;
- payout authority;
- settlement claims;
- access changes;
- deployment authority.

## Decision Writes

Operators can record a decision for one catalog item with:

```json
{
  "catalogRef": "site_payment:site_otec:version_site_otec_v2:product:consultation_deposit",
  "reviewStatus": "accepted",
  "reasonRefs": ["reason.site_commerce_review.catalog_ok"]
}
```

Supported decision states are:

- `accepted`;
- `held`;
- `rejected`;
- `needs_customer_input`.

The decision route requires:

- OpenAgents admin API token;
- `Idempotency-Key`;
- an existing catalog item for the target Site.

The decision is persisted in `site_commerce_review_decisions`. Replays with the
same idempotency key return the existing decision. A later decision for the same
Site version and catalog ref updates the current review state.

## Source Safety

Generated Site source must continue to call OpenAgents product surface-hosted payment boundaries,
such as checkout intent, L402 challenge, and checkout return routes. It must
not embed:

- MDK access tokens;
- wallet mnemonics or wallet state;
- raw invoices, payment hashes, or preimages;
- provider grants;
- customer private values;
- checkout query state;
- payout targets or Treasury material.

The review projection rejects unsafe refs and omits raw timestamps from
customer-facing review output.

## Related Surfaces

- `/api/sites/{siteId}/commerce/discovery` now lists review endpoints and
  surface states.
- `/.well-known/openagents.json` lists review read and review-decision actions.
- `/api/openapi.json` documents the review route, decision request, and
  response envelopes.
- `/AGENTS.md` instructs agents to inspect commerce review before proposing
  generated checkout UI changes.

## Verification

- `bun run --cwd workers/api test -- src/site-commerce-review.test.ts src/site-payment-discovery.test.ts src/site-commerce-routes.test.ts`
- `bun run --cwd workers/api test -- src/openagents-openapi-routes.test.ts src/openagents-capability-manifest-routes.test.ts src/openagents-agent-onboarding-routes.test.ts`
- `bun run --cwd workers/api typecheck`
