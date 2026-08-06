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
import { pathRefFromPrefix } from './http/router'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

export type PublicNip90MarketReceiptRouteDependencies<Bindings> = Readonly<{
  makeStore: (env: Bindings) => Nip90MarketReceiptStore
}>

const RECEIPT_PATH_PREFIX = '/api/public/nip90-market/receipts/'

/**
 * A ref whose percent-escapes are malformed (PRO-1215). Answered exactly like a
 * well-formed unknown ref — 404 on GET, 405 on anything else — because the two
 * are indistinguishable to a caller. Previously the unguarded decode threw a
 * `URIError` defect out of the route matcher, which surfaced as 500 plus a
 * `severity: 'critical'` backend incident.
 */
const malformedRefResponse = (request: Request): Effect.Effect<HttpResponse> =>
  Effect.succeed(
    request.method === 'GET' ? notFound() : methodNotAllowed(['GET']),
  )

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
            : publicNip90MarketReceiptFromRecord(record, currentIsoTimestamp())

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
    const match = pathRefFromPrefix(
      new URL(request.url).pathname,
      RECEIPT_PATH_PREFIX,
    )

    if (match._tag === 'no_match') {
      return undefined
    }

    return match._tag === 'malformed'
      ? malformedRefResponse(request)
      : readReceiptResponse(dependencies, request, env, match.ref)
  }

  return { routePublicNip90MarketReceiptRequest }
}
