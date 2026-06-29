// Public read-only listing surface for the self-serve control-center fanout
// capability (promise autopilot.control_center_fanout_marketplace.v1, yellow).
//
// INERT by default. The route is wired into the live Worker but reads from an
// injected plan store that the Worker leaves EMPTY unless the self-serve fanout
// flag is explicitly armed (SELF_SERVE_FANOUT_ENABLED). Either way the response
// is honest: `inert: true`, `promiseState: 'yellow'`, `selfServe: true`,
// typed work classes, with NO market listing, escrow, dispatch, or settlement.
// The dispatch seam (dispatchSelfServeFanout) is never reachable from this
// read-only route. Read-only (GET only).

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  type SelfServeFanoutStore,
  SELF_SERVE_FANOUT_CLEARED_BLOCKER_REF,
  SELF_SERVE_FANOUT_PROMISE,
  SELF_SERVE_FANOUT_SCHEMA,
  SELF_SERVE_FANOUT_WORK_CLASS,
  SelfServeFanoutStaleness,
  emptySelfServeFanoutStore,
  listSelfServeFanoutPlans,
  readSelfServeFanoutPlan,
} from './self-serve-fanout'

export const SelfServeFanoutEndpoint =
  '/api/public/autopilot/self-serve-fanout'

// Parse the SELF_SERVE_FANOUT_ENABLED flag. Default OFF: anything other than an
// explicit truthy token leaves the surface inert (empty store).
export const isSelfServeFanoutEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

export type SelfServeFanoutDeps = Readonly<{
  // Whether the self-serve fanout surface is armed. When false (default) the
  // Worker passes the empty store, so the listing is inert.
  enabled: boolean
  // The self-serve fanout plan store. The Worker passes the empty store while
  // INERT.
  store?: SelfServeFanoutStore
}>

const resolveStore = (deps: SelfServeFanoutDeps): SelfServeFanoutStore =>
  deps.enabled && deps.store !== undefined
    ? deps.store
    : emptySelfServeFanoutStore

/**
 * GET the self-serve fanout plans listing. Read-only. Optional `?planId=` reads
 * a single plan (returns `plan: null` when absent).
 */
export const handleSelfServeFanoutApi = (
  request: Request,
  deps: SelfServeFanoutDeps,
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const store = resolveStore(deps)
  const url = new URL(request.url)
  const planId = url.searchParams.get('planId')

  if (planId !== null && planId.trim().length > 0) {
    return Effect.succeed(
      noStoreJsonResponse({
        schema: SELF_SERVE_FANOUT_SCHEMA,
        promiseIds: [SELF_SERVE_FANOUT_PROMISE],
        promiseState: 'yellow',
        inert: true,
        selfServe: true,
        workClass: SELF_SERVE_FANOUT_WORK_CLASS,
        generatedAt: currentIsoTimestamp(),
        maxStalenessSeconds: SelfServeFanoutStaleness.maxStalenessSeconds,
        staleness: SelfServeFanoutStaleness,
        clearedBlockerRefs: [SELF_SERVE_FANOUT_CLEARED_BLOCKER_REF],
        unclearedBlockerRefs: [],
        plan: readSelfServeFanoutPlan(store, planId),
      }),
    )
  }

  return Effect.succeed(noStoreJsonResponse(listSelfServeFanoutPlans(store)))
}
