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

export type GlmFleetReadinessProjection = Readonly<{
  kind: 'glm_fleet_readiness'
  model: typeof HYDRALISK_GLM_52_REAP_504B_MODEL_ID
  status: GlmFleetReadinessStatus
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

const replicaRefFor = (replicaId: string): string =>
  `${REPLICA_REF_PREFIX}.${replicaId}`

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

export const projectGlmFleetReadiness = (
  input: Readonly<{
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

  return {
    configuredReplicaRefs: replicas.map(replica => replica.replicaRef),
    counts,
    kind: 'glm_fleet_readiness',
    model: HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
    replicas,
    status: fleetStatus(counts),
  }
}

export const projectGlmFleetReadinessForEnv = (
  env: SupplyLaneCredentialEnv,
  latestHeartbeatRecord: (
    replicaId: string,
  ) => GlmFleetReadinessHeartbeatRecord | undefined,
): GlmFleetReadinessProjection =>
  projectGlmFleetReadiness({
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
