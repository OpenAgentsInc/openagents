
import { Effect } from 'effect'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
} from './http/responses'
import type { EcommerceCampaignReceiptStore } from './ecommerce-campaign-receipt-store'
import { notFound } from '@openagentsinc/sync-worker'

type HttpResponse = globalThis.Response

export type EcommerceCampaignReceiptRoutesDependencies<Bindings> = Readonly<{
  makeStore: (env: Bindings) => EcommerceCampaignReceiptStore
}>

const receiptRefFromPath = (
  pathname: string,
  prefix: string,
): string | null =>
  pathname.startsWith(prefix) && pathname.length > prefix.length
    ? decodeURIComponent(pathname.slice(prefix.length))
    : null

const readReceiptResponse = <Bindings>(
  dependencies: EcommerceCampaignReceiptRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  receiptRef: string,
): Effect.Effect<HttpResponse> =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.tryPromise({
        catch: () => 'receipt_read_failed' as const,
        try: () => dependencies.makeStore(env).get(receiptRef),
      }).pipe(
        Effect.map(sealed =>
          sealed === undefined
            ? notFound()
            : noStoreJsonResponse(sealed.document)
        ),
        Effect.catch(() => Effect.succeed(serverError())),
      )

export const makeEcommerceCampaignReceiptRoutes = <Bindings>(
  dependencies: EcommerceCampaignReceiptRoutesDependencies<Bindings>,
) => ({
  routeEcommerceCampaignReceiptRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    const receiptRef = receiptRefFromPath(
      url.pathname,
      '/api/public/ecommerce-campaign/receipts/',
    )

    if (receiptRef !== null) {
      return readReceiptResponse(dependencies, request, env, receiptRef)
    }

    return undefined
  },
})
