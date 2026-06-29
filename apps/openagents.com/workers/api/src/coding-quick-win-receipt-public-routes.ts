import { Effect } from 'effect'

import {
  type CodingQuickWinPaidDeliveryClaimStore,
  projectCodingQuickWinPaidDeliveryClaims,
  projectCodingQuickWinReceiptRead,
} from './coding-quick-win-claim-upgrade'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import type { PublicProjectionStalenessContract } from './public-projection-staleness'

type HttpResponse = globalThis.Response

type PublicProjectionPayload = Readonly<{
  staleness: PublicProjectionStalenessContract
}>

const withDeclaredStaleness = <Payload extends PublicProjectionPayload>(
  payload: Payload,
): Payload => payload

export const CodingQuickWinReceiptsEndpoint =
  '/api/public/business/coding-quick-win-receipts' as const

export type CodingQuickWinReceiptRoutesDependencies<Bindings> = Readonly<{
  makeClaimStore: (env: Bindings) => CodingQuickWinPaidDeliveryClaimStore
}>

const receiptRefFromPath = (
  pathname: string,
  prefix: string,
): string | null =>
  pathname.startsWith(prefix) && pathname.length > prefix.length
    ? decodeURIComponent(pathname.slice(prefix.length))
    : null

export const makeCodingQuickWinReceiptPublicRoutes = <Bindings>(
  dependencies: CodingQuickWinReceiptRoutesDependencies<Bindings>,
) => ({
  routeCodingQuickWinReceiptRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (
      url.pathname === CodingQuickWinReceiptsEndpoint &&
      url.searchParams.get('view') === 'paid-delivery-claims'
    ) {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }
      const claims = dependencies.makeClaimStore(env).list()
      return Effect.succeed(
        noStoreJsonResponse(
          withDeclaredStaleness(projectCodingQuickWinPaidDeliveryClaims(claims)),
        ),
      )
    }

    const receiptRef = receiptRefFromPath(
      url.pathname,
      `${CodingQuickWinReceiptsEndpoint}/`,
    )

    if (receiptRef === null) {
      return undefined
    }

    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    const receipt = dependencies
      .makeClaimStore(env)
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
      noStoreJsonResponse(
        withDeclaredStaleness(projectCodingQuickWinReceiptRead(receipt)),
      ),
    )
  },
})
