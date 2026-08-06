import { Effect } from 'effect'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { pathRefFromPrefix } from './http/router'
import {
  type MarketingAgencyPaidDeliveryClaimStore,
  projectMarketingAgencyPaidDeliveryClaims,
} from './marketing-agency-claim-upgrade'
import { firstPaidMarketingAgencyDeliveryReceiptFixture } from './marketing-agency-delivery-receipt-fixture'
import { liveAtReadStaleness } from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

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

export type MarketingAgencyReceiptRoutesDependencies<Bindings> = Readonly<{
  makeClaimStore: (env: Bindings) => MarketingAgencyPaidDeliveryClaimStore
}>

export const makeMarketingAgencyReceiptPublicRoutes = <Bindings>(
  dependencies: MarketingAgencyReceiptRoutesDependencies<Bindings>,
) => {
  const routeMarketingAgencyReceiptRequest = (
    request: Request,
    env: Bindings
  ): Effect.Effect<HttpResponse> | undefined => {

    const url = new URL(request.url)

    if (
      url.pathname === '/api/public/marketing-agency/receipts' &&
      url.searchParams.get('view') === 'paid-delivery-claims'
    ) {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }
      return Effect.tryPromise({
        catch: () => 'claim_store_failed' as const,
        try: async () => dependencies.makeClaimStore(env).list(),
      }).pipe(
        Effect.map(claims =>
          noStoreJsonResponse(projectMarketingAgencyPaidDeliveryClaims(claims))
        ),
        Effect.catch(() => Effect.succeed(noStoreJsonResponse({ error: 'server_error' }, { status: 500 }))),
      )
    }

    const match = pathRefFromPrefix(
      url.pathname,
      '/api/public/marketing-agency/receipts/',
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

    // Expose the mocked first-paid receipt fixture for the blocker.
    if (receiptRef === firstPaidMarketingAgencyDeliveryReceiptFixture.workItemRef) {
        return Effect.succeed(noStoreJsonResponse({
          generatedAt: currentIsoTimestamp(),
          staleness: liveAtReadStaleness(['fixture_only']),
          receipt: firstPaidMarketingAgencyDeliveryReceiptFixture
        }))
    }

    return receiptNotFoundResponse()
  }

  return { routeMarketingAgencyReceiptRequest }
}
