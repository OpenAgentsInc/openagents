# autopilot.agentic_labor_products.v1 — vertex-fleet worker note

Date: 2026-06-20
Promise: `autopilot.agentic_labor_products.v1` (yellow — **stays yellow**)

## Blocker advanced

`blocker.product_promises.agentic_labor_product_real_sale_receipt_missing`
(NOT cleared — see "What remains").

## What this run built

The labor-product flow had two ends but no middle:

- a **self-serve order** path (`planSelfServeLaborProductOrder`) that yields an
  `ordered`-stage flow plan, and
- a **settlement seam** (`settleLaborProductOrder`) that only acts on a
  `delivered` plan.

There was no typed way to carry an `ordered` order forward through `dispatch` ->
`deliver`, so the settlement seam was **unreachable from a self-serve order** —
a real sale could never be carried through end to end.

This run adds that missing connective tissue:

- `advanceLaborProductFlow(plan, transition)` in
  `apps/openagents.com/workers/api/src/agentic-labor-product.ts` — a **pure,
  forward-only, single-stage** transition:
  - `{ kind: 'dispatch', workerRef }` requires an `ordered` flow + non-empty
    worker, yields a `dispatched` plan;
  - `{ kind: 'deliver', artifactRef }` requires a `dispatched` flow + non-empty
    artifact, yields a `delivered` plan (the only stage the seam settles).
  - The order's identity (orderId, listing, buyerRef, createdAt, and the
    public-safe would-be settlement receipt ref) is carried unchanged.
  - Rebuilt through the same `buildLaborProductFlowPlan` validator, so every
    lifecycle-coherence guarantee holds; the result stays `inert: true` /
    `promiseState: 'yellow'`.

With this, the full in-memory carry-through exists and is tested:
`order -> dispatch -> deliver -> settle`.

### INERT and honest

`advanceLaborProductFlow` dispatches no worker, performs no delivery effect,
moves no money, and writes no receipt — it only computes the next coherent flow
plan. It does **not** touch the ledger and is **not** wired into any live route
that settles. Settlement remains the flag-gated + owner-gated seam.

## Tests

- `apps/openagents.com/workers/api/src/agentic-labor-product.test.ts` — new
  `advanceLaborProductFlow` block: full `ordered -> dispatched -> delivered`
  carry-through (identity + receipt-ref preserved, inert/yellow at each step),
  plus stage-guard and empty-field rejections.

## Validation

- `bunx tsc -p tsconfig.json --noEmit` (workers/api): 0 errors from this change.
  One **pre-existing, unrelated** error remains —
  `src/training-data-refinery.ts(18,3): TS6133 'Cs336A4EvalDeltaMeasurementRef'
  is declared but its value is never read` — confirmed present on the base
  branch before my change (verified via `git stash`).
- `bun run check:deploy` (apps/openagents.com): passed.
- `git diff --check`: clean.

## What remains (blocker NOT cleared)

Per `proof.claim_upgrade_receipts.v1` + `proof.demand_provenance.v1`, green still
needs a **real** sale, not a typed carry-through:

1. A real labor product **ordered by an external buyer** (demand provenance).
2. Carried `order -> dispatch -> deliver -> settle` with the settlement flag
   **armed**, producing a **dereferenceable settlement receipt** on the `labor`
   stream.
3. **Owner sign-off** on the upgrade.

This run made the carry-through *expressible and tested*; it did not produce a
real settled sale, so the blocker stays listed.
