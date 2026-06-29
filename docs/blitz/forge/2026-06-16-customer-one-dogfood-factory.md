# Customer #1 Dogfood Factory

Date: 2026-06-16
Scope: #5097, Epic D / customer #1 dogfood.

## What Shipped

The `/forge` factory dashboard now has a first-party dogfood status strip for
the OpenAgents development pipeline:

- open work is derived from the loaded Runs projection;
- accepted outcomes are derived from accepted Run states;
- incidents are derived from blocked, rejected, and invalid Run states;
- eligible nodes are derived from the provider-account pool summary;
- the strip marks itself `live` only when both Runs and provider-pool capacity
  are loaded.

The existing production-line dashboard continues to render stage-level metrics,
automation catalog rows, and tuning controls from the same typed projections.

## Honesty Boundary

The customer #1 strip does not claim private repo content, raw prompts, raw
shell logs, provider payloads, settlement, payout, accepted-work authority, or
merge authority. It is a product-surface projection over existing operator-safe
Run and provider-pool summaries.

Seeded placeholders remain visually tagged wherever a source projection is not
loaded. The strip is useful for dogfooding only when it reports the live state.

## Verification

Regression coverage lives in
`apps/openagents.com/apps/web/src/page/loggedIn/view.scene.test.ts`, which now
asserts the `/forge` dashboard renders the customer #1 dogfood panel from live
Runs and provider-pool fixture data.
