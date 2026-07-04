// Khala Sync catch-up log route (KS-4.3, #8296): GET /api/sync/log.
//
// Offset-resumable `LogPage` catch-up (docs/khala-sync/SPEC.md §3):
// `?scope=&cursor=&limit=` → `LogPage { entries, nextCursor, upToDate }`,
// resolved for the AUTHENTICATED user via the Worker's standard actor auth
// (browser session or programmatic agent bearer — same seam as
// POST /api/sync/push).
//
// SERVING ORDER (hub is cache, Postgres is authoritative):
//   1. The per-scope KhalaSyncHubDO (`env.KHALA_SYNC_HUB`) serves the page
//      from its DO SQLite window when the requested range is inside it.
//   2. When the hub cannot prove the range — 410 behind its window, 409
//      ahead of a reset/rebuilding hub, binding absent, or any other hub
//      failure — the route falls through to an authoritative Postgres
//      `logPage` read through the `KHALA_SYNC_DB` Hyperdrive binding
//      (KS-2.2 read substrate; REPEATABLE READ, transaction-mode safe).
//   3. Only Postgres decides `cursor_behind_retained_window`: a cursor
//      behind `khala_sync_scopes.retained_from_version` gets a 410 typed
//      `SyncError { code: cursor_behind_retained_window, retryable: false }`
//      — MustRefetch, never a silently partial log (invariant 6).
//
// SCOPE AUTHORIZATION (KS-7.1, #8305): the injected `resolveScopeRead`
// seam — the taxonomy-complete resolver from
// `@openagentsinc/khala-sync-server` wired over live D1 membership/
// ownership and the Postgres `khala_sync_scope_owners` lookup
// (`makeKhalaSyncScopeReadResolver` in ./khala-sync-scope-auth). One
// decision per request: allowed, denied (403 `unauthorized_scope`, or 403
// `unknown_scope` for taxonomy members with no read policy), or
// unavailable (503 — a failed lookup fails CLOSED, never grants).
//
// CACHING (issue contract: ETag on (scope, nextCursor)):
//   - Pages that are NOT `upToDate` are immutable for their URL: the
//     changelog is append-only, so for a fixed (scope, cursor, limit) the
//     page's content is fully determined once `nextCursor` is known. They
//     are served with `ETag: "khala-sync-log.<scope>.<nextCursor>"` and
//     `Cache-Control: private, max-age=0, must-revalidate` (auth'd content —
//     never shared caches), and revalidate to 304 on If-None-Match.
//   - `upToDate` pages change as soon as the next mutation commits — they
//     are `Cache-Control: no-store`, no ETag.
//
// HTTP-level failures are typed `SyncError` bodies (same taxonomy as push):
// 401 unauthenticated, 400 invalid_request, 403 unauthorized_scope, 410
// cursor_behind_retained_window, 503 storage_unavailable (retryable), 500
// internal. Error responses are always no-store.
//
// The real postgres.js client is dynamically imported ONLY when no
// `makeSqlClient` is injected (deployed Worker with `nodejs_compat`); tests
// inject fakes so CI never needs a database.

import { Effect, Schema as S } from 'effect'

import {
  decodeLogPage,
  LogPage,
  SyncError,
  type SyncErrorCode,
  SyncScope,
} from '@openagentsinc/khala-sync'
import {
  DEFAULT_LOG_PAGE_LIMIT,
  KhalaSyncCursorBehindRetainedWindowError,
  KhalaSyncStorageError,
  logPage as logPageFromPostgres,
  type LogPageInput,
  MAX_LOG_PAGE_LIMIT,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import { scopeReadDecisionResponse } from './http/khala-sync-scope-read-response'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import type { KhalaSyncHubNamespaceLike } from './khala-sync-hub-do'
import type {
  KhalaSyncHyperdriveBinding,
  KhalaSyncPushSqlClient,
  MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import type { KhalaSyncScopeReadResolver } from './khala-sync-scope-auth'

type HttpResponse = globalThis.Response

export const KHALA_SYNC_LOG_PATH = '/api/sync/log'
export const KHALA_SYNC_LOG_ROUTE_REF = 'route.khala_sync.log.v0_1'

const decodeScope = S.decodeUnknownSync(SyncScope)
const encodeLogPage = S.encodeSync(LogPage)
const encodeSyncError = S.encodeSync(SyncError)

// ---------------------------------------------------------------------------
// Dependencies (injectable seams mirror khala-sync-push-routes)
// ---------------------------------------------------------------------------

/** Postgres `logPage` seam so route tests never need a database. */
export type LogPageFromPostgresFn = (
  sql: SyncSql,
  input: LogPageInput,
) => Promise<LogPage>

export type KhalaSyncLogDependencies = Readonly<{
  /**
   * Resolve the authenticated caller via the Worker's standard actor auth
   * (`authenticateRequestActor`: browser session or agent bearer token).
   * `undefined` ⇒ 401.
   */
  authenticate: () => Promise<{ readonly userId: string } | undefined>
  /**
   * Scope-read authorization (KS-7.1): the taxonomy-complete resolver
   * (`makeKhalaSyncScopeReadResolver`). Runs after authentication, before
   * any storage read; non-allowed decisions map through
   * `scopeReadDecisionResponse` (403/503, fail-closed).
   */
  resolveScopeRead: KhalaSyncScopeReadResolver
  /** `env.KHALA_SYNC_HUB` — absent until the DO binding is deployed. */
  hubNamespace: KhalaSyncHubNamespaceLike | undefined
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /**
   * Injectable client factory. Default: dynamic import of `postgres`
   * (postgres.js), Worker-runtime only. Tests inject a fake — no network,
   * no database.
   */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /** Injectable read seam for route tests. Default: the real KS-2.2 read. */
  logPageFromPostgres?: LogPageFromPostgresFn | undefined
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

const invalidRequest = (messageSafe: string): HttpResponse =>
  syncErrorResponse(400, 'invalid_request', messageSafe, false)

// Same transaction-mode-safe postgres.js client discipline as the push
// route (SPEC §4): one connection, unnamed statements only, no session
// state. Duplicated rather than exported from the push route so each
// route's driver seam stays independently visible and testable.
const defaultMakeSqlClient: MakeKhalaSyncPushSqlClient = async (
  connectionString,
) => {
  const mod = (await import('postgres')) as unknown as {
    default: (
      connectionString: string,
      options: Record<string, unknown>,
    ) => {
      end: (options?: { timeout?: number }) => Promise<void>
    }
  }
  const sql = mod.default(connectionString, {
    connect_timeout: 10,
    max: 1,
    prepare: false,
  })
  return {
    end: () => sql.end({ timeout: 5 }),
    sql: sql as unknown as SyncSql,
  }
}

// ---------------------------------------------------------------------------
// Query parsing
// ---------------------------------------------------------------------------

const parseNonNegativeInt = (raw: string): number | undefined => {
  if (!/^\d+$/.test(raw)) return undefined
  const value = Number(raw)
  return Number.isSafeInteger(value) ? value : undefined
}

type LogQuery = Readonly<{
  scope: SyncScope
  cursor: number
  limit: number
}>

const parseLogQuery = (url: URL): LogQuery | HttpResponse => {
  let scope: SyncScope
  try {
    scope = decodeScope(url.searchParams.get('scope'))
  } catch {
    return invalidRequest(
      'scope query parameter must be a valid Khala Sync scope id.',
    )
  }
  const cursorRaw = url.searchParams.get('cursor')
  const cursor = cursorRaw === null ? 0 : parseNonNegativeInt(cursorRaw)
  if (cursor === undefined) {
    return invalidRequest(
      'cursor query parameter must be a non-negative integer version watermark.',
    )
  }
  const limitRaw = url.searchParams.get('limit')
  const parsedLimit =
    limitRaw === null ? DEFAULT_LOG_PAGE_LIMIT : parseNonNegativeInt(limitRaw)
  if (parsedLimit === undefined || parsedLimit < 1) {
    return invalidRequest('limit query parameter must be a positive integer.')
  }
  return { cursor, limit: Math.min(parsedLimit, MAX_LOG_PAGE_LIMIT), scope }
}

// ---------------------------------------------------------------------------
// Response shaping (issue contract: ETag on (scope, nextCursor))
// ---------------------------------------------------------------------------

/** Strong ETag for a non-upToDate page: content-determining for its URL. */
export const logPageEtag = (scope: SyncScope, nextCursor: number): string =>
  `"khala-sync-log.${scope}.${nextCursor}"`

const logPageResponse = (request: Request, page: LogPage): HttpResponse => {
  if (page.upToDate) {
    // The live edge changes on the next commit — never cache it.
    return noStoreJsonResponse(encodeLogPage(page))
  }
  const etag = logPageEtag(page.scope, Number(page.nextCursor))
  const headers = {
    'cache-control': 'private, max-age=0, must-revalidate',
    etag,
  }
  const ifNoneMatch = request.headers.get('if-none-match')
  if (
    ifNoneMatch !== null &&
    ifNoneMatch
      .split(',')
      .map(candidate => candidate.trim())
      .includes(etag)
  ) {
    return new Response(null, { headers, status: 304 })
  }
  return new Response(JSON.stringify(encodeLogPage(page)), {
    headers: { ...headers, 'content-type': 'application/json' },
    status: 200,
  })
}

// ---------------------------------------------------------------------------
// Hub attempt (never authoritative — any failure falls through)
// ---------------------------------------------------------------------------

/**
 * Try the per-scope hub window. Returns the decoded page on a window hit,
 * or `undefined` when the route must fall through to Postgres: hub 410
 * (range behind the window / empty window), hub 409 (cursor ahead of a
 * reset hub, or scope pin mismatch), binding absent, or ANY unexpected hub
 * response/failure — the hub is a cache and must never take down or
 * substitute for the authoritative read path.
 */
const tryHubLogPage = async (
  namespace: KhalaSyncHubNamespaceLike | undefined,
  query: LogQuery,
): Promise<LogPage | undefined> => {
  if (namespace === undefined) return undefined
  try {
    const stub = namespace.get(namespace.idFromName(query.scope))
    const target = new URL('https://khala-sync-hub.openagents.internal/log')
    target.searchParams.set('scope', query.scope)
    target.searchParams.set('cursor', String(query.cursor))
    target.searchParams.set('limit', String(query.limit))
    const response = await stub.fetch(new Request(target.toString()))
    if (response.status !== 200) return undefined
    return decodeLogPage(await response.json())
  } catch {
    return undefined
  }
}

// ---------------------------------------------------------------------------
// Route handler
// ---------------------------------------------------------------------------

/**
 * `GET /api/sync/log?scope=&cursor=&limit=` — authenticated (session or
 * agent bearer). Success: 200 (or 304) with the encoded `LogPage`; failures
 * are typed `SyncError` bodies (see the module doc for the status map).
 */
export const handleKhalaSyncLog = (
  request: Request,
  deps: KhalaSyncLogDependencies,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const actor = await deps.authenticate()
    if (actor === undefined) {
      return syncErrorResponse(
        401,
        'unauthenticated',
        'Khala Sync log reads require an authenticated session or agent token.',
        false,
      )
    }

    const query = parseLogQuery(new URL(request.url))
    if (query instanceof Response) {
      return query
    }

    const authDenied = scopeReadDecisionResponse(
      await deps.resolveScopeRead(actor.userId, query.scope),
    )
    if (authDenied !== undefined) {
      return authDenied
    }

    // 1. Hub window (cache) first.
    const hubPage = await tryHubLogPage(deps.hubNamespace, query)
    if (hubPage !== undefined) {
      return logPageResponse(request, hubPage)
    }

    // 2. Authoritative Postgres fallthrough.
    if (
      deps.binding === undefined ||
      typeof deps.binding.connectionString !== 'string' ||
      deps.binding.connectionString.length === 0
    ) {
      return syncErrorResponse(
        503,
        'storage_unavailable',
        'Khala Sync storage is not configured on this deployment ' +
          '(env.KHALA_SYNC_DB Hyperdrive binding is absent).',
        true,
      )
    }

    const makeSqlClient = deps.makeSqlClient ?? defaultMakeSqlClient
    const readLogPage = deps.logPageFromPostgres ?? logPageFromPostgres

    let client: KhalaSyncPushSqlClient | undefined
    try {
      client = await makeSqlClient(deps.binding.connectionString)
      const page = await readLogPage(client.sql, {
        afterVersion: query.cursor,
        limit: query.limit,
        scope: query.scope,
      })
      return logPageResponse(request, page)
    } catch (error) {
      if (error instanceof KhalaSyncCursorBehindRetainedWindowError) {
        // Compaction passed the cursor: the range is permanently gone.
        // MustRefetch (invariant 6) — the client clears scope-local state
        // and re-bootstraps; retrying the same cursor can never succeed.
        return syncErrorResponse(
          410,
          'cursor_behind_retained_window',
          'Cursor is behind the retained window for this scope; re-bootstrap.',
          false,
        )
      }
      if (error instanceof KhalaSyncStorageError) {
        return syncErrorResponse(
          503,
          'storage_unavailable',
          `Khala Sync storage failed (${error.reason}); retry the read.`,
          true,
        )
      }
      return syncErrorResponse(
        500,
        'internal',
        'Khala Sync log read failed unexpectedly; retry the read.',
        true,
      )
    } finally {
      if (client !== undefined) {
        try {
          await client.end()
        } catch {
          // best-effort teardown: never mask the real result with a close
          // error; the `max: 1` client is dropped with the isolate anyway.
        }
      }
    }
  })
