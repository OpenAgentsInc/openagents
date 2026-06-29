import {
  epochMillisToIsoTimestamp,
  isoTimestampAfterIso,
  randomUuid,
} from '../runtime-primitives'
import { HYDRALISK_GLM_52_REAP_504B_MODEL_ID } from './pricing'

export const SERVING_RATE_MONITOR_CADENCE_MINUTES = 5
export const SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR = 50_000_000
export const SERVING_RATE_MONITOR_GLM_DOWN_MINUTES = 15

export type ServingRateMonitorAlertKind = 'glm_down' | 'serving_rate_low'
export type ServingRateMonitorStatus =
  | 'disabled'
  | 'glm_down'
  | 'healthy'
  | 'serving_rate_low'

export const SERVING_RATE_LOW_REASON_REF =
  'blocker.public.serving_rate.tokens_per_hour_below_floor'
export const SERVING_RATE_GLM_DOWN_REASON_REF =
  'blocker.public.serving_rate.glm_lane_down'

export type ServingRateMonitorConfig = Readonly<{
  enabled: boolean
  cadenceMinutes: number
  glmDownMinutes: number
  tokenFloorPerHour: number
}>

export type ServingRateMonitorClassificationInput = Readonly<{
  glmDownMinutes: number
  latestHealthyGlmHeartbeatAt: string | null
  nowIso: string
  tokenFloorPerHour: number
  tokensLastHour: number
}>

export type ServingRateMonitorClassification = Readonly<{
  alertKinds: ReadonlyArray<ServingRateMonitorAlertKind>
  glmHealthy: boolean
  latestHealthyGlmHeartbeatAt: string | null
  reasonRefs: ReadonlyArray<string>
  status: ServingRateMonitorStatus
  tokenFloorPerHour: number
  tokensLastHour: number
}>

export type ServingRateMonitorStore = Readonly<{
  hasRecentAlert: (
    classification: ServingRateMonitorAlertKind,
    sinceIso: string,
  ) => Promise<boolean>
  insertAlert: (record: ServingRateMonitorAlertRecord) => Promise<void>
  latestHealthyGlmHeartbeatAt: () => Promise<string | null>
  tokensSince: (sinceIso: string) => Promise<number>
}>

export type ServingRateMonitorAlertRecord = Readonly<{
  id: string
  alertRef: string
  detectedAt: string
  classification: ServingRateMonitorAlertKind
  reasonRef: string
  burnTokensWindow: number
  windowMinutes: number
  stallThresholdTokens: number
  recoveryActions: ReadonlyArray<string>
  createdAt: string
}>

export type ServingRateMonitorTickResult = Readonly<{
  alertRefs: ReadonlyArray<string>
  classification: ServingRateMonitorClassification
  enabled: boolean
  ranAt: string
}>

const nonNegativeInt = (value: number): number => Math.max(0, Math.trunc(value))

const parseBool = (value: unknown, fallback: boolean): boolean => {
  if (typeof value !== 'string' || value.trim() === '') {
    return fallback
  }
  return value.trim().toLowerCase() === 'true'
}

const parsePositiveInt = (value: unknown, fallback: number): number => {
  if (typeof value !== 'string' || value.trim() === '') {
    return fallback
  }
  const parsed = Number.parseInt(value.trim(), 10)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback
}

const isIsoWithinMinutes = (
  timestamp: string | null,
  nowIso: string,
  minutes: number,
): boolean => {
  if (timestamp === null) {
    return false
  }
  const observedMs = Date.parse(timestamp)
  const nowMs = Date.parse(nowIso)
  return (
    Number.isFinite(observedMs) &&
    Number.isFinite(nowMs) &&
    nowMs - observedMs <= minutes * 60_000
  )
}

const alertRefFor = (
  kind: ServingRateMonitorAlertKind,
  nowIso: string,
  id: string,
): string => `serving_rate_alert.${kind}.${nowIso}.${id.slice(0, 8)}`

const shouldRunAt = (
  scheduledTimeMs: number,
  cadenceMinutes: number,
): boolean => {
  const minute = Math.floor(scheduledTimeMs / 60_000)
  return minute % Math.max(1, cadenceMinutes) === 0
}

export const readServingRateMonitorConfig = (
  env: unknown,
): ServingRateMonitorConfig => {
  const record = (env ?? {}) as Record<string, unknown>
  return {
    cadenceMinutes: parsePositiveInt(
      record.SERVING_RATE_MONITOR_CADENCE_MINUTES,
      SERVING_RATE_MONITOR_CADENCE_MINUTES,
    ),
    enabled: parseBool(record.SERVING_RATE_MONITOR_ENABLED, true),
    glmDownMinutes: parsePositiveInt(
      record.SERVING_RATE_MONITOR_GLM_DOWN_MINUTES,
      SERVING_RATE_MONITOR_GLM_DOWN_MINUTES,
    ),
    tokenFloorPerHour: parsePositiveInt(
      record.SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR,
      SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR,
    ),
  }
}

export const classifyServingRateMonitor = (
  input: ServingRateMonitorClassificationInput,
): ServingRateMonitorClassification => {
  const tokensLastHour = nonNegativeInt(input.tokensLastHour)
  const tokenFloorPerHour = Math.max(1, nonNegativeInt(input.tokenFloorPerHour))
  const glmHealthy = isIsoWithinMinutes(
    input.latestHealthyGlmHeartbeatAt,
    input.nowIso,
    input.glmDownMinutes,
  )
  const servingRateLow = tokensLastHour < tokenFloorPerHour
  const alertKinds: ReadonlyArray<ServingRateMonitorAlertKind> = [
    ...(servingRateLow ? (['serving_rate_low'] as const) : []),
    ...(!glmHealthy ? (['glm_down'] as const) : []),
  ]
  const reasonRefs = [
    ...(servingRateLow ? [SERVING_RATE_LOW_REASON_REF] : []),
    ...(!glmHealthy ? [SERVING_RATE_GLM_DOWN_REASON_REF] : []),
  ]
  const status: ServingRateMonitorStatus = !glmHealthy
    ? 'glm_down'
    : servingRateLow
      ? 'serving_rate_low'
      : 'healthy'

  return {
    alertKinds,
    glmHealthy,
    latestHealthyGlmHeartbeatAt: input.latestHealthyGlmHeartbeatAt,
    reasonRefs,
    status,
    tokenFloorPerHour,
    tokensLastHour,
  }
}

export const runServingRateMonitorTick = async (
  input: Readonly<{
    config: ServingRateMonitorConfig
    log?: (line: string, fields: Record<string, unknown>) => void
    makeId: () => string
    nowIso: string
    store: ServingRateMonitorStore
  }>,
): Promise<ServingRateMonitorTickResult> => {
  const log = input.log ?? (() => {})
  if (!input.config.enabled) {
    return {
      alertRefs: [],
      classification: {
        alertKinds: [],
        glmHealthy: false,
        latestHealthyGlmHeartbeatAt: null,
        reasonRefs: [],
        status: 'disabled',
        tokenFloorPerHour: input.config.tokenFloorPerHour,
        tokensLastHour: 0,
      },
      enabled: false,
      ranAt: input.nowIso,
    }
  }

  const oneHourAgoIso = isoTimestampAfterIso(input.nowIso, -60 * 60_000)
  const [tokensLastHour, latestHealthyGlmHeartbeatAt] = await Promise.all([
    input.store.tokensSince(oneHourAgoIso),
    input.store.latestHealthyGlmHeartbeatAt(),
  ])
  const classification = classifyServingRateMonitor({
    glmDownMinutes: input.config.glmDownMinutes,
    latestHealthyGlmHeartbeatAt,
    nowIso: input.nowIso,
    tokenFloorPerHour: input.config.tokenFloorPerHour,
    tokensLastHour,
  })

  const dedupeSinceIso = isoTimestampAfterIso(
    input.nowIso,
    -input.config.cadenceMinutes * 60_000,
  )
  const insertedAlertRefs = (
    await Promise.all(
      classification.alertKinds.map(async kind => {
        if (await input.store.hasRecentAlert(kind, dedupeSinceIso)) {
          return null
        }
        const id = input.makeId()
        const reasonRef =
          kind === 'serving_rate_low'
            ? SERVING_RATE_LOW_REASON_REF
            : SERVING_RATE_GLM_DOWN_REASON_REF
        const alertRef = alertRefFor(kind, input.nowIso, id)
        await input.store.insertAlert({
          alertRef,
          burnTokensWindow: classification.tokensLastHour,
          classification: kind,
          createdAt: input.nowIso,
          detectedAt: input.nowIso,
          id,
          reasonRef,
          recoveryActions: [
            'monitor.serving_rate.read_only',
            `monitor.glm.latest_healthy_at.${latestHealthyGlmHeartbeatAt ?? 'missing'}`,
          ],
          stallThresholdTokens: classification.tokenFloorPerHour,
          windowMinutes: 60,
        })
        log('SERVING-RATE monitor alert', {
          alertRef,
          classification: kind,
          latestHealthyGlmHeartbeatAt: latestHealthyGlmHeartbeatAt ?? 'missing',
          reasonRef,
          tokenFloorPerHour: classification.tokenFloorPerHour,
          tokensLastHour: classification.tokensLastHour,
        })
        return alertRef
      }),
    )
  ).filter((ref): ref is string => ref !== null)

  return {
    alertRefs: insertedAlertRefs,
    classification,
    enabled: true,
    ranAt: input.nowIso,
  }
}

export const makeD1ServingRateMonitorStore = (
  db: D1Database,
): ServingRateMonitorStore => ({
  hasRecentAlert: async (classification, sinceIso) => {
    const row = await db
      .prepare(
        `SELECT alert_ref
           FROM fleet_alerts
          WHERE classification = ?
            AND detected_at >= ?
          ORDER BY detected_at DESC
          LIMIT 1`,
      )
      .bind(classification, sinceIso)
      .first<{ alert_ref: string | null }>()
    return typeof row?.alert_ref === 'string'
  },
  insertAlert: async record => {
    await db
      .prepare(
        `INSERT INTO fleet_alerts
          (id, alert_ref, detected_at, classification, reason_ref,
           burn_tokens_window, window_minutes, stall_threshold_tokens,
           active_assignments, queued_assignments, recovery_actions_json,
           recovered_lease_count, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, 0, ?, 0, ?)`,
      )
      .bind(
        record.id,
        record.alertRef,
        record.detectedAt,
        record.classification,
        record.reasonRef,
        record.burnTokensWindow,
        record.windowMinutes,
        record.stallThresholdTokens,
        JSON.stringify(record.recoveryActions),
        record.createdAt,
      )
      .run()
  },
  latestHealthyGlmHeartbeatAt: async () => {
    const row = await db
      .prepare(
        `SELECT observed_at
           FROM token_usage_events
          WHERE model = ?
            AND (
              (
                demand_source = 'glm-pool-heartbeat'
                AND json_extract(safe_metadata_json, '$.heartbeatKind') = 'glm_pool_heartbeat'
                AND (
                  json_extract(safe_metadata_json, '$.watchdogStatus') = 'healthy'
                  OR json_extract(safe_metadata_json, '$.healthStatus') IN ('ok', 'ready', 'healthy')
                  OR json_extract(safe_metadata_json, '$.modelsStatus') IN ('ok', 'ready', 'healthy')
                )
              )
              OR (
                demand_source = 'heartbeat'
                AND provider = 'hydralisk-vllm-glm-5p2-reap-504b'
                AND total_tokens > 0
              )
            )
          ORDER BY observed_at DESC
          LIMIT 1`,
      )
      .bind(HYDRALISK_GLM_52_REAP_504B_MODEL_ID)
      .first<{ observed_at: string | null }>()
    return typeof row?.observed_at === 'string' ? row.observed_at : null
  },
  tokensSince: async sinceIso => {
    const row = await db
      .prepare(
        `SELECT COALESCE(SUM(total_tokens), 0) AS tokens
           FROM token_usage_events
          WHERE observed_at >= ?`,
      )
      .bind(sinceIso)
      .first<{ tokens: number | null }>()
    return nonNegativeInt(row?.tokens ?? 0)
  },
})

export const runServingRateMonitorScheduled = async (
  db: D1Database,
  env: unknown,
  input: Readonly<{ scheduledTimeMs: number }>,
  log: (line: string, fields: Record<string, unknown>) => void,
): Promise<ServingRateMonitorTickResult | null> => {
  const config = readServingRateMonitorConfig(env)
  if (!shouldRunAt(input.scheduledTimeMs, config.cadenceMinutes)) {
    return null
  }
  const nowIso = epochMillisToIsoTimestamp(input.scheduledTimeMs)
  try {
    return await runServingRateMonitorTick({
      config,
      log,
      makeId: () => randomUuid(),
      nowIso,
      store: makeD1ServingRateMonitorStore(db),
    })
  } catch (error) {
    log('ServingRateMonitor tick failed', {
      errorName: error instanceof Error ? error.name : 'unknown',
    })
    return null
  }
}
