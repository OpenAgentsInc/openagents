import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import {
  AgentRunAssignment,
  AppDeployAssignment,
  type BlueprintAssignmentScope,
  GitHubWorkOrder,
  type RunnerBackend,
  type RunnerRuntime,
  normalizeOmniRunnerEventPayload,
} from '@openagentsinc/sync-schema'
import {
  type SyncChangeInput,
  agentRunScope,
  makeD1SyncOutboxRepository,
  personalWorkroomScope,
  teamScope,
  threadScope,
} from '@openagentsinc/sync-worker'
import { Schema as S } from 'effect'

import { buildAgentGoalAssignmentContext } from './agent-goal-runtime'
import {
  codexUsageDebitInsert,
  codexUsageDebitMirrorRef,
  ensureBillingAccount,
  recordContainerUsageDebitForRun,
  type BillingRuntime,
} from './billing'
import {
  parseJsonStringArray,
  parseJsonUnknown,
  parseJsonWithSchema,
  recordFromUnknown,
  safeJsonRecord,
} from './json-boundary'
import {
  OmniDispatchMalformedResponse,
  OmniDispatchMissingCredentials,
  OmniDispatchRejectedRequest,
  OmniDispatchTimeout,
  OmniDispatchTransportFailure,
  OmniDispatchUnavailableEndpoint,
} from './omni/errors'
import { assertProbeBlueprintAssignmentScopeSafe } from './probe-blueprint-assignment-scope'
import {
  compactRandomId,
  currentIsoTimestamp,
  randomUuid,
} from './runtime-primitives'
import {
  resolveTokenUsageAccountAttribution,
  sourceRefForTokenUsageEvent,
  tokenUsageFromEvent,
} from './token-usage'

export type AgentRunStatus =
  | 'queued'
  | 'running'
  | 'waiting_for_input'
  | 'completed'
  | 'failed'
  | 'canceled'

export type DeploymentStatus =
  | 'queued'
  | 'running'
  | 'promoted'
  | 'rolled_back'
  | 'failed'
  | 'canceled'

export type RepositoryRef = Readonly<{
  provider: 'github'
  owner: string
  repo: string
  ref: string
}>

export type GitHubWorkOrderInput = Readonly<{
  baseRef?: string | undefined
  branchName?: string | undefined
  commitMessage?: string | undefined
  issueComment?: string | undefined
  issueNumber?: number | undefined
  issueUrl?: string | undefined
  openPullRequest?: boolean | undefined
  pullRequestBody?: string | undefined
  pullRequestTitle?: string | undefined
  repository: RepositoryRef
  runId: string
}>

export type AgentRunRecord = Readonly<{
  id: string
  userId: string
  teamId: string | null
  projectId: string | null
  runtime: RunnerRuntime
  backend: RunnerBackend
  runnerId: string
  assignmentKind: 'workroom_agent'
  repository: RepositoryRef
  goal: string
  goalId: string | null
  providerAccountRef: string | null
  authGrantRef: string | null
  externalRunId: string | null
  status: AgentRunStatus
  eventCursor: number
  assignment: AgentRunAssignment
  createdAt: string
  updatedAt: string
  startedAt: string | null
  completedAt: string | null
  failedAt: string | null
  canceledAt: string | null
  archivedAt: string | null
}>

export type DeploymentRecord = Readonly<{
  id: string
  userId: string
  teamId: string | null
  service: string
  runtime: RunnerRuntime
  primaryBackend: RunnerBackend
  fallbackBackend: RunnerBackend
  repository: RepositoryRef
  externalDeployId: string | null
  status: DeploymentStatus
  eventCursor: number
  assignment: AppDeployAssignment
  createdAt: string
  updatedAt: string
  startedAt: string | null
  completedAt: string | null
  failedAt: string | null
  canceledAt: string | null
}>

export type OmniEventRecord = Readonly<{
  id: string
  parentId: string
  sequence: number
  type: string
  summary: string
  status: string | null
  source: string
  payloadJson: string | null
  artifactRefs: ReadonlyArray<string>
  externalEventId: string | null
  createdAt: string
}>

export type AgentRunBundle = Readonly<{
  run: AgentRunRecord
  events: ReadonlyArray<OmniEventRecord>
}>

export type DeploymentBundle = Readonly<{
  deployment: DeploymentRecord
  events: ReadonlyArray<OmniEventRecord>
}>

export type DispatchResult = Readonly<{
  callbackDelivery?: RunnerCallbackDeliveryProjection | undefined
  externalId: string
  mode: 'live'
  status: string
  payload?: unknown
}>

export type RunnerCallbackDeliveryProjection = Readonly<{
  eventType: string | null
  httpStatus: number
  message: string
  reason: 'callback_delivery_failed' | 'callback_ingest_rejected'
  sequence: number | null
  status: 'failed'
}>

export type OmniRunStore = Readonly<{
  appendAgentRunEvents: (
    runId: string,
    events: ReadonlyArray<OmniEventRecord>,
    status?: AgentRunStatus,
    externalRunId?: string,
  ) => Promise<void>
  appendDeploymentEvents: (
    deployId: string,
    events: ReadonlyArray<OmniEventRecord>,
    status?: DeploymentStatus,
    externalDeployId?: string,
  ) => Promise<void>
  findAgentRunForUser: (
    userId: string,
    runId: string,
  ) => Promise<AgentRunBundle | undefined>
  findDeploymentForUser: (
    userId: string,
    deployId: string,
  ) => Promise<DeploymentBundle | undefined>
  listAgentRunsForUser: (
    userId: string,
    limit: number,
  ) => Promise<ReadonlyArray<AgentRunBundle>>
  listDeploymentsForUser: (
    userId: string,
    limit: number,
  ) => Promise<ReadonlyArray<DeploymentBundle>>
  saveAgentRun: (
    run: AgentRunRecord,
    events: ReadonlyArray<OmniEventRecord>,
  ) => Promise<void>
  saveDeployment: (
    deployment: DeploymentRecord,
    events: ReadonlyArray<OmniEventRecord>,
  ) => Promise<void>
}>

export type OmniRunStoreHooks = Readonly<{
  afterAgentRunMetered?: (run: AgentRunRecord) => Promise<void>
  /**
   * KS-8.7 (#8318): billing runtime carrying the fail-soft Postgres mirror
   * for the codex-usage debits, billing account ensure, and container
   * usage debits this store writes. Default: plain D1 (no mirror).
   */
  billingRuntime?: BillingRuntime | undefined
}>

export type ShcControlActionResult =
  | Readonly<{
      ok: true
      payload?: unknown
      status: number
    }>
  | Readonly<{
      error: string
      ok: false
      status: number | 'not_configured' | 'not_found'
      targetPath?: string | undefined
    }>

export type ShcControlEventsResult =
  | Readonly<{
      events: ReadonlyArray<Record<string, unknown>>
      nextCursor: number
      ok: true
      payload?: unknown
      status: number
      runStatus?: string | undefined
    }>
  | Readonly<{
      error: string
      ok: false
      status: number | 'not_configured' | 'not_found'
      targetPath?: string | undefined
    }>

export type BillingCanceledAgentRun = Readonly<{
  event: OmniEventRecord
  run: AgentRunRecord
}>

type AgentRunRow = Readonly<{
  id: string
  user_id: string
  team_id: string | null
  project_id?: string | null
  runtime: RunnerRuntime
  backend: RunnerBackend
  runner_id: string
  assignment_kind: 'workroom_agent'
  repository_provider: 'github'
  repository_owner: string
  repository_repo: string
  repository_ref: string
  goal: string
  goal_id?: string | null
  provider_account_ref: string | null
  auth_grant_ref: string | null
  external_run_id: string | null
  status: AgentRunStatus
  event_cursor: number
  assignment_json: string
  created_at: string
  updated_at: string
  started_at: string | null
  completed_at: string | null
  failed_at: string | null
  canceled_at: string | null
  archived_at: string | null
}>

type DeploymentRow = Readonly<{
  id: string
  user_id: string
  team_id: string | null
  service: string
  runtime: RunnerRuntime
  primary_backend: RunnerBackend
  fallback_backend: RunnerBackend
  repository_provider: 'github'
  repository_owner: string
  repository_repo: string
  repository_ref: string
  external_deploy_id: string | null
  status: DeploymentStatus
  event_cursor: number
  assignment_json: string
  created_at: string
  updated_at: string
  started_at: string | null
  completed_at: string | null
  failed_at: string | null
  canceled_at: string | null
}>

type EventRow = Readonly<{
  id: string
  sequence: number
  type: string
  summary: string
  status: string | null
  source: string
  payload_json: string | null
  artifact_refs_json: string
  external_event_id: string | null
  created_at: string
}>

const DEFAULT_RUNNER_ID = 'oa-shc-katy-01'
const DEFAULT_AGENT_RUNTIME: RunnerRuntime = 'opencode_codex'
const DEFAULT_REPOSITORY_REF = 'main'
const DEFAULT_ASSIGNMENT_APP_ORIGIN = 'https://openagents.com'
const SHC_CONTROL_REQUEST_TIMEOUT_MS = 10_000
const DEFAULT_TIMEOUT_MS = 300_000

export class OmniRunValidationError extends S.TaggedErrorClass<OmniRunValidationError>()(
  'OmniRunValidationError',
  {
    message: S.String,
    reason: S.Union([
      S.Literal('unsafe_runner_event_payload'),
      S.Literal('unsafe_artifact_refs'),
      S.Literal('repository_required'),
      S.Literal('repository_invalid'),
    ]),
  },
) {}

const textOrUndefined = (value: string | null): string | undefined =>
  value === null ? undefined : value

const jsonOrNull = (value: unknown | undefined): string | null => {
  if (value === undefined) {
    return null
  }

  const json = JSON.stringify(value)

  if (containsProviderSecretMaterial(json)) {
    throw new OmniRunValidationError({
      message: 'Runner event payload contains credential-shaped material.',
      reason: 'unsafe_runner_event_payload',
    })
  }

  return json
}

const jsonArray = (values: ReadonlyArray<string>): string => {
  const json = JSON.stringify(values)

  if (containsProviderSecretMaterial(json)) {
    throw new OmniRunValidationError({
      message: 'Artifact refs contain credential-shaped material.',
      reason: 'unsafe_artifact_refs',
    })
  }

  return json
}

export type OmniRunRuntime = Readonly<{
  nowIso: () => string
  randomId: (prefix: string) => string
  uuid: () => string
}>

export const systemOmniRunRuntime: OmniRunRuntime = {
  nowIso: currentIsoTimestamp,
  randomId: compactRandomId,
  uuid: randomUuid,
}

export const createOmniId = systemOmniRunRuntime.randomId
export const createAgentRunId = (): string => systemOmniRunRuntime.uuid()

export const legacyAgentRunUuid = (runId: string): string | undefined => {
  const match = /^agent_run_([0-9a-fA-F]{32})$/.exec(runId)
  const hex = match?.[1]?.toLowerCase()

  return hex === undefined
    ? undefined
    : `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}

export const agentRunRouteId = (runId: string): string =>
  legacyAgentRunUuid(runId) ?? runId

export const legacyAgentRunIdFromUuid = (runId: string): string | undefined => {
  const match =
    /^([0-9a-fA-F]{8})-([0-9a-fA-F]{4})-([0-9a-fA-F]{4})-([0-9a-fA-F]{4})-([0-9a-fA-F]{12})$/.exec(
      runId,
    )
  const hex = match?.slice(1).join('').toLowerCase()

  return hex === undefined ? undefined : `agent_run_${hex}`
}

const clampText = (value: string, maxLength: number): string =>
  value.trim().slice(0, maxLength)

const compactText = (
  value: string,
  fallback: string,
  maxLength: number,
): string => {
  const compact = value.replace(/\s+/g, ' ').trim() || fallback

  return compact.length <= maxLength
    ? compact
    : `${compact.slice(0, Math.max(0, maxLength - 3))}...`
}

const missionStatusForSync = (
  status: AgentRunStatus,
): 'active' | 'complete' | 'failed' | 'queued' => {
  if (status === 'running' || status === 'waiting_for_input') {
    return 'active'
  }

  if (status === 'completed') {
    return 'complete'
  }

  if (status === 'failed' || status === 'canceled') {
    return 'failed'
  }

  return 'queued'
}

const safeBranchPart = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._/-]+/g, '-')
    .replace(/\/+/g, '/')
    .replace(/^-+|-+$/g, '')
    .slice(0, 96)

const issueNumberFromUrl = (
  issueUrl: string | undefined,
  repository: RepositoryRef,
): number | undefined => {
  if (issueUrl === undefined) {
    return undefined
  }

  try {
    const url = new URL(issueUrl)
    const parts = url.pathname.split('/').filter(part => part !== '')

    if (
      url.hostname !== 'github.com' ||
      parts[0] !== repository.owner ||
      parts[1] !== repository.repo ||
      parts[2] !== 'issues'
    ) {
      return undefined
    }

    const parsed = Number(parts[3])

    return Number.isInteger(parsed) && parsed > 0 ? parsed : undefined
  } catch {
    return undefined
  }
}

export const parseGithubRepository = (value: string): RepositoryRef => {
  const trimmed = value.trim()

  if (trimmed === '') {
    throw new OmniRunValidationError({
      message: 'Repository is required.',
      reason: 'repository_required',
    })
  }

  let path = trimmed

  if (trimmed.startsWith('https://github.com/')) {
    path = new URL(trimmed).pathname.replace(/^\/+/, '')
  }

  const slashIndex = path.indexOf('/')
  const owner = slashIndex === -1 ? undefined : path.slice(0, slashIndex)
  const repoWithSuffix =
    slashIndex === -1 ? undefined : path.slice(slashIndex + 1)
  const [repoText, refText] = repoWithSuffix?.split('@', 2) ?? []
  const repo = repoText?.replace(/\.git$/, '')
  const ref = refText?.trim() === '' ? undefined : refText?.trim()

  if (
    owner === undefined ||
    repo === undefined ||
    !/^[A-Za-z0-9_.-]+$/.test(owner) ||
    !/^[A-Za-z0-9_.-]+$/.test(repo) ||
    (ref !== undefined && !/^[A-Za-z0-9_.\/-]+$/.test(ref))
  ) {
    throw new OmniRunValidationError({
      message:
        'Repository must be owner/repo, owner/repo@ref, or a GitHub repository URL.',
      reason: 'repository_invalid',
    })
  }

  return {
    provider: 'github',
    owner,
    repo,
    ref: ref ?? DEFAULT_REPOSITORY_REF,
  }
}

export const createGitHubWorkOrder = (
  input: GitHubWorkOrderInput,
): GitHubWorkOrder => {
  const issueNumber =
    input.issueNumber ?? issueNumberFromUrl(input.issueUrl, input.repository)
  const shortRunId = safeBranchPart(input.runId).slice(-24) || 'run'
  const branchName =
    safeBranchPart(input.branchName ?? '') ||
    `openagents/autopilot-${issueNumber ?? shortRunId}`
  const baseRef = clampText(input.baseRef ?? input.repository.ref, 160)
  const commitMessage = clampText(
    input.commitMessage ??
      (issueNumber === undefined
        ? `Complete OpenAgents Autopilot run ${input.runId}`
        : `Address issue #${issueNumber} with OpenAgents Autopilot`),
    240,
  )
  const pullRequestTitle = clampText(
    input.pullRequestTitle ??
      (issueNumber === undefined
        ? `OpenAgents Autopilot changes for ${input.repository.repo}`
        : `Address #${issueNumber}`),
    240,
  )
  const issueUrl =
    input.issueUrl ??
    (issueNumber === undefined
      ? undefined
      : `https://github.com/${input.repository.owner}/${input.repository.repo}/issues/${issueNumber}`)

  return new GitHubWorkOrder({
    provider: 'github',
    repository: input.repository,
    baseRef,
    branchName,
    commitMessage,
    ...(input.issueComment === undefined
      ? {}
      : { issueComment: clampText(input.issueComment, 2_000) }),
    ...(issueNumber === undefined ? {} : { issueNumber }),
    ...(issueUrl === undefined ? {} : { issueUrl: clampText(issueUrl, 500) }),
    ...(input.pullRequestBody === undefined
      ? {}
      : { pullRequestBody: clampText(input.pullRequestBody, 4_000) }),
    pullRequestTitle,
    writeback: {
      commentOnIssue: issueNumber !== undefined,
      openPullRequest: input.openPullRequest ?? issueNumber !== undefined,
      pushBranch: true,
    },
  })
}

const eventRecord = (
  parentId: string,
  sequence: number,
  type: string,
  summary: string,
  options: Readonly<{
    artifactRefs?: ReadonlyArray<string>
    externalEventId?: string
    payload?: unknown
    source?: string
    status?: string
  }> = {},
  runtime: OmniRunRuntime = systemOmniRunRuntime,
): OmniEventRecord => ({
  id: runtime.randomId('omni_event'),
  parentId,
  sequence,
  type,
  summary,
  status: options.status ?? null,
  source: options.source ?? 'openagents',
  payloadJson: jsonOrNull(options.payload),
  artifactRefs: options.artifactRefs ?? [],
  externalEventId: options.externalEventId ?? null,
  createdAt: runtime.nowIso(),
})

const repositoryRefFromRunRow = (row: AgentRunRow): RepositoryRef => ({
  provider: row.repository_provider,
  owner: row.repository_owner,
  repo: row.repository_repo,
  ref: row.repository_ref,
})

const repositoryRefFromDeploymentRow = (row: DeploymentRow): RepositoryRef => ({
  provider: row.repository_provider,
  owner: row.repository_owner,
  repo: row.repository_repo,
  ref: row.repository_ref,
})

const decodeStoredAgentRunAssignment = (
  row: AgentRunRow,
): AgentRunAssignment => {
  try {
    return parseJsonWithSchema(AgentRunAssignment, row.assignment_json)
  } catch {
    return buildAgentRunAssignment({
      appOrigin: DEFAULT_ASSIGNMENT_APP_ORIGIN,
      authGrantRef: textOrUndefined(row.auth_grant_ref),
      backend: row.backend,
      goal: row.goal,
      providerAccountRef: textOrUndefined(row.provider_account_ref),
      repository: repositoryRefFromRunRow(row),
      runId: row.id,
      runtime: row.runtime,
    })
  }
}

const decodeStoredDeploymentAssignment = (
  row: DeploymentRow,
): AppDeployAssignment => {
  try {
    return parseJsonWithSchema(AppDeployAssignment, row.assignment_json)
  } catch {
    return buildAppDeployAssignment({
      appOrigin: DEFAULT_ASSIGNMENT_APP_ORIGIN,
      deployId: row.id,
      repository: repositoryRefFromDeploymentRow(row),
    })
  }
}

const toAgentRunRecord = (row: AgentRunRow): AgentRunRecord => ({
  id: row.id,
  userId: row.user_id,
  teamId: row.team_id,
  projectId: row.project_id ?? null,
  runtime: row.runtime,
  backend: row.backend,
  runnerId: row.runner_id,
  assignmentKind: row.assignment_kind,
  repository: repositoryRefFromRunRow(row),
  goal: row.goal,
  goalId: row.goal_id ?? null,
  providerAccountRef: row.provider_account_ref,
  authGrantRef: row.auth_grant_ref,
  externalRunId: row.external_run_id,
  status: row.status,
  eventCursor: row.event_cursor,
  assignment: decodeStoredAgentRunAssignment(row),
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  startedAt: row.started_at,
  completedAt: row.completed_at,
  failedAt: row.failed_at,
  canceledAt: row.canceled_at,
  archivedAt: row.archived_at,
})

const toDeploymentRecord = (row: DeploymentRow): DeploymentRecord => ({
  id: row.id,
  userId: row.user_id,
  teamId: row.team_id,
  service: row.service,
  runtime: row.runtime,
  primaryBackend: row.primary_backend,
  fallbackBackend: row.fallback_backend,
  repository: repositoryRefFromDeploymentRow(row),
  externalDeployId: row.external_deploy_id,
  status: row.status,
  eventCursor: row.event_cursor,
  assignment: decodeStoredDeploymentAssignment(row),
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  startedAt: row.started_at,
  completedAt: row.completed_at,
  failedAt: row.failed_at,
  canceledAt: row.canceled_at,
})

const toEventRecord = (parentId: string, row: EventRow): OmniEventRecord => ({
  id: row.id,
  parentId,
  sequence: row.sequence,
  type: row.type,
  summary: row.summary,
  status: row.status,
  source: row.source,
  payloadJson: row.payload_json,
  artifactRefs: parseJsonStringArray(row.artifact_refs_json),
  externalEventId: row.external_event_id,
  createdAt: row.created_at,
})

export const readAgentRunById = async (
  db: D1Database,
  runId: string,
): Promise<AgentRunRecord | undefined> => {
  const row = await db
    .prepare(`SELECT * FROM agent_runs WHERE id = ?`)
    .bind(runId)
    .first<AgentRunRow>()

  return row === null ? undefined : toAgentRunRecord(row)
}

export const listActiveAgentRunsForBilling = async (
  db: D1Database,
  limit = 100,
): Promise<ReadonlyArray<AgentRunRecord>> => {
  const rows = await db
    .prepare(
      `SELECT *
       FROM agent_runs
       WHERE status IN ('queued', 'running', 'waiting_for_input')
         AND started_at IS NOT NULL
       ORDER BY updated_at ASC
       LIMIT ?`,
    )
    .bind(Math.max(1, Math.min(500, Math.trunc(limit))))
    .all<AgentRunRow>()

  return rows.results.map(toAgentRunRecord)
}

const repositoryText = (run: AgentRunRecord): string =>
  `${run.repository.owner}/${run.repository.repo}`

export const agentRunMissionProjection = (run: AgentRunRecord) => {
  const routeId = agentRunRouteId(run.id)
  const repository = repositoryText(run)
  const owner =
    run.projectId === null
      ? run.teamId === null
        ? 'personal'
        : 'team'
      : 'project'

  return {
    detail: `${repository} - ${run.status}`,
    href: `/t/${encodeURIComponent(routeId)}`,
    id: routeId,
    owner,
    ownerUserId: run.userId,
    projectId: run.projectId,
    repository,
    runId: run.id,
    status: missionStatusForSync(run.status),
    teamId: run.teamId,
    threadId: routeId,
    title: compactText(run.goal, routeId, 72),
    updatedAt: run.updatedAt,
  }
}

export const agentRunProjection = (run: AgentRunRecord) => ({
  backend: run.backend,
  completedAt: run.completedAt,
  createdAt: run.createdAt,
  eventCursor: run.eventCursor,
  externalRunId: run.externalRunId,
  failedAt: run.failedAt,
  goalId: run.goalId,
  goal: run.goal,
  id: run.id,
  repository: {
    owner: run.repository.owner,
    provider: run.repository.provider,
    ref: run.repository.ref,
    repo: run.repository.repo,
  },
  routeId: agentRunRouteId(run.id),
  runnerId: run.runnerId,
  runtime: run.runtime,
  startedAt: run.startedAt,
  status: run.status,
  teamId: run.teamId,
  projectId: run.projectId,
  updatedAt: run.updatedAt,
  userId: run.userId,
})

const agentRunEventProjection = (event: OmniEventRecord) => ({
  artifactRefs: event.artifactRefs,
  createdAt: event.createdAt,
  externalEventId: event.externalEventId,
  id: event.id,
  payloadJson: event.payloadJson,
  runId: event.parentId,
  sequence: event.sequence,
  source: event.source,
  status: event.status,
  summary: event.summary,
  type: event.type,
})

const agentRunSyncChanges = (
  run: AgentRunRecord,
  events: ReadonlyArray<OmniEventRecord>,
): ReadonlyArray<SyncChangeInput> => {
  const routeId = agentRunRouteId(run.id)
  const mission = agentRunMissionProjection(run)
  const publicRun = agentRunProjection(run)
  const teamMissionScope =
    run.teamId === null ? undefined : teamScope(run.teamId)
  const teamMissionDelete =
    teamMissionScope === undefined
      ? []
      : [
          {
            actorId: run.userId,
            collection: 'missions',
            id: routeId,
            op: 'delete' as const,
            scope: teamMissionScope,
            serverTime: run.archivedAt ?? run.updatedAt,
          },
        ]
  const teamMissionPut =
    teamMissionScope === undefined
      ? []
      : [
          {
            actorId: run.userId,
            collection: 'missions',
            id: routeId,
            op: 'put' as const,
            scope: teamMissionScope,
            serverTime: run.updatedAt,
            value: mission,
          },
        ]
  const eventChanges = events.flatMap(event => [
    {
      collection: 'agent_run_events',
      id: event.id,
      op: 'put' as const,
      scope: agentRunScope(run.id),
      serverTime: event.createdAt,
      value: agentRunEventProjection(event),
    },
    {
      collection: 'agent_run_events',
      id: event.id,
      op: 'put' as const,
      scope: threadScope(routeId),
      serverTime: event.createdAt,
      value: agentRunEventProjection(event),
    },
  ])

  if (run.archivedAt !== null) {
    return [
      {
        actorId: run.userId,
        collection: 'missions',
        id: routeId,
        op: 'delete',
        scope: personalWorkroomScope(run.userId),
        serverTime: run.archivedAt,
      },
      {
        actorId: run.userId,
        collection: 'agent_runs',
        id: run.id,
        op: 'delete',
        scope: personalWorkroomScope(run.userId),
        serverTime: run.archivedAt,
      },
      {
        actorId: run.userId,
        collection: 'agent_runs',
        id: run.id,
        op: 'delete',
        scope: agentRunScope(run.id),
        serverTime: run.archivedAt,
      },
      {
        actorId: run.userId,
        collection: 'agent_runs',
        id: run.id,
        op: 'delete',
        scope: threadScope(routeId),
        serverTime: run.archivedAt,
      },
      ...teamMissionDelete,
    ]
  }

  return [
    {
      actorId: run.userId,
      collection: 'missions',
      id: routeId,
      op: 'put',
      scope: personalWorkroomScope(run.userId),
      serverTime: run.updatedAt,
      value: mission,
    },
    ...teamMissionPut,
    {
      actorId: run.userId,
      collection: 'agent_runs',
      id: run.id,
      op: 'put',
      scope: personalWorkroomScope(run.userId),
      serverTime: run.updatedAt,
      value: publicRun,
    },
    {
      actorId: run.userId,
      collection: 'agent_runs',
      id: run.id,
      op: 'put',
      scope: agentRunScope(run.id),
      serverTime: run.updatedAt,
      value: publicRun,
    },
    {
      actorId: run.userId,
      collection: 'agent_runs',
      id: run.id,
      op: 'put',
      scope: threadScope(routeId),
      serverTime: run.updatedAt,
      value: publicRun,
    },
    ...eventChanges,
  ]
}

const appendAgentRunSyncChanges = async (
  db: D1Database,
  run: AgentRunRecord,
  events: ReadonlyArray<OmniEventRecord>,
): Promise<void> => {
  await makeD1SyncOutboxRepository(db).appendChanges(
    agentRunSyncChanges(run, events),
  )
}

const agentRunInsert = (
  db: D1Database,
  run: AgentRunRecord,
): D1PreparedStatement =>
  db
    .prepare(
      `INSERT INTO agent_runs
        (id, user_id, team_id, project_id, runtime, backend, runner_id, assignment_kind,
         repository_provider, repository_owner, repository_repo, repository_ref,
         goal, goal_id, provider_account_ref, auth_grant_ref, external_run_id, status,
         event_cursor, assignment_json, created_at, updated_at, started_at,
         completed_at, failed_at, canceled_at, archived_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      run.id,
      run.userId,
      run.teamId,
      run.projectId,
      run.runtime,
      run.backend,
      run.runnerId,
      run.assignmentKind,
      run.repository.provider,
      run.repository.owner,
      run.repository.repo,
      run.repository.ref,
      run.goal,
      run.goalId,
      run.providerAccountRef,
      run.authGrantRef,
      run.externalRunId,
      run.status,
      run.eventCursor,
      JSON.stringify(run.assignment),
      run.createdAt,
      run.updatedAt,
      run.startedAt,
      run.completedAt,
      run.failedAt,
      run.canceledAt,
      run.archivedAt,
    )

const deploymentInsert = (
  db: D1Database,
  deployment: DeploymentRecord,
): D1PreparedStatement =>
  db
    .prepare(
      `INSERT INTO deployments
        (id, user_id, team_id, service, runtime, primary_backend, fallback_backend,
         repository_provider, repository_owner, repository_repo, repository_ref,
         external_deploy_id, status, event_cursor, assignment_json, created_at,
         updated_at, started_at, completed_at, failed_at, canceled_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      deployment.id,
      deployment.userId,
      deployment.teamId,
      deployment.service,
      deployment.runtime,
      deployment.primaryBackend,
      deployment.fallbackBackend,
      deployment.repository.provider,
      deployment.repository.owner,
      deployment.repository.repo,
      deployment.repository.ref,
      deployment.externalDeployId,
      deployment.status,
      deployment.eventCursor,
      JSON.stringify(deployment.assignment),
      deployment.createdAt,
      deployment.updatedAt,
      deployment.startedAt,
      deployment.completedAt,
      deployment.failedAt,
      deployment.canceledAt,
    )

const agentEventInsert = (
  db: D1Database,
  event: OmniEventRecord,
): D1PreparedStatement =>
  db
    .prepare(
      `INSERT OR IGNORE INTO agent_run_events
        (id, run_id, sequence, type, summary, status, source, payload_json,
         artifact_refs_json, external_event_id, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      event.id,
      event.parentId,
      event.sequence,
      event.type,
      event.summary,
      event.status,
      event.source,
      event.payloadJson,
      jsonArray(event.artifactRefs),
      event.externalEventId,
      event.createdAt,
    )

const deploymentEventInsert = (
  db: D1Database,
  event: OmniEventRecord,
): D1PreparedStatement =>
  db
    .prepare(
      `INSERT OR IGNORE INTO deployment_events
        (id, deploy_id, sequence, type, summary, status, source, payload_json,
         artifact_refs_json, external_event_id, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      event.id,
      event.parentId,
      event.sequence,
      event.type,
      event.summary,
      event.status,
      event.source,
      event.payloadJson,
      jsonArray(event.artifactRefs),
      event.externalEventId,
      event.createdAt,
    )

export const cancelActiveAgentRunsForBillingExhaustion = async (
  db: D1Database,
  userId: string,
  input: Readonly<{
    balanceCents: number
    balanceFormatted: string
  }>,
): Promise<ReadonlyArray<BillingCanceledAgentRun>> => {
  const rows = await db
    .prepare(
      `SELECT *
       FROM agent_runs
       WHERE user_id = ?
         AND status IN ('queued', 'running', 'waiting_for_input')
       ORDER BY updated_at ASC`,
    )
    .bind(userId)
    .all<AgentRunRow>()
  const activeRuns = rows.results.map(toAgentRunRecord)

  if (activeRuns.length === 0) {
    return []
  }

  const canceled = activeRuns.map(run => {
    const event = eventRecord(
      run.id,
      run.eventCursor + 1,
      'billing.credits_exhausted',
      'Credits reached zero; OpenAgents stopped this computer.',
      {
        externalEventId: `billing:credits-exhausted:${run.id}`,
        payload: {
          balanceCents: input.balanceCents,
          balanceFormatted: input.balanceFormatted,
          backend: run.backend,
          externalRunId: run.externalRunId,
          reason: 'credits_exhausted',
          runnerId: run.runnerId,
        },
        source: 'billing',
        status: 'canceled',
      },
    )
    const canceledRun: AgentRunRecord = {
      ...run,
      canceledAt: run.canceledAt ?? event.createdAt,
      eventCursor: Math.max(run.eventCursor, event.sequence),
      status: 'canceled',
      updatedAt: event.createdAt,
    }

    return { event, run: canceledRun }
  })

  await db.batch(
    canceled.flatMap(item => [
      agentEventInsert(db, item.event),
      db
        .prepare(
          `UPDATE agent_runs
           SET status = 'canceled',
               event_cursor = MAX(event_cursor, ?),
               updated_at = ?,
               canceled_at = COALESCE(canceled_at, ?)
           WHERE id = ?
             AND status IN ('queued', 'running', 'waiting_for_input')`,
        )
        .bind(
          item.event.sequence,
          item.event.createdAt,
          item.event.createdAt,
          item.run.id,
        ),
    ]),
  )

  await makeD1SyncOutboxRepository(db).appendChanges(
    canceled.flatMap(item => agentRunSyncChanges(item.run, [item.event])),
  )

  return canceled
}

const tokenUsageInsert = (
  db: D1Database,
  input: Readonly<{
    event: OmniEventRecord
    providerAccountRef: string | null
    teamId: string | null
    userId: string
  }>,
): D1PreparedStatement | undefined => {
  const usage = tokenUsageFromEvent(input.event)

  if (usage === undefined) {
    return undefined
  }

  // Attribution: a run launched against an M8/M9 provider-account lease carries
  // that lease's provider_account_ref on `agent_runs`, so the leaderboard usage
  // row is attributed to it. Runs with no lease ref are recorded under the typed
  // 'unattributed' sentinel rather than faking a provider-account total.
  const attribution = resolveTokenUsageAccountAttribution(
    input.providerAccountRef,
  )

  return db
    .prepare(
      `INSERT OR IGNORE INTO autopilot_token_usage
        (id, run_id, event_id, user_id, team_id, account_ref, provider, model,
         input_tokens, output_tokens, reasoning_tokens, cache_read_tokens,
         cache_write_5m_tokens, cache_write_1h_tokens, total_tokens, source,
         source_ref, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      systemOmniRunRuntime.randomId('token_usage'),
      input.event.parentId,
      input.event.id,
      input.userId,
      input.teamId,
      attribution.accountRef,
      usage.provider,
      usage.model,
      usage.inputTokens,
      usage.outputTokens,
      usage.reasoningTokens,
      usage.cacheReadTokens,
      usage.cacheWrite5mTokens,
      usage.cacheWrite1hTokens,
      usage.totalTokens,
      input.event.source,
      sourceRefForTokenUsageEvent(input.event),
      input.event.createdAt,
    )
}

const readAgentRunEvents = async (
  db: D1Database,
  runId: string,
): Promise<ReadonlyArray<OmniEventRecord>> => {
  const rows = await db
    .prepare(
      `SELECT id, sequence, type, summary, status, source, payload_json,
              artifact_refs_json, external_event_id, created_at
       FROM agent_run_events
       WHERE run_id = ?
       ORDER BY sequence ASC`,
    )
    .bind(runId)
    .all<EventRow>()

  return rows.results.map(row => toEventRecord(runId, row))
}

const readDeploymentEvents = async (
  db: D1Database,
  deployId: string,
): Promise<ReadonlyArray<OmniEventRecord>> => {
  const rows = await db
    .prepare(
      `SELECT id, sequence, type, summary, status, source, payload_json,
              artifact_refs_json, external_event_id, created_at
       FROM deployment_events
       WHERE deploy_id = ?
       ORDER BY sequence ASC`,
    )
    .bind(deployId)
    .all<EventRow>()

  return rows.results.map(row => toEventRecord(deployId, row))
}

export const makeD1OmniRunStore = (
  db: D1Database,
  hooks: OmniRunStoreHooks = {},
): OmniRunStore => ({
  appendAgentRunEvents: async (runId, events, status, externalRunId) => {
    const now = systemOmniRunRuntime.nowIso()
    const maxSequence = Math.max(0, ...events.map(event => event.sequence))
    const usageEvents = events.filter(
      event => tokenUsageFromEvent(event) !== undefined,
    )
    const runRow =
      usageEvents.length === 0
        ? undefined
        : await db
            .prepare(
              `SELECT user_id, team_id, provider_account_ref FROM agent_runs WHERE id = ?`,
            )
            .bind(runId)
            .first<
              Readonly<{
                provider_account_ref: string | null
                team_id: string | null
                user_id: string
              }>
            >()
    const tokenUsageInserts =
      runRow === null || runRow === undefined
        ? []
        : usageEvents
            .map(event =>
              tokenUsageInsert(db, {
                event,
                providerAccountRef: runRow.provider_account_ref,
                teamId: runRow.team_id,
                userId: runRow.user_id,
              }),
            )
            .filter(
              (statement): statement is D1PreparedStatement =>
                statement !== undefined,
            )
    const codexBillingInserts =
      runRow === null || runRow === undefined
        ? []
        : usageEvents
            .map(event =>
              codexUsageDebitInsert(db, {
                event,
                teamId: runRow.team_id,
                userId: runRow.user_id,
              }),
            )
            .filter(
              (statement): statement is D1PreparedStatement =>
                statement !== undefined,
            )

    if (runRow !== null && runRow !== undefined) {
      await ensureBillingAccount(db, runRow.user_id, hooks.billingRuntime)
    }

    await db.batch([
      ...events.map(event => agentEventInsert(db, event)),
      ...tokenUsageInserts,
      ...codexBillingInserts,
      db
        .prepare(
          `UPDATE agent_runs
           SET status = COALESCE(?, status),
               external_run_id = COALESCE(?, external_run_id),
               event_cursor = MAX(event_cursor, ?),
               updated_at = ?,
               started_at = CASE WHEN ? = 'running' THEN COALESCE(started_at, ?) ELSE started_at END,
               completed_at = CASE WHEN ? = 'completed' THEN COALESCE(completed_at, ?) ELSE completed_at END,
               failed_at = CASE WHEN ? = 'failed' THEN COALESCE(failed_at, ?) ELSE failed_at END,
               canceled_at = CASE WHEN ? = 'canceled' THEN COALESCE(canceled_at, ?) ELSE canceled_at END
           WHERE id = ?`,
        )
        .bind(
          status ?? null,
          externalRunId ?? null,
          maxSequence,
          now,
          status ?? null,
          now,
          status ?? null,
          now,
          status ?? null,
          now,
          status ?? null,
          now,
          runId,
        ),
    ])

    // KS-8.7: mirror the codex-usage debit ledger rows the batch above
    // just inserted (fail-soft, by their deterministic idempotency keys).
    if (
      runRow !== null &&
      runRow !== undefined &&
      hooks.billingRuntime?.mirror !== undefined &&
      codexBillingInserts.length > 0
    ) {
      const refs = usageEvents
        .map(event => codexUsageDebitMirrorRef(event))
        .filter(ref => ref !== undefined)
      if (refs.length > 0) {
        await hooks.billingRuntime.mirror(db, refs)
      }
    }

    const run = await readAgentRunById(db, runId)

    if (run !== undefined) {
      await recordContainerUsageDebitForRun(db, run, {}, hooks.billingRuntime)
      await appendAgentRunSyncChanges(db, run, events)
      await hooks.afterAgentRunMetered?.(run)
    }
  },
  appendDeploymentEvents: async (
    deployId,
    events,
    status,
    externalDeployId,
  ) => {
    const now = systemOmniRunRuntime.nowIso()
    const maxSequence = Math.max(0, ...events.map(event => event.sequence))
    await db.batch([
      ...events.map(event => deploymentEventInsert(db, event)),
      db
        .prepare(
          `UPDATE deployments
           SET status = COALESCE(?, status),
               external_deploy_id = COALESCE(?, external_deploy_id),
               event_cursor = MAX(event_cursor, ?),
               updated_at = ?,
               started_at = CASE WHEN ? = 'running' THEN COALESCE(started_at, ?) ELSE started_at END,
               completed_at = CASE WHEN ? IN ('promoted', 'rolled_back') THEN COALESCE(completed_at, ?) ELSE completed_at END,
               failed_at = CASE WHEN ? = 'failed' THEN COALESCE(failed_at, ?) ELSE failed_at END,
               canceled_at = CASE WHEN ? = 'canceled' THEN COALESCE(canceled_at, ?) ELSE canceled_at END
           WHERE id = ?`,
        )
        .bind(
          status ?? null,
          externalDeployId ?? null,
          maxSequence,
          now,
          status ?? null,
          now,
          status ?? null,
          now,
          status ?? null,
          now,
          status ?? null,
          now,
          deployId,
        ),
    ])
  },
  findAgentRunForUser: async (userId, runId) => {
    const row = await db
      .prepare(
        `SELECT * FROM agent_runs
         WHERE user_id = ?
           AND id = ?
           AND archived_at IS NULL`,
      )
      .bind(userId, runId)
      .first<AgentRunRow>()

    if (row === null) {
      return undefined
    }

    return {
      run: toAgentRunRecord(row),
      events: await readAgentRunEvents(db, runId),
    }
  },
  findDeploymentForUser: async (userId, deployId) => {
    const row = await db
      .prepare(`SELECT * FROM deployments WHERE user_id = ? AND id = ?`)
      .bind(userId, deployId)
      .first<DeploymentRow>()

    if (row === null) {
      return undefined
    }

    return {
      deployment: toDeploymentRecord(row),
      events: await readDeploymentEvents(db, deployId),
    }
  },
  listAgentRunsForUser: async (userId, limit) => {
    const rows = await db
      .prepare(
        `SELECT * FROM agent_runs
         WHERE user_id = ?
           AND archived_at IS NULL
         ORDER BY created_at DESC
         LIMIT ?`,
      )
      .bind(userId, limit)
      .all<AgentRunRow>()

    return Promise.all(
      rows.results.map(async row => ({
        run: toAgentRunRecord(row),
        events: await readAgentRunEvents(db, row.id),
      })),
    )
  },
  listDeploymentsForUser: async (userId, limit) => {
    const rows = await db
      .prepare(
        `SELECT * FROM deployments
         WHERE user_id = ?
         ORDER BY created_at DESC
         LIMIT ?`,
      )
      .bind(userId, limit)
      .all<DeploymentRow>()

    return Promise.all(
      rows.results.map(async row => ({
        deployment: toDeploymentRecord(row),
        events: await readDeploymentEvents(db, row.id),
      })),
    )
  },
  saveAgentRun: async (run, events) => {
    await db.batch([
      agentRunInsert(db, run),
      ...events.map(event => agentEventInsert(db, event)),
    ])
    await appendAgentRunSyncChanges(db, run, events)
  },
  saveDeployment: async (deployment, events) => {
    await db.batch([
      deploymentInsert(db, deployment),
      ...events.map(event => deploymentEventInsert(db, event)),
    ])
  },
})

const callbackUrl = (appOrigin: string, path: string, id: string): string =>
  `${appOrigin.replace(/\/+$/, '')}${path}/${encodeURIComponent(id)}/events/ingest`

export const buildAgentRunAssignment = (
  input: Readonly<{
    appOrigin: string
    authGrantRef?: string | undefined
    backend?: RunnerBackend | undefined
    blueprint?: BlueprintAssignmentScope | undefined
    dispatchGoal?: string | undefined
    githubWriteConnectionRef?: string | undefined
    githubWriteGrantRef?: string | undefined
    githubWorkOrder?: GitHubWorkOrder | undefined
    goal: string
    goalId?: string | undefined
    goalStatus?: string | undefined
    goalVisibility?: 'private' | 'team' | 'public' | undefined
    providerAccountRef?: string | undefined
    repository: RepositoryRef
    runId: string
    runtime?: RunnerRuntime | undefined
    timeUsedSeconds?: number | undefined
    timeoutMs?: number | undefined
    tokenBudget?: number | null | undefined
    tokensUsed?: number | undefined
  }>,
): AgentRunAssignment => {
  const blueprint =
    input.blueprint === undefined
      ? undefined
      : assertProbeBlueprintAssignmentScopeSafe(input.blueprint)

  return new AgentRunAssignment({
    schemaVersion: 'openagents.agent_run_assignment.v1',
    runId: input.runId,
    runtime: input.runtime ?? DEFAULT_AGENT_RUNTIME,
    backend: input.backend ?? 'shc_vm',
    assignmentKind: 'workroom_agent',
    goal: clampText(input.dispatchGoal ?? input.goal, 8_000),
    repository: input.repository,
    ...(input.providerAccountRef === undefined
      ? {}
      : { providerAccountRef: input.providerAccountRef }),
    ...(input.authGrantRef === undefined
      ? {}
      : { authGrantRef: input.authGrantRef }),
    ...(input.githubWriteConnectionRef === undefined
      ? {}
      : { githubWriteConnectionRef: input.githubWriteConnectionRef }),
    ...(input.githubWriteGrantRef === undefined
      ? {}
      : { githubWriteGrantRef: input.githubWriteGrantRef }),
    ...(input.githubWorkOrder === undefined
      ? {}
      : { githubWorkOrder: input.githubWorkOrder }),
    ...(input.goalId === undefined
      ? {}
      : {
          goalContext: buildAgentGoalAssignmentContext({
            goalId: input.goalId,
            objective: input.goal,
            status: input.goalStatus ?? 'active',
            timeUsedSeconds: input.timeUsedSeconds,
            tokenBudget: input.tokenBudget,
            tokensUsed: input.tokensUsed,
            visibility: input.goalVisibility,
          }),
        }),
    modelProfile: {
      kind: 'codex',
      provider: 'openai',
      model: 'default',
    },
    sandbox: {
      mode: 'workspace_write',
      network: 'restricted',
      timeoutMs: input.timeoutMs ?? DEFAULT_TIMEOUT_MS,
    },
    artifactPolicy: 'redacted_logs',
    retentionMode: 'openagents_durable',
    trainingUse: 'denied',
    callback: {
      url: callbackUrl(input.appOrigin, '/api/omni/agent-runs', input.runId),
      tokenRef: 'runner_callback_token',
    },
    ...(blueprint === undefined ? {} : { blueprint }),
  })
}

export const buildAppDeployAssignment = (
  input: Readonly<{
    appOrigin: string
    deployId: string
    repository: RepositoryRef
  }>,
): AppDeployAssignment =>
  new AppDeployAssignment({
    schemaVersion: 'openagents.app_deploy_assignment.v1',
    deployId: input.deployId,
    runtime: DEFAULT_AGENT_RUNTIME,
    primaryBackend: 'shc_vm',
    fallbackBackend: 'gcloud_vm',
    service: 'openagents-autopilot',
    repository: input.repository,
    commands: {
      install: 'bun install --frozen-lockfile',
      typecheck: 'bun run typecheck',
      test: 'bun run test',
      build: 'bun run build',
      smoke: 'bun run smoke:shc',
    },
    callback: {
      url: callbackUrl(
        input.appOrigin,
        '/api/omni/deployments',
        input.deployId,
      ),
      tokenRef: 'runner_callback_token',
    },
    rollback: {
      retainPreviousRelease: true,
      healthCheckUrl: `${input.appOrigin.replace(/\/+$/, '')}/api/health`,
    },
  })

export const createQueuedAgentRun = (
  input: Readonly<{
    appOrigin: string
    authGrantRef?: string | undefined
    backend?: RunnerBackend | undefined
    blueprint?: BlueprintAssignmentScope | undefined
    dispatchGoal?: string | undefined
    githubWriteConnectionRef?: string | undefined
    githubWriteGrantRef?: string | undefined
    githubWorkOrder?: GitHubWorkOrder | undefined
    goal: string
    goalId?: string | undefined
    goalStatus?: string | undefined
    goalVisibility?: 'private' | 'team' | 'public' | undefined
    providerAccountRef?: string | undefined
    projectId?: string | undefined
    repository: RepositoryRef
    runId?: string | undefined
    omniRuntime?: OmniRunRuntime | undefined
    teamId?: string | undefined
    runtime?: RunnerRuntime | undefined
    timeUsedSeconds?: number | undefined
    timeoutMs?: number | undefined
    tokenBudget?: number | null | undefined
    tokensUsed?: number | undefined
    userId: string
  }>,
): Readonly<{
  events: ReadonlyArray<OmniEventRecord>
  run: AgentRunRecord
}> => {
  const omniRuntime = input.omniRuntime ?? systemOmniRunRuntime
  const id = input.runId ?? omniRuntime.uuid()
  const now = omniRuntime.nowIso()
  const assignment = buildAgentRunAssignment({
    appOrigin: input.appOrigin,
    authGrantRef: input.authGrantRef,
    backend: input.backend,
    blueprint: input.blueprint,
    dispatchGoal: input.dispatchGoal,
    githubWriteConnectionRef: input.githubWriteConnectionRef,
    githubWriteGrantRef: input.githubWriteGrantRef,
    githubWorkOrder: input.githubWorkOrder,
    goal: input.goal,
    goalId: input.goalId,
    goalStatus: input.goalStatus,
    goalVisibility: input.goalVisibility,
    providerAccountRef: input.providerAccountRef,
    repository: input.repository,
    runId: id,
    runtime: input.runtime,
    timeUsedSeconds: input.timeUsedSeconds,
    timeoutMs: input.timeoutMs,
    tokenBudget: input.tokenBudget,
    tokensUsed: input.tokensUsed,
  })
  const run: AgentRunRecord = {
    id,
    userId: input.userId,
    teamId: input.teamId ?? null,
    projectId: input.projectId ?? null,
    runtime: assignment.runtime,
    backend: assignment.backend,
    runnerId: DEFAULT_RUNNER_ID,
    assignmentKind: 'workroom_agent',
    repository: input.repository,
    goal: clampText(input.goal, 8_000),
    goalId: input.goalId ?? null,
    providerAccountRef: input.providerAccountRef ?? null,
    authGrantRef: input.authGrantRef ?? null,
    externalRunId: null,
    status: 'queued',
    eventCursor: 1,
    assignment,
    createdAt: now,
    updatedAt: now,
    startedAt: null,
    completedAt: null,
    failedAt: null,
    canceledAt: null,
    archivedAt: null,
  }

  return {
    run,
    events: [
      eventRecord(
        id,
        1,
        'agent_run.accepted',
        'OpenAgents accepted the Autopilot assignment for computer dispatch.',
        {
          payload: {
            backend: run.backend,
            repository: `${run.repository.owner}/${run.repository.repo}`,
            runtime: run.runtime,
          },
          status: 'queued',
        },
        omniRuntime,
      ),
    ],
  }
}

export const createQueuedDeployment = (
  input: Readonly<{
    appOrigin: string
    repository: RepositoryRef
    userId: string
  }>,
): Readonly<{
  deployment: DeploymentRecord
  events: ReadonlyArray<OmniEventRecord>
}> => {
  const id = systemOmniRunRuntime.randomId('deploy')
  const now = systemOmniRunRuntime.nowIso()
  const assignment = buildAppDeployAssignment({
    appOrigin: input.appOrigin,
    deployId: id,
    repository: input.repository,
  })
  const deployment: DeploymentRecord = {
    id,
    userId: input.userId,
    teamId: null,
    service: assignment.service,
    runtime: assignment.runtime,
    primaryBackend: assignment.primaryBackend,
    fallbackBackend: assignment.fallbackBackend,
    repository: input.repository,
    externalDeployId: null,
    status: 'queued',
    eventCursor: 1,
    assignment,
    createdAt: now,
    updatedAt: now,
    startedAt: null,
    completedAt: null,
    failedAt: null,
    canceledAt: null,
  }

  return {
    deployment,
    events: [
      eventRecord(
        id,
        1,
        'deploy.accepted',
        'OpenAgents accepted the computer deploy assignment.',
        {
          payload: {
            primaryBackend: deployment.primaryBackend,
            repository: `${deployment.repository.owner}/${deployment.repository.repo}`,
            service: deployment.service,
          },
          status: 'queued',
        },
      ),
    ],
  }
}

const controlCandidates = (controlUrl: string): ReadonlyArray<string> => {
  const base = new URL(controlUrl)
  const path = base.pathname.replace(/\/+$/, '') || ''
  const candidates: Array<string> = []
  const push = (pathname: string): void => {
    const candidate = new URL(base)
    candidate.pathname = pathname.startsWith('/') ? pathname : `/${pathname}`
    const href = candidate.toString()

    if (!candidates.includes(href)) {
      candidates.push(href)
    }
  }

  if (path.endsWith('/codex-runs')) {
    push(path)
  } else if (path.endsWith('/v1')) {
    push(`${path}/codex-runs`)
  } else {
    push(`${path}/v1/codex-runs`)
    push(path)
  }

  return candidates
}

const controlActionCandidates = (
  controlUrl: string,
  action: 'cancel' | 'continue' | 'start' | 'steer',
): ReadonlyArray<string> => {
  const base = new URL(controlUrl)
  const path = base.pathname.replace(/\/+$/, '') || ''
  const candidates: Array<string> = []
  const push = (pathname: string): void => {
    const candidate = new URL(base)
    candidate.pathname = pathname.startsWith('/') ? pathname : `/${pathname}`
    const href = candidate.toString()

    if (!candidates.includes(href)) {
      candidates.push(href)
    }
  }

  if (path.endsWith(`/codex-runs/${action}`)) {
    push(path)

    return candidates
  }

  if (path.endsWith('/codex-runs')) {
    push(`${path}/${action}`)
    push(path)

    return candidates
  }

  if (path.endsWith('/v1')) {
    push(`${path}/codex-runs/${action}`)
    push(`${path}/codex-runs`)
    push(`${path}/${action}`)
    push(path)

    return candidates
  }

  push(`${path}/v1/codex-runs/${action}`)
  push(`${path}/v1/codex-runs`)
  push(`${path}/${action}`)
  push(path)

  return candidates
}

const controlRunActionCandidates = (
  controlUrl: string,
  runId: string,
  action: 'events' | 'stream' | 'turns',
): ReadonlyArray<string> => {
  const base = new URL(controlUrl)
  const path = base.pathname.replace(/\/+$/, '') || ''
  const candidates: Array<string> = []
  const push = (pathname: string): void => {
    const candidate = new URL(base)
    candidate.pathname = pathname.startsWith('/') ? pathname : `/${pathname}`
    const href = candidate.toString()

    if (!candidates.includes(href)) {
      candidates.push(href)
    }
  }

  if (path.endsWith('/codex-runs')) {
    push(`${path}/${encodeURIComponent(runId)}/${action}`)

    return candidates
  }

  if (path.endsWith('/v1')) {
    push(`${path}/codex-runs/${encodeURIComponent(runId)}/${action}`)

    return candidates
  }

  push(`${path}/v1/codex-runs/${encodeURIComponent(runId)}/${action}`)
  push(`${path}/codex-runs/${encodeURIComponent(runId)}/${action}`)

  return candidates
}

const controlHealthCandidates = (controlUrl: string): ReadonlyArray<string> => {
  const base = new URL(controlUrl)
  const candidates: Array<string> = []
  const push = (pathname: string): void => {
    const candidate = new URL(base)
    candidate.pathname = pathname.startsWith('/') ? pathname : `/${pathname}`
    candidate.search = ''
    const href = candidate.toString()

    if (!candidates.includes(href)) {
      candidates.push(href)
    }
  }

  push('/healthz')
  push('/health')

  return candidates
}

const optionalRecord = recordFromUnknown

const optionalText = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

const responseErrorSnippet = (text: string): string => {
  const compact = text.replace(/\s+/g, ' ').trim()

  if (compact === '') {
    return ''
  }

  return ` ${compact.slice(0, 240)}`
}

const safeDispatchMessage = (text: string, fallback: string): string => {
  const compact = text.replace(/\s+/g, ' ').trim()

  if (compact === '' || containsProviderSecretMaterial(compact)) {
    return fallback
  }

  return compact.slice(0, 240)
}

const optionalNumber = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) ? value : undefined

const parseOptionalShcPayload = (
  text: string,
): Record<string, unknown> | undefined => {
  if (text.trim() === '') {
    return undefined
  }

  try {
    return recordFromUnknown(parseJsonUnknown(text))
  } catch {
    return undefined
  }
}

const shcCallbackDeliveryFailure = (
  payload: Record<string, unknown> | undefined,
  text: string,
  httpStatus: number,
): RunnerCallbackDeliveryProjection | undefined => {
  const payloadText = payload === undefined ? '' : JSON.stringify(payload)
  const haystack = `${payloadText}\n${text}`.toLowerCase()
  const hasCallbackSignal =
    haystack.includes('callback') &&
    (haystack.includes('ingest') ||
      haystack.includes('delivery') ||
      haystack.includes('rejected'))

  if (!hasCallbackSignal) {
    return undefined
  }

  const callback = recordFromUnknown(payload?.callback)
  const event = recordFromUnknown(payload?.event)
  const reason = haystack.includes('ingest')
    ? 'callback_ingest_rejected'
    : 'callback_delivery_failed'
  const eventType =
    optionalText(payload?.eventType) ??
    optionalText(callback?.eventType) ??
    optionalText(event?.type) ??
    null
  const sequence =
    optionalNumber(payload?.sequence) ??
    optionalNumber(callback?.sequence) ??
    optionalNumber(event?.sequence) ??
    null

  return {
    eventType,
    httpStatus,
    message: safeDispatchMessage(
      text === '' ? payloadText : text,
      'Computer control reported a redacted runner callback delivery failure.',
    ),
    reason,
    sequence,
    status: 'failed',
  }
}

const shcDispatchStatusFromPayload = (
  payload: Record<string, unknown> | undefined,
  callbackDelivery: RunnerCallbackDeliveryProjection | undefined,
): string | undefined => {
  const run = recordFromUnknown(payload?.run)
  const runnerStatus =
    optionalText(payload?.runnerStatus) ??
    optionalText(payload?.runStatus) ??
    optionalText(run?.status)
  const topLevelStatus = optionalText(payload?.status)

  if (callbackDelivery !== undefined) {
    return runnerStatus ?? 'running'
  }

  return topLevelStatus ?? runnerStatus
}

const controlUrlForError = (value: string): string => {
  const url = new URL(value)

  return `${url.origin}${url.pathname}`
}

const isTimeoutAbort = (error: unknown): boolean =>
  error instanceof DOMException
    ? error.name === 'TimeoutError' || error.name === 'AbortError'
    : error instanceof Error &&
      (error.name === 'TimeoutError' ||
        error.name === 'AbortError' ||
        error.message.toLowerCase().includes('timeout'))

const fetchShcControl = async (
  fetcher: typeof fetch,
  url: string,
  init: RequestInit,
  operation: string,
): Promise<Response> => {
  try {
    return await fetcher(url, {
      ...init,
      signal: AbortSignal.timeout(SHC_CONTROL_REQUEST_TIMEOUT_MS),
    })
  } catch (error) {
    if (isTimeoutAbort(error)) {
      throw new OmniDispatchTimeout({
        endpoint: controlUrlForError(url),
        message: `Computer control API did not respond within ${SHC_CONTROL_REQUEST_TIMEOUT_MS / 1000}s for ${controlUrlForError(url)}. Check runner control health.`,
        operation,
        timeoutMs: SHC_CONTROL_REQUEST_TIMEOUT_MS,
      })
    }

    throw new OmniDispatchTransportFailure({
      endpoint: controlUrlForError(url),
      message: error instanceof Error ? error.message : String(error),
      operation,
    })
  }
}

const parseShcDispatchJson = (
  text: string,
  url: string,
  operation: string,
): unknown => {
  try {
    return parseJsonUnknown(text)
  } catch (error) {
    throw new OmniDispatchMalformedResponse({
      endpoint: controlUrlForError(url),
      message: error instanceof Error ? error.message : String(error),
      operation,
    })
  }
}

const shcDispatchPayload = (
  text: string,
  url: string,
  operation: string,
): Record<string, unknown> | undefined => {
  if (text === '') {
    return undefined
  }

  const payload = optionalRecord(parseShcDispatchJson(text, url, operation))

  if (payload === undefined) {
    throw new OmniDispatchMalformedResponse({
      endpoint: controlUrlForError(url),
      message: 'Computer control API returned a non-object JSON response.',
      operation,
    })
  }

  return payload
}

const shcCodexSandboxMode = (assignment: AgentRunAssignment): string => {
  if (assignment.sandbox.mode === 'workspace_write') {
    return 'danger_full_access'
  }

  return assignment.sandbox.mode
}

const shcCodexControlRequest = (assignment: AgentRunAssignment) => ({
  agentRuntime: assignment.runtime,
  authGrantRef: assignment.authGrantRef,
  blueprint: assignment.blueprint,
  githubWriteConnectionRef: assignment.githubWriteConnectionRef,
  githubWriteGrantRef: assignment.githubWriteGrantRef,
  githubWorkOrder: assignment.githubWorkOrder,
  goal: assignment.goal,
  goalContext: assignment.goalContext,
  providerAccountRef: assignment.providerAccountRef,
  repository: `${assignment.repository.owner}/${assignment.repository.repo}`,
  repositoryCloneUrl: `https://github.com/${assignment.repository.owner}/${assignment.repository.repo}.git`,
  repositoryRef: assignment.repository.ref,
  requiredArtifacts:
    assignment.githubWorkOrder === undefined
      ? ['result.md']
      : ['result.md', 'github-writeback.json'],
  retentionMode: assignment.retentionMode,
  runnerId: DEFAULT_RUNNER_ID,
  runId: assignment.runId,
  sandboxMode: shcCodexSandboxMode(assignment),
  timeoutMs: assignment.sandbox.timeoutMs,
})

export const dispatchAgentRunToShc = async (
  assignment: AgentRunAssignment,
  input: Readonly<{
    controlApiBearerToken?: string | undefined
    controlApiUrl?: string | undefined
    dispatchMode?: string | undefined
    fetcher?: typeof fetch | undefined
  }>,
): Promise<DispatchResult> => {
  const operation = 'dispatch_agent_run_to_shc'

  if (
    input.dispatchMode !== 'live' ||
    input.controlApiUrl === undefined ||
    input.controlApiBearerToken === undefined
  ) {
    throw new OmniDispatchMissingCredentials({
      message: 'Computer live dispatch is not configured.',
      operation,
    })
  }

  const fetcher = input.fetcher ?? fetch

  for (const url of controlCandidates(input.controlApiUrl)) {
    const response = await fetchShcControl(
      fetcher,
      url,
      {
        body: JSON.stringify(shcCodexControlRequest(assignment)),
        headers: {
          Authorization: `Bearer ${input.controlApiBearerToken}`,
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      operation,
    )
    const text = await response.text()

    if (response.status === 404) {
      continue
    }

    if (!response.ok) {
      const payload = parseOptionalShcPayload(text)
      const callbackDelivery = shcCallbackDeliveryFailure(
        payload,
        text,
        response.status,
      )

      if (callbackDelivery !== undefined) {
        const run = optionalRecord(payload?.run)
        const externalId =
          optionalText(payload?.externalRunId) ??
          optionalText(run?.externalRunId) ??
          `shc:${DEFAULT_RUNNER_ID}:${assignment.runId}`

        return {
          callbackDelivery,
          externalId,
          mode: 'live',
          payload,
          status:
            shcDispatchStatusFromPayload(payload, callbackDelivery) ??
            'running',
        }
      }

      throw new OmniDispatchRejectedRequest({
        endpoint: controlUrlForError(url),
        message: `Computer control API returned HTTP ${response.status}.${responseErrorSnippet(text)}`,
        operation,
        status: response.status,
      })
    }

    const payload = shcDispatchPayload(text, url, operation)
    const run = optionalRecord(payload?.run)
    const callbackDelivery = shcCallbackDeliveryFailure(
      payload,
      text,
      response.status,
    )
    const externalId =
      optionalText(payload?.externalRunId) ??
      optionalText(run?.externalRunId) ??
      `shc:${DEFAULT_RUNNER_ID}:${assignment.runId}`

    return {
      ...(callbackDelivery === undefined ? {} : { callbackDelivery }),
      externalId,
      mode: 'live',
      payload,
      status: shcDispatchStatusFromPayload(payload, callbackDelivery) ?? 'queued',
    }
  }

  throw new OmniDispatchUnavailableEndpoint({
    endpoint: controlUrlForError(input.controlApiUrl),
    message:
      'Computer control API did not expose a compatible codex-runs route.',
    operation,
  })
}

export const cancelAgentRunOnShc = async (
  run: AgentRunRecord,
  input: Readonly<{
    controlApiBearerToken?: string | undefined
    controlApiUrl?: string | undefined
    dispatchMode?: string | undefined
    fetcher?: typeof fetch | undefined
    reason: string
  }>,
): Promise<ShcControlActionResult> => {
  if (
    input.dispatchMode !== 'live' ||
    input.controlApiUrl === undefined ||
    input.controlApiBearerToken === undefined
  ) {
    return {
      error: 'Computer live control is not configured.',
      ok: false,
      status: 'not_configured',
    }
  }

  const fetcher = input.fetcher ?? fetch

  for (const url of controlActionCandidates(input.controlApiUrl, 'cancel')) {
    const response = await fetchShcControl(
      fetcher,
      url,
      {
        body: JSON.stringify({
          action: 'cancel',
          externalRunId: run.externalRunId,
          reason: input.reason,
          runId: run.id,
        }),
        headers: {
          Authorization: `Bearer ${input.controlApiBearerToken}`,
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      'cancel_agent_run_on_shc',
    )
    const text = await response.text()
    const targetPath = new URL(url).pathname

    if (response.status === 404) {
      continue
    }

    if (!response.ok) {
      return {
        error: `Computer control API returned HTTP ${response.status}.${responseErrorSnippet(text)}`,
        ok: false,
        status: response.status,
        targetPath,
      }
    }

    return {
      ok: true,
      payload:
        text === ''
          ? undefined
          : optionalRecord(
              parseShcDispatchJson(text, url, 'cancel_agent_run_on_shc'),
            ),
      status: response.status,
    }
  }

  return {
    error: 'Computer control API did not expose a compatible cancel route.',
    ok: false,
    status: 'not_found',
  }
}

export const checkShcControlHealth = async (
  input: Readonly<{
    controlApiBearerToken?: string | undefined
    controlApiUrl?: string | undefined
    dispatchMode?: string | undefined
    fetcher?: typeof fetch | undefined
  }>,
): Promise<ShcControlActionResult> => {
  if (
    input.dispatchMode !== 'live' ||
    input.controlApiUrl === undefined ||
    input.controlApiBearerToken === undefined
  ) {
    return {
      error: 'Computer live control is not configured.',
      ok: false,
      status: 'not_configured',
    }
  }

  const fetcher = input.fetcher ?? fetch

  for (const url of controlHealthCandidates(input.controlApiUrl)) {
    const response = await fetchShcControl(
      fetcher,
      url,
      {
        headers: {
          Authorization: `Bearer ${input.controlApiBearerToken}`,
        },
        method: 'GET',
      },
      'check_shc_control_health',
    )
    const text = await response.text()
    const targetPath = new URL(url).pathname

    if (response.status === 404) {
      continue
    }

    if (!response.ok) {
      return {
        error: `Computer control API returned HTTP ${response.status}.${responseErrorSnippet(text)}`,
        ok: false,
        status: response.status,
        targetPath,
      }
    }

    return {
      ok: true,
      payload:
        text === ''
          ? undefined
          : optionalRecord(
              parseShcDispatchJson(text, url, 'check_shc_control_health'),
            ),
      status: response.status,
    }
  }

  return {
    error: 'Computer control API did not expose a compatible health route.',
    ok: false,
    status: 'not_found',
  }
}

export const fetchAgentRunEventsFromShc = async (
  run: AgentRunRecord,
  input: Readonly<{
    controlApiBearerToken?: string | undefined
    controlApiUrl?: string | undefined
    cursor: number
    dispatchMode?: string | undefined
    fetcher?: typeof fetch | undefined
  }>,
): Promise<ShcControlEventsResult> => {
  if (
    input.dispatchMode !== 'live' ||
    input.controlApiUrl === undefined ||
    input.controlApiBearerToken === undefined
  ) {
    return {
      error: 'Computer live control is not configured.',
      ok: false,
      status: 'not_configured',
    }
  }

  const fetcher = input.fetcher ?? fetch

  for (const baseUrl of controlRunActionCandidates(
    input.controlApiUrl,
    run.id,
    'events',
  )) {
    const url = new URL(baseUrl)

    url.searchParams.set('cursor', String(input.cursor))

    const response = await fetchShcControl(
      fetcher,
      url.toString(),
      {
        headers: {
          Authorization: `Bearer ${input.controlApiBearerToken}`,
        },
        method: 'GET',
      },
      'fetch_agent_run_events_from_shc',
    )
    const text = await response.text()
    const targetPath = url.pathname

    if (response.status === 404) {
      continue
    }

    if (!response.ok) {
      return {
        error: `Computer control API returned HTTP ${response.status}.${responseErrorSnippet(text)}`,
        ok: false,
        status: response.status,
        targetPath,
      }
    }

    const payload = shcDispatchPayload(
      text,
      url.toString(),
      'fetch_agent_run_events_from_shc',
    )
    const events = Array.isArray(payload?.events)
      ? payload.events.filter(recordFromUnknown)
      : []
    const nextCursor =
      typeof payload?.nextCursor === 'number' &&
      Number.isFinite(payload.nextCursor)
        ? payload.nextCursor
        : input.cursor
    const runRecord = optionalRecord(payload?.run)

    return {
      events,
      nextCursor,
      ok: true,
      payload,
      runStatus:
        optionalText(payload?.status) ?? optionalText(runRecord?.status),
      status: response.status,
    }
  }

  return {
    error: 'Computer control API did not expose a compatible events route.',
    ok: false,
    status: 'not_found',
  }
}

export const continueAgentRunOnShc = async (
  run: AgentRunRecord,
  input: Readonly<{
    authGrantRef?: string | undefined
    controlApiBearerToken?: string | undefined
    controlApiUrl?: string | undefined
    dispatchMode?: string | undefined
    fetcher?: typeof fetch | undefined
    prompt: string
    turnId: string
  }>,
): Promise<ShcControlActionResult> => {
  if (
    input.dispatchMode !== 'live' ||
    input.controlApiUrl === undefined ||
    input.controlApiBearerToken === undefined
  ) {
    return {
      error: 'Computer live control is not configured.',
      ok: false,
      status: 'not_configured',
    }
  }

  const fetcher = input.fetcher ?? fetch
  const requestBody = JSON.stringify({
    ...(input.authGrantRef === undefined
      ? {}
      : { authGrantRef: input.authGrantRef }),
    externalRunId: run.externalRunId,
    prompt: input.prompt,
    runId: run.id,
    turnId: input.turnId,
  })
  const urls = [
    ...controlRunActionCandidates(input.controlApiUrl, run.id, 'turns'),
    ...controlActionCandidates(input.controlApiUrl, 'continue'),
  ]

  for (const url of urls) {
    const response = await fetchShcControl(
      fetcher,
      url,
      {
        body: requestBody,
        headers: {
          Authorization: `Bearer ${input.controlApiBearerToken}`,
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      'continue_agent_run_on_shc',
    )
    const text = await response.text()
    const targetPath = new URL(url).pathname

    if (response.status === 404) {
      continue
    }

    if (!response.ok) {
      return {
        error: `Computer control API returned HTTP ${response.status}.${responseErrorSnippet(text)}`,
        ok: false,
        status: response.status,
        targetPath,
      }
    }

    return {
      ok: true,
      payload:
        text === ''
          ? undefined
          : optionalRecord(
              parseShcDispatchJson(text, url, 'continue_agent_run_on_shc'),
            ),
      status: response.status,
    }
  }

  return {
    error: 'Computer control API did not expose a compatible continue route.',
    ok: false,
    status: 'not_found',
  }
}

export const dispatchDeploymentToShc = async (
  assignment: AppDeployAssignment,
  input: Readonly<{
    controlApiBearerToken?: string | undefined
    controlApiUrl?: string | undefined
    dispatchMode?: string | undefined
    fetcher?: typeof fetch | undefined
  }>,
): Promise<DispatchResult> => {
  const operation = 'dispatch_deployment_to_shc'

  if (
    input.dispatchMode !== 'live' ||
    input.controlApiUrl === undefined ||
    input.controlApiBearerToken === undefined
  ) {
    throw new OmniDispatchMissingCredentials({
      message: 'Computer live dispatch is not configured.',
      operation,
    })
  }

  const fetcher = input.fetcher ?? fetch
  const goal = [
    `Deploy ${assignment.service} from ${assignment.repository.owner}/${assignment.repository.repo}@${assignment.repository.ref}.`,
    `Run install, typecheck, test, build, smoke, and retain rollback state.`,
    `Use backup computer backend ${assignment.fallbackBackend} if the primary computer cannot complete safely.`,
  ].join(' ')

  for (const url of controlCandidates(input.controlApiUrl)) {
    const response = await fetchShcControl(
      fetcher,
      url,
      {
        body: JSON.stringify({
          action: 'start',
          assignmentKind: 'app_deploy',
          backend: assignment.primaryBackend,
          deployment: assignment,
          goal,
          repository: `${assignment.repository.owner}/${assignment.repository.repo}`,
          runId: assignment.deployId,
          runtime: assignment.runtime,
          runnerBackend: assignment.primaryBackend,
          runnerId: DEFAULT_RUNNER_ID,
        }),
        headers: {
          Authorization: `Bearer ${input.controlApiBearerToken}`,
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      operation,
    )
    const text = await response.text()

    if (response.status === 404) {
      continue
    }

    if (!response.ok) {
      throw new OmniDispatchRejectedRequest({
        endpoint: controlUrlForError(url),
        message: `Computer control API returned HTTP ${response.status}.${responseErrorSnippet(text)}`,
        operation,
        status: response.status,
      })
    }

    const payload = shcDispatchPayload(text, url, operation)
    const run = optionalRecord(payload?.run)
    const externalId =
      optionalText(payload?.externalRunId) ??
      optionalText(run?.externalRunId) ??
      `shc:${DEFAULT_RUNNER_ID}:${assignment.deployId}`

    return {
      externalId,
      mode: 'live',
      payload,
      status:
        optionalText(payload?.status) ?? optionalText(run?.status) ?? 'queued',
    }
  }

  throw new OmniDispatchUnavailableEndpoint({
    endpoint: controlUrlForError(input.controlApiUrl),
    message:
      'Computer control API did not expose a compatible codex-runs route.',
    operation,
  })
}

export const dispatchEventForAgentRun = (
  runId: string,
  sequence: number,
  result: DispatchResult,
): OmniEventRecord =>
  eventRecord(
    runId,
    sequence,
    'runner.dispatched',
    'OpenAgents dispatched the assignment to the computer.',
    {
      payload: {
        ...(result.callbackDelivery === undefined
          ? {}
          : { callbackDelivery: result.callbackDelivery }),
        externalRunId: result.externalId,
        mode: result.mode,
        status: result.status,
      },
      source: 'shc',
      status: result.status,
    },
  )

export const dispatchEventForDeployment = (
  deployId: string,
  sequence: number,
  result: DispatchResult,
): OmniEventRecord =>
  eventRecord(
    deployId,
    sequence,
    'deploy.dispatched',
    'OpenAgents dispatched the deploy assignment to the computer.',
    {
      payload: {
        externalDeployId: result.externalId,
        mode: result.mode,
        status: result.status,
      },
      source: 'shc',
      status: result.status,
    },
  )

export const publicGoalContext = (assignment: AgentRunAssignment) =>
  assignment.goalContext === undefined
    ? undefined
    : {
        goalId: assignment.goalContext.goalId,
        objective: assignment.goalContext.objective,
        remainingTokens: assignment.goalContext.remainingTokens,
        schemaVersion: assignment.goalContext.schemaVersion,
        status: assignment.goalContext.status,
        timeUsedSeconds: assignment.goalContext.timeUsedSeconds,
        tokenBudget: assignment.goalContext.tokenBudget,
        tokensUsed: assignment.goalContext.tokensUsed,
        toolContract: assignment.goalContext.toolContract,
        visibility: assignment.goalContext.visibility,
      }

const eventCallbackDelivery = (
  event: OmniEventRecord,
): RunnerCallbackDeliveryProjection | undefined => {
  const payload = safeJsonRecord(event.payloadJson)
  const nestedPayload = recordFromUnknown(payload?.payload)
  const callbackDelivery = payload?.callbackDelivery ?? nestedPayload?.callbackDelivery

  return recordFromUnknown(callbackDelivery) as
    | RunnerCallbackDeliveryProjection
    | undefined
}

const lastRunnerEvent = (
  events: ReadonlyArray<OmniEventRecord>,
): OmniEventRecord | undefined =>
  [...events]
    .reverse()
    .find(
      event =>
        event.type.startsWith('runner.') || event.type.startsWith('cloud.run.'),
    )

export const agentRunOperationalState = (bundle: AgentRunBundle) => {
  const latestRunnerEvent = lastRunnerEvent(bundle.events)
  const callbackFailure = [...bundle.events]
    .reverse()
    .map(eventCallbackDelivery)
    .find(
      (
        delivery,
      ): delivery is RunnerCallbackDeliveryProjection =>
        delivery !== undefined,
    )

  return {
    callbackDelivery:
      callbackFailure === undefined
        ? { status: 'ok' as const }
        : {
            eventType: callbackFailure.eventType,
            httpStatus: callbackFailure.httpStatus,
            message: callbackFailure.message,
            reason: callbackFailure.reason,
            sequence: callbackFailure.sequence,
            status: callbackFailure.status,
          },
    runner: {
      lastEventSequence: latestRunnerEvent?.sequence ?? null,
      lastEventType: latestRunnerEvent?.type ?? null,
      status: bundle.run.status,
    },
  }
}

export const publicAgentRunBundle = (bundle: AgentRunBundle) => {
  const goalContext = publicGoalContext(bundle.run.assignment)

  return {
    events: bundle.events,
    operationalState: agentRunOperationalState(bundle),
    run: {
      ...bundle.run,
      assignment: {
        ...bundle.run.assignment,
        callback: {
          ...bundle.run.assignment.callback,
          tokenRef: 'runner_callback_token',
        },
        ...(goalContext === undefined ? {} : { goalContext }),
      },
    },
  }
}

export const publicDeploymentBundle = (bundle: DeploymentBundle) => ({
  deployment: {
    ...bundle.deployment,
    assignment: {
      ...bundle.deployment.assignment,
      callback: {
        ...bundle.deployment.assignment.callback,
        tokenRef: 'runner_callback_token',
      },
    },
  },
  events: bundle.events,
})

export const eventFromRunnerPayload = (
  parentId: string,
  fallbackSequence: number,
  payload: Record<string, unknown>,
): OmniEventRecord => {
  const event = normalizeOmniRunnerEventPayload(payload, fallbackSequence)

  return eventRecord(
    parentId,
    event?.sequence ?? fallbackSequence,
    event?.type ?? 'runner.event',
    event?.summary ?? 'Runner event received.',
    {
      artifactRefs: event?.artifactRefs ?? [],
      payload: event?.payload ?? payload,
      source: event?.source ?? 'runner',
      ...(event?.externalEventId === undefined
        ? {}
        : { externalEventId: event.externalEventId }),
      ...(event?.status === undefined ? {} : { status: event.status }),
    },
  )
}

export const runStatusFromText = (
  status: string | undefined,
): AgentRunStatus | undefined =>
  status === 'queued' ||
  status === 'running' ||
  status === 'waiting_for_input' ||
  status === 'completed' ||
  status === 'failed' ||
  status === 'canceled'
    ? status
    : undefined

export const deploymentStatusFromText = (
  status: string | undefined,
): DeploymentStatus | undefined =>
  status === 'queued' ||
  status === 'running' ||
  status === 'promoted' ||
  status === 'rolled_back' ||
  status === 'failed' ||
  status === 'canceled'
    ? status
    : undefined

export const firstText = (
  ...values: ReadonlyArray<unknown>
): string | undefined => {
  for (const value of values) {
    if (typeof value === 'string' && value.trim() !== '') {
      return value.trim()
    }
  }

  return undefined
}

export const numberOrUndefined = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) ? value : undefined

export const maybeText = textOrUndefined
