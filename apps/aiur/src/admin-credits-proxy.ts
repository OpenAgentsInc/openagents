/**
 * Server-side proxy for the Aiur credits console (AIUR-2, #8500) and the
 * ops views (AIUR-3, #8501).
 *
 * The main `openagents.com` Worker owns the D1 credit ledger and exposes
 * owner-gated `/api/admin/credits/*` and `/api/admin/ops/*` routes there
 * (see `apps/openagents.com/workers/api/src/admin-credits-routes.ts` and
 * `admin-ops-routes.ts`). Aiur is a SEPARATE origin, so a browser `fetch`
 * from the Aiur UI straight to `openagents.com` would need CORS headers
 * that Worker deliberately does not grant to arbitrary origins (same
 * reasoning as `khala-sync-proxy.ts`).
 *
 * This proxy forwards owner-gated requests server-to-server, attaching the
 * signed-in owner's OWN OpenAuth access token as the `Authorization` bearer
 * — the exact same token that authenticated the caller's Aiur session,
 * never a shared static token. The main Worker independently re-verifies
 * that bearer and its own admin-email allowlist before touching any ledger
 * data (`requireAdminCreditsCaller` in its `index.ts`), so this proxy is a
 * pure forwarding layer, not a second trust boundary — the owner check here
 * (`resolveAiurAccess`) is defense-in-depth, never the only gate.
 */

import { resolveAiurAccess, type ResolveAiurAccessDeps } from './auth/access'
import { AIUR_ACCESS_COOKIE, parseCookies } from './auth/cookies'
import type { AiurEnv } from './auth/config'

export const AIUR_ADMIN_CREDITS_USERS_PATH = '/api/admin/credits/users'
export const AIUR_ADMIN_CREDITS_BALANCE_PATH = '/api/admin/credits/balance'
export const AIUR_ADMIN_CREDITS_HISTORY_PATH = '/api/admin/credits/history'
export const AIUR_ADMIN_CREDITS_RECENT_GRANTS_PATH =
  '/api/admin/credits/recent-grants'
export const AIUR_ADMIN_CREDITS_GRANT_PATH = '/api/admin/credits/grant'
export const AIUR_ADMIN_CREDITS_CLAWBACK_PATH = '/api/admin/credits/clawback'

// AIUR-3 (#8501): the ops views' read-only routes reuse this SAME generic
// owner-gated forwarding proxy — same auth boundary, same upstream, just a
// different path prefix on the main Worker.
export const AIUR_ADMIN_OPS_RUNS_PATH = '/api/admin/ops/runs'
export const AIUR_ADMIN_OPS_HEALTH_PATH = '/api/admin/ops/health'

// OB-6 (P1 Track C, #8563): the daily sales ledger route — same proxy, same
// auth boundary (see business-outreach-daily-ledger-routes.ts).
export const AIUR_ADMIN_OPS_DAILY_SALES_LEDGER_PATH =
  '/api/admin/ops/daily-sales-ledger'

const PROXIED_PATHS: ReadonlySet<string> = new Set([
  AIUR_ADMIN_CREDITS_USERS_PATH,
  AIUR_ADMIN_CREDITS_BALANCE_PATH,
  AIUR_ADMIN_CREDITS_HISTORY_PATH,
  AIUR_ADMIN_CREDITS_RECENT_GRANTS_PATH,
  AIUR_ADMIN_CREDITS_GRANT_PATH,
  AIUR_ADMIN_CREDITS_CLAWBACK_PATH,
  AIUR_ADMIN_OPS_RUNS_PATH,
  AIUR_ADMIN_OPS_HEALTH_PATH,
  AIUR_ADMIN_OPS_DAILY_SALES_LEDGER_PATH,
])

const noStoreJson = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      'content-type': 'application/json; charset=utf-8',
      'cache-control': 'no-store',
    },
  })

const unauthenticated = (): Response =>
  noStoreJson(
    {
      code: 'unauthenticated',
      messageSafe: 'Sign in as the Aiur owner before using the credits console.',
    },
    401,
  )

export type AdminCreditsProxyFetch = (
  input: string,
  init?: RequestInit,
) => Promise<Response>

export type AiurAdminCreditsProxyDeps = ResolveAiurAccessDeps &
  Readonly<{
    fetch?: AdminCreditsProxyFetch
    upstreamBaseUrl?: string
  }>

const upstreamUrl = (
  env: AiurEnv,
  deps: AiurAdminCreditsProxyDeps,
  path: string,
): string => {
  const base = (
    deps.upstreamBaseUrl ??
    env.KHALA_SYNC_UPSTREAM_BASE_URL ??
    'https://openagents.com'
  ).replace(/\/$/, '')

  return `${base}${path}`
}

const requireOwnerBearer = async (
  request: Request,
  env: AiurEnv,
  deps: AiurAdminCreditsProxyDeps,
): Promise<string | undefined> => {
  const access = await resolveAiurAccess(request, env, deps)
  if (access.kind !== 'owner') return undefined

  return parseCookies(request).get(AIUR_ACCESS_COOKIE)
}

/**
 * Routes one `/api/admin/credits/*` request, or returns `undefined` for
 * anything else so the caller falls through to the normal app router. Pure
 * w.r.t. its `deps` — tests inject a fake owner-access client and fake
 * `fetch`, no real network required.
 */
export const routeAiurAdminCreditsProxyRequest = (
  request: Request,
  env: AiurEnv,
  deps: AiurAdminCreditsProxyDeps = {},
): Promise<Response> | undefined => {
  const path = new URL(request.url).pathname
  if (!PROXIED_PATHS.has(path)) return undefined

  return (async (): Promise<Response> => {
    if (request.method !== 'GET' && request.method !== 'POST') {
      return noStoreJson(
        { code: 'invalid_request', messageSafe: 'GET or POST required' },
        405,
      )
    }

    const bearer = await requireOwnerBearer(request, env, deps)
    if (bearer === undefined) return unauthenticated()

    const fetchImpl = deps.fetch ?? globalThis.fetch.bind(globalThis)
    const requestUrl = new URL(request.url)
    const target = new URL(upstreamUrl(env, deps, path))
    target.search = requestUrl.search

    const upstream = await fetchImpl(target.toString(), {
      method: request.method,
      headers: {
        authorization: `Bearer ${bearer}`,
        ...(request.method === 'POST'
          ? { 'content-type': 'application/json' }
          : {}),
      },
      ...(request.method === 'POST' ? { body: await request.text() } : {}),
    })
    const responseText = await upstream.text()

    return new Response(responseText, {
      status: upstream.status,
      headers: {
        'content-type': 'application/json; charset=utf-8',
        'cache-control': 'no-store',
      },
    })
  })()
}
