// Khala Sync bootstrap route (KS-4.4, #8297): POST /api/sync/bootstrap.
//
// Serves consistent snapshot pages of one scope's current entity states
// (docs/khala-sync/SPEC.md §3): the client POSTs a typed `BootstrapRequest`
// (contracts from `@openagentsinc/khala-sync`) and drains
// `BootstrapResponse` pages via the `nextPageToken` flow; the FINAL page
// carries `cursor` — the scope version the snapshot was taken at — and the
// client stitches by catching up from exactly that cursor
// (`GET /api/sync/log?cursor=<cursor>`), then live-tailing
// (`GET /api/sync/connect`). Apply is idempotent per (scope, version,
// entity), so entities that changed while paging are simply re-delivered by
// the log with newer post-images — the seam is exact (see the KS-2.2
// read-service module doc for the self-contained page-token proof).
//
// The caller is resolved via the Worker's standard actor auth (browser
// session or programmatic agent bearer — same closure as push/log), and
// scope reads are gated by the same KS-7.1 resolver seam as GET
// /api/sync/log (`resolveScopeRead`: full taxonomy — personal, public,
// live team membership, agent_run/thread ownership, fleet_run scope owner;
// unknown kinds and failed lookups fail CLOSED).
//
// AUTH (KS-8.x anonymous-read exception; docs/khala-sync/RUNBOOK.md
// "Anonymous read scopes"): `scope.public.*` is the ONE exception —
// bootstrappable WITHOUT an authenticated actor. Because the scope lives
// INSIDE the POST body (not a query param), this route decodes the body
// FIRST (JSON parse, protocol/schema version gates, full `BootstrapRequest`
// decode) and only THEN decides whether a missing actor is fatal, via
// `isAnonymousReadableScope` (the exact-match parse `resolveScopeRead`
// itself uses). This deliberately moves the 401 check after body/version
// validation (previously first): a malformed or unsupported-version body
// now fails 400 before 401 even when the caller has no session at all —
// input-shape validation is not itself sensitive (the wire schema is
// public), so this ordering change carries no confidentiality cost.
// `authenticate()` is still attempted for a public scope too, so a
// signed-in caller's userId still reaches `resolveScopeRead` unchanged.
// Anonymous bootstraps additionally pass a best-effort per-IP window rate
// limit (`khala-sync-anonymous-rate-limit.ts`); authenticated bootstraps
// never see it.
//
// CACHING (deliberate: always `Cache-Control: no-store`): a bootstrap page
// is specific to the CALLER'S PAGING POSITION, not just its URL — the same
// (scope, pageSize) body yields different pages as `pageToken` advances, the
// first (token-less) page pins a fresh snapshot cursor on every call, and
// POST bodies are not part of shared-cache keys anyway. Unlike log pages
// (immutable for their URL once `nextCursor` is known, hence ETag'd), no
// snapshot page is safely cacheable; every response here is no-store.
//
// TRANSACTION-MODE SAFE (SPEC §4): the read runs through the `KHALA_SYNC_DB`
// Hyperdrive binding with postgres.js (`prepare: false`, `max: 1`, no
// session state). The KS-2.2 bootstrap holds NO transaction between pages —
// page tokens are self-contained — so each page is one REPEATABLE READ
// transaction and the client is ALWAYS ended in a finally block.
//
// HTTP-level failures are typed `SyncError` bodies (same taxonomy as
// push/log): 401 unauthenticated, 400 invalid_request /
// protocol_version_unsupported / schema_version_unsupported (bad body or
// undecodable/foreign-scope page token — restart the bootstrap without a
// token), 403 unauthorized_scope, 410 cursor_behind_retained_window
// (compaction passed the pinned snapshot cursor mid-drain: the stitch can
// no longer succeed — re-bootstrap from scratch), 429 rate_limited
// (anonymous bootstraps only), 503 storage_unavailable (retryable), 500
// internal.
//
// The real postgres.js client is dynamically imported ONLY when no
// `makeSqlClient` is injected (deployed Worker with `nodejs_compat`); tests
// inject fakes so CI never needs a database.

import { Effect, Schema as S } from 'effect'

import {
  type BootstrapRequest,
  BootstrapResponse,
  decodeBootstrapRequest,
  KHALA_SYNC_PROTOCOL_VERSION,
  SyncError,
  type SyncErrorCode,
} from '@openagentsinc/khala-sync'
import {
  bootstrap as bootstrapFromPostgresReal,
  type BootstrapInput,
  isAnonymousReadableScope,
  KhalaSyncCursorBehindRetainedWindowError,
  KhalaSyncInvalidPageTokenError,
  KhalaSyncStorageError,
  MAX_BOOTSTRAP_PAGE_SIZE,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import { scopeReadDecisionResponse } from './http/khala-sync-scope-read-response'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  makeKhalaSyncAnonymousReadRateLimiter,
  type KhalaSyncAnonymousRateLimiter,
} from './khala-sync-anonymous-rate-limit'
import type {
  KhalaSyncHyperdriveBinding,
  KhalaSyncPushSqlClient,
  MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { acquireSharedPostgresClient } from './khala-sync-postgres-pool'
import type { KhalaSyncScopeReadResolver } from './khala-sync-scope-auth'

type HttpResponse = globalThis.Response

export const KHALA_SYNC_BOOTSTRAP_PATH = '/api/sync/bootstrap'
export const KHALA_SYNC_BOOTSTRAP_ROUTE_REF = 'route.khala_sync.bootstrap.v0_1'

/** Client data-schema versions this Worker build can bootstrap. */
export const KHALA_SYNC_BOOTSTRAP_SUPPORTED_SCHEMA_VERSIONS: ReadonlyArray<number> =
  [1]

const encodeBootstrapResponse = S.encodeSync(BootstrapResponse)
const encodeSyncError = S.encodeSync(SyncError)

// ---------------------------------------------------------------------------
// Dependencies (injectable seams mirror khala-sync-push/log routes)
// ---------------------------------------------------------------------------

/** Postgres `bootstrap` seam so route tests never need a database. */
export type BootstrapFromPostgresFn = (
  sql: SyncSql,
  input: BootstrapInput,
) => Promise<BootstrapResponse>

export type KhalaSyncBootstrapDependencies = Readonly<{
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
   * (`makeKhalaSyncScopeReadResolver`) — same seam as GET /api/sync/log.
   */
  resolveScopeRead: KhalaSyncScopeReadResolver
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /**
   * Best-effort per-IP window rate limit applied ONLY to anonymous
   * bootstraps (KS-8.x); authenticated bootstraps never consult this.
   * Defaults to a module-level `makeKhalaSyncAnonymousReadRateLimiter()`
   * instance so a real deployment is protected with zero wiring; tests
   * inject a deterministic fake.
   */
  anonymousRateLimit?: KhalaSyncAnonymousRateLimiter | undefined
  /**
   * Injectable client factory. Default: dynamic import of `postgres`
   * (postgres.js), Worker-runtime only. Tests inject a fake — no network,
   * no database.
   */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /** Injectable read seam for route tests. Default: the real KS-2.2 read. */
  bootstrapFromPostgres?: BootstrapFromPostgresFn | undefined
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

// Same transaction-mode-safe postgres.js discipline as the push and log
// routes (SPEC §4): unnamed statements only, no session state. On Cloud Run
// this reuses the shared 'sync' pool via `acquireSharedPostgresClient`
// instead of a fresh connection per request.
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
// Route handler
// ---------------------------------------------------------------------------

/** Module-level default so a real deployment gets rate limiting with zero wiring. */
const defaultAnonymousReadRateLimit = makeKhalaSyncAnonymousReadRateLimiter()

/**
 * `POST /api/sync/bootstrap` — authenticated (session or agent bearer) for
 * every scope except `scope.public.*` (KS-8.x anonymous-read exception —
 * see the module doc). Success: 200 with one encoded `BootstrapResponse`
 * page (drain via `nextPageToken`; the final page carries `cursor`).
 * Failures are typed `SyncError` bodies (see the module doc for the status
 * map). Every response — success and failure — is `Cache-Control: no-store`.
 */
export const handleKhalaSyncBootstrap = (
  request: Request,
  deps: KhalaSyncBootstrapDependencies,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    let raw: unknown
    try {
      raw = await request.json()
    } catch {
      return syncErrorResponse(
        400,
        'invalid_request',
        'Request body is not valid JSON.',
        false,
      )
    }

    // Version gates BEFORE the full decode so mismatched clients get the
    // specific typed code they key their upgrade/refetch behavior on (same
    // ordering as POST /api/sync/push).
    const rawProtocolVersion =
      typeof raw === 'object' && raw !== null
        ? (raw as { protocolVersion?: unknown }).protocolVersion
        : undefined
    if (rawProtocolVersion !== KHALA_SYNC_PROTOCOL_VERSION) {
      return syncErrorResponse(
        400,
        'protocol_version_unsupported',
        `This server speaks Khala Sync protocol version ${KHALA_SYNC_PROTOCOL_VERSION}.`,
        false,
      )
    }

    let bootstrapRequest: BootstrapRequest
    try {
      bootstrapRequest = decodeBootstrapRequest(raw)
    } catch {
      // Decode errors can embed raw request values — never echo them.
      return syncErrorResponse(
        400,
        'invalid_request',
        'Request body failed to decode as a BootstrapRequest.',
        false,
      )
    }

    if (
      !KHALA_SYNC_BOOTSTRAP_SUPPORTED_SCHEMA_VERSIONS.includes(
        Number(bootstrapRequest.schemaVersion),
      )
    ) {
      return syncErrorResponse(
        400,
        'schema_version_unsupported',
        `Unsupported client schema version ${bootstrapRequest.schemaVersion}; ` +
          `this server supports ${KHALA_SYNC_BOOTSTRAP_SUPPORTED_SCHEMA_VERSIONS.join(', ')}.`,
        false,
      )
    }

    // Auth: `scope.public.*` is the ONLY kind bootstrappable without an
    // actor. Deferred until here (rather than before the body decode)
    // because the scope lives inside the body — see the module doc.
    // `authenticate()` is still attempted so a signed-in caller's userId
    // reaches `resolveScopeRead` even on a public scope.
    // CFG D1 evacuation (#8515): a bearer-token actor lookup can hit the
    // 401-dead D1 agent-registration store and THROW. For an anonymous-
    // readable `scope.public.*` bootstrap that throw must not crash the read
    // (no actor is needed) — degrade to anonymous; every non-public scope
    // keeps a fatal typed 503 on an auth-store failure.
    const anonymousAllowed = isAnonymousReadableScope(bootstrapRequest.scope)
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
          }); retry the bootstrap.`,
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
          'Khala Sync bootstrap requires an authenticated session or agent token.',
          false,
        )
      }
      const rateLimit =
        deps.anonymousRateLimit ?? defaultAnonymousReadRateLimit
      if (!rateLimit(request)) {
        return syncErrorResponse(
          429,
          'rate_limited',
          'Too many anonymous Khala Sync bootstrap requests from this address; retry later.',
          true,
        )
      }
    }

    const authDenied = scopeReadDecisionResponse(
      await deps.resolveScopeRead(actor?.userId, bootstrapRequest.scope),
    )
    if (authDenied !== undefined) {
      return authDenied
    }

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
    const bootstrapFromPostgres =
      deps.bootstrapFromPostgres ?? bootstrapFromPostgresReal

    let client: KhalaSyncPushSqlClient | undefined
    try {
      client = await makeSqlClient(deps.binding.connectionString)
      const page = await bootstrapFromPostgres(client.sql, {
        // The KS-2.2 read clamps again; bounding here keeps the route's
        // page-size contract visible without trusting a second layer.
        pageSize: Math.min(
          bootstrapRequest.pageSize ?? MAX_BOOTSTRAP_PAGE_SIZE,
          MAX_BOOTSTRAP_PAGE_SIZE,
        ),
        pageToken: bootstrapRequest.pageToken,
        scope: bootstrapRequest.scope,
      })
      return noStoreJsonResponse(encodeBootstrapResponse(page))
    } catch (error) {
      if (error instanceof KhalaSyncInvalidPageTokenError) {
        // Tokens are server-minted and opaque: an undecodable, foreign-scope,
        // or out-of-range token is a client error. Restart without a token.
        return syncErrorResponse(
          400,
          'invalid_request',
          'Bootstrap page token is invalid for this scope; restart the bootstrap.',
          false,
        )
      }
      if (error instanceof KhalaSyncCursorBehindRetainedWindowError) {
        // Compaction passed the pinned snapshot cursor mid-drain: the
        // post-snapshot stitch (logPage from that cursor) can never succeed,
        // so every page fails closed (invariant 6). Re-bootstrap fresh.
        return syncErrorResponse(
          410,
          'cursor_behind_retained_window',
          'The bootstrap snapshot fell behind the retained window; restart the bootstrap.',
          false,
        )
      }
      if (error instanceof KhalaSyncStorageError) {
        return syncErrorResponse(
          503,
          'storage_unavailable',
          `Khala Sync storage failed (${error.reason}); retry the bootstrap page.`,
          true,
        )
      }
      return syncErrorResponse(
        500,
        'internal',
        'Khala Sync bootstrap failed unexpectedly; retry the bootstrap page.',
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
