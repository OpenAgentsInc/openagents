import { Effect } from 'effect'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
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

export const makeMarketingAgencyReceiptPublicRoutes = () => {
  const routeMarketingAgencyReceiptRequest = (
    request: Request,
  ): Effect.Effect<HttpResponse> | undefined => {
    const receiptRef = receiptRefFromPath(
      new URL(request.url).pathname,
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
