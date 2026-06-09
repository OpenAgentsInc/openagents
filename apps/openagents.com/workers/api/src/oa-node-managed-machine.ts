import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import {
  OmniProjectionAudience,
} from './omni-data-classification'
import {
  OpenAgentsRunnerBackendKind,
  OpenAgentsRunnerWorkloadTrust,
} from './runner-backends'

export const OpenAgentsOaNodeMachineKind = S.Literals([
  'desktop',
  'gcloud_vm',
  'pylon_candidate',
  'shc_vm',
])
export type OpenAgentsOaNodeMachineKind =
  typeof OpenAgentsOaNodeMachineKind.Type

export const OpenAgentsOaNodeTrustTier = S.Literals([
  'blocked',
  'quarantined',
  'reviewed',
  'unverified',
  'verified',
])
export type OpenAgentsOaNodeTrustTier =
  typeof OpenAgentsOaNodeTrustTier.Type

export const OpenAgentsOaNodeAvailability = S.Literals([
  'available',
  'busy',
  'draining',
  'offline',
  'unknown',
])
export type OpenAgentsOaNodeAvailability =
  typeof OpenAgentsOaNodeAvailability.Type

export const OpenAgentsOaNodeQuarantineState = S.Literals([
  'none',
  'quarantined',
  'released',
  'suspected',
])
export type OpenAgentsOaNodeQuarantineState =
  typeof OpenAgentsOaNodeQuarantineState.Type

export const OpenAgentsOaNodeRuntime = S.Literals([
  'browser',
  'codex',
  'container',
  'opencode',
  'probe',
  'psionic',
  'pylon_worker',
  'shell',
])
export type OpenAgentsOaNodeRuntime = typeof OpenAgentsOaNodeRuntime.Type

export const OpenAgentsOaNodeWorkloadClass = S.Literals([
  'coding',
  'deploy',
  'eval_training',
  'forum_job',
  'pylon_provider',
  'research',
  'site_build',
])
export type OpenAgentsOaNodeWorkloadClass =
  typeof OpenAgentsOaNodeWorkloadClass.Type

export const OpenAgentsOaNodeManagedLiveness = S.Literals([
  'heartbeat_seen',
  'healthy',
  'unproven',
])
export type OpenAgentsOaNodeManagedLiveness =
  typeof OpenAgentsOaNodeManagedLiveness.Type

export const OpenAgentsOaNodeProviderPayoutEligibility = S.Literals([
  'eligible_pending_settlement',
  'not_eligible',
  'not_provider',
])
export type OpenAgentsOaNodeProviderPayoutEligibility =
  typeof OpenAgentsOaNodeProviderPayoutEligibility.Type

export class OpenAgentsOaNodeMachineRecord extends S.Class<OpenAgentsOaNodeMachineRecord>(
  'OpenAgentsOaNodeMachineRecord',
)({
  activeWorkroomRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  availability: OpenAgentsOaNodeAvailability,
  backendKind: OpenAgentsRunnerBackendKind,
  capabilityRefs: S.Array(S.String),
  displayNameRef: S.String,
  healthRefs: S.Array(S.String),
  heartbeatRefs: S.Array(S.String),
  id: S.String,
  lastHeartbeatAtIso: S.String,
  machineKind: OpenAgentsOaNodeMachineKind,
  managedLiveness: OpenAgentsOaNodeManagedLiveness,
  maxWorkloadTrust: OpenAgentsRunnerWorkloadTrust,
  nodeRef: S.String,
  operatorCaveatRefs: S.Array(S.String),
  operatorDiagnosticRefs: S.Array(S.String),
  placementEligibilityRefs: S.Array(S.String),
  policyRefs: S.Array(S.String),
  providerPayoutEligibility: OpenAgentsOaNodeProviderPayoutEligibility,
  providerPayoutEligibilityRefs: S.Array(S.String),
  publicSummaryRef: S.String,
  quarantineState: OpenAgentsOaNodeQuarantineState,
  receiptRefs: S.Array(S.String),
  supportedRuntimes: S.Array(OpenAgentsOaNodeRuntime),
  trustTier: OpenAgentsOaNodeTrustTier,
  updatedAtIso: S.String,
  workloadClasses: S.Array(OpenAgentsOaNodeWorkloadClass),
}) {}

export class OpenAgentsOaNodeMachineProjection extends S.Class<OpenAgentsOaNodeMachineProjection>(
  'OpenAgentsOaNodeMachineProjection',
)({
  activeWorkroomRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  availability: OpenAgentsOaNodeAvailability,
  backendKind: OpenAgentsRunnerBackendKind,
  capabilityRefs: S.Array(S.String),
  displayNameRef: S.String,
  healthRefs: S.Array(S.String),
  heartbeatRefs: S.Array(S.String),
  id: S.String,
  lastHeartbeatDisplay: S.String,
  machineKind: OpenAgentsOaNodeMachineKind,
  managedAvailable: S.Boolean,
  managedLiveness: OpenAgentsOaNodeManagedLiveness,
  maxWorkloadTrust: OpenAgentsRunnerWorkloadTrust,
  nodeRef: S.String,
  operatorCaveatRefs: S.Array(S.String),
  operatorDiagnosticRefs: S.Array(S.String),
  placementEligibilityRefs: S.Array(S.String),
  policyRefs: S.Array(S.String),
  providerPayoutEligibility: OpenAgentsOaNodeProviderPayoutEligibility,
  providerPayoutEligibilityRefs: S.Array(S.String),
  providerPayoutEligible: S.Boolean,
  publicSummaryRef: S.String,
  quarantineState: OpenAgentsOaNodeQuarantineState,
  receiptRefs: S.Array(S.String),
  supportedRuntimes: S.Array(OpenAgentsOaNodeRuntime),
  trustTier: OpenAgentsOaNodeTrustTier,
  updatedAtDisplay: S.String,
  workloadClasses: S.Array(OpenAgentsOaNodeWorkloadClass),
}) {}

export class OpenAgentsOaNodeMachineUnsafe extends S.TaggedErrorClass<OpenAgentsOaNodeMachineUnsafe>()(
  'OpenAgentsOaNodeMachineUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeOaNodeRefPattern =
  /(@|\/Users\/|\/home\/|127\.0\.0\.1|10\.\d{1,3}\.|172\.(1[6-9]|2\d|3[0-1])\.|192\.168\.|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|host(name)?|invoice|localhost|lnbc|lntb|lnbcrt|lno1|local[_-]?path|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(key|network)|provider[_-]?(account|grant|payload|token)|raw[_-]?(auth|email|invoice|payment|payload|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|tailnet|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(diagnostic|operator|provider\.private|payout|source\.private|workroom\.private)/i
const customerUnsafeRefPattern =
  /(diagnostic|operator|provider\.private|payout|source\.private|workroom\.private)/i
const teamUnsafeRefPattern =
  /(provider\.private|payout\.private|source\.private|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeOaNodeRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new OpenAgentsOaNodeMachineUnsafe({
      reason: `${label} contains hostnames, local paths, private network details, raw logs, operator diagnostics, auth material, private repo material, wallet/payment material, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: typeof OmniProjectionAudience.Type,
): RegExp | null => {
  if (audience === 'public' || audience === 'agent') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const safeRefForAudience = (
  label: string,
  ref: string,
  audience: typeof OmniProjectionAudience.Type,
): string =>
  safeRefsForAudience(label, [ref], audience)[0] ??
  `${label.replaceAll(' ', '_')}.redacted`

const recordRefs = (
  record: OpenAgentsOaNodeMachineRecord,
): ReadonlyArray<string> => [
  record.id,
  record.nodeRef,
  record.displayNameRef,
  record.publicSummaryRef,
  ...record.activeWorkroomRefs,
  ...record.artifactRefs,
  ...record.capabilityRefs,
  ...record.healthRefs,
  ...record.heartbeatRefs,
  ...record.operatorCaveatRefs,
  ...record.operatorDiagnosticRefs,
  ...record.placementEligibilityRefs,
  ...record.policyRefs,
  ...record.providerPayoutEligibilityRefs,
  ...record.receiptRefs,
]

const assertRecordSafe = (
  record: OpenAgentsOaNodeMachineRecord,
): void => {
  assertSafeRefs('oa-node managed-machine refs', recordRefs(record))
}

export const openAgentsOaNodeManagedAvailable = (
  record: OpenAgentsOaNodeMachineRecord,
): boolean =>
  (record.availability === 'available' ||
    record.availability === 'busy' ||
    record.availability === 'draining') &&
  (record.managedLiveness === 'healthy' ||
    record.managedLiveness === 'heartbeat_seen') &&
  record.quarantineState === 'none' &&
  record.trustTier !== 'blocked' &&
  record.trustTier !== 'quarantined'

export const openAgentsOaNodeProviderPayoutEligible = (
  record: OpenAgentsOaNodeMachineRecord,
): boolean =>
  record.providerPayoutEligibility === 'eligible_pending_settlement' &&
  record.quarantineState === 'none' &&
  (record.trustTier === 'reviewed' || record.trustTier === 'verified') &&
  record.providerPayoutEligibilityRefs.length > 0

export const projectOpenAgentsOaNodeMachine = (
  record: OpenAgentsOaNodeMachineRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OpenAgentsOaNodeMachineProjection => {
  assertRecordSafe(record)

  const projection: OpenAgentsOaNodeMachineProjection = {
    activeWorkroomRefs: safeRefsForAudience(
      'oa-node active workroom refs',
      record.activeWorkroomRefs,
      audience,
    ),
    artifactRefs: safeRefsForAudience(
      'oa-node artifact refs',
      record.artifactRefs,
      audience,
    ),
    audience,
    availability: record.availability,
    backendKind: record.backendKind,
    capabilityRefs: safeRefsForAudience(
      'oa-node capability refs',
      record.capabilityRefs,
      audience,
    ),
    displayNameRef: safeRefForAudience(
      'oa-node display name ref',
      record.displayNameRef,
      audience,
    ),
    healthRefs: safeRefsForAudience(
      'oa-node health refs',
      record.healthRefs,
      audience,
    ),
    heartbeatRefs: safeRefsForAudience(
      'oa-node heartbeat refs',
      record.heartbeatRefs,
      audience,
    ),
    id: safeRefForAudience('oa-node id', record.id, audience),
    lastHeartbeatDisplay: friendlyBlueprintMissionBriefingTime(
      record.lastHeartbeatAtIso,
      nowIso,
    ),
    machineKind: record.machineKind,
    managedAvailable: openAgentsOaNodeManagedAvailable(record),
    managedLiveness: record.managedLiveness,
    maxWorkloadTrust: record.maxWorkloadTrust,
    nodeRef: safeRefForAudience('oa-node ref', record.nodeRef, audience),
    operatorCaveatRefs:
      audience === 'operator' || audience === 'private'
        ? safeRefsForAudience(
            'oa-node operator caveat refs',
            record.operatorCaveatRefs,
            audience,
          )
        : [],
    operatorDiagnosticRefs:
      audience === 'operator' || audience === 'private'
        ? safeRefsForAudience(
            'oa-node operator diagnostic refs',
            record.operatorDiagnosticRefs,
            audience,
          )
        : [],
    placementEligibilityRefs: safeRefsForAudience(
      'oa-node placement eligibility refs',
      record.placementEligibilityRefs,
      audience,
    ),
    policyRefs: safeRefsForAudience(
      'oa-node policy refs',
      record.policyRefs,
      audience,
    ),
    providerPayoutEligibility: record.providerPayoutEligibility,
    providerPayoutEligibilityRefs:
      audience === 'operator' || audience === 'private'
        ? safeRefsForAudience(
            'oa-node provider payout eligibility refs',
            record.providerPayoutEligibilityRefs,
            audience,
          )
        : [],
    providerPayoutEligible: openAgentsOaNodeProviderPayoutEligible(record),
    publicSummaryRef: safeRefForAudience(
      'oa-node public summary ref',
      record.publicSummaryRef,
      audience,
    ),
    quarantineState: record.quarantineState,
    receiptRefs: safeRefsForAudience(
      'oa-node receipt refs',
      record.receiptRefs,
      audience,
    ),
    supportedRuntimes: [...new Set(record.supportedRuntimes)].sort(),
    trustTier: record.trustTier,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workloadClasses: [...new Set(record.workloadClasses)].sort(),
  }

  if (openAgentsOaNodeMachineProjectionHasPrivateMaterial(projection)) {
    throw new OpenAgentsOaNodeMachineUnsafe({
      reason: 'oa-node managed-machine projection contains unsafe material.',
    })
  }

  return projection
}

export const openAgentsOaNodeMachineProjectionHasPrivateMaterial = (
  projection: OpenAgentsOaNodeMachineProjection,
): boolean => {
  const serialized = JSON.stringify(projection)

  return unsafeOaNodeRefPattern.test(serialized) ||
    rawTimestampPattern.test(serialized)
}

export const OPENAGENTS_OA_NODE_CONFORMANCE_FIXTURES:
  ReadonlyArray<OpenAgentsOaNodeMachineRecord> = [
    {
      activeWorkroomRefs: ['workroom.site_public_otec'],
      artifactRefs: ['artifact.otec.preview_bundle'],
      availability: 'available',
      backendKind: 'shc_vm',
      capabilityRefs: ['capability.codex_exec', 'capability.site_build'],
      displayNameRef: 'oa_node.bertha_public',
      healthRefs: ['health.heartbeat_ok', 'health.disk_ok'],
      heartbeatRefs: ['heartbeat.oa_node.bertha.latest'],
      id: 'oa_node_machine.bertha',
      lastHeartbeatAtIso: '2026-06-07T00:58:00.000Z',
      machineKind: 'shc_vm',
      managedLiveness: 'healthy',
      maxWorkloadTrust: 'medium',
      nodeRef: 'oa_node.bertha',
      operatorCaveatRefs: ['operator.caveat.requires_manual_upgrade'],
      operatorDiagnosticRefs: ['operator.diagnostic.disk_pressure_ok'],
      placementEligibilityRefs: [
        'provider.eligibility.reviewed_private',
        'placement.eligible.site_build',
      ],
      policyRefs: ['policy.oa_node.no_raw_credentials'],
      providerPayoutEligibility: 'not_provider',
      providerPayoutEligibilityRefs: [],
      publicSummaryRef: 'summary.oa_node.available_for_site_build',
      quarantineState: 'none',
      receiptRefs: ['receipt.oa_node.heartbeat_seen'],
      supportedRuntimes: ['codex', 'opencode', 'shell'],
      trustTier: 'reviewed',
      updatedAtIso: '2026-06-07T00:59:00.000Z',
      workloadClasses: ['coding', 'site_build'],
    },
    {
      activeWorkroomRefs: ['workroom.pylon_provider_test'],
      artifactRefs: ['artifact.pylon.provider_summary'],
      availability: 'busy',
      backendKind: 'gcloud_vm',
      capabilityRefs: ['capability.pylon_provider', 'capability.forum_job'],
      displayNameRef: 'oa_node.pylon_candidate_public',
      healthRefs: ['health.heartbeat_ok'],
      heartbeatRefs: ['heartbeat.oa_node.pylon_candidate.latest'],
      id: 'oa_node_machine.pylon_candidate',
      lastHeartbeatAtIso: '2026-06-07T00:55:00.000Z',
      machineKind: 'pylon_candidate',
      managedLiveness: 'heartbeat_seen',
      maxWorkloadTrust: 'medium',
      nodeRef: 'oa_node.pylon_candidate',
      operatorCaveatRefs: ['operator.caveat.awaiting_provider_review'],
      operatorDiagnosticRefs: [],
      placementEligibilityRefs: ['placement.eligible.forum_job'],
      policyRefs: ['policy.oa_node.no_direct_payment_mutation'],
      providerPayoutEligibility: 'eligible_pending_settlement',
      providerPayoutEligibilityRefs: [
        'payout_eligibility.pylon_provider.reviewed',
      ],
      publicSummaryRef: 'summary.oa_node.pylon_candidate',
      quarantineState: 'none',
      receiptRefs: ['receipt.oa_node.pylon_provider_seen'],
      supportedRuntimes: ['pylon_worker', 'shell'],
      trustTier: 'verified',
      updatedAtIso: '2026-06-07T00:57:00.000Z',
      workloadClasses: ['forum_job', 'pylon_provider'],
    },
  ]
