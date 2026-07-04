import { notFound } from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
} from './http/responses'
import {
  FIRST_DOLLAR_EVIDENCE_PUBLIC_ENDPOINT,
  readFirstDollarEvidenceBundle,
} from './revenue-event-provenance'

type HttpResponse = globalThis.Response

const bundleRefFromPath = (pathname: string): string | null => {
  const prefix = `${FIRST_DOLLAR_EVIDENCE_PUBLIC_ENDPOINT}/`
  return pathname.startsWith(prefix) && pathname.length > prefix.length
    ? decodeURIComponent(pathname.slice(prefix.length))
    : null
}

export type PublicFirstDollarEvidenceRouteDependencies<Bindings> = Readonly<{
  makeDb: (env: Bindings) => D1Database
  nowIso: () => string
}>

export const makePublicFirstDollarEvidenceRoutes = <Bindings>(
  dependencies: PublicFirstDollarEvidenceRouteDependencies<Bindings>,
) => ({
  routePublicFirstDollarEvidenceRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const bundleRef = bundleRefFromPath(new URL(request.url).pathname)
    if (bundleRef === null) {
      return undefined
    }

    return request.method !== 'GET'
      ? Effect.succeed(methodNotAllowed(['GET']))
      : Effect.tryPromise({
          try: () =>
            readFirstDollarEvidenceBundle(
              dependencies.makeDb(env),
              bundleRef,
              dependencies.nowIso(),
            ),
          catch: () => 'first_dollar_evidence_read_failed' as const,
        }).pipe(
          Effect.map(bundle =>
            bundle === null
              ? notFound()
              : noStoreJsonResponse({
                  generatedAt: bundle.generatedAt,
                  staleness: bundle.staleness,
                  bundle,
                }),
          ),
          Effect.catch(() => Effect.succeed(serverError())),
        )
  },
})
