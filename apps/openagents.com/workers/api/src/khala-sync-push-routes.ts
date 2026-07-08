// Khala Sync push route (KS-3.1, #8291): POST /api/sync/push.
//
// Decodes a `PushRequest` (typed contracts from `@openagentsinc/khala-sync`),
// resolves the AUTHENTICATED user via the Worker's standard actor auth
// (browser session or programmatic agent bearer — never from the request
// body), and executes the batch through the transactional push engine
// (`@openagentsinc/khala-sync-server`): ONE Postgres transaction per
// mutation envelope — client-state binding → idempotency/ordering gate →
// mutator execution → changelog appends → mutation-ledger recording, all
// atomic (docs/khala-sync/SPEC.md §2.4/§3, invariants 3 and 5).
//
// ACCEPTANCE RULES (SPEC §2.4, invariant 5 of the issue): per-mutation
// validation failures are IN-BAND `MutationResult` values inside a 200
// `PushResponse` — they ack the mutation and NEVER 4xx/block the client
// queue. HTTP-level errors are reserved for whole-request failures and are
// typed `SyncError` bodies: 401 unauthenticated, 400 invalid_request /
// protocol_version_unsupported / schema_version_unsupported (the client
// build is wrong — a retry of the same bytes can never succeed), 403
// unauthorized_scope (client group bound to another user), 503
// storage_unavailable (retryable — committed prefixes replay as
// duplicates), 500 internal.
//
// TRANSACTION-MODE SAFE (SPEC §4): the route reaches Cloud SQL through the
// `KHALA_SYNC_DB` Hyperdrive binding with postgres.js — `prepare: false`
// (unnamed statements only), `max: 1`, no session state, no LISTEN/NOTIFY,
// no advisory locks; the engine uses only single BEGIN…COMMIT transactions
// and ordinary row locks. The client is ALWAYS ended in a finally block.
//
// The real postgres.js client is dynamically imported ONLY when no
// `makeSqlClient` is injected (deployed Worker with `nodejs_compat`); tests
// inject fakes so CI never needs a database.

import { Effect } from 'effect'

import {
  decodePushRequest,
  encodePushResponse,
  encodeSyncError,
  KHALA_SYNC_PROTOCOL_VERSION,
  type PushRequest,
  type PushResponse,
  SyncError,
  type SyncErrorCode,
} from '@openagentsinc/khala-sync'
import {
  executePush as executePushEngine,
  KhalaSyncClientStateMismatchError,
  KhalaSyncStorageError,
  type MutatorRegistry,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { acquireSharedPostgresClient } from './khala-sync-postgres-pool'

type HttpResponse = globalThis.Response

export const KHALA_SYNC_PUSH_PATH = '/api/sync/push'
export const KHALA_SYNC_PUSH_ROUTE_REF = 'route.khala_sync.push.v0_1'

/** Client data-schema versions this Worker build can execute mutators for. */
export const KHALA_SYNC_PUSH_SUPPORTED_SCHEMA_VERSIONS: ReadonlyArray<number> =
  [1]

/** The `env.KHALA_SYNC_DB` Hyperdrive binding slice this route reads. */
export type KhalaSyncHyperdriveBinding = Readonly<{
  connectionString: string
}>

/** A driver handle the push engine can run on, plus its teardown. */
export type KhalaSyncPushSqlClient = Readonly<{
  sql: SyncSql
  /** Release the underlying connection(s). Always called, even on error. */
  end: () => Promise<void>
}>

export type MakeKhalaSyncPushSqlClient = (
  connectionString: string,
) => Promise<KhalaSyncPushSqlClient>

export type ExecutePushFn = (input: {
  readonly sql: SyncSql
  readonly registry: MutatorRegistry
  readonly userId: string
  readonly request: PushRequest
}) => Promise<PushResponse>

export type KhalaSyncPushDependencies = Readonly<{
  /**
   * Resolve the authenticated caller via the Worker's standard actor auth
   * (`authenticateRequestActor`: browser session or agent bearer token).
   * `undefined` ⇒ 401.
   */
  authenticate: () => Promise<{ readonly userId: string } | undefined>
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  registry: MutatorRegistry
  /**
   * Injectable client factory. Default: dynamic import of `postgres`
   * (postgres.js), Worker-runtime only. Tests inject a fake — no network,
   * no database.
   */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /** Injectable engine seam for route tests. Default: the real engine. */
  executePush?: ExecutePushFn | undefined
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

/**
 * Default transaction-mode-safe postgres.js client factory for the
 * KHALA_SYNC_DB Hyperdrive binding. Exported so other Worker seams that
 * reach Khala Sync Postgres (e.g. the KS-6.1 fleet projection dual-write)
 * reuse the exact same driver discipline instead of re-deriving it.
 *
 * On the Cloud Run monolith this returns a SHARED pool-backed client
 * (reused across requests/statements) via `acquireSharedPostgresClient`; on
 * Cloudflare Workers it falls back to a fresh `max: 1` client. `prepare:
 * false` keeps unnamed statements only (SPEC §4). postgres.js exposes the
 * same tagged-template + `begin` surface as the engine's structural
 * `SyncSql`; the cast is the single deliberate driver seam.
 */
export const defaultMakeKhalaSyncSqlClient: MakeKhalaSyncPushSqlClient = async (
  connectionString,
) => {
  const { sql, end } = await acquireSharedPostgresClient({
    connectionString,
    options: { connect_timeout: 10, prepare: false },
    variant: 'sync',
  })
  return { end, sql: sql as unknown as SyncSql }
}

/**
 * `POST /api/sync/push` — authenticated (session or agent bearer).
 *
 * Success: 200 with the encoded `PushResponse` (`results` in request order
 * + `lastMutationId` watermark). Whole-request failures are typed
 * `SyncError` bodies (see the module doc for the status map).
 */
export const handleKhalaSyncPush = (
  request: Request,
  deps: KhalaSyncPushDependencies,
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
        'Khala Sync push requires an authenticated session or agent token.',
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
    // specific typed code they key their upgrade/refetch behavior on.
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

    let pushRequest: PushRequest
    try {
      pushRequest = decodePushRequest(raw)
    } catch {
      // Decode errors can embed raw request values — never echo them.
      return syncErrorResponse(
        400,
        'invalid_request',
        'Request body failed to decode as a PushRequest.',
        false,
      )
    }

    if (
      !KHALA_SYNC_PUSH_SUPPORTED_SCHEMA_VERSIONS.includes(
        Number(pushRequest.schemaVersion),
      )
    ) {
      return syncErrorResponse(
        400,
        'schema_version_unsupported',
        `Unsupported client schema version ${pushRequest.schemaVersion}; ` +
          `this server supports ${KHALA_SYNC_PUSH_SUPPORTED_SCHEMA_VERSIONS.join(', ')}.`,
        false,
      )
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

    const makeSqlClient = deps.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
    const executePush = deps.executePush ?? executePushEngine

    let client: KhalaSyncPushSqlClient | undefined
    try {
      client = await makeSqlClient(deps.binding.connectionString)
      const response = await executePush({
        registry: deps.registry,
        request: pushRequest,
        sql: client.sql,
        userId: actor.userId,
      })
      return noStoreJsonResponse(encodePushResponse(response))
    } catch (error) {
      if (error instanceof KhalaSyncClientStateMismatchError) {
        // The client group is bound to a DIFFERENT user; it never migrates
        // (SPEC §2.4). Not retryable — the client must re-bootstrap with a
        // fresh client group.
        return syncErrorResponse(
          403,
          'unauthorized_scope',
          'This client group is bound to a different user.',
          false,
        )
      }
      if (error instanceof KhalaSyncStorageError) {
        // Batch aborted mid-way is safe: committed envelopes replay as
        // duplicates on the retry.
        return syncErrorResponse(
          503,
          'storage_unavailable',
          `Khala Sync storage failed (${error.reason}); retry the push.`,
          true,
        )
      }
      return syncErrorResponse(
        500,
        'internal',
        'Khala Sync push failed unexpectedly; retry the push.',
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
