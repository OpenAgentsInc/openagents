# Agentic labor-product self-serve order path (autopilot.agentic_labor_products.v1)

Date: 2026-06-20
Promise: `autopilot.agentic_labor_products.v1` (yellow — stays yellow)
Registry: `2026-06-20.9`

## What this adds

A **self-serve** order-planning path for the agentic labor-product flow. Before
this, a `LaborProductFlowPlan` could only be assembled **operator-side** — staged
by hand, the way the lane-c fanout
(`autopilot.control_center_fanout_marketplace.v1`) stages a work order. That was
the blocker `blocker.product_promises.not_all_labor_flows_self_serve`.

Now a buyer/agent can plan a labor-product order **with no operator in the loop**:

- `planSelfServeLaborProductOrder(request)` in
  `workers/api/src/agentic-labor-product.ts` builds a typed `ordered`-stage flow
  plan from the buyer's own request, validated by the same pure
  `buildLaborProductFlowPlan` lifecycle checks.
- `POST /api/public/autopilot/labor-products`
  (`workers/api/src/agentic-labor-product-routes.ts`) exposes it on the
  already-mounted live route. It decodes the request body
  (`decodeLaborProductOrderRequest`), plans the order, and returns the typed flow
  plan.

## Self-serve order request / response

Request body:

```json
{
  "orderId": "order-self-1",
  "buyerRef": "agent:buyer",
  "listing": {
    "listingId": "listing-1",
    "sellerRef": "agent:raynor",
    "title": "Repo triage labor product",
    "summary": "Triage one repo backlog and deliver a report.",
    "capabilityRef": "promise:autopilot.agentic_labor_products.v1",
    "priceSats": 100
  }
}
```

Response (200, when armed):

```json
{
  "schema": "openagents.agentic_labor_product.v1",
  "promiseIds": ["autopilot.agentic_labor_products.v1"],
  "promiseState": "yellow",
  "inert": true,
  "unclearedBlockerRefs": [
    "blocker.product_promises.agentic_labor_product_real_sale_receipt_missing"
  ],
  "flow": { "stage": "ordered", "workerRef": null, "artifactRef": null, ... }
}
```

## Still INERT and honest

The self-serve path is **flag-gated and INERT** end to end:

- `POST` returns **503** unless `AGENTIC_LABOR_PRODUCTS_ENABLED` is armed.
- Even when armed, the POST **dispatches nothing, debits nothing, writes no
  receipt, and settles nothing** — it returns a pure `ordered`-stage plan that
  carries only the public-safe would-be receipt ref (derived from the shared
  cloud-metering helper).
- The settlement seam (`settleLaborProductOrder`) is **never reachable** from the
  route and stays flag-gated + owner-gated.
- The response always reports `inert: true` / `promiseState: 'yellow'`.

## Blocker status

- **Cleared:** `blocker.product_promises.not_all_labor_flows_self_serve` — the
  flow now has a deployed self-serve order path (route + planner), not just an
  operator-staged one.
- **Stays:** `blocker.product_promises.agentic_labor_product_real_sale_receipt_missing`
  — no real labor product has been ordered by an external buyer and carried
  through to a settled receipt.

## What remains for green (owner/money-gated)

Per `proof.claim_upgrade_receipts.v1` + `proof.demand_provenance.v1`, green needs:

1. A real labor product **ordered by an external buyer** (demand provenance).
2. The order carried post -> order -> dispatch -> deliver -> **settle** with the
   settlement flag armed, producing a **dereferenceable settlement receipt** on
   the `labor` stream.
3. **Owner sign-off** on the upgrade.

## Tests

- `workers/api/src/agentic-labor-product.test.ts` — self-serve decode + plan
  cases (ordered-stage plan, validation rejection, receipt-ref parity).
- `workers/api/src/agentic-labor-product-routes.test.ts` — POST 503 (disabled),
  200 (armed, inert/yellow ordered plan), 400 (invalid body / non-JSON).

## Files

- `workers/api/src/agentic-labor-product.ts` — `planSelfServeLaborProductOrder`,
  `decodeLaborProductOrderRequest`, `LaborProductOrderRequest`
- `workers/api/src/agentic-labor-product-routes.ts` — self-serve POST handler
- `workers/api/src/index.ts` — route registration comment (GET + self-serve POST)
- `workers/api/src/product-promises.ts` — promise record (blocker dropped,
  evidence + verification updated, registry `2026-06-20.9`)
