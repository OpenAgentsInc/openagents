import type { LinkedAgentOwnerRecord } from '../agent-registration'
import {
  type PylonApiAssignmentRecord,
  type PylonApiCreateAssignmentRequest,
  type PylonApiRegistrationRecord,
  type PylonApiStore,
  PylonApiStoreError,
  buildPylonApiAssignmentRecord,
  codexAccountCapacityKeyFromAccountRefHash,
  pylonCodexAccountCapacity,
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
  linkedAgents: ReadonlyArray<
    LinkedAgentOwnerRecord | Readonly<{ agentUserId: string }>
  >
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
    | 'coding_delegation_store_unavailable'
    | 'invalid_coding_objective_summary'
    | 'invalid_spawn_ref'
    | 'invalid_target_account_ref'
    | 'invalid_target_pylon_ref'
    | 'target_pylon_not_authorized'
    | 'target_pylon_unavailable'
  evidenceRefs: ReadonlyArray<string>
  kind: 'rejected'
  reason: string
  requestedPylonRef: string | null
  statusCode: 400 | 403 | 409 | 503
}>

type CodingDelegationStoreUnavailableStage =
  | 'assignment_create'
  | 'assignment_list_read'
  | 'assignment_request_validation'
  | 'linked_owner_registration_read'
  | 'unknown_store_operation'

export type CodingDelegationResult =
  | CodingDelegationAssignmentResult
  | CodingDelegationRejection

export const khalaCodingRequestIdRef = (requestId: string): string =>
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

const unsafeObjectiveSummaryPattern =
  /(@|\/Users\/|\/home\/|\.env|access[_-]?token|auth\.json|bearer\s+|cookie|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github[_-]?pat_[A-Za-z0-9_]+|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(key|repo)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(auth|command|content|payload|prompt|provider|runner|source|trace)|secret|seed[_-]?phrase|sk-[a-z0-9]|ssh:\/\/|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

const objectiveSummaryFromBody = (
  body: unknown,
):
  | Readonly<{ kind: 'summary'; summary: string | null }>
  | Readonly<{ kind: 'rejected'; rejection: CodingDelegationRejection }> => {
  const rawValue = rawCodingFromBody(body)?.objectiveSummary
  if (rawValue === undefined || rawValue === null || rawValue === '') {
    return { kind: 'summary', summary: null }
  }
  if (typeof rawValue !== 'string') {
    return {
      kind: 'rejected',
      rejection: {
        error: 'invalid_coding_objective_summary',
        evidenceRefs: ['evidence.khala_coding.objective_summary.invalid_type'],
        kind: 'rejected',
        reason:
          'openagents.coding.objectiveSummary must be a bounded public-safe string.',
        requestedPylonRef: null,
        statusCode: 400,
      },
    }
  }
  const summary = rawValue.trim()
  if (
    summary.length < 3 ||
    summary.length > 1000 ||
    unsafeObjectiveSummaryPattern.test(summary)
  ) {
    return {
      kind: 'rejected',
      rejection: {
        error: 'invalid_coding_objective_summary',
        evidenceRefs: ['evidence.khala_coding.objective_summary.unsafe'],
        kind: 'rejected',
        reason:
          'openagents.coding.objectiveSummary must not contain private, credential, wallet, raw prompt, or local-path material.',
        requestedPylonRef: null,
        statusCode: 400,
      },
    }
  }
  return { kind: 'summary', summary }
}

const pylonRefPattern = /^[a-z0-9][a-z0-9_.:-]{2,119}$/
const spawnRunRefPattern = /^spawn\.public\.khala_coding\.[A-Za-z0-9_.:-]{2,160}$/
const spawnWorkerRefPattern = /^worker\.public\.khala_coding\.[A-Za-z0-9_.:-]{2,160}$/
// #6354: public-safe Codex account-ref hash (`account.pylon.codex.<hex>`). The
// caller's Pylon computes this from its local account ref; the wire never
// carries a raw account ref, email, or home path.
const codexAccountRefHashPattern = /^account\.pylon\.codex\.[a-f0-9]{6,64}$/

const targetAccountRefHashFromBody = (
  body: unknown,
):
  | Readonly<{ kind: 'none' }>
  | Readonly<{ kind: 'account'; accountRefHash: string }>
  | Readonly<{ kind: 'rejected'; rejection: CodingDelegationRejection }> => {
  const coding = rawCodingFromBody(body)
  const rawValue = coding?.targetAccountRefHash ?? coding?.accountRefHash

  if (rawValue === undefined || rawValue === null || rawValue === '') {
    return { kind: 'none' }
  }
  if (typeof rawValue !== 'string') {
    return {
      kind: 'rejected',
      rejection: {
        error: 'invalid_target_account_ref',
        evidenceRefs: [
          'evidence.khala_coding.target_account_ref.invalid_type',
        ],
        kind: 'rejected',
        reason:
          'openagents.coding.targetAccountRefHash must be a public-safe Codex account-ref hash string.',
        requestedPylonRef: null,
        statusCode: 400,
      },
    }
  }
  const accountRefHash = rawValue.trim()
  if (!codexAccountRefHashPattern.test(accountRefHash)) {
    return {
      kind: 'rejected',
      rejection: {
        error: 'invalid_target_account_ref',
        evidenceRefs: [
          'evidence.khala_coding.target_account_ref.invalid_ref',
        ],
        kind: 'rejected',
        reason:
          'openagents.coding.targetAccountRefHash must match the public-safe account.pylon.codex.<hex> contract.',
        requestedPylonRef: null,
        statusCode: 400,
      },
    }
  }
  return { kind: 'account', accountRefHash }
}

const targetPylonRefFromBody = (
  body: unknown,
):
  | Readonly<{ kind: 'none' }>
  | Readonly<{ kind: 'target'; pylonRef: string }>
  | Readonly<{
      kind: 'rejected'
      rejection: CodingDelegationRejection
    }> => {
  const bodyRecord =
    body !== null && typeof body === 'object'
      ? (body as Record<string, unknown>)
      : undefined
  const openagents =
    bodyRecord?.openagents !== null &&
    typeof bodyRecord?.openagents === 'object'
      ? (bodyRecord.openagents as Record<string, unknown>)
      : undefined
  const coding = rawCodingFromBody(body)
  const rawValue =
    coding?.targetPylonRef ??
    coding?.pylonRef ??
    openagents?.targetPylonRef ??
    openagents?.pylonRef ??
    bodyRecord?.targetPylonRef ??
    bodyRecord?.pylonRef

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

const spawnRefsFromBody = (
  body: unknown,
):
  | Readonly<{ kind: 'refs'; refs: ReadonlyArray<string> }>
  | Readonly<{ kind: 'rejected'; rejection: CodingDelegationRejection }> => {
  const coding = rawCodingFromBody(body)
  const rawRunRef = coding?.spawnRunRef ?? coding?.spawnRef
  const rawWorkerRef = coding?.spawnWorkerRef ?? coding?.workerRef
  const refs: string[] = []

  if (rawRunRef !== undefined && rawRunRef !== null && rawRunRef !== '') {
    if (typeof rawRunRef !== 'string') {
      return {
        kind: 'rejected',
        rejection: {
          error: 'invalid_spawn_ref',
          evidenceRefs: ['evidence.khala_coding.spawn_ref.invalid_type'],
          kind: 'rejected',
          reason:
            'openagents.coding.spawnRunRef must be a bounded public-safe spawn ref string.',
          requestedPylonRef: null,
          statusCode: 400,
        },
      }
    }
    const ref = rawRunRef.trim()
    if (!spawnRunRefPattern.test(ref)) {
      return {
        kind: 'rejected',
        rejection: {
          error: 'invalid_spawn_ref',
          evidenceRefs: ['evidence.khala_coding.spawn_ref.invalid_ref'],
          kind: 'rejected',
          reason:
            'openagents.coding.spawnRunRef must match the Khala spawn parent ref contract.',
          requestedPylonRef: null,
          statusCode: 400,
        },
      }
    }
    refs.push(ref)
  }

  if (rawWorkerRef !== undefined && rawWorkerRef !== null && rawWorkerRef !== '') {
    if (typeof rawWorkerRef !== 'string') {
      return {
        kind: 'rejected',
        rejection: {
          error: 'invalid_spawn_ref',
          evidenceRefs: ['evidence.khala_coding.spawn_worker_ref.invalid_type'],
          kind: 'rejected',
          reason:
            'openagents.coding.spawnWorkerRef must be a bounded public-safe worker ref string.',
          requestedPylonRef: null,
          statusCode: 400,
        },
      }
    }
    const ref = rawWorkerRef.trim()
    if (!spawnWorkerRefPattern.test(ref)) {
      return {
        kind: 'rejected',
        rejection: {
          error: 'invalid_spawn_ref',
          evidenceRefs: ['evidence.khala_coding.spawn_worker_ref.invalid_ref'],
          kind: 'rejected',
          reason:
            'openagents.coding.spawnWorkerRef must match the Khala spawn worker ref contract.',
          requestedPylonRef: null,
          statusCode: 400,
        },
      }
    }
    refs.push(ref)
  }

  return { kind: 'refs', refs }
}

const codingAssignmentFromInput = (
  input: CodingDelegationInput,
  objectiveSummary: string | null,
  accountRefHash: string | null,
): Record<string, unknown> => {
  const workspace = rawWorkspaceFromBody(input.rawBody)

  return {
    codex: {
      agentKind: 'codex_sdk',
      schema: CODEX_AGENT_TASK_SCHEMA,
      ...(workspace === null
        ? { fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF }
        : {}),
      // #6354: pin the target Codex account so the dispatch gate scopes
      // capacity/leases to it and the Pylon runs on that account's home.
      ...(accountRefHash === null ? {} : { accountRefHash }),
      timeoutSeconds: 1200,
    },
    objective: {
      objectiveRef: workflowRef(input.classification),
      publicSummary:
        objectiveSummary ??
        'Run the caller-owned Khala coding workflow on a linked local Codex Pylon.',
    },
    ...(workspace === null ? {} : { workspace }),
    requiredCapabilityRefs: [CODEX_AGENT_CAPABILITY_REF],
    routing: {
      durableStreamRef: khalaCodingRequestIdRef(input.requestId),
      schema: 'openagents.khala.coding_delegation.v1',
    },
  }
}

const assignmentRequestFromInput = (
  input: CodingDelegationInput,
  objectiveSummary: string | null,
  pylonRef: string,
  spawnRefs: ReadonlyArray<string>,
  accountRefHash: string | null,
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
    khalaCodingRequestIdRef(input.requestId),
  ],
  codingAssignment: codingAssignmentFromInput(
    input,
    objectiveSummary,
    accountRefHash,
  ),
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
  taskRefs: [
    workflowRef(input.classification),
    khalaCodingRequestIdRef(input.requestId),
    ...spawnRefs,
  ],
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

const hasCodexCapability = (
  registration: PylonApiRegistrationRecord,
): boolean =>
  registration.capabilityRefs.includes(CODEX_AGENT_CAPABILITY_REF)

// #6354: when an account is requested AND the heartbeat advertises per-account
// capacity for it, availability is checked against THAT account's slots so a
// saturated account A does not hide account B's free capacity. With no requested
// account (or no per-account refs advertised), this falls back to the pooled
// codex availability, preserving legacy behavior.
const hasAvailableCodexCapacity =
  (accountKey: string | null) =>
  (registration: PylonApiRegistrationRecord): boolean => {
    if (!hasCodexCapability(registration)) {
      return false
    }
    const accountCapacity = pylonCodexAccountCapacity(registration, accountKey)
    if (accountCapacity !== null) {
      return accountCapacity.available > 0
    }
    return pylonCodingServiceCapacityProjection(registration).some(
      capacity => capacity.service === 'codex' && capacity.available > 0,
    )
  }

// #6354: name exactly which admission sub-condition failed so a refused
// caller-owned coding delegation is debuggable instead of an opaque 409. The
// gate admits a Pylon only when it is active AND heartbeat-fresh AND
// Codex-capable AND advertising codex `available>0`; this reports the failing
// sub-conditions for the targeted Pylon ("active" vs "fresh" vs "codex-capable"
// vs "available"), preferring the registration that is closest to dispatchable.
type CodexAdmissionSubCondition =
  | 'not_active'
  | 'stale_or_missing_heartbeat'
  | 'not_codex_capable'
  | 'no_available_codex_capacity'

const codexAdmissionFailures = (
  registration: PylonApiRegistrationRecord,
  nowIso: string,
  accountKey: string | null,
): ReadonlyArray<CodexAdmissionSubCondition> => {
  const failures: CodexAdmissionSubCondition[] = []
  if (registration.status !== 'active') {
    failures.push('not_active')
  }
  if (!hasFreshOnlineHeartbeat(registration, nowIso)) {
    failures.push('stale_or_missing_heartbeat')
  }
  if (!hasCodexCapability(registration)) {
    failures.push('not_codex_capable')
  }
  const accountCapacity = pylonCodexAccountCapacity(registration, accountKey)
  const hasCapacity =
    accountCapacity !== null
      ? accountCapacity.available > 0
      : pylonCodingServiceCapacityProjection(registration).some(
          capacity => capacity.service === 'codex' && capacity.available > 0,
        )
  if (!hasCapacity) {
    failures.push('no_available_codex_capacity')
  }
  return failures
}

const diagnoseCodexUnavailability = (
  registrations: ReadonlyArray<PylonApiRegistrationRecord>,
  nowIso: string,
  accountKey: string | null,
): Readonly<{
  evidenceRefs: ReadonlyArray<string>
  reason: string
}> => {
  const reasonText: Record<CodexAdmissionSubCondition, string> = {
    no_available_codex_capacity:
      'it is not advertising any available Codex capacity (heartbeat codex available=0)',
    not_active: 'it is not active',
    not_codex_capable:
      'it is not Codex-capable (the heartbeat is not publishing the local Codex capability)',
    stale_or_missing_heartbeat: 'its online heartbeat is stale or missing',
  }
  const best =
    registrations.length === 0
      ? null
      : registrations
          .map(registration => ({
            failures: codexAdmissionFailures(registration, nowIso, accountKey),
          }))
          .sort((left, right) => left.failures.length - right.failures.length)[0]
  const failures =
    best === null || best === undefined
      ? (['not_active'] as ReadonlyArray<CodexAdmissionSubCondition>)
      : best.failures
  return {
    evidenceRefs: [
      'evidence.khala_coding.target_pylon_ref.unavailable',
      ...failures.map(
        failure =>
          `evidence.khala_coding.target_pylon_ref.unavailable.${failure}`,
      ),
    ],
    reason:
      'The requested linked Pylon cannot take a Codex coding assignment because ' +
      `${failures.map(failure => reasonText[failure]).join('; ')}.`,
  }
}

const codingDelegationStoreUnavailableRejection = (
  requestedPylonRef: string | null,
  stage: CodingDelegationStoreUnavailableStage,
): CodingDelegationRejection => {
  const stageLabel = stage.replaceAll('_', ' ')
  return {
    error: 'coding_delegation_store_unavailable',
    evidenceRefs: [
      'evidence.khala_coding.dispatch.store_unavailable',
      `evidence.khala_coding.dispatch.${stage}_unavailable`,
    ],
    kind: 'rejected',
    reason:
      `The Khala coding dispatch gate could not read linked Pylon capacity right now at stage "${stageLabel}". ` +
      'This is a transient gate failure, not an account problem — retry shortly.',
    requestedPylonRef,
    statusCode: 503,
  }
}

const listRegistrationsForLinkedOwnerAgents = async (
  pylonStore: PylonApiStore,
  ownerAgentUserIds: ReadonlyArray<string>,
  targetPylonRef: string | null,
): Promise<ReadonlyArray<PylonApiRegistrationRecord>> => {
  const readTargetAndFilter = async () => {
    if (targetPylonRef === null) {
      return null
    }
    try {
      const registration = await pylonStore.readRegistration(targetPylonRef)
      return registration !== undefined &&
        ownerAgentUserIds.includes(registration.ownerAgentUserId)
        ? [registration]
        : []
    } catch (error) {
      if (error instanceof PylonApiStoreError) {
        throw error
      }
      throw new PylonApiStoreError({
        kind: 'storage_error',
        reason: 'target linked Pylon registration read failed',
      })
    }
  }

  const tryReadTargetAndFilter = async () => {
    try {
      return await readTargetAndFilter()
    } catch {
      return null
    }
  }

  const listAllAndFilter = async () => {
    try {
      return (await pylonStore.listRegistrations(200)).filter(registration =>
        ownerAgentUserIds.includes(registration.ownerAgentUserId),
      )
    } catch (error) {
      if (error instanceof PylonApiStoreError) {
        throw error
      }
      throw new PylonApiStoreError({
        kind: 'storage_error',
        reason: 'linked Pylon registration fallback read failed',
      })
    }
  }

  if (pylonStore.listRegistrationsForOwnerAgentUserIds === undefined) {
    const targetRegistration = await tryReadTargetAndFilter()
    if (targetRegistration !== null) {
      return targetRegistration
    }
    return listAllAndFilter()
  }

  try {
    return await pylonStore.listRegistrationsForOwnerAgentUserIds(
      ownerAgentUserIds,
      200,
    )
  } catch {
    const targetRegistration = await tryReadTargetAndFilter()
    if (targetRegistration !== null) {
      return targetRegistration
    }
    return listAllAndFilter()
  }
}

/**
 * Public delegation entry point. Any Pylon-store failure inside the gate is
 * caught and surfaced as a clean, diagnosable 503 rejection rather than
 * bubbling out as an unhandled defect (which the chat route turned into an
 * opaque `500 internal_server_error` — the other half of openagents #6331). A
 * gate refusal must always be an honest, reasoned status, never a bare 500.
 */
export const delegateCodingWorkflow = async (
  input: CodingDelegationInput,
): Promise<CodingDelegationResult | null> => {
  if (input.classification.workflowClass === 'none') {
    return null
  }
  const target = targetPylonRefFromBody(input.rawBody)
  try {
    return await delegateCodingWorkflowUnsafe(input, target)
  } catch (error) {
    if (error instanceof PylonApiStoreError) {
      return codingDelegationStoreUnavailableRejection(
        target.kind === 'target' ? target.pylonRef : null,
        'unknown_store_operation',
      )
    }
    throw error
  }
}

const delegateCodingWorkflowUnsafe = async (
  input: CodingDelegationInput,
  target: ReturnType<typeof targetPylonRefFromBody>,
): Promise<CodingDelegationResult | null> => {
  if (target.kind === 'rejected') {
    return target.rejection
  }

  const objectiveSummary = objectiveSummaryFromBody(input.rawBody)
  if (objectiveSummary.kind === 'rejected') {
    return objectiveSummary.rejection
  }
  const spawnRefs = spawnRefsFromBody(input.rawBody)
  if (spawnRefs.kind === 'rejected') {
    return spawnRefs.rejection
  }
  const targetAccount = targetAccountRefHashFromBody(input.rawBody)
  if (targetAccount.kind === 'rejected') {
    return targetAccount.rejection
  }
  const targetAccountRefHash =
    targetAccount.kind === 'account' ? targetAccount.accountRefHash : null
  const targetAccountKey = codexAccountCapacityKeyFromAccountRefHash(
    targetAccountRefHash,
  )

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

  const registrationsOrRejection = await (async () => {
    try {
      return await listRegistrationsForLinkedOwnerAgents(
        input.pylonStore,
        ownerAgentUserIds,
        target.kind === 'target' ? target.pylonRef : null,
      )
    } catch (error) {
      if (error instanceof PylonApiStoreError) {
        return codingDelegationStoreUnavailableRejection(
          target.kind === 'target' ? target.pylonRef : null,
          'linked_owner_registration_read',
        )
      }
      throw error
    }
  })()
  if ('kind' in registrationsOrRejection) {
    return registrationsOrRejection
  }
  const registrations = registrationsOrRejection

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
    .filter(hasAvailableCodexCapacity(targetAccountKey))
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))

  if (target.kind === 'target' && candidates.length === 0) {
    const diagnosis = diagnoseCodexUnavailability(
      authorizedRegistrations,
      input.nowIso,
      targetAccountKey,
    )
    return {
      error: 'target_pylon_unavailable',
      evidenceRefs: diagnosis.evidenceRefs,
      kind: 'rejected',
      reason: diagnosis.reason,
      requestedPylonRef: target.pylonRef,
      statusCode: 409,
    }
  }

  const blockedGateRefs: string[] = []
  for (const registration of candidates) {
    const body = assignmentRequestFromInput(
      input,
      objectiveSummary.summary,
      registration.pylonRef,
      spawnRefs.refs,
      targetAccountRefHash,
    )
    const activeAssignments = await (async () => {
      try {
        return await input.pylonStore.listAssignmentsForPylon(
          registration.pylonRef,
          100,
        )
      } catch {
        return null
      }
    })()
    if (activeAssignments === null) {
      return codingDelegationStoreUnavailableRejection(
        registration.pylonRef,
        'assignment_list_read',
      )
    }
    const gate = controlledPylonAssignmentDispatchGate({
      activeAssignments,
      assignmentRef: body.assignmentRef ?? null,
      body,
      nowIso: input.nowIso,
      registration,
    })

    if (!gate.dispatchAllowed) {
      blockedGateRefs.push(...gate.blockerRefs)
      continue
    }

    const assignmentOrRejection = await (async () => {
      try {
        return buildPylonApiAssignmentRecord({
          idempotencyKeyHash: `khala-coding:${input.requestId}`,
          makeId: input.makeId,
          nowIso: input.nowIso,
          ownerAgentUserId: registration.ownerAgentUserId,
          request: body,
        })
      } catch (error) {
        if (error instanceof PylonApiStoreError) {
          return codingDelegationStoreUnavailableRejection(
            registration.pylonRef,
            'assignment_request_validation',
          )
        }
        throw error
      }
    })()
    if ('kind' in assignmentOrRejection) {
      return assignmentOrRejection
    }
    const assignment = assignmentOrRejection
    const result = await (async () => {
      try {
        return await input.pylonStore.createAssignment(assignment)
      } catch {
        return null
      }
    })()
    if (result === null) {
      return codingDelegationStoreUnavailableRejection(
        registration.pylonRef,
        'assignment_create',
      )
    }

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

  if (target.kind === 'target' && blockedGateRefs.length > 0) {
    return {
      error: 'target_pylon_unavailable',
      evidenceRefs: [
        'evidence.khala_coding.target_pylon_ref.dispatch_gate_blocked',
        ...[...new Set(blockedGateRefs)].sort(),
      ],
      kind: 'rejected',
      reason:
        'The requested linked Pylon is available but the controlled assignment dispatch gate refused the coding lease.',
      requestedPylonRef: target.pylonRef,
      statusCode: 409,
    }
  }

  return null
}

export const codingDelegationUnavailableError = () =>
  new PylonApiStoreError({
    kind: 'not_found',
    reason:
      'No linked, heartbeat-fresh, Codex-capable Pylon capacity is available for this account.',
  })
