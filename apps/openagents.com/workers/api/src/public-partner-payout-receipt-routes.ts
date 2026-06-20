import { notFound } from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
} from './http/responses'
import type { PartnerPayoutReceiptStore } from './partner-payout-receipts'

type HttpResponse = globalThis.Response

export type PublicPartnerPayoutReceiptRouteDependencies<Bindings> = Readonly<{
  makeStore: (env: Bindings) => PartnerPayoutReceiptStore
  nowIso: () => string
}>

const receiptRefFromPath = (pathname: string): string | null => {
  const prefix = '/api/public/partner-payout-receipts/'
  return pathname.startsWith(prefix) && pathname.length > prefix.length
    ? decodeURIComponent(pathname.slice(prefix.length))
    : null
}

const readReceiptResponse = <Bindings>(
  dependencies: PublicPartnerPayoutReceiptRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  receiptRef: string,
): Effect.Effect<HttpResponse> =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.tryPromise({
        catch: () => 'partner_payout_receipt_read_failed' as const,
        try: () =>
          dependencies
            .makeStore(env)
            .readPartnerPayoutReceipt(receiptRef, dependencies.nowIso()),
      }).pipe(
        Effect.map(receipt =>
          receipt === null ? notFound() : noStoreJsonResponse({ receipt }),
        ),
        Effect.catch(() => Effect.succeed(serverError())),
      )

export const makePublicPartnerPayoutReceiptRoutes = <Bindings>(
  dependencies: PublicPartnerPayoutReceiptRouteDependencies<Bindings>,
) => ({
  routePublicPartnerPayoutReceiptRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const receiptRef = receiptRefFromPath(new URL(request.url).pathname)
    return receiptRef === null
      ? undefined
      : readReceiptResponse(dependencies, request, env, receiptRef)
  },
})
