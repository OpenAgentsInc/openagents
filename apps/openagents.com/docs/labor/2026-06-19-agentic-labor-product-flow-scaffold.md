# Agentic labor-product flow scaffold (autopilot.agentic_labor_products.v1)

Date: 2026-06-19
Promise: `autopilot.agentic_labor_products.v1` (yellow — stays yellow)
Branch: `wave3-agentic-labor-products`

## What this is

A typed, end-to-end **agentic labor-product flow** for the yellow promise
"OpenAgents sells agentic labor/products instead of dumb base-inference resale"
(docs/transcripts/239.md, "Let's Make Money"). It models the full
customer-facing lifecycle the promise's verification asks for — order, dispatch,
delivered artifact, and a settlement receipt seam — as an **INERT** scaffold
that touches **no real money** by default.

The flow is the missing customer-facing piece between two things that already
existed:

- The **lane-c fanout** proved a single *operator-staged* work order can fan to
  the open labor market and settle with public receipts
  (`autopilot.control_center_fanout_marketplace.v1`,
  `docs/labor/2026-06-14-p7-lane-c-fanout-closeout.md`).
- The **NIP-90 LBR rails** (`packages/nip90/src/lbr.ts`) prove the
  request → quote → acceptance → result event chain.

Neither models a labor **product** a buyer ORDERS off a listing and carries
through to a settled receipt. This scaffold is that flow.

## The flow (post → order → dispatch → deliver → settle)

`workers/api/src/agentic-labor-product.ts`:

1. **post** — an agent posts a `LaborProductListing` (a reusable, orderable unit
   of agentic labor: seller ref, title, capability ref, public-safe sats price).
2. **order** — a buyer orders it; `buildLaborProductFlowPlan` produces a typed
   `LaborProductFlowPlan` at the `ordered` stage.
3. **dispatch** — the order advances to `dispatched` with a `workerRef` (the
   lifecycle is coherent: a dispatched order must name a worker).
4. **deliver** — the order advances to `delivered` with a public-safe
   `artifactRef` (a delivered order must carry an artifact).
5. **settle** — `settleLaborProductOrder` settles the delivered order under the
   order's public-safe receipt ref, derived from the SAME shared cloud-metering
   receipt-ref helper (`cloudChargeReceiptRef`) every other priced primitive
   uses, on the NIP-90 `labor` stream.

The lifecycle stages (`LaborProductStage`) advance strictly forward:
`posted → ordered → dispatched → delivered → settled`.

## INERT / settlement seam (flag-gated, owner-gated)

`settleLaborProductOrder` reuses the cloud-metering / referral receipt patterns
and is **flag-gated INERT**:

- **Flag off (default)** → `{ _tag: 'disabled' }`: it returns the plan and
  touches **no ledger** (no money moved, no row written). Verified by a D1 stub
  that throws on any IO.
- **Armed but no owner sign-off, or order not delivered** →
  `{ _tag: 'not_authorized' }`: no debit. Green is delivery-and-owner-gated.
- **Armed + owner sign-off + delivered** → `{ _tag: 'settled' }`: a
  receipt-first charge runs through `settleCloudPrimitiveCharge` on the shared
  credit ledger — idempotent per order, never goes negative (the balance CHECK
  fails the batch otherwise). Verified against **real SQL** (node:sqlite) so the
  never-negative and idempotency guards are genuine, not modeled.

The public read-only route (`/api/public/autopilot/labor-products`,
`workers/api/src/agentic-labor-product-routes.ts`) is wired into the live Worker
but reads an EMPTY store unless `AGENTIC_LABOR_PRODUCTS_ENABLED` is armed. Either
way the response is honest: `inert: true`, `promiseState: 'yellow'`. The
settlement seam is never reachable from the read-only route.

## Why the promise stays YELLOW (not green)

A typed flow + an inert, owner-gated settlement seam is **not** proof of a real
labor product sold. The promise stays yellow and records two uncleared blockers:

- `blocker.product_promises.not_all_labor_flows_self_serve`
- `blocker.product_promises.agentic_labor_product_real_sale_receipt_missing`

## Receipt needed for green (owner-gated)

Green requires, per `proof.claim_upgrade_receipts.v1` +
`proof.demand_provenance.v1`:

1. A **real** agentic labor PRODUCT posted and **ordered by an external buyer**
   (demand provenance — internal use is plumbing proof, not market proof).
2. The order carried post → order → dispatch → deliver → **settle** with the
   settlement flag armed, producing a **dereferenceable settlement receipt**
   (resolvable via the public receipt surface, e.g.
   `/api/public/nip90-market/receipts/<ref>` on the `labor` stream).
3. **Owner sign-off** on the upgrade.

Until all three land, this scaffold advances the promise toward green without
flipping it.

## Files

- `workers/api/src/agentic-labor-product.ts` — flow model + INERT settlement seam
- `workers/api/src/agentic-labor-product-routes.ts` — read-only public route
- `workers/api/src/agentic-labor-product.test.ts` — model + seam tests
- `workers/api/src/agentic-labor-product-settlement.test.ts` — armed settlement
  against real SQL
- `workers/api/src/agentic-labor-product-routes.test.ts` — route tests
- `workers/api/src/config.ts` — `AGENTIC_LABOR_PRODUCTS_ENABLED` flag
- `workers/api/src/index.ts` — route registration
