import { Schema as S } from 'effect'

import { sha256Hex } from './agent-registration'
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
  randomUuid,
} from './runtime-primitives'

export const TeamWorkspaceInviteRole = S.Literals(['admin', 'member', 'viewer'])
export type TeamWorkspaceInviteRole = typeof TeamWorkspaceInviteRole.Type

export const TeamWorkspaceInviteStatus = S.Literals([
  'pending',
  'accepted',
  'expired',
  'revoked',
])
export type TeamWorkspaceInviteStatus = typeof TeamWorkspaceInviteStatus.Type

export type TeamWorkspaceInviteRecord = Readonly<{
  acceptedAt: string | null
  acceptedByUserId: string | null
  createdAt: string
  emailMessageId: string | null
  expiresAt: string
  id: string
  inviteeEmail: string
  inviteeEmailNormalized: string
  invitedByActorRef: string
  lastSentAt: string | null
  metadataJson: string
  projectId: string | null
  revokedAt: string | null
  role: TeamWorkspaceInviteRole
  sendCount: number
  status: TeamWorkspaceInviteStatus
  teamId: string
  tokenHash: string
  updatedAt: string
}>

export type SafeTeamWorkspaceInviteProjection = Readonly<{
  acceptedAt: string | null
  createdAt: string
  emailMessageId: string | null
  expiresAt: string
  id: string
  lastSentAt: string | null
  projectId: string | null
  role: TeamWorkspaceInviteRole
  sendCount: number
  status: TeamWorkspaceInviteStatus
  teamId: string
  updatedAt: string
}>

export type TeamWorkspaceInviteCreateInput = Readonly<{
  email: string
  expiresAt?: string | undefined
  expiresInHours?: number | undefined
  id?: string | undefined
  invitedByActorRef: string
  metadataJson?: string | undefined
  projectId?: string | null | undefined
  role?: TeamWorkspaceInviteRole | undefined
  teamId: string
  token?: string | undefined
}>

export type TeamWorkspaceInviteAcceptInput = Readonly<{
  sessionEmail: string
  token: string
  userId: string
}>

export type TeamWorkspaceInviteCreateResult =
  | Readonly<{
      _tag: 'Created'
      invite: TeamWorkspaceInviteRecord
      token: string
    }>
  | Readonly<{
      _tag: 'Refreshed'
      invite: TeamWorkspaceInviteRecord
      token: string
    }>
  | Readonly<{ _tag: 'InvalidEmail' }>
  | Readonly<{ _tag: 'ProjectNotFound' }>
  | Readonly<{ _tag: 'TeamNotFound' }>

export type TeamWorkspaceInviteAcceptResult =
  | Readonly<{
      _tag: 'Accepted'
      invite: TeamWorkspaceInviteRecord
      membershipId: string
    }>
  | Readonly<{
      _tag: 'AlreadyAccepted'
      invite: TeamWorkspaceInviteRecord
      membershipId: string
    }>
  | Readonly<{ _tag: 'Expired'; invite: TeamWorkspaceInviteRecord }>
  | Readonly<{ _tag: 'InviteUnavailable'; status: TeamWorkspaceInviteStatus }>
  | Readonly<{ _tag: 'NotFound' }>
  | Readonly<{ _tag: 'WrongUser' }>

export type TeamWorkspaceInviteStore = Readonly<{
  acceptInvite: (
    input: TeamWorkspaceInviteAcceptInput,
  ) => Promise<TeamWorkspaceInviteAcceptResult>
  createOrRefreshInvite: (
    input: TeamWorkspaceInviteCreateInput,
  ) => Promise<TeamWorkspaceInviteCreateResult>
  recordEmailAttempt: (
    input: Readonly<{
      attemptedAt: string
      emailMessageId: string
      inviteId: string
    }>,
  ) => Promise<TeamWorkspaceInviteRecord | undefined>
}>

type TeamWorkspaceInviteRow = Readonly<{
  accepted_at: string | null
  accepted_by_user_id: string | null
  created_at: string
  email_message_id: string | null
  expires_at: string
  id: string
  invitee_email: string
  invitee_email_normalized: string
  invited_by_actor_ref: string
  last_sent_at: string | null
  metadata_json: string
  project_id: string | null
  revoked_at: string | null
  role: TeamWorkspaceInviteRole
  send_count: number
  status: TeamWorkspaceInviteStatus
  team_id: string
  token_hash: string
  updated_at: string
}>

class TeamWorkspaceInviteWriteError extends Error {
  constructor(inviteId: string) {
    super(`Invite ${inviteId} was not found after write.`)
    this.name = 'TeamWorkspaceInviteWriteError'
  }
}

const SIMPLE_EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
const DEFAULT_INVITE_TTL_HOURS = 72

const bytesToBase64Url = (bytes: Uint8Array): string =>
  btoa(Array.from(bytes, byte => String.fromCharCode(byte)).join(''))
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replaceAll('=', '')

const randomBase64Url = (byteLength: number): string => {
  const bytes = new Uint8Array(byteLength)
  crypto.getRandomValues(bytes)

  return bytesToBase64Url(bytes)
}

export const createTeamWorkspaceInviteToken = (): string =>
  `oa_team_invite_${randomBase64Url(32)}`

export const normalizeTeamWorkspaceInviteEmail = (
  email: string,
): string | undefined => {
  const normalized = email.trim().toLowerCase()

  return SIMPLE_EMAIL_PATTERN.test(normalized) ? normalized : undefined
}

export const hashTeamWorkspaceInviteToken = (token: string): Promise<string> =>
  sha256Hex(token.trim())

export const safeTeamWorkspaceInviteProjection = (
  invite: TeamWorkspaceInviteRecord,
): SafeTeamWorkspaceInviteProjection => ({
  acceptedAt: invite.acceptedAt,
  createdAt: invite.createdAt,
  emailMessageId: invite.emailMessageId,
  expiresAt: invite.expiresAt,
  id: invite.id,
  lastSentAt: invite.lastSentAt,
  projectId: invite.projectId,
  role: invite.role,
  sendCount: invite.sendCount,
  status: invite.status,
  teamId: invite.teamId,
  updatedAt: invite.updatedAt,
})

const recordFromRow = (
  row: TeamWorkspaceInviteRow,
): TeamWorkspaceInviteRecord => ({
  acceptedAt: row.accepted_at,
  acceptedByUserId: row.accepted_by_user_id,
  createdAt: row.created_at,
  emailMessageId: row.email_message_id,
  expiresAt: row.expires_at,
  id: row.id,
  inviteeEmail: row.invitee_email,
  inviteeEmailNormalized: row.invitee_email_normalized,
  invitedByActorRef: row.invited_by_actor_ref,
  lastSentAt: row.last_sent_at,
  metadataJson: row.metadata_json,
  projectId: row.project_id,
  revokedAt: row.revoked_at,
  role: row.role,
  sendCount: row.send_count,
  status: row.status,
  teamId: row.team_id,
  tokenHash: row.token_hash,
  updatedAt: row.updated_at,
})

const membershipIdFor = (teamId: string, userId: string): string =>
  `team_member_${teamId}_${userId}`
    .replace(/[^A-Za-z0-9_]+/g, '_')
    .slice(0, 240)

const readActiveTeamExists = async (
  db: D1Database,
  teamId: string,
): Promise<boolean> => {
  const row = await db
    .prepare(
      `SELECT id
         FROM teams
        WHERE id = ?
          AND status = 'active'
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(teamId)
    .first<Readonly<{ id: string }>>()

  return row !== null
}

const readActiveProjectExists = async (
  db: D1Database,
  teamId: string,
  projectId: string,
): Promise<boolean> => {
  const row = await db
    .prepare(
      `SELECT id
         FROM team_projects
        WHERE id = ?
          AND team_id = ?
          AND status = 'active'
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(projectId, teamId)
    .first<Readonly<{ id: string }>>()

  return row !== null
}

const readPendingInviteByTarget = async (
  db: D1Database,
  input: Readonly<{
    emailNormalized: string
    projectId: string | null
    teamId: string
  }>,
): Promise<TeamWorkspaceInviteRecord | undefined> => {
  const row = await db
    .prepare(
      `SELECT id, team_id, project_id, invitee_email, invitee_email_normalized,
              role, status, token_hash, invited_by_actor_ref, accepted_by_user_id,
              email_message_id, created_at, updated_at, expires_at, accepted_at,
              revoked_at, last_sent_at, send_count, metadata_json
         FROM team_workspace_invites
        WHERE team_id = ?
          AND COALESCE(project_id, '') = COALESCE(?, '')
          AND invitee_email_normalized = ?
          AND status = 'pending'
        ORDER BY updated_at DESC
        LIMIT 1`,
    )
    .bind(input.teamId, input.projectId, input.emailNormalized)
    .first<TeamWorkspaceInviteRow>()

  return row === null ? undefined : recordFromRow(row)
}

const readInviteByTokenHash = async (
  db: D1Database,
  tokenHash: string,
): Promise<TeamWorkspaceInviteRecord | undefined> => {
  const row = await db
    .prepare(
      `SELECT id, team_id, project_id, invitee_email, invitee_email_normalized,
              role, status, token_hash, invited_by_actor_ref, accepted_by_user_id,
              email_message_id, created_at, updated_at, expires_at, accepted_at,
              revoked_at, last_sent_at, send_count, metadata_json
         FROM team_workspace_invites
        WHERE token_hash = ?
        LIMIT 1`,
    )
    .bind(tokenHash)
    .first<TeamWorkspaceInviteRow>()

  return row === null ? undefined : recordFromRow(row)
}

const readInviteById = async (
  db: D1Database,
  inviteId: string,
): Promise<TeamWorkspaceInviteRecord> => {
  const row = await db
    .prepare(
      `SELECT id, team_id, project_id, invitee_email, invitee_email_normalized,
              role, status, token_hash, invited_by_actor_ref, accepted_by_user_id,
              email_message_id, created_at, updated_at, expires_at, accepted_at,
              revoked_at, last_sent_at, send_count, metadata_json
         FROM team_workspace_invites
        WHERE id = ?
        LIMIT 1`,
    )
    .bind(inviteId)
    .first<TeamWorkspaceInviteRow>()

  if (row === null) {
    throw new TeamWorkspaceInviteWriteError(inviteId)
  }

  return recordFromRow(row)
}

export const makeD1TeamWorkspaceInviteStore = (
  db: D1Database,
  runtime: Readonly<{
    makeId?: (prefix: string) => string
    makeToken?: () => string
    nowIso?: () => string
  }> = {},
): TeamWorkspaceInviteStore => ({
  createOrRefreshInvite: async input => {
    const nowIso = (runtime.nowIso ?? currentIsoTimestamp)()
    const emailNormalized = normalizeTeamWorkspaceInviteEmail(input.email)

    if (emailNormalized === undefined) {
      return { _tag: 'InvalidEmail' }
    }

    if (!(await readActiveTeamExists(db, input.teamId))) {
      return { _tag: 'TeamNotFound' }
    }

    const projectId = input.projectId ?? null

    if (
      projectId !== null &&
      !(await readActiveProjectExists(db, input.teamId, projectId))
    ) {
      return { _tag: 'ProjectNotFound' }
    }

    const token =
      input.token ?? (runtime.makeToken ?? createTeamWorkspaceInviteToken)()
    const tokenHash = await hashTeamWorkspaceInviteToken(token)
    const expiresAt =
      input.expiresAt ??
      isoTimestampAfterIso(
        nowIso,
        (input.expiresInHours ?? DEFAULT_INVITE_TTL_HOURS) * 60 * 60 * 1000,
      )
    const role = input.role ?? 'member'
    const existing = await readPendingInviteByTarget(db, {
      emailNormalized,
      projectId,
      teamId: input.teamId,
    })

    if (existing !== undefined) {
      await db
        .prepare(
          `UPDATE team_workspace_invites
              SET invitee_email = ?,
                  role = ?,
                  token_hash = ?,
                  invited_by_actor_ref = ?,
                  expires_at = ?,
                  updated_at = ?,
                  metadata_json = ?
            WHERE id = ?`,
        )
        .bind(
          input.email.trim(),
          role,
          tokenHash,
          input.invitedByActorRef,
          expiresAt,
          nowIso,
          input.metadataJson ?? existing.metadataJson,
          existing.id,
        )
        .run()

      return {
        _tag: 'Refreshed',
        invite: await readInviteById(db, existing.id),
        token,
      }
    }

    const inviteId = input.id ?? `team_workspace_invite_${randomUuid()}`

    await db
      .prepare(
        `INSERT INTO team_workspace_invites
          (id, team_id, project_id, invitee_email, invitee_email_normalized,
           role, status, token_hash, invited_by_actor_ref, accepted_by_user_id,
           email_message_id, created_at, updated_at, expires_at, accepted_at,
           revoked_at, last_sent_at, send_count, metadata_json)
         VALUES (?, ?, ?, ?, ?, ?, 'pending', ?, ?, NULL, NULL, ?, ?, ?, NULL,
                 NULL, NULL, 0, ?)`,
      )
      .bind(
        inviteId,
        input.teamId,
        projectId,
        input.email.trim(),
        emailNormalized,
        role,
        tokenHash,
        input.invitedByActorRef,
        nowIso,
        nowIso,
        expiresAt,
        input.metadataJson ?? '{}',
      )
      .run()

    return {
      _tag: 'Created',
      invite: await readInviteById(db, inviteId),
      token,
    }
  },

  acceptInvite: async input => {
    const nowIso = (runtime.nowIso ?? currentIsoTimestamp)()
    const tokenHash = await hashTeamWorkspaceInviteToken(input.token)
    const invite = await readInviteByTokenHash(db, tokenHash)

    if (invite === undefined) {
      return { _tag: 'NotFound' }
    }

    const membershipId = membershipIdFor(invite.teamId, input.userId)

    if (invite.status === 'accepted') {
      return invite.acceptedByUserId === input.userId
        ? { _tag: 'AlreadyAccepted', invite, membershipId }
        : { _tag: 'InviteUnavailable', status: invite.status }
    }

    if (invite.status !== 'pending') {
      return { _tag: 'InviteUnavailable', status: invite.status }
    }

    if (Date.parse(invite.expiresAt) <= Date.parse(nowIso)) {
      await db
        .prepare(
          `UPDATE team_workspace_invites
              SET status = 'expired',
                  updated_at = ?
            WHERE id = ?
              AND status = 'pending'`,
        )
        .bind(nowIso, invite.id)
        .run()

      return {
        _tag: 'Expired',
        invite: await readInviteById(db, invite.id),
      }
    }

    if (
      normalizeTeamWorkspaceInviteEmail(input.sessionEmail) !==
      invite.inviteeEmailNormalized
    ) {
      return { _tag: 'WrongUser' }
    }

    await db.batch([
      db
        .prepare(
          `INSERT INTO team_memberships
            (id, team_id, user_id, role, status, joined_at, created_at,
             updated_at, removed_at)
           VALUES (?, ?, ?, ?, 'active', ?, ?, ?, NULL)
           ON CONFLICT(team_id, user_id) DO UPDATE SET
             role = excluded.role,
             status = 'active',
             joined_at = COALESCE(team_memberships.joined_at, excluded.joined_at),
             updated_at = excluded.updated_at,
             removed_at = NULL`,
        )
        .bind(
          membershipId,
          invite.teamId,
          input.userId,
          invite.role,
          nowIso,
          nowIso,
          nowIso,
        ),
      db
        .prepare(
          `UPDATE team_workspace_invites
              SET status = 'accepted',
                  accepted_by_user_id = ?,
                  accepted_at = ?,
                  updated_at = ?
            WHERE id = ?
              AND status = 'pending'`,
        )
        .bind(input.userId, nowIso, nowIso, invite.id),
    ])

    return {
      _tag: 'Accepted',
      invite: await readInviteById(db, invite.id),
      membershipId,
    }
  },

  recordEmailAttempt: async input => {
    await db
      .prepare(
        `UPDATE team_workspace_invites
            SET email_message_id = ?,
                last_sent_at = ?,
                send_count = send_count + 1,
                updated_at = ?
          WHERE id = ?`,
      )
      .bind(
        input.emailMessageId,
        input.attemptedAt,
        input.attemptedAt,
        input.inviteId,
      )
      .run()

    const row = await db
      .prepare(
        `SELECT id, team_id, project_id, invitee_email, invitee_email_normalized,
                role, status, token_hash, invited_by_actor_ref, accepted_by_user_id,
                email_message_id, created_at, updated_at, expires_at, accepted_at,
                revoked_at, last_sent_at, send_count, metadata_json
           FROM team_workspace_invites
          WHERE id = ?
          LIMIT 1`,
      )
      .bind(input.inviteId)
      .first<TeamWorkspaceInviteRow>()

    return row === null ? undefined : recordFromRow(row)
  },
})
