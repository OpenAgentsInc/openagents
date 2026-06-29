// Public read-only multi-earning-node projection surface for Pylon
// (EPIC #5523 / DE-4 #5527; promise pylon.v0_3_multi_earning_node.v1, red).
//
// INERT by default. The route is wired into the live Worker but reads from an
// injected store that the Worker leaves EMPTY unless the surface flag is
// explicitly armed (PYLON_MULTI_EARNING_PROJECTION_ENABLED). Either way the
// response is honest: `inert: true`, `promiseState: 'red'`, the three
// install/receipt/settlement blockers still open, with NO settlement,
// live-earning, or install-closed claim. Read-only (GET only).

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  type PylonMultiEarningStore,
  emptyPylonMultiEarningStore,
  projectPylonMultiEarningNode,
} from './pylon-multi-earning-node'

export const PylonMultiEarningNodeEndpoint =
  '/api/public/pylon/multi-earning-node'

// Parse the PYLON_MULTI_EARNING_PROJECTION_ENABLED flag. Default OFF: anything
// other than an explicit truthy token leaves the surface inert (empty store).
export const isPylonMultiEarningProjectionEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

export type PylonMultiEarningNodeDeps = Readonly<{
  // Whether the projection is armed. When false (default) the Worker passes the
  // empty store, so the projection is inert.
  enabled: boolean
  // The earning store. The Worker passes the empty store while INERT.
  store?: PylonMultiEarningStore
}>

const resolveStore = (
  deps: PylonMultiEarningNodeDeps,
): PylonMultiEarningStore =>
  deps.enabled && deps.store !== undefined
    ? deps.store
    : emptyPylonMultiEarningStore

/**
 * GET the Pylon multi-earning-node projection. Read-only, no-store JSON.
 */
export const handlePylonMultiEarningNodeApi = (
  request: Request,
  deps: PylonMultiEarningNodeDeps,
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return Effect.succeed(
    noStoreJsonResponse(projectPylonMultiEarningNode(resolveStore(deps))),
  )
}
