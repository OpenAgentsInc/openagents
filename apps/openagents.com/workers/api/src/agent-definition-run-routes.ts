import {
  AgentRuntimeEvent as AgentRuntimeEventSchema,
  AgentRuntimeRun as AgentRuntimeRunSchema,
  type AgentDefinition,
  type AgentRuntimeAdapterKind,
  type AgentRuntimeEvent,
  type AgentRuntimeRun,
} from '@openagentsinc/agent-runtime-schema'
import { Schema as S } from 'effect'

import {
  type AgentDefinitionStore,
} from './agent-definition-routes'
import { withAgentRateLimitHeaders } from './agent-rate-limit-policy'
import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
  type LinkedAgentOwnerRecord,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
} from './agent-registration'
import {
  type ForgeCoordinationStore,
} from './forge-coordination-store'
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
const AGENT_DEFINITION_RUN_SCHEMA =
  'openagents.agent_definition_run.v0.1' as const
const AGENT_DEFINITION_RUN_SESSION_EVENT_SCHEMA =
  'openagents.agent_definition_run.session_event.v0.1' as const
const AGENT_DEFINITION_RUN_DISPATCH_CREDITS_RESERVED = 0

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
  upsertRun: (record: AgentDefinitionRunRecord) => Promise<AgentDefinitionRunRecord>
  readRun: (
    ownerAgentUserId: string,
    runId: string,
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
  upsertRun: async record => {
    await db
      .prepare(
        `INSERT INTO agent_definition_runs (
           run_id, owner_agent_user_id, definition_id, definition_ref,
           trigger_ref, lane, status, pylon_ref, assignment_ref,
           durable_request_id, durable_stream_url, forge_tenant_ref,
           forge_work_ref, refusal_error, refusal_reason, evidence_refs_json,
           trigger_payload_json, runtime_run_json, initial_events_json,
           budget_credits_reserved, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(run_id) DO UPDATE SET
           status = excluded.status,
           pylon_ref = excluded.pylon_ref,
           assignment_ref = excluded.assignment_ref,
           durable_stream_url = excluded.durable_stream_url,
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
  forgeStore: ForgeCoordinationStore
  makeId?: () => string
  nowIso?: () => string
  pylonStore: PylonApiStore
  runStore: AgentDefinitionRunStore
}>

export type AgentDefinitionRunDispatchDependencies = Readonly<{
  durableStreamNamespace?: DurableStreamNamespace | undefined
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

export const matchAgentDefinitionRunRequest = (
  request: Request,
): Readonly<{ definitionId: string }> | undefined => {
  const match = /^\/v1\/agent-definitions\/([^/]+)\/runs$/.exec(
    new URL(request.url).pathname,
  )

  if (match === null) {
    return undefined
  }

  try {
    const definitionId = decodeURIComponent(match[1] ?? '').trim()

    return definitionId === '' ? undefined : { definitionId }
  } catch {
    return undefined
  }
}

const bearerTokenFromRequest = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')

  if (authorization === null) {
    return undefined
  }

  const [scheme, token] = authorization.split(' ')

  return scheme?.toLowerCase() === 'bearer' &&
    token !== undefined &&
    token.startsWith(AGENT_TOKEN_PREFIX)
    ? token
    : undefined
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

const projectionForRun = (
  record: AgentDefinitionRunRecord,
  sessionEventStreamSeeded: boolean,
) => ({
  schema: AGENT_DEFINITION_RUN_SCHEMA,
  assignmentRef: record.assignmentRef,
  definitionRef: record.definitionRef,
  durableRequestId: record.durableRequestId,
  durableStreamUrl: record.durableStreamUrl,
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
  runId: record.runId,
  sessionEventStream: {
    seeded: sessionEventStreamSeeded,
    eventCount: record.initialEvents.length,
    durableRequestId: record.durableRequestId,
    durableStreamUrl: record.durableStreamUrl,
  },
  status: record.status,
  triggerRef: record.triggerRef,
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

const buildRunRecord = (input: Readonly<{
  assignment: PylonApiAssignmentRecord | null
  budgetCreditsReserved: number
  definition: AgentDefinition
  durableStreamUrl: string | null
  evidenceRefs: ReadonlyArray<string>
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

const registerForgeWorkRecord = async (
  dependencies: Pick<AgentDefinitionRunDispatchDependencies, 'forgeStore'>,
  input: Readonly<{
    definition: AgentDefinition
    forgeWorkRef: string
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
    nowIso: input.nowIso,
  })

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
    request: AgentDefinitionRunRequest
    runId: string
  }>,
) => ({
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
      ...(input.request.workspace === undefined
        ? {}
        : { workspace: input.request.workspace }),
      timeoutSeconds: input.definition.budget.maxRunSeconds,
    },
  },
})

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
      'evidence.agent_definition_run.exact_accounting_assignment_ref',
    ],
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
    return { kind: 'storage_error' }
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

  if (request.method !== 'POST') {
    return withAgentRateLimitHeaders(methodNotAllowed(['POST']))
  }

  const session = await requireAgentSession(request, dependencies)

  if (session === undefined) {
    return withAgentRateLimitHeaders(unauthorized())
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

  const definition = await dependencies.definitionStore.readDefinition(
    session.user.id,
    matched.definitionId,
  )

  if (definition === undefined) {
    return notFoundResponse()
  }

  const dispatch = await dispatchAgentDefinitionRun({
    durableStreamNamespace: dependencies.durableStreamNamespace,
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
