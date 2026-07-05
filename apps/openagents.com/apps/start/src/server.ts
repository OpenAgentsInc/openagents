import { withStartRequestContext } from '@openagentsinc/effect-start'
import handler, { createServerEntry } from '@tanstack/react-start/server-entry'
import { Effect } from 'effect'

import {
  DISCOVERY_SURFACE_PATHS,
  type DiscoverySurfacePath,
  renderDiscoverySurface,
} from '../../../workers/api/src/inference/discovery-surfaces'
import { routeSiteCrawlSurfaceRequest } from '../../../workers/api/src/site-crawl-surfaces-routes'
import { routeWellKnownAgentSurfaceRequest } from '../../../workers/api/src/well-known-agent-surfaces-routes'
import { routeKhalaSyncProxyRequest } from './khala-sync-proxy'

type StartWorkerEnv = Record<string, unknown>

type StartExecutionContext = Readonly<{
  waitUntil(promise: Promise<unknown>): void
}>

export const SECURITY_HEADERS = {
  'X-Frame-Options': 'DENY',
  'X-Content-Type-Options': 'nosniff',
  'X-XSS-Protection': '1; mode=block',
  'Referrer-Policy': 'strict-origin-when-cross-origin',
  'Permissions-Policy': 'camera=(), microphone=(), geolocation=()',
  'Strict-Transport-Security': 'max-age=31536000; includeSubDomains',
} as const

export function applySecurityHeaders(response: Response): Response {
  const headers = new Headers(response.headers)

  for (const [key, value] of Object.entries(SECURITY_HEADERS)) {
    headers.set(key, value)
  }

  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  })
}

export async function routeSharedAgentSurface(
  request: Request,
): Promise<Response | undefined> {
  const path = new URL(request.url).pathname
  const crawlSurface = routeSiteCrawlSurfaceRequest(request)
  if (crawlSurface !== undefined) {
    return Effect.runPromise(crawlSurface)
  }

  const wellKnownSurface = routeWellKnownAgentSurfaceRequest(request)
  if (wellKnownSurface !== undefined) {
    return Effect.runPromise(wellKnownSurface)
  }

  if ((DISCOVERY_SURFACE_PATHS as ReadonlyArray<string>).includes(path)) {
    return Effect.runPromise(
      renderDiscoverySurface(request, path as DiscoverySurfacePath),
    )
  }

  return undefined
}

const server = createServerEntry({
  async fetch(request) {
    const sharedSurfaceResponse = await routeSharedAgentSurface(request)
    if (sharedSurfaceResponse !== undefined) {
      return applySecurityHeaders(sharedSurfaceResponse)
    }

    // Khala Sync proxy (#8413): same-origin bootstrap/push/connect bridge to
    // the production Khala Sync API. A successful WebSocket upgrade (status
    // 101) carries a Workers-runtime `webSocket` pairing that a
    // `new Response(response.body, ...)` reconstruction (what
    // `applySecurityHeaders` does) would silently drop, so it is returned
    // directly rather than wrapped.
    const khalaSyncProxyResponse = await routeKhalaSyncProxyRequest(request)
    if (khalaSyncProxyResponse !== undefined) {
      return khalaSyncProxyResponse.status === 101
        ? khalaSyncProxyResponse
        : applySecurityHeaders(khalaSyncProxyResponse)
    }

    const response = await handler.fetch(request, {
      responseLinkHeader: {
        filter: ({ phase }: { phase: 'static' | 'dynamic' }) =>
          phase === 'static',
      },
    })

    return applySecurityHeaders(response)
  },
})

export default {
  fetch(
    request: Request,
    env: StartWorkerEnv,
    executionCtx: StartExecutionContext,
  ) {
    return withStartRequestContext({ request, env, executionCtx }, () =>
      server.fetch(request),
    )
  },
}
