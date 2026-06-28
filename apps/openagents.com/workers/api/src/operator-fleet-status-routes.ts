import { jsonResponse } from '@openagentsinc/sync-worker'

import {
  projectGlmFleetReadinessForEnv,
  summarizeGlmFleetReadinessForOperators,
} from './inference/glm-fleet-readiness'
import type { SupplyLaneCredentialEnv } from './inference/model-serving-policy'
import { openAgentsDatabase } from './runtime'

type FleetStatusEnv = SupplyLaneCredentialEnv &
  Readonly<{
    OPENAGENTS_DB: D1Database
    SYNC_ROOM?: DurableObjectNamespace | undefined
    KHALA_INFERENCE_ANALYTICS?: AnalyticsEngineDataset | undefined
  }>

type Row = Record<string, unknown>

const cacheSeconds = 10
const activeAssignmentStates = [
  'offered',
  'accepted',
  'running',
  'proof_submitted',
  'blocked',
]

const numberFrom = (value: unknown): number =>
  typeof value === 'number' && Number.isFinite(value)
    ? value
    : typeof value === 'string' && value.trim() !== ''
      ? Number(value)
      : 0

const stringFrom = (value: unknown): string | null =>
  typeof value === 'string' && value.length > 0 ? value : null

const jsonArrayFrom = (value: unknown): ReadonlyArray<unknown> => {
  if (Array.isArray(value)) return value
  if (typeof value !== 'string' || value.trim() === '') return []
  try {
    const parsed = JSON.parse(value) as unknown
    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
}

const jsonRecordFrom = (value: unknown): Row => {
  if (typeof value !== 'string' || value.trim() === '') return {}
  try {
    const parsed = JSON.parse(value) as unknown
    return parsed !== null && typeof parsed === 'object' && !Array.isArray(parsed)
      ? (parsed as Row)
      : {}
  } catch {
    return {}
  }
}

const elapsedSeconds = (startedAt: string | null, nowMs: number): number | null => {
  if (startedAt === null) return null
  const startedMs = Date.parse(startedAt)
  if (!Number.isFinite(startedMs)) return null
  return Math.max(0, Math.floor((nowMs - startedMs) / 1000))
}

const safeQuery = async <A>(
  read: () => Promise<A>,
  fallback: A,
): Promise<A> => {
  try {
    return await read()
  } catch {
    return fallback
  }
}

const activeStateSql = activeAssignmentStates.map(() => '?').join(', ')

const accountRefFromAssignment = (row: Row): string => {
  const coding = jsonRecordFrom(row.coding_assignment_json)
  const workspace = jsonRecordFrom(coding.workspace)
  const accountRef =
    stringFrom(coding.accountRef) ??
    stringFrom(coding.account_ref) ??
    stringFrom(workspace.accountRef) ??
    stringFrom(workspace.account_ref) ??
    'account.public.unattributed'
  return accountRef
}

const parseCapacityRefs = (
  rows: ReadonlyArray<Row>,
): ReadonlyArray<{
  pylonRef: string
  available: number
  busy: number
  queued: number
  ready: number
}> =>
  rows.map(row => {
    const refs = jsonArrayFrom(row.latest_capacity_refs_json)
      .filter((value): value is string => typeof value === 'string')
    const readCount = (suffix: string): number => {
      const ref = refs.find(item => item.startsWith(suffix))
      if (ref === undefined) return 0
      const [, count] = ref.split('=')
      return Number.isFinite(Number(count)) ? Number(count) : 1
    }
    return {
      available: readCount('capacity.coding.codex.available'),
      busy: readCount('load.coding.codex.busy'),
      pylonRef: stringFrom(row.pylon_ref) ?? 'pylon.public.unknown',
      queued: readCount('load.coding.codex.queued'),
      ready: readCount('capacity.coding.codex.ready'),
    }
  })

export const readOperatorFleetStatusSnapshot = async (
  env: FleetStatusEnv,
  nowMs = Date.now(),
) => {
  const db = openAgentsDatabase(env)
  const generatedAt = new Date(nowMs).toISOString()
  const tenMinutesAgo = new Date(nowMs - 10 * 60 * 1000).toISOString()
  const oneHourAgo = new Date(nowMs - 60 * 60 * 1000).toISOString()

  const [pace10m, pace60m, pylonRows, assignmentRows, alertRows, loopRows] =
    await Promise.all([
      safeQuery(
        () =>
          db
            .prepare(
              `SELECT COALESCE(SUM(total_tokens), 0) AS total_tokens
                 FROM token_usage_events
                WHERE observed_at >= ?
                  AND usage_truth = 'exact'
                  AND demand_kind = 'own_capacity'
                  AND demand_source = 'khala_coding_delegation'`,
            )
            .bind(tenMinutesAgo)
            .first<Row>(),
        { total_tokens: 0 },
      ),
      safeQuery(
        () =>
          db
            .prepare(
              `SELECT COALESCE(SUM(total_tokens), 0) AS total_tokens
                 FROM token_usage_events
                WHERE observed_at >= ?
                  AND usage_truth = 'exact'
                  AND demand_kind = 'own_capacity'
                  AND demand_source = 'khala_coding_delegation'`,
            )
            .bind(oneHourAgo)
            .first<Row>(),
        { total_tokens: 0 },
      ),
      safeQuery(
        async () =>
          (
            await db
              .prepare(
                `SELECT pylon_ref, latest_capacity_refs_json, latest_heartbeat_at
                   FROM pylon_api_registrations
                  WHERE archived_at IS NULL
                    AND status = 'online'
                  ORDER BY updated_at DESC
                  LIMIT 200`,
              )
              .all<Row>()
          ).results ?? [],
        [],
      ),
      safeQuery(
        async () =>
          (
            await db
              .prepare(
                `SELECT assignment_ref, pylon_ref, state, created_at, updated_at,
                        lease_expires_at, coding_assignment_json
                   FROM pylon_api_assignments
                  WHERE archived_at IS NULL
                    AND state IN (${activeStateSql})
                    AND lease_expires_at > ?
                  ORDER BY updated_at DESC
                  LIMIT 50`,
              )
              .bind(...activeAssignmentStates, generatedAt)
              .all<Row>()
          ).results ?? [],
        [],
      ),
      safeQuery(
        async () =>
          (
            await db
              .prepare(
                `SELECT alert_ref, detected_at, classification, reason_ref,
                        active_assignments, queued_assignments, recovered_lease_count
                   FROM fleet_alerts
                  ORDER BY detected_at DESC
                  LIMIT 10`,
              )
              .all<Row>()
          ).results ?? [],
        [],
      ),
      safeQuery(
        async () =>
          (
            await db
              .prepare(
                `SELECT record_ref, public_projection_json, state, updated_at
                   FROM artanis_loop_records
                  ORDER BY updated_at DESC
                  LIMIT 3`,
              )
              .all<Row>()
          ).results ?? [],
        [],
      ),
    ])

  const capacities = parseCapacityRefs(pylonRows)
  const activeSlots = assignmentRows.length
  const availableSlots = capacities.reduce((sum, row) => sum + row.available, 0)
  const busySlots = capacities.reduce((sum, row) => sum + row.busy, 0)
  const readySlots = capacities.reduce((sum, row) => sum + row.ready, 0)
  const queuedSlots =
    capacities.reduce((sum, row) => sum + row.queued, 0) +
    assignmentRows.filter(row => row.state === 'offered').length
  const totalSlots = activeSlots + availableSlots

  const accountSpread = [...new Set(assignmentRows.map(accountRefFromAssignment))]
    .sort()
    .map(accountRef => ({
      accountRef,
      inFlight: assignmentRows.filter(
        row => accountRefFromAssignment(row) === accountRef,
      ).length,
    }))

  const recentAlert = alertRows[0]
  const recentAlertAt = stringFrom(recentAlert?.detected_at)
  const recentAlertFresh =
    recentAlertAt !== null && nowMs - Date.parse(recentAlertAt) <= 10 * 60 * 1000
  const watchdogState =
    recentAlertFresh && recentAlert?.classification === 'stalled'
      ? 'STALLED'
      : recentAlertFresh && numberFrom(recentAlert?.recovered_lease_count) > 0
        ? 'RECOVERING'
        : 'HEALTHY'

  const loopHealth =
    loopRows.some(row => row.state === 'running') || loopRows.length > 0
      ? 'active'
      : 'unknown'

  const glmProjection = projectGlmFleetReadinessForEnv(env, () => undefined)
  const glmReadout = summarizeGlmFleetReadinessForOperators(glmProjection)

  const tokens10m = numberFrom(pace10m?.total_tokens)
  const tokens60m = numberFrom(pace60m?.total_tokens)
  const tokensPerMinute10m = tokens10m / 10
  const floorTokensPerMinute = 1

  return {
    brain: {
      currentGoals: loopRows
        .map(row => {
          const projection = jsonRecordFrom(row.public_projection_json)
          return stringFrom(projection.loopRef) ?? stringFrom(row.record_ref)
        })
        .filter((value): value is string => value !== null),
      last3Decisions: loopRows.map(row => ({
        recordRef: stringFrom(row.record_ref) ?? 'artanis.loop.unknown',
        state: stringFrom(row.state) ?? 'unknown',
        updatedAt: stringFrom(row.updated_at),
      })),
      loopHealth,
    },
    cache: {
      maxAgeSeconds: cacheSeconds,
      strategy: 'worker_http_cache',
    },
    fleet: {
      activeSlots,
      accountSpread,
      availableSlots,
      busySlots,
      inFlightAssignments: assignmentRows.map(row => ({
        assignmentRef:
          stringFrom(row.assignment_ref) ?? 'assignment.public.unknown',
        elapsedSeconds: elapsedSeconds(
          stringFrom(row.created_at) ?? stringFrom(row.updated_at),
          nowMs,
        ),
        pylonRef: stringFrom(row.pylon_ref) ?? 'pylon.public.unknown',
        state: stringFrom(row.state) ?? 'unknown',
      })),
      readySlots,
      totalSlots,
    },
    generatedAt,
    glm: {
      acceptanceStatus: glmReadout.acceptanceStatus,
      counts: glmReadout.counts,
      readiness: glmReadout.servingStatus,
      replicaCounts: {
        ready: glmReadout.counts.readyReplicaCount,
        total: glmReadout.counts.totalReplicaCount,
        warm: glmReadout.counts.warmReplicaCount,
      },
    },
    ok: true,
    pace: {
      floorTokensPerMinute,
      paceToFloorTokensPerMinute: Math.max(
        0,
        floorTokensPerMinute - tokensPerMinute10m,
      ),
      tokensLast10m: tokens10m,
      tokensLast60m: tokens60m,
      tokensPerMinute10m,
      tokensPerHour60m: tokens60m,
    },
    substrates: {
      d1: 'queried',
      durableObjects:
        env.SYNC_ROOM === undefined ? 'not_configured' : 'binding_configured',
      workersAnalyticsEngine:
        env.KHALA_INFERENCE_ANALYTICS === undefined
          ? 'not_configured'
          : 'binding_configured',
    },
    watchdog: {
      activeLeases: activeSlots,
      activeAlerts: alertRows.length,
      lastCronHeartbeatAt: recentAlertAt,
      queuedAssignments: queuedSlots,
      recentAlerts: alertRows.map(row => ({
        alertRef: stringFrom(row.alert_ref) ?? 'alert.public.unknown',
        classification: stringFrom(row.classification) ?? 'unknown',
        detectedAt: stringFrom(row.detected_at),
        reasonRef: stringFrom(row.reason_ref) ?? 'reason.public.unknown',
      })),
      state: watchdogState,
    },
  }
}

export const handleOperatorFleetStatusApi = async (
  request: Request,
  env: FleetStatusEnv,
): Promise<Response> => {
  if (request.method !== 'GET') {
    return jsonResponse(
      { error: 'method_not_allowed' },
      {
        headers: {
          allow: 'GET',
          'cache-control': 'no-store',
        },
        status: 405,
      },
    )
  }

  const headers = new Headers({
    'cache-control': `public, max-age=${cacheSeconds}`,
  })
  return jsonResponse(await readOperatorFleetStatusSnapshot(env), { headers })
}
