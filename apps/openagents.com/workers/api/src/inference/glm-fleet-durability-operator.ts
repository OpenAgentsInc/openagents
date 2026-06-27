import type {
  GlmFleetAcceptanceDimensionStatus,
  GlmFleetReadinessOperatorReadout,
  GlmFleetReadinessProjection,
} from './glm-fleet-readiness'
import { summarizeGlmFleetReadinessForOperators } from './glm-fleet-readiness'

export const GLM_FLEET_DURABILITY_OPERATOR_BUNDLE_SCHEMA =
  'openagents.khala.glm_fleet_durability_operator_bundle.v1' as const

export type GlmFleetDurabilityOperatorInputStatus =
  | 'missing'
  | 'pending'
  | 'rejected_unsafe'

export type GlmFleetDurabilityOperatorInput = Readonly<{
  env: string
  flag: string
  label: string
  status: GlmFleetDurabilityOperatorInputStatus
}>

export type GlmFleetDurabilityOwnerArmedCommandInput = Readonly<{
  outputDir?: string | undefined
  readinessUrl?: string | undefined
}>

export type GlmFleetDurabilityOperatorBundle = Readonly<{
  schemaVersion: typeof GLM_FLEET_DURABILITY_OPERATOR_BUNDLE_SCHEMA
  generatedAt: string
  issueRef: 'github.issue.OpenAgentsInc.openagents.6311'
  publicSafe: true
  readiness: GlmFleetReadinessOperatorReadout
  acceptance: GlmFleetReadinessProjection['acceptance']
  counts: GlmFleetReadinessProjection['counts']
  missingOperatorInputs: ReadonlyArray<GlmFleetDurabilityOperatorInput>
  ownerArmedCommand: string
  retentionNotes: ReadonlyArray<string>
}>

const DEFAULT_READINESS_URL =
  'https://openagents.com/v1/gateway/glm-fleet/readiness'
const DEFAULT_PUBLIC_OUTPUT_DIR = '.pilot-evidence/glm-fleet-durability-6311'
const safeRelativeOutputDirPattern =
  /^(?:\.?[a-z0-9][a-z0-9._-]*)(?:\/[a-z0-9][a-z0-9._-]*){0,8}$/i

const publicSafeOutputDirForCommand = (
  outputDir: string | undefined,
): string => {
  const trimmed = outputDir?.trim()
  if (
    trimmed === undefined ||
    trimmed === '' ||
    trimmed.startsWith('/') ||
    trimmed.startsWith('~') ||
    trimmed.includes('..') ||
    trimmed.includes('://') ||
    !safeRelativeOutputDirPattern.test(trimmed)
  ) {
    return DEFAULT_PUBLIC_OUTPUT_DIR
  }
  return trimmed
}

const publicSafeReadinessUrlForCommand = (
  readinessUrl: string | undefined,
): string => {
  const trimmed = readinessUrl?.trim()
  if (trimmed === undefined || trimmed === '') {
    return DEFAULT_READINESS_URL
  }
  try {
    const parsed = new URL(trimmed)
    return parsed.protocol === 'https:' &&
      parsed.hostname === 'openagents.com' &&
      parsed.pathname === '/v1/gateway/glm-fleet/readiness'
      ? parsed.toString()
      : DEFAULT_READINESS_URL
  } catch {
    return DEFAULT_READINESS_URL
  }
}

const input = (
  blockerRef: string,
): GlmFleetDurabilityOperatorInput | undefined => {
  switch (blockerRef) {
    case 'blocker.hydralisk_glm_52_reap_504b.all_replica_keep_warm_watchdog_incomplete':
      return {
        env: 'HYDRALISK_GLM_52_REAP_504B_<REPLICA>_KEEPWARM_WATCHDOG',
        flag: 'owner-local heartbeat rows',
        label:
          'keep-warm timer plus STOP-watchdog healthy on every required GLM replica',
        status: 'missing',
      }
    case 'blocker.hydralisk_glm_52_reap_504b.all_replica_keep_warm_watchdog_no_required_replicas':
      return {
        env: 'HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS',
        flag: '--replica-roster',
        label: 'non-empty GLM replica roster',
        status: 'missing',
      }
    case 'blocker.hydralisk_glm_52_reap_504b.forced_stop_recovery_evidence_missing':
      return {
        env: 'HYDRALISK_GLM_52_REAP_504B_FORCED_STOP_RECOVERY_REFS',
        flag: '--forced-stop-recovery-ref',
        label:
          'public ref proving a previously-unwatched Spot STOP auto-recovered to ready',
        status: 'missing',
      }
    case 'blocker.hydralisk_glm_52_reap_504b.capacity_floor_owner_decision_missing':
      return {
        env: 'HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION',
        flag: '--capacity-floor-decision',
        label:
          'owner decision: non_spot_floor_approved or owner_accepted_all_spot',
        status: 'missing',
      }
    case 'blocker.hydralisk_glm_52_reap_504b.capacity_floor_owner_decision_evidence_missing':
      return {
        env: 'HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION_REF',
        flag: '--capacity-floor-decision-ref',
        label: 'public-safe capacity-floor decision evidence ref',
        status: 'missing',
      }
    case 'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_evidence_missing':
      return {
        env: 'HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_REF',
        flag: '--auto-replace-ref',
        label: 'public-safe multi-region auto-replace plan/proof ref',
        status: 'missing',
      }
    case 'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_replacement_region_missing':
      return {
        env: 'HYDRALISK_GLM_52_REAP_504B_<REPLICA>_BENCHMARK_RESERVED',
        flag: '--replacement-region-ref',
        label: 'configured replacement/reserve replica in a second region',
        status: 'missing',
      }
    case 'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_reserve_evidence_missing':
      return {
        env: 'HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_RESERVE_REFS',
        flag: '--reserve-ref',
        label: 'public-safe reserve capacity evidence ref',
        status: 'missing',
      }
    case 'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_prebake_evidence_missing':
      return {
        env: 'HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_PREBAKE_REFS',
        flag: '--prebake-ref',
        label: 'public-safe prebaked-image/weights evidence ref',
        status: 'missing',
      }
    case 'blocker.hydralisk_glm_52_reap_504b.quota_request_state_missing':
      return {
        env: 'HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_STATE',
        flag: '--quota-request-state',
        label: 'quota request state: pending, approved, or denied',
        status: 'missing',
      }
    case 'blocker.hydralisk_glm_52_reap_504b.quota_request_evidence_missing':
      return {
        env: 'HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_REF',
        flag: '--quota-request-ref',
        label: 'public-safe GCP quota request evidence ref',
        status: 'missing',
      }
    case 'blocker.hydralisk_glm_52_reap_504b.quota_request_pending':
      return {
        env: 'HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_STATE',
        flag: '--quota-request-state approved',
        label: 'quota request still pending',
        status: 'pending',
      }
    case 'blocker.hydralisk_glm_52_reap_504b.quota_request_denied':
      return {
        env: 'HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_REF',
        flag: '--quota-request-ref',
        label: 'quota request denied; capacity strategy needs replacement evidence',
        status: 'missing',
      }
    case 'blocker.hydralisk_glm_52_reap_504b.quota_request_state_unknown':
      return {
        env: 'HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_STATE',
        flag: '--quota-request-state',
        label: 'bounded quota request state',
        status: 'rejected_unsafe',
      }
    default:
      return undefined
  }
}

const uniqueInputs = (
  inputs: ReadonlyArray<GlmFleetDurabilityOperatorInput>,
): ReadonlyArray<GlmFleetDurabilityOperatorInput> => [
  ...new Map(
    inputs.map(operatorInput => [
      `${operatorInput.env}:${operatorInput.flag}:${operatorInput.status}`,
      operatorInput,
    ]),
  ).values(),
]

export const collectGlmFleetDurabilityMissingOperatorInputs = (
  readout: GlmFleetReadinessOperatorReadout,
): ReadonlyArray<GlmFleetDurabilityOperatorInput> =>
  uniqueInputs(readout.blockerRefs.flatMap(ref => input(ref) ?? []))

const ownerArmedCommandLines = (
  input: Required<GlmFleetDurabilityOwnerArmedCommandInput>,
): ReadonlyArray<string> => [
  '# 1. Configure these public-safe Worker vars/refs, then deploy.',
  '# HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION="owner_accepted_all_spot"',
  '# HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION_REF="decision.public.khala.glm_52_reap_504b.capacity_floor.<owner-issued>"',
  '# HYDRALISK_GLM_52_REAP_504B_FORCED_STOP_RECOVERY_REFS="evidence.public.khala.glm_52_reap_504b.forced_stop_recovery.<owner-issued>"',
  '# HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_REF="evidence.public.khala.glm_52_reap_504b.auto_replace.<owner-issued>"',
  '# HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_RESERVE_REFS="reserve.public.khala.glm_52_reap_504b.<owner-issued>"',
  '# HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_PREBAKE_REFS="prebake.public.khala.glm_52_reap_504b.<owner-issued>"',
  '# HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_STATE="pending"',
  '# HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_REF="quota_request.public.gcp.rtx_pro_6000.<owner-issued>"',
  'bun run --cwd apps/openagents.com/workers/api deploy:safe',
  '# 2. Retain the public-safe durability bundle after the deployed projection updates.',
  `bun run --cwd apps/openagents.com/workers/api glm-fleet:durability --readiness-url ${input.readinessUrl} --output-dir ${input.outputDir}`,
]

export const buildGlmFleetDurabilityOwnerArmedCommand = (
  input: GlmFleetDurabilityOwnerArmedCommandInput = {},
): string =>
  ownerArmedCommandLines({
    outputDir: publicSafeOutputDirForCommand(input.outputDir),
    readinessUrl: publicSafeReadinessUrlForCommand(input.readinessUrl),
  }).join('\n')

export const buildGlmFleetDurabilityOperatorBundle = (input: {
  generatedAt: string
  outputDir?: string | undefined
  projection: GlmFleetReadinessProjection
  readinessUrl?: string | undefined
}): GlmFleetDurabilityOperatorBundle => {
  const readiness = summarizeGlmFleetReadinessForOperators(input.projection)
  return {
    schemaVersion: GLM_FLEET_DURABILITY_OPERATOR_BUNDLE_SCHEMA,
    generatedAt: input.generatedAt,
    issueRef: 'github.issue.OpenAgentsInc.openagents.6311',
    publicSafe: true,
    readiness,
    acceptance: input.projection.acceptance,
    counts: input.projection.counts,
    missingOperatorInputs:
      collectGlmFleetDurabilityMissingOperatorInputs(readiness),
    ownerArmedCommand: buildGlmFleetDurabilityOwnerArmedCommand({
      outputDir: input.outputDir,
      readinessUrl: input.readinessUrl,
    }),
    retentionNotes: [
      'This bundle intentionally contains public refs, aggregate readiness counts, blockers, and operator-safe command templates only.',
      'It does not contain replica origin URLs, IPs, API keys, bearer tokens, private GCP project details, raw traces, or host-local paths.',
      'A blocked bundle is evidence that #6311 remains incomplete; it is not evidence that the durable GLM fleet is accepted.',
      'The forced-stop recovery ref must represent a real Spot STOP drill on a previously-unwatched host that recovered to ready without manual intervention.',
    ],
  }
}

const statusLine = (
  label: string,
  status: GlmFleetAcceptanceDimensionStatus,
): string => `- ${label}: ${status}`

const listOrNone = (values: ReadonlyArray<string>): string =>
  values.length === 0 ? 'none' : values.join(', ')

export const formatGlmFleetDurabilityOperatorReadme = (
  bundle: GlmFleetDurabilityOperatorBundle,
): string => {
  const missingInputs =
    bundle.missingOperatorInputs.length === 0
      ? ['- none']
      : bundle.missingOperatorInputs.map(
          input =>
            `- ${input.env} (${input.status}) - ${input.label} [${input.flag}]`,
        )
  const actionItems =
    bundle.readiness.operatorActionItems.length === 0
      ? ['- none']
      : bundle.readiness.operatorActionItems.map(
          item =>
            `- ${item.action} (${item.severity}) - ${item.label}; replicas: ${listOrNone(item.replicaRefs)}; blockers: ${listOrNone(item.blockerRefs)}`,
        )
  return [
    '# GLM fleet durability operator bundle',
    '',
    `Generated: ${bundle.generatedAt}`,
    `Issue: ${bundle.issueRef}`,
    `Acceptance: ${bundle.readiness.acceptanceStatus}`,
    `Serving: ${bundle.readiness.servingStatus}`,
    `Serving capacity summary: ${bundle.readiness.servingCapacitySummary}`,
    `Serving ready but durability acceptance incomplete: ${bundle.readiness.servingReadyButAcceptanceNotComplete}`,
    '',
    '## Serving counts',
    '',
    `- total replicas: ${bundle.readiness.counts.totalReplicaCount}`,
    `- ready replicas: ${bundle.readiness.counts.readyReplicaCount}`,
    `- warm replicas: ${bundle.readiness.counts.warmReplicaCount}`,
    `- reclaimed replicas: ${bundle.readiness.counts.reclaimedReplicaCount}`,
    `- unavailable replicas: ${bundle.readiness.counts.unavailableReplicaCount}`,
    `- disabled replicas: ${bundle.readiness.counts.disabledReplicaCount}`,
    `- warm-or-ready max inflight: ${bundle.readiness.counts.warmOrReadyMaxInflight}`,
    '',
    '## Dimensions',
    '',
    statusLine(
      'all-replica keep-warm/watchdog',
      bundle.acceptance.allReplicaKeepWarmWatchdog.status,
    ),
    statusLine(
      'capacity-floor owner decision',
      bundle.acceptance.capacityFloorOwnerDecision.status,
    ),
    statusLine(
      'multi-region auto-replace',
      bundle.acceptance.multiRegionAutoReplace.status,
    ),
    statusLine(
      'quota request tracking',
      bundle.acceptance.quotaRequestTracking.status,
    ),
    '',
    '## Operator action items',
    '',
    ...actionItems,
    '',
    '## Missing operator inputs',
    '',
    ...missingInputs,
    '',
    '## Owner-armed command template',
    '',
    '```sh',
    bundle.ownerArmedCommand,
    '```',
    '',
    '## Retention notes',
    '',
    ...bundle.retentionNotes.map(note => `- ${note}`),
    '',
  ].join('\n')
}
