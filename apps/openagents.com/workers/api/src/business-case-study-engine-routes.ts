import { Effect } from 'effect'

import {
  BusinessCaseStudyEndpoint,
  BusinessCaseStudyStaleness,
  type BusinessCaseStudyStore,
  projectBusinessCaseStudies,
  publicBusinessCaseStudyProjection,
} from './business-case-study-engine'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

export type BusinessCaseStudyRoutesDependencies<Bindings> = Readonly<{
  makeCaseStudyStore: (env: Bindings) => BusinessCaseStudyStore
}>

const caseStudyRefFromPath = (
  pathname: string,
  prefix: string,
): string | null =>
  pathname.startsWith(prefix) && pathname.length > prefix.length
    ? decodeURIComponent(pathname.slice(prefix.length))
    : null

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

    const caseStudyRef = caseStudyRefFromPath(
      url.pathname,
      `${BusinessCaseStudyEndpoint}/`,
    )

    if (caseStudyRef === null) {
      return undefined
    }

    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    const caseStudy = dependencies
      .makeCaseStudyStore(env)
      .list()
      .find(input => input.caseStudyRef === caseStudyRef)

    if (caseStudy === undefined || caseStudy.status !== 'published') {
      return Effect.succeed(
        noStoreJsonResponse(
          { error: 'not_found', reason: 'Case study not found.' },
          { status: 404 },
        ),
      )
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
