import {
  AgentRuntimeEvent as AgentRuntimeEventSchema,
  AgentRuntimeRun as AgentRuntimeRunSchema,
  compileAgentDefinitionToolRuntimePolicy,
  type AgentDefinition,
  type AgentRuntimeAdapterKind,
  type AgentRuntimeEvent,
  type AgentRuntimeRun,
} from '@openagentsinc/agent-runtime-schema'
import type { ForgeGitAccessScope } from '@openagentsinc/forge-protocol'
import { Schema as S } from 'effect'

import {
  type AgentDefinitionStore,
} from './agent-definition-routes'
import { withAgentRateLimitHeaders } from './agent-rate-limit-policy'
import {
  type AgentRegistrationStore,
  type LinkedAgentOwnerRecord,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
} from './agent-registration'
import {
  readAgentBearerToken as bearerTokenFromRequest,
} from './auth/bearer-token'
import {
  type ForgeCoordinationStore,
} from './forge-coordination-store'
import {
  compileAgentDefinitionForgeGitAccessScopes,
  type ForgeTenantGitAuthStore,
} from './forge-tenant-git-auth-store'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import {
  decodeUnknownWithSchema,
  parseJsonWithSchema,
  readJsonObject,
} from './json-boundary'
import {
  type DurableStreamNamespace,
  seedDurableInferenceStreamDO,
} from './inference/durable-inference-do-transport'
import {
  type CodingDelegationResult,
  delegateCodingWorkflow,
} from './inference/coding-workflow-delegation'
import type { CodingWorkflowClassification } from './inference/coding-workflow-classifier'
import {
  type PylonApiAssignmentRecord,
  type PylonApiStore,
} from './pylon-api'
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
  randomUuid,
  utcStartOfDayIsoTimestamp,
} from './runtime-primitives'

type HttpResponse = globalThis.Response

const AGENT_DEFINITION_RUN_TENANT_REF = 'tenant.openagents.background_agents'
const AGENT_DEFINITION_RUN_TENANT_DISPLAY_NAME = 'OpenAgents Background Agents'
const AGENT_DEFINITION_RUN_REPOSITORY_REF = 'repo.openagents.openagents'
const AGENT_DEFINITION_RUN_SCHEMA =
  'openagents.agent_definition_run.v0.1' as const
const AGENT_DEFINITION_RUN_SESSION_EVENT_SCHEMA =
  'openagents.agent_definition_run.session_event.v0.1' as const
const AGENT_DEFINITION_RUN_DISPATCH_CREDITS_RESERVED = 0
const AGENT_DEFINITION_RUN_FORGE_GIT_TOKEN_MAX_TTL_MS = 60 * 60 * 1000
const AGENT_DEFINITION_RUN_FORGE_GIT_TOKEN_BUFFER_MS = 10 * 60 * 1000
const AGENT_DEFINITION_RUN_FORGE_GIT_SCOPES: ReadonlyArray<ForgeGitAccessScope> = [
  'git:receive-pack',
]
const AGENT_DEFINITION_RUN_SCM_AUTH_BROKER_SCHEMA =
  'openagents.pylon.scm_auth_broker.v1' as const
const AGENT_DEFINITION_RUN_SCM_AUTH_BROKER_URL =
  'https://openagents.com/api/pylon/forge/git-credentials'
const AGENT_DEFINITION_RUN_SCM_AUTH_BROKER_CACHE_TTL_SECONDS = 60

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const PublicSafeRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/=-]*$/),
)
const PublicSafeSummary = NonEmptyTrimmedString.check(S.isMaxLength(1000))
const TriggerPayload = S.Record(S.String, S.Unknown)
const WorkspaceHint = S.Record(S.String, S.Unknown)

const AgentDefinitionRunRequest = S.Struct({
  triggerRef: S.optionalKey(PublicSafeRef),
  triggerPayload: S.optionalKey(TriggerPayload),
  objectiveSummary: S.optionalKey(PublicSafeSummary),
  targetPylonRef: S.optionalKey(PublicSafeRef),
  targetAccountRefHash: S.optionalKey(PublicSafeRef),
  workspace: S.optionalKey(WorkspaceHint),
})
export type AgentDefinitionRunRequest = typeof AgentDefinitionRunRequest.Type

export type AgentDefinitionRunStatus = 'dispatched' | 'pending' | 'refused'

export type AgentDefinitionRunRecord = Readonly<{
  assignmentRef: string | null
  budgetCreditsReserved: number
  createdAt: string
  definitionId: string
  definitionRef: string
  durableRequestId: string
  durableStreamUrl: string | null
  evidenceRefs: ReadonlyArray<string>
  forgeGitTokenRefs: ReadonlyArray<string>
  forgeRepositoryRef: string | null
  forgeTenantRef: string
  forgeWorkRef: string
  initialEvents: ReadonlyArray<AgentRuntimeEvent>
  lane: AgentDefinition['lane']
  ownerAgentUserId: string
  pylonRef: string | null
  refusalError: string | null
  refusalReason: string | null
  run: AgentRuntimeRun
  runId: string
  status: AgentDefinitionRunStatus
  triggerPayload: Record<string, unknown>
  triggerRef: string
  updatedAt: string
}>

export type AgentDefinitionRunDailyBudgetUsage = Readonly<{
  creditsReserved: number
  runCount: number
}>

export type AgentDefinitionRunStore = Readonly<{
  listRunsForDefinition: (
    ownerAgentUserId: string,
    definitionId: string,
    limit: number,
  ) => Promise<ReadonlyArray<AgentDefinitionRunRecord>>
  upsertRun: (record: AgentDefinitionRunRecord) => Promise<AgentDefinitionRunRecord>
  readRun: (
    ownerAgentUserId: string,
    runId: string,
  ) => Promise<AgentDefinitionRunRecord | undefined>
  readRunByAssignmentRef: (
    assignmentRef: string,
  ) => Promise<AgentDefinitionRunRecord | undefined>
  readDailyBudgetUsage: (
    ownerAgentUserId: string,
    definitionId: string,
    dayStartIso: string,
    dayEndIso: string,
  ) => Promise<AgentDefinitionRunDailyBudgetUsage>
}>

type AgentDefinitionRunRow = Readonly<{
  assignment_ref: string | null
  budget_credits_reserved: number
  created_at: string
  definition_id: string
  definition_ref: string
  durable_request_id: string
  durable_stream_url: string | null
  evidence_refs_json: string
  forge_git_token_refs_json?: string | undefined
  forge_repository_ref?: string | null | undefined
  forge_tenant_ref: string
  forge_work_ref: string
  initial_events_json: string
  lane: AgentDefinition['lane']
  owner_agent_user_id: string
  pylon_ref: string | null
  refusal_error: string | null
  refusal_reason: string | null
  run_id: string
  runtime_run_json: string
  status: AgentDefinitionRunStatus
  trigger_payload_json: string
  trigger_ref: string
  updated_at: string
}>

const StringArray = S.Array(S.String)
const AgentRuntimeEvents = S.Array(AgentRuntimeEventSchema)

const rowToRunRecord = (row: AgentDefinitionRunRow): AgentDefinitionRunRecord => ({
  assignmentRef: row.assignment_ref,
  budgetCreditsReserved: row.budget_credits_reserved,
  createdAt: row.created_at,
  definitionId: row.definition_id,
  definitionRef: row.definition_ref,
  durableRequestId: row.durable_request_id,
  durableStreamUrl: row.durable_stream_url,
  evidenceRefs: parseJsonWithSchema(StringArray, row.evidence_refs_json),
  forgeGitTokenRefs: parseJsonWithSchema(
    StringArray,
    row.forge_git_token_refs_json ?? '[]',
  ),
  forgeRepositoryRef: row.forge_repository_ref ?? null,
  forgeTenantRef: row.forge_tenant_ref,
  forgeWorkRef: row.forge_work_ref,
  initialEvents: parseJsonWithSchema(AgentRuntimeEvents, row.initial_events_json),
  lane: row.lane,
  ownerAgentUserId: row.owner_agent_user_id,
  pylonRef: row.pylon_ref,
  refusalError: row.refusal_error,
  refusalReason: row.refusal_reason,
  run: parseJsonWithSchema(AgentRuntimeRunSchema, row.runtime_run_json),
  runId: row.run_id,
  status: row.status,
  triggerPayload: parseJsonWithSchema(TriggerPayload, row.trigger_payload_json),
  triggerRef: row.trigger_ref,
  updatedAt: row.updated_at,
})

export const makeD1AgentDefinitionRunStore = (
  db: D1Database,
): AgentDefinitionRunStore => ({
  listRunsForDefinition: async (ownerAgentUserId, definitionId, limit) => {
    const rows = await db
      .prepare(
        `SELECT *
           FROM agent_definition_runs
          WHERE owner_agent_user_id = ?
            AND definition_id = ?
          ORDER BY updated_at DESC
          LIMIT ?`,
      )
      .bind(ownerAgentUserId, definitionId, limit)
      .all<AgentDefinitionRunRow>()

    return rows.results.map(rowToRunRecord)
  },
  upsertRun: async record => {
    await db
      .prepare(
        `INSERT INTO agent_definition_runs (
           run_id, owner_agent_user_id, definition_id, definition_ref,
           trigger_ref, lane, status, pylon_ref, assignment_ref,
           durable_request_id, durable_stream_url, forge_tenant_ref,
           forge_work_ref, forge_repository_ref, forge_git_token_refs_json,
           refusal_error, refusal_reason, evidence_refs_json,
           trigger_payload_json, runtime_run_json, initial_events_json,
           budget_credits_reserved, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(run_id) DO UPDATE SET
           status = excluded.status,
           pylon_ref = excluded.pylon_ref,
           assignment_ref = excluded.assignment_ref,
           durable_stream_url = excluded.durable_stream_url,
           forge_repository_ref = excluded.forge_repository_ref,
           forge_git_token_refs_json = excluded.forge_git_token_refs_json,
           refusal_error = excluded.refusal_error,
           refusal_reason = excluded.refusal_reason,
           evidence_refs_json = excluded.evidence_refs_json,
           runtime_run_json = excluded.runtime_run_json,
           initial_events_json = excluded.initial_events_json,
           budget_credits_reserved = excluded.budget_credits_reserved,
           updated_at = excluded.updated_at`,
      )
      .bind(
        record.runId,
        record.ownerAgentUserId,
        record.definitionId,
        record.definitionRef,
        record.triggerRef,
        record.lane,
        record.status,
        record.pylonRef,
        record.assignmentRef,
        record.durableRequestId,
        record.durableStreamUrl,
        record.forgeTenantRef,
        record.forgeWorkRef,
        record.forgeRepositoryRef,
        JSON.stringify(record.forgeGitTokenRefs),
        record.refusalError,
        record.refusalReason,
        JSON.stringify(record.evidenceRefs),
        JSON.stringify(record.triggerPayload),
        JSON.stringify(record.run),
        JSON.stringify(record.initialEvents),
        record.budgetCreditsReserved,
        record.createdAt,
        record.updatedAt,
      )
      .run()

    return record
  },
  readRunByAssignmentRef: async assignmentRef => {
    const row = await db
      .prepare(
        `SELECT *
           FROM agent_definition_runs
          WHERE assignment_ref = ?
          ORDER BY updated_at DESC
          LIMIT 1`,
      )
      .bind(assignmentRef)
      .first<AgentDefinitionRunRow>()

    return row === null ? undefined : rowToRunRecord(row)
  },
  readRun: async (ownerAgentUserId, runId) => {
    const row = await db
      .prepare(
        `SELECT *
           FROM agent_definition_runs
          WHERE owner_agent_user_id = ?
            AND run_id = ?
          LIMIT 1`,
      )
      .bind(ownerAgentUserId, runId)
      .first<AgentDefinitionRunRow>()

    return row === null ? undefined : rowToRunRecord(row)
  },
  readDailyBudgetUsage: async (
    ownerAgentUserId,
    definitionId,
    dayStartIso,
    dayEndIso,
  ) => {
    const row = await db
      .prepare(
        `SELECT COUNT(*) AS run_count,
                COALESCE(SUM(budget_credits_reserved), 0) AS credits_reserved
           FROM agent_definition_runs
          WHERE owner_agent_user_id = ?
            AND definition_id = ?
            AND created_at >= ?
            AND created_at < ?`,
      )
      .bind(ownerAgentUserId, definitionId, dayStartIso, dayEndIso)
      .first<{
        readonly credits_reserved: number | null
        readonly run_count: number
      }>()

    return {
      creditsReserved: row?.credits_reserved ?? 0,
      runCount: row?.run_count ?? 0,
    }
  },
})

export type AgentDefinitionRunRouteDependencies = Readonly<{
  agentStore: AgentRegistrationStore
  definitionStore: Pick<AgentDefinitionStore, 'readDefinition'>
  durableStreamNamespace?: DurableStreamNamespace | undefined
  forgeGitAuthStore: ForgeTenantGitAuthStore
  forgeStore: ForgeCoordinationStore
  makeId?: () => string
  nowIso?: () => string
  pylonStore: PylonApiStore
  runStore: AgentDefinitionRunStore
}>

export type AgentDefinitionRunDispatchDependencies = Readonly<{
  durableStreamNamespace?: DurableStreamNamespace | undefined
  forgeGitAuthStore: ForgeTenantGitAuthStore
  forgeStore: ForgeCoordinationStore
  linkedAgents: ReadonlyArray<
    LinkedAgentOwnerRecord | Readonly<{ agentUserId: string }>
  >
  makeId?: (() => string) | undefined
  nowIso?: (() => string) | undefined
  pylonStore: PylonApiStore
  runStore: AgentDefinitionRunStore
}>

export type AgentDefinitionRunDispatchOutcome =
  | Readonly<{
      kind: 'dispatched'
      assignmentRef: string
      durableStreamUrl: string
      record: AgentDefinitionRunRecord
      seeded: boolean
    }>
  | Readonly<{
      evidenceRefs: ReadonlyArray<string>
      error: string
      kind: 'refused'
      reason: string
      record: AgentDefinitionRunRecord
      requestedPylonRef: string | null
      seeded: boolean
      statusCode: number
    }>
  | Readonly<{
      kind: 'invalid'
      reason: string
    }>
  | Readonly<{
      kind: 'storage_error'
    }>

type AgentDefinitionRunRouteKind = 'history' | 'run_now'

export const matchAgentDefinitionRunRequest = (
  request: Request,
): Readonly<{ definitionId: string; route: AgentDefinitionRunRouteKind }> | undefined => {
  const pathname = new URL(request.url).pathname
  const runsMatch = /^\/v1\/agent-definitions\/([^/]+)\/runs$/.exec(pathname)
  const runNowMatch = /^\/v1\/agent-definitions\/([^/]+)\/run-now$/.exec(pathname)
  const match = runsMatch ?? runNowMatch

  if (match === null) {
    return undefined
  }

  try {
    const definitionId = decodeURIComponent(match[1] ?? '').trim()

    return definitionId === ''
      ? undefined
      : {
          definitionId,
          route: runsMatch === null ? 'run_now' : 'history',
        }
  } catch {
    return undefined
  }
}

const boundedRunHistoryLimitFromRequest = (request: Request): number => {
  const rawLimit = new URL(request.url).searchParams.get('limit')
  const parsed = rawLimit === null ? 50 : Number(rawLimit)

  return Number.isFinite(parsed)
    ? Math.max(1, Math.min(100, Math.trunc(parsed)))
    : 50
}

const requireAgentSession = async (
  request: Request,
  dependencies: AgentDefinitionRunRouteDependencies,
): Promise<ProgrammaticAgentSession | undefined> => {
  const bearerToken = bearerTokenFromRequest(request)

  return bearerToken === undefined
    ? undefined
    : authenticateProgrammaticAgent(
        dependencies.agentStore,
        bearerToken,
        dependencies.nowIso,
      )
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const refFragment = (value: string): string => {
  const fragment = value.replaceAll(/[^A-Za-z0-9_.:-]/g, '_').slice(0, 120)

  return fragment === '' ? 'run' : fragment
}

const makeRunId = (
  dependencies: Readonly<{ makeId?: (() => string) | undefined }>,
): string => `agent_definition_run.${dependencies.makeId?.() ?? randomUuid()}`

const definitionRef = (definition: AgentDefinition): string =>
  `agent_definition.${refFragment(definition.id)}`

const runRef = (runId: string): string =>
  `agent_definition_run.${refFragment(runId)}`

const triggerRefsForDefinition = (
  definition: AgentDefinition,
): ReadonlyArray<string> =>
  definition.triggers.map(trigger => trigger.triggerRef)

const manualTriggerRefForDefinition = (
  definition: AgentDefinition,
): string | undefined =>
  definition.triggers.find(trigger => trigger.kind === 'manual')?.triggerRef

const triggerRefFromRequest = (
  definition: AgentDefinition,
  input: AgentDefinitionRunRequest,
): Readonly<{ kind: 'ok'; triggerRef: string }> | Readonly<{ kind: 'invalid' }> => {
  if (input.triggerRef !== undefined) {
    return triggerRefsForDefinition(definition).includes(input.triggerRef)
      ? { kind: 'ok', triggerRef: input.triggerRef }
      : { kind: 'invalid' }
  }

  const manual = definition.triggers.find(trigger => trigger.kind === 'manual')
  const first = definition.triggers[0]

  return {
    kind: 'ok',
    triggerRef:
      manual?.triggerRef ??
      first?.triggerRef ??
      `trigger.manual.${refFragment(definition.id)}`,
  }
}

const workflowClassForDefinition = (
  definition: AgentDefinition,
): CodingWorkflowClassification['workflowClass'] | null => {
  if (definition.harness.kind === 'claude_code') {
    return 'claude_agent_task'
  }

  return definition.harness.kind === 'codex' || definition.harness.kind === 'khala'
    ? 'codex_agent_task'
    : null
}

const adapterKindForDefinition = (
  definition: AgentDefinition,
): AgentRuntimeAdapterKind =>
  definition.harness.kind === 'claude_code' ? 'claude_code' : 'codex'

const runSourceRefs = (input: Readonly<{
  definition: AgentDefinition
  runId: string
  triggerRef: string
}>): ReadonlyArray<string> =>
  uniqueRefs([
    'github.issue.8189',
    definitionRef(input.definition),
    runRef(input.runId),
    `trigger.${refFragment(input.triggerRef)}`,
    ...input.definition.sourceRefs,
  ])

const buildRuntimeRun = (input: Readonly<{
  assignmentRef: string | null
  definition: AgentDefinition
  forgeWorkRef: string
  runId: string
  sourceRefs: ReadonlyArray<string>
  state: AgentRuntimeRun['state']
  updatedAt: string
}>): AgentRuntimeRun =>
  decodeUnknownWithSchema(AgentRuntimeRunSchema, {
    runId: input.runId,
    agentDefinitionId: input.definition.id,
    ...(input.assignmentRef === null ? {} : { assignmentId: input.assignmentRef }),
    workOrderId: input.forgeWorkRef,
    workspaceRef: `workspace.background_agent.${refFragment(input.runId)}`,
    adapterKind: adapterKindForDefinition(input.definition),
    loopKind: 'external_agent_loop',
    sourceRefs: input.sourceRefs,
    budgetRef: `budget.agent_definition.${refFragment(input.definition.id)}`,
    usagePolicy: 'usage.agent_definition.exact_pylon_closeout',
    permissionPolicy: 'policy.agent_definition.owner_pylon_dispatch_only',
    redactionPolicy: {
      policyRef: 'policy.agent_definition.owner_private_redaction.v1',
      rawPromptAllowed: false,
      rawShellLogAllowed: false,
      providerPayloadAllowed: false,
      localPathAllowed: false,
      secretMaterialAllowed: false,
    },
    visibility: 'private',
    publicProjectionAllowed: false,
    state: input.state,
    createdAt: input.updatedAt,
    updatedAt: input.updatedAt,
    adapterSessionRefs:
      input.assignmentRef === null ? [] : [`pylon.assignment.${input.assignmentRef}`],
  })

const runtimeEvent = (input: Readonly<{
  blockerRefs?: ReadonlyArray<string>
  generatedAt: string
  refs?: ReadonlyArray<string>
  runId: string
  sequence: number
  summary: string
  tag: AgentRuntimeEvent['tag']
}>): AgentRuntimeEvent =>
  decodeUnknownWithSchema(AgentRuntimeEventSchema, {
    tag: input.tag,
    eventId: `agent_runtime_event.${refFragment(input.runId)}.${input.sequence}`,
    runId: input.runId,
    sequence: input.sequence,
    generatedAt: input.generatedAt,
    visibility: 'private',
    redactionClass: 'redacted_summary',
    summary: input.summary,
    refs: uniqueRefs(input.refs ?? []),
    blockerRefs: uniqueRefs(input.blockerRefs ?? []),
  })

const sessionEventFrame = (event: AgentRuntimeEvent): string =>
  `data: ${JSON.stringify({
    schema: AGENT_DEFINITION_RUN_SESSION_EVENT_SCHEMA,
    event,
  })}\n\n`

const seedSessionEvents = async (
  dependencies: Pick<AgentDefinitionRunDispatchDependencies, 'durableStreamNamespace'>,
  input: Readonly<{
    close: boolean
    durableRequestId: string
    events: ReadonlyArray<AgentRuntimeEvent>
  }>,
): Promise<boolean> =>
  dependencies.durableStreamNamespace === undefined
    ? false
    : seedDurableInferenceStreamDO({
        close: input.close,
        frames: input.events.map(sessionEventFrame),
        namespace: dependencies.durableStreamNamespace,
        requestId: input.durableRequestId,
    })

const receiptRefsForRun = (record: AgentDefinitionRunRecord): ReadonlyArray<string> =>
  uniqueRefs([
    ...record.evidenceRefs,
    record.forgeWorkRef,
    ...record.forgeGitTokenRefs,
    ...(record.assignmentRef === null
      ? []
      : [
          record.assignmentRef,
          `pylon.assignment.${refFragment(record.assignmentRef)}`,
        ]),
    ...(record.durableStreamUrl === null
      ? []
      : [`durable_stream.${refFragment(record.durableRequestId)}`]),
    ...(record.refusalError === null
      ? []
      : [`refusal.agent_definition_run.${refFragment(record.refusalError)}`]),
  ])

const projectionForRun = (
  record: AgentDefinitionRunRecord,
  sessionEventStreamSeeded: boolean,
) => ({
  schema: AGENT_DEFINITION_RUN_SCHEMA,
  assignmentRef: record.assignmentRef,
  definitionRef: record.definitionRef,
  durableRequestId: record.durableRequestId,
  durableStreamUrl: record.durableStreamUrl,
  evidenceRefs: record.evidenceRefs,
  exactAccounting:
    record.assignmentRef === null
      ? null
      : {
          demandSource: 'khala_coding_delegation',
          settlesOn: 'pylon_worker_closeout',
          taskRef: record.assignmentRef,
          usageTruth: 'exact',
        },
  forge: {
    gitTokenRefs: record.forgeGitTokenRefs,
    repositoryRef: record.forgeRepositoryRef,
    tenantRef: record.forgeTenantRef,
    workRef: record.forgeWorkRef,
  },
  lane: record.lane,
  pylonRef: record.pylonRef,
  refusal:
    record.refusalError === null
      ? null
      : {
          error: record.refusalError,
          reason: record.refusalReason,
        },
  receiptRefs: receiptRefsForRun(record),
  runId: record.runId,
  sessionEventStream: {
    seeded: sessionEventStreamSeeded,
    eventCount: record.initialEvents.length,
    durableRequestId: record.durableRequestId,
    durableStreamUrl: record.durableStreamUrl,
  },
  status: record.status,
  triggerRef: record.triggerRef,
  updatedAt: record.updatedAt,
  createdAt: record.createdAt,
})

const invalidRunResponse = (reason: string): HttpResponse =>
  withAgentRateLimitHeaders(
    noStoreJsonResponse(
      { error: 'invalid_agent_definition_run', reason },
      { status: 400 },
    ),
  )

const notFoundResponse = (): HttpResponse =>
  withAgentRateLimitHeaders(
    noStoreJsonResponse(
      { error: 'agent_definition_not_found' },
      { status: 404 },
    ),
  )

const storageErrorResponse = (): HttpResponse =>
  withAgentRateLimitHeaders(
    noStoreJsonResponse(
      { error: 'agent_definition_run_storage_error' },
      { status: 503 },
    ),
  )

const listRunsResponse = (
  input: Readonly<{
    definition: AgentDefinition
    limit: number
    runs: ReadonlyArray<AgentDefinitionRunRecord>
  }>,
): HttpResponse =>
  withAgentRateLimitHeaders(
    noStoreJsonResponse({
      schema: 'openagents.agent_definition_run_list.v0.1',
      count: input.runs.length,
      definitionId: input.definition.id,
      definitionRef: definitionRef(input.definition),
      limit: input.limit,
      runs: input.runs.map(record =>
        projectionForRun(record, record.durableStreamUrl !== null),
      ),
    }),
  )

const manualRunRequestForDefinition = (
  definition: AgentDefinition,
  input: AgentDefinitionRunRequest,
): Readonly<{ kind: 'ok'; request: AgentDefinitionRunRequest }>
  | Readonly<{ kind: 'invalid'; reason: string }> => {
  const manualTriggerRef = manualTriggerRefForDefinition(definition)

  if (manualTriggerRef === undefined) {
    return {
      kind: 'invalid',
      reason: 'definition must include a manual trigger for run-now.',
    }
  }

  if (input.triggerRef !== undefined && input.triggerRef !== manualTriggerRef) {
    return {
      kind: 'invalid',
      reason: 'run-now triggerRef must name the definition manual trigger.',
    }
  }

  return {
    kind: 'ok',
    request: {
      ...input,
      triggerRef: manualTriggerRef,
      triggerPayload: {
        ...(input.triggerPayload ?? {}),
        manualRunNow: true,
      },
    },
  }
}

type AgentDefinitionRunRefusal = Readonly<{
  error: string
  evidenceRefs: ReadonlyArray<string>
  kind: 'rejected'
  reason: string
  requestedPylonRef: string | null
  statusCode: number
}>

const unsupportedLaneRefusal = (
  lane: AgentDefinition['lane'],
): AgentDefinitionRunRefusal => ({
  error: 'target_pylon_unavailable',
  evidenceRefs: [
    'evidence.agent_definition_run.lane.unsupported',
    `evidence.agent_definition_run.lane.${lane}`,
  ],
  kind: 'rejected',
  reason:
    'Only lane=own_pylon definitions can be dispatched by this route right now.',
  requestedPylonRef: null,
  statusCode: 409,
})

const unsupportedHarnessRefusal = (
  definition: AgentDefinition,
): AgentDefinitionRunRefusal => ({
  error: 'target_pylon_unavailable',
  evidenceRefs: [
    'evidence.agent_definition_run.harness_adapter_unavailable',
    `evidence.agent_definition_run.harness.${definition.harness.kind}`,
  ],
  kind: 'rejected',
  reason:
    'This definition harness does not have an own_pylon dispatch adapter yet.',
  requestedPylonRef: null,
  statusCode: 409,
})

const unavailableRefusal = (): Extract<
  CodingDelegationResult,
  { kind: 'rejected' }
> => ({
  error: 'target_pylon_unavailable',
  evidenceRefs: ['evidence.agent_definition_run.no_linked_pylon_capacity'],
  kind: 'rejected',
  reason:
    'No linked, heartbeat-fresh Pylon capacity is available for this definition run.',
  requestedPylonRef: null,
  statusCode: 503,
})

const invalidBudgetRefusal = (reason: string): AgentDefinitionRunRefusal => ({
  error: 'agent_definition_budget_invalid',
  evidenceRefs: ['evidence.agent_definition_run.budget.invalid'],
  kind: 'rejected',
  reason,
  requestedPylonRef: null,
  statusCode: 400,
})

const runsPerDayBudgetRefusal = (
  input: Readonly<{
    maxRunsPerDay: number
    runCount: number
  }>,
): AgentDefinitionRunRefusal => ({
  error: 'agent_definition_budget_runs_exhausted',
  evidenceRefs: [
    'evidence.agent_definition_run.budget.max_runs_per_day_exhausted',
    `budget.agent_definition.max_runs_per_day.${input.maxRunsPerDay}`,
  ],
  kind: 'rejected',
  reason:
    `This background-agent definition has already used ${input.runCount} ` +
    `of ${input.maxRunsPerDay} allowed runs for the current UTC day.`,
  requestedPylonRef: null,
  statusCode: 429,
})

const creditsPerDayBudgetRefusal = (
  input: Readonly<{
    creditsReserved: number
    maxCreditsPerDay: number
  }>,
): AgentDefinitionRunRefusal => ({
  error: 'agent_definition_budget_credits_exhausted',
  evidenceRefs: [
    'evidence.agent_definition_run.budget.max_credits_per_day_exhausted',
    `budget.agent_definition.max_credits_per_day.${input.maxCreditsPerDay}`,
  ],
  kind: 'rejected',
  reason:
    `This background-agent definition has already reserved ${input.creditsReserved} ` +
    `of ${input.maxCreditsPerDay} allowed credits for the current UTC day.`,
  requestedPylonRef: null,
  statusCode: 429,
})

const isPositiveInteger = (value: number): boolean =>
  Number.isInteger(value) && value > 0

const isNonNegativeFinite = (value: number): boolean =>
  Number.isFinite(value) && value >= 0

const utcDayWindowFor = (
  iso: string,
): Readonly<{ dayEndIso: string; dayStartIso: string }> => {
  const dayStartIso = utcStartOfDayIsoTimestamp(iso)

  return {
    dayEndIso: isoTimestampAfterIso(dayStartIso, 24 * 60 * 60 * 1000),
    dayStartIso,
  }
}

const dispatchBudgetRefusal = async (
  dependencies: Pick<AgentDefinitionRunDispatchDependencies, 'runStore'>,
  input: Readonly<{
    definition: AgentDefinition
    nowIso: string
  }>,
): Promise<AgentDefinitionRunRefusal | undefined> => {
  const budget = input.definition.budget

  if (!isPositiveInteger(budget.maxRunSeconds)) {
    return invalidBudgetRefusal(
      'maxRunSeconds must be a positive integer before dispatch.',
    )
  }

  if (!isPositiveInteger(budget.maxRunsPerDay)) {
    return invalidBudgetRefusal(
      'maxRunsPerDay must be a positive integer before dispatch.',
    )
  }

  if (
    budget.maxCreditsPerDay !== undefined &&
    !isNonNegativeFinite(budget.maxCreditsPerDay)
  ) {
    return invalidBudgetRefusal(
      'maxCreditsPerDay must be non-negative when configured.',
    )
  }

  const { dayEndIso, dayStartIso } = utcDayWindowFor(input.nowIso)
  const usage = await dependencies.runStore.readDailyBudgetUsage(
    input.definition.ownerRef.replace(/^agent:/, ''),
    input.definition.id,
    dayStartIso,
    dayEndIso,
  )

  if (usage.runCount >= budget.maxRunsPerDay) {
    return runsPerDayBudgetRefusal({
      maxRunsPerDay: budget.maxRunsPerDay,
      runCount: usage.runCount,
    })
  }

  if (
    budget.maxCreditsPerDay !== undefined &&
    usage.creditsReserved + AGENT_DEFINITION_RUN_DISPATCH_CREDITS_RESERVED >
      budget.maxCreditsPerDay
  ) {
    return creditsPerDayBudgetRefusal({
      creditsReserved: usage.creditsReserved,
      maxCreditsPerDay: budget.maxCreditsPerDay,
    })
  }

  return undefined
}

type AgentDefinitionRunForgeGitAccess = Readonly<{
  branchRef: string
  expiresAt: string
  repositoryRef: string
  scopes: ReadonlyArray<ForgeGitAccessScope>
  tokenRefs: ReadonlyArray<string>
}>

const workspaceRepositoryRefFromRequest = (
  request: AgentDefinitionRunRequest,
): string => {
  const workspace = request.workspace
  const repository =
    typeof workspace?.repository === 'object' && workspace.repository !== null
      ? workspace.repository as Record<string, unknown>
      : undefined
  const explicitRepositoryRef = repository?.repositoryRef ?? repository?.ref

  if (
    typeof explicitRepositoryRef === 'string' &&
    explicitRepositoryRef.trim() !== ''
  ) {
    return explicitRepositoryRef.trim()
  }

  const fullName = repository?.fullName

  return typeof fullName === 'string' &&
    fullName.trim().toLowerCase() === 'openagentsinc/openagents'
    ? AGENT_DEFINITION_RUN_REPOSITORY_REF
    : AGENT_DEFINITION_RUN_REPOSITORY_REF
}

const gitRefFragment = (value: string): string => {
  const fragment = value.replaceAll(/[^A-Za-z0-9._-]/g, '_').slice(0, 120)

  return fragment === '' ? 'run' : fragment
}

const forgeBranchRefForRun = (runId: string): string =>
  `refs/heads/background-agents/${gitRefFragment(runId)}`

const forgeGitTokenRefForRun = (runId: string): string =>
  `forge_git_token.background_agent.${refFragment(runId)}.receive_pack`

const forgeGitTokenExpiresAt = (
  definition: AgentDefinition,
  nowIso: string,
): string =>
  isoTimestampAfterIso(
    nowIso,
    Math.min(
      definition.budget.maxRunSeconds * 1000 +
        AGENT_DEFINITION_RUN_FORGE_GIT_TOKEN_BUFFER_MS,
      AGENT_DEFINITION_RUN_FORGE_GIT_TOKEN_MAX_TTL_MS,
    ),
  )

const forgeGitScopeRefusal = (
  result: Exclude<
    ReturnType<typeof compileAgentDefinitionForgeGitAccessScopes>,
    { status: 'allowed' }
  >,
): AgentDefinitionRunRefusal => ({
  error:
    result.status === 'denied'
      ? 'agent_definition_forge_git_scope_denied'
      : 'agent_definition_forge_git_scope_requires_operator',
  evidenceRefs: uniqueRefs([
    'evidence.agent_definition_run.forge_git_scope_policy',
    ...result.blockerRefs,
    ...result.escalationRefs,
  ]),
  kind: 'rejected',
  reason:
    result.status === 'denied'
      ? `The definition toolset denied the Forge git token scope (${result.reasonRef}).`
      : `The definition toolset requires operator approval before minting a Forge git token (${result.reasonRef}).`,
  requestedPylonRef: null,
  statusCode: result.status === 'denied' ? 403 : 409,
})

const registerForgeWorkRecord = async (
  dependencies: Pick<AgentDefinitionRunDispatchDependencies, 'forgeStore'>,
  input: Readonly<{
    definition: AgentDefinition
    forgeWorkRef: string
    gitTokenRefs?: ReadonlyArray<string>
    nowIso: string
    runId: string
    triggerRef: string
  }>,
) =>
  dependencies.forgeStore.upsertIssue({
    tenantRef: AGENT_DEFINITION_RUN_TENANT_REF,
    issueRef: input.forgeWorkRef,
    title: `Background agent run: ${input.definition.slug}`,
    state: 'open',
    priorityRef: 'priority.background_agents.dispatch',
    sourceRefs: runSourceRefs({
      definition: input.definition,
      runId: input.runId,
      triggerRef: input.triggerRef,
    }),
    gitTokenRefs: input.gitTokenRefs ?? [],
    nowIso: input.nowIso,
  })

const mintForgeGitAccessForRun = async (
  dependencies: Pick<
    AgentDefinitionRunDispatchDependencies,
    'forgeGitAuthStore' | 'forgeStore'
  >,
  input: Readonly<{
    definition: AgentDefinition
    forgeWorkRef: string
    nowIso: string
    request: AgentDefinitionRunRequest
    runId: string
    triggerRef: string
  }>,
): Promise<
  | Readonly<{ kind: 'minted'; gitAccess: AgentDefinitionRunForgeGitAccess }>
  | Readonly<{ kind: 'refused'; refusal: AgentDefinitionRunRefusal }>
> => {
  const policy = compileAgentDefinitionToolRuntimePolicy(input.definition)
  const tokenRef = forgeGitTokenRefForRun(input.runId)
  const compiled = compileAgentDefinitionForgeGitAccessScopes({
    policy,
    requestedScopes: AGENT_DEFINITION_RUN_FORGE_GIT_SCOPES,
    invocationRef: tokenRef,
  })

  if (compiled.status !== 'allowed') {
    return {
      kind: 'refused',
      refusal: forgeGitScopeRefusal(compiled),
    }
  }

  const repositoryRef = workspaceRepositoryRefFromRequest(input.request)
  const branchRef = forgeBranchRefForRun(input.runId)
  const expiresAt = forgeGitTokenExpiresAt(input.definition, input.nowIso)
  const sourceRefs = uniqueRefs([
    ...runSourceRefs({
      definition: input.definition,
      runId: input.runId,
      triggerRef: input.triggerRef,
    }),
    input.forgeWorkRef,
    repositoryRef,
    branchRef,
    'github.issue.8200',
  ])

  await dependencies.forgeGitAuthStore.upsertTenant({
    tenantRef: AGENT_DEFINITION_RUN_TENANT_REF,
    displayName: AGENT_DEFINITION_RUN_TENANT_DISPLAY_NAME,
    nowIso: input.nowIso,
  })

  const minted = await dependencies.forgeGitAuthStore.mintGitAccessToken({
    tenantRef: AGENT_DEFINITION_RUN_TENANT_REF,
    tokenRef,
    subjectRef: definitionRef(input.definition),
    repositoryRef,
    scopes: compiled.scopes,
    refRestrictions: [branchRef],
    agentDefinitionToolPolicy: policy,
    expiresAt,
    sourceRefs,
    nowIso: input.nowIso,
  })

  try {
    await registerForgeWorkRecord(dependencies, {
      definition: input.definition,
      forgeWorkRef: input.forgeWorkRef,
      gitTokenRefs: [minted.record.token_ref],
      nowIso: input.nowIso,
      runId: input.runId,
      triggerRef: input.triggerRef,
    })
  } catch (error) {
    await dependencies.forgeGitAuthStore
      .revokeGitAccessToken(
        AGENT_DEFINITION_RUN_TENANT_REF,
        minted.record.token_ref,
        input.nowIso,
      )
      .catch(() => undefined)
    throw error
  }

  return {
    kind: 'minted',
    gitAccess: {
      branchRef,
      expiresAt,
      repositoryRef,
      scopes: minted.scopes.map(scope => scope.scope),
      tokenRefs: [minted.record.token_ref],
    },
  }
}

const revokeForgeGitAccess = async (
  dependencies: Pick<AgentDefinitionRunDispatchDependencies, 'forgeGitAuthStore'>,
  input: Readonly<{
    gitAccess: AgentDefinitionRunForgeGitAccess | null
    nowIso: string
  }>,
): Promise<void> => {
  if (input.gitAccess === null) {
    return
  }

  await Promise.all(
    input.gitAccess.tokenRefs.map(tokenRef =>
      dependencies.forgeGitAuthStore.revokeGitAccessToken(
        AGENT_DEFINITION_RUN_TENANT_REF,
        tokenRef,
        input.nowIso,
      )
    ),
  )
}

const buildRunRecord = (input: Readonly<{
  assignment: PylonApiAssignmentRecord | null
  budgetCreditsReserved: number
  definition: AgentDefinition
  durableStreamUrl: string | null
  evidenceRefs: ReadonlyArray<string>
  forgeGitTokenRefs?: ReadonlyArray<string>
  forgeRepositoryRef?: string | null
  forgeWorkRef: string
  nowIso: string
  pylonRef: string | null
  refusalError: string | null
  refusalReason: string | null
  runId: string
  state: AgentRuntimeRun['state']
  status: AgentDefinitionRunStatus
  triggerPayload: Record<string, unknown>
  triggerRef: string
}>): AgentDefinitionRunRecord => {
  const sourceRefs = runSourceRefs({
    definition: input.definition,
    runId: input.runId,
    triggerRef: input.triggerRef,
  })
  const assignmentRef = input.assignment?.assignmentRef ?? null
  const run = buildRuntimeRun({
    assignmentRef,
    definition: input.definition,
    forgeWorkRef: input.forgeWorkRef,
    runId: input.runId,
    sourceRefs,
    state: input.state,
    updatedAt: input.nowIso,
  })
  const terminalRef =
    input.refusalError === null
      ? []
      : [`refusal.agent_definition_run.${refFragment(input.refusalError)}`]
  const initialEvents = [
    runtimeEvent({
      generatedAt: input.nowIso,
      refs: sourceRefs,
      runId: input.runId,
      sequence: 0,
      summary: 'Background agent run request accepted.',
      tag: 'run.input_accepted',
    }),
    runtimeEvent({
      blockerRefs: input.refusalError === null ? [] : input.evidenceRefs,
      generatedAt: input.nowIso,
      refs: uniqueRefs([
        ...sourceRefs,
        ...(assignmentRef === null ? [] : [`pylon.assignment.${assignmentRef}`]),
        ...terminalRef,
      ]),
      runId: input.runId,
      sequence: 1,
      summary:
        input.refusalError === null
          ? 'Background agent run dispatched to caller-owned Pylon capacity.'
          : 'Background agent run refused before dispatch.',
      tag: input.refusalError === null ? 'external_agent.started' : 'run.failed',
    }),
  ]

  return {
    assignmentRef,
    budgetCreditsReserved: input.budgetCreditsReserved,
    createdAt: input.nowIso,
    definitionId: input.definition.id,
    definitionRef: definitionRef(input.definition),
    durableRequestId: input.runId,
    durableStreamUrl: input.durableStreamUrl,
    evidenceRefs: uniqueRefs(input.evidenceRefs),
    forgeGitTokenRefs: uniqueRefs(input.forgeGitTokenRefs ?? []),
    forgeRepositoryRef: input.forgeRepositoryRef ?? null,
    forgeTenantRef: AGENT_DEFINITION_RUN_TENANT_REF,
    forgeWorkRef: input.forgeWorkRef,
    initialEvents,
    lane: input.definition.lane,
    ownerAgentUserId: input.definition.ownerRef.replace(/^agent:/, ''),
    pylonRef: input.pylonRef,
    refusalError: input.refusalError,
    refusalReason: input.refusalReason,
    run,
    runId: input.runId,
    status: input.status,
    triggerPayload: input.triggerPayload,
    triggerRef: input.triggerRef,
    updatedAt: input.nowIso,
  }
}

const linkedAgentsForSession = async (
  dependencies: AgentDefinitionRunRouteDependencies,
  session: ProgrammaticAgentSession,
): Promise<ReadonlyArray<LinkedAgentOwnerRecord | Readonly<{ agentUserId: string }>>> => {
  const selfLinkedAgent = [{ agentUserId: session.user.id }]
  const openauthUserId = session.credential.openauthUserId ?? null
  const linked =
    openauthUserId === null ||
    dependencies.agentStore.listLinkedAgentsForOpenAuthUser === undefined
      ? []
      : await dependencies.agentStore
          .listLinkedAgentsForOpenAuthUser(openauthUserId, 100)
          .catch(() => [])
  const byAgentUserId = [...selfLinkedAgent, ...linked].reduce(
    (current, agent) => current.set(agent.agentUserId, agent),
    new Map<string, LinkedAgentOwnerRecord | Readonly<{ agentUserId: string }>>(),
  )

  return [...byAgentUserId.values()]
}

const rawDelegationBody = (
  input: Readonly<{
    definition: AgentDefinition
    forgeGitAccess: AgentDefinitionRunForgeGitAccess
    request: AgentDefinitionRunRequest
    runId: string
  }>,
) => {
  const workspace = workspaceWithForgeScmAuthBroker({
    forgeGitAccess: input.forgeGitAccess,
    workspace: input.request.workspace,
  })

  return {
    workflowClass: workflowClassForDefinition(input.definition),
    openagents: {
      workflowClass: workflowClassForDefinition(input.definition),
      coding: {
        authorityScope: 'owner_self',
        objectiveSummary:
          input.request.objectiveSummary ??
          `Run background agent definition ${input.definition.id}.`,
        spawnRunRef: `spawn.public.khala_coding.${refFragment(input.runId)}`,
        ...(input.request.targetPylonRef === undefined
          ? {}
          : { targetPylonRef: input.request.targetPylonRef }),
        ...(input.request.targetAccountRefHash === undefined
          ? {}
          : { targetAccountRefHash: input.request.targetAccountRefHash }),
        ...(workspace === undefined ? {} : { workspace }),
        timeoutSeconds: input.definition.budget.maxRunSeconds,
      },
    },
  }
}

const forgeGitCredentialPathPrefix = (
  input: Readonly<{ repositoryRef: string }>,
): string =>
  `/git/${encodeURIComponent(
    AGENT_DEFINITION_RUN_TENANT_REF,
  )}/${encodeURIComponent(input.repositoryRef)}.git`

const workspaceWithForgeScmAuthBroker = (
  input: Readonly<{
    forgeGitAccess: AgentDefinitionRunForgeGitAccess
    workspace: AgentDefinitionRunRequest['workspace']
  }>,
): AgentDefinitionRunRequest['workspace'] => {
  if (input.workspace === undefined) return undefined
  if (input.workspace.kind !== 'git_checkout') return input.workspace

  return {
    ...input.workspace,
    scmAuthBroker: {
      schema: AGENT_DEFINITION_RUN_SCM_AUTH_BROKER_SCHEMA,
      kind: 'forge_git_access',
      brokerUrl: AGENT_DEFINITION_RUN_SCM_AUTH_BROKER_URL,
      authRefs: input.forgeGitAccess.tokenRefs,
      repositoryRef: input.forgeGitAccess.repositoryRef,
      allowed: {
        protocol: 'https',
        host: 'openagents.com',
        pathPrefix: forgeGitCredentialPathPrefix({
          repositoryRef: input.forgeGitAccess.repositoryRef,
        }),
      },
      cacheTtlSeconds: AGENT_DEFINITION_RUN_SCM_AUTH_BROKER_CACHE_TTL_SECONDS,
      fallback: 'fail_closed',
    },
  }
}

const classificationForDefinition = (
  definition: AgentDefinition,
): CodingWorkflowClassification | null => {
  const workflowClass = workflowClassForDefinition(definition)

  return workflowClass === null
    ? null
    : {
        confidence: 1,
        evidenceRefs: [
          'evidence.agent_definition_run.definition_backed',
          `evidence.agent_definition_run.workflow.${workflowClass}`,
        ],
        workflowClass,
      }
}

const persistRefusedRun = async (
  dependencies: AgentDefinitionRunDispatchDependencies,
  input: Readonly<{
    definition: AgentDefinition
    forgeWorkRef: string
    nowIso: string
    refusal: AgentDefinitionRunRefusal
    runId: string
    triggerPayload: Record<string, unknown>
    triggerRef: string
  }>,
): Promise<Readonly<{ record: AgentDefinitionRunRecord; seeded: boolean }>> => {
  const record = buildRunRecord({
    assignment: null,
    budgetCreditsReserved: 0,
    definition: input.definition,
    durableStreamUrl: null,
    evidenceRefs: input.refusal.evidenceRefs,
    forgeWorkRef: input.forgeWorkRef,
    nowIso: input.nowIso,
    pylonRef: input.refusal.requestedPylonRef,
    refusalError: input.refusal.error,
    refusalReason: input.refusal.reason,
    runId: input.runId,
    state: 'failed',
    status: 'refused',
    triggerPayload: input.triggerPayload,
    triggerRef: input.triggerRef,
  })
  const stored = await dependencies.runStore.upsertRun(record)
  const seeded = await seedSessionEvents(dependencies, {
    close: true,
    durableRequestId: input.runId,
    events: stored.initialEvents,
  })

  return { record: stored, seeded }
}

export const dispatchAgentDefinitionRun = async (
  dependencies: AgentDefinitionRunDispatchDependencies,
  input: Readonly<{
    definition: AgentDefinition
    request: AgentDefinitionRunRequest
  }>,
): Promise<AgentDefinitionRunDispatchOutcome> => {
  const trigger = triggerRefFromRequest(input.definition, input.request)

  if (trigger.kind === 'invalid') {
    return {
      kind: 'invalid',
      reason: 'triggerRef must name one of the definition triggers.',
    }
  }

  const nowIso = dependencies.nowIso?.() ?? currentIsoTimestamp()
  const runId = makeRunId(dependencies)
  const forgeWorkRef = `work.background_agent.${refFragment(runId)}`
  const triggerPayload = input.request.triggerPayload ?? {}

  try {
    await registerForgeWorkRecord(dependencies, {
      definition: input.definition,
      forgeWorkRef,
      nowIso,
      runId,
      triggerRef: trigger.triggerRef,
    })
  } catch {
    return { kind: 'storage_error' }
  }

  const budgetRefusal = await dispatchBudgetRefusal(dependencies, {
    definition: input.definition,
    nowIso,
  }).catch(() => 'storage_error' as const)

  if (budgetRefusal === 'storage_error') {
    return { kind: 'storage_error' }
  }

  if (budgetRefusal !== undefined) {
    try {
      const refused = await persistRefusedRun(dependencies, {
        definition: input.definition,
        forgeWorkRef,
        nowIso,
        refusal: budgetRefusal,
        runId,
        triggerPayload,
        triggerRef: trigger.triggerRef,
      })

      return {
        evidenceRefs: budgetRefusal.evidenceRefs,
        error: budgetRefusal.error,
        kind: 'refused',
        reason: budgetRefusal.reason,
        record: refused.record,
        requestedPylonRef: budgetRefusal.requestedPylonRef,
        seeded: refused.seeded,
        statusCode: budgetRefusal.statusCode,
      }
    } catch {
      return { kind: 'storage_error' }
    }
  }

  if (input.definition.lane !== 'own_pylon') {
    const refusal = unsupportedLaneRefusal(input.definition.lane)

    try {
      const refused = await persistRefusedRun(dependencies, {
        definition: input.definition,
        forgeWorkRef,
        nowIso,
        refusal,
        runId,
        triggerPayload,
        triggerRef: trigger.triggerRef,
      })

      return {
        evidenceRefs: refusal.evidenceRefs,
        error: refusal.error,
        kind: 'refused',
        reason: refusal.reason,
        record: refused.record,
        requestedPylonRef: refusal.requestedPylonRef,
        seeded: refused.seeded,
        statusCode: refusal.statusCode,
      }
    } catch {
      return { kind: 'storage_error' }
    }
  }

  const classification = classificationForDefinition(input.definition)

  if (classification === null) {
    const refusal = unsupportedHarnessRefusal(input.definition)

    try {
      const refused = await persistRefusedRun(dependencies, {
        definition: input.definition,
        forgeWorkRef,
        nowIso,
        refusal,
        runId,
        triggerPayload,
        triggerRef: trigger.triggerRef,
      })

      return {
        evidenceRefs: refusal.evidenceRefs,
        error: refusal.error,
        kind: 'refused',
        reason: refusal.reason,
        record: refused.record,
        requestedPylonRef: refusal.requestedPylonRef,
        seeded: refused.seeded,
        statusCode: refusal.statusCode,
      }
    } catch {
      return { kind: 'storage_error' }
    }
  }

  const mintedGitAccess = await mintForgeGitAccessForRun(dependencies, {
    definition: input.definition,
    forgeWorkRef,
    nowIso,
    request: input.request,
    runId,
    triggerRef: trigger.triggerRef,
  }).catch(() => 'storage_error' as const)

  if (mintedGitAccess === 'storage_error') {
    return { kind: 'storage_error' }
  }

  if (mintedGitAccess.kind === 'refused') {
    const refusal = mintedGitAccess.refusal

    try {
      const refused = await persistRefusedRun(dependencies, {
        definition: input.definition,
        forgeWorkRef,
        nowIso,
        refusal,
        runId,
        triggerPayload,
        triggerRef: trigger.triggerRef,
      })

      return {
        evidenceRefs: refusal.evidenceRefs,
        error: refusal.error,
        kind: 'refused',
        reason: refusal.reason,
        record: refused.record,
        requestedPylonRef: refusal.requestedPylonRef,
        seeded: refused.seeded,
        statusCode: refusal.statusCode,
      }
    } catch {
      return { kind: 'storage_error' }
    }
  }

  const forgeGitAccess = mintedGitAccess.gitAccess
  let delegation: CodingDelegationResult | null

  try {
    delegation = await delegateCodingWorkflow({
      classification,
      linkedAgents: dependencies.linkedAgents,
      makeId: () => dependencies.makeId?.() ?? randomUuid(),
      nowIso,
      pylonStore: dependencies.pylonStore,
      rawBody: rawDelegationBody({
        definition: input.definition,
        forgeGitAccess,
        request: input.request,
        runId,
      }),
      requestId: runId,
    })
  } catch {
    delegation = {
      error: 'coding_delegation_store_unavailable',
      evidenceRefs: ['evidence.agent_definition_run.dispatch.unavailable'],
      kind: 'rejected',
      reason:
        'The background-agent dispatch gate could not read linked Pylon capacity right now.',
      requestedPylonRef: null,
      statusCode: 503,
    }
  }

  if (delegation === null) {
    delegation = unavailableRefusal()
  }

  if (delegation.kind === 'rejected') {
    try {
      await revokeForgeGitAccess(dependencies, {
        gitAccess: forgeGitAccess,
        nowIso,
      })
      const refused = await persistRefusedRun(dependencies, {
        definition: input.definition,
        forgeWorkRef,
        nowIso,
        refusal: delegation,
        runId,
        triggerPayload,
        triggerRef: trigger.triggerRef,
      })

      return {
        evidenceRefs: delegation.evidenceRefs,
        error: delegation.error,
        kind: 'refused',
        reason: delegation.reason,
        record: refused.record,
        requestedPylonRef: delegation.requestedPylonRef,
        seeded: refused.seeded,
        statusCode: delegation.statusCode,
      }
    } catch {
      return { kind: 'storage_error' }
    }
  }

  const record = buildRunRecord({
    assignment: delegation.assignment,
    budgetCreditsReserved: AGENT_DEFINITION_RUN_DISPATCH_CREDITS_RESERVED,
    definition: input.definition,
    durableStreamUrl: delegation.durableStreamUrl,
    evidenceRefs: [
      ...delegation.evidenceRefs,
      'evidence.agent_definition_run.forge_work_record_registered',
      'evidence.agent_definition_run.forge_git_token_minted',
      'evidence.agent_definition_run.exact_accounting_assignment_ref',
    ],
    forgeGitTokenRefs: forgeGitAccess.tokenRefs,
    forgeRepositoryRef: forgeGitAccess.repositoryRef,
    forgeWorkRef,
    nowIso,
    pylonRef: delegation.pylon.pylonRef,
    refusalError: null,
    refusalReason: null,
    runId,
    state: 'running',
    status: 'dispatched',
    triggerPayload,
    triggerRef: trigger.triggerRef,
  })

  try {
    const stored = await dependencies.runStore.upsertRun(record)
    const seeded = await seedSessionEvents(dependencies, {
      close: false,
      durableRequestId: stored.durableRequestId,
      events: stored.initialEvents,
    })

    return {
      assignmentRef: delegation.assignment.assignmentRef,
      durableStreamUrl: delegation.durableStreamUrl,
      kind: 'dispatched',
      record: stored,
      seeded,
    }
  } catch {
    await revokeForgeGitAccess(dependencies, {
      gitAccess: forgeGitAccess,
      nowIso,
    }).catch(() => undefined)
    return { kind: 'storage_error' }
  }
}

export type AgentDefinitionRunForgeGitTokenRevocationResult = Readonly<{
  assignmentRef: string
  foundRun: boolean
  revokedTokenRefs: ReadonlyArray<string>
}>

export const revokeAgentDefinitionRunForgeGitTokensForAssignment = async (
  dependencies: Pick<AgentDefinitionRunDispatchDependencies, 'forgeGitAuthStore' | 'runStore'>,
  input: Readonly<{
    assignmentRef: string
    nowIso: string
  }>,
): Promise<AgentDefinitionRunForgeGitTokenRevocationResult> => {
  const run = await dependencies.runStore.readRunByAssignmentRef(input.assignmentRef)

  if (run === undefined) {
    return {
      assignmentRef: input.assignmentRef,
      foundRun: false,
      revokedTokenRefs: [],
    }
  }

  await Promise.all(
    run.forgeGitTokenRefs.map(tokenRef =>
      dependencies.forgeGitAuthStore.revokeGitAccessToken(
        run.forgeTenantRef,
        tokenRef,
        input.nowIso,
      )
    ),
  )

  if (run.forgeGitTokenRefs.length > 0) {
    await dependencies.runStore.upsertRun({
      ...run,
      evidenceRefs: uniqueRefs([
        ...run.evidenceRefs,
        'evidence.agent_definition_run.forge_git_tokens_revoked',
      ]),
      updatedAt: input.nowIso,
    })
  }

  return {
    assignmentRef: input.assignmentRef,
    foundRun: true,
    revokedTokenRefs: run.forgeGitTokenRefs,
  }
}

export const handleAgentDefinitionRunRequest = async (
  request: Request,
  dependencies: AgentDefinitionRunRouteDependencies,
): Promise<HttpResponse | undefined> => {
  const matched = matchAgentDefinitionRunRequest(request)

  if (matched === undefined) {
    return undefined
  }

  const allowedMethods = matched.route === 'history' ? ['GET', 'POST'] : ['POST']

  if (!allowedMethods.includes(request.method)) {
    return withAgentRateLimitHeaders(methodNotAllowed(allowedMethods))
  }

  const session = await requireAgentSession(request, dependencies)

  if (session === undefined) {
    return withAgentRateLimitHeaders(unauthorized())
  }

  const definition = await dependencies.definitionStore.readDefinition(
    session.user.id,
    matched.definitionId,
  )

  if (definition === undefined) {
    return notFoundResponse()
  }

  if (matched.route === 'history' && request.method === 'GET') {
    const limit = boundedRunHistoryLimitFromRequest(request)

    try {
      return listRunsResponse({
        definition,
        limit,
        runs: await dependencies.runStore.listRunsForDefinition(
          session.user.id,
          matched.definitionId,
          limit,
        ),
      })
    } catch {
      return storageErrorResponse()
    }
  }

  let body: AgentDefinitionRunRequest

  try {
    body = decodeUnknownWithSchema(
      AgentDefinitionRunRequest,
      await readJsonObject(request),
    )
  } catch (error) {
    return invalidRunResponse(
      error instanceof Error ? error.message : String(error),
    )
  }

  if (matched.route === 'run_now') {
    const manual = manualRunRequestForDefinition(definition, body)

    if (manual.kind === 'invalid') {
      return invalidRunResponse(manual.reason)
    }

    body = manual.request
  }

  const dispatch = await dispatchAgentDefinitionRun({
    durableStreamNamespace: dependencies.durableStreamNamespace,
    forgeGitAuthStore: dependencies.forgeGitAuthStore,
    forgeStore: dependencies.forgeStore,
    linkedAgents: await linkedAgentsForSession(dependencies, session),
    makeId: dependencies.makeId,
    nowIso: dependencies.nowIso,
    pylonStore: dependencies.pylonStore,
    runStore: dependencies.runStore,
  }, {
    definition,
    request: body,
  })

  if (dispatch.kind === 'invalid') {
    return invalidRunResponse(dispatch.reason)
  }

  if (dispatch.kind === 'storage_error') {
    return storageErrorResponse()
  }

  if (dispatch.kind === 'refused') {
    return withAgentRateLimitHeaders(
      noStoreJsonResponse(
        {
          error: dispatch.error,
          evidenceRefs: dispatch.evidenceRefs,
          reason: dispatch.reason,
          requestedPylonRef: dispatch.requestedPylonRef,
          run: projectionForRun(dispatch.record, dispatch.seeded),
        },
        { status: dispatch.statusCode },
      ),
    )
  }

  return withAgentRateLimitHeaders(
    noStoreJsonResponse(
      { run: projectionForRun(dispatch.record, dispatch.seeded) },
      {
        headers: {
          'openagents-coding-assignment-ref': dispatch.assignmentRef,
          'openagents-durable-stream-url': dispatch.durableStreamUrl,
        },
        status: 201,
      },
    ),
  )
}
