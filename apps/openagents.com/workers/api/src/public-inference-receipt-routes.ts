import { notFound } from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
} from './http/responses'
import { pathRefFromPrefix } from './http/router'
import {
  type InferenceReceiptReadStore,
  publicInferenceReceiptFromRecord,
} from './inference-receipts'

type HttpResponse = globalThis.Response

export type PublicInferenceReceiptRouteDependencies<Bindings> = Readonly<{
  makeStore: (env: Bindings) => InferenceReceiptReadStore
  nowIso: () => string
}>

const RECEIPT_PATH_PREFIX = '/api/public/inference/receipts/'

/**
 * A ref whose percent-escapes are malformed (PRO-101). Answered exactly like a
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
  dependencies: PublicInferenceReceiptRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  receiptRef: string,
): Effect.Effect<HttpResponse> =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.tryPromise({
        catch: () => 'inference_receipt_read_failed' as const,
        try: () =>
          dependencies.makeStore(env).readInferenceReceiptByRef(receiptRef),
      }).pipe(
        Effect.map(record => {
          const receipt =
            record === null
              ? null
              : publicInferenceReceiptFromRecord(record, dependencies.nowIso())

          return receipt === null
            ? notFound()
            : noStoreJsonResponse({ receipt })
        }),
        Effect.catch(() => Effect.succeed(serverError())),
      )

export const makePublicInferenceReceiptRoutes = <Bindings>(
  dependencies: PublicInferenceReceiptRouteDependencies<Bindings>,
) => {
  const routePublicInferenceReceiptRequest = (
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

  return { routePublicInferenceReceiptRequest }
}
