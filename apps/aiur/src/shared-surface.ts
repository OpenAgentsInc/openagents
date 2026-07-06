/**
 * Runtime-portable Aiur surface shared by BOTH deploy targets:
 *
 * - the Cloudflare Worker entry (`src/server.ts`, TanStack Start SSR), and
 * - the Cloud Run Bun entry (`src/cloudrun/server.ts`, static shell + proxy).
 *
 * Nothing in here may import `@tanstack/react-start/server-entry` (a
 * Vite-build-only virtual module) or any Workers-runtime-specific API — the
 * Bun server imports this file directly (bundled with `bun build`), so it
 * must stay pure fetch/Request/Response code.
 */

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

/**
 * Routes everything that is NOT a normal app-shell page render: OpenAuth
 * sign-in/callback/logout, the `/api/aiur/access` UI-status endpoint, the
 * owner-gated Khala Sync proxy, and the owner-gated admin credits proxy
 * (AIUR-2, #8500). Returns `undefined` to fall through to the app shell
 * (which itself re-checks access before rendering anything sensitive).
 *
 * Every data-bearing route below re-resolves the owner gate itself and
 * FAILS CLOSED (empty/missing `AIUR_OWNER_USER_IDS` denies everyone).
 */
export async function routeAiurSharedSurfaceRequest(
  request: Request,
  env: AiurEnv,
): Promise<Response | undefined> {
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
