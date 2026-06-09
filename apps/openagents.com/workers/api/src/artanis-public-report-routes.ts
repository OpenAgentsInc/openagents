import { Effect } from 'effect'

import {
  ArtanisPublicReportUnsafe,
  artanisPublicReportSnapshot,
} from './artanis-public-report'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  type PublicPylonStatsStore,
  publicPylonStatsSnapshot,
} from './public-pylon-stats'
import { makeD1PylonApiStore } from './pylon-api'

const routeErrorResponse = (error: ArtanisPublicReportUnsafe) =>
  noStoreJsonResponse(
    {
      error: 'public_artanis_report_unsafe',
      reason: error.reason,
    },
    { status: 500 },
  )

type PublicArtanisReportRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  store?: PublicPylonStatsStore
}>

export const handlePublicArtanisReportApi = (
  request: Request,
  input: PublicArtanisReportRouteInput,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : publicPylonStatsSnapshot({
        store:
          input.store ?? makeD1PylonApiStore(input.OPENAGENTS_DB as D1Database),
      }).pipe(
        Effect.flatMap(pylonStats =>
          Effect.try({
            try: () =>
              noStoreJsonResponse(
                artanisPublicReportSnapshot({
                  pylonStats,
                }),
              ),
            catch: error =>
              error instanceof ArtanisPublicReportUnsafe
                ? error
                : new ArtanisPublicReportUnsafe({
                    reason: 'Artanis public report projection failed.',
                  }),
          }),
        ),
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
