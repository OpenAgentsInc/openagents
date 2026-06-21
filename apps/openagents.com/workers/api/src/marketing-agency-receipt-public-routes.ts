import { Effect } from 'effect'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  type MarketingAgencyPaidDeliveryClaimStore,
  projectMarketingAgencyPaidDeliveryClaims,
} from './marketing-agency-claim-upgrade'
import { firstPaidMarketingAgencyDeliveryReceiptFixture } from './marketing-agency-delivery-receipt-fixture'
import { liveAtReadStaleness } from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

const receiptRefFromPath = (
  pathname: string,
  prefix: string,
): string | null =>
  pathname.startsWith(prefix) && pathname.length > prefix.length
    ? decodeURIComponent(pathname.slice(prefix.length))
    : null

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

    const receiptRef = receiptRefFromPath(
      url.pathname,
      '/api/public/marketing-agency/receipts/',
    )

    if (receiptRef === null) {
      return undefined
    }

    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    // Expose the mocked first-paid receipt fixture for the blocker.
    if (receiptRef === firstPaidMarketingAgencyDeliveryReceiptFixture.workItemRef) {
        return Effect.succeed(noStoreJsonResponse({
          generatedAt: currentIsoTimestamp(),
          staleness: liveAtReadStaleness(['fixture_only']),
          receipt: firstPaidMarketingAgencyDeliveryReceiptFixture
        }))
    }

    return Effect.succeed(noStoreJsonResponse({ error: 'not_found', reason: 'Receipt not found.' }, { status: 404 }))
  }

  return { routeMarketingAgencyReceiptRequest }
}
