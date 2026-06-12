import { Context, Effect, Layer } from 'effect'

import {
  type ProviderAccountAuthGrantRecord,
  type ProviderAccountAuthGrantRow,
  type ProviderAccountEventRecord,
  type ProviderAccountRecord,
  type ProviderAccountRepository,
  type ProviderAccountRow,
  type ProviderConnectionAttemptRecord,
  type ProviderConnectionAttemptRow,
  toAccountRecord,
  toAttemptRecord,
  toGrantRecord,
} from './provider-account-domain'
import {
  type ProviderAccountError,
  ProviderAccountReloadFailed,
  providerAccountErrorFromUnknown,
} from './provider-account-errors'

export const makeD1ProviderAccountRepository = (
  db: D1Database,
): ProviderAccountRepository => ({
  findAccountByRef: async (userId, providerAccountRef) => {
    const row = await db
      .prepare(
        `SELECT *
         FROM provider_accounts
         WHERE user_id = ?
           AND provider_account_ref = ?
           AND deleted_at IS NULL`,
      )
      .bind(userId, providerAccountRef)
      .first<ProviderAccountRow>()

    return row === null ? undefined : toAccountRecord(row)
  },

  findAccountByProviderAccountRef: async providerAccountRef => {
    const row = await db
      .prepare(
        `SELECT *
         FROM provider_accounts
         WHERE provider_account_ref = ?
           AND deleted_at IS NULL`,
      )
      .bind(providerAccountRef)
      .first<ProviderAccountRow>()

    return row === null ? undefined : toAccountRecord(row)
  },

  findReusableAccount: async userId => {
    const row = await db
      .prepare(
        `SELECT *
         FROM provider_accounts
         WHERE user_id = ?
           AND provider = 'chatgpt_codex'
           AND status IN ('pending', 'expired', 'denied', 'disconnected', 'unhealthy')
           AND deleted_at IS NULL
         ORDER BY updated_at DESC
         LIMIT 1`,
      )
      .bind(userId)
      .first<ProviderAccountRow>()

    return row === null ? undefined : toAccountRecord(row)
  },

  listAccountsForUser: async userId => {
    const rows = await db
      .prepare(
        `SELECT *
         FROM provider_accounts
         WHERE user_id = ?
           AND provider = 'chatgpt_codex'
           AND deleted_at IS NULL
         ORDER BY updated_at DESC
         LIMIT 50`,
      )
      .bind(userId)
      .all<ProviderAccountRow>()

    return rows.results.map(toAccountRecord)
  },

  listPendingAttemptsForUser: async userId => {
    const rows = await db
      .prepare(
        `SELECT *
         FROM provider_account_connection_attempts
         WHERE user_id = ?
           AND provider = 'chatgpt_codex'
           AND status = 'pending'
         ORDER BY created_at DESC
         LIMIT 50`,
      )
      .bind(userId)
      .all<ProviderConnectionAttemptRow>()

    return rows.results.map(toAttemptRecord)
  },

  findAttemptForUser: async (userId, attemptId) => {
    const attemptRow = await db
      .prepare(
        `SELECT *
         FROM provider_account_connection_attempts
         WHERE user_id = ?
           AND id = ?
           AND provider = 'chatgpt_codex'`,
      )
      .bind(userId, attemptId)
      .first<ProviderConnectionAttemptRow>()

    if (attemptRow === null) {
      return undefined
    }

    const accountRow = await db
      .prepare(
        `SELECT *
         FROM provider_accounts
         WHERE id = ?
           AND user_id = ?
           AND provider = 'chatgpt_codex'
           AND deleted_at IS NULL`,
      )
      .bind(attemptRow.provider_account_id, userId)
      .first<ProviderAccountRow>()

    return accountRow === null
      ? undefined
      : {
          account: toAccountRecord(accountRow),
          attempt: toAttemptRecord(attemptRow),
        }
  },

  findAttemptById: async attemptId => {
    const attemptRow = await db
      .prepare(
        `SELECT *
         FROM provider_account_connection_attempts
         WHERE id = ?
           AND provider = 'chatgpt_codex'`,
      )
      .bind(attemptId)
      .first<ProviderConnectionAttemptRow>()

    if (attemptRow === null) {
      return undefined
    }

    const accountRow = await db
      .prepare(
        `SELECT *
         FROM provider_accounts
         WHERE id = ?
           AND provider = 'chatgpt_codex'
           AND deleted_at IS NULL`,
      )
      .bind(attemptRow.provider_account_id)
      .first<ProviderAccountRow>()

    return accountRow === null
      ? undefined
      : {
          account: toAccountRecord(accountRow),
          attempt: toAttemptRecord(attemptRow),
        }
  },

  saveStartedDeviceLogin: async (
    account,
    attempt,
    event,
    accountAlreadyExists,
  ) => {
    const accountStatement = accountAlreadyExists
      ? db
          .prepare(
            `UPDATE provider_accounts
             SET auth_mode = ?,
                 status = ?,
                 health = ?,
                 secret_ref = ?,
                 account_label = ?,
                 plan_type = ?,
                 connected_at = ?,
                 disconnected_at = ?,
                 denied_at = ?,
                 last_status_at = ?,
                 metadata_json = ?,
                 updated_at = ?,
                 deleted_at = NULL
             WHERE id = ?
               AND user_id = ?`,
          )
          .bind(
            account.authMode,
            account.status,
            account.health,
            account.secretRef,
            account.accountLabel,
            account.planType,
            account.connectedAt,
            account.disconnectedAt,
            account.deniedAt,
            account.lastStatusAt,
            account.metadataJson,
            account.updatedAt,
            account.id,
            account.userId,
          )
      : db
          .prepare(
            `INSERT INTO provider_accounts
              (id, user_id, team_id, provider, auth_mode, status, health,
               provider_account_ref, secret_ref, account_label, plan_type,
               connected_at, disconnected_at, denied_at, last_status_at,
               metadata_json, created_at, updated_at, deleted_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            account.id,
            account.userId,
            account.teamId,
            account.provider,
            account.authMode,
            account.status,
            account.health,
            account.providerAccountRef,
            account.secretRef,
            account.accountLabel,
            account.planType,
            account.connectedAt,
            account.disconnectedAt,
            account.deniedAt,
            account.lastStatusAt,
            account.metadataJson,
            account.createdAt,
            account.updatedAt,
            account.deletedAt,
          )

    await db.batch([
      accountStatement,
      db
        .prepare(
          `INSERT INTO provider_account_connection_attempts
            (id, provider_account_id, user_id, team_id, provider, method, source,
             login_ref, verification_url, user_code, status, expires_at,
             completed_at, failed_at, metadata_json, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          attempt.id,
          attempt.providerAccountId,
          attempt.userId,
          attempt.teamId,
          attempt.provider,
          attempt.method,
          attempt.source,
          attempt.loginRef,
          attempt.verificationUrl,
          attempt.userCode,
          attempt.status,
          attempt.expiresAt,
          attempt.completedAt,
          attempt.failedAt,
          attempt.metadataJson,
          attempt.createdAt,
          attempt.updatedAt,
        ),
      db
        .prepare(
          `INSERT INTO provider_account_events
            (id, provider_account_id, auth_grant_id, user_id, team_id, thread_id,
             workroom_id, runner_session_id, kind, summary, source_refs_json,
             evidence_refs_json, target_ref, metadata_json, actor_id, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          event.id,
          event.providerAccountId,
          event.authGrantId,
          event.userId,
          event.teamId,
          event.threadId,
          event.workroomId,
          event.runnerSessionId,
          event.kind,
          event.summary,
          event.sourceRefsJson,
          event.evidenceRefsJson,
          event.targetRef,
          event.metadataJson,
          event.actorId,
          event.createdAt,
        ),
    ])
  },

  recordConnectedAttempt: async (account, attempt, event) => {
    await db.batch([
      db
        .prepare(
          `UPDATE provider_accounts
           SET account_label = ?,
               plan_type = ?,
               connected_at = ?,
               disconnected_at = NULL,
               denied_at = NULL,
               health = 'healthy',
               last_status_at = ?,
               metadata_json = ?,
               secret_ref = ?,
               status = 'connected',
               updated_at = ?
           WHERE id = ?`,
        )
        .bind(
          account.accountLabel,
          account.planType,
          account.connectedAt,
          account.lastStatusAt,
          account.metadataJson,
          account.secretRef,
          account.updatedAt,
          account.id,
        ),
      db
        .prepare(
          `UPDATE provider_account_connection_attempts
           SET completed_at = ?,
               failed_at = NULL,
               login_ref = NULL,
               metadata_json = ?,
               status = 'connected',
               updated_at = ?,
               user_code = NULL,
               verification_url = NULL
           WHERE id = ?`,
        )
        .bind(
          attempt.completedAt,
          attempt.metadataJson,
          attempt.updatedAt,
          attempt.id,
        ),
      db
        .prepare(
          `INSERT INTO provider_account_events
            (id, provider_account_id, auth_grant_id, user_id, team_id, thread_id,
             workroom_id, runner_session_id, kind, summary, source_refs_json,
             evidence_refs_json, target_ref, metadata_json, actor_id, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          event.id,
          event.providerAccountId,
          event.authGrantId,
          event.userId,
          event.teamId,
          event.threadId,
          event.workroomId,
          event.runnerSessionId,
          event.kind,
          event.summary,
          event.sourceRefsJson,
          event.evidenceRefsJson,
          event.targetRef,
          event.metadataJson,
          event.actorId,
          event.createdAt,
        ),
    ])

    const updated = await db
      .prepare(
        `SELECT *
         FROM provider_accounts
         WHERE id = ?`,
      )
      .bind(account.id)
      .first<ProviderAccountRow>()

    if (updated === null) {
      throw new ProviderAccountReloadFailed({
        operation: 'record_connected_attempt',
        message: 'Connected provider account could not be reloaded.',
      })
    }

    return toAccountRecord(updated)
  },

  recordFailedAttempt: async (account, attempt, event) => {
    await db.batch([
      db
        .prepare(
          `UPDATE provider_accounts
           SET denied_at = ?,
               health = 'requires_reauth',
               last_status_at = ?,
               metadata_json = ?,
               status = ?,
               updated_at = ?
           WHERE id = ?`,
        )
        .bind(
          account.deniedAt,
          account.lastStatusAt,
          account.metadataJson,
          account.status,
          account.updatedAt,
          account.id,
        ),
      db
        .prepare(
          `UPDATE provider_account_connection_attempts
           SET failed_at = ?,
               metadata_json = ?,
               status = ?,
               updated_at = ?
           WHERE id = ?`,
        )
        .bind(
          attempt.failedAt,
          attempt.metadataJson,
          attempt.status,
          attempt.updatedAt,
          attempt.id,
        ),
      db
        .prepare(
          `INSERT INTO provider_account_events
            (id, provider_account_id, auth_grant_id, user_id, team_id, thread_id,
             workroom_id, runner_session_id, kind, summary, source_refs_json,
             evidence_refs_json, target_ref, metadata_json, actor_id, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          event.id,
          event.providerAccountId,
          event.authGrantId,
          event.userId,
          event.teamId,
          event.threadId,
          event.workroomId,
          event.runnerSessionId,
          event.kind,
          event.summary,
          event.sourceRefsJson,
          event.evidenceRefsJson,
          event.targetRef,
          event.metadataJson,
          event.actorId,
          event.createdAt,
        ),
    ])

    const updated = await db
      .prepare(
        `SELECT *
         FROM provider_accounts
         WHERE id = ?`,
      )
      .bind(account.id)
      .first<ProviderAccountRow>()

    if (updated === null) {
      throw new ProviderAccountReloadFailed({
        operation: 'record_failed_attempt',
        message: 'Failed provider account could not be reloaded.',
      })
    }

    return toAccountRecord(updated)
  },

  recordAccountHealth: async (providerAccountRef, account, event) => {
    const reauthRequiredReason =
      account.health === 'requires_reauth' &&
      event.summary.includes('token_invalidated')
        ? 'token_invalidated'
        : account.health === 'requires_reauth'
          ? 'requires_reauth'
          : null

    await db.batch([
      db
        .prepare(
          `UPDATE provider_accounts
           SET health = ?,
               last_status_at = ?,
               metadata_json = ?,
               status = ?,
               low_credit_flag = CASE
                 WHEN ? = 'healthy' THEN 0
                 ELSE COALESCE(low_credit_flag, 0)
               END,
               cooldown_until = CASE
                 WHEN ? = 'healthy' THEN NULL
                 ELSE cooldown_until
               END,
               recent_failure_class = CASE
                 WHEN ? = 'healthy' THEN NULL
                 ELSE recent_failure_class
               END,
               reauth_required_reason = CASE
                 WHEN ? = 'healthy' THEN NULL
                 WHEN ? = 'requires_reauth' THEN ?
                 ELSE reauth_required_reason
               END,
               updated_at = ?
           WHERE provider_account_ref = ?
             AND provider = 'chatgpt_codex'
             AND deleted_at IS NULL`,
        )
        .bind(
          account.health,
          account.lastStatusAt,
          account.metadataJson,
          account.status,
          account.health,
          account.health,
          account.health,
          account.health,
          account.health,
          reauthRequiredReason,
          account.updatedAt,
          providerAccountRef,
        ),
      db
        .prepare(
          `INSERT INTO provider_account_events
            (id, provider_account_id, auth_grant_id, user_id, team_id, thread_id,
             workroom_id, runner_session_id, kind, summary, source_refs_json,
             evidence_refs_json, target_ref, metadata_json, actor_id, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          event.id,
          event.providerAccountId,
          event.authGrantId,
          event.userId,
          event.teamId,
          event.threadId,
          event.workroomId,
          event.runnerSessionId,
          event.kind,
          event.summary,
          event.sourceRefsJson,
          event.evidenceRefsJson,
          event.targetRef,
          event.metadataJson,
          event.actorId,
          event.createdAt,
        ),
    ])

    const updated = await db
      .prepare(
        `SELECT *
         FROM provider_accounts
         WHERE provider_account_ref = ?
           AND provider = 'chatgpt_codex'
           AND deleted_at IS NULL`,
      )
      .bind(providerAccountRef)
      .first<ProviderAccountRow>()

    return updated === null ? undefined : toAccountRecord(updated)
  },

  createAuthGrant: async (grant, event) => {
    await db.batch([
      db
        .prepare(
          `INSERT INTO provider_account_auth_grants
            (id, provider_account_id, user_id, team_id, thread_id, workroom_id,
             runner_session_id, provider, provider_account_ref, provider_secret_ref,
             grant_ref, status, requested_action, metadata_json, created_at,
             updated_at, expires_at, used_at, revoked_at, failed_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          grant.id,
          grant.providerAccountId,
          grant.userId,
          grant.teamId,
          grant.threadId,
          grant.workroomId,
          grant.runnerSessionId,
          grant.provider,
          grant.providerAccountRef,
          grant.providerSecretRef,
          grant.grantRef,
          grant.status,
          grant.requestedAction,
          grant.metadataJson,
          grant.createdAt,
          grant.updatedAt,
          grant.expiresAt,
          grant.usedAt,
          grant.revokedAt,
          grant.failedAt,
        ),
      db
        .prepare(
          `INSERT INTO provider_account_events
            (id, provider_account_id, auth_grant_id, user_id, team_id, thread_id,
             workroom_id, runner_session_id, kind, summary, source_refs_json,
             evidence_refs_json, target_ref, metadata_json, actor_id, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          event.id,
          event.providerAccountId,
          grant.id,
          event.userId,
          event.teamId,
          event.threadId,
          event.workroomId,
          event.runnerSessionId,
          event.kind,
          event.summary,
          event.sourceRefsJson,
          event.evidenceRefsJson,
          event.targetRef,
          event.metadataJson,
          event.actorId,
          event.createdAt,
        ),
    ])

    return grant
  },

  findGrantByRef: async grantRef => {
    const row = await db
      .prepare(
        `SELECT *
         FROM provider_account_auth_grants
         WHERE grant_ref = ?`,
      )
      .bind(grantRef)
      .first<ProviderAccountAuthGrantRow>()

    return row === null ? undefined : toGrantRecord(row)
  },

  markGrantUsed: async (grant, event) => {
    await db.batch([
      db
        .prepare(
          `UPDATE provider_account_auth_grants
           SET status = 'used',
               used_at = ?,
               updated_at = ?
           WHERE id = ?
             AND status = 'issued'`,
        )
        .bind(grant.usedAt, grant.updatedAt, grant.id),
      db
        .prepare(
          `INSERT INTO provider_account_events
            (id, provider_account_id, auth_grant_id, user_id, team_id, thread_id,
             workroom_id, runner_session_id, kind, summary, source_refs_json,
             evidence_refs_json, target_ref, metadata_json, actor_id, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          event.id,
          event.providerAccountId,
          grant.id,
          event.userId,
          event.teamId,
          event.threadId,
          event.workroomId,
          event.runnerSessionId,
          event.kind,
          event.summary,
          event.sourceRefsJson,
          event.evidenceRefsJson,
          event.targetRef,
          event.metadataJson,
          event.actorId,
          event.createdAt,
        ),
    ])

    const updated = await db
      .prepare(
        `SELECT *
         FROM provider_account_auth_grants
         WHERE id = ?`,
      )
      .bind(grant.id)
      .first<ProviderAccountAuthGrantRow>()

    if (updated === null) {
      throw new ProviderAccountReloadFailed({
        operation: 'mark_grant_used',
        message: 'Used provider-account grant could not be reloaded.',
      })
    }

    return toGrantRecord(updated)
  },

  disconnectAccount: async (
    userId,
    providerAccountRef,
    now,
    metadataJson,
    event,
  ) => {
    const account = await db
      .prepare(
        `SELECT *
         FROM provider_accounts
         WHERE user_id = ?
           AND provider_account_ref = ?
           AND provider = 'chatgpt_codex'
           AND deleted_at IS NULL`,
      )
      .bind(userId, providerAccountRef)
      .first<ProviderAccountRow>()

    if (account === null) {
      return undefined
    }

    await db.batch([
      db
        .prepare(
          `UPDATE provider_accounts
           SET secret_ref = NULL,
               status = 'disconnected',
               health = 'requires_reauth',
               disconnected_at = ?,
               last_status_at = ?,
               metadata_json = ?,
               updated_at = ?
           WHERE id = ?
             AND user_id = ?`,
        )
        .bind(now, now, metadataJson, now, account.id, userId),
      db
        .prepare(
          `UPDATE provider_account_auth_grants
           SET status = 'revoked',
               revoked_at = ?,
               updated_at = ?
           WHERE provider_account_id = ?
             AND status = 'issued'`,
        )
        .bind(now, now, account.id),
      db
        .prepare(
          `INSERT INTO provider_account_events
            (id, provider_account_id, auth_grant_id, user_id, team_id, thread_id,
             workroom_id, runner_session_id, kind, summary, source_refs_json,
             evidence_refs_json, target_ref, metadata_json, actor_id, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          event.id,
          account.id,
          event.authGrantId,
          event.userId,
          account.team_id,
          event.threadId,
          event.workroomId,
          event.runnerSessionId,
          event.kind,
          event.summary,
          event.sourceRefsJson,
          event.evidenceRefsJson,
          event.targetRef,
          event.metadataJson,
          event.actorId,
          event.createdAt,
        ),
    ])

    const updated = await db
      .prepare(
        `SELECT *
         FROM provider_accounts
         WHERE id = ?`,
      )
      .bind(account.id)
      .first<ProviderAccountRow>()

    return updated === null ? undefined : toAccountRecord(updated)
  },
})

export type ProviderAccountRepositoryServiceShape = Readonly<{
  findAccountByRef: (
    userId: string,
    providerAccountRef: string,
  ) => Effect.Effect<ProviderAccountRecord | undefined, ProviderAccountError>
  findAccountByProviderAccountRef: (
    providerAccountRef: string,
  ) => Effect.Effect<ProviderAccountRecord | undefined, ProviderAccountError>
  findReusableAccount: (
    userId: string,
  ) => Effect.Effect<ProviderAccountRecord | undefined, ProviderAccountError>
  listAccountsForUser: (
    userId: string,
  ) => Effect.Effect<ReadonlyArray<ProviderAccountRecord>, ProviderAccountError>
  listPendingAttemptsForUser: (
    userId: string,
  ) => Effect.Effect<
    ReadonlyArray<ProviderConnectionAttemptRecord>,
    ProviderAccountError
  >
  findAttemptForUser: (
    userId: string,
    attemptId: string,
  ) => Effect.Effect<
    | Readonly<{
        account: ProviderAccountRecord
        attempt: ProviderConnectionAttemptRecord
      }>
    | undefined,
    ProviderAccountError
  >
  findAttemptById: (attemptId: string) => Effect.Effect<
    | Readonly<{
        account: ProviderAccountRecord
        attempt: ProviderConnectionAttemptRecord
      }>
    | undefined,
    ProviderAccountError
  >
  saveStartedDeviceLogin: (
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
    accountAlreadyExists: boolean,
  ) => Effect.Effect<void, ProviderAccountError>
  recordConnectedAttempt: (
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
  ) => Effect.Effect<ProviderAccountRecord, ProviderAccountError>
  recordFailedAttempt: (
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
  ) => Effect.Effect<ProviderAccountRecord, ProviderAccountError>
  recordAccountHealth: (
    providerAccountRef: string,
    account: ProviderAccountRecord,
    event: ProviderAccountEventRecord,
  ) => Effect.Effect<ProviderAccountRecord | undefined, ProviderAccountError>
  createAuthGrant: (
    grant: ProviderAccountAuthGrantRecord,
    event: ProviderAccountEventRecord,
  ) => Effect.Effect<ProviderAccountAuthGrantRecord, ProviderAccountError>
  findGrantByRef: (
    grantRef: string,
  ) => Effect.Effect<
    ProviderAccountAuthGrantRecord | undefined,
    ProviderAccountError
  >
  markGrantUsed: (
    grant: ProviderAccountAuthGrantRecord,
    event: ProviderAccountEventRecord,
  ) => Effect.Effect<ProviderAccountAuthGrantRecord, ProviderAccountError>
  disconnectAccount: (
    userId: string,
    providerAccountRef: string,
    now: string,
    metadataJson: string,
    event: ProviderAccountEventRecord,
  ) => Effect.Effect<ProviderAccountRecord | undefined, ProviderAccountError>
}>

export class ProviderAccountRepositoryService extends Context.Service<
  ProviderAccountRepositoryService,
  ProviderAccountRepositoryServiceShape
>()('openagents/ProviderAccountRepositoryService') {}

const repositoryEffect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, ProviderAccountError> =>
  Effect.tryPromise({
    try: run,
    catch: error => providerAccountErrorFromUnknown(operation, error),
  })

export const makeProviderAccountRepositoryService = (
  repository: ProviderAccountRepository,
): ProviderAccountRepositoryServiceShape => ({
  findAccountByRef: (userId, providerAccountRef) =>
    repositoryEffect('find_account_by_ref', () =>
      repository.findAccountByRef(userId, providerAccountRef),
    ),
  findAccountByProviderAccountRef: providerAccountRef =>
    repositoryEffect('find_account_by_provider_account_ref', () =>
      repository.findAccountByProviderAccountRef(providerAccountRef),
    ),
  findReusableAccount: userId =>
    repositoryEffect('find_reusable_account', () =>
      repository.findReusableAccount(userId),
    ),
  listAccountsForUser: userId =>
    repositoryEffect('list_accounts_for_user', () =>
      repository.listAccountsForUser(userId),
    ),
  listPendingAttemptsForUser: userId =>
    repositoryEffect('list_pending_attempts_for_user', () =>
      repository.listPendingAttemptsForUser(userId),
    ),
  findAttemptForUser: (userId, attemptId) =>
    repositoryEffect('find_attempt_for_user', () =>
      repository.findAttemptForUser(userId, attemptId),
    ),
  findAttemptById: attemptId =>
    repositoryEffect('find_attempt_by_id', () =>
      repository.findAttemptById(attemptId),
    ),
  saveStartedDeviceLogin: (account, attempt, event, accountAlreadyExists) =>
    repositoryEffect('save_started_device_login', () =>
      repository.saveStartedDeviceLogin(
        account,
        attempt,
        event,
        accountAlreadyExists,
      ),
    ),
  recordConnectedAttempt: (account, attempt, event) =>
    repositoryEffect('record_connected_attempt', () =>
      repository.recordConnectedAttempt(account, attempt, event),
    ),
  recordFailedAttempt: (account, attempt, event) =>
    repositoryEffect('record_failed_attempt', () =>
      repository.recordFailedAttempt(account, attempt, event),
    ),
  recordAccountHealth: (providerAccountRef, account, event) =>
    repositoryEffect('record_account_health', () =>
      repository.recordAccountHealth(providerAccountRef, account, event),
    ),
  createAuthGrant: (grant, event) =>
    repositoryEffect('create_auth_grant', () =>
      repository.createAuthGrant(grant, event),
    ),
  findGrantByRef: grantRef =>
    repositoryEffect('find_grant_by_ref', () =>
      repository.findGrantByRef(grantRef),
    ),
  markGrantUsed: (grant, event) =>
    repositoryEffect('mark_grant_used', () =>
      repository.markGrantUsed(grant, event),
    ),
  disconnectAccount: (userId, providerAccountRef, now, metadataJson, event) =>
    repositoryEffect('disconnect_account', () =>
      repository.disconnectAccount(
        userId,
        providerAccountRef,
        now,
        metadataJson,
        event,
      ),
    ),
})

export const makeProviderAccountRepositoryLayer = (
  repository: ProviderAccountRepository,
) =>
  Layer.succeed(
    ProviderAccountRepositoryService,
    makeProviderAccountRepositoryService(repository),
  )

export const makeD1ProviderAccountRepositoryLayer = (db: D1Database) =>
  makeProviderAccountRepositoryLayer(makeD1ProviderAccountRepository(db))
