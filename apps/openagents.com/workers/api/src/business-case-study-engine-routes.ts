import { Effect } from 'effect'

import {
  BusinessCaseStudyEndpoint,
  BusinessCaseStudyStaleness,
  type BusinessCaseStudyStore,
  projectBusinessCaseStudies,
  publicBusinessCaseStudyProjection,
} from './business-case-study-engine'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { pathRefFromPrefix } from './http/router'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

export type BusinessCaseStudyRoutesDependencies<Bindings> = Readonly<{
  makeCaseStudyStore: (env: Bindings) => BusinessCaseStudyStore
}>

/**
 * The 404 a caller gets for a ref this reader does not publish. A ref whose
 * percent-escapes are malformed (PRO-1215) is answered with exactly this, after
 * the method check, because the two cases are indistinguishable to a caller and
 * 404 declines to leak that the ref was parsed at all. Previously the unguarded
 * decode threw a `URIError` defect out of the route matcher, which surfaced as
 * 500 plus a `severity: 'critical'` backend incident.
 */
const caseStudyNotFoundResponse = (): Effect.Effect<HttpResponse> =>
  Effect.succeed(
    noStoreJsonResponse(
      { error: 'not_found', reason: 'Case study not found.' },
      { status: 404 },
    ),
  )

export const makeBusinessCaseStudyRoutes = <Bindings>(
  dependencies: BusinessCaseStudyRoutesDependencies<Bindings>,
) => ({
  routeBusinessCaseStudyRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (
      url.pathname === BusinessCaseStudyEndpoint &&
      url.searchParams.get('view') === 'published-case-studies'
    ) {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }
      return Effect.succeed(
        noStoreJsonResponse(
          projectBusinessCaseStudies(
            dependencies.makeCaseStudyStore(env).list(),
          ),
        ),
      )
    }

    const match = pathRefFromPrefix(
      url.pathname,
      `${BusinessCaseStudyEndpoint}/`,
    )

    if (match._tag === 'no_match') {
      return undefined
    }

    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    if (match._tag === 'malformed') {
      return caseStudyNotFoundResponse()
    }

    const caseStudyRef = match.ref

    const caseStudy = dependencies
      .makeCaseStudyStore(env)
      .list()
      .find(input => input.caseStudyRef === caseStudyRef)

    if (caseStudy === undefined || caseStudy.status !== 'published') {
      return caseStudyNotFoundResponse()
    }

    return Effect.succeed(
      noStoreJsonResponse({
        generatedAt: currentIsoTimestamp(),
        staleness: BusinessCaseStudyStaleness,
        maxStalenessSeconds: BusinessCaseStudyStaleness.maxStalenessSeconds,
        caseStudy: publicBusinessCaseStudyProjection(caseStudy),
        authorityBoundary:
          'This public-safe case-study read exposes only opaque engagement refs, receipt refs, cycle-time metrics, and attribution hooks. It grants no customer identity, payout, settlement, self-serve, or green-claim authority.',
      }),
    )
  },
})
