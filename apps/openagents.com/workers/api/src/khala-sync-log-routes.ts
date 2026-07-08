// Khala Sync catch-up log route (KS-4.3, #8296): GET /api/sync/log.
//
// Offset-resumable `LogPage` catch-up (docs/khala-sync/SPEC.md §3):
// `?scope=&cursor=&limit=` → `LogPage { entries, nextCursor, upToDate }`,
// resolved for the AUTHENTICATED user via the Worker's standard actor auth
// (browser session or programmatic agent bearer — same seam as
// POST /api/sync/push).
//
// AUTH (KS-8.x anonymous-read exception; docs/khala-sync/RUNBOOK.md
// "Anonymous read scopes"): `scope.public.*` is the ONE exception —
// readable WITHOUT an authenticated actor. `isAnonymousReadableScope` (the
// exact-match parse `resolveScopeRead` itself uses; single source of
// truth) decides this BEFORE `authenticate()` is required to succeed.
// Every other scope kind still 401s on a missing/invalid session.
// `authenticate()` is still attempted for a public scope too, so a
// signed-in caller's userId still reaches `resolveScopeRead` unchanged.
// Anonymous reads additionally pass a best-effort per-IP window rate limit
// (`khala-sync-anonymous-rate-limit.ts`); authenticated reads never see it.
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
// cursor_behind_retained_window, 429 rate_limited (anonymous reads only),
// 503 storage_unavailable (retryable), 500 internal. Error responses are
// always no-store.
//
// The real postgres.js client is dynamically imported ONLY when no
// `makeSqlClient` is injected (deployed Worker with `nodejs_compat`); tests
// inject fakes so CI never needs a database.

import { Effect, Schema as S } from 'effect'

import {
  decodeLogPage,
  KHALA_SYNC_PROTOCOL_VERSION,
  LogPage,
  SyncError,
  type SyncErrorCode,
  SyncScope,
  SyncVersionWatermark,
} from '@openagentsinc/khala-sync'
import {
  DEFAULT_LOG_PAGE_LIMIT,
  isAnonymousReadableScope,
  KhalaSyncCursorBehindRetainedWindowError,
  KhalaSyncStorageError,
  logPage as logPageFromPostgres,
  type LogPageInput,
  MAX_LOG_PAGE_LIMIT,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import { scopeReadDecisionResponse } from './http/khala-sync-scope-read-response'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  makeKhalaSyncAnonymousReadRateLimiter,
  type KhalaSyncAnonymousRateLimiter,
} from './khala-sync-anonymous-rate-limit'
import type { KhalaSyncHubNamespaceLike } from './khala-sync-hub-do'
import type {
  KhalaSyncHyperdriveBinding,
  KhalaSyncPushSqlClient,
  MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { acquireSharedPostgresClient } from './khala-sync-postgres-pool'
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
   * `undefined` ⇒ no actor. ALWAYS attempted, even for a public scope (so a
   * signed-in caller's userId still reaches `resolveScopeRead`); only fatal
   * (401) when the requested scope is NOT anonymous-readable (see
   * `isAnonymousReadableScope`).
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
   * Best-effort per-IP window rate limit applied ONLY to anonymous reads
   * (KS-8.x); authenticated reads never consult this. Defaults to a
   * module-level `makeKhalaSyncAnonymousReadRateLimiter()` instance so a
   * real deployment is protected with zero wiring; tests inject a
   * deterministic fake.
   */
  anonymousRateLimit?: KhalaSyncAnonymousRateLimiter | undefined
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

// Same transaction-mode-safe postgres.js discipline as the push route
// (SPEC §4): unnamed statements only, no session state. On Cloud Run this
// reuses the shared 'sync' pool via `acquireSharedPostgresClient` instead of
// opening a fresh connection per request.
const defaultMakeSqlClient: MakeKhalaSyncPushSqlClient = async (
  connectionString,
) => {
  const { sql, end } = await acquireSharedPostgresClient({
    connectionString,
    options: { connect_timeout: 10, prepare: false },
    variant: 'sync',
  })
  return { end, sql: sql as unknown as SyncSql }
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

/** Module-level default so a real deployment gets rate limiting with zero wiring. */
const defaultAnonymousReadRateLimit = makeKhalaSyncAnonymousReadRateLimiter()

/**
 * `GET /api/sync/log?scope=&cursor=&limit=` — authenticated (session or
 * agent bearer) for every scope except `scope.public.*` (KS-8.x
 * anonymous-read exception — see the module doc). Success: 200 (or 304)
 * with the encoded `LogPage`; failures are typed `SyncError` bodies (see
 * the module doc for the status map).
 */
export const handleKhalaSyncLog = (
  request: Request,
  deps: KhalaSyncLogDependencies,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const query = parseLogQuery(new URL(request.url))
    if (query instanceof Response) {
      return query
    }

    // Auth: `scope.public.*` is the ONLY kind readable without an actor.
    // `authenticate()` is still attempted so a signed-in caller's userId
    // reaches `resolveScopeRead` even on a public scope.
    //
    // CFG D1 evacuation (#8515): the actor-auth path can consult the D1-backed
    // agent-registration store when a bearer token is present; with the
    // Cloudflare D1 bridge 401-dead that lookup THROWS. For an
    // anonymous-readable `scope.public.*` read (e.g. the public
    // activity-timeline the Trigger verifier polls) that throw must NOT crash
    // the read — the scope needs no actor — so a failed auth degrades to
    // anonymous and the read proceeds from Postgres. For every non-public
    // scope an auth-store failure stays fatal (typed 503, retryable) rather
    // than a false grant.
    const anonymousAllowed = isAnonymousReadableScope(query.scope)
    let actor: { readonly userId: string } | undefined
    try {
      actor = await deps.authenticate()
    } catch (error) {
      if (!anonymousAllowed) {
        return syncErrorResponse(
          503,
          'storage_unavailable',
          `Khala Sync actor authentication is unavailable (${
            error instanceof Error ? error.name : 'error'
          }); retry the read.`,
          true,
        )
      }
      actor = undefined
    }
    if (actor === undefined) {
      if (!anonymousAllowed) {
        return syncErrorResponse(
          401,
          'unauthenticated',
          'Khala Sync log reads require an authenticated session or agent token.',
          false,
        )
      }
      const rateLimit =
        deps.anonymousRateLimit ?? defaultAnonymousReadRateLimit
      if (!rateLimit(request)) {
        return syncErrorResponse(
          429,
          'rate_limited',
          'Too many anonymous Khala Sync log reads from this address; retry later.',
          true,
        )
      }
    }

    // CFG D1 evacuation (#8515): a `scope.public.*` read is a public
    // PROJECTION and must NEVER 500. Live on the Cloud Run monolith,
    // `scope.public.activity-timeline` returned an empty-body top-level 500
    // (an uncaught throw from the scope resolver / hub / the oversized-snapshot
    // Postgres transaction itself — NOT the typed inner catch). So for an
    // anonymous-readable scope every failure below degrades to a 200 EMPTY page
    // (`upToDate:false`, so a poller simply retries next tick) instead of a
    // 500/503. Non-public scopes keep their fail-CLOSED typed errors unchanged.
    const servePublicDegraded = (): HttpResponse =>
      logPageResponse(
        request,
        new LogPage({
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          scope: query.scope,
          entries: [],
          nextCursor: SyncVersionWatermark.make(query.cursor),
          upToDate: false,
        }),
      )

    try {
      const authDenied = scopeReadDecisionResponse(
        await deps.resolveScopeRead(actor?.userId, query.scope),
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
        return anonymousAllowed
          ? servePublicDegraded()
          : syncErrorResponse(
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
          // A valid client-refetch signal even for a public poller.
          return syncErrorResponse(
            410,
            'cursor_behind_retained_window',
            'Cursor is behind the retained window for this scope; re-bootstrap.',
            false,
          )
        }
        if (anonymousAllowed) {
          return servePublicDegraded()
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
    } catch (error) {
      // Uncaught throw from the scope resolver, the hub attempt, or the
      // Postgres transaction itself (the empty-body top-level 500 observed
      // live on `scope.public.activity-timeline`). A public projection
      // degrades to 200-empty; every other scope stays fail-CLOSED.
      if (anonymousAllowed) {
        return servePublicDegraded()
      }
      throw error
    }
  })
