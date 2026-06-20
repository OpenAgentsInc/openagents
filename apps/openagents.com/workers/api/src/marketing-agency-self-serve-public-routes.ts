import { Effect } from 'effect'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
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

export const makeMarketingAgencySelfServePublicRoutes = () => {
  const routeMarketingAgencySelfServeRequest = (
    request: Request,
  ): Effect.Effect<HttpResponse> | undefined => {
    const workspaceRef = workspaceRefFromPath(
      new URL(request.url).pathname,
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
