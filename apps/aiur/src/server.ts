import { getStartRequestContext, withStartRequestContext } from '@openagentsinc/effect-start'
import handler, { createServerEntry } from '@tanstack/react-start/server-entry'

import { routeAiurAdminCreditsProxyRequest } from './admin-credits-proxy'
import { AIUR_ACCESS_PATH, handleAiurAccessRequest } from './auth/access-route'
import { type AiurEnv } from './auth/config'
import { routeAiurAuthRequest } from './auth/routes'
import { routeAiurKhalaSyncProxyRequest } from './khala-sync-proxy'

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

const currentEnv = (): AiurEnv =>
  getStartRequestContext<AiurEnv>()?.env ?? {}

/**
 * Routes everything that is NOT a normal TanStack Start page render:
 * OpenAuth sign-in/callback/logout, the `/api/aiur/access` UI-status
 * endpoint, the owner-gated Khala Sync proxy, and the owner-gated admin
 * credits proxy (AIUR-2, #8500). Returns `undefined` to fall through to
 * the TanStack Start handler (the app shell — see `routes/index.tsx`,
 * which itself re-checks access before rendering anything sensitive).
 */
export async function routeAiurSharedSurface(
  request: Request,
): Promise<Response | undefined> {
  const env = currentEnv()

  const authResponse = routeAiurAuthRequest(request, env)
  if (authResponse !== undefined) {
    return authResponse
  }

  if (new URL(request.url).pathname === AIUR_ACCESS_PATH) {
    return handleAiurAccessRequest(request, env)
  }

  const adminCreditsResponse = routeAiurAdminCreditsProxyRequest(request, env)
  if (adminCreditsResponse !== undefined) {
    return adminCreditsResponse
  }

  return routeAiurKhalaSyncProxyRequest(request, env)
}

const server = createServerEntry({
  async fetch(request) {
    const sharedSurfaceResponse = await routeAiurSharedSurface(request)
    if (sharedSurfaceResponse !== undefined) {
      // A successful WebSocket upgrade (status 101) carries a
      // Workers-runtime `webSocket` pairing that a
      // `new Response(response.body, ...)` reconstruction (what
      // `applySecurityHeaders` does) would silently drop.
      return sharedSurfaceResponse.status === 101
        ? sharedSurfaceResponse
        : applySecurityHeaders(sharedSurfaceResponse)
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

type StartWorkerEnv = Record<string, unknown>

type StartExecutionContext = Readonly<{
  waitUntil(promise: Promise<unknown>): void
}>

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
