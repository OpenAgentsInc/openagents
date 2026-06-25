import type { LinkedAgentOwnerRecord } from '../agent-registration'
import {
  type PylonApiAssignmentRecord,
  type PylonApiCreateAssignmentRequest,
  type PylonApiRegistrationRecord,
  type PylonApiStore,
  PylonApiStoreError,
  buildPylonApiAssignmentRecord,
  pylonCodingServiceCapacityProjection,
} from '../pylon-api'
import { controlledPylonAssignmentDispatchGate } from '../pylon-api-routes'
import type { CodingWorkflowClassification } from './coding-workflow-classifier'

const CODEX_AGENT_CAPABILITY_REF = 'capability.pylon.local_codex'
const CODEX_AGENT_TASK_SCHEMA = 'openagents.pylon.codex_agent_task.v0.3'
const CODEX_AGENT_SUM_REPAIR_FIXTURE_REF =
  'fixture.public.pylon.codex_agent.sum_repair.v1'

export type CodingDelegationInput = Readonly<{
  classification: CodingWorkflowClassification
  linkedAgents: ReadonlyArray<LinkedAgentOwnerRecord>
  makeId: () => string
  nowIso: string
  pylonStore: PylonApiStore
  rawBody: unknown
  requestId: string
}>

export type CodingDelegationAssignmentResult = Readonly<{
  assignment: PylonApiAssignmentRecord
  durableStreamUrl: string
  evidenceRefs: ReadonlyArray<string>
  kind: 'assigned'
  pylon: PylonApiRegistrationRecord
}>

export type CodingDelegationRejection = Readonly<{
  error:
    | 'invalid_target_pylon_ref'
    | 'target_pylon_not_authorized'
    | 'target_pylon_unavailable'
  evidenceRefs: ReadonlyArray<string>
  kind: 'rejected'
  reason: string
  requestedPylonRef: string | null
  statusCode: 400 | 403 | 409
}>

export type CodingDelegationResult =
  | CodingDelegationAssignmentResult
  | CodingDelegationRejection

const requestIdRef = (requestId: string): string =>
  `request.public.khala_coding.${requestId.replaceAll(/[^A-Za-z0-9_.:-]/g, '_')}`

const workflowRef = (classification: CodingWorkflowClassification): string =>
  `workflow.public.khala_coding.${classification.workflowClass}`

const rawCodingFromBody = (body: unknown): Record<string, unknown> | null => {
  const openagents =
    body !== null && typeof body === 'object'
      ? (body as Record<string, unknown>).openagents
      : undefined
  const coding =
    openagents !== null && typeof openagents === 'object'
      ? (openagents as Record<string, unknown>).coding
      : undefined

  return coding !== null && typeof coding === 'object'
    ? (coding as Record<string, unknown>)
    : null
}

const rawWorkspaceFromBody = (
  body: unknown,
): Record<string, unknown> | null => {
  const workspace = rawCodingFromBody(body)?.workspace

  return workspace !== null && typeof workspace === 'object'
    ? (workspace as Record<string, unknown>)
    : null
}

const pylonRefPattern = /^[a-z0-9][a-z0-9_.:-]{2,119}$/

const targetPylonRefFromBody = (
  body: unknown,
):
  | Readonly<{ kind: 'none' }>
  | Readonly<{ kind: 'target'; pylonRef: string }>
  | Readonly<{
      kind: 'rejected'
      rejection: CodingDelegationRejection
    }> => {
  const coding = rawCodingFromBody(body)
  const rawValue = coding?.targetPylonRef ?? coding?.pylonRef

  if (rawValue === undefined || rawValue === null || rawValue === '') {
    return { kind: 'none' }
  }

  if (typeof rawValue !== 'string') {
    return {
      kind: 'rejected',
      rejection: {
        error: 'invalid_target_pylon_ref',
        evidenceRefs: ['evidence.khala_coding.target_pylon_ref.invalid_type'],
        kind: 'rejected',
        reason:
          'openagents.coding.targetPylonRef must be a public-safe Pylon ref string.',
        requestedPylonRef: null,
        statusCode: 400,
      },
    }
  }

  const pylonRef = rawValue.trim()
  if (!pylonRefPattern.test(pylonRef)) {
    return {
      kind: 'rejected',
      rejection: {
        error: 'invalid_target_pylon_ref',
        evidenceRefs: ['evidence.khala_coding.target_pylon_ref.invalid_ref'],
        kind: 'rejected',
        reason:
          'openagents.coding.targetPylonRef must match the Pylon public ref contract.',
        requestedPylonRef: pylonRef,
        statusCode: 400,
      },
    }
  }

  return { kind: 'target', pylonRef }
}

const codingAssignmentFromInput = (
  input: CodingDelegationInput,
): Record<string, unknown> => {
  const workspace = rawWorkspaceFromBody(input.rawBody)

  return {
    codex: {
      agentKind: 'codex_sdk',
      schema: CODEX_AGENT_TASK_SCHEMA,
      ...(workspace === null
        ? { fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF }
        : {}),
      timeoutSeconds: 1200,
    },
    objective: {
      objectiveRef: workflowRef(input.classification),
      publicSummary:
        'Run the caller-owned Khala coding workflow on a linked local Codex Pylon.',
    },
    ...(workspace === null ? {} : { workspace }),
    requiredCapabilityRefs: [CODEX_AGENT_CAPABILITY_REF],
    routing: {
      durableStreamRef: requestIdRef(input.requestId),
      schema: 'openagents.khala.coding_delegation.v1',
    },
  }
}

const assignmentRequestFromInput = (
  input: CodingDelegationInput,
  pylonRef: string,
): PylonApiCreateAssignmentRequest => ({
  acceptanceCriteriaRefs: [
    'acceptance.public.khala_coding.owner_requested',
    ...input.classification.evidenceRefs,
  ],
  assignmentRef: `assignment.public.khala_coding.${input.makeId()}`,
  campaignPaused: false,
  campaignPolicyRefs: ['policy.public.khala_coding.own_capacity_only'],
  campaignRef: 'campaign.public.khala_coding.own_capacity',
  closeoutPathRefs: [
    'closeout.public.khala_coding.durable_stream',
    requestIdRef(input.requestId),
  ],
  codingAssignment: codingAssignmentFromInput(input),
  forumAutoPublishAllowed: false,
  idempotencyRefs: ['idempotency.public.khala_coding.request'],
  jobKind: 'codex_agent_task',
  leaseSeconds: 3600,
  noDuplicateAssignmentRefs: ['dedupe.public.pylon_assignment.active_lease'],
  noForumAutoPublishRefs: ['policy.public.no_forum_auto_publish'],
  operatorPauseRefs: ['pause.public.khala_coding.kill_switch_default_off'],
  paymentMode: 'unpaid_smoke',
  pylonRef,
  requiredCapabilityRefs: [CODEX_AGENT_CAPABILITY_REF],
  resultExpectationRefs: ['result.public.khala_coding.worker_closeout'],
  rollbackRefs: ['rollback.public.khala_coding.assignment_cancel'],
  selectionPolicyRefs: ['selection.public.khala_coding.codex_first'],
  spendCapRefs: [],
  taskRefs: [workflowRef(input.classification), requestIdRef(input.requestId)],
})

const hasFreshOnlineHeartbeat = (
  registration: PylonApiRegistrationRecord,
  nowIso: string,
): boolean => {
  if (registration.latestHeartbeatAt === null) {
    return false
  }

  const now = Date.parse(nowIso)
  const heartbeat = Date.parse(registration.latestHeartbeatAt)
  const status = (registration.latestHeartbeatStatus ?? '').trim().toLowerCase()

  return (
    Number.isFinite(now) &&
    Number.isFinite(heartbeat) &&
    now - heartbeat >= 0 &&
    now - heartbeat <= 5 * 60 * 1000 &&
    ['available', 'healthy', 'idle', 'online', 'ready'].includes(status)
  )
}

const hasAvailableCodexCapacity = (
  registration: PylonApiRegistrationRecord,
): boolean =>
  registration.capabilityRefs.includes(CODEX_AGENT_CAPABILITY_REF) &&
  pylonCodingServiceCapacityProjection(registration).some(
    capacity => capacity.service === 'codex' && capacity.available > 0,
  )

export const delegateCodingWorkflow = async (
  input: CodingDelegationInput,
): Promise<CodingDelegationResult | null> => {
  if (input.classification.workflowClass === 'none') {
    return null
  }

  const target = targetPylonRefFromBody(input.rawBody)
  if (target.kind === 'rejected') {
    return target.rejection
  }

  const ownerAgentUserIds = input.linkedAgents.map(agent => agent.agentUserId)
  if (ownerAgentUserIds.length === 0) {
    return target.kind === 'target'
      ? {
          error: 'target_pylon_not_authorized',
          evidenceRefs: [
            'evidence.khala_coding.target_pylon_ref.no_linked_agents',
          ],
          kind: 'rejected',
          reason:
            'The requested Pylon is not linked to this OpenAuth account.',
          requestedPylonRef: target.pylonRef,
          statusCode: 403,
        }
      : null
  }

  const registrations =
    input.pylonStore.listRegistrationsForOwnerAgentUserIds === undefined
      ? (await input.pylonStore.listRegistrations(200)).filter(registration =>
          ownerAgentUserIds.includes(registration.ownerAgentUserId),
        )
      : await input.pylonStore.listRegistrationsForOwnerAgentUserIds(
          ownerAgentUserIds,
          200,
        )

  const authorizedRegistrations =
    target.kind === 'target'
      ? registrations.filter(
          registration => registration.pylonRef === target.pylonRef,
        )
      : registrations

  if (target.kind === 'target' && authorizedRegistrations.length === 0) {
    return {
      error: 'target_pylon_not_authorized',
      evidenceRefs: ['evidence.khala_coding.target_pylon_ref.not_linked'],
      kind: 'rejected',
      reason:
        'The requested Pylon is not linked to this OpenAuth account and cannot be used for caller-owned Khala coding capacity.',
      requestedPylonRef: target.pylonRef,
      statusCode: 403,
    }
  }

  const candidates = authorizedRegistrations
    .filter(registration => registration.status === 'active')
    .filter(registration => hasFreshOnlineHeartbeat(registration, input.nowIso))
    .filter(hasAvailableCodexCapacity)
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))

  if (target.kind === 'target' && candidates.length === 0) {
    return {
      error: 'target_pylon_unavailable',
      evidenceRefs: ['evidence.khala_coding.target_pylon_ref.unavailable'],
      kind: 'rejected',
      reason:
        'The requested linked Pylon is not active, heartbeat-fresh, Codex-capable, and available.',
      requestedPylonRef: target.pylonRef,
      statusCode: 409,
    }
  }

  for (const registration of candidates) {
    const body = assignmentRequestFromInput(input, registration.pylonRef)
    const activeAssignments = await input.pylonStore.listAssignmentsForPylon(
      registration.pylonRef,
      100,
    )
    const gate = controlledPylonAssignmentDispatchGate({
      activeAssignments,
      assignmentRef: body.assignmentRef ?? null,
      body,
      nowIso: input.nowIso,
      registration,
    })

    if (!gate.dispatchAllowed) {
      continue
    }

    const assignment = buildPylonApiAssignmentRecord({
      idempotencyKeyHash: `khala-coding:${input.requestId}`,
      makeId: input.makeId,
      nowIso: input.nowIso,
      ownerAgentUserId: registration.ownerAgentUserId,
      request: body,
    })
    const result = await input.pylonStore.createAssignment(assignment)

    return {
      assignment: result.record,
      durableStreamUrl: `/v1/chat/completions/durable/${encodeURIComponent(input.requestId)}`,
      evidenceRefs: [
        ...input.classification.evidenceRefs,
        'evidence.khala_coding.own_capacity_linked_pylon',
      ],
      kind: 'assigned',
      pylon: registration,
    }
  }

  return null
}

export const estimatedDelegatedCodingUsage = (
  messages: ReadonlyArray<unknown>,
): { completionTokens: number; promptTokens: number; totalTokens: number } => {
  const json = JSON.stringify(messages)
  const promptTokens = Math.max(1, Math.ceil(json.length / 4))
  const completionTokens = 32
  return {
    completionTokens,
    promptTokens,
    totalTokens: promptTokens + completionTokens,
  }
}

export const codingDelegationUnavailableError = () =>
  new PylonApiStoreError({
    kind: 'not_found',
    reason:
      'No linked, heartbeat-fresh, Codex-capable Pylon capacity is available for this account.',
  })
