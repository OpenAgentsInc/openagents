import { Effect } from 'effect'

import {
  BusinessAlreadySoldReceiptStaleness,
  type BusinessAlreadySoldEngagementReceiptStore,
  projectBusinessAlreadySoldEngagementReceipts,
  publicBusinessAlreadySoldEngagementReceiptProjection,
} from './business-already-sold-engagement-receipt'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
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

const receiptRefFromPath = (
  pathname: string,
  prefix: string,
): string | null =>
  pathname.startsWith(prefix) && pathname.length > prefix.length
    ? decodeURIComponent(pathname.slice(prefix.length))
    : null

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

    const receiptRef = receiptRefFromPath(
      url.pathname,
      `${BusinessAlreadySoldEngagementReceiptsEndpoint}/`,
    )

    if (receiptRef === null) {
      return undefined
    }

    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    const receipt = dependencies
      .makeReceiptStore(env)
      .list()
      .find(input => input.receiptRef === receiptRef)

    if (receipt === undefined) {
      return Effect.succeed(
        noStoreJsonResponse(
          { error: 'not_found', reason: 'Receipt not found.' },
          { status: 404 },
        ),
      )
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
