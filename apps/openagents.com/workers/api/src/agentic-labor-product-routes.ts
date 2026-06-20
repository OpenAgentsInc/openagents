// Public read-only listing surface for the agentic labor-product flow scaffold
// (promise autopilot.agentic_labor_products.v1, yellow; docs/transcripts/239.md).
//
// INERT by default. The route is wired into the live Worker but reads from an
// injected flow store that the Worker leaves EMPTY unless the labor-product flag
// is explicitly armed (AGENTIC_LABOR_PRODUCTS_ENABLED). Either way the response
// is honest: `inert: true` and `promiseState: 'yellow'`, with NO sale, billing,
// balance debit, or live-product claim. The settlement seam
// (settleLaborProductOrder) is never reachable from this read-only route.
// Read-only (GET only).

import { Effect } from 'effect'

import {
  type LaborProductFlowStore,
  LaborProductFlowStaleness,
  emptyLaborProductFlowStore,
  listLaborProductFlows,
  readLaborProductFlow,
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
}>

const resolveStore = (
  deps: AgenticLaborProductDeps,
): LaborProductFlowStore =>
  deps.enabled && deps.store !== undefined
    ? deps.store
    : emptyLaborProductFlowStore

/**
 * GET the labor-product flows listing. Read-only. Optional `?orderId=` reads a
 * single flow (returns `flow: null` when absent).
 */
export const handleAgenticLaborProductApi = (
  request: Request,
  deps: AgenticLaborProductDeps,
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const store = resolveStore(deps)
  const url = new URL(request.url)
  const orderId = url.searchParams.get('orderId')

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
