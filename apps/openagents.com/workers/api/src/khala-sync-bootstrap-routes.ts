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
// no longer succeed — re-bootstrap from scratch), 503 storage_unavailable
// (retryable), 500 internal.
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
  KhalaSyncCursorBehindRetainedWindowError,
  KhalaSyncInvalidPageTokenError,
  KhalaSyncStorageError,
  MAX_BOOTSTRAP_PAGE_SIZE,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import type {
  KhalaSyncHyperdriveBinding,
  KhalaSyncPushSqlClient,
  MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import {
  type KhalaSyncScopeReadResolver,
  scopeReadDecisionResponse,
} from './khala-sync-scope-auth'

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
   * `undefined` ⇒ 401.
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

// Same transaction-mode-safe postgres.js client discipline as the push and
// log routes (SPEC §4): one connection, unnamed statements only, no session
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
// Route handler
// ---------------------------------------------------------------------------

/**
 * `POST /api/sync/bootstrap` — authenticated (session or agent bearer).
 * Success: 200 with one encoded `BootstrapResponse` page (drain via
 * `nextPageToken`; the final page carries `cursor`). Failures are typed
 * `SyncError` bodies (see the module doc for the status map). Every
 * response — success and failure — is `Cache-Control: no-store`.
 */
export const handleKhalaSyncBootstrap = (
  request: Request,
  deps: KhalaSyncBootstrapDependencies,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const actor = await deps.authenticate()
    if (actor === undefined) {
      return syncErrorResponse(
        401,
        'unauthenticated',
        'Khala Sync bootstrap requires an authenticated session or agent token.',
        false,
      )
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

    const authDenied = scopeReadDecisionResponse(
      await deps.resolveScopeRead(actor.userId, bootstrapRequest.scope),
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
