import { Effect } from 'effect'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { pathRefFromPrefix } from './http/router'
import {
  type MarketingAgencySelfServeClaimStore,
  projectMarketingAgencySelfServeClaims,
} from './marketing-agency-self-serve-claim-upgrade'
import { selfServeDeliverabilityFixture } from './marketing-agency-self-serve-fixture'
import { liveAtReadStaleness } from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

/**
 * The 404 a caller gets for a workspace ref this reader does not know. A ref
 * whose percent-escapes are malformed (PRO-1215) is answered with exactly this,
 * after the method check, because the two cases are indistinguishable to a
 * caller and 404 declines to leak that the ref was parsed at all. Previously the
 * unguarded decode threw a `URIError` defect out of the route matcher, which
 * surfaced as 500 plus a `severity: 'critical'` backend incident.
 */
const deliverabilityNotFoundResponse = (): Effect.Effect<HttpResponse> =>
  Effect.succeed(
    noStoreJsonResponse(
      { error: 'not_found', reason: 'Deliverability record not found.' },
      { status: 404 },
    ),
  )

export type MarketingAgencySelfServeRoutesDependencies<Bindings> = Readonly<{
  makeClaimStore: (env: Bindings) => MarketingAgencySelfServeClaimStore
}>

export const makeMarketingAgencySelfServePublicRoutes = <Bindings>(
  dependencies: MarketingAgencySelfServeRoutesDependencies<Bindings>,
) => {
  const routeMarketingAgencySelfServeRequest = (
    request: Request,
    env: Bindings
  ): Effect.Effect<HttpResponse> | undefined => {

    const url = new URL(request.url)

    if (
      url.pathname === '/api/public/marketing-agency/self-serve/deliverability' &&
      url.searchParams.get('view') === 'self-serve-claims'
    ) {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }
      return Effect.tryPromise({
        catch: () => 'claim_store_failed' as const,
        try: async () => dependencies.makeClaimStore(env).list(),
      }).pipe(
        Effect.map(claims =>
          noStoreJsonResponse(projectMarketingAgencySelfServeClaims(claims))
        ),
        Effect.catch(() => Effect.succeed(noStoreJsonResponse({ error: 'server_error' }, { status: 500 }))),
      )
    }

    const match = pathRefFromPrefix(
      url.pathname,
      '/api/public/marketing-agency/self-serve/deliverability/',
    )

    if (match._tag === 'no_match') {
      return undefined
    }

    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    if (match._tag === 'malformed') {
      return deliverabilityNotFoundResponse()
    }

    const workspaceRef = match.ref

    // Expose the mocked self-serve deliverability fixture for the blocker.
    if (workspaceRef === selfServeDeliverabilityFixture.workspaceId) {
      return Effect.succeed(
        noStoreJsonResponse({
          generatedAt: currentIsoTimestamp(),
          staleness: liveAtReadStaleness(['fixture_only']),
          deliverability: selfServeDeliverabilityFixture,
        }),
      )
    }

    return deliverabilityNotFoundResponse()
  }

  return { routeMarketingAgencySelfServeRequest }
}
