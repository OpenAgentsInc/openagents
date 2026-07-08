// Khala Sync CVR diff pull (KS-7.2, #8306): POST /api/sync/cvr-pull,
// FLAG-GATED behind `KHALA_SYNC_CVR=1` (env var). Unflagged deployments
// return 404 from this handler — byte-for-byte the Worker's unknown-route
// behavior, so registering the route is a zero-behavior change until the
// flag is set. Design: docs/khala-sync/CVR_DESIGN.md.
//
// The client POSTs a typed `CvrPullRequest` (contracts from
// `@openagentsinc/khala-sync`): the CVR version its durable state was last
// reconciled to (absent ⇒ reset-mode pull) plus its drift rows. The server
// (KS-7.2 cvr-service) computes the current authorized row set at ONE
// REPEATABLE READ snapshot, set-diffs it against the stored CVR, stores the
// new CVR, and answers puts/dels + the new cvrVersion + the snapshot
// cursor; the client stitches with `GET /api/sync/log?cursor=<cursor>` and
// live-tails exactly like the bootstrap path. This is the SLOW/recovery
// path (`must_refetch`); live deltas remain primary.
//
// Auth mirrors POST /api/sync/bootstrap exactly: the Worker's standard
// actor auth (browser session or agent bearer) and the KS-7.1
// taxonomy-complete scope-read resolver (fail-closed). Responses are
// no-store (a pull is specific to the caller's CVR position, never
// shareable). Transaction-mode safe: one postgres.js client
// (`prepare: false`, `max: 1`) through the KHALA_SYNC_DB Hyperdrive
// binding, ended in a finally block.
//
// HTTP failure taxonomy (typed `SyncError` bodies, same as bootstrap):
// 401 unauthenticated, 400 invalid_request / protocol_version_unsupported /
// schema_version_unsupported (also: row set too large — fall back to the
// paged bootstrap), 403 unauthorized_scope, 503 storage_unavailable
// (retryable), 500 internal.

import { Effect, Schema as S } from 'effect'

import {
  type CvrPullRequest,
  CvrPullResponse,
  decodeCvrPullRequest,
  KHALA_SYNC_PROTOCOL_VERSION,
  SyncError,
  type SyncErrorCode,
} from '@openagentsinc/khala-sync'
import {
  cvrPull as cvrPullFromPostgresReal,
  type CvrPullInput,
  KhalaSyncCvrRowSetTooLargeError,
  KhalaSyncStorageError,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import { scopeReadDecisionResponse } from './http/khala-sync-scope-read-response'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import type {
  KhalaSyncHyperdriveBinding,
  KhalaSyncPushSqlClient,
  MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { acquireSharedPostgresClient } from './khala-sync-postgres-pool'
import type { KhalaSyncScopeReadResolver } from './khala-sync-scope-auth'

// Re-exported so route registration (index.ts) reads the flag through one
// canonical parser without importing the server package directly.
export { isKhalaSyncCvrEnabled } from '@openagentsinc/khala-sync-server'

type HttpResponse = globalThis.Response

export const KHALA_SYNC_CVR_PULL_PATH = '/api/sync/cvr-pull'
export const KHALA_SYNC_CVR_PULL_ROUTE_REF = 'route.khala_sync.cvr_pull.v0_1'

/** Client data-schema versions this Worker build can serve CVR pulls for. */
export const KHALA_SYNC_CVR_SUPPORTED_SCHEMA_VERSIONS: ReadonlyArray<number> = [
  1,
]

const encodeCvrPullResponse = S.encodeSync(CvrPullResponse)
const encodeSyncError = S.encodeSync(SyncError)

// ---------------------------------------------------------------------------
// Dependencies (injectable seams mirror the bootstrap route)
// ---------------------------------------------------------------------------

/** Postgres `cvrPull` seam so route tests never need a database. */
export type CvrPullFromPostgresFn = (
  sql: SyncSql,
  input: CvrPullInput,
) => Promise<CvrPullResponse>

export type KhalaSyncCvrPullDependencies = Readonly<{
  /**
   * The KS-7.2 flag: `isKhalaSyncCvrEnabled(env.KHALA_SYNC_CVR)`. False ⇒
   * this handler answers 404 exactly like an unregistered route.
   */
  enabled: boolean
  /** Standard actor auth (browser session or agent bearer). `undefined` ⇒ 401. */
  authenticate: () => Promise<{ readonly userId: string } | undefined>
  /** KS-7.1 taxonomy-complete scope-read resolver — same seam as bootstrap. */
  resolveScopeRead: KhalaSyncScopeReadResolver
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /** Injectable client factory (tests inject a fake — no network/database). */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /** Injectable pull seam for route tests. Default: the real KS-7.2 service. */
  cvrPullFromPostgres?: CvrPullFromPostgresFn | undefined
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

// Same transaction-mode-safe postgres.js discipline as the push/log/
// bootstrap routes (SPEC §4): unnamed statements only, no session state. On
// Cloud Run this reuses the shared 'sync' pool via
// `acquireSharedPostgresClient` instead of a fresh connection per request.
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

/**
 * `POST /api/sync/cvr-pull` — flag-gated (KHALA_SYNC_CVR=1), authenticated
 * (session or agent bearer). Success: 200 with one encoded
 * `CvrPullResponse`. Failures are typed `SyncError` bodies (see the module
 * doc for the status map). Every response is `Cache-Control: no-store`.
 */
export const handleKhalaSyncCvrPull = (
  request: Request,
  deps: KhalaSyncCvrPullDependencies,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (!deps.enabled) {
      // Flag OFF: indistinguishable from the route not existing.
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const actor = await deps.authenticate()
    if (actor === undefined) {
      return syncErrorResponse(
        401,
        'unauthenticated',
        'Khala Sync CVR pull requires an authenticated session or agent token.',
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

    // Version gates BEFORE the full decode (same ordering as push/bootstrap)
    // so mismatched clients get the specific typed code they key on.
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

    let pullRequest: CvrPullRequest
    try {
      pullRequest = decodeCvrPullRequest(raw)
    } catch {
      // Decode errors can embed raw request values — never echo them.
      return syncErrorResponse(
        400,
        'invalid_request',
        'Request body failed to decode as a CvrPullRequest.',
        false,
      )
    }

    if (
      !KHALA_SYNC_CVR_SUPPORTED_SCHEMA_VERSIONS.includes(
        Number(pullRequest.schemaVersion),
      )
    ) {
      return syncErrorResponse(
        400,
        'schema_version_unsupported',
        `Unsupported client schema version ${pullRequest.schemaVersion}; ` +
          `this server supports ${KHALA_SYNC_CVR_SUPPORTED_SCHEMA_VERSIONS.join(', ')}.`,
        false,
      )
    }

    const authDenied = scopeReadDecisionResponse(
      await deps.resolveScopeRead(actor.userId, pullRequest.scope),
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
    const cvrPullFromPostgres =
      deps.cvrPullFromPostgres ?? cvrPullFromPostgresReal

    let client: KhalaSyncPushSqlClient | undefined
    try {
      client = await makeSqlClient(deps.binding.connectionString)
      const response = await cvrPullFromPostgres(client.sql, {
        scope: pullRequest.scope,
        clientGroupId: pullRequest.clientGroupId,
        cvrVersion:
          pullRequest.cvrVersion === undefined
            ? null
            : Number(pullRequest.cvrVersion),
        drift: (pullRequest.drift ?? []).map((entry) => ({
          entityType: String(entry.entityType),
          entityId: String(entry.entityId),
          version: Number(entry.version),
        })),
      })
      return noStoreJsonResponse(encodeCvrPullResponse(response))
    } catch (error) {
      if (error instanceof KhalaSyncCvrRowSetTooLargeError) {
        // Not a fault: the scope is simply too large for the single-
        // response CVR path. The client falls back to the paged bootstrap.
        return syncErrorResponse(
          400,
          'invalid_request',
          'The scope row set exceeds the CVR pull limit; use the paged bootstrap.',
          false,
        )
      }
      if (error instanceof KhalaSyncStorageError) {
        return syncErrorResponse(
          503,
          'storage_unavailable',
          `Khala Sync storage failed (${error.reason}); retry the CVR pull.`,
          true,
        )
      }
      return syncErrorResponse(
        500,
        'internal',
        'Khala Sync CVR pull failed unexpectedly; retry the CVR pull.',
        true,
      )
    } finally {
      if (client !== undefined) {
        try {
          await client.end()
        } catch {
          // best-effort teardown — same posture as the bootstrap route.
        }
      }
    }
  })
