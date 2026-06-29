import { jsonResponse } from '@openagentsinc/sync-worker'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonStringArray } from './json-boundary'
import { openAgentsDatabase } from './runtime'
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
} from './runtime-primitives'

export const OPERATOR_FLEET_STATUS_PATH = '/api/operator/fleet/status'
export const OPERATOR_FLEET_STATE_PATH = '/api/operator/fleet/state'
const CACHE_TTL_MILLIS = 10_000
const SERVING_RATE_MONITOR_WINDOW_MINUTES = 60
const SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR = 50_000_000
const SERVING_RATE_MONITOR_GLM_DOWN_MINUTES = 15

type OperatorFleetStatusEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type OperatorFleetStatusDependencies<Bindings extends OperatorFleetStatusEnv> =
  Readonly<{
    authenticateAgentToken?: (
      request: Request,
      env: Bindings,
    ) => Promise<{ userId: string } | undefined>
    currentIsoTimestamp?: () => string
    requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  }>

type D1Rows<T> = Readonly<{ results?: ReadonlyArray<T> }>

type PylonRegistrationRow = Readonly<{
  pylon_ref: string
  owner_agent_user_id: string
  latest_heartbeat_at: string | null
  latest_capacity_refs_json: string | null
  latest_load_refs_json: string | null
  capability_refs_json: string | null
  status: string | null
}>

type AssignmentRow = Readonly<{
  assignment_ref: string
  pylon_ref: string
  job_kind: string
  state: string
  created_at: string
  updated_at: string
  lease_expires_at: string
}>

type AlertRow = Readonly<{
  alert_ref: string
  classification: string
  detected_at: string
  reason_ref: string
  active_assignments: number
  queued_assignments: number
}>

type ProviderAccountRow = Readonly<{
  provider_account_ref: string
  provider: string
  status: string | null
  health: string | null
  cooldown_until: string | null
  recent_failure_class: string | null
  reauth_required_reason: string | null
  low_credit_flag: number | null
  lease_limit: number | null
}>

type TokenTodayRow = Readonly<{ tokens_today: number | null }>
type TokenYesterdayRow = Readonly<{ tokens_yesterday: number | null }>
type TokenWindowRow = Readonly<{ tokens_window: number | null }>
type TokenHourRow = Readonly<{ tokens_hour: number | null }>
type BrainRow = Readonly<{
  memory_ref: string
  created_at: string
  note_category: string | null
}>

type GlmHeartbeatRow = Readonly<{
  replica_id: string
  health_status: string | null
  observed_at: string | null
}>

type CacheEntry = Readonly<{
  expiresAt: number
  generatedAt: string
  response: unknown
}>

const snapshotCache = new Map<string, CacheEntry>()

export const clearOperatorFleetStatusCacheForTests = (): void => {
  snapshotCache.clear()
}

const metricFromRefs = (
  refs: ReadonlyArray<string>,
  prefix: string,
): number => {
  const ref = refs.find(item => item.startsWith(prefix))
  const raw = ref?.slice(prefix.length)
  const value = raw === undefined ? Number.NaN : Number.parseInt(raw, 10)
  return Number.isFinite(value) ? Math.max(0, value) : 0
}

const millisBetween = (startIso: string, endIso: string): number => {
  const start = Date.parse(startIso)
  const end = Date.parse(endIso)
  return Number.isFinite(start) && Number.isFinite(end)
    ? Math.max(0, end - start)
    : 0
}

const isoAgeMs = (timestampIso: string | null, nowIso: string): number | null => {
  if (timestampIso === null) {
    return null
  }
  const timestampMs = Date.parse(timestampIso)
  const nowMs = Date.parse(nowIso)
  if (!Number.isFinite(timestampMs) || !Number.isFinite(nowMs)) {
    return null
  }
  return Math.max(0, nowMs - timestampMs)
}

const isHealthyGlmStatus = (status: string | null): boolean =>
  status === 'ok' || status === 'ready' || status === 'healthy'

const earlierIso = (left: string | null, right: string): string =>
  left === null || right < left ? right : left

const laterIso = (left: string | null, right: string): string =>
  left === null || right > left ? right : left

const safeAll = async <T>(
  db: D1Database,
  sql: string,
  ...bindings: ReadonlyArray<unknown>
): Promise<ReadonlyArray<T>> => {
  try {
    const statement = db.prepare(sql)
    const query =
      bindings.length === 0 ? statement : statement.bind(...bindings)
    const result = await query.all<T>() as D1Rows<T>
    return result.results ?? []
  } catch {
    return []
  }
}

const safeFirst = async <T>(
  db: D1Database,
  sql: string,
  ...bindings: ReadonlyArray<unknown>
): Promise<T | null> => {
  try {
    const statement = db.prepare(sql)
    const query =
      bindings.length === 0 ? statement : statement.bind(...bindings)
    return await query.first<T>()
  } catch {
    return null
  }
}

const buildFleetStatusSnapshot = async (
  db: D1Database,
  nowIso: string,
  scope: OperatorFleetReadScope,
): Promise<unknown> => {
  const registrationOwnerClause =
    scope.kind === 'agent' ? 'AND owner_agent_user_id = ?' : ''
  const assignmentOwnerClause =
    scope.kind === 'agent'
      ? `AND pylon_ref IN (
           SELECT pylon_ref
             FROM pylon_api_registrations
            WHERE archived_at IS NULL
              AND owner_agent_user_id = ?
         )`
      : ''
  const accountOwnerClause = scope.kind === 'agent' ? 'AND user_id = ?' : ''
  const ownerBindings = scope.kind === 'agent' ? [scope.userId] : []
  const [
    registrations,
    assignments,
    providerAccounts,
    latestAlert,
    today,
    yesterday,
    window,
    hour,
    brainRows,
    glmRows,
  ] = await Promise.all([
    safeAll<PylonRegistrationRow>(
      db,
      `SELECT pylon_ref, owner_agent_user_id, latest_heartbeat_at,
              latest_capacity_refs_json, latest_load_refs_json,
              capability_refs_json, status
         FROM pylon_api_registrations
        WHERE archived_at IS NULL
          ${registrationOwnerClause}
        ORDER BY updated_at DESC
        LIMIT 100`,
      ...ownerBindings,
    ),
    safeAll<AssignmentRow>(
      db,
      `SELECT assignment_ref, pylon_ref, job_kind, state, created_at, updated_at,
              lease_expires_at
         FROM pylon_api_assignments
        WHERE archived_at IS NULL
          ${assignmentOwnerClause}
          AND state IN ('offered','accepted','running','proof_submitted')
        ORDER BY updated_at DESC
        LIMIT 50`,
      ...ownerBindings,
    ),
    safeAll<ProviderAccountRow>(
      db,
      `SELECT provider_account_ref, provider, status, health, cooldown_until,
              recent_failure_class, reauth_required_reason, low_credit_flag,
              lease_limit
         FROM provider_accounts
        WHERE deleted_at IS NULL
          AND provider IN ('chatgpt_codex', 'claude')
          ${accountOwnerClause}
        ORDER BY
          CASE provider WHEN 'chatgpt_codex' THEN 0 ELSE 1 END,
          provider_account_ref ASC
        LIMIT 100`,
      ...ownerBindings,
    ),
    safeFirst<AlertRow>(
      db,
      `SELECT alert_ref, classification, detected_at, reason_ref,
              active_assignments, queued_assignments
         FROM fleet_alerts
        ORDER BY detected_at DESC
        LIMIT 1`,
    ),
    safeFirst<TokenTodayRow>(
      db,
      `SELECT COALESCE(SUM(total_tokens), 0) AS tokens_today
         FROM token_usage_events
        WHERE observed_at >= datetime('now', 'start of day')`,
    ),
    safeFirst<TokenYesterdayRow>(
      db,
      `SELECT COALESCE(SUM(total_tokens), 0) AS tokens_yesterday
         FROM token_usage_events
        WHERE observed_at >= datetime('now', 'start of day', '-1 day')
          AND observed_at < datetime('now', 'start of day')`,
    ),
    safeFirst<TokenWindowRow>(
      db,
      `SELECT COALESCE(SUM(total_tokens), 0) AS tokens_window
         FROM token_usage_events
        WHERE observed_at >= datetime('now', '-10 minutes')`,
    ),
    safeFirst<TokenHourRow>(
      db,
      `SELECT COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0)
              AS tokens_hour
         FROM token_usage_events
        WHERE observed_at >= ?`,
      isoTimestampAfterIso(nowIso, -SERVING_RATE_MONITOR_WINDOW_MINUTES * 60_000),
    ),
    safeAll<BrainRow>(
      db,
      `SELECT memory_ref, created_at, note_category
         FROM artanis_owner_memory
        WHERE kind = 'note'
          AND note_category = 'decision'
        ORDER BY created_at DESC
        LIMIT 3`,
    ),
    safeAll<GlmHeartbeatRow>(
      db,
      `SELECT
          json_extract(safe_metadata_json, '$.selectedReplicaId') AS replica_id,
          json_extract(safe_metadata_json, '$.healthStatus') AS health_status,
          observed_at
         FROM token_usage_events
        WHERE model = 'openagents/glm-5.2-reap-504b'
          AND demand_source = 'glm-pool-heartbeat'
          AND json_extract(safe_metadata_json, '$.heartbeatKind') = 'glm_pool_heartbeat'
        ORDER BY observed_at DESC
        LIMIT 100`,
    ),
  ])

  const nowMs = Date.parse(nowIso)
  const activeFreshCutoffMs = nowMs - 90_000
  const fleetAccounts = registrations.map(row => {
    const capacityRefs = parseJsonStringArray(row.latest_capacity_refs_json)
    const loadRefs = parseJsonStringArray(row.latest_load_refs_json)
    const capabilityRefs = parseJsonStringArray(row.capability_refs_json)
    const latestHeartbeatMs =
      row.latest_heartbeat_at === null ? Number.NaN : Date.parse(row.latest_heartbeat_at)
    const heartbeatFresh =
      Number.isFinite(latestHeartbeatMs) && latestHeartbeatMs >= activeFreshCutoffMs
    return {
      pylonRef: row.pylon_ref,
      status: row.status ?? 'unknown',
      heartbeatFresh,
      latestHeartbeatAt: row.latest_heartbeat_at,
      activeSlots: metricFromRefs(capacityRefs, 'capacity.coding.codex.available='),
      readySlots: metricFromRefs(capacityRefs, 'capacity.coding.codex.ready='),
      busySlots: metricFromRefs(loadRefs, 'load.coding.codex.busy='),
      queuedSlots: metricFromRefs(loadRefs, 'load.coding.codex.queued='),
      codexCapable: capabilityRefs.some(ref => ref.includes('codex')),
    }
  })

  const activeSlots = fleetAccounts.reduce((sum, account) => sum + account.activeSlots, 0)
  const readySlots = fleetAccounts.reduce((sum, account) => sum + account.readySlots, 0)
  const busySlots = fleetAccounts.reduce((sum, account) => sum + account.busySlots, 0)
  const queuedSlots = fleetAccounts.reduce((sum, account) => sum + account.queuedSlots, 0)
  const activeAssignments = assignments.filter(row =>
    row.state === 'accepted' || row.state === 'running' || row.state === 'proof_submitted',
  )
  const accountLedger = providerAccounts.map(row => {
    const cooldownExpiresAt =
      row.cooldown_until !== null && row.cooldown_until > nowIso
        ? row.cooldown_until
        : null
    const reason =
      row.reauth_required_reason ??
      row.recent_failure_class ??
      (row.low_credit_flag === 1 ? 'low_credit' : null)
    const status =
      row.reauth_required_reason !== null
        ? 'revoked'
        : cooldownExpiresAt !== null || row.recent_failure_class === 'rate_limited'
          ? 'rate_limited'
          : row.recent_failure_class === 'usage_limited'
            ? 'usage_limited'
            : row.status === 'connected' && (row.health === null || row.health === 'healthy')
              ? 'healthy'
              : row.health ?? row.status ?? 'unknown'

    return {
      accountRefHash: row.provider_account_ref,
      provider: row.provider === 'chatgpt_codex' ? 'codex' : row.provider,
      status,
      resetAt: cooldownExpiresAt,
      concurrency: Math.max(1, Math.trunc(row.lease_limit ?? 1)),
      reason,
    }
  })

  const todayTokens = Math.max(0, Math.trunc(today?.tokens_today ?? 0))
  const yesterdayTokens = Math.max(0, Math.trunc(yesterday?.tokens_yesterday ?? 0))
  const windowTokens = Math.max(0, Math.trunc(window?.tokens_window ?? 0))
  const hourTokens = Math.max(0, Math.trunc(hour?.tokens_hour ?? 0))
  const burnRateTokensPerMinute = Math.round(windowTokens / 10)
  const targetFloor = yesterdayTokens * 4
  const paceToFloor =
    targetFloor === 0 ? 'no_floor' : todayTokens >= targetFloor ? 'ahead' : 'behind'

  const latestAlertAgeMs =
    latestAlert === null ? Number.POSITIVE_INFINITY : millisBetween(latestAlert.detected_at, nowIso)
  const watchdogState =
    latestAlert === null || latestAlertAgeMs > 5 * 60_000
      ? 'HEALTHY'
      : latestAlert.classification === 'stalled'
        ? 'STALLED'
        : 'RECOVERING'

  const uniqueGlmReplicas = new Map<string, GlmHeartbeatRow>()
  let oldestGlmHeartbeatAt: string | null = null
  let latestHealthyGlmHeartbeatAt: string | null = null
  for (const row of glmRows) {
    if (row.replica_id === null || row.replica_id === '') {
      continue
    }
    if (!uniqueGlmReplicas.has(row.replica_id)) {
      uniqueGlmReplicas.set(row.replica_id, row)
    }
    if (typeof row.observed_at === 'string') {
      oldestGlmHeartbeatAt = earlierIso(oldestGlmHeartbeatAt, row.observed_at)
      latestHealthyGlmHeartbeatAt =
        isHealthyGlmStatus(row.health_status)
          ? laterIso(latestHealthyGlmHeartbeatAt, row.observed_at)
          : latestHealthyGlmHeartbeatAt
    }
  }
  const glmReady = [...uniqueGlmReplicas.values()].filter(row =>
    isHealthyGlmStatus(row.health_status),
  ).length
  const glmTotal = uniqueGlmReplicas.size
  const glmDownSince = latestHealthyGlmHeartbeatAt ?? oldestGlmHeartbeatAt
  const glmDownDurationMs =
    glmReady > 0 ? 0 : isoAgeMs(glmDownSince, nowIso) ?? 0
  const glmDownAlert =
    glmTotal > 0 &&
    glmReady === 0 &&
    glmDownDurationMs >= SERVING_RATE_MONITOR_GLM_DOWN_MINUTES * 60_000
  const servingRateAlert =
    hourTokens < SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR
  const opsMonitorAlerts = [
    ...(servingRateAlert
      ? ['alert.public.ops.serving_rate.tokens_per_hour_below_floor']
      : []),
    ...(glmDownAlert ? ['alert.public.ops.serving_rate.glm_down'] : []),
  ]

  return {
    schemaVersion: 'operator.fleet_status.v1',
    generatedAt: nowIso,
    cache: {
      maxAgeSeconds: 10,
      source: 'worker_memory_ttl',
    },
    authority: {
      buyerChargeMutationAllowed: false,
      dispatchMutationAllowed: false,
      payoutMutationAllowed: false,
      settlementMutationAllowed: false,
    },
    pace: {
      liveBurnRateTokensPerMinute: burnRateTokensPerMinute,
      paceToFloor,
      todayTokens,
      yesterdayTokens,
      targetFloorTokens: targetFloor,
      sourceRefs: ['d1:token_usage_events'],
    },
    fleet: {
      activeSlots,
      readySlots,
      busySlots,
      queuedSlots,
      spread: fleetAccounts,
      activeAssignments: assignments.map(row => ({
        assignmentRef: row.assignment_ref,
        pylonRef: row.pylon_ref,
        jobKind: row.job_kind,
        state: row.state,
        elapsedMs: millisBetween(row.created_at, nowIso),
        phase:
          row.state === 'proof_submitted'
            ? 'proof'
            : row.state === 'running' || row.state === 'accepted'
              ? 'running'
              : 'queued',
        tokensSoFar: null,
        lastProgressEvent: null,
        lastLog: null,
        updatedAt: row.updated_at,
        leaseExpiresAt: row.lease_expires_at,
      })),
      inFlightAssignments: assignments.map(row => ({
        assignmentRef: row.assignment_ref,
        pylonRef: row.pylon_ref,
        jobKind: row.job_kind,
        state: row.state,
        elapsedMs: millisBetween(row.created_at, nowIso),
        updatedAt: row.updated_at,
        leaseExpiresAt: row.lease_expires_at,
      })),
      activeAssignmentCount: activeAssignments.length,
      sourceRefs: ['d1:pylon_api_registrations', 'd1:pylon_api_assignments'],
    },
    accounts: {
      status: accountLedger,
      healthyCount: accountLedger.filter(row => row.status === 'healthy').length,
      limitedCount: accountLedger.filter(row =>
        row.status === 'rate_limited' || row.status === 'usage_limited',
      ).length,
      sourceRefs: ['d1:provider_accounts'],
    },
    supervisor: {
      state: readySlots > busySlots ? 'ready' : activeAssignments.length > 0 ? 'busy' : 'idle',
      desiredCodexSlots: readySlots,
      availableCodexSlots: activeSlots,
      queueDepth: queuedSlots,
      sourceRefs: ['d1:pylon_api_registrations'],
    },
    recentFailures: [
      ...accountLedger
        .filter(row => row.reason !== null && row.status !== 'healthy')
        .slice(0, 10)
        .map(row => ({
          scope: 'account',
          ref: row.accountRefHash,
          reasonCode: row.reason,
          observedAt: nowIso,
        })),
      ...(latestAlert === null || latestAlertAgeMs > 5 * 60_000
        ? []
        : [{
            scope: 'fleet',
            ref: latestAlert.alert_ref,
            reasonCode: latestAlert.reason_ref,
            observedAt: latestAlert.detected_at,
          }]),
    ].slice(0, 10),
    watchdog: {
      state: watchdogState,
      lastCronHeartbeatAt: latestAlert?.detected_at ?? null,
      activeLeases: activeAssignments.length,
      activeAlerts: latestAlert === null || latestAlertAgeMs > 5 * 60_000 ? [] : [{
        alertRef: latestAlert.alert_ref,
        classification: latestAlert.classification,
        reasonRef: latestAlert.reason_ref,
        detectedAt: latestAlert.detected_at,
      }],
      sourceRefs: ['d1:fleet_alerts', 'd1:pylon_api_assignments'],
    },
    glm: {
      status: glmTotal === 0 ? 'unknown' : glmReady === glmTotal ? 'ready' : 'degraded',
      readyReplicas: glmReady,
      totalReplicas: glmTotal,
      latestHealthyHeartbeatAt: latestHealthyGlmHeartbeatAt,
      sourceRefs: ['d1:token_usage_events'],
      caveatRefs: glmTotal === 0 ? ['caveat.public.operator_fleet_status.glm_heartbeat_rows_missing'] : [],
    },
    opsMonitor: {
      status: opsMonitorAlerts.length === 0 ? 'ok' : 'alert',
      activeAlertRefs: opsMonitorAlerts,
      tokenVelocity: {
        status: servingRateAlert ? 'alert' : 'ok',
        windowMinutes: SERVING_RATE_MONITOR_WINDOW_MINUTES,
        tokensInWindow: hourTokens,
        tokensPerHour: hourTokens,
        floorTokensPerHour: SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR,
        reasonRef: servingRateAlert
          ? 'blocker.public.ops.serving_rate.tokens_per_hour_below_floor'
          : 'ops.serving_rate.tokens_per_hour_ok',
      },
      glmLaneHealth: {
        status: glmTotal === 0 ? 'unknown' : glmDownAlert ? 'alert' : 'ok',
        readyReplicas: glmReady,
        totalReplicas: glmTotal,
        downWindowMinutes: SERVING_RATE_MONITOR_GLM_DOWN_MINUTES,
        downSince: glmReady === 0 ? glmDownSince : null,
        downDurationMs: glmDownDurationMs,
        reasonRef:
          glmTotal === 0
            ? 'caveat.public.ops.serving_rate.glm_heartbeat_rows_missing'
            : glmDownAlert
              ? 'blocker.public.ops.serving_rate.glm_down_over_15m'
              : 'ops.serving_rate.glm_lane_ok',
      },
      sourceRefs: ['d1:token_usage_events', 'worker:scheduled'],
    },
    brain: {
      loopHealth: watchdogState === 'STALLED' ? 'stalled' : 'healthy',
      currentGoals: [
        {
          goalRef: 'goal.public.artanis.serve_token_pace',
          state: paceToFloor,
        },
      ],
      recentDecisions: brainRows.map(row => ({
        decisionRef: row.memory_ref,
        createdAt: row.created_at,
        summaryRef: `summary.public.artanis_decision.${row.memory_ref}`,
      })),
      sourceRefs: ['d1:artanis_owner_memory'],
    },
  }
}

type ServingRateMonitorSnapshot = Readonly<{
  status: 'ok' | 'alert'
  activeAlertRefs: ReadonlyArray<string>
}>

export const runServingRateMonitorScheduled = async (
  db: D1Database,
  input: Readonly<{ nowIso: string }>,
  log: (line: string, fields: Record<string, unknown>) => void,
): Promise<ServingRateMonitorSnapshot | null> => {
  const snapshot = await buildFleetStatusSnapshot(db, input.nowIso, {
    kind: 'admin',
  })
  const monitor =
    typeof snapshot === 'object' && snapshot !== null && 'opsMonitor' in snapshot
      ? (snapshot as { opsMonitor?: ServingRateMonitorSnapshot }).opsMonitor
      : undefined

  if (monitor === undefined) {
    return null
  }

  if (monitor.status === 'alert') {
    log('serving_rate_monitor_alert', {
      activeAlertRefs: monitor.activeAlertRefs.join(','),
      monitor,
    })
  }

  return monitor
}

type OperatorFleetReadScope =
  | Readonly<{ kind: 'admin' }>
  | Readonly<{ kind: 'agent'; userId: string }>

const authorizeFleetRead = async <Bindings extends OperatorFleetStatusEnv>(
  dependencies: OperatorFleetStatusDependencies<Bindings>,
  request: Request,
  env: Bindings,
  allowAgentToken: boolean,
): Promise<OperatorFleetReadScope | null> => {
  if (await dependencies.requireAdminApiToken(request, env)) {
    return { kind: 'admin' }
  }

  if (!allowAgentToken) {
    return null
  }

  const agent = await dependencies.authenticateAgentToken?.(request, env)
  if (agent !== undefined) {
    return { kind: 'agent', userId: agent.userId }
  }

  return null
}

export const makeOperatorFleetStatusRoutes = <
  Bindings extends OperatorFleetStatusEnv,
>(
  dependencies: OperatorFleetStatusDependencies<Bindings>,
) => ({
  handleOperatorFleetStatusApi: async (
    request: Request,
    env: Bindings,
  ) => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const allowAgentToken =
      new URL(request.url).pathname === OPERATOR_FLEET_STATE_PATH
    const scope = await authorizeFleetRead(
      dependencies,
      request,
      env,
      allowAgentToken,
    )
    if (scope === null) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const nowIso = (dependencies.currentIsoTimestamp ?? currentIsoTimestamp)()
    const cacheKey =
      scope.kind === 'admin'
        ? 'operator:fleet:state:v1:admin'
        : `operator:fleet:state:v1:agent:${scope.userId}`
    const cached = snapshotCache.get(cacheKey)
    const nowMs = Date.parse(nowIso)
    if (
      cached !== undefined &&
      Number.isFinite(nowMs) &&
      cached.expiresAt > nowMs
    ) {
      return jsonResponse(cached.response, {
        headers: {
          'cache-control': 'private, max-age=10',
          'x-openagents-cache': 'hit',
        },
      })
    }

    const response = await buildFleetStatusSnapshot(
      openAgentsDatabase(env),
      nowIso,
      scope,
    )
    if (Number.isFinite(nowMs)) {
      snapshotCache.set(cacheKey, {
        expiresAt: nowMs + CACHE_TTL_MILLIS,
        generatedAt: nowIso,
        response,
      })
    }

    return jsonResponse(response, {
      headers: {
        'cache-control': 'private, max-age=10',
        'x-openagents-cache': 'miss',
      },
    })
  },
})
