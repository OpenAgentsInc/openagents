# RX-11 Reactor Improvement Ladder

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-07-04
Issue: [#8279](https://github.com/OpenAgentsInc/openagents/issues/8279)
Status: design doc plus internal dogfood receipts landed; no customer flywheel
or public capability claim.

RX-11 makes Reactor's improvement ladder explicit:

1. **Harness evolution**: Mutalisk proposes one deterministic harness-code
   mechanism at a time, Psionic evaluates it, and acceptance requires a measured
   eval delta above the noise floor. No model weights change on this rung.
2. **Distill-to-fit**: capture the observed input distribution, select a smaller
   candidate model/artifact, distill against the bounded distribution, and allow
   any swap only through the RX-3 eval-gated router with model policy still
   revalidated.
3. **Flywheel training**: once the harness has flattened, customer interaction
   data can become a consented dataset snapshot and training run inside the
   customer boundary. Resulting weights are owned by the customer.

The ordering follows the harness-optimization audit: evolve the wrapper first,
then make weights carry the work only when deterministic harness gains flatten.

## Customer-Boundary Design

For customer deployments, every ladder run must record:

- consent receipt ref before interaction data is captured;
- dataset snapshot ref;
- training or harness run ref;
- eval delta ref;
- boundary ref (`customer_premises` or BF-3.4 regulated-private lane);
- resulting weights owner ref;
- model-policy ref/version for both source and candidate artifacts.

Receipts, metrics, and opaque refs may leave the boundary. Raw customer
interaction data, prompts, documents, outputs, and trained weights do not.

## Distill-To-Fit Gate

Distill-to-fit is not a free model swap. The candidate artifact must pass:

- input-distribution capture receipt;
- distillation run receipt;
- eval delta receipt covering quality and cost;
- `reactor.model_policy.v1` revalidation on the source and distilled artifact;
- RX-3 router gate before any route can move.

The source code receipt intentionally keeps `routeSwapAuthorized: false` for
the dogfood fixture: it records the experiment and measured delta, not a live
production swap.

## Dogfood Receipts

`packages/reactor-contracts` now exports four RX-11 receipts:

- `openagents.reactor.improvement_ladder_plan_receipt.v1`
- `openagents.reactor.harness_evolution_dogfood_receipt.v1`
- `openagents.reactor.distill_to_fit_dogfood_receipt.v1`
- `openagents.reactor.improvement_ladder_dogfood_receipt.v1`

The dogfood harness-evolution receipt records a Psionic/Mutalisk
`deliverable_landing` mechanism on internal OpenAgents lead-gen traffic:
baseline score 63.40%, candidate score 80.10%, delta +16.70 points, accepted
above a 1-point threshold, with `weightChangesAllowed: false`.

The dogfood distill-to-fit receipt records an internal OpenAgents shrink
experiment: baseline quality 80.10%, candidate quality 78.90%, delta -1.20
points, with cost per 1K tokens reduced from 420 to 176 microusd
(58.10% reduction). Policy is revalidated, the RX-3 router gate is passed,
and route swap remains unauthorized.

The aggregate dogfood receipt stays internal and claim-blocked. It requires a
real customer consent receipt, customer-boundary run, and owner-approved public
copy before any external claim can move.

## Verification

The guard runs in two places:

- `packages/reactor-contracts/src/index.test.ts`
- `apps/openagents.com/workers/api/src/reactor-improvement-ladder.test.ts`

The Worker test is included in `apps/openagents.com` `check:deploy`.

## Boundary

This clears only the source-level design and internal dogfood receipt blocker.
It does not create a customer flywheel, customer dataset, customer-owned
weights, public training capability, customer route swap, external pilot,
pricing, compliance claim, payout, or settlement.
