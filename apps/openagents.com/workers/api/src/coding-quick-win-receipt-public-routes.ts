import { Effect } from 'effect'

import {
  type CodingQuickWinPaidDeliveryClaimStore,
  projectCodingQuickWinPaidDeliveryClaims,
  projectCodingQuickWinReceiptRead,
} from './coding-quick-win-claim-upgrade'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { pathRefFromPrefix } from './http/router'
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

    const match = pathRefFromPrefix(
      url.pathname,
      `${CodingQuickWinReceiptsEndpoint}/`,
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
      .makeClaimStore(env)
      .list()
      .find(input => input.receiptRef === receiptRef)

    if (receipt === undefined) {
      return receiptNotFoundResponse()
    }

    return Effect.succeed(
      noStoreJsonResponse(
        withDeclaredStaleness(projectCodingQuickWinReceiptRead(receipt)),
      ),
    )
  },
})
