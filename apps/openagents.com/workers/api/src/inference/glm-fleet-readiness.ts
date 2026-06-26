import type {
  GlmPoolHeartbeatReplicaRecord,
  GlmPoolHeartbeatProbeStatus,
  GlmPoolHeartbeatWarmState,
  GlmPoolKeepWarmStatus,
  GlmPoolWatchdogStatus,
} from './glm-pool-heartbeat'
import {
  type HydraliskGlm52ReplicaArming,
  type SupplyLaneCredentialEnv,
  resolveHydraliskGlm52Reap504bReplicaArmings,
} from './model-serving-policy'
import { HYDRALISK_GLM_52_REAP_504B_MODEL_ID } from './pricing'

export type GlmFleetReplicaReadinessStatus =
  | 'disabled'
  | 'ready'
  | 'reclaimed'
  | 'unavailable'
  | 'warm'

export type GlmFleetReadinessStatus = 'degraded' | 'ready' | 'unavailable'

export type GlmFleetReadinessReplica = Readonly<{
  armingEvidenceRefs: ReadonlyArray<string>
  benchmarkReserved: boolean
  blockerRefs: ReadonlyArray<string>
  costProfileRef?: string | undefined
  draining: boolean
  healthStatus?: GlmPoolHeartbeatProbeStatus | undefined
  keepWarmStatus?: GlmPoolKeepWarmStatus | undefined
  latestHeartbeatAt?: string | undefined
  maxInflight: number
  modelsStatus?: GlmPoolHeartbeatProbeStatus | undefined
  profileRef?: string | undefined
  replicaId: string
  replicaRef: string
  status: GlmFleetReplicaReadinessStatus
  totalWallClockMs?: number | undefined
  warmCompletionStatus?: GlmPoolHeartbeatProbeStatus | undefined
  warmState?: GlmPoolHeartbeatWarmState | undefined
  watchdogStatus?: GlmPoolWatchdogStatus | undefined
}>

export type GlmFleetReadinessCounts = Readonly<{
  activeMaxInflight: number
  benchmarkReservedReplicaCount: number
  configuredMaxInflight: number
  disabledReplicaCount: number
  drainingReplicaCount: number
  readyMaxInflight: number
  readyReplicaCount: number
  reclaimedReplicaCount: number
  totalReplicaCount: number
  unavailableReplicaCount: number
  warmMaxInflight: number
  warmOrReadyMaxInflight: number
  warmReplicaCount: number
}>

export type GlmFleetAcceptanceDimensionStatus =
  | 'blocked'
  | 'complete'
  | 'incomplete'

export type GlmFleetCapacityFloorDecision =
  | 'missing'
  | 'non_spot_floor_approved'
  | 'owner_accepted_all_spot'

export type GlmFleetQuotaRequestState =
  | 'approved'
  | 'missing'
  | 'pending'
  | 'rejected'
  | 'unknown'

export type GlmFleetReadinessAcceptance = Readonly<{
  allReplicaKeepWarmWatchdog: Readonly<{
    blockerRefs: ReadonlyArray<string>
    coveredReplicaCount: number
    evidenceRefs: ReadonlyArray<string>
    missingReplicaRefs: ReadonlyArray<string>
    status: GlmFleetAcceptanceDimensionStatus
    totalRequiredReplicaCount: number
  }>
  capacityFloorOwnerDecision: Readonly<{
    blockerRefs: ReadonlyArray<string>
    decision: GlmFleetCapacityFloorDecision
    evidenceRefs: ReadonlyArray<string>
    status: GlmFleetAcceptanceDimensionStatus
  }>
  multiRegionAutoReplace: Readonly<{
    blockerRefs: ReadonlyArray<string>
    evidenceRefs: ReadonlyArray<string>
    status: GlmFleetAcceptanceDimensionStatus
  }>
  quotaRequestTracking: Readonly<{
    blockerRefs: ReadonlyArray<string>
    evidenceRefs: ReadonlyArray<string>
    requestState: GlmFleetQuotaRequestState
    status: GlmFleetAcceptanceDimensionStatus
  }>
  status: GlmFleetAcceptanceDimensionStatus
}>

export type GlmFleetReadinessProjection = Readonly<{
  kind: 'glm_fleet_readiness'
  model: typeof HYDRALISK_GLM_52_REAP_504B_MODEL_ID
  status: GlmFleetReadinessStatus
  acceptance: GlmFleetReadinessAcceptance
  configuredReplicaRefs: ReadonlyArray<string>
  replicas: ReadonlyArray<GlmFleetReadinessReplica>
  counts: GlmFleetReadinessCounts
}>

export type GlmFleetReadinessHeartbeatRecord = Readonly<{
  benchmarkReserved?: GlmPoolHeartbeatReplicaRecord['benchmarkReserved']
  breakerConsecutiveFailures?: GlmPoolHeartbeatReplicaRecord['breakerConsecutiveFailures']
  breakerConsecutiveSuccesses?: GlmPoolHeartbeatReplicaRecord['breakerConsecutiveSuccesses']
  breakerFailureThreshold?: GlmPoolHeartbeatReplicaRecord['breakerFailureThreshold']
  breakerReadmitSuccessThreshold?: GlmPoolHeartbeatReplicaRecord['breakerReadmitSuccessThreshold']
  draining?: GlmPoolHeartbeatReplicaRecord['draining']
  healthStatus?: GlmPoolHeartbeatReplicaRecord['healthStatus']
  keepWarmStatus?: GlmPoolHeartbeatReplicaRecord['keepWarmStatus']
  modelsStatus?: GlmPoolHeartbeatReplicaRecord['modelsStatus']
  observedAt: GlmPoolHeartbeatReplicaRecord['observedAt']
  probeTimeoutMs?: GlmPoolHeartbeatReplicaRecord['probeTimeoutMs']
  replicaId: GlmPoolHeartbeatReplicaRecord['replicaId']
  runRef?: GlmPoolHeartbeatReplicaRecord['runRef']
  totalWallClockMs?: GlmPoolHeartbeatReplicaRecord['totalWallClockMs']
  warmCompletionStatus?: GlmPoolHeartbeatReplicaRecord['warmCompletionStatus']
  warmState: GlmPoolHeartbeatReplicaRecord['warmState']
  watchdogStatus: GlmPoolHeartbeatReplicaRecord['watchdogStatus']
}>

const REPLICA_REF_PREFIX = 'replica.hydralisk.glm_52_reap_504b'
const INTERNAL_CONFIGURATION_REPLICA_ID = 'configuration'
const PUBLIC_SAFE_REF = /^[a-z0-9][a-z0-9._:-]{1,199}$/iu

type GlmFleetReadinessEnv = SupplyLaneCredentialEnv &
  Readonly<{
    HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION?: string | undefined
    HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION_REF?:
      | string
      | undefined
    HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_REF?:
      | string
      | undefined
    HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_REF?: string | undefined
    HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_STATE?: string | undefined
  }>

const replicaRefFor = (replicaId: string): string =>
  `${REPLICA_REF_PREFIX}.${replicaId}`

const isPublicSafeRef = (value: string | undefined): value is string => {
  if (typeof value !== 'string' || value.trim() === '') {
    return false
  }
  const trimmed = value.trim()
  return (
    trimmed === value &&
    PUBLIC_SAFE_REF.test(trimmed) &&
    !trimmed.includes('://') &&
    !trimmed.toLowerCase().startsWith('sk-')
  )
}

const publicSafeRefs = (
  values: ReadonlyArray<string | undefined>,
): ReadonlyArray<string> => values.filter(isPublicSafeRef)

const replicaStatus = (
  arming: HydraliskGlm52ReplicaArming,
  heartbeat: GlmFleetReadinessHeartbeatRecord | undefined,
): GlmFleetReplicaReadinessStatus => {
  if (
    arming.replica?.benchmarkReserved === true ||
    arming.replica?.draining === true
  ) {
    return 'disabled'
  }
  if (!arming.armed) {
    return arming.blockerRefs.some(ref => ref.endsWith('.route_not_ready')) &&
      arming.blockerRefs.every(
        ref =>
          ref.endsWith('.route_not_ready') ||
          ref.endsWith('.replica_id_duplicate'),
      )
      ? 'disabled'
      : 'unavailable'
  }
  if (heartbeat === undefined) {
    return 'unavailable'
  }
  if (heartbeat.watchdogStatus === 'unhealthy') {
    return 'reclaimed'
  }
  if (heartbeat.watchdogStatus === 'degraded') {
    return 'unavailable'
  }
  if (heartbeat.warmState === 'warm') {
    return 'warm'
  }
  if (heartbeat.watchdogStatus === 'healthy') {
    return 'ready'
  }
  return 'unavailable'
}

const zeroCounts = (): GlmFleetReadinessCounts => ({
  activeMaxInflight: 0,
  benchmarkReservedReplicaCount: 0,
  configuredMaxInflight: 0,
  disabledReplicaCount: 0,
  drainingReplicaCount: 0,
  readyMaxInflight: 0,
  readyReplicaCount: 0,
  reclaimedReplicaCount: 0,
  totalReplicaCount: 0,
  unavailableReplicaCount: 0,
  warmMaxInflight: 0,
  warmOrReadyMaxInflight: 0,
  warmReplicaCount: 0,
})

const countReplicas = (
  replicas: ReadonlyArray<GlmFleetReadinessReplica>,
): GlmFleetReadinessCounts =>
  replicas.reduce((counts, replica) => {
    const configuredMaxInflight =
      counts.configuredMaxInflight + replica.maxInflight
    const activeMaxInflight =
      replica.status === 'disabled'
        ? counts.activeMaxInflight
        : counts.activeMaxInflight + replica.maxInflight
    const benchmarkReservedReplicaCount = replica.benchmarkReserved
      ? counts.benchmarkReservedReplicaCount + 1
      : counts.benchmarkReservedReplicaCount
    const drainingReplicaCount = replica.draining
      ? counts.drainingReplicaCount + 1
      : counts.drainingReplicaCount

    if (replica.status === 'disabled') {
      return {
        ...counts,
        activeMaxInflight,
        benchmarkReservedReplicaCount,
        configuredMaxInflight,
        disabledReplicaCount: counts.disabledReplicaCount + 1,
        drainingReplicaCount,
        totalReplicaCount: counts.totalReplicaCount + 1,
      }
    }
    if (replica.status === 'ready') {
      return {
        ...counts,
        activeMaxInflight,
        benchmarkReservedReplicaCount,
        configuredMaxInflight,
        drainingReplicaCount,
        readyMaxInflight: counts.readyMaxInflight + replica.maxInflight,
        readyReplicaCount: counts.readyReplicaCount + 1,
        totalReplicaCount: counts.totalReplicaCount + 1,
        warmOrReadyMaxInflight:
          counts.warmOrReadyMaxInflight + replica.maxInflight,
      }
    }
    if (replica.status === 'reclaimed') {
      return {
        ...counts,
        activeMaxInflight,
        benchmarkReservedReplicaCount,
        configuredMaxInflight,
        drainingReplicaCount,
        reclaimedReplicaCount: counts.reclaimedReplicaCount + 1,
        totalReplicaCount: counts.totalReplicaCount + 1,
      }
    }
    if (replica.status === 'unavailable') {
      return {
        ...counts,
        activeMaxInflight,
        benchmarkReservedReplicaCount,
        configuredMaxInflight,
        drainingReplicaCount,
        totalReplicaCount: counts.totalReplicaCount + 1,
        unavailableReplicaCount: counts.unavailableReplicaCount + 1,
      }
    }
    return {
      ...counts,
      activeMaxInflight,
      benchmarkReservedReplicaCount,
      configuredMaxInflight,
      drainingReplicaCount,
      totalReplicaCount: counts.totalReplicaCount + 1,
      warmMaxInflight: counts.warmMaxInflight + replica.maxInflight,
      warmOrReadyMaxInflight:
        counts.warmOrReadyMaxInflight + replica.maxInflight,
      warmReplicaCount: counts.warmReplicaCount + 1,
    }
  }, zeroCounts())

const replicaMaxInflight = (arming: HydraliskGlm52ReplicaArming): number =>
  arming.replica?.maxInflight ?? 0

const replicaBenchmarkReserved = (
  arming: HydraliskGlm52ReplicaArming,
  heartbeat: GlmFleetReadinessHeartbeatRecord | undefined,
): boolean =>
  arming.replica?.benchmarkReserved ?? heartbeat?.benchmarkReserved ?? false

const replicaDraining = (
  arming: HydraliskGlm52ReplicaArming,
  heartbeat: GlmFleetReadinessHeartbeatRecord | undefined,
): boolean => arming.replica?.draining ?? heartbeat?.draining ?? false

const fleetStatus = (
  counts: GlmFleetReadinessCounts,
): GlmFleetReadinessStatus => {
  if (
    counts.warmReplicaCount + counts.readyReplicaCount ===
    counts.totalReplicaCount
  ) {
    return counts.totalReplicaCount === 0 ? 'unavailable' : 'ready'
  }
  if (counts.warmReplicaCount + counts.readyReplicaCount > 0) {
    return 'degraded'
  }
  return 'unavailable'
}

const normalizeCapacityFloorDecision = (
  value: string | undefined,
): GlmFleetCapacityFloorDecision => {
  const normalized = value?.trim().toLowerCase()
  if (
    normalized === 'non_spot_floor_approved' ||
    normalized === 'owner_accepted_all_spot'
  ) {
    return normalized
  }
  return 'missing'
}

const normalizeQuotaRequestState = (
  value: string | undefined,
): GlmFleetQuotaRequestState => {
  const normalized = value?.trim().toLowerCase()
  if (
    normalized === 'approved' ||
    normalized === 'pending' ||
    normalized === 'rejected'
  ) {
    return normalized
  }
  return normalized === undefined || normalized === '' ? 'missing' : 'unknown'
}

const dimensionStatus = (
  statuses: ReadonlyArray<GlmFleetAcceptanceDimensionStatus>,
): GlmFleetAcceptanceDimensionStatus => {
  if (statuses.some(status => status === 'blocked')) {
    return 'blocked'
  }
  if (statuses.some(status => status === 'incomplete')) {
    return 'incomplete'
  }
  return 'complete'
}

const acceptanceFor = (
  input: Readonly<{
    env: GlmFleetReadinessEnv
    replicas: ReadonlyArray<GlmFleetReadinessReplica>
  }>,
): GlmFleetReadinessAcceptance => {
  const capacityFloorEvidenceRefs = publicSafeRefs([
    input.env.HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION_REF,
  ])
  const capacityFloorDecision = normalizeCapacityFloorDecision(
    input.env.HYDRALISK_GLM_52_REAP_504B_CAPACITY_FLOOR_DECISION,
  )
  const capacityFloorOwnerDecision =
    capacityFloorDecision === 'missing' || capacityFloorEvidenceRefs.length === 0
      ? {
          blockerRefs: [
            'blocker.hydralisk_glm_52_reap_504b.capacity_floor_owner_decision_missing',
          ],
          decision: capacityFloorDecision,
          evidenceRefs: capacityFloorEvidenceRefs,
          status: 'blocked' as const,
        }
      : {
          blockerRefs: [],
          decision: capacityFloorDecision,
          evidenceRefs: capacityFloorEvidenceRefs,
          status: 'complete' as const,
        }

  const requiredReplicas = input.replicas.filter(
    replica => !replica.benchmarkReserved && !replica.draining,
  )
  const coveredReplicas = requiredReplicas.filter(
    replica =>
      replica.keepWarmStatus === 'completed' &&
      replica.watchdogStatus === 'healthy',
  )
  const missingReplicaRefs = requiredReplicas
    .filter(replica => {
      return !coveredReplicas.some(
        covered => covered.replicaRef === replica.replicaRef,
      )
    })
    .map(replica => replica.replicaRef)
  const allReplicaKeepWarmWatchdog: GlmFleetReadinessAcceptance['allReplicaKeepWarmWatchdog'] =
    requiredReplicas.length === 0
      ? {
          blockerRefs: [
            'blocker.hydralisk_glm_52_reap_504b.all_replica_keep_warm_watchdog_no_required_replicas',
          ],
          coveredReplicaCount: 0,
          evidenceRefs: [],
          missingReplicaRefs: [],
          status: 'blocked' as const,
          totalRequiredReplicaCount: 0,
        }
      : missingReplicaRefs.length === 0
        ? {
            blockerRefs: [],
            coveredReplicaCount: coveredReplicas.length,
            evidenceRefs: coveredReplicas.map(replica => replica.replicaRef),
            missingReplicaRefs,
            status: 'complete' as const,
            totalRequiredReplicaCount: requiredReplicas.length,
          }
        : {
            blockerRefs: [
              'blocker.hydralisk_glm_52_reap_504b.all_replica_keep_warm_watchdog_incomplete',
            ],
            coveredReplicaCount: coveredReplicas.length,
            evidenceRefs: coveredReplicas.map(replica => replica.replicaRef),
            missingReplicaRefs,
            status:
              coveredReplicas.length === 0
                ? ('blocked' as const)
                : ('incomplete' as const),
            totalRequiredReplicaCount: requiredReplicas.length,
          }

  const autoReplaceEvidenceRefs = publicSafeRefs([
    input.env.HYDRALISK_GLM_52_REAP_504B_MULTI_REGION_AUTO_REPLACE_REF,
  ])
  const multiRegionAutoReplace =
    autoReplaceEvidenceRefs.length === 0
      ? {
          blockerRefs: [
            'blocker.hydralisk_glm_52_reap_504b.multi_region_auto_replace_evidence_missing',
          ],
          evidenceRefs: [],
          status: 'blocked' as const,
        }
      : {
          blockerRefs: [],
          evidenceRefs: autoReplaceEvidenceRefs,
          status: 'complete' as const,
        }

  const quotaRequestEvidenceRefs = publicSafeRefs([
    input.env.HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_REF,
  ])
  const quotaRequestState = normalizeQuotaRequestState(
    input.env.HYDRALISK_GLM_52_REAP_504B_QUOTA_REQUEST_STATE,
  )
  const quotaRequestTracking =
    quotaRequestEvidenceRefs.length === 0 || quotaRequestState === 'missing'
      ? {
          blockerRefs: [
            'blocker.hydralisk_glm_52_reap_504b.quota_request_tracking_missing',
          ],
          evidenceRefs: quotaRequestEvidenceRefs,
          requestState: quotaRequestState,
          status: 'blocked' as const,
        }
      : {
          blockerRefs:
            quotaRequestState === 'approved'
              ? []
              : [
                  `blocker.hydralisk_glm_52_reap_504b.quota_request_${quotaRequestState}`,
                ],
          evidenceRefs: quotaRequestEvidenceRefs,
          requestState: quotaRequestState,
          status:
            quotaRequestState === 'approved'
              ? ('complete' as const)
              : ('incomplete' as const),
        }

  return {
    allReplicaKeepWarmWatchdog,
    capacityFloorOwnerDecision,
    multiRegionAutoReplace,
    quotaRequestTracking,
    status: dimensionStatus([
      allReplicaKeepWarmWatchdog.status,
      capacityFloorOwnerDecision.status,
      multiRegionAutoReplace.status,
      quotaRequestTracking.status,
    ]),
  }
}

export const projectGlmFleetReadiness = (
  input: Readonly<{
    env?: GlmFleetReadinessEnv | undefined
    replicaArmings: ReadonlyArray<HydraliskGlm52ReplicaArming>
    latestHeartbeatRecord: (
      replicaId: string,
    ) => GlmFleetReadinessHeartbeatRecord | undefined
  }>,
): GlmFleetReadinessProjection => {
  const replicas = input.replicaArmings
    .filter(arming => arming.replicaId !== INTERNAL_CONFIGURATION_REPLICA_ID)
    .map((arming): GlmFleetReadinessReplica => {
      const heartbeat = input.latestHeartbeatRecord(arming.replicaId)
      return {
        armingEvidenceRefs: arming.evidenceRefs,
        benchmarkReserved: replicaBenchmarkReserved(arming, heartbeat),
        blockerRefs: arming.blockerRefs,
        draining: replicaDraining(arming, heartbeat),
        ...(heartbeat?.healthStatus === undefined
          ? {}
          : { healthStatus: heartbeat.healthStatus }),
        ...(heartbeat?.keepWarmStatus === undefined
          ? {}
          : { keepWarmStatus: heartbeat.keepWarmStatus }),
        ...(heartbeat?.observedAt === undefined
          ? {}
          : { latestHeartbeatAt: heartbeat.observedAt }),
        maxInflight: replicaMaxInflight(arming),
        ...(heartbeat?.modelsStatus === undefined
          ? {}
          : { modelsStatus: heartbeat.modelsStatus }),
        ...(arming.replica?.costProfileRef === undefined
          ? {}
          : { costProfileRef: arming.replica.costProfileRef }),
        ...(arming.replica?.profileRef === undefined
          ? {}
          : { profileRef: arming.replica.profileRef }),
        replicaId: arming.replicaId,
        replicaRef: replicaRefFor(arming.replicaId),
        status: replicaStatus(arming, heartbeat),
        ...(heartbeat?.totalWallClockMs === undefined
          ? {}
          : { totalWallClockMs: heartbeat.totalWallClockMs }),
        ...(heartbeat?.warmCompletionStatus === undefined
          ? {}
          : { warmCompletionStatus: heartbeat.warmCompletionStatus }),
        ...(heartbeat?.warmState === undefined
          ? {}
          : { warmState: heartbeat.warmState }),
        ...(heartbeat?.watchdogStatus === undefined
          ? {}
          : { watchdogStatus: heartbeat.watchdogStatus }),
      }
    })
    .sort((left, right) => left.replicaRef.localeCompare(right.replicaRef))
  const counts = countReplicas(replicas)
  const acceptance = acceptanceFor({ env: input.env ?? {}, replicas })

  return {
    acceptance,
    configuredReplicaRefs: replicas.map(replica => replica.replicaRef),
    counts,
    kind: 'glm_fleet_readiness',
    model: HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
    replicas,
    status: fleetStatus(counts),
  }
}

export const projectGlmFleetReadinessForEnv = (
  env: GlmFleetReadinessEnv,
  latestHeartbeatRecord: (
    replicaId: string,
  ) => GlmFleetReadinessHeartbeatRecord | undefined,
): GlmFleetReadinessProjection =>
  projectGlmFleetReadiness({
    env,
    latestHeartbeatRecord,
    replicaArmings: resolveHydraliskGlm52Reap504bReplicaArmings(env),
  })

const warmStates = new Set<GlmPoolHeartbeatWarmState>([
  'cold',
  'unknown',
  'warm',
])
const probeStatuses = new Set<GlmPoolHeartbeatProbeStatus>([
  'failed',
  'ok',
  'skipped',
])
const keepWarmStatuses = new Set<GlmPoolKeepWarmStatus>([
  'completed',
  'control_plane_only',
  'disabled',
  'failed',
  'skipped_benchmark_reserved',
  'skipped_benchmark_window',
  'skipped_draining',
])
const watchdogStatuses = new Set<GlmPoolWatchdogStatus>([
  'degraded',
  'healthy',
  'skipped',
  'unhealthy',
])

export const isGlmFleetReadinessHeartbeatRecord = (
  value: unknown,
): value is GlmFleetReadinessHeartbeatRecord => {
  const candidate = value as Partial<GlmFleetReadinessHeartbeatRecord>
  const optionalProbeStatusesValid =
    (candidate.healthStatus === undefined ||
      probeStatuses.has(candidate.healthStatus)) &&
    (candidate.modelsStatus === undefined ||
      probeStatuses.has(candidate.modelsStatus)) &&
    (candidate.warmCompletionStatus === undefined ||
      probeStatuses.has(candidate.warmCompletionStatus))
  const optionalKeepWarmStatusValid =
    candidate.keepWarmStatus === undefined ||
    keepWarmStatuses.has(candidate.keepWarmStatus)
  const optionalBooleanFieldsValid =
    (candidate.benchmarkReserved === undefined ||
      typeof candidate.benchmarkReserved === 'boolean') &&
    (candidate.draining === undefined || typeof candidate.draining === 'boolean')
  const optionalNumberFieldsValid =
    (candidate.totalWallClockMs === undefined ||
      (typeof candidate.totalWallClockMs === 'number' &&
        Number.isFinite(candidate.totalWallClockMs) &&
        candidate.totalWallClockMs >= 0)) &&
    (candidate.probeTimeoutMs === undefined ||
      (typeof candidate.probeTimeoutMs === 'number' &&
        Number.isFinite(candidate.probeTimeoutMs) &&
        candidate.probeTimeoutMs >= 0)) &&
    (candidate.breakerConsecutiveFailures === undefined ||
      (typeof candidate.breakerConsecutiveFailures === 'number' &&
        Number.isFinite(candidate.breakerConsecutiveFailures) &&
        candidate.breakerConsecutiveFailures >= 0)) &&
    (candidate.breakerConsecutiveSuccesses === undefined ||
      (typeof candidate.breakerConsecutiveSuccesses === 'number' &&
        Number.isFinite(candidate.breakerConsecutiveSuccesses) &&
        candidate.breakerConsecutiveSuccesses >= 0)) &&
    (candidate.breakerFailureThreshold === undefined ||
      (typeof candidate.breakerFailureThreshold === 'number' &&
        Number.isFinite(candidate.breakerFailureThreshold) &&
        candidate.breakerFailureThreshold >= 0)) &&
    (candidate.breakerReadmitSuccessThreshold === undefined ||
      (typeof candidate.breakerReadmitSuccessThreshold === 'number' &&
        Number.isFinite(candidate.breakerReadmitSuccessThreshold) &&
        candidate.breakerReadmitSuccessThreshold >= 0))
  const optionalRunRefValid =
    candidate.runRef === undefined ||
    (typeof candidate.runRef === 'string' && candidate.runRef.trim() !== '')

  return (
    typeof candidate.observedAt === 'string' &&
    candidate.observedAt.trim() !== '' &&
    typeof candidate.replicaId === 'string' &&
    candidate.replicaId.trim() !== '' &&
    typeof candidate.warmState === 'string' &&
    warmStates.has(candidate.warmState as GlmPoolHeartbeatWarmState) &&
    typeof candidate.watchdogStatus === 'string' &&
    watchdogStatuses.has(candidate.watchdogStatus as GlmPoolWatchdogStatus) &&
    optionalBooleanFieldsValid &&
    optionalKeepWarmStatusValid &&
    optionalNumberFieldsValid &&
    optionalProbeStatusesValid &&
    optionalRunRefValid
  )
}
