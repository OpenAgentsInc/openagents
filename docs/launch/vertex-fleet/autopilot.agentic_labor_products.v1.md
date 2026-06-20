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

## Follow-up run (2026-06-20): settled-receipt recording

The carry-through previously stopped at the settlement seam's RESULT: nothing
turned a `settled` result into the lifecycle's terminal `settled` stage or into a
dereferenceable receipt artifact. The `settled` stage existed in the enum but was
**never produced anywhere**, and there was no receipt object a claim-upgrade
review could dereference.

This run adds the last connective piece:

- `recordLaborProductSettlement(plan, result)` in
  `apps/openagents.com/workers/api/src/agentic-labor-product.ts` — a **pure**
  transform of a genuine settlement into:
  - a `settled`-stage flow plan (rebuilt through `buildLaborProductFlowPlan`, so
    coherence holds), and
  - a typed, public-safe `LaborProductSettlementReceipt`
    (`openagents.agentic_labor_product.settlement_receipt.v1`) carrying the
    dereferenceable `receiptRef`, neutral seller/buyer/account refs, the `labor`
    stream, and `settled: true`.
  - It rejects anything that is not a genuine metered settlement (non-`settled`
    seam result, unmetered/zero-charge outcome, receipt-ref mismatch, or an
    order that was never delivered), so a receipt is only minted when money
    actually moved.
  - The receipt is honestly **not** marked `inert` (a real settlement moved
    money) but stays `promiseState: 'yellow'` — one settled order does not flip
    the promise.

Tests: new `recordLaborProductSettlement (PURE)` block in
`agentic-labor-product.test.ts` covering the happy path, all four rejection
guards, and the full `order -> dispatch -> deliver -> settle -> receipt`
carry-through.

This does **not** clear the blocker: it only makes a settlement *recordable*. A
real external sale (demand provenance) settled under an armed, owner-signed seam
with a published receipt is still required for green.
