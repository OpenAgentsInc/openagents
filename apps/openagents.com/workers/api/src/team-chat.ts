import { readIdentityUserProfiles, type IdentityDb } from './identity-db'
import { Schema as S } from 'effect'

import {
  isRecord,
  optionalInteger,
  optionalString,
  safeJsonRecord,
} from './json-boundary'
import { currentDate, randomUuid } from './runtime-primitives'

export type TeamChatKind =
  | 'message'
  | 'autopilot_intent'
  | 'adjutant_intent'
  | 'system'

export type TeamChatMessage = Readonly<{
  id: string
  teamId: string
  projectId: string | null
  kind: TeamChatKind
  body: string
  autopilotThreadId: string | null
  agentRunId: string | null
  launchError?: string
  runSummary?: TeamChatRunSummary
  createdAt: string
  author: Readonly<{
    userId: string
    name: string
    avatarUrl: string | null
    githubUsername: string | null
  }>
}>

export type TeamChatRunSummary = Readonly<{
  runId: string
  status:
    | 'queued'
    | 'running'
    | 'waiting_for_input'
    | 'completed'
    | 'failed'
    | 'canceled'
  runtime: string
  backend: string
  repository: string
  eventCount: number
  toolCallCount: number
  tokenTotal: number
  durationSeconds: number | null
  updatedAt: string
}>

export type TeamChatMessageWithMetadata = Readonly<{
  message: TeamChatMessage
  metadataJson: string
}>

type TeamChatRepositoryOptions = Readonly<{
  makeUuid?: () => string
  now?: () => Date
}>

export class TeamChatRepositoryError extends S.TaggedErrorClass<TeamChatRepositoryError>()(
  'TeamChatRepositoryError',
  {
    message: S.String,
    operation: S.String,
  },
) {}

const teamChatNow = (options?: TeamChatRepositoryOptions): Date =>
  options?.now?.() ?? currentDate()

const teamChatUuid = (options?: TeamChatRepositoryOptions): string =>
  options?.makeUuid?.() ?? randomUuid()

export const makeTeamChatMessageId = (
  options?: TeamChatRepositoryOptions,
): string => `team_chat_${teamChatUuid(options)}`

export const makeTeamChatThreadId = (
  options?: TeamChatRepositoryOptions,
): string => teamChatUuid(options)

export const teamChatRunSummaryFromUnknown = (
  value: unknown,
): TeamChatRunSummary | undefined => {
  if (!isRecord(value)) {
    return undefined
  }

  const runId = optionalString(value.runId)

  if (runId === undefined) {
    return undefined
  }

  const runtime = optionalString(value.runtime)
  const backend = optionalString(value.backend)
  const repository = optionalString(value.repository)
  const rawStatus = optionalString(value.status)
  const status =
    rawStatus === 'queued' ||
    rawStatus === 'running' ||
    rawStatus === 'waiting_for_input' ||
    rawStatus === 'completed' ||
    rawStatus === 'failed' ||
    rawStatus === 'canceled'
      ? rawStatus
      : undefined
  const eventCount = optionalInteger(value.eventCount)
  const toolCallCount = optionalInteger(value.toolCallCount)
  const tokenTotal = optionalInteger(value.tokenTotal)
  const durationSeconds =
    value.durationSeconds === null
      ? null
      : (optionalInteger(value.durationSeconds) ?? null)
  const updatedAt = optionalString(value.updatedAt)

  if (
    status === undefined ||
    runtime === undefined ||
    backend === undefined ||
    repository === undefined ||
    eventCount === undefined ||
    toolCallCount === undefined ||
    tokenTotal === undefined ||
    updatedAt === undefined
  ) {
    return undefined
  }

  return {
    runId,
    status,
    runtime,
    backend,
    repository,
    eventCount,
    toolCallCount,
    tokenTotal,
    durationSeconds,
    updatedAt,
  }
}

// CFG-4 Domain 2 (#8519): `users`/`auth_identities` are Postgres-
// authoritative — the author display fields can no longer ride the D1
// message query. Messages read from D1 without the join; this helper
// enriches one page through ONE identity IN-list read, preserving the old
// INNER JOIN semantics (a message whose author row is gone is dropped).
type TeamChatMessageD1Row = Readonly<{
  id: string
  team_id: string
  project_id: string | null
  kind: TeamChatKind
  body: string
  autopilot_thread_id: string | null
  agent_run_id: string | null
  metadata_json: string | null
  created_at: string
  author_user_id: string
}>

type TeamChatMessageAuthorRow = TeamChatMessageD1Row &
  Readonly<{
    author_name: string
    author_avatar_url: string | null
    author_github_username: string | null
  }>

const enrichTeamChatAuthorRows = async <Row extends TeamChatMessageD1Row>(
  identityDb: IdentityDb,
  rows: ReadonlyArray<Row>,
): Promise<ReadonlyArray<Row & TeamChatMessageAuthorRow>> => {
  const profiles = await readIdentityUserProfiles(
    identityDb,
    rows.map(row => row.author_user_id),
  )
  return rows.flatMap(row => {
    const profile = profiles.get(row.author_user_id)
    return profile === undefined
      ? []
      : [
          {
            ...row,
            author_avatar_url: profile.avatarUrl,
            author_github_username: profile.githubUsername,
            author_name: profile.displayName,
          },
        ]
  })
}

export const publicTeamChatMessage = (
  row: Readonly<{
    id: string
    team_id: string
    project_id?: string | null
    kind: TeamChatKind
    body: string
    autopilot_thread_id: string | null
    agent_run_id: string | null
    metadata_json?: string | null
    created_at: string
    author_user_id: string
    author_name: string
    author_avatar_url: string | null
    author_github_username: string | null
  }>,
): TeamChatMessage => {
  const metadata = safeJsonRecord(row.metadata_json)
  const launchError = optionalString(metadata?.launchError)
  const runSummary = teamChatRunSummaryFromUnknown(metadata?.runSummary)

  return {
    id: row.id,
    teamId: row.team_id,
    projectId: row.project_id ?? null,
    kind: row.kind,
    body: row.body,
    autopilotThreadId: row.autopilot_thread_id,
    agentRunId: row.agent_run_id,
    ...(launchError === undefined ? {} : { launchError }),
    ...(runSummary === undefined ? {} : { runSummary }),
    createdAt: row.created_at,
    author: {
      userId: row.author_user_id,
      name: row.author_name,
      avatarUrl: row.author_avatar_url,
      githubUsername: row.author_github_username,
    },
  }
}

export const listTeamChatMessages = async (
  db: D1Database,
  identityDb: IdentityDb,
  teamId: string,
  limit: number,
  kind?: TeamChatKind,
  autopilotThreadId?: string,
  projectId: string | null = null,
): Promise<ReadonlyArray<TeamChatMessage>> => {
  const whereKind =
    kind === undefined ? '' : '         AND team_chat_messages.kind = ?\n'
  const whereAutopilotThread =
    autopilotThreadId === undefined
      ? ''
      : '         AND team_chat_messages.autopilot_thread_id = ?\n'
  const whereProject =
    projectId === null
      ? '         AND team_chat_messages.project_id IS NULL\n'
      : '         AND team_chat_messages.project_id = ?\n'
  const bindings = [
    teamId,
    ...(kind === undefined ? [] : [kind]),
    ...(autopilotThreadId === undefined ? [] : [autopilotThreadId]),
    ...(projectId === null ? [] : [projectId]),
    limit,
  ]
  const rows = await db
    .prepare(
      `SELECT
         team_chat_messages.id,
         team_chat_messages.team_id,
         team_chat_messages.project_id,
         team_chat_messages.kind,
         team_chat_messages.body,
         team_chat_messages.autopilot_thread_id,
         team_chat_messages.agent_run_id,
         team_chat_messages.metadata_json,
         team_chat_messages.created_at,
         team_chat_messages.author_user_id
       FROM team_chat_messages
       WHERE team_chat_messages.team_id = ?
         AND team_chat_messages.deleted_at IS NULL
         AND team_chat_messages.archived_at IS NULL
${whereKind}
${whereAutopilotThread}
${whereProject}
       ORDER BY team_chat_messages.created_at DESC
       LIMIT ?`,
    )
    .bind(...bindings)
    .all<TeamChatMessageD1Row>()

  const withAuthors = await enrichTeamChatAuthorRows(
    identityDb,
    rows.results ?? [],
  )

  return withAuthors.map(publicTeamChatMessage).reverse()
}

export const readTeamChatMessageById = async (
  db: D1Database,
  identityDb: IdentityDb,
  messageId: string,
): Promise<TeamChatMessage | undefined> => {
  const row = await db
    .prepare(
      `SELECT
         team_chat_messages.id,
         team_chat_messages.team_id,
         team_chat_messages.project_id,
         team_chat_messages.kind,
         team_chat_messages.body,
         team_chat_messages.autopilot_thread_id,
         team_chat_messages.agent_run_id,
         team_chat_messages.metadata_json,
         team_chat_messages.created_at,
         team_chat_messages.author_user_id
       FROM team_chat_messages
       WHERE team_chat_messages.id = ?
         AND team_chat_messages.deleted_at IS NULL
         AND team_chat_messages.archived_at IS NULL
       LIMIT 1`,
    )
    .bind(messageId)
    .first<TeamChatMessageD1Row>()

  const withAuthors = await enrichTeamChatAuthorRows(
    identityDb,
    row === null ? [] : [row],
  )

  return withAuthors.length === 0
    ? undefined
    : publicTeamChatMessage(withAuthors[0]!)
}

export const readTeamChatMessageByAgentRunId = async (
  db: D1Database,
  identityDb: IdentityDb,
  agentRunId: string,
): Promise<TeamChatMessageWithMetadata | undefined> => {
  const row = await db
    .prepare(
      `SELECT
         team_chat_messages.id,
         team_chat_messages.team_id,
         team_chat_messages.project_id,
         team_chat_messages.kind,
         team_chat_messages.body,
         team_chat_messages.autopilot_thread_id,
         team_chat_messages.agent_run_id,
         team_chat_messages.metadata_json,
         team_chat_messages.created_at,
         team_chat_messages.author_user_id
       FROM team_chat_messages
       WHERE team_chat_messages.agent_run_id = ?
         AND team_chat_messages.kind = 'autopilot_intent'
         AND team_chat_messages.deleted_at IS NULL
         AND team_chat_messages.archived_at IS NULL
       ORDER BY team_chat_messages.created_at ASC
       LIMIT 1`,
    )
    .bind(agentRunId)
    .first<TeamChatMessageD1Row & Readonly<{ metadata_json: string }>>()

  const withAuthors = await enrichTeamChatAuthorRows(
    identityDb,
    row === null ? [] : [row],
  )
  const enriched = withAuthors[0]

  return enriched === undefined || row === null
    ? undefined
    : {
        message: publicTeamChatMessage(enriched),
        metadataJson: row.metadata_json,
      }
}

export const insertTeamChatMessage = async (
  db: D1Database,
  identityDb: IdentityDb,
  input: Readonly<{
    id?: string
    teamId: string
    projectId?: string
    authorUserId: string
    body: string
    kind: TeamChatKind
    agentRunId?: string
    autopilotThreadId?: string
    metadataJson?: string
  }>,
  options?: TeamChatRepositoryOptions,
): Promise<TeamChatMessage> => {
  const now = teamChatNow(options).toISOString()
  const id = input.id ?? makeTeamChatMessageId(options)

  await db
    .prepare(
      `INSERT INTO team_chat_messages
        (id, team_id, project_id, author_user_id, kind, body, autopilot_thread_id, agent_run_id, metadata_json, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      id,
      input.teamId,
      input.projectId ?? null,
      input.authorUserId,
      input.kind,
      input.body,
      input.autopilotThreadId ?? null,
      input.agentRunId ?? null,
      input.metadataJson ?? '{}',
      now,
      now,
    )
    .run()

  const message = await readTeamChatMessageById(db, identityDb, id)

  if (message === undefined) {
    throw new TeamChatRepositoryError({
      message: 'Inserted team chat message could not be loaded.',
      operation: 'insert',
    })
  }

  return message
}

export const updateTeamChatMessageRunSummary = async (
  db: D1Database,
  identityDb: IdentityDb,
  input: Readonly<{
    messageId: string
    metadataJson: string
    runSummary: TeamChatRunSummary
  }>,
  options?: TeamChatRepositoryOptions,
): Promise<TeamChatMessage | undefined> => {
  const metadata = {
    ...(safeJsonRecord(input.metadataJson) ?? {}),
    runSummary: input.runSummary,
  }

  await db
    .prepare(
      `UPDATE team_chat_messages
       SET metadata_json = ?, updated_at = ?
       WHERE id = ?
         AND deleted_at IS NULL
         AND archived_at IS NULL`,
    )
    .bind(
      JSON.stringify(metadata),
      teamChatNow(options).toISOString(),
      input.messageId,
    )
    .run()

  return readTeamChatMessageById(db, identityDb, input.messageId)
}

type TeamChatLaunchErrorResponse = Readonly<{
  clone: () => Readonly<{
    json: () => Promise<unknown>
  }>
  status: number
}>

export const teamChatLaunchErrorFromResponse = async (
  response: TeamChatLaunchErrorResponse,
): Promise<string> => {
  const payload = await response
    .clone()
    .json()
    .catch((): unknown => undefined)

  if (isRecord(payload)) {
    const message =
      optionalString(payload.message) ??
      optionalString(payload.reason) ??
      optionalString(payload.error)

    if (message !== undefined) {
      return message
    }
  }

  return `Autopilot launch failed with HTTP ${response.status}.`
}
