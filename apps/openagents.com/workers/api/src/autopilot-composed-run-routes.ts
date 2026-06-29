// Public read-only listing surface for the Autopilot all-in-one composed-run
// scaffold (EPIC #5510, child #5519; promises
// autopilot.all_in_one_business_system.v1 + cloud.primitives_suite.v1).
//
// INERT by default. The route is wired into the live Worker but reads from an
// injected composed-run store that the Worker leaves EMPTY unless the composed-
// run flag is explicitly armed (AUTOPILOT_COMPOSED_RUN_ENABLED). Either way the
// response is honest: `inert: true` and `promiseState: 'planned'`, with NO
// provisioning, billing, unified-balance debit, or live-business claim.
// Read-only (GET only).

import { Effect } from 'effect'

import {
  type ComposedRunStore,
  ComposedRunStaleness,
  emptyComposedRunStore,
  listComposedRuns,
  readComposedRun,
} from './autopilot-composed-run'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { currentIsoTimestamp } from './runtime-primitives'

export const AutopilotComposedRunEndpoint =
  '/api/public/autopilot/composed-runs'

// Parse the AUTOPILOT_COMPOSED_RUN_ENABLED flag. Default OFF: anything other
// than an explicit truthy token leaves the surface inert (empty store).
export const isAutopilotComposedRunEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

export type AutopilotComposedRunDeps = Readonly<{
  // Whether the composed-run surface is armed. When false (default) the Worker
  // passes the empty store, so the listing is inert.
  enabled: boolean
  // The composed-run store. The Worker passes the empty store while INERT.
  store?: ComposedRunStore
}>

const resolveStore = (deps: AutopilotComposedRunDeps): ComposedRunStore =>
  deps.enabled && deps.store !== undefined
    ? deps.store
    : emptyComposedRunStore

/**
 * GET the composed-runs listing. Read-only. Optional `?runId=` reads a single
 * composed run (returns `run: null` when absent).
 */
export const handleAutopilotComposedRunApi = (
  request: Request,
  deps: AutopilotComposedRunDeps,
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const store = resolveStore(deps)
  const url = new URL(request.url)
  const runId = url.searchParams.get('runId')

  if (runId !== null && runId.trim().length > 0) {
    return Effect.succeed(
      noStoreJsonResponse({
        schema: 'openagents.autopilot_composed_run.v1',
        promiseIds: [
          'autopilot.all_in_one_business_system.v1',
          'cloud.primitives_suite.v1',
        ],
        promiseState: 'planned',
        inert: true,
        generatedAt: currentIsoTimestamp(),
        maxStalenessSeconds: ComposedRunStaleness.maxStalenessSeconds,
        staleness: ComposedRunStaleness,
        run: readComposedRun(store, runId),
      }),
    )
  }

  return Effect.succeed(noStoreJsonResponse(listComposedRuns(store)))
}
