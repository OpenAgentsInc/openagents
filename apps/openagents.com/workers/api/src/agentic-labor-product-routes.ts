// Public surface for the agentic labor-product flow scaffold
// (promise autopilot.agentic_labor_products.v1, yellow; docs/transcripts/239.md).
//
// GET  — read-only listing of flows from the injected store.
// POST — SELF-SERVE order planning: a buyer/agent posts a listing and orders it
//        in one request and gets back the typed flow plan, with NO operator
//        staging in the loop (closes blocker.product_promises
//        .not_all_labor_flows_self_serve). It is still INERT: it dispatches
//        nothing, debits nothing, writes no receipt, and settles nothing — it
//        returns a pure `ordered`-stage flow plan.
//
// INERT by default. The route is wired into the live Worker but is gated on the
// labor-product flag (AGENTIC_LABOR_PRODUCTS_ENABLED). When disabled the GET
// listing reads the EMPTY store and POST returns 503 (the self-serve path is not
// armed). Either way the response is honest: `inert: true` and
// `promiseState: 'yellow'`, with NO sale, billing, balance debit, or
// live-product claim. The settlement seam (settleLaborProductOrder) is never
// reachable from these routes.

import { Effect } from 'effect'

import {
  type LaborProductFlowStore,
  type LaborProductReceiptStore,
  AGENTIC_LABOR_PRODUCT_SCHEMA,
  AGENTIC_LABOR_PRODUCTS_PROMISE,
  LABOR_PRODUCT_NO_REAL_SALE_RECEIPT_REF,
  LaborProductFlowStaleness,
  decodeLaborProductOrderRequest,
  emptyLaborProductFlowStore,
  emptyLaborProductReceiptStore,
  listLaborProductFlows,
  planSelfServeLaborProductOrder,
  readLaborProductFlow,
  readLaborProductSettlementReceipt,
} from './agentic-labor-product'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { currentIsoTimestamp } from './runtime-primitives'

export const AgenticLaborProductEndpoint =
  '/api/public/autopilot/labor-products'

// Parse the AGENTIC_LABOR_PRODUCTS_ENABLED flag. Default OFF: anything other
// than an explicit truthy token leaves the surface inert (empty store).
export const isAgenticLaborProductsEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

export type AgenticLaborProductDeps = Readonly<{
  // Whether the labor-product surface is armed. When false (default) the Worker
  // passes the empty store, so the listing is inert.
  enabled: boolean
  // The labor-product flow store. The Worker passes the empty store while INERT.
  store?: LaborProductFlowStore
  // The settlement-receipt store a `?receiptRef=` GET dereferences against. The
  // Worker passes the empty store while INERT (no real receipt is published).
  receiptStore?: LaborProductReceiptStore
}>

const resolveStore = (
  deps: AgenticLaborProductDeps,
): LaborProductFlowStore =>
  deps.enabled && deps.store !== undefined
    ? deps.store
    : emptyLaborProductFlowStore

const resolveReceiptStore = (
  deps: AgenticLaborProductDeps,
): LaborProductReceiptStore =>
  deps.enabled && deps.receiptStore !== undefined
    ? deps.receiptStore
    : emptyLaborProductReceiptStore

/**
 * SELF-SERVE POST: plan a labor-product order from the buyer's own request body
 * with no operator staging. INERT: it dispatches nothing, debits nothing, writes
 * no receipt, and settles nothing — it returns a pure `ordered`-stage flow plan.
 * Gated on the flag: when disabled, 503 (the self-serve path is not armed). On a
 * malformed/invalid body, 400. On success, 200 with the typed flow plan, still
 * reporting `inert: true` / `promiseState: 'yellow'` and the uncleared
 * real-sale-receipt blocker.
 */
const handleSelfServeOrder = (request: Request, deps: AgenticLaborProductDeps) =>
  Effect.gen(function* () {
    if (!deps.enabled) {
      return noStoreJsonResponse(
        {
          error: 'agentic_labor_products_disabled',
          reason:
            'The self-serve labor-product flow is not armed (AGENTIC_LABOR_PRODUCTS_ENABLED is off).',
          promiseIds: [AGENTIC_LABOR_PRODUCTS_PROMISE],
          promiseState: 'yellow',
          inert: true,
        },
        { status: 503 },
      )
    }

    const body = yield* Effect.tryPromise({
      try: () => request.json() as Promise<unknown>,
      catch: () => 'invalid_json' as const,
    }).pipe(Effect.orElseSucceed(() => 'invalid_json' as const))

    if (body === 'invalid_json') {
      return noStoreJsonResponse(
        {
          error: 'invalid_request',
          reason: 'request body must be valid JSON',
          promiseIds: [AGENTIC_LABOR_PRODUCTS_PROMISE],
          promiseState: 'yellow',
          inert: true,
        },
        { status: 400 },
      )
    }

    const decoded = decodeLaborProductOrderRequest(body)
    if (!decoded.ok) {
      return noStoreJsonResponse(
        {
          error: 'invalid_request',
          reason: decoded.error.reason,
          promiseIds: [AGENTIC_LABOR_PRODUCTS_PROMISE],
          promiseState: 'yellow',
          inert: true,
        },
        { status: 400 },
      )
    }

    const planned = planSelfServeLaborProductOrder(decoded.request)
    if (!planned.ok) {
      return noStoreJsonResponse(
        {
          error: 'invalid_request',
          reason: planned.error.reason,
          promiseIds: [AGENTIC_LABOR_PRODUCTS_PROMISE],
          promiseState: 'yellow',
          inert: true,
        },
        { status: 400 },
      )
    }

    return noStoreJsonResponse({
      schema: AGENTIC_LABOR_PRODUCT_SCHEMA,
      promiseIds: [AGENTIC_LABOR_PRODUCTS_PROMISE],
      promiseState: 'yellow',
      // INERT: a self-serve plan moves no money and dispatches nothing.
      inert: true,
      // The self-serve path now exists; the real-sale-receipt blocker stays.
      unclearedBlockerRefs: [LABOR_PRODUCT_NO_REAL_SALE_RECEIPT_REF],
      generatedAt: currentIsoTimestamp(),
      flow: planned.plan,
    })
  })

/**
 * Labor-product surface. GET lists flows (read-only); POST self-serve plans an
 * order. Optional `?orderId=` on GET reads a single flow (returns `flow: null`
 * when absent).
 */
export const handleAgenticLaborProductApi = (
  request: Request,
  deps: AgenticLaborProductDeps,
): Effect.Effect<Response> => {
  if (request.method === 'POST') {
    return handleSelfServeOrder(request, deps)
  }

  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET', 'POST']))
  }

  const store = resolveStore(deps)
  const url = new URL(request.url)
  const receiptRef = url.searchParams.get('receiptRef')
  const orderId = url.searchParams.get('orderId')

  // Dereference a published settlement receipt by its public-safe receiptRef.
  // Read-only and INERT: in production the receipt store is empty (no real
  // settled receipt has been published), so this returns `receipt: null`.
  if (receiptRef !== null && receiptRef.trim().length > 0) {
    return Effect.succeed(
      noStoreJsonResponse({
        schema: 'openagents.agentic_labor_product.settlement_receipt.v1',
        promiseIds: [AGENTIC_LABOR_PRODUCTS_PROMISE],
        promiseState: 'yellow',
        unclearedBlockerRefs: [LABOR_PRODUCT_NO_REAL_SALE_RECEIPT_REF],
        generatedAt: currentIsoTimestamp(),
        maxStalenessSeconds: LaborProductFlowStaleness.maxStalenessSeconds,
        staleness: LaborProductFlowStaleness,
        receipt: readLaborProductSettlementReceipt(
          resolveReceiptStore(deps),
          receiptRef.trim(),
        ),
      }),
    )
  }

  if (orderId !== null && orderId.trim().length > 0) {
    return Effect.succeed(
      noStoreJsonResponse({
        schema: 'openagents.agentic_labor_product.v1',
        promiseIds: ['autopilot.agentic_labor_products.v1'],
        promiseState: 'yellow',
        inert: true,
        generatedAt: currentIsoTimestamp(),
        maxStalenessSeconds: LaborProductFlowStaleness.maxStalenessSeconds,
        staleness: LaborProductFlowStaleness,
        flow: readLaborProductFlow(store, orderId),
      }),
    )
  }

  return Effect.succeed(noStoreJsonResponse(listLaborProductFlows(store)))
}
