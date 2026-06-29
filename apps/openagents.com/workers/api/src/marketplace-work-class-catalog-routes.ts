// Public read-only listing surface for the marketplace work-class catalog
// (promise autopilot.control_center_fanout_marketplace.v1, yellow).
//
// HONESTY / SCOPE: this route exposes the catalog projection unchanged. It is
// ALWAYS honest: `inert: false`, `promiseState: 'yellow'`, the live work
// classes (`code_task` and `data_labeling`), and no uncleared
// plugin-marketplace-beyond-code_task blocker. There is NO flag and NO store to
// arm here because the catalog itself only declares typed work-class contracts;
// `assertCatalogInvariants` (called inside the projection) throws rather than let
// a misedit silently over-claim live fanout support. This route lists no market
// job, opens no escrow, and moves no money; it is purely a read-only registry
// view. Read-only (GET only).

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  getMarketplaceWorkClass,
  projectMarketplaceWorkClassCatalog,
} from './marketplace-work-class-catalog'

export const MarketplaceWorkClassCatalogEndpoint =
  '/api/public/autopilot/marketplace-work-classes'

/**
 * GET the marketplace work-class catalog projection. Read-only. Optional
 * `?workClass=` narrows the response to a single class (`workClass: null` when
 * the id is unknown), keeping the same honest envelope.
 */
export const handleMarketplaceWorkClassCatalogApi = (
  request: Request,
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const projection = projectMarketplaceWorkClassCatalog()
  const url = new URL(request.url)
  const workClass = url.searchParams.get('workClass')

  if (workClass !== null && workClass.trim().length > 0) {
    const entry = getMarketplaceWorkClass(workClass.trim())
    return Effect.succeed(
      noStoreJsonResponse({
        schema: projection.schema,
        promiseIds: projection.promiseIds,
        promiseState: projection.promiseState,
        inert: projection.inert,
        liveWorkClass: projection.liveWorkClass,
        pluginMarketplaceBeyondCodeTaskLive:
          projection.pluginMarketplaceBeyondCodeTaskLive,
        generatedAt: projection.generatedAt,
        maxStalenessSeconds: projection.maxStalenessSeconds,
        staleness: projection.staleness,
        unclearedBlockerRefs: projection.unclearedBlockerRefs,
        workClass: entry,
      }),
    )
  }

  return Effect.succeed(noStoreJsonResponse(projection))
}
