# RX-3 Reactor Serving Skeleton Receipt

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-07-04
Issue: [#8273](https://github.com/OpenAgentsInc/openagents/issues/8273)
Status: source skeleton landed; no deployed Reactor node

## What Landed

- `packages/reactor-contracts` now defines the lane-neutral serving skeleton:
  `openagents.reactor.node_model_profile.v1`,
  `openagents.reactor.model_install_receipt.v1`,
  `openagents.reactor.route_decision.v1`, and
  `openagents.reactor.local_token_metering_receipt.v1`.
- The fixture server-class profile declares `servingLane: hydralisk`, vLLM, an
  OpenAI-compatible local gateway, `offline_once_provisioned` serving-path
  networking, `phoneHomeAllowedInServingPath: false`, and exact local metering.
- `provisionReactorModel` recomputes `reactor.model_policy.v1` against the
  catalog and refuses nonconforming weights before pull.
- `routeReactorOpenAiCompatibleRequest` recomputes policy before routing, so a
  forged installed receipt cannot route a nonconforming model.
- `buildReactorLocalTokenMeteringReceipt` emits exact reconciled local token
  rows or `not_measured`; estimated usage labels are rejected.

## Boundary

This is a skeleton smoke over fixture weights. It grants no live serving,
customer deployment, model pull, customer-data custody, air-gap update,
compliance, pricing, payout, settlement, or public availability authority.

Hydralisk remains the default execution lane and Psionic remains the
by-exception lane; the contracts are lane-neutral through the profile
`servingLane` field so lane swaps are config and eval-gated cutovers, not
re-integrations.

## Verification

- `bun run --cwd packages/reactor-contracts test`
- `bun run --cwd packages/reactor-contracts typecheck`
- `bun run --cwd apps/openagents.com/workers/api test -- src/product-promises.test.ts`
