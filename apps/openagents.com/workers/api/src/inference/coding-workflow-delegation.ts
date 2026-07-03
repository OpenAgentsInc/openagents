import type { LinkedAgentOwnerRecord } from '../agent-registration'
import {
  ARTANIS_OWNER_SELF_AUTHORITY_SCOPE,
  artanisAuthorityScopeAllowsOwnerLinkedCapacity,
  artanisAuthorityScopeEvidenceRef,
  artanisAuthorityScopePublicRef,
  isArtanisAuthorityScope,
  type ArtanisAuthorityScope,
} from '../artanis-authority-scope'
import {
  type PylonApiAssignmentRecord,
  type PylonApiCreateAssignmentRequest,
  type PylonApiRegistrationRecord,
  type PylonApiStore,
  type PylonCodingServiceCapacityProjection,
  PylonApiStoreError,
  buildPylonApiAssignmentRecord,
  codexAccountCapacityKeyFromAccountRefHash,
  pylonCodingServiceAccountCapacity,
  pylonCodingServiceCapacityProjection,
} from '../pylon-api'
import {
  controlledPylonAssignmentDispatchGate,
  sweepStalePylonAssignmentLeases,
} from '../pylon-api-routes'
import type { CodingWorkflowClassification } from './coding-workflow-classifier'

const CODEX_AGENT_CAPABILITY_REF = 'capability.pylon.local_codex'
const CODEX_AGENT_TASK_SCHEMA = 'openagents.pylon.codex_agent_task.v0.3'
const CODEX_AGENT_SUM_REPAIR_FIXTURE_REF =
  'fixture.public.pylon.codex_agent.sum_repair.v1'

const CLAUDE_AGENT_CAPABILITY_REF = 'capability.pylon.local_claude_agent'
const CLAUDE_AGENT_TASK_SCHEMA = 'openagents.pylon.claude_agent_task.v0.3'
const CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF =
  'fixture.public.pylon.claude_agent.sum_repair.v1'
const CODEX_AGENT_ASSIGNMENT_TIMEOUT_SECONDS = 2400
const CLAUDE_AGENT_ASSIGNMENT_TIMEOUT_SECONDS = 1200

// #6388: a coding workflow class resolves to one local-agent execution profile.
// The Codex lane (`cloud_coding_session`, `codex_agent_task`) and the Claude
// lane (`claude_agent_task`) share the entire request/dispatch pipeline; only
// the advertised capability, the assignment job kind, the bounded fixture, the
// capacity service projection key, and the on-assignment coding sub-object differ.
// Keying the gate per-capability (instead of hardcoding Codex) is the wiring that
// lets a caller route Claude work to their own linked Claude-capable Pylon.
type CodingAgentProfile = Readonly<{
  // The key under `codingAssignment` the Pylon executor reads (claudeAgent | codex).
  agentAssignmentKey: 'claudeAgent' | 'codex'
  // The agentKind the Pylon executor validates on that sub-object.
  agentKind: 'claude_agent_sdk' | 'codex_sdk'
  // #6421: the Pylon account provider whose per-account hash this lane accepts
  // (`account.pylon.codex.<hex>` for Codex, `account.pylon.claude_agent.<hex>`
  // for Claude). The wire never carries a raw ref, email, or home path.
  accountProvider: 'claude_agent' | 'codex'
  // Heartbeat-advertised capability ref that must be present to dispatch.
  capabilityRef: string
  // Counted capacity service projected from the heartbeat capacity refs.
  capacityService: PylonCodingServiceCapacityProjection['service']
  // Bounded public fixture used when the request carries no pinned workspace.
  fixtureRef: string
  // Assignment job kind persisted on the Pylon assignment row.
  jobKind: 'claude_agent_task' | 'codex_agent_task'
  // Lowercase agent label used in diagnosable refusals ("Codex" / "Claude").
  label: string
  // Public-safe objective fallback summary.
  objectiveFallback: string
  // Codex-first vs Claude-first selection policy ref.
  selectionPolicyRef: string
  // Public assignment-task schema ref for the agent sub-object.
  taskSchema: string
  // Assignment wall-clock budget accepted by the local executor for this lane.
  timeoutSeconds: number
}>

const CODEX_AGENT_PROFILE: CodingAgentProfile = {
  accountProvider: 'codex',
  agentAssignmentKey: 'codex',
  agentKind: 'codex_sdk',
  capabilityRef: CODEX_AGENT_CAPABILITY_REF,
  capacityService: 'codex',
  fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
  jobKind: 'codex_agent_task',
  label: 'Codex',
  objectiveFallback:
    'Run the caller-owned Khala coding workflow on a linked local Codex Pylon.',
  selectionPolicyRef: 'selection.public.khala_coding.codex_first',
  taskSchema: CODEX_AGENT_TASK_SCHEMA,
  timeoutSeconds: CODEX_AGENT_ASSIGNMENT_TIMEOUT_SECONDS,
}

const CLAUDE_AGENT_PROFILE: CodingAgentProfile = {
  accountProvider: 'claude_agent',
  agentAssignmentKey: 'claudeAgent',
  agentKind: 'claude_agent_sdk',
  capabilityRef: CLAUDE_AGENT_CAPABILITY_REF,
  capacityService: 'claude',
  fixtureRef: CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF,
  jobKind: 'claude_agent_task',
  label: 'Claude',
  objectiveFallback:
    'Run the caller-owned Khala coding workflow on a linked local Claude Agent Pylon.',
  selectionPolicyRef: 'selection.public.khala_coding.claude_first',
  taskSchema: CLAUDE_AGENT_TASK_SCHEMA,
  timeoutSeconds: CLAUDE_AGENT_ASSIGNMENT_TIMEOUT_SECONDS,
}

const codingAgentProfileFor = (
  classification: CodingWorkflowClassification,
): CodingAgentProfile =>
  classification.workflowClass === 'claude_agent_task'
    ? CLAUDE_AGENT_PROFILE
    : CODEX_AGENT_PROFILE

export type CodingDelegationInput = Readonly<{
  authorityScope?: ArtanisAuthorityScope | undefined
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
    | 'authority_scope_capacity_unavailable'
    | 'invalid_authority_scope'
    | 'invalid_assignment_timeout'
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

const authorityScopeFromInput = (
  input: CodingDelegationInput,
  requestedPylonRef: string | null,
):
  | Readonly<{ kind: 'scope'; authorityScope: ArtanisAuthorityScope }>
  | Readonly<{ kind: 'rejected'; rejection: CodingDelegationRejection }> => {
  if (input.authorityScope !== undefined) {
    return { authorityScope: input.authorityScope, kind: 'scope' }
  }

  const coding = rawCodingFromBody(input.rawBody)
  const openagents =
    input.rawBody !== null && typeof input.rawBody === 'object'
      ? (input.rawBody as Record<string, unknown>).openagents
      : undefined
  const topLevelScope =
    openagents !== null && typeof openagents === 'object'
      ? (openagents as Record<string, unknown>).authorityScope
      : undefined
  const raw =
    coding?.authorityScope ?? topLevelScope

  if (raw === undefined || raw === null || raw === '') {
    return {
      authorityScope: ARTANIS_OWNER_SELF_AUTHORITY_SCOPE,
      kind: 'scope',
    }
  }

  if (!isArtanisAuthorityScope(raw)) {
    return {
      kind: 'rejected',
      rejection: {
        error: 'invalid_authority_scope',
        evidenceRefs: ['evidence.khala_coding.authority_scope.invalid'],
        kind: 'rejected',
        reason:
          'The coding delegation authorityScope must be owner_self, shared_fleet, or owner_operator.',
        requestedPylonRef,
        statusCode: 400,
      },
    }
  }

  return { authorityScope: raw, kind: 'scope' }
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
  /([A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}|\/Users\/|\/home\/|\.env|access[_-]?token|auth\.json|bearer\s+(?:oa_agent_|sk-|gho_|ghp_|github_pat_|[A-Za-z0-9._~+/=-]{16,})|cookie|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github[_-]?pat_[A-Za-z0-9_]+|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(key|repo)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(auth|command|content|payload|prompt|provider|runner|source|trace)|secret|seed[_-]?phrase|sk-[a-z0-9]|ssh:\/\/|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

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

const timeoutSecondsFromBody = (
  body: unknown,
  profile: CodingAgentProfile,
):
  | Readonly<{ kind: 'timeout'; timeoutSeconds: number }>
  | Readonly<{ kind: 'rejected'; rejection: CodingDelegationRejection }> => {
  const rawValue = rawCodingFromBody(body)?.timeoutSeconds

  if (rawValue === undefined || rawValue === null || rawValue === '') {
    return { kind: 'timeout', timeoutSeconds: profile.timeoutSeconds }
  }

  if (
    typeof rawValue !== 'number' ||
    !Number.isInteger(rawValue) ||
    rawValue <= 0
  ) {
    return {
      kind: 'rejected',
      rejection: {
        error: 'invalid_assignment_timeout',
        evidenceRefs: ['evidence.khala_coding.assignment_timeout.invalid'],
        kind: 'rejected',
        reason:
          'openagents.coding.timeoutSeconds must be a positive integer.',
        requestedPylonRef: null,
        statusCode: 400,
      },
    }
  }

  return {
    kind: 'timeout',
    timeoutSeconds: Math.min(rawValue, profile.timeoutSeconds),
  }
}

const pylonRefPattern = /^[a-z0-9][a-z0-9_.:-]{2,119}$/
const spawnRunRefPattern = /^spawn\.public\.khala_coding\.[A-Za-z0-9_.:-]{2,160}$/
const spawnWorkerRefPattern = /^worker\.public\.khala_coding\.[A-Za-z0-9_.:-]{2,160}$/
// #6354/#6421: public-safe per-account hash (`account.pylon.<provider>.<hex>`).
// The caller's Pylon computes this from its local account ref; the wire never
// carries a raw account ref, email, or home path. The accepted provider is the
// lane's own (`codex` for Codex, `claude_agent` for Claude) so a Codex hash can
// never be pinned to a Claude assignment and vice versa.
const accountRefHashPatternFor = (
  provider: CodingAgentProfile['accountProvider'],
): RegExp => new RegExp(`^account\\.pylon\\.${provider}\\.[a-f0-9]{6,64}$`)

const targetAccountRefHashFromBody = (
  body: unknown,
  profile: CodingAgentProfile,
):
  | Readonly<{ kind: 'none' }>
  | Readonly<{ kind: 'account'; accountRefHash: string }>
  | Readonly<{ kind: 'rejected'; rejection: CodingDelegationRejection }> => {
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
    coding?.targetAccountRefHash ??
    coding?.accountRefHash ??
    openagents?.targetAccountRefHash ??
    openagents?.accountRefHash ??
    bodyRecord?.targetAccountRefHash ??
    bodyRecord?.accountRefHash

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
          'targetAccountRefHash must be a public-safe per-account hash string.',
        requestedPylonRef: null,
        statusCode: 400,
      },
    }
  }
  const accountRefHash = rawValue.trim()
  if (!accountRefHashPatternFor(profile.accountProvider).test(accountRefHash)) {
    return {
      kind: 'rejected',
      rejection: {
        error: 'invalid_target_account_ref',
        evidenceRefs: [
          'evidence.khala_coding.target_account_ref.invalid_ref',
        ],
        kind: 'rejected',
        reason:
          `targetAccountRefHash must match the public-safe account.pylon.${profile.accountProvider}.<hex> contract.`,
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
  profile: CodingAgentProfile,
  accountRefHash: string | null,
  timeoutSeconds: number,
): Record<string, unknown> => {
  const workspace = rawWorkspaceFromBody(input.rawBody)

  return {
    [profile.agentAssignmentKey]: {
      agentKind: profile.agentKind,
      schema: profile.taskSchema,
      ...(workspace === null ? { fixtureRef: profile.fixtureRef } : {}),
      // #6354/#6421: pin the target account so the dispatch gate scopes
      // capacity/leases to it and the Pylon runs on that account's home. Both
      // the Codex and Claude lanes now carry per-account capacity; null when no
      // account was pinned.
      ...(accountRefHash === null ? {} : { accountRefHash }),
      timeoutSeconds,
    },
    objective: {
      objectiveRef: workflowRef(input.classification),
      publicSummary: objectiveSummary ?? profile.objectiveFallback,
    },
    ...(workspace === null ? {} : { workspace }),
    requiredCapabilityRefs: [profile.capabilityRef],
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
  profile: CodingAgentProfile,
  accountRefHash: string | null,
  timeoutSeconds: number,
  authorityScope: ArtanisAuthorityScope,
): PylonApiCreateAssignmentRequest => ({
  acceptanceCriteriaRefs: [
    'acceptance.public.khala_coding.owner_requested',
    artanisAuthorityScopePublicRef(authorityScope),
    ...input.classification.evidenceRefs,
  ],
  assignmentRef: `assignment.public.khala_coding.${input.makeId()}`,
  campaignPaused: false,
  campaignPolicyRefs: [
    'policy.public.khala_coding.own_capacity_only',
    artanisAuthorityScopePublicRef(authorityScope),
  ],
  campaignRef: 'campaign.public.khala_coding.own_capacity',
  closeoutPathRefs: [
    'closeout.public.khala_coding.durable_stream',
    khalaCodingRequestIdRef(input.requestId),
  ],
  codingAssignment: codingAssignmentFromInput(
    input,
    objectiveSummary,
    profile,
    accountRefHash,
    timeoutSeconds,
  ),
  forumAutoPublishAllowed: false,
  idempotencyRefs: ['idempotency.public.khala_coding.request'],
  jobKind: profile.jobKind,
  leaseSeconds: 3600,
  noDuplicateAssignmentRefs: ['dedupe.public.pylon_assignment.active_lease'],
  noForumAutoPublishRefs: ['policy.public.no_forum_auto_publish'],
  operatorPauseRefs: ['pause.public.khala_coding.kill_switch_default_off'],
  paymentMode: 'unpaid_smoke',
  pylonRef,
  requiredCapabilityRefs: [profile.capabilityRef],
  resultExpectationRefs: ['result.public.khala_coding.worker_closeout'],
  rollbackRefs: ['rollback.public.khala_coding.assignment_cancel'],
  selectionPolicyRefs: [profile.selectionPolicyRef],
  spendCapRefs: [],
  taskRefs: [
    workflowRef(input.classification),
    khalaCodingRequestIdRef(input.requestId),
    artanisAuthorityScopePublicRef(authorityScope),
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

const hasAgentCapability = (
  registration: PylonApiRegistrationRecord,
  profile: CodingAgentProfile,
): boolean => registration.capabilityRefs.includes(profile.capabilityRef)

// #6354/#6388/#6421: capacity admission, per-agent (Codex|Claude) and
// per-account (both lanes). When an account is requested AND the heartbeat
// advertises per-account capacity for it, availability is checked against THAT
// account's slots so a saturated account A does not hide account B's free
// capacity. Otherwise (no requested account or no per-account refs advertised)
// this falls back to the pooled per-service availability.
const hasPooledServiceCapacity = (
  registration: PylonApiRegistrationRecord,
  profile: CodingAgentProfile,
): boolean =>
  pylonCodingServiceCapacityProjection(registration).some(
    capacity =>
      capacity.service === profile.capacityService && capacity.available > 0,
  )

const hasAvailableAgentCapacity = (
  registration: PylonApiRegistrationRecord,
  profile: CodingAgentProfile,
  accountKey: string | null,
): boolean => {
  if (!hasAgentCapability(registration, profile)) {
    return false
  }
  if (accountKey !== null) {
    const accountCapacity = pylonCodingServiceAccountCapacity(
      registration,
      profile.capacityService,
      accountKey,
    )
    if (accountCapacity !== null) {
      return accountCapacity.available > 0
    }
  }
  return hasPooledServiceCapacity(registration, profile)
}

const accountRefHashForCapacityKey = (
  profile: CodingAgentProfile,
  accountKey: string,
): string => `account.pylon.${profile.accountProvider}.${accountKey}`

const accountRefHashesAdvertisedByRegistration = (
  registration: PylonApiRegistrationRecord,
  profile: CodingAgentProfile,
): ReadonlyArray<string> => {
  const capacity = pylonCodingServiceCapacityProjection(registration).find(
    item => item.service === profile.capacityService,
  )
  if (capacity === undefined || capacity.accounts.length === 0) {
    return []
  }
  return capacity.accounts
    .filter(account => account.available > 0 || account.ready > 0)
    .sort((left, right) => {
      const leftPressure = left.busy + left.queued
      const rightPressure = right.busy + right.queued
      return (
        leftPressure - rightPressure ||
        left.accountKey.localeCompare(right.accountKey)
      )
    })
    .map(account => accountRefHashForCapacityKey(profile, account.accountKey))
}

const accountRefHashSelectionCandidates = (
  registration: PylonApiRegistrationRecord,
  profile: CodingAgentProfile,
  requestedAccountRefHash: string | null,
): ReadonlyArray<string | null> =>
  requestedAccountRefHash === null
    ? accountRefHashesAdvertisedByRegistration(registration, profile)
    : [requestedAccountRefHash]

// #6354/#6388: name exactly which admission sub-condition failed so a refused
// caller-owned coding delegation is debuggable instead of an opaque 409. The
// gate admits a Pylon only when it is active AND heartbeat-fresh AND
// agent-capable (Codex or Claude) AND advertising that service's `available>0`;
// this reports the failing sub-conditions for the targeted Pylon ("active" vs
// "fresh" vs "agent-capable" vs "available"), preferring the registration that
// is closest to dispatchable. Sub-condition names are kept service-neutral so
// the same diagnosis covers both the Codex and Claude lanes.
type AgentAdmissionSubCondition =
  | 'not_active'
  | 'stale_or_missing_heartbeat'
  | 'not_agent_capable'
  | 'no_available_agent_capacity'

const agentAdmissionFailures = (
  registration: PylonApiRegistrationRecord,
  nowIso: string,
  profile: CodingAgentProfile,
  accountKey: string | null,
): ReadonlyArray<AgentAdmissionSubCondition> => {
  const failures: AgentAdmissionSubCondition[] = []
  if (registration.status !== 'active') {
    failures.push('not_active')
  }
  if (!hasFreshOnlineHeartbeat(registration, nowIso)) {
    failures.push('stale_or_missing_heartbeat')
  }
  if (!hasAgentCapability(registration, profile)) {
    failures.push('not_agent_capable')
  }
  const accountCapacity =
    accountKey !== null
      ? pylonCodingServiceAccountCapacity(
          registration,
          profile.capacityService,
          accountKey,
        )
      : null
  const hasCapacity =
    accountCapacity !== null
      ? accountCapacity.available > 0
      : hasPooledServiceCapacity(registration, profile)
  if (!hasCapacity) {
    failures.push('no_available_agent_capacity')
  }
  return failures
}

const diagnoseAgentUnavailability = (
  registrations: ReadonlyArray<PylonApiRegistrationRecord>,
  nowIso: string,
  profile: CodingAgentProfile,
  accountKey: string | null,
): Readonly<{
  evidenceRefs: ReadonlyArray<string>
  reason: string
}> => {
  const reasonText: Record<AgentAdmissionSubCondition, string> = {
    no_available_agent_capacity: `it is not advertising any available ${profile.label} capacity (heartbeat ${profile.capacityService} available=0)`,
    not_active: 'it is not active',
    not_agent_capable: `it is not ${profile.label}-capable (the heartbeat is not publishing the local ${profile.label} capability)`,
    stale_or_missing_heartbeat: 'its online heartbeat is stale or missing',
  }
  const best =
    registrations.length === 0
      ? null
      : registrations
          .map(registration => ({
            failures: agentAdmissionFailures(
              registration,
              nowIso,
              profile,
              accountKey,
            ),
          }))
          .sort((left, right) => left.failures.length - right.failures.length)[0]
  const failures =
    best === null || best === undefined
      ? (['not_active'] as ReadonlyArray<AgentAdmissionSubCondition>)
      : best.failures
  // Emit service-specific evidence ref suffixes (`not_codex_capable` /
  // `no_available_codex_capacity` for the Codex lane, the `claude` equivalents
  // for the Claude lane) so each lane's refusal is grep-able on its own and the
  // pre-existing Codex contract is preserved verbatim.
  const failureRefSuffix = (failure: AgentAdmissionSubCondition): string => {
    switch (failure) {
      case 'not_agent_capable':
        return `not_${profile.capacityService}_capable`
      case 'no_available_agent_capacity':
        return `no_available_${profile.capacityService}_capacity`
      default:
        return failure
    }
  }
  return {
    evidenceRefs: [
      'evidence.khala_coding.target_pylon_ref.unavailable',
      ...failures.map(
        failure =>
          `evidence.khala_coding.target_pylon_ref.unavailable.${failureRefSuffix(failure)}`,
      ),
    ],
    reason:
      `The requested linked Pylon cannot take a ${profile.label} coding assignment because ` +
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
  const parsedIsoMs = (iso: string | null): number => {
    const value = iso === null ? Number.NaN : Date.parse(iso)
    return Number.isFinite(value) ? value : Number.NEGATIVE_INFINITY
  }

  const registrationFreshnessMs = (
    registration: PylonApiRegistrationRecord,
  ): number =>
    Math.max(
      parsedIsoMs(registration.latestHeartbeatAt),
      parsedIsoMs(registration.updatedAt),
      parsedIsoMs(registration.createdAt),
    )

  const freshestRegistrationsByPylonRef = (
    registrations: ReadonlyArray<PylonApiRegistrationRecord>,
  ): ReadonlyArray<PylonApiRegistrationRecord> =>
    [
      ...registrations
        .reduce((byPylonRef, registration) => {
          const existing = byPylonRef.get(registration.pylonRef)
          if (
            existing === undefined ||
            registrationFreshnessMs(registration) >
              registrationFreshnessMs(existing) ||
            (registrationFreshnessMs(registration) ===
              registrationFreshnessMs(existing) &&
              registration.updatedAt > existing.updatedAt)
          ) {
            return new Map(byPylonRef).set(registration.pylonRef, registration)
          }
          return byPylonRef
        }, new Map<string, PylonApiRegistrationRecord>())
        .values(),
    ]

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

  const withTargetRegistrationRefresh = async (
    registrations: ReadonlyArray<PylonApiRegistrationRecord>,
  ): Promise<ReadonlyArray<PylonApiRegistrationRecord>> => {
    const targetRegistration = await tryReadTargetAndFilter()
    if (targetRegistration === null || targetRegistration.length === 0) {
      return registrations
    }
    return freshestRegistrationsByPylonRef([
      ...registrations,
      ...targetRegistration,
    ])
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
    return await withTargetRegistrationRefresh(
      await pylonStore.listRegistrationsForOwnerAgentUserIds(
        ownerAgentUserIds,
        200,
      ),
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

  const profile = codingAgentProfileFor(input.classification)
  const objectiveSummary = objectiveSummaryFromBody(input.rawBody)
  if (objectiveSummary.kind === 'rejected') {
    return objectiveSummary.rejection
  }
  const timeout = timeoutSecondsFromBody(input.rawBody, profile)
  if (timeout.kind === 'rejected') {
    return timeout.rejection
  }
  const spawnRefs = spawnRefsFromBody(input.rawBody)
  if (spawnRefs.kind === 'rejected') {
    return spawnRefs.rejection
  }
  const targetAccount = targetAccountRefHashFromBody(input.rawBody, profile)
  if (targetAccount.kind === 'rejected') {
    return targetAccount.rejection
  }
  const targetAccountRefHash =
    targetAccount.kind === 'account' ? targetAccount.accountRefHash : null
  const authorityScope = authorityScopeFromInput(
    input,
    target.kind === 'target' ? target.pylonRef : null,
  )
  if (authorityScope.kind === 'rejected') {
    return authorityScope.rejection
  }
  if (
    !artanisAuthorityScopeAllowsOwnerLinkedCapacity(authorityScope.authorityScope)
  ) {
    return {
      error: 'authority_scope_capacity_unavailable',
      evidenceRefs: [
        artanisAuthorityScopeEvidenceRef(authorityScope.authorityScope),
        'evidence.khala_coding.authority_scope.owner_linked_capacity_not_allowed',
      ],
      kind: 'rejected',
      reason:
        `The ${authorityScope.authorityScope} Artanis authority scope is not wired to caller-owned linked Pylon capacity.`,
      requestedPylonRef: target.kind === 'target' ? target.pylonRef : null,
      statusCode: 403,
    }
  }
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
    .filter(registration =>
      hasAvailableAgentCapacity(registration, profile, targetAccountKey),
    )
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))

  if (target.kind === 'target' && candidates.length === 0) {
    const diagnosis = diagnoseAgentUnavailability(
      authorizedRegistrations,
      input.nowIso,
      profile,
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
    const activeAssignments = await (async () => {
      try {
        await sweepStalePylonAssignmentLeases({
          nowIso: input.nowIso,
          pylonRef: registration.pylonRef,
          store: input.pylonStore,
        })
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
    const accountCandidates = accountRefHashSelectionCandidates(
      registration,
      profile,
      targetAccountRefHash,
    )
    const effectiveAccountCandidates =
      accountCandidates.length === 0 ? [targetAccountRefHash] : accountCandidates

    const activeQuarantine =
      input.pylonStore.readActiveQuarantineForPylon === undefined
        ? undefined
        : await input.pylonStore.readActiveQuarantineForPylon(
            registration.pylonRef,
            input.nowIso,
          )
    let admittedBody: PylonApiCreateAssignmentRequest | null = null
    for (const accountRefHash of effectiveAccountCandidates) {
      const body = assignmentRequestFromInput(
        input,
        objectiveSummary.summary,
        registration.pylonRef,
        spawnRefs.refs,
        profile,
        accountRefHash,
        timeout.timeoutSeconds,
        authorityScope.authorityScope,
      )
      const gate = controlledPylonAssignmentDispatchGate({
        activeAssignments,
        assignmentRef: body.assignmentRef ?? null,
        body,
        nowIso: input.nowIso,
        quarantine: activeQuarantine,
        registration,
      })

      if (!gate.dispatchAllowed) {
        blockedGateRefs.push(...gate.blockerRefs)
        continue
      }

      admittedBody = body
      break
    }

    if (admittedBody === null) {
      continue
    }

    const assignmentOrRejection = await (async () => {
      try {
        return buildPylonApiAssignmentRecord({
          idempotencyKeyHash: `khala-coding:${input.requestId}`,
          makeId: input.makeId,
          nowIso: input.nowIso,
          ownerAgentUserId: registration.ownerAgentUserId,
          request: admittedBody,
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
        artanisAuthorityScopeEvidenceRef(authorityScope.authorityScope),
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
