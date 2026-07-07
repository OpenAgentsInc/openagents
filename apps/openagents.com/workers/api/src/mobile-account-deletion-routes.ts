// `DELETE /api/mobile/account` — Khala Mobile account deletion (MM-I2b, #8502).
//
// App Review 5.1.1(v) requires an in-app account deletion path. This route is
// intentionally mobile-bearer-only: it reuses the same OpenAuth bearer boundary
// as `/api/mobile/session`, deletes owner-scoped Khala Sync data, removes local
// GitHub/session/push/credit state, records a short-lived deletion receipt for
// safe retries with the same bearer, then revokes that bearer.

import type { StorageAdapter } from '@openauthjs/openauth/storage/storage'
import { Effect, Schema as S } from 'effect'

import { personalScope } from '@openagentsinc/khala-sync'
import type { SyncSql } from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import type { VerifiedSession } from './auth/session'
import {
  hasMobileAccountDeletionReceipt,
  recordMobileAccountDeletionReceipt,
  revokeMobileAccessToken,
  revokeOpenAuthRefreshToken,
  type MobileAccessRevocationStore,
} from './auth/mobile-session'
import { methodNotAllowed, noStoreJsonResponse, unauthorized } from './http/responses'
import { optionalString, readJsonObject } from './json-boundary'
import { agentRefForUser } from './inference/usd-credit-bridge'
import type { IdentityDb } from './identity-db'
import { readAgentBalance, runLedgerStatements } from './payments-ledger'
import type { PaymentsLedgerDb } from './payments-ledger-db'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

export const MOBILE_ACCOUNT_PATH = '/api/mobile/account'
export const MOBILE_ACCOUNT_ROUTE_REF = 'route.khala_mobile.account_delete.v0_1'

export class MobileAccountDeletionSyncError extends S.TaggedErrorClass<MobileAccountDeletionSyncError>()(
  'MobileAccountDeletionSyncError',
  {
    cause: S.Defect,
    reason: S.Union([
      S.Literal('binding_missing'),
      S.Literal('storage_unavailable'),
    ]),
  },
) {}

export type MobileAccountDeletionD1Outcome = Readonly<{
  forfeitedBalanceMsat: number
  githubConnectionsDisconnected: number
  githubWriteGrantsRevoked: number
  pushDeviceTokensRemoved: number
  userRowsMarkedDeleted: number
}>

export type KhalaSyncAccountDeletionOutcome = Readonly<{
  clientGroupsRemoved: number
  scopesRemoved: number
  threadScopesRemoved: number
}>

export type MobileAccountDeletionDependencies<Bindings, User = unknown> = Readonly<{
  authStorage: (env: Bindings) => MobileAccessRevocationStore
  db: (env: Bindings) => D1Database
  /** CFG-4 (#8519): the Postgres-authoritative credits ledger — the
   * `agent_balances` forfeiture read + zeroing run here, never on D1. */
  ledgerDb: (env: Bindings) => PaymentsLedgerDb
  /** CFG-4 Domain 2 (#8519): the Postgres-authoritative identity handle —
   * the `users`/`auth_identities` disable runs here, never on D1. */
  identityDb: (env: Bindings) => IdentityDb
  khalaSyncBinding: (env: Bindings) => KhalaSyncHyperdriveBinding | undefined
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  nowIso?: () => string
  openAuthStorage: (env: Bindings) => StorageAdapter
  readBearerToken: (request: Request) => string | undefined
  requireUserBearerSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<VerifiedSession<User> | undefined>
  userIdFromSession: (session: VerifiedSession<User>) => string
  deleteKhalaSyncAccountData?: (
    binding: KhalaSyncHyperdriveBinding | undefined,
    userId: string,
  ) => Promise<KhalaSyncAccountDeletionOutcome>
}>

const resultChanges = (result: D1Result): number => result.meta.changes ?? 0

// CFG-3 (#8518): OpenAuth issuer state lives on the owned KvStore now, so
// subject-row cleanup goes through the injected `StorageAdapter` (scan +
// remove under each per-subject prefix) instead of a D1 DELETE. This also
// fixes the previous D1 pattern's separator mismatch: real OpenAuth keys are
// `joinKey`-joined (0x1f), while the old LIKE patterns assumed ':'.
const deleteOpenAuthSubjectEntries = async (
  storage: StorageAdapter,
  userId: string,
): Promise<number> => {
  let removed = 0

  for (const scope of ['refresh', 'access', 'subject', 'user']) {
    for await (const [key] of storage.scan([`oauth:${scope}`, userId])) {
      await storage.remove(key)
      removed += 1
    }
  }

  return removed
}

export const deleteMobileAccountD1Data = async (
  db: D1Database,
  ledgerDb: PaymentsLedgerDb,
  identityDb: IdentityDb,
  input: Readonly<{ nowIso: string; userId: string }>,
): Promise<MobileAccountDeletionD1Outcome> => {
  const accountRef = agentRefForUser(input.userId)
  const balance = await readAgentBalance(ledgerDb, accountRef)
  const forfeitedBalanceMsat = balance?.balanceMsat ?? 0

  // CFG-4 (#8519) NON-ATOMIC SEAM: the credits forfeiture (`agent_balances`,
  // Postgres), the push-token removal (`push_device_tokens`, Postgres since
  // the Domain 4 hard cut), the GitHub anonymization (D1), and — since the
  // Domain 2 hard cut — the `users`/`auth_identities` disable (Postgres
  // identity handle) can no longer share one atomic batch. Every side is an
  // idempotent zero-out/UPDATE/DELETE, and the mobile deletion receipt (the thing
  // that makes a retry a no-op) is only recorded by the route AFTER all
  // sides succeed, so a crash in between heals on retry: the caller re-runs
  // the whole deletion with the same bearer. The credits zeroing runs
  // FIRST, while the caller's session is guaranteed still verifiable; the
  // identity disable runs LAST because it is what marks the user deleted.
  await runLedgerStatements(ledgerDb, [
    {
      params: [input.nowIso, accountRef],
      sql: `UPDATE agent_balances
            SET balance_msat = 0,
                held_msat = 0,
                usd_credit_msat = 0,
                updated_at = ?
          WHERE actor_ref = ?`,
    },
  ])

  // CFG-4 Domain 4 (#8519): the push registry moved to Postgres, so this
  // DELETE runs on the ledger handle (same khala_sync database as the
  // credits/identity handles). RETURNING gives the removed-row count.
  const pushRows = await ledgerDb.query(
    `DELETE FROM push_device_tokens WHERE user_id = ? RETURNING user_id`,
    [input.userId],
  )

  const results = await db.batch([
    db
      .prepare(
        `UPDATE github_write_auth_grants
            SET status = 'revoked',
                revoked_at = COALESCE(revoked_at, ?),
                updated_at = ?
          WHERE user_id = ?
            AND status IN ('issued', 'expired', 'failed')`,
      )
      .bind(input.nowIso, input.nowIso, input.userId),
    db
      .prepare(
        `UPDATE github_write_connections
            SET status = 'disconnected',
                health = 'requires_reauth',
                disconnected_at = COALESCE(disconnected_at, ?),
                deleted_at = COALESCE(deleted_at, ?),
                updated_at = ?
          WHERE user_id = ?`,
      )
      .bind(input.nowIso, input.nowIso, input.nowIso, input.userId),
  ])

  // CFG-4 Domain 2 (#8519): Postgres-authoritative identity disable.
  // RETURNING gives the touched-row counts on both engines.
  const identityRows = await identityDb.query(
    `UPDATE auth_identities
        SET deleted_at = COALESCE(deleted_at, ?),
            updated_at = ?
      WHERE user_id = ?
      RETURNING id`,
    [input.nowIso, input.nowIso, input.userId],
  )
  const userRows = await identityDb.query(
    `UPDATE users
        SET status = 'disabled',
            deleted_at = COALESCE(deleted_at, ?),
            updated_at = ?
      WHERE id = ?
      RETURNING id`,
    [input.nowIso, input.nowIso, input.userId],
  )

  return {
    forfeitedBalanceMsat,
    githubConnectionsDisconnected: resultChanges(results[1]!),
    githubWriteGrantsRevoked: resultChanges(results[0]!),
    pushDeviceTokensRemoved: pushRows.length,
    userRowsMarkedDeleted: identityRows.length + userRows.length,
  }
}

const unique = <A>(values: ReadonlyArray<A>): ReadonlyArray<A> => [
  ...new Set(values),
]

type ThreadRow = Readonly<{ thread_id: string }>
type ClientGroupRow = Readonly<{ client_group_id: string }>

export const deleteKhalaSyncAccountDataWithSql = async (
  sql: SyncSql,
  userId: string,
): Promise<KhalaSyncAccountDeletionOutcome> =>
  sql.begin(async tx => {
    const threadRows = (await tx`
      SELECT thread_id FROM khala_sync_chat_threads WHERE owner_user_id = ${userId}
      UNION
      SELECT thread_id FROM khala_sync_runtime_turns WHERE owner_user_id = ${userId}
      UNION
      SELECT thread_id FROM khala_sync_runtime_control_intents WHERE owner_user_id = ${userId}
      UNION
      SELECT thread_id FROM khala_sync_runtime_events WHERE owner_user_id = ${userId}
    `) as ReadonlyArray<ThreadRow>
    const clientRows = (await tx`
      SELECT client_group_id FROM khala_sync_client_state WHERE user_id = ${userId}
    `) as ReadonlyArray<ClientGroupRow>

    const threadIds = unique(threadRows.map(row => row.thread_id))
    const scopes = unique([
      String(personalScope(userId)),
      ...threadIds.map(threadId => `scope.thread.${threadId}`),
    ])
    const clientGroupIds = unique(clientRows.map(row => row.client_group_id))

    await tx`DELETE FROM khala_sync_runtime_events WHERE owner_user_id = ${userId}`
    await tx`DELETE FROM khala_sync_runtime_control_intents WHERE owner_user_id = ${userId}`
    await tx`DELETE FROM khala_sync_runtime_turns WHERE owner_user_id = ${userId}`
    if (threadIds.length > 0) {
      await tx`DELETE FROM khala_sync_chat_messages WHERE author_user_id = ${userId} OR thread_id = ANY(${threadIds}::text[])`
    } else {
      await tx`DELETE FROM khala_sync_chat_messages WHERE author_user_id = ${userId}`
    }
    await tx`DELETE FROM khala_sync_chat_threads WHERE owner_user_id = ${userId}`

    await tx`DELETE FROM khala_sync_mutations WHERE scope = ANY(${scopes}::text[])`
    await tx`DELETE FROM khala_sync_client_state WHERE user_id = ${userId}`
    await tx`DELETE FROM khala_sync_changelog WHERE scope = ANY(${scopes}::text[])`
    await tx`DELETE FROM khala_sync_cvrs WHERE scope = ANY(${scopes}::text[])`
    await tx`DELETE FROM khala_sync_capture_checkpoints WHERE scope = ANY(${scopes}::text[])`
    await tx`DELETE FROM khala_sync_scope_owners WHERE owner_user_id = ${userId} OR scope = ANY(${scopes}::text[])`
    await tx`DELETE FROM khala_sync_scopes WHERE scope = ANY(${scopes}::text[])`

    if (clientGroupIds.length > 0) {
      await tx`DELETE FROM khala_sync_mutations WHERE client_group_id = ANY(${clientGroupIds}::text[])`
      await tx`DELETE FROM khala_sync_client_state WHERE client_group_id = ANY(${clientGroupIds}::text[])`
      await tx`DELETE FROM khala_sync_cvrs WHERE client_group_id = ANY(${clientGroupIds}::text[])`
    }

    return {
      clientGroupsRemoved: clientGroupIds.length,
      scopesRemoved: scopes.length,
      threadScopesRemoved: threadIds.length,
    }
  })

export const deleteKhalaSyncAccountData = async (
  binding: KhalaSyncHyperdriveBinding | undefined,
  userId: string,
  makeSqlClient: MakeKhalaSyncPushSqlClient = defaultMakeKhalaSyncSqlClient,
): Promise<KhalaSyncAccountDeletionOutcome> => {
  if (
    binding === undefined ||
    typeof binding.connectionString !== 'string' ||
    binding.connectionString.trim() === ''
  ) {
    throw new MobileAccountDeletionSyncError({
      cause: 'KHALA_SYNC_DB binding is not configured',
      reason: 'binding_missing',
    })
  }

  let client: Awaited<ReturnType<MakeKhalaSyncPushSqlClient>>
  try {
    client = await makeSqlClient(binding.connectionString)
  } catch (error) {
    throw new MobileAccountDeletionSyncError({
      cause: error,
      reason: 'storage_unavailable',
    })
  }

  try {
    return await deleteKhalaSyncAccountDataWithSql(client.sql, userId)
  } catch (error) {
    throw new MobileAccountDeletionSyncError({
      cause: error,
      reason: 'storage_unavailable',
    })
  } finally {
    await client.end()
  }
}

const alreadyDeletedResponse = (): HttpResponse =>
  noStoreJsonResponse(
    {
      alreadyDeleted: true,
      deleted: true,
      ok: true,
    },
    { status: 200 },
  )

export const handleMobileAccountDeletionRequest = <Bindings, User>(
  dependencies: MobileAccountDeletionDependencies<Bindings, User>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method !== 'DELETE') return methodNotAllowed(['DELETE'])

    const accessToken = dependencies.readBearerToken(request)
    if (accessToken === undefined) return unauthorized()

    const session = await dependencies.requireUserBearerSession(request, env, ctx)
    if (session === undefined) {
      return (await hasMobileAccountDeletionReceipt(
        dependencies.authStorage(env),
        accessToken,
      ))
        ? alreadyDeletedResponse()
        : unauthorized()
    }

    let body: Record<string, unknown>
    try {
      body = await readJsonObject(request)
    } catch {
      return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
    }

    const userId = dependencies.userIdFromSession(session)
    const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()

    let syncOutcome: KhalaSyncAccountDeletionOutcome
    try {
      syncOutcome = await (dependencies.deleteKhalaSyncAccountData ?? ((binding, ownerUserId) =>
        deleteKhalaSyncAccountData(
          binding,
          ownerUserId,
          dependencies.makeSqlClient,
        )))(
          dependencies.khalaSyncBinding(env),
          userId,
        )
    } catch (error) {
      if (error instanceof MobileAccountDeletionSyncError) {
        return noStoreJsonResponse(
          {
            error: 'storage_unavailable',
            messageSafe:
              'Account deletion could not reach Khala Sync storage. Please retry.',
            retryable: true,
          },
          { status: 503 },
        )
      }
      return noStoreJsonResponse({ error: 'internal_server_error' }, { status: 500 })
    }

    const d1Outcome = await deleteMobileAccountD1Data(
      dependencies.db(env),
      dependencies.ledgerDb(env),
      dependencies.identityDb(env),
      {
        nowIso,
        userId,
      },
    )
    const openAuthRowsRemoved = await deleteOpenAuthSubjectEntries(
      dependencies.openAuthStorage(env),
      userId,
    )
    const refreshRevoked = await revokeOpenAuthRefreshToken(
      dependencies.openAuthStorage(env),
      optionalString(body.refreshToken),
    )

    await recordMobileAccountDeletionReceipt(
      dependencies.authStorage(env),
      accessToken,
      userId,
    )
    await revokeMobileAccessToken(dependencies.authStorage(env), accessToken)

    return noStoreJsonResponse(
      {
        cleanup: {
          credits: {
            forfeitedBalanceMsat: d1Outcome.forfeitedBalanceMsat,
          },
          github: {
            connectionsDisconnected: d1Outcome.githubConnectionsDisconnected,
            writeGrantsRevoked: d1Outcome.githubWriteGrantsRevoked,
          },
          openAuth: {
            refreshRevoked,
            storageRowsRemoved: openAuthRowsRemoved,
          },
          push: {
            deviceTokensRemoved: d1Outcome.pushDeviceTokensRemoved,
          },
          sync: syncOutcome,
        },
        deleted: true,
        ok: true,
      },
      { status: 200 },
    )
  })
