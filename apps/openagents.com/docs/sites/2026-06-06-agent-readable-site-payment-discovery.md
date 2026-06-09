# Agent-Readable Site Payment Discovery

Issue #303 adds the agent-facing discovery contract for generated Site
commerce.

The implementation lives in `workers/api/src/site-payment-discovery.ts` and is
served through:

```text
GET /api/sites/{siteId}/commerce/discovery
```

## What Agents Can Read

The discovery projection includes:

- checkout products and paid actions;
- catalog refs, product IDs, action IDs, paths, and methods;
- prices and exact denominations;
- checkout intent endpoints;
- commerce review endpoints;
- payment proof endpoints;
- L402 challenge and redemption endpoints for paid actions;
- `WWW-Authenticate: L402` header semantics;
- entitlement scope refs;
- spend-cap hint refs;
- sandbox state;
- public projection state;
- whether each surface is available, fake-provider-only, gated, or planned.

## Current Surface States

- Checkout intent: fake-provider-only contract.
- Commerce review: available.
- Commerce review decision: operator-gated.
- Payment proof: available.
- L402 challenge: available contract.
- L402 redemption: available contract.
- WFP payment middleware: available contract.
- Entitlement projection: available through checkout return and payment proof
  reads when durable records exist.
- Final settlement: planned.

This means agents can discover and validate shape, but must not claim final
payment settlement, payout, or accepted-work state.

Commerce review is available through:

```text
GET /api/sites/{siteId}/commerce/review
```

It projects generated checkout products and paid actions with review state and
source-safe checkout UI primitive refs. Review decisions are written through:

```text
POST /api/sites/{siteId}/commerce/review-decisions
```

That write route is operator-gated and idempotent. A decision can accept, hold,
reject, or request customer input for a catalog item, but it does not create
payment, payout, settlement, access, or deployment authority.

## Redaction Boundary

The projection rejects or removes customer private data, raw payment material,
wallet state, MDK credentials, provider grants, provider payout claims,
checkout query state, source archives, runner logs, and secrets.

## Related Public Docs

- `/AGENTS.md` now points agents to payment discovery before checkout or L402
  work.
- `/.well-known/openagents.json` lists the payment discovery resource and
  Site checkout/L402 action contracts.
- `/api/openapi.json` documents the discovery endpoint and response envelope.

## Verification

- `bun run --cwd workers/api test -- src/site-payment-discovery.test.ts src/site-commerce-routes.test.ts`
- `bun run --cwd workers/api test -- src/openagents-openapi-routes.test.ts src/openagents-capability-manifest-routes.test.ts src/openagents-agent-onboarding-routes.test.ts`
- `bun run --cwd workers/api typecheck`
