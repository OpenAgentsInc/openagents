import type { GlmPoolHeartbeatReplicaRecord } from './glm-pool-heartbeat'
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
  replicaRef: string
  status: GlmFleetReplicaReadinessStatus
}>

export type GlmFleetReadinessCounts = Readonly<{
  totalReplicaCount: number
  warmReplicaCount: number
  readyReplicaCount: number
  reclaimedReplicaCount: number
  disabledReplicaCount: number
  unavailableReplicaCount: number
}>

export type GlmFleetReadinessProjection = Readonly<{
  kind: 'glm_fleet_readiness'
  model: typeof HYDRALISK_GLM_52_REAP_504B_MODEL_ID
  status: GlmFleetReadinessStatus
  configuredReplicaRefs: ReadonlyArray<string>
  replicas: ReadonlyArray<GlmFleetReadinessReplica>
  counts: GlmFleetReadinessCounts
}>

const REPLICA_REF_PREFIX = 'replica.hydralisk.glm_52_reap_504b'
const INTERNAL_CONFIGURATION_REPLICA_ID = 'configuration'

const replicaRefFor = (replicaId: string): string =>
  `${REPLICA_REF_PREFIX}.${replicaId}`

const replicaStatus = (
  arming: HydraliskGlm52ReplicaArming,
  heartbeat: GlmPoolHeartbeatReplicaRecord | undefined,
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
  disabledReplicaCount: 0,
  readyReplicaCount: 0,
  reclaimedReplicaCount: 0,
  totalReplicaCount: 0,
  unavailableReplicaCount: 0,
  warmReplicaCount: 0,
})

const countReplicas = (
  replicas: ReadonlyArray<GlmFleetReadinessReplica>,
): GlmFleetReadinessCounts =>
  replicas.reduce((counts, replica) => {
    if (replica.status === 'disabled') {
      return {
        ...counts,
        disabledReplicaCount: counts.disabledReplicaCount + 1,
        totalReplicaCount: counts.totalReplicaCount + 1,
      }
    }
    if (replica.status === 'ready') {
      return {
        ...counts,
        readyReplicaCount: counts.readyReplicaCount + 1,
        totalReplicaCount: counts.totalReplicaCount + 1,
      }
    }
    if (replica.status === 'reclaimed') {
      return {
        ...counts,
        reclaimedReplicaCount: counts.reclaimedReplicaCount + 1,
        totalReplicaCount: counts.totalReplicaCount + 1,
      }
    }
    if (replica.status === 'unavailable') {
      return {
        ...counts,
        totalReplicaCount: counts.totalReplicaCount + 1,
        unavailableReplicaCount: counts.unavailableReplicaCount + 1,
      }
    }
    return {
      ...counts,
      totalReplicaCount: counts.totalReplicaCount + 1,
      warmReplicaCount: counts.warmReplicaCount + 1,
    }
  }, zeroCounts())

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
    ) => GlmPoolHeartbeatReplicaRecord | undefined
  }>,
): GlmFleetReadinessProjection => {
  const replicas = input.replicaArmings
    .filter(arming => arming.replicaId !== INTERNAL_CONFIGURATION_REPLICA_ID)
    .map((arming): GlmFleetReadinessReplica => {
      const heartbeat = input.latestHeartbeatRecord(arming.replicaId)
      return {
        replicaRef: replicaRefFor(arming.replicaId),
        status: replicaStatus(arming, heartbeat),
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
  ) => GlmPoolHeartbeatReplicaRecord | undefined,
): GlmFleetReadinessProjection =>
  projectGlmFleetReadiness({
    latestHeartbeatRecord,
    replicaArmings: resolveHydraliskGlm52Reap504bReplicaArmings(env),
  })
