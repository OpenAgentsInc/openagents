import { notFound } from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
} from './http/responses'
import type { CardCreditSpendReceiptStore } from './inference/card-credit-spend-receipt-store'

type HttpResponse = globalThis.Response

export type PublicCardCreditSpendReceiptRouteDependencies<Bindings> = Readonly<{
  makeStore: (env: Bindings) => CardCreditSpendReceiptStore
  nowIso: () => string
}>

const receiptRefFromPath = (pathname: string): string | null => {
  const prefix = '/api/public/inference/card-credit-spend-receipts/'
  return pathname.startsWith(prefix) && pathname.length > prefix.length
    ? decodeURIComponent(pathname.slice(prefix.length))
    : null
}

const readReceiptResponse = <Bindings>(
  dependencies: PublicCardCreditSpendReceiptRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  receiptRef: string,
): Effect.Effect<HttpResponse> =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.tryPromise({
        catch: () => 'card_credit_spend_receipt_read_failed' as const,
        try: () =>
          dependencies
            .makeStore(env)
            .readCardCreditSpendReceipt(receiptRef, dependencies.nowIso()),
      }).pipe(
        Effect.map(receipt =>
          receipt === null ? notFound() : noStoreJsonResponse({ receipt }),
        ),
        Effect.catch(() => Effect.succeed(serverError())),
      )

export const makePublicCardCreditSpendReceiptRoutes = <Bindings>(
  dependencies: PublicCardCreditSpendReceiptRouteDependencies<Bindings>,
) => ({
  routePublicCardCreditSpendReceiptRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const receiptRef = receiptRefFromPath(new URL(request.url).pathname)
    return receiptRef === null
      ? undefined
      : readReceiptResponse(dependencies, request, env, receiptRef)
  },
})
