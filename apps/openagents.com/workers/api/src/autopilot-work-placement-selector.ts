import type { PylonResourceMode } from './pylon-resource-mode-setup'
import {
  type PylonApiRegistrationRecord,
  pylonClientVersionMeetsMinimum,
} from './pylon-api'
import { PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION } from './public-pylon-stats'
import type {
  OpenAgentsAutopilotPlacementPolicy,
  OpenAgentsAutopilotRunnerKind,
} from './autopilot-work-request'
import { pricingReasonRefsForRunnerKind } from './autopilot-work-pricing-policy'

export const onlineHeartbeatStatuses = new Set([
  'available',
  'healthy',
  'idle',
  'online',
  'ready',
])

const onlineWindowMs = 5 * 60 * 1000
export const assignmentReadyCapabilityRef = 'capability.pylon.assignment_ready'
export const localClaudeAgentCapabilityRef = 'capability.pylon.local_claude_agent'
export const localCodexCapabilityRef = 'capability.pylon.local_codex'
export const localCodingAgentCapabilityRefs = [
  localClaudeAgentCapabilityRef,
  localCodexCapabilityRef,
] as const

export type AutopilotPylonPlacementCandidateProjection = Readonly<{
  assignmentReady: boolean
  capabilityRefs: ReadonlyArray<string>
  clientVersion: string | null
  heartbeatFresh: boolean
  latestHeartbeatAt: string | null
  latestHeartbeatStatus: string | null
  latestResourceMode: typeof PylonResourceMode.Type | null
  localExecutionReady: boolean
  ownerLinked: boolean
  pylonRef: string
  reasonRefs: ReadonlyArray<string>
  selected: boolean
  status: PylonApiRegistrationRecord['status']
  versionCompatible: boolean
  walletReady: boolean
}>

export type AutopilotPlacementDecisionProjection = Readonly<{
  availabilityState: 'needs_input' | 'retry_later' | 'selected'
  callerActionRefs: ReadonlyArray<string>
  fallbackRunnerKind: OpenAgentsAutopilotRunnerKind | null
  reasonRefs: ReadonlyArray<string>
  refusalReasonRefs: ReadonlyArray<string>
  retryAfterSeconds: number | null
  selectedPylonRef: string | null
  selectedRunnerKind: OpenAgentsAutopilotRunnerKind | null
  source: 'fallback' | 'none_available' | 'requester_pylon'
  pylonCandidates: ReadonlyArray<AutopilotPylonPlacementCandidateProjection>
}>

export const pylonHeartbeatFresh = (
  latestHeartbeatAt: string | null,
  nowIso: string,
): boolean => {
  if (latestHeartbeatAt === null) {
    return false
  }

  const heartbeatTime = Date.parse(latestHeartbeatAt)
  const nowTime = Date.parse(nowIso)

  if (!Number.isFinite(heartbeatTime) || !Number.isFinite(nowTime)) {
    return false
  }

  const ageMs = nowTime - heartbeatTime

  return ageMs >= 0 && ageMs <= onlineWindowMs
}

const runnerKindAllowed = (
  policy: OpenAgentsAutopilotPlacementPolicy,
  runnerKind: OpenAgentsAutopilotRunnerKind,
): boolean =>
  policy.allowedRunnerKinds.includes(runnerKind) &&
  !policy.disallowedRunnerKinds.includes(runnerKind)

const firstFallbackRunnerKind = (
  policy: OpenAgentsAutopilotPlacementPolicy,
): OpenAgentsAutopilotRunnerKind | null =>
  policy.preferredRunnerKinds.find(
    runnerKind =>
      runnerKind !== 'requester_pylon' &&
      runnerKind !== 'pylon_network' &&
      runnerKindAllowed(policy, runnerKind),
  ) ??
  policy.allowedRunnerKinds.find(
    runnerKind =>
      runnerKind !== 'requester_pylon' &&
      runnerKind !== 'pylon_network' &&
      runnerKindAllowed(policy, runnerKind),
  ) ??
  null

const candidateProjection = (
  candidate: PylonApiRegistrationRecord,
  ownerAgentUserId: string,
  nowIso: string,
): AutopilotPylonPlacementCandidateProjection => {
  const ownerLinked = candidate.ownerAgentUserId === ownerAgentUserId
  const heartbeatOnline = onlineHeartbeatStatuses.has(
    (candidate.latestHeartbeatStatus ?? '').trim().toLowerCase(),
  )
  const fresh = pylonHeartbeatFresh(candidate.latestHeartbeatAt, nowIso)
  const versionCompatible = pylonClientVersionMeetsMinimum(
    candidate.clientVersion,
    PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION,
  )
  const assignmentReady = candidate.capabilityRefs.includes(
    assignmentReadyCapabilityRef,
  )
  const localExecutionReady = localCodingAgentCapabilityRefs.some(ref =>
    candidate.capabilityRefs.includes(ref)
  )
  const reasonRefs = [
    ownerLinked
      ? 'placement.pylon.owner_linked'
      : 'placement.pylon.owner_mismatch',
    candidate.status === 'active'
      ? 'placement.pylon.active'
      : 'placement.pylon.not_active',
    candidate.walletReady
      ? 'placement.pylon.wallet_ready'
      : 'placement.pylon.wallet_not_ready',
    heartbeatOnline && fresh
      ? 'placement.pylon.heartbeat_online'
      : 'placement.pylon.heartbeat_unavailable',
    versionCompatible
      ? 'placement.pylon.version_compatible'
      : 'placement.pylon.version_incompatible',
    assignmentReady
      ? 'placement.pylon.assignment_ready'
      : 'placement.pylon.assignment_not_ready',
    localExecutionReady
      ? 'placement.pylon.local_execution_ready'
      : 'placement.pylon.local_execution_missing',
  ]

  return {
    assignmentReady,
    capabilityRefs: candidate.capabilityRefs,
    clientVersion: candidate.clientVersion,
    heartbeatFresh: fresh,
    latestHeartbeatAt: candidate.latestHeartbeatAt,
    latestHeartbeatStatus: candidate.latestHeartbeatStatus,
    latestResourceMode: candidate.latestResourceMode,
    localExecutionReady,
    ownerLinked,
    pylonRef: candidate.pylonRef,
    reasonRefs,
    selected: false,
    status: candidate.status,
    versionCompatible,
    walletReady: candidate.walletReady,
  }
}

const pylonCandidateEligible = (
  candidate: AutopilotPylonPlacementCandidateProjection,
): boolean =>
  candidate.ownerLinked &&
  candidate.status === 'active' &&
  candidate.walletReady &&
  candidate.heartbeatFresh &&
  candidate.versionCompatible &&
  candidate.assignmentReady &&
  candidate.localExecutionReady

const pylonCandidateCanRetry = (
  candidate: AutopilotPylonPlacementCandidateProjection,
): boolean =>
  candidate.ownerLinked &&
  candidate.status === 'active' &&
  candidate.walletReady &&
  candidate.assignmentReady &&
  candidate.localExecutionReady &&
  candidate.versionCompatible &&
  !candidate.heartbeatFresh

const noneAvailableCallerActionRefs = (
  policy: OpenAgentsAutopilotPlacementPolicy,
  candidates: ReadonlyArray<AutopilotPylonPlacementCandidateProjection>,
): ReadonlyArray<string> => {
  const refs = new Set<string>()

  if (
    policy.localOnlyAllowed ||
    runnerKindAllowed(policy, 'requester_pylon') ||
    runnerKindAllowed(policy, 'pylon_network')
  ) {
    refs.add('caller.add_or_restart_pylon')
  }

  if (candidates.some(pylonCandidateCanRetry)) {
    refs.add('caller.retry_after_pylon_heartbeat')
  }

  if (
    policy.localOnlyAllowed ||
    policy.allowedRunnerKinds.every(
      runnerKind =>
        runnerKind === 'requester_pylon' ||
        runnerKind === 'pylon_network',
    )
  ) {
    refs.add('caller.relax_privacy_or_runner_policy')
  }

  if (refs.size === 0) {
    refs.add('caller.contact_operator_for_runner_capacity')
  }

  return [...refs]
}

const noneAvailableRefusalReasonRefs = (
  policy: OpenAgentsAutopilotPlacementPolicy,
  candidates: ReadonlyArray<AutopilotPylonPlacementCandidateProjection>,
): ReadonlyArray<string> => {
  const refs = new Set<string>(['placement.blocked.no_compatible_runner'])

  if (policy.localOnlyAllowed) {
    refs.add('placement.blocked.local_only_without_eligible_pylon')
  }

  if (candidates.length === 0) {
    refs.add('placement.blocked.no_pylon_candidates')
  }

  if (candidates.some(candidate => candidate.ownerLinked)) {
    refs.add('placement.blocked.owner_pylon_not_eligible')
  }

  if (candidates.some(pylonCandidateCanRetry)) {
    refs.add('placement.retry.pylon_heartbeat_expected')
  }

  return [...refs]
}

export const selectAutopilotPlacement = (
  input: Readonly<{
    nowIso: string
    ownerAgentUserId: string
    placementPolicy: OpenAgentsAutopilotPlacementPolicy
    pylonRegistrations: ReadonlyArray<PylonApiRegistrationRecord>
  }>,
): AutopilotPlacementDecisionProjection => {
  const candidates = input.pylonRegistrations.map(candidate =>
    candidateProjection(candidate, input.ownerAgentUserId, input.nowIso)
  )
  const requesterPylonAllowed =
    runnerKindAllowed(input.placementPolicy, 'requester_pylon') ||
    runnerKindAllowed(input.placementPolicy, 'pylon_network')
  const selectedPylon = requesterPylonAllowed
    ? candidates.find(pylonCandidateEligible) ?? null
    : null

  if (selectedPylon !== null) {
    return {
      availabilityState: 'selected',
      callerActionRefs: [],
      fallbackRunnerKind: firstFallbackRunnerKind(input.placementPolicy),
      pylonCandidates: candidates.map(candidate => ({
        ...candidate,
        selected: candidate.pylonRef === selectedPylon.pylonRef,
      })),
      reasonRefs: [
        'placement.selected.requester_pylon',
        'placement.pylon.preferred_before_fallback',
        ...pricingReasonRefsForRunnerKind('requester_pylon'),
      ],
      refusalReasonRefs: [],
      retryAfterSeconds: null,
      selectedPylonRef: selectedPylon.pylonRef,
      selectedRunnerKind: 'requester_pylon',
      source: 'requester_pylon',
    }
  }

  const fallbackRunnerKind = input.placementPolicy.localOnlyAllowed
    ? null
    : firstFallbackRunnerKind(input.placementPolicy)
  const refusalReasonRefs = fallbackRunnerKind === null
    ? noneAvailableRefusalReasonRefs(input.placementPolicy, candidates)
    : []
  const callerActionRefs = fallbackRunnerKind === null
    ? noneAvailableCallerActionRefs(input.placementPolicy, candidates)
    : []
  const availabilityState = fallbackRunnerKind !== null
    ? 'selected'
    : candidates.some(pylonCandidateCanRetry)
      ? 'retry_later'
      : 'needs_input'

  return {
    availabilityState,
    callerActionRefs,
    fallbackRunnerKind,
    pylonCandidates: candidates,
    reasonRefs: fallbackRunnerKind === null
      ? refusalReasonRefs
      : [
          'placement.selected.fallback',
          `placement.fallback.${fallbackRunnerKind}`,
          ...pricingReasonRefsForRunnerKind(fallbackRunnerKind),
        ],
    refusalReasonRefs,
    retryAfterSeconds: availabilityState === 'retry_later' ? 300 : null,
    selectedPylonRef: null,
    selectedRunnerKind: fallbackRunnerKind,
    source: fallbackRunnerKind === null ? 'none_available' : 'fallback',
  }
}
