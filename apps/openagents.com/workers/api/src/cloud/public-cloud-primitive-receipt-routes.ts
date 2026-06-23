// Public GET route that dereferences a sellable-Cloud-primitive PAID charge
// receipt (EPIC #5510: sandbox compute #5517, fine-tuning #5516).
//
//   GET /api/public/cloud/receipts/:receiptRef
//
// Mirrors `public-inference-receipt-routes.ts`. It reads the settled `pay_ins`
// row for the ref and returns a public-safe receipt proving the metered debit
// landed (`state = 'paid'`), or 404 when there is no such PAID cloud-primitive
// charge. No auth (public proof read), no writes, no payment material — the
// projection's redaction guard refuses to publish anything sensitive.

import { notFound } from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
} from '../http/responses'
import {
  type CloudPrimitiveReceiptReadStore,
  publicCloudPrimitiveReceiptFromRecord,
} from './cloud-primitive-receipts'

type HttpResponse = globalThis.Response

export type PublicCloudPrimitiveReceiptRouteDependencies<Bindings> = Readonly<{
  makeStore: (env: Bindings) => CloudPrimitiveReceiptReadStore
  nowIso: () => string
}>

const RECEIPT_PATH_PREFIX = '/api/public/cloud/receipts/'

const receiptRefFromPath = (pathname: string): string | null =>
  pathname.startsWith(RECEIPT_PATH_PREFIX) &&
  pathname.length > RECEIPT_PATH_PREFIX.length
    ? decodeURIComponent(pathname.slice(RECEIPT_PATH_PREFIX.length))
    : null

const readReceiptResponse = <Bindings>(
  dependencies: PublicCloudPrimitiveReceiptRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  receiptRef: string,
): Effect.Effect<HttpResponse> =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.tryPromise({
        catch: () => 'cloud_primitive_receipt_read_failed' as const,
        try: () =>
          dependencies
            .makeStore(env)
            .readCloudPrimitiveReceiptByRef(receiptRef),
      }).pipe(
        Effect.map(record => {
          const receipt =
            record === null
              ? null
              : publicCloudPrimitiveReceiptFromRecord(
                  record,
                  dependencies.nowIso(),
                )

          return receipt === null
            ? notFound()
            : noStoreJsonResponse({ receipt })
        }),
        Effect.catch(() => Effect.succeed(serverError())),
      )

export const makePublicCloudPrimitiveReceiptRoutes = <Bindings>(
  dependencies: PublicCloudPrimitiveReceiptRouteDependencies<Bindings>,
) => {
  const routePublicCloudPrimitiveReceiptRequest = (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const receiptRef = receiptRefFromPath(new URL(request.url).pathname)

    return receiptRef === null
      ? undefined
      : readReceiptResponse(dependencies, request, env, receiptRef)
  }

  return { routePublicCloudPrimitiveReceiptRequest }
}
