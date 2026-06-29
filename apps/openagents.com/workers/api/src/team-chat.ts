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
         team_chat_messages.author_user_id,
         users.display_name AS author_name,
         users.avatar_url AS author_avatar_url,
         auth_identities.provider_username AS author_github_username
       FROM team_chat_messages
       INNER JOIN users ON users.id = team_chat_messages.author_user_id
       LEFT JOIN auth_identities
         ON auth_identities.user_id = users.id
        AND auth_identities.provider = 'github'
        AND auth_identities.deleted_at IS NULL
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
    .all<
      Readonly<{
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
        author_name: string
        author_avatar_url: string | null
        author_github_username: string | null
      }>
    >()

  return rows.results.map(publicTeamChatMessage).reverse()
}

export const readTeamChatMessageById = async (
  db: D1Database,
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
         team_chat_messages.author_user_id,
         users.display_name AS author_name,
         users.avatar_url AS author_avatar_url,
         auth_identities.provider_username AS author_github_username
       FROM team_chat_messages
       INNER JOIN users ON users.id = team_chat_messages.author_user_id
       LEFT JOIN auth_identities
         ON auth_identities.user_id = users.id
        AND auth_identities.provider = 'github'
        AND auth_identities.deleted_at IS NULL
       WHERE team_chat_messages.id = ?
         AND team_chat_messages.deleted_at IS NULL
         AND team_chat_messages.archived_at IS NULL
       LIMIT 1`,
    )
    .bind(messageId)
    .first<
      Readonly<{
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
        author_name: string
        author_avatar_url: string | null
        author_github_username: string | null
      }>
    >()

  return row === null ? undefined : publicTeamChatMessage(row)
}

export const readTeamChatMessageByAgentRunId = async (
  db: D1Database,
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
         team_chat_messages.author_user_id,
         users.display_name AS author_name,
         users.avatar_url AS author_avatar_url,
         auth_identities.provider_username AS author_github_username
       FROM team_chat_messages
       INNER JOIN users ON users.id = team_chat_messages.author_user_id
       LEFT JOIN auth_identities
         ON auth_identities.user_id = users.id
        AND auth_identities.provider = 'github'
        AND auth_identities.deleted_at IS NULL
       WHERE team_chat_messages.agent_run_id = ?
         AND team_chat_messages.kind = 'autopilot_intent'
         AND team_chat_messages.deleted_at IS NULL
         AND team_chat_messages.archived_at IS NULL
       ORDER BY team_chat_messages.created_at ASC
       LIMIT 1`,
    )
    .bind(agentRunId)
    .first<
      Readonly<{
        id: string
        team_id: string
        project_id: string | null
        kind: TeamChatKind
        body: string
        autopilot_thread_id: string | null
        agent_run_id: string | null
        metadata_json: string
        created_at: string
        author_user_id: string
        author_name: string
        author_avatar_url: string | null
        author_github_username: string | null
      }>
    >()

  return row === null
    ? undefined
    : {
        message: publicTeamChatMessage(row),
        metadataJson: row.metadata_json,
      }
}

export const insertTeamChatMessage = async (
  db: D1Database,
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

  const message = await readTeamChatMessageById(db, id)

  if (message === undefined) {
    throw new Error('Inserted team chat message could not be loaded.')
  }

  return message
}

export const updateTeamChatMessageRunSummary = async (
  db: D1Database,
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

  return readTeamChatMessageById(db, input.messageId)
}

export const teamChatLaunchErrorFromResponse = async (
  response: Response,
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
