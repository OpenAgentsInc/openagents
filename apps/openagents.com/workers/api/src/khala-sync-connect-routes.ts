// Khala Sync live-tail connect route (KS-4.4, #8297): GET /api/sync/connect.
//
// `WS /api/sync/connect?scope=…&cursor=…` (docs/khala-sync/SPEC.md §3): the
// authenticated live-tail channel. The route authenticates and scope-gates
// BEFORE the upgrade — via the Worker's standard actor auth (browser session
// or programmatic agent bearer; same closure as push/log/bootstrap) and the
// same KS-7.1 `resolveScopeRead` seam as GET /api/sync/log (full taxonomy,
// fail-closed; sockets re-run this gate on every reconnect, so a revoked
// user whose socket was closed by the hub's access_changed broadcast can
// never re-attach) — then PROXIES the
// WebSocket upgrade to the per-scope `KhalaSyncHubDO`
// (`env.KHALA_SYNC_HUB.idFromName(scope)`), the standard DO WebSocket-proxy
// pattern: forwarding `new Request(target, request)` preserves the method,
// the `Upgrade: websocket` header, and the client socket end, and the hub's
// own `/connect` handler performs the Hibernation-API accept + catch-up
// (DeltaFrames from the socket cursor out of the DO window, or MustRefetch
// when the cursor is behind the retained window — SPEC §5).
//
// The ADMIN-GUARDED internal route (`/api/internal/khala-sync/hub/connect`,
// KS-4.2) stays in place for capture/operator use only; THIS route is the
// public client surface.
//
// Pre-upgrade failures are typed `SyncError` bodies (same taxonomy as
// push/log/bootstrap), always no-store: 401 unauthenticated, 400
// invalid_request (bad scope/cursor), 403 unauthorized_scope, 426
// invalid_request when the Upgrade header is missing (connect is
// WebSocket-only), 503 storage_unavailable while the KHALA_SYNC_HUB binding
// is absent, 500 internal if the hub upgrade itself fails unexpectedly.

import { Effect, Schema as S } from 'effect'

import {
  SyncError,
  type SyncErrorCode,
  SyncScope,
} from '@openagentsinc/khala-sync'

import { scopeReadDecisionResponse } from './http/khala-sync-scope-read-response'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import type { KhalaSyncHubNamespaceLike } from './khala-sync-hub-do'
import type { KhalaSyncScopeReadResolver } from './khala-sync-scope-auth'

type HttpResponse = globalThis.Response

export const KHALA_SYNC_CONNECT_PATH = '/api/sync/connect'
export const KHALA_SYNC_CONNECT_ROUTE_REF = 'route.khala_sync.connect.v0_1'

const decodeScope = S.decodeUnknownSync(SyncScope)
const encodeSyncError = S.encodeSync(SyncError)

export type KhalaSyncConnectDependencies = Readonly<{
  /**
   * Resolve the authenticated caller via the Worker's standard actor auth
   * (`authenticateRequestActor`: browser session or agent bearer token).
   * `undefined` ⇒ 401. Runs BEFORE the upgrade is forwarded.
   */
  authenticate: () => Promise<{ readonly userId: string } | undefined>
  /**
   * Scope-read authorization (KS-7.1): the taxonomy-complete resolver
   * (`makeKhalaSyncScopeReadResolver`) — same seam as GET /api/sync/log.
   * Runs BEFORE the upgrade is forwarded, and again on every reconnect.
   */
  resolveScopeRead: KhalaSyncScopeReadResolver
  /** `env.KHALA_SYNC_HUB` — absent until the DO binding is deployed. */
  hubNamespace: KhalaSyncHubNamespaceLike | undefined
}>

const syncErrorResponse = (
  status: number,
  code: SyncErrorCode,
  messageSafe: string,
  retryable: boolean,
): HttpResponse =>
  noStoreJsonResponse(
    encodeSyncError(new SyncError({ code, messageSafe, retryable })),
    { status },
  )

const parseNonNegativeInt = (raw: string): number | undefined => {
  if (!/^\d+$/.test(raw)) return undefined
  const value = Number(raw)
  return Number.isSafeInteger(value) ? value : undefined
}

/**
 * `GET /api/sync/connect?scope=&cursor=` — authenticated WebSocket upgrade,
 * proxied to the per-scope hub DO. Success is the hub's own 101 upgrade
 * response (the `webSocket` end rides it back to the runtime); pre-upgrade
 * failures are typed `SyncError` bodies (see the module doc).
 */
export const handleKhalaSyncConnect = (
  request: Request,
  deps: KhalaSyncConnectDependencies,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    // Auth BEFORE the upgrade: an unauthenticated caller never reaches the
    // hub, and the socket is only ever accepted for a gated scope.
    const actor = await deps.authenticate()
    if (actor === undefined) {
      return syncErrorResponse(
        401,
        'unauthenticated',
        'Khala Sync connect requires an authenticated session or agent token.',
        false,
      )
    }

    const url = new URL(request.url)
    let scope: SyncScope
    try {
      scope = decodeScope(url.searchParams.get('scope'))
    } catch {
      return syncErrorResponse(
        400,
        'invalid_request',
        'scope query parameter must be a valid Khala Sync scope id.',
        false,
      )
    }
    const cursorRaw = url.searchParams.get('cursor')
    const cursor = cursorRaw === null ? 0 : parseNonNegativeInt(cursorRaw)
    if (cursor === undefined) {
      return syncErrorResponse(
        400,
        'invalid_request',
        'cursor query parameter must be a non-negative integer version watermark.',
        false,
      )
    }

    const authDenied = scopeReadDecisionResponse(
      await deps.resolveScopeRead(actor.userId, scope),
    )
    if (authDenied !== undefined) {
      return authDenied
    }

    if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket') {
      // 426 Upgrade Required: connect is WebSocket-only (SPEC §3); plain
      // HTTP catch-up is GET /api/sync/log.
      return syncErrorResponse(
        426,
        'invalid_request',
        'GET /api/sync/connect requires a WebSocket upgrade; use /api/sync/log for HTTP catch-up.',
        false,
      )
    }

    if (deps.hubNamespace === undefined) {
      return syncErrorResponse(
        503,
        'storage_unavailable',
        'Khala Sync live tail is not configured on this deployment ' +
          '(env.KHALA_SYNC_HUB Durable Object binding is absent).',
        true,
      )
    }

    try {
      const stub = deps.hubNamespace.get(deps.hubNamespace.idFromName(scope))
      const target = new URL('https://khala-sync-hub.openagents.internal/connect')
      target.searchParams.set('scope', scope)
      target.searchParams.set('cursor', String(cursor))
      // `new Request(url, request)` preserves the method, headers (including
      // the WebSocket Upgrade header), and the client socket end; the hub's
      // 101 response carries the server socket back through this proxy.
      return await stub.fetch(new Request(target.toString(), request))
    } catch {
      return syncErrorResponse(
        500,
        'internal',
        'Khala Sync live-tail upgrade failed unexpectedly; reconnect.',
        true,
      )
    }
  })
