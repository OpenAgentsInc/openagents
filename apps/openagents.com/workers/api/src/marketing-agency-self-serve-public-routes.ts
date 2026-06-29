import { Effect } from 'effect'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  type MarketingAgencySelfServeClaimStore,
  projectMarketingAgencySelfServeClaims,
} from './marketing-agency-self-serve-claim-upgrade'
import { selfServeDeliverabilityFixture } from './marketing-agency-self-serve-fixture'
import { liveAtReadStaleness } from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

const workspaceRefFromPath = (
  pathname: string,
  prefix: string,
): string | null =>
  pathname.startsWith(prefix) && pathname.length > prefix.length
    ? decodeURIComponent(pathname.slice(prefix.length))
    : null

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

    const workspaceRef = workspaceRefFromPath(
      url.pathname,
      '/api/public/marketing-agency/self-serve/deliverability/',
    )

    if (workspaceRef === null) {
      return undefined
    }

    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

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

    return Effect.succeed(
      noStoreJsonResponse(
        { error: 'not_found', reason: 'Deliverability record not found.' },
        { status: 404 },
      ),
    )
  }

  return { routeMarketingAgencySelfServeRequest }
}
