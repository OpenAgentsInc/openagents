/**
 * Owner-gated server-to-server proxy for Aiur's non-monetary operations
 * views. Billing, credit, checkout, and payment routes are intentionally not
 * part of this allowlist.
 */

import { resolveAiurAccess, type ResolveAiurAccessDeps } from './auth/access'
import { AIUR_ACCESS_COOKIE, parseCookies } from './auth/cookies'
import type { AiurEnv } from './auth/config'

export const AIUR_ADMIN_OPS_RUNS_PATH = '/api/admin/ops/runs'
export const AIUR_ADMIN_OPS_HEALTH_PATH = '/api/admin/ops/health'
export const AIUR_ADMIN_OPS_DAILY_SALES_LEDGER_PATH =
  '/api/admin/ops/daily-sales-ledger'
export const AIUR_ADMIN_OPS_CRM_BATCH_QUEUE_PATH =
  '/api/admin/ops/crm/batch-queue'
export const AIUR_ADMIN_OPS_CRM_BATCH_APPROVE_PATH =
  '/api/admin/ops/crm/batch-approve'

const PROXIED_PATHS: ReadonlySet<string> = new Set([
  AIUR_ADMIN_OPS_RUNS_PATH,
  AIUR_ADMIN_OPS_HEALTH_PATH,
  AIUR_ADMIN_OPS_DAILY_SALES_LEDGER_PATH,
  AIUR_ADMIN_OPS_CRM_BATCH_QUEUE_PATH,
  AIUR_ADMIN_OPS_CRM_BATCH_APPROVE_PATH,
])

const noStoreJson = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      'content-type': 'application/json; charset=utf-8',
      'cache-control': 'no-store',
    },
  })

export type AdminOpsProxyFetch = (
  input: string,
  init?: RequestInit,
) => Promise<Response>

export type AiurAdminOpsProxyDeps = ResolveAiurAccessDeps &
  Readonly<{
    fetch?: AdminOpsProxyFetch
    upstreamBaseUrl?: string
  }>

const requireOwnerBearer = async (
  request: Request,
  env: AiurEnv,
  deps: AiurAdminOpsProxyDeps,
): Promise<string | undefined> => {
  const access = await resolveAiurAccess(request, env, deps)
  if (access.kind !== 'owner') return undefined
  return parseCookies(request).get(AIUR_ACCESS_COOKIE)
}

export const routeAiurAdminOpsProxyRequest = (
  request: Request,
  env: AiurEnv,
  deps: AiurAdminOpsProxyDeps = {},
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
    if (bearer === undefined) {
      return noStoreJson(
        {
          code: 'unauthenticated',
          messageSafe: 'Sign in as the Aiur owner before using operations views.',
        },
        401,
      )
    }

    const fetchImpl = deps.fetch ?? globalThis.fetch.bind(globalThis)
    const source = new URL(request.url)
    const base = (
      deps.upstreamBaseUrl ??
      env.KHALA_SYNC_UPSTREAM_BASE_URL ??
      'https://openagents.com'
    ).replace(/\/$/, '')
    const target = new URL(`${base}${path}`)
    target.search = source.search
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

    return new Response(await upstream.text(), {
      status: upstream.status,
      headers: {
        'content-type': 'application/json; charset=utf-8',
        'cache-control': 'no-store',
      },
    })
  })()
}
