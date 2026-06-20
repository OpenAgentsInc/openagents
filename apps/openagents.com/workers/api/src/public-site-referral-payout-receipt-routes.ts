import { notFound } from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
} from './http/responses'
import type { SiteReferralPayoutReceiptStore } from './site-referral-payout-receipts'

type HttpResponse = globalThis.Response

export type PublicSiteReferralPayoutReceiptRouteDependencies<Bindings> =
  Readonly<{
    makeStore: (env: Bindings) => SiteReferralPayoutReceiptStore
    nowIso: () => string
  }>

const receiptRefFromPath = (pathname: string): string | null => {
  const prefix = '/api/public/site-referral-payout-receipts/'
  return pathname.startsWith(prefix) && pathname.length > prefix.length
    ? decodeURIComponent(pathname.slice(prefix.length))
    : null
}

const readReceiptResponse = <Bindings>(
  dependencies: PublicSiteReferralPayoutReceiptRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  receiptRef: string,
): Effect.Effect<HttpResponse> =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.tryPromise({
        catch: () => 'site_referral_payout_receipt_read_failed' as const,
        try: () =>
          dependencies
            .makeStore(env)
            .readSiteReferralPayoutReceipt(receiptRef, dependencies.nowIso()),
      }).pipe(
        Effect.map(receipt =>
          receipt === null ? notFound() : noStoreJsonResponse({ receipt }),
        ),
        Effect.catch(() => Effect.succeed(serverError())),
      )

export const makePublicSiteReferralPayoutReceiptRoutes = <Bindings>(
  dependencies: PublicSiteReferralPayoutReceiptRouteDependencies<Bindings>,
) => ({
  routePublicSiteReferralPayoutReceiptRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const receiptRef = receiptRefFromPath(new URL(request.url).pathname)
    return receiptRef === null
      ? undefined
      : readReceiptResponse(dependencies, request, env, receiptRef)
  },
})
