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
// SCOPE AUTHORIZATION (v1 — deliberate seam): `canReadScopeV1` grants a user
// their own personal scope (`scope.user.<userId>`) and every public scope
// (`scope.public.*`). Fleet cockpit scopes (`scope.fleet_run.*`, KS-6.1
// #8302) are additionally granted to the scope OWNER via a storage-backed
// `khala_sync_scope_owners` lookup (fail-closed: binding absent or lookup
// failure denies with 503/403, never grants). Team/thread/agent-run scope
// membership is the KS-7 scope-auth workstream; it replaces these
// predicates, not the route.
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
  personalScope,
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
  readScopeOwner,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import type { KhalaSyncHubNamespaceLike } from './khala-sync-hub-do'
import type {
  KhalaSyncHyperdriveBinding,
  KhalaSyncPushSqlClient,
  MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'

type HttpResponse = globalThis.Response

export const KHALA_SYNC_LOG_PATH = '/api/sync/log'
export const KHALA_SYNC_LOG_ROUTE_REF = 'route.khala_sync.log.v0_1'

const decodeScope = S.decodeUnknownSync(SyncScope)
const encodeLogPage = S.encodeSync(LogPage)
const encodeSyncError = S.encodeSync(SyncError)

// ---------------------------------------------------------------------------
// v1 scope-read authorization (KS-7 SEAM — see module doc)
// ---------------------------------------------------------------------------

/**
 * v1 scope-read gate (synchronous part): a user may read their OWN
 * personal scope and any public scope. Fleet cockpit scopes
 * (`scope.fleet_run.*`) return false HERE and are instead resolved by the
 * storage-backed owner check in the handler (KS-6.1 #8302 —
 * `khala_sync_scope_owners` via `readScopeOwner`). DELIBERATE SEAM: the
 * remaining membership-backed scopes (team, thread, agent_run) are denied
 * until the KS-7 scope-auth workstream replaces these predicates.
 */
export const canReadScopeV1 = (userId: string, scope: SyncScope): boolean =>
  scope === personalScope(userId) || scope.startsWith('scope.public.')

export const FLEET_RUN_SCOPE_PREFIX = 'scope.fleet_run.'

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

    if (query.scope.startsWith(FLEET_RUN_SCOPE_PREFIX)) {
      // KS-6.1 (#8302): fleet cockpit scopes are readable by their OWNER
      // per khala_sync_scope_owners. Fail-closed: no binding or a failed
      // lookup can never grant access.
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
      let ownerClient: KhalaSyncPushSqlClient | undefined
      let owner: string | null
      try {
        ownerClient = await (deps.makeSqlClient ?? defaultMakeSqlClient)(
          deps.binding.connectionString,
        )
        owner = await readScopeOwner(ownerClient.sql, query.scope)
      } catch {
        return syncErrorResponse(
          503,
          'storage_unavailable',
          'Khala Sync scope-owner lookup failed; retry the read.',
          true,
        )
      } finally {
        if (ownerClient !== undefined) {
          try {
            await ownerClient.end()
          } catch {
            // best-effort teardown (same discipline as the read path).
          }
        }
      }
      if (owner === null || owner !== actor.userId) {
        return syncErrorResponse(
          403,
          'unauthorized_scope',
          'This user cannot read the requested scope.',
          false,
        )
      }
    } else if (!canReadScopeV1(actor.userId, query.scope)) {
      return syncErrorResponse(
        403,
        'unauthorized_scope',
        'This user cannot read the requested scope.',
        false,
      )
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
