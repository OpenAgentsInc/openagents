import { Effect } from 'effect'

import {
  BusinessAlreadySoldReceiptStaleness,
  type BusinessAlreadySoldEngagementReceiptStore,
  projectBusinessAlreadySoldEngagementReceipts,
  publicBusinessAlreadySoldEngagementReceiptProjection,
} from './business-already-sold-engagement-receipt'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { pathRefFromPrefix } from './http/router'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

export const BusinessAlreadySoldEngagementReceiptsEndpoint =
  '/api/public/business/already-sold-engagement-receipts' as const

export type BusinessAlreadySoldEngagementReceiptRoutesDependencies<Bindings> =
  Readonly<{
    makeReceiptStore: (
      env: Bindings,
    ) => BusinessAlreadySoldEngagementReceiptStore
  }>

/**
 * The 404 a caller gets for a ref this reader does not know. A ref whose
 * percent-escapes are malformed (PRO-1215) is answered with exactly this, after
 * the method check, because the two cases are indistinguishable to a caller and
 * 404 declines to leak that the ref was parsed at all. Previously the unguarded
 * decode threw a `URIError` defect out of the route matcher, which surfaced as
 * 500 plus a `severity: 'critical'` backend incident.
 */
const receiptNotFoundResponse = (): Effect.Effect<HttpResponse> =>
  Effect.succeed(
    noStoreJsonResponse(
      { error: 'not_found', reason: 'Receipt not found.' },
      { status: 404 },
    ),
  )

export const makeBusinessAlreadySoldEngagementReceiptRoutes = <Bindings>(
  dependencies: BusinessAlreadySoldEngagementReceiptRoutesDependencies<Bindings>,
) => ({
  routeBusinessAlreadySoldEngagementReceiptRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (
      url.pathname === BusinessAlreadySoldEngagementReceiptsEndpoint &&
      url.searchParams.get('view') === 'paid-business-receipts'
    ) {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }
      return Effect.succeed(
        noStoreJsonResponse(
          projectBusinessAlreadySoldEngagementReceipts(
            dependencies.makeReceiptStore(env).list(),
          ),
        ),
      )
    }

    const match = pathRefFromPrefix(
      url.pathname,
      `${BusinessAlreadySoldEngagementReceiptsEndpoint}/`,
    )

    if (match._tag === 'no_match') {
      return undefined
    }

    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    if (match._tag === 'malformed') {
      return receiptNotFoundResponse()
    }

    const receiptRef = match.ref

    const receipt = dependencies
      .makeReceiptStore(env)
      .list()
      .find(input => input.receiptRef === receiptRef)

    if (receipt === undefined) {
      return receiptNotFoundResponse()
    }

    return Effect.succeed(
      noStoreJsonResponse({
        generatedAt: currentIsoTimestamp(),
        staleness: BusinessAlreadySoldReceiptStaleness,
        maxStalenessSeconds:
          BusinessAlreadySoldReceiptStaleness.maxStalenessSeconds,
        receipt:
          publicBusinessAlreadySoldEngagementReceiptProjection(receipt),
        authorityBoundary:
          'This public-safe receipt read exposes only opaque already-sold business payment evidence and grants no delivery, payout, settlement, self-serve, or green-claim authority.',
      }),
    )
  },
})
