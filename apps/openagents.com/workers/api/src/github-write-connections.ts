import { Context, Effect, Layer, Schema as S } from 'effect'

import { parseJsonStringArray } from './json-boundary'
import {
  compactRandomId,
  currentDate,
  isoTimestampAfter,
} from './runtime-primitives'

export const GITHUB_WRITE_REQUIRED_SCOPES = ['repo', 'workflow'] as const

const ATTEMPT_TTL_MS = 1000 * 60 * 10
const GRANT_TTL_MS = 1000 * 60 * 60 * 2

export class GitHubWriteMissingConnection extends S.TaggedErrorClass<GitHubWriteMissingConnection>()(
  'GitHubWriteMissingConnection',
  {
    message: S.String,
  },
) {}

export class GitHubWriteGrantExpired extends S.TaggedErrorClass<GitHubWriteGrantExpired>()(
  'GitHubWriteGrantExpired',
  {
    message: S.String,
  },
) {}

export class GitHubWriteGrantNotIssued extends S.TaggedErrorClass<GitHubWriteGrantNotIssued>()(
  'GitHubWriteGrantNotIssued',
  {
    message: S.String,
  },
) {}

export class GitHubWriteGrantRunnerSessionMismatch extends S.TaggedErrorClass<GitHubWriteGrantRunnerSessionMismatch>()(
  'GitHubWriteGrantRunnerSessionMismatch',
  {
    message: S.String,
  },
) {}

export class GitHubWriteConnectionNotUsable extends S.TaggedErrorClass<GitHubWriteConnectionNotUsable>()(
  'GitHubWriteConnectionNotUsable',
  {
    message: S.String,
  },
) {}

export class GitHubWriteCallbackMismatch extends S.TaggedErrorClass<GitHubWriteCallbackMismatch>()(
  'GitHubWriteCallbackMismatch',
  {
    message: S.String,
  },
) {}

export class GitHubWriteApiFailure extends S.TaggedErrorClass<GitHubWriteApiFailure>()(
  'GitHubWriteApiFailure',
  {
    message: S.String,
    operation: S.String,
    status: S.Number,
  },
) {}

export class GitHubWriteTokenStorageFailure extends S.TaggedErrorClass<GitHubWriteTokenStorageFailure>()(
  'GitHubWriteTokenStorageFailure',
  {
    message: S.String,
    operation: S.String,
  },
) {}

export class GitHubWritePermissionFailure extends S.TaggedErrorClass<GitHubWritePermissionFailure>()(
  'GitHubWritePermissionFailure',
  {
    message: S.String,
  },
) {}

export class GitHubWriteReloadFailure extends S.TaggedErrorClass<GitHubWriteReloadFailure>()(
  'GitHubWriteReloadFailure',
  {
    message: S.String,
    operation: S.String,
  },
) {}

export class GitHubWriteRepositoryFailure extends S.TaggedErrorClass<GitHubWriteRepositoryFailure>()(
  'GitHubWriteRepositoryFailure',
  {
    message: S.String,
    operation: S.String,
  },
) {}

export const GitHubWriteError = S.Union([
  GitHubWriteMissingConnection,
  GitHubWriteGrantExpired,
  GitHubWriteGrantNotIssued,
  GitHubWriteGrantRunnerSessionMismatch,
  GitHubWriteConnectionNotUsable,
  GitHubWriteCallbackMismatch,
  GitHubWriteApiFailure,
  GitHubWriteTokenStorageFailure,
  GitHubWritePermissionFailure,
  GitHubWriteReloadFailure,
  GitHubWriteRepositoryFailure,
])
export type GitHubWriteError = typeof GitHubWriteError.Type

const gitHubWriteErrorTags = new Set([
  'GitHubWriteMissingConnection',
  'GitHubWriteGrantExpired',
  'GitHubWriteGrantNotIssued',
  'GitHubWriteGrantRunnerSessionMismatch',
  'GitHubWriteConnectionNotUsable',
  'GitHubWriteCallbackMismatch',
  'GitHubWriteApiFailure',
  'GitHubWriteTokenStorageFailure',
  'GitHubWritePermissionFailure',
  'GitHubWriteReloadFailure',
  'GitHubWriteRepositoryFailure',
])

export const isGitHubWriteError = (error: unknown): error is GitHubWriteError =>
  typeof error === 'object' &&
  error !== null &&
  '_tag' in error &&
  typeof error._tag === 'string' &&
  gitHubWriteErrorTags.has(error._tag)

export const gitHubWriteErrorFromUnknown = (
  operation: string,
  error: unknown,
): GitHubWriteError =>
  isGitHubWriteError(error)
    ? error
    : new GitHubWriteRepositoryFailure({
        operation,
        message: error instanceof Error ? error.message : String(error),
      })

export type GitHubWriteConnectionStatus =
  | 'connected'
  | 'disconnected'
  | 'unhealthy'

export type GitHubWriteConnectionHealth =
  | 'healthy'
  | 'unhealthy'
  | 'requires_reauth'

export type GitHubWriteConnectionAttemptStatus =
  | 'pending'
  | 'connected'
  | 'expired'
  | 'denied'
  | 'failed'

export type GitHubWriteGrantStatus =
  | 'issued'
  | 'used'
  | 'expired'
  | 'revoked'
  | 'failed'

type IdFactory = (prefix: string) => string

export type GitHubWriteRuntime = Readonly<{
  makeId: IdFactory
  now: () => Date
}>

export const systemGitHubWriteRuntime: GitHubWriteRuntime = {
  makeId: compactRandomId,
  now: currentDate,
}

export type GitHubWriteConnectionRecord = Readonly<{
  id: string
  userId: string
  githubId: string
  githubLogin: string
  connectionRef: string
  secretRef: string | null
  scopes: ReadonlyArray<string>
  status: GitHubWriteConnectionStatus
  health: GitHubWriteConnectionHealth
  connectedAt: string | null
  disconnectedAt: string | null
  lastStatusAt: string
  metadataJson: string | null
  createdAt: string
  updatedAt: string
  deletedAt: string | null
}>

export type GitHubWriteConnectionAttemptRecord = Readonly<{
  id: string
  userId: string
  state: string
  expectedGithubId: string
  expectedGithubLogin: string
  redirectAfter: string | null
  scopes: ReadonlyArray<string>
  status: GitHubWriteConnectionAttemptStatus
  expiresAt: string
  completedAt: string | null
  failedAt: string | null
  failureReason: string | null
  createdAt: string
  updatedAt: string
}>

export type GitHubWriteAuthGrantRecord = Readonly<{
  id: string
  connectionId: string
  userId: string
  runnerSessionId: string | null
  connectionRef: string
  secretRef: string
  grantRef: string
  status: GitHubWriteGrantStatus
  requestedAction: string | null
  metadataJson: string | null
  createdAt: string
  updatedAt: string
  expiresAt: string
  usedAt: string | null
  revokedAt: string | null
  failedAt: string | null
}>

export type PublicGitHubWriteConnection = Readonly<{
  id: string
  githubId: string
  githubLogin: string
  connectionRef: string
  scopes: ReadonlyArray<string>
  status: GitHubWriteConnectionStatus
  health: GitHubWriteConnectionHealth
  hasSecretRef: boolean
  connectedAt?: string | undefined
  disconnectedAt?: string | undefined
  lastStatusAt: string
  createdAt: string
  updatedAt: string
}>

export type PublicGitHubWriteConnectionAttempt = Readonly<{
  id: string
  status: GitHubWriteConnectionAttemptStatus
  scopes: ReadonlyArray<string>
  expiresAt: string
  createdAt: string
  updatedAt: string
}>

export type GitHubWriteConnectionBundle = Readonly<{
  attempts: ReadonlyArray<PublicGitHubWriteConnectionAttempt>
  connections: ReadonlyArray<PublicGitHubWriteConnection>
}>

export type PublicGitHubWriteGrant = Readonly<{
  id: string
  connectionRef: string
  grantRef: string
  status: GitHubWriteGrantStatus
  requestedAction?: string | undefined
  runnerSessionId?: string | undefined
  expiresAt: string
  createdAt: string
  updatedAt: string
}>

export type ResolvedGitHubWriteGrant = Readonly<{
  connectionRef: string
  expiresAt: string
  githubLogin: string
  grantRef: string
  requestedAction?: string | undefined
  runnerSessionId?: string | undefined
  scopes: ReadonlyArray<string>
  secretRef: string
  status: 'used'
}>

export type GitHubWriteRepository = Readonly<{
  createAttempt: (
    attempt: GitHubWriteConnectionAttemptRecord,
  ) => Promise<GitHubWriteConnectionAttemptRecord>
  findAttemptByState: (
    state: string,
  ) => Promise<GitHubWriteConnectionAttemptRecord | undefined>
  markAttemptFailed: (
    attempt: GitHubWriteConnectionAttemptRecord,
    status: Exclude<
      GitHubWriteConnectionAttemptStatus,
      'pending' | 'connected'
    >,
    reason: string,
    now: string,
  ) => Promise<GitHubWriteConnectionAttemptRecord>
  recordConnectedAttempt: (
    input: Readonly<{
      attempt: GitHubWriteConnectionAttemptRecord
      connection: GitHubWriteConnectionRecord
    }>,
  ) => Promise<GitHubWriteConnectionRecord>
  listConnectionsForUser: (
    userId: string,
  ) => Promise<ReadonlyArray<GitHubWriteConnectionRecord>>
  listPendingAttemptsForUser: (
    userId: string,
  ) => Promise<ReadonlyArray<GitHubWriteConnectionAttemptRecord>>
  disconnectConnection: (
    input: Readonly<{
      connectionRef: string
      metadataJson: string
      now: string
      userId: string
    }>,
  ) => Promise<GitHubWriteConnectionRecord | undefined>
  findUsableConnectionForUser: (
    userId: string,
  ) => Promise<GitHubWriteConnectionRecord | undefined>
  createGrant: (
    grant: GitHubWriteAuthGrantRecord,
  ) => Promise<GitHubWriteAuthGrantRecord>
  findGrantByRef: (
    grantRef: string,
  ) => Promise<GitHubWriteAuthGrantRecord | undefined>
  markGrantUsed: (
    grant: GitHubWriteAuthGrantRecord,
  ) => Promise<GitHubWriteAuthGrantRecord>
}>

type ConnectionRow = Readonly<{
  id: string
  user_id: string
  github_id: string
  github_login: string
  connection_ref: string
  secret_ref: string | null
  scopes_json: string
  status: GitHubWriteConnectionStatus
  health: GitHubWriteConnectionHealth
  connected_at: string | null
  disconnected_at: string | null
  last_status_at: string
  metadata_json: string | null
  created_at: string
  updated_at: string
  deleted_at: string | null
}>

type AttemptRow = Readonly<{
  id: string
  user_id: string
  state: string
  expected_github_id: string
  expected_github_login: string
  redirect_after: string | null
  scopes_json: string
  status: GitHubWriteConnectionAttemptStatus
  expires_at: string
  completed_at: string | null
  failed_at: string | null
  failure_reason: string | null
  created_at: string
  updated_at: string
}>

type GrantRow = Readonly<{
  id: string
  connection_id: string
  user_id: string
  runner_session_id: string | null
  connection_ref: string
  secret_ref: string
  grant_ref: string
  status: GitHubWriteGrantStatus
  requested_action: string | null
  metadata_json: string | null
  created_at: string
  updated_at: string
  expires_at: string
  used_at: string | null
  revoked_at: string | null
  failed_at: string | null
}>

const textOrUndefined = (value: string | null): string | undefined =>
  value === null || value === '' ? undefined : value

const addMilliseconds = (date: Date, milliseconds: number): string =>
  isoTimestampAfter(date, milliseconds)

const clampText = (value: string, maxLength: number): string =>
  value.trim().replace(/\s+/g, ' ').slice(0, maxLength)

const safeText = (
  value: string | undefined,
  maxLength: number,
): string | undefined => {
  const text = value?.trim()

  return text === undefined || text === ''
    ? undefined
    : clampText(text, maxLength)
}

const parseScopes = (value: string): ReadonlyArray<string> => {
  return parseJsonStringArray(value)
}

const scopesJson = (scopes: ReadonlyArray<string>): string =>
  JSON.stringify([...new Set(scopes.map(scope => clampText(scope, 80)))].sort())

const toConnectionRecord = (
  row: ConnectionRow,
): GitHubWriteConnectionRecord => ({
  id: row.id,
  userId: row.user_id,
  githubId: row.github_id,
  githubLogin: row.github_login,
  connectionRef: row.connection_ref,
  secretRef: row.secret_ref,
  scopes: parseScopes(row.scopes_json),
  status: row.status,
  health: row.health,
  connectedAt: row.connected_at,
  disconnectedAt: row.disconnected_at,
  lastStatusAt: row.last_status_at,
  metadataJson: row.metadata_json,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  deletedAt: row.deleted_at,
})

const toAttemptRecord = (
  row: AttemptRow,
): GitHubWriteConnectionAttemptRecord => ({
  id: row.id,
  userId: row.user_id,
  state: row.state,
  expectedGithubId: row.expected_github_id,
  expectedGithubLogin: row.expected_github_login,
  redirectAfter: row.redirect_after,
  scopes: parseScopes(row.scopes_json),
  status: row.status,
  expiresAt: row.expires_at,
  completedAt: row.completed_at,
  failedAt: row.failed_at,
  failureReason: row.failure_reason,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const toGrantRecord = (row: GrantRow): GitHubWriteAuthGrantRecord => ({
  id: row.id,
  connectionId: row.connection_id,
  userId: row.user_id,
  runnerSessionId: row.runner_session_id,
  connectionRef: row.connection_ref,
  secretRef: row.secret_ref,
  grantRef: row.grant_ref,
  status: row.status,
  requestedAction: row.requested_action,
  metadataJson: row.metadata_json,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  expiresAt: row.expires_at,
  usedAt: row.used_at,
  revokedAt: row.revoked_at,
  failedAt: row.failed_at,
})

export const githubWriteConnectionRef = (id: string): string =>
  `github-write_${id.replace(/^github_write_/, '')}`

export const githubWriteSecretRef = (connectionRef: string): string =>
  `github-write://${connectionRef}`

export const githubWriteSecretKey = (connectionRef: string): string =>
  `github-write:token:${connectionRef}`

export const hasRequiredGitHubWriteScopes = (
  scopes: ReadonlyArray<string>,
): boolean => {
  const granted = new Set(scopes)

  return GITHUB_WRITE_REQUIRED_SCOPES.every(scope => granted.has(scope))
}

export const requireGitHubWriteCallbackAccount = (
  attempt: GitHubWriteConnectionAttemptRecord,
  githubId: string,
): void => {
  if (githubId !== attempt.expectedGithubId) {
    throw new GitHubWriteCallbackMismatch({
      message: 'Connected GitHub account does not match the signed-in user.',
    })
  }
}

export const requireGitHubWritePermissions = (
  scopes: ReadonlyArray<string>,
): void => {
  if (!hasRequiredGitHubWriteScopes(scopes)) {
    throw new GitHubWritePermissionFailure({
      message: 'GitHub write OAuth token is missing repo/workflow scopes.',
    })
  }
}

export const gitHubWriteConnectionMetadataJson = (
  input: Readonly<{
    githubLogin: string
    scopes: ReadonlyArray<string>
    source: string
    status: string
  }>,
): string =>
  JSON.stringify({
    githubLogin: clampText(input.githubLogin, 120),
    scopes: [...input.scopes],
    source: clampText(input.source, 120),
    status: clampText(input.status, 80),
  })

export const toPublicGitHubWriteConnection = (
  connection: GitHubWriteConnectionRecord,
): PublicGitHubWriteConnection => ({
  id: connection.id,
  githubId: connection.githubId,
  githubLogin: connection.githubLogin,
  connectionRef: connection.connectionRef,
  scopes: connection.scopes,
  status: connection.status,
  health: connection.health,
  hasSecretRef: connection.secretRef !== null,
  connectedAt: textOrUndefined(connection.connectedAt),
  disconnectedAt: textOrUndefined(connection.disconnectedAt),
  lastStatusAt: connection.lastStatusAt,
  createdAt: connection.createdAt,
  updatedAt: connection.updatedAt,
})

export const toPublicGitHubWriteAttempt = (
  attempt: GitHubWriteConnectionAttemptRecord,
  now = systemGitHubWriteRuntime.now(),
): PublicGitHubWriteConnectionAttempt => ({
  id: attempt.id,
  status:
    attempt.status === 'pending' &&
    Date.parse(attempt.expiresAt) <= now.getTime()
      ? 'expired'
      : attempt.status,
  scopes: attempt.scopes,
  expiresAt: attempt.expiresAt,
  createdAt: attempt.createdAt,
  updatedAt: attempt.updatedAt,
})

export const makeGitHubWriteConnectionBundle = (
  connections: ReadonlyArray<GitHubWriteConnectionRecord>,
  attempts: ReadonlyArray<GitHubWriteConnectionAttemptRecord>,
  now = systemGitHubWriteRuntime.now(),
): GitHubWriteConnectionBundle => ({
  attempts: attempts.map(attempt => toPublicGitHubWriteAttempt(attempt, now)),
  connections: connections.map(toPublicGitHubWriteConnection),
})

export const makeD1GitHubWriteRepository = (
  db: D1Database,
): GitHubWriteRepository => ({
  createAttempt: async attempt => {
    await db
      .prepare(
        `INSERT INTO github_write_connection_attempts
          (id, user_id, state, expected_github_id, expected_github_login,
           redirect_after, scopes_json, status, expires_at, completed_at,
           failed_at, failure_reason, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        attempt.id,
        attempt.userId,
        attempt.state,
        attempt.expectedGithubId,
        attempt.expectedGithubLogin,
        attempt.redirectAfter,
        scopesJson(attempt.scopes),
        attempt.status,
        attempt.expiresAt,
        attempt.completedAt,
        attempt.failedAt,
        attempt.failureReason,
        attempt.createdAt,
        attempt.updatedAt,
      )
      .run()

    return attempt
  },

  findAttemptByState: async state => {
    const row = await db
      .prepare(
        `SELECT *
         FROM github_write_connection_attempts
         WHERE state = ?`,
      )
      .bind(state)
      .first<AttemptRow>()

    return row === null ? undefined : toAttemptRecord(row)
  },

  markAttemptFailed: async (attempt, status, reason, now) => {
    await db
      .prepare(
        `UPDATE github_write_connection_attempts
         SET status = ?,
             failed_at = ?,
             failure_reason = ?,
             updated_at = ?
         WHERE id = ?`,
      )
      .bind(status, now, clampText(reason, 240), now, attempt.id)
      .run()

    const updated = await db
      .prepare(
        `SELECT *
         FROM github_write_connection_attempts
         WHERE id = ?`,
      )
      .bind(attempt.id)
      .first<AttemptRow>()

    if (updated === null) {
      throw new GitHubWriteReloadFailure({
        operation: 'mark_attempt_failed',
        message: 'GitHub write attempt could not be reloaded.',
      })
    }

    return toAttemptRecord(updated)
  },

  recordConnectedAttempt: async ({ attempt, connection }) => {
    await db.batch([
      db
        .prepare(
          `INSERT INTO github_write_connections
            (id, user_id, github_id, github_login, connection_ref, secret_ref,
             scopes_json, status, health, connected_at, disconnected_at,
             last_status_at, metadata_json, created_at, updated_at, deleted_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(user_id, github_id) DO UPDATE SET
             github_login = excluded.github_login,
             connection_ref = excluded.connection_ref,
             secret_ref = excluded.secret_ref,
             scopes_json = excluded.scopes_json,
             status = excluded.status,
             health = excluded.health,
             connected_at = excluded.connected_at,
             disconnected_at = NULL,
             last_status_at = excluded.last_status_at,
             metadata_json = excluded.metadata_json,
             updated_at = excluded.updated_at,
             deleted_at = NULL`,
        )
        .bind(
          connection.id,
          connection.userId,
          connection.githubId,
          connection.githubLogin,
          connection.connectionRef,
          connection.secretRef,
          scopesJson(connection.scopes),
          connection.status,
          connection.health,
          connection.connectedAt,
          connection.disconnectedAt,
          connection.lastStatusAt,
          connection.metadataJson,
          connection.createdAt,
          connection.updatedAt,
          connection.deletedAt,
        ),
      db
        .prepare(
          `UPDATE github_write_connection_attempts
           SET status = 'connected',
               completed_at = ?,
               updated_at = ?
           WHERE id = ?`,
        )
        .bind(connection.updatedAt, connection.updatedAt, attempt.id),
    ])

    const row = await db
      .prepare(
        `SELECT *
         FROM github_write_connections
         WHERE user_id = ?
           AND github_id = ?
           AND deleted_at IS NULL`,
      )
      .bind(connection.userId, connection.githubId)
      .first<ConnectionRow>()

    if (row === null) {
      throw new GitHubWriteReloadFailure({
        operation: 'record_connected_attempt',
        message: 'GitHub write connection could not be reloaded.',
      })
    }

    return toConnectionRecord(row)
  },

  listConnectionsForUser: async userId => {
    const rows = await db
      .prepare(
        `SELECT *
         FROM github_write_connections
         WHERE user_id = ?
           AND deleted_at IS NULL
         ORDER BY updated_at DESC`,
      )
      .bind(userId)
      .all<ConnectionRow>()

    return rows.results.map(toConnectionRecord)
  },

  listPendingAttemptsForUser: async userId => {
    const rows = await db
      .prepare(
        `SELECT *
         FROM github_write_connection_attempts
         WHERE user_id = ?
           AND status = 'pending'
         ORDER BY created_at DESC
         LIMIT 5`,
      )
      .bind(userId)
      .all<AttemptRow>()

    return rows.results.map(toAttemptRecord)
  },

  disconnectConnection: async ({
    connectionRef,
    metadataJson,
    now,
    userId,
  }) => {
    const row = await db
      .prepare(
        `SELECT *
         FROM github_write_connections
         WHERE user_id = ?
           AND connection_ref = ?
           AND deleted_at IS NULL`,
      )
      .bind(userId, connectionRef)
      .first<ConnectionRow>()

    if (row === null) {
      return undefined
    }

    await db.batch([
      db
        .prepare(
          `UPDATE github_write_connections
           SET secret_ref = NULL,
               status = 'disconnected',
               health = 'requires_reauth',
               disconnected_at = ?,
               last_status_at = ?,
               metadata_json = ?,
               updated_at = ?
           WHERE id = ?`,
        )
        .bind(now, now, metadataJson, now, row.id),
      db
        .prepare(
          `UPDATE github_write_auth_grants
           SET status = 'revoked',
               revoked_at = ?,
               updated_at = ?
           WHERE connection_id = ?
             AND status = 'issued'`,
        )
        .bind(now, now, row.id),
    ])

    const updated = await db
      .prepare(
        `SELECT *
         FROM github_write_connections
         WHERE id = ?`,
      )
      .bind(row.id)
      .first<ConnectionRow>()

    return updated === null ? undefined : toConnectionRecord(updated)
  },

  findUsableConnectionForUser: async userId => {
    const row = await db
      .prepare(
        `SELECT *
         FROM github_write_connections
         WHERE user_id = ?
           AND status = 'connected'
           AND health = 'healthy'
           AND secret_ref IS NOT NULL
           AND deleted_at IS NULL
         ORDER BY updated_at DESC
         LIMIT 1`,
      )
      .bind(userId)
      .first<ConnectionRow>()

    return row === null ? undefined : toConnectionRecord(row)
  },

  createGrant: async grant => {
    await db
      .prepare(
        `INSERT INTO github_write_auth_grants
          (id, connection_id, user_id, runner_session_id, connection_ref,
           secret_ref, grant_ref, status, requested_action, metadata_json,
           created_at, updated_at, expires_at, used_at, revoked_at, failed_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        grant.id,
        grant.connectionId,
        grant.userId,
        grant.runnerSessionId,
        grant.connectionRef,
        grant.secretRef,
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
      )
      .run()

    return grant
  },

  findGrantByRef: async grantRef => {
    const row = await db
      .prepare(
        `SELECT *
         FROM github_write_auth_grants
         WHERE grant_ref = ?`,
      )
      .bind(grantRef)
      .first<GrantRow>()

    return row === null ? undefined : toGrantRecord(row)
  },

  markGrantUsed: async grant => {
    await db
      .prepare(
        `UPDATE github_write_auth_grants
         SET status = 'used',
             used_at = ?,
             updated_at = ?
         WHERE id = ?
           AND status = 'issued'`,
      )
      .bind(grant.usedAt, grant.updatedAt, grant.id)
      .run()

    const row = await db
      .prepare(
        `SELECT *
         FROM github_write_auth_grants
         WHERE id = ?`,
      )
      .bind(grant.id)
      .first<GrantRow>()

    if (row === null) {
      throw new GitHubWriteReloadFailure({
        operation: 'mark_grant_used',
        message: 'GitHub write grant could not be reloaded.',
      })
    }

    return toGrantRecord(row)
  },
})

export const startGitHubWriteConnectionAttempt = async (
  repository: GitHubWriteRepository,
  input: Readonly<{
    expectedGithubId: string
    expectedGithubLogin: string
    redirectAfter?: string | undefined
    scopes?: ReadonlyArray<string> | undefined
    userId: string
  }>,
  options: Readonly<{
    makeId?: IdFactory
    now?: () => Date
  }> = {},
): Promise<GitHubWriteConnectionAttemptRecord> => {
  const runtime = { ...systemGitHubWriteRuntime, ...options }
  const nowDate = runtime.now()
  const now = nowDate.toISOString()
  const makeId = runtime.makeId
  const scopes = input.scopes ?? GITHUB_WRITE_REQUIRED_SCOPES

  return repository.createAttempt({
    id: makeId('github_write_attempt'),
    userId: input.userId,
    state: makeId('github_write_state'),
    expectedGithubId: clampText(input.expectedGithubId, 80),
    expectedGithubLogin: clampText(input.expectedGithubLogin, 120),
    redirectAfter: safeText(input.redirectAfter, 240) ?? '/',
    scopes,
    status: 'pending',
    expiresAt: addMilliseconds(nowDate, ATTEMPT_TTL_MS),
    completedAt: null,
    failedAt: null,
    failureReason: null,
    createdAt: now,
    updatedAt: now,
  })
}

export const recordGitHubWriteConnectionConnected = async (
  repository: GitHubWriteRepository,
  input: Readonly<{
    attempt: GitHubWriteConnectionAttemptRecord
    connectionRef: string
    githubId: string
    githubLogin: string
    scopes: ReadonlyArray<string>
    secretRef: string
  }>,
  options: Readonly<{
    makeId?: IdFactory
    now?: () => Date
  }> = {},
): Promise<PublicGitHubWriteConnection> => {
  const runtime = { ...systemGitHubWriteRuntime, ...options }
  const nowDate = runtime.now()
  const now = nowDate.toISOString()
  const makeId = runtime.makeId
  const connection: GitHubWriteConnectionRecord = {
    id: makeId('github_write_connection'),
    userId: input.attempt.userId,
    githubId: clampText(input.githubId, 80),
    githubLogin: clampText(input.githubLogin, 120),
    connectionRef: clampText(input.connectionRef, 180),
    secretRef: input.secretRef,
    scopes: input.scopes,
    status: 'connected',
    health: 'healthy',
    connectedAt: now,
    disconnectedAt: null,
    lastStatusAt: now,
    metadataJson: gitHubWriteConnectionMetadataJson({
      githubLogin: input.githubLogin,
      scopes: input.scopes,
      source: 'github_oauth_callback',
      status: 'connected',
    }),
    createdAt: now,
    updatedAt: now,
    deletedAt: null,
  }
  const saved = await repository.recordConnectedAttempt({
    attempt: input.attempt,
    connection,
  })

  return toPublicGitHubWriteConnection(saved)
}

export const listGitHubWriteConnectionsForUser = async (
  repository: GitHubWriteRepository,
  userId: string,
  now = systemGitHubWriteRuntime.now(),
): Promise<GitHubWriteConnectionBundle> => {
  const [connections, attempts] = await Promise.all([
    repository.listConnectionsForUser(userId),
    repository.listPendingAttemptsForUser(userId),
  ])

  return makeGitHubWriteConnectionBundle(connections, attempts, now)
}

export const issueGitHubWriteGrant = async (
  repository: GitHubWriteRepository,
  input: Readonly<{
    requestedAction?: string | undefined
    runnerSessionId?: string | undefined
    userId: string
  }>,
  options: Readonly<{
    makeId?: IdFactory
    now?: () => Date
  }> = {},
): Promise<PublicGitHubWriteGrant | undefined> => {
  const connection = await repository.findUsableConnectionForUser(input.userId)

  if (connection === undefined || connection.secretRef === null) {
    return undefined
  }

  if (!hasRequiredGitHubWriteScopes(connection.scopes)) {
    throw new GitHubWritePermissionFailure({
      message: 'GitHub write connection is missing required scopes.',
    })
  }

  const runtime = { ...systemGitHubWriteRuntime, ...options }
  const nowDate = runtime.now()
  const now = nowDate.toISOString()
  const makeId = runtime.makeId
  const grant: GitHubWriteAuthGrantRecord = {
    id: makeId('github_write_grant'),
    connectionId: connection.id,
    userId: input.userId,
    runnerSessionId: safeText(input.runnerSessionId, 180) ?? null,
    connectionRef: connection.connectionRef,
    secretRef: connection.secretRef,
    grantRef: `github-write-grant_${makeId('grant_ref')}`,
    status: 'issued',
    requestedAction: safeText(input.requestedAction, 160) ?? null,
    metadataJson: gitHubWriteConnectionMetadataJson({
      githubLogin: connection.githubLogin,
      scopes: connection.scopes,
      source: 'browser_grant_issue',
      status: 'issued',
    }),
    createdAt: now,
    updatedAt: now,
    expiresAt: addMilliseconds(nowDate, GRANT_TTL_MS),
    usedAt: null,
    revokedAt: null,
    failedAt: null,
  }
  const saved = await repository.createGrant(grant)

  return {
    id: saved.id,
    connectionRef: saved.connectionRef,
    grantRef: saved.grantRef,
    status: saved.status,
    requestedAction: textOrUndefined(saved.requestedAction),
    runnerSessionId: textOrUndefined(saved.runnerSessionId),
    expiresAt: saved.expiresAt,
    createdAt: saved.createdAt,
    updatedAt: saved.updatedAt,
  }
}

export const resolveGitHubWriteGrant = async (
  repository: GitHubWriteRepository,
  input: Readonly<{
    grantRef: string
    runnerSessionId?: string | undefined
  }>,
  options: Readonly<{
    now?: () => Date
  }> = {},
): Promise<ResolvedGitHubWriteGrant | undefined> => {
  const grant = await repository.findGrantByRef(clampText(input.grantRef, 220))

  if (grant === undefined) {
    return undefined
  }

  const runnerSessionId = safeText(input.runnerSessionId, 180)

  if (
    grant.runnerSessionId !== null &&
    runnerSessionId !== undefined &&
    grant.runnerSessionId !== runnerSessionId
  ) {
    throw new GitHubWriteGrantRunnerSessionMismatch({
      message: 'GitHub write grant runner session does not match request.',
    })
  }

  const nowDate = (options.now ?? systemGitHubWriteRuntime.now)()

  if (Date.parse(grant.expiresAt) <= nowDate.getTime()) {
    throw new GitHubWriteGrantExpired({
      message: 'GitHub write grant is expired.',
    })
  }

  if (grant.status !== 'issued') {
    throw new GitHubWriteGrantNotIssued({
      message: 'GitHub write grant is not issued.',
    })
  }

  const connection = await repository.findUsableConnectionForUser(grant.userId)

  if (
    connection === undefined ||
    connection.connectionRef !== grant.connectionRef ||
    connection.secretRef !== grant.secretRef
  ) {
    throw new GitHubWriteConnectionNotUsable({
      message: 'GitHub write connection is not usable.',
    })
  }

  const now = nowDate.toISOString()
  const saved = await repository.markGrantUsed({
    ...grant,
    status: 'used',
    updatedAt: now,
    usedAt: now,
  })

  return {
    connectionRef: saved.connectionRef,
    expiresAt: saved.expiresAt,
    githubLogin: connection.githubLogin,
    grantRef: saved.grantRef,
    requestedAction: textOrUndefined(saved.requestedAction),
    runnerSessionId: textOrUndefined(saved.runnerSessionId),
    scopes: connection.scopes,
    secretRef: saved.secretRef,
    status: 'used',
  }
}

export type GitHubWriteRepositoryServiceShape = Readonly<{
  createAttempt: (
    attempt: GitHubWriteConnectionAttemptRecord,
  ) => Effect.Effect<GitHubWriteConnectionAttemptRecord, GitHubWriteError>
  findAttemptByState: (
    state: string,
  ) => Effect.Effect<
    GitHubWriteConnectionAttemptRecord | undefined,
    GitHubWriteError
  >
  markAttemptFailed: (
    attempt: GitHubWriteConnectionAttemptRecord,
    status: Exclude<
      GitHubWriteConnectionAttemptStatus,
      'pending' | 'connected'
    >,
    reason: string,
    now: string,
  ) => Effect.Effect<GitHubWriteConnectionAttemptRecord, GitHubWriteError>
  recordConnectedAttempt: (
    input: Readonly<{
      attempt: GitHubWriteConnectionAttemptRecord
      connection: GitHubWriteConnectionRecord
    }>,
  ) => Effect.Effect<GitHubWriteConnectionRecord, GitHubWriteError>
  listConnectionsForUser: (
    userId: string,
  ) => Effect.Effect<
    ReadonlyArray<GitHubWriteConnectionRecord>,
    GitHubWriteError
  >
  listPendingAttemptsForUser: (
    userId: string,
  ) => Effect.Effect<
    ReadonlyArray<GitHubWriteConnectionAttemptRecord>,
    GitHubWriteError
  >
  disconnectConnection: (
    input: Readonly<{
      connectionRef: string
      metadataJson: string
      now: string
      userId: string
    }>,
  ) => Effect.Effect<GitHubWriteConnectionRecord | undefined, GitHubWriteError>
  findUsableConnectionForUser: (
    userId: string,
  ) => Effect.Effect<GitHubWriteConnectionRecord | undefined, GitHubWriteError>
  createGrant: (
    grant: GitHubWriteAuthGrantRecord,
  ) => Effect.Effect<GitHubWriteAuthGrantRecord, GitHubWriteError>
  findGrantByRef: (
    grantRef: string,
  ) => Effect.Effect<GitHubWriteAuthGrantRecord | undefined, GitHubWriteError>
  markGrantUsed: (
    grant: GitHubWriteAuthGrantRecord,
  ) => Effect.Effect<GitHubWriteAuthGrantRecord, GitHubWriteError>
}>

export class GitHubWriteRepositoryService extends Context.Service<
  GitHubWriteRepositoryService,
  GitHubWriteRepositoryServiceShape
>()('openagents/GitHubWriteRepositoryService') {}

const gitHubWriteRepositoryEffect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, GitHubWriteError> =>
  Effect.tryPromise({
    try: run,
    catch: error => gitHubWriteErrorFromUnknown(operation, error),
  })

export const makeGitHubWriteRepositoryService = (
  repository: GitHubWriteRepository,
): GitHubWriteRepositoryServiceShape => ({
  createAttempt: attempt =>
    gitHubWriteRepositoryEffect('create_attempt', () =>
      repository.createAttempt(attempt),
    ),
  findAttemptByState: state =>
    gitHubWriteRepositoryEffect('find_attempt_by_state', () =>
      repository.findAttemptByState(state),
    ),
  markAttemptFailed: (attempt, status, reason, now) =>
    gitHubWriteRepositoryEffect('mark_attempt_failed', () =>
      repository.markAttemptFailed(attempt, status, reason, now),
    ),
  recordConnectedAttempt: input =>
    gitHubWriteRepositoryEffect('record_connected_attempt', () =>
      repository.recordConnectedAttempt(input),
    ),
  listConnectionsForUser: userId =>
    gitHubWriteRepositoryEffect('list_connections_for_user', () =>
      repository.listConnectionsForUser(userId),
    ),
  listPendingAttemptsForUser: userId =>
    gitHubWriteRepositoryEffect('list_pending_attempts_for_user', () =>
      repository.listPendingAttemptsForUser(userId),
    ),
  disconnectConnection: input =>
    gitHubWriteRepositoryEffect('disconnect_connection', () =>
      repository.disconnectConnection(input),
    ),
  findUsableConnectionForUser: userId =>
    gitHubWriteRepositoryEffect('find_usable_connection_for_user', () =>
      repository.findUsableConnectionForUser(userId),
    ),
  createGrant: grant =>
    gitHubWriteRepositoryEffect('create_grant', () =>
      repository.createGrant(grant),
    ),
  findGrantByRef: grantRef =>
    gitHubWriteRepositoryEffect('find_grant_by_ref', () =>
      repository.findGrantByRef(grantRef),
    ),
  markGrantUsed: grant =>
    gitHubWriteRepositoryEffect('mark_grant_used', () =>
      repository.markGrantUsed(grant),
    ),
})

export const makeGitHubWriteRepositoryLayer = (
  repository: GitHubWriteRepository,
) =>
  Layer.succeed(
    GitHubWriteRepositoryService,
    makeGitHubWriteRepositoryService(repository),
  )

export const makeD1GitHubWriteRepositoryLayer = (db: D1Database) =>
  makeGitHubWriteRepositoryLayer(makeD1GitHubWriteRepository(db))

export type GitHubWriteConnectionServiceDependencies = Readonly<{
  repository: GitHubWriteRepository
  makeId?: IdFactory | undefined
  now?: (() => Date) | undefined
}>

export type GitHubWriteConnectionServiceShape = Readonly<{
  startConnectionAttempt: (
    input: Parameters<typeof startGitHubWriteConnectionAttempt>[1],
  ) => Effect.Effect<GitHubWriteConnectionAttemptRecord, GitHubWriteError>
  recordConnectionConnected: (
    input: Parameters<typeof recordGitHubWriteConnectionConnected>[1],
  ) => Effect.Effect<PublicGitHubWriteConnection, GitHubWriteError>
  listConnectionsForUser: (
    userId: string,
  ) => Effect.Effect<GitHubWriteConnectionBundle, GitHubWriteError>
  issueGrant: (
    input: Parameters<typeof issueGitHubWriteGrant>[1],
  ) => Effect.Effect<PublicGitHubWriteGrant, GitHubWriteError>
  resolveGrant: (
    input: Parameters<typeof resolveGitHubWriteGrant>[1],
  ) => Effect.Effect<ResolvedGitHubWriteGrant | undefined, GitHubWriteError>
}>

export class GitHubWriteConnectionService extends Context.Service<
  GitHubWriteConnectionService,
  GitHubWriteConnectionServiceShape
>()('openagents/GitHubWriteConnectionService') {}

const gitHubWriteRuntimeOptions = (
  dependencies: GitHubWriteConnectionServiceDependencies,
) => ({
  ...(dependencies.makeId === undefined ? {} : { makeId: dependencies.makeId }),
  ...(dependencies.now === undefined ? {} : { now: dependencies.now }),
})

const gitHubWriteLifecycleEffect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, GitHubWriteError> =>
  Effect.tryPromise({
    try: run,
    catch: error => gitHubWriteErrorFromUnknown(operation, error),
  })

export const makeGitHubWriteConnectionService = (
  dependencies: GitHubWriteConnectionServiceDependencies,
): GitHubWriteConnectionServiceShape => ({
  startConnectionAttempt: input =>
    gitHubWriteLifecycleEffect('start_connection_attempt', () =>
      startGitHubWriteConnectionAttempt(
        dependencies.repository,
        input,
        gitHubWriteRuntimeOptions(dependencies),
      ),
    ),
  recordConnectionConnected: input =>
    gitHubWriteLifecycleEffect('record_connection_connected', () =>
      recordGitHubWriteConnectionConnected(
        dependencies.repository,
        input,
        gitHubWriteRuntimeOptions(dependencies),
      ),
    ),
  listConnectionsForUser: userId =>
    gitHubWriteLifecycleEffect('list_connections_for_user', () =>
      listGitHubWriteConnectionsForUser(
        dependencies.repository,
        userId,
        dependencies.now?.() ?? systemGitHubWriteRuntime.now(),
      ),
    ),
  issueGrant: input =>
    gitHubWriteLifecycleEffect('issue_grant', async () => {
      const grant = await issueGitHubWriteGrant(
        dependencies.repository,
        input,
        gitHubWriteRuntimeOptions(dependencies),
      )

      if (grant === undefined) {
        throw new GitHubWriteMissingConnection({
          message: 'GitHub write connection is not usable.',
        })
      }

      return grant
    }),
  resolveGrant: input =>
    gitHubWriteLifecycleEffect('resolve_grant', () =>
      resolveGitHubWriteGrant(
        dependencies.repository,
        input,
        gitHubWriteRuntimeOptions(dependencies),
      ),
    ),
})

export const makeGitHubWriteConnectionLayer = (
  dependencies: GitHubWriteConnectionServiceDependencies,
) =>
  Layer.succeed(
    GitHubWriteConnectionService,
    makeGitHubWriteConnectionService(dependencies),
  )
