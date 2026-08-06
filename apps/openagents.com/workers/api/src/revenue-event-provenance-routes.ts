import { notFound } from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
} from './http/responses'
import { pathRefFromPrefix } from './http/router'
import {
  FIRST_DOLLAR_EVIDENCE_PUBLIC_ENDPOINT,
  readFirstDollarEvidenceBundle,
} from './revenue-event-provenance'

type HttpResponse = globalThis.Response

const BUNDLE_PATH_PREFIX = `${FIRST_DOLLAR_EVIDENCE_PUBLIC_ENDPOINT}/`

/**
 * A ref whose percent-escapes are malformed (PRO-1215). Answered exactly like a
 * well-formed unknown ref — 404 on GET, 405 on anything else — because the two
 * are indistinguishable to a caller. Previously the unguarded decode threw a
 * `URIError` defect out of the route matcher, which surfaced as 500 plus a
 * `severity: 'critical'` backend incident. Short-circuiting here also keeps an
 * attacker-controlled malformed ref from reaching the database at all.
 */
const malformedRefResponse = (request: Request): Effect.Effect<HttpResponse> =>
  Effect.succeed(
    request.method === 'GET' ? notFound() : methodNotAllowed(['GET']),
  )

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
    const match = pathRefFromPrefix(
      new URL(request.url).pathname,
      BUNDLE_PATH_PREFIX,
    )

    if (match._tag === 'no_match') {
      return undefined
    }

    if (match._tag === 'malformed') {
      return malformedRefResponse(request)
    }

    const bundleRef = match.ref

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
