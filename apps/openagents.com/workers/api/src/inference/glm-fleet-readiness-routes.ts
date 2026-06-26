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
  observed_at: string | null
  replica_id: string | null
  warm_state: string | null
  watchdog_status: string | null
}>

const heartbeatRecordFromRow = (
  row: GlmFleetReadinessHeartbeatRow,
): GlmFleetReadinessHeartbeatRecord | undefined => {
  const candidate = {
    observedAt: row.observed_at ?? undefined,
    replicaId: row.replica_id ?? undefined,
    warmState: row.warm_state ?? undefined,
    watchdogStatus: row.watchdog_status ?? undefined,
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
          observed_at,
          json_extract(safe_metadata_json, '$.selectedReplicaId') AS replica_id,
          json_extract(safe_metadata_json, '$.replicaWarmState') AS warm_state,
          json_extract(safe_metadata_json, '$.watchdogStatus') AS watchdog_status
        FROM token_usage_events
        WHERE demand_source = 'glm-pool-heartbeat'
          AND model = 'openagents/glm-5.2-reap-504b'
          AND json_extract(safe_metadata_json, '$.heartbeatKind') = 'glm_pool_heartbeat'
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
