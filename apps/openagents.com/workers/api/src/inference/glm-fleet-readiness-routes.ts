import { Effect } from 'effect'

import { noStoreJsonResponse } from '../http/responses'
import { glmPoolHeartbeatLatestRecordOracle } from './glm-pool-heartbeat'
import {
  type GlmFleetReadinessHeartbeatRecord,
  isGlmFleetReadinessHeartbeatRecord,
  projectGlmFleetReadinessForEnv,
} from './glm-fleet-readiness'
import type { SupplyLaneCredentialEnv } from './model-serving-policy'

export type GlmFleetReadinessDeps = Readonly<{
  db?: D1Database | undefined
  enabled: boolean
  env: SupplyLaneCredentialEnv
  latestHeartbeatRecord?: (
    replicaId: string,
  ) => GlmFleetReadinessHeartbeatRecord | undefined
  readPersistedHeartbeatRecords?: (() => Promise<
    ReadonlyArray<GlmFleetReadinessHeartbeatRecord>
  >) | undefined
}>

type GlmFleetReadinessHeartbeatRow = Readonly<{
  benchmark_reserved: number | string | null
  breaker_consecutive_failures: number | null
  breaker_consecutive_successes: number | null
  breaker_failure_threshold: number | null
  breaker_readmit_success_threshold: number | null
  demand_source: string | null
  draining: number | string | null
  health_status: string | null
  heartbeat_kind: string | null
  heartbeat_run_ref: string | null
  keep_warm_status: string | null
  models_status: string | null
  observed_at: string | null
  probe_timeout_ms: number | null
  provider: string | null
  replica_id: string | null
  total_wall_clock_ms: number | null
  total_tokens: number | null
  warm_completion_status: string | null
  warm_state: string | null
  watchdog_status: string | null
}>

const optionalNumber = (value: number | null): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) ? value : undefined

const optionalBoolean = (
  value: number | string | null,
): boolean | undefined => {
  if (typeof value === 'number') {
    return value === 1 ? true : value === 0 ? false : undefined
  }
  if (typeof value === 'string') {
    const normalized = value.trim().toLowerCase()
    if (normalized === 'true' || normalized === '1') {
      return true
    }
    if (normalized === 'false' || normalized === '0') {
      return false
    }
  }
  return undefined
}

const heartbeatRecordFromRow = (
  row: GlmFleetReadinessHeartbeatRow,
): GlmFleetReadinessHeartbeatRecord | undefined => {
  const routedCompletionHeartbeat =
    row.demand_source === 'heartbeat' &&
    row.provider === 'hydralisk-vllm-glm-5p2-reap-504b' &&
    typeof row.total_tokens === 'number' &&
    row.total_tokens > 0
  const canonicalPoolHeartbeat =
    row.demand_source === 'glm-pool-heartbeat' &&
    row.heartbeat_kind === 'glm_pool_heartbeat'
  const benchmarkReserved = optionalBoolean(row.benchmark_reserved)
  const breakerConsecutiveFailures = optionalNumber(
    row.breaker_consecutive_failures,
  )
  const breakerConsecutiveSuccesses = optionalNumber(
    row.breaker_consecutive_successes,
  )
  const breakerFailureThreshold = optionalNumber(row.breaker_failure_threshold)
  const breakerReadmitSuccessThreshold = optionalNumber(
    row.breaker_readmit_success_threshold,
  )
  const draining = optionalBoolean(row.draining)
  const probeTimeoutMs = optionalNumber(row.probe_timeout_ms)
  const totalWallClockMs = optionalNumber(row.total_wall_clock_ms)
  const candidate = {
    ...(benchmarkReserved === undefined ? {} : { benchmarkReserved }),
    ...(breakerConsecutiveFailures === undefined
      ? {}
      : { breakerConsecutiveFailures }),
    ...(breakerConsecutiveSuccesses === undefined
      ? {}
      : { breakerConsecutiveSuccesses }),
    ...(breakerFailureThreshold === undefined
      ? {}
      : { breakerFailureThreshold }),
    ...(breakerReadmitSuccessThreshold === undefined
      ? {}
      : { breakerReadmitSuccessThreshold }),
    ...(draining === undefined ? {} : { draining }),
    ...(row.health_status === null ? {} : { healthStatus: row.health_status }),
    ...(row.keep_warm_status === null
      ? {}
      : { keepWarmStatus: row.keep_warm_status }),
    ...(row.models_status === null ? {} : { modelsStatus: row.models_status }),
    observedAt: row.observed_at ?? undefined,
    ...(probeTimeoutMs === undefined ? {} : { probeTimeoutMs }),
    replicaId: row.replica_id ?? undefined,
    ...(row.heartbeat_run_ref === null ? {} : { runRef: row.heartbeat_run_ref }),
    ...(totalWallClockMs === undefined ? {} : { totalWallClockMs }),
    ...(row.warm_completion_status === null
      ? {}
      : { warmCompletionStatus: row.warm_completion_status }),
    warmState: row.warm_state ?? undefined,
    watchdogStatus:
      row.watchdog_status ??
      (routedCompletionHeartbeat && !canonicalPoolHeartbeat
        ? 'healthy'
        : undefined),
  }
  return isGlmFleetReadinessHeartbeatRecord(candidate) ? candidate : undefined
}

export const readPersistedGlmFleetReadinessHeartbeatRecords = async (
  db: D1Database,
): Promise<ReadonlyArray<GlmFleetReadinessHeartbeatRecord>> => {
  const response = await db
    .prepare(
      `
        SELECT
          json_extract(safe_metadata_json, '$.benchmarkReserved') AS benchmark_reserved,
          json_extract(safe_metadata_json, '$.breakerConsecutiveFailures') AS breaker_consecutive_failures,
          json_extract(safe_metadata_json, '$.breakerConsecutiveSuccesses') AS breaker_consecutive_successes,
          json_extract(safe_metadata_json, '$.breakerFailureThreshold') AS breaker_failure_threshold,
          json_extract(safe_metadata_json, '$.breakerReadmitSuccessThreshold') AS breaker_readmit_success_threshold,
          demand_source,
          json_extract(safe_metadata_json, '$.draining') AS draining,
          json_extract(safe_metadata_json, '$.healthStatus') AS health_status,
          json_extract(safe_metadata_json, '$.heartbeatKind') AS heartbeat_kind,
          json_extract(safe_metadata_json, '$.heartbeatRunRef') AS heartbeat_run_ref,
          json_extract(safe_metadata_json, '$.keepWarmStatus') AS keep_warm_status,
          json_extract(safe_metadata_json, '$.modelsStatus') AS models_status,
          observed_at,
          json_extract(safe_metadata_json, '$.probeTimeoutMs') AS probe_timeout_ms,
          provider,
          json_extract(safe_metadata_json, '$.selectedReplicaId') AS replica_id,
          json_extract(safe_metadata_json, '$.totalWallClockMs') AS total_wall_clock_ms,
          total_tokens,
          json_extract(safe_metadata_json, '$.warmCompletionStatus') AS warm_completion_status,
          json_extract(safe_metadata_json, '$.replicaWarmState') AS warm_state,
          json_extract(safe_metadata_json, '$.watchdogStatus') AS watchdog_status
        FROM token_usage_events
        WHERE model = 'openagents/glm-5.2-reap-504b'
          AND (
            (
              demand_source = 'glm-pool-heartbeat'
              AND json_extract(safe_metadata_json, '$.heartbeatKind') = 'glm_pool_heartbeat'
            )
            OR (
              demand_source = 'heartbeat'
              AND provider = 'hydralisk-vllm-glm-5p2-reap-504b'
              AND total_tokens > 0
              AND json_extract(safe_metadata_json, '$.selectedReplicaId') IS NOT NULL
            )
          )
        ORDER BY observed_at DESC
        LIMIT 100
      `,
    )
    .all<GlmFleetReadinessHeartbeatRow>()

  const latestByReplica = new Map<string, GlmFleetReadinessHeartbeatRecord>()
  for (const row of response.results ?? []) {
    const record = heartbeatRecordFromRow(row)
    if (record !== undefined && !latestByReplica.has(record.replicaId)) {
      latestByReplica.set(record.replicaId, record)
    }
  }
  return [...latestByReplica.values()]
}

const persistedHeartbeatRecords = (
  deps: GlmFleetReadinessDeps,
): Effect.Effect<ReadonlyArray<GlmFleetReadinessHeartbeatRecord>, unknown> => {
  if (deps.readPersistedHeartbeatRecords !== undefined) {
    return Effect.tryPromise({
      catch: () => 'glm_fleet_readiness_persisted_read_failed' as const,
      try: deps.readPersistedHeartbeatRecords,
    })
  }
  if (deps.db === undefined) {
    return Effect.succeed([])
  }
  return Effect.tryPromise({
    catch: () => 'glm_fleet_readiness_persisted_read_failed' as const,
    try: () => readPersistedGlmFleetReadinessHeartbeatRecords(deps.db!),
  })
}

const readinessResponse = (
  deps: GlmFleetReadinessDeps,
  persistedRecords: ReadonlyArray<GlmFleetReadinessHeartbeatRecord>,
) => {
  const latestHeartbeatRecord =
    deps.latestHeartbeatRecord ?? glmPoolHeartbeatLatestRecordOracle
  const persistedByReplica = new Map(
    persistedRecords.map(record => [record.replicaId, record] as const),
  )
  return noStoreJsonResponse(
    projectGlmFleetReadinessForEnv(
      deps.env,
      replicaId =>
        latestHeartbeatRecord(replicaId) ?? persistedByReplica.get(replicaId),
    ),
  )
}

export const handleGlmFleetReadiness = (
  request: Request,
  deps: GlmFleetReadinessDeps,
) =>
  Effect.gen(function* () {
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'inference_gateway_disabled' },
        { status: 404 },
      )
    }

    if (request.method !== 'GET') {
      return noStoreJsonResponse(
        { error: 'method_not_allowed' },
        { status: 405 },
      )
    }

    const persistedRecords = yield* persistedHeartbeatRecords(deps).pipe(
      Effect.catch(() => Effect.succeed([])),
    )
    return readinessResponse(deps, persistedRecords)
  })
