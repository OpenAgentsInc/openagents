import { notFound } from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
} from './http/responses'
import {
  type InferenceReceiptStore,
  publicInferenceReceiptFromRecord,
} from './inference-receipts'

type HttpResponse = globalThis.Response

export type PublicInferenceReceiptRouteDependencies<Bindings> = Readonly<{
  makeStore: (env: Bindings) => InferenceReceiptStore
  nowIso: () => string
}>

const receiptRefFromPath = (pathname: string, prefix: string): string | null =>
  pathname.startsWith(prefix) && pathname.length > prefix.length
    ? decodeURIComponent(pathname.slice(prefix.length))
    : null

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
    const receiptRef = receiptRefFromPath(
      new URL(request.url).pathname,
      '/api/public/inference/receipts/',
    )

    return receiptRef === null
      ? undefined
      : readReceiptResponse(dependencies, request, env, receiptRef)
  }

  return { routePublicInferenceReceiptRequest }
}
