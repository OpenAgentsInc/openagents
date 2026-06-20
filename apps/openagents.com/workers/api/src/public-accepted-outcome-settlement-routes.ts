// COORDINATOR WIRING:
// Add to workers/api/src/http/router.ts (mirroring the other public route
// wiring). This lane does NOT edit router.ts or index.ts.
//
//   import { makePublicAcceptedOutcomeSettlementRoutes } from '../public-accepted-outcome-settlement-routes'
//
//   const publicAcceptedOutcomeSettlementRoutes =
//     makePublicAcceptedOutcomeSettlementRoutes<Env>({ db: env => env.DB })
//
//   const settlementResponse =
//     publicAcceptedOutcomeSettlementRoutes.routePublicAcceptedOutcomeSettlementRequest(
//       request,
//       env,
//     )
//   if (settlementResponse !== undefined) {
//     return await runEffectProgram(settlementResponse)
//   }

import { notFound } from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import { publicOmniAcceptedOutcomeSettlementBundleProjection } from './omni-accepted-outcome-settlement-bundle'
import { dereferenceOmniAcceptedOutcomeSettlementBundle } from './omni-accepted-outcome-settlement-bundle-store'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
} from './http/responses'

type HttpResponse = globalThis.Response

export type PublicAcceptedOutcomeSettlementRouteDependencies<Bindings> =
  Readonly<{
    db: (env: Bindings) => D1Database
  }>

const PREFIX = '/api/public/accepted-outcome/settlement/'

const economicsIdFromPath = (pathname: string): string | null =>
  pathname.startsWith(PREFIX) && pathname.length > PREFIX.length
    ? decodeURIComponent(pathname.slice(PREFIX.length))
    : null

/**
 * Public, read-only projection of one accepted outcome's INERT settlement
 * bundle: the eight ordered settlement states (with honest evidence labels and
 * movedMoney flags), plus the contributor accrual ledger and gross-margin receipt
 * lifecycle -- all with internal monetary figures dropped. No private data, no
 * money movement, GET only.
 */
const readSettlementBundleResponse = <Bindings>(
  dependencies: PublicAcceptedOutcomeSettlementRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  economicsId: string,
): Effect.Effect<HttpResponse> =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : dereferenceOmniAcceptedOutcomeSettlementBundle(
        dependencies.db(env),
        economicsId,
      ).pipe(
        Effect.map(bundle =>
          bundle === null
            ? notFound()
            : noStoreJsonResponse({
                settlement:
                  publicOmniAcceptedOutcomeSettlementBundleProjection(bundle),
              }),
        ),
        // Both an unattributable record and a storage fault collapse to a 500
        // here rather than leaking internal reasons publicly.
        Effect.catch(() => Effect.succeed(serverError())),
      )

export const makePublicAcceptedOutcomeSettlementRoutes = <Bindings>(
  dependencies: PublicAcceptedOutcomeSettlementRouteDependencies<Bindings>,
) => {
  const routePublicAcceptedOutcomeSettlementRequest = (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const economicsId = economicsIdFromPath(new URL(request.url).pathname)

    return economicsId === null
      ? undefined
      : readSettlementBundleResponse(dependencies, request, env, economicsId)
  }

  return { routePublicAcceptedOutcomeSettlementRequest }
}
