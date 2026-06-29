import { notFound } from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import {
  type Nip90MarketReceiptStore,
  publicNip90MarketReceiptFromRecord,
} from './nip90-market-receipts'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
} from './http/responses'

type HttpResponse = globalThis.Response

export type PublicNip90MarketReceiptRouteDependencies<Bindings> = Readonly<{
  makeStore: (env: Bindings) => Nip90MarketReceiptStore
}>

const receiptRefFromPath = (
  pathname: string,
  prefix: string,
): string | null =>
  pathname.startsWith(prefix) && pathname.length > prefix.length
    ? decodeURIComponent(pathname.slice(prefix.length))
    : null

const readReceiptResponse = <Bindings>(
  dependencies: PublicNip90MarketReceiptRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  receiptRef: string,
): Effect.Effect<HttpResponse> =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.tryPromise({
        catch: () => 'receipt_read_failed' as const,
        try: () =>
          dependencies.makeStore(env).readSettledMarketReceiptByRef(receiptRef),
      }).pipe(
        Effect.map(record => {
          const receipt = record === null
            ? null
            : publicNip90MarketReceiptFromRecord(record)

          return receipt === null
            ? notFound()
            : noStoreJsonResponse({ receipt })
        }),
        Effect.catch(() =>
          Effect.succeed(serverError())
        ),
      )

export const makePublicNip90MarketReceiptRoutes = <Bindings>(
  dependencies: PublicNip90MarketReceiptRouteDependencies<Bindings>,
) => {
  const routePublicNip90MarketReceiptRequest = (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const receiptRef = receiptRefFromPath(
      new URL(request.url).pathname,
      '/api/public/nip90-market/receipts/',
    )

    return receiptRef === null
      ? undefined
      : readReceiptResponse(dependencies, request, env, receiptRef)
  }

  return { routePublicNip90MarketReceiptRequest }
}
