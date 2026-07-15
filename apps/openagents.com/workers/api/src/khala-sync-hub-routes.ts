import { SyncScope } from '@openagentsinc/khala-sync'
import { Schema as S } from 'effect'

type HttpResponse = globalThis.Response

export const KHALA_SYNC_HUB_APPEND_PATH = "/api/internal/khala-sync/hub/append"
export const KHALA_SYNC_HUB_LOG_PATH = "/api/internal/khala-sync/hub/log"
export const KHALA_SYNC_HUB_CONNECT_PATH = "/api/internal/khala-sync/hub/connect"
export const KHALA_SYNC_HUB_ACCESS_CHANGED_PATH = "/api/internal/khala-sync/hub/access-changed"
export const KHALA_SYNC_HUB_ROUTE_REF = "route.internal.khala_sync.hub.v0_1"

const decodeScope = S.decodeUnknownSync(SyncScope)
const json = (value: unknown, init: ResponseInit = {}): HttpResponse => {
  const headers = new Headers(init.headers)
  headers.set("cache-control", "no-store")
  return Response.json(value, { ...init, headers })
}
const hubMethodNotAllowed = (allowedMethods: ReadonlyArray<string>): HttpResponse =>
  json({ error: "method_not_allowed" }, {
    status: 405,
    headers: { allow: allowedMethods.join(", ") },
  })

export type KhalaSyncHubStubLike = Readonly<{
  fetch: (request: Request) => Promise<HttpResponse>
}>

export type KhalaSyncHubNamespaceLike = Readonly<{
  idFromName: (name: string) => unknown
  get: (id: unknown) => KhalaSyncHubStubLike
}>

export type KhalaSyncHubInternalRouteDependencies = Readonly<{
  /** Same admin bearer predicate as the KS-0.2 db-smoke route. */
  requireOperator: () => Promise<boolean>
  /** LiveHub service adapter. */
  namespace: KhalaSyncHubNamespaceLike | undefined
  hubPath: '/append' | '/connect' | '/log'
}>

export const handleKhalaSyncHubInternalRoute = async (
  request: Request,
  deps: KhalaSyncHubInternalRouteDependencies,
): Promise<HttpResponse> => {
  const expectedMethod = deps.hubPath === '/append' ? 'POST' : 'GET'
  if (request.method !== expectedMethod) {
    return hubMethodNotAllowed([expectedMethod])
  }

  if (!(await deps.requireOperator())) {
    return json({ error: 'unauthorized' }, { status: 401 })
  }

  const url = new URL(request.url)
  const scopeRaw = url.searchParams.get('scope')
  try {
    decodeScope(scopeRaw)
  } catch {
    return json(
      {
        error: 'khala_sync_hub_invalid_scope',
        reason: 'scope query parameter must be a valid Khala Sync scope id',
        routeRef: KHALA_SYNC_HUB_ROUTE_REF,
      },
      { status: 400 },
    )
  }

  if (deps.namespace === undefined) {
    return json(
      {
        error: 'khala_sync_live_hub_unconfigured',
        reason: 'The Google Cloud LiveHub service is not configured.',
        routeRef: KHALA_SYNC_HUB_ROUTE_REF,
      },
      { status: 503 },
    )
  }

  const stub = deps.namespace.get(
    deps.namespace.idFromName(scopeRaw as string),
  )
  const target = new URL(
    `https://khala-sync-hub.openagents.internal${deps.hubPath}`,
  )
  url.searchParams.forEach((value, key) => {
    target.searchParams.set(key, value)
  })
  // `new Request(url, request)` preserves method, headers (including the
  // WebSocket Upgrade header for /connect), and body.
  return stub.fetch(new Request(target.toString(), request))
}

// ---------------------------------------------------------------------------
// Access-change refetch trigger (KS-7.1, #8305 — SPEC §7 invariant 7)
// ---------------------------------------------------------------------------

/**
 * FAIL-SOFT hub notification for scope-access revocation: tell the scope's
 * LiveHub scope to broadcast `MustRefetch(access_changed)` and close every
 * connected socket. Never throws — a hub/network failure must not fail the
 * revocation write it accompanies (correctness does not depend on this
 * push: the KS-7.1 resolver re-reads live membership on every
 * log/bootstrap/connect, so a revoked user is denied at their next request
 * regardless; this broadcast just retracts ALREADY-OPEN live tails
 * promptly).
 *
 * CALL-SITE CONTRACT: every future API write path that revokes scope
 * access — team-membership removal/deactivation, scope-owner deletion —
 * MUST call this after its commit. TODAY no such write path exists in this
 * Worker (memberships are only created/reactivated by invite acceptance;
 * they are removed by operator D1 edits, and nothing deletes
 * khala_sync_scope_owners rows), so the operator-facing trigger is the
 * admin-bearer `POST /api/internal/khala-sync/hub/access-changed` route
 * (`handleKhalaSyncHubAccessChangedRoute`) — run it after any manual
 * membership revocation (see docs/khala-sync/RUNBOOK.md).
 */
export const notifyKhalaSyncHubAccessChangedBestEffort = async (
  namespace: KhalaSyncHubNamespaceLike | undefined,
  scope: string,
): Promise<
  | { readonly ok: true; readonly notified: number }
  | { readonly ok: false; readonly reason: string }
> => {
  try {
    decodeScope(scope)
  } catch {
    return { ok: false, reason: 'invalid_scope' }
  }
  if (namespace === undefined) {
    return { ok: false, reason: 'hub_binding_missing' }
  }
  try {
    const stub = namespace.get(namespace.idFromName(scope))
    const target = new URL(
      'https://khala-sync-hub.openagents.internal/access-changed',
    )
    target.searchParams.set('scope', scope)
    const response = await stub.fetch(
      new Request(target.toString(), { method: 'POST' }),
    )
    if (response.status !== 200) {
      return { ok: false, reason: `hub_status_${response.status}` }
    }
    const body = (await response.json().catch(() => undefined)) as
      | { notified?: unknown }
      | undefined
    return {
      notified: typeof body?.notified === 'number' ? body.notified : 0,
      ok: true,
    }
  } catch {
    return { ok: false, reason: 'hub_unreachable' }
  }
}

export type KhalaSyncHubAccessChangedRouteDependencies = Readonly<{
  /** Same admin bearer predicate as the other internal hub routes. */
  requireOperator: () => Promise<boolean>
  /** LiveHub service adapter. */
  namespace: KhalaSyncHubNamespaceLike | undefined
}>

/**
 * `POST /api/internal/khala-sync/hub/access-changed` `{ scope }` —
 * admin-bearer internal route (same guard as the hub append/log/connect
 * proxies): instructs the scope's LiveHub to broadcast
 * `MustRefetch(access_changed)` and close all sockets. This is the KS-7.1
 * revocation trigger for operator-driven membership changes; API write
 * paths call `notifyKhalaSyncHubAccessChangedBestEffort` directly.
 */
export const handleKhalaSyncHubAccessChangedRoute = async (
  request: Request,
  deps: KhalaSyncHubAccessChangedRouteDependencies,
): Promise<HttpResponse> => {
  if (request.method !== 'POST') {
    return hubMethodNotAllowed(['POST'])
  }
  if (!(await deps.requireOperator())) {
    return json({ error: 'unauthorized' }, { status: 401 })
  }

  const body = (await request.json().catch(() => undefined)) as
    | Record<string, unknown>
    | undefined
  const scopeRaw = typeof body?.scope === 'string' ? body.scope : null
  try {
    decodeScope(scopeRaw)
  } catch {
    return json(
      {
        error: 'khala_sync_hub_invalid_scope',
        reason: 'request body must carry a valid Khala Sync scope id in `scope`',
        routeRef: KHALA_SYNC_HUB_ROUTE_REF,
      },
      { status: 400 },
    )
  }

  if (deps.namespace === undefined) {
    return json(
      {
        error: 'khala_sync_live_hub_unconfigured',
        reason: 'The Google Cloud LiveHub service is not configured.',
        routeRef: KHALA_SYNC_HUB_ROUTE_REF,
      },
      { status: 503 },
    )
  }

  const stub = deps.namespace.get(
    deps.namespace.idFromName(scopeRaw as string),
  )
  const target = new URL(
    'https://khala-sync-hub.openagents.internal/access-changed',
  )
  target.searchParams.set('scope', scopeRaw as string)
  return stub.fetch(new Request(target.toString(), { method: 'POST' }))
}
