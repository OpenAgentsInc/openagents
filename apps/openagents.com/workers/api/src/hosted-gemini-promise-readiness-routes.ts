import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse, serverError } from './http/responses'
import {
  type InferenceReceiptReadStore,
  publicInferenceReceiptFromRecord,
} from './inference-receipts'
import {
  buildHostedGeminiPromiseReadiness,
  HostedGeminiPromiseReadinessRoute,
} from './hosted-gemini-promise-readiness'
import type { PromiseTransitionReceiptStore } from './promise-transition-receipt-routes'

type HttpResponse = globalThis.Response

export type HostedGeminiPromiseReadinessDependencies<Bindings> = Readonly<{
  makeInferenceReceiptStore: (env: Bindings) => InferenceReceiptReadStore
  makeTransitionReceiptStore: (env: Bindings) => PromiseTransitionReceiptStore
  nowIso: () => string
}>

const readReceiptRef = (request: Request): string | null => {
  const value = new URL(request.url).searchParams.get('receiptRef')?.trim()
  return value === undefined || value === '' ? null : value
}

export const makeHostedGeminiPromiseReadinessRoutes = <Bindings>(
  dependencies: HostedGeminiPromiseReadinessDependencies<Bindings>,
) => {
  const routeHostedGeminiPromiseReadinessRequest = (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    if (url.pathname !== HostedGeminiPromiseReadinessRoute) {
      return undefined
    }

    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    const receiptRef = readReceiptRef(request)
    if (receiptRef === null) {
      return Effect.succeed(
        noStoreJsonResponse(
          {
            error: 'hosted_gemini_receipt_ref_required',
            receiptRefQueryParam: 'receiptRef',
          },
          { status: 400 },
        ),
      )
    }

    return Effect.tryPromise({
      catch: () => 'hosted_gemini_readiness_failed' as const,
      try: async () => {
        const generatedAt = dependencies.nowIso()
        const [record, transitionReceipts] = await Promise.all([
          dependencies
            .makeInferenceReceiptStore(env)
            .readInferenceReceiptByRef(receiptRef),
          dependencies.makeTransitionReceiptStore(env).listReceipts(200),
        ])
        const receipt =
          record === null
            ? null
            : publicInferenceReceiptFromRecord(record, generatedAt)

        return buildHostedGeminiPromiseReadiness({
          generatedAt,
          receipt,
          receiptRef,
          transitionReceipts,
        })
      },
    }).pipe(
      Effect.map(readiness =>
        noStoreJsonResponse({
          generatedAt: readiness.generatedAt,
          maxStalenessSeconds: readiness.staleness.maxStalenessSeconds,
          readiness,
          staleness: readiness.staleness,
        }),
      ),
      Effect.catch(() => Effect.succeed(serverError())),
    )
  }

  return { routeHostedGeminiPromiseReadinessRequest }
}
