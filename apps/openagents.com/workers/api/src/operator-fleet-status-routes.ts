import { jsonResponse } from '@openagentsinc/sync-worker'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonRecord, parseJsonStringArray } from './json-boundary'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp, todayAndYesterdayBoundsInTimezone } from './runtime-primitives'
import { PUBLIC_KHALA_TOKENS_SERVED_TIMEZONE } from './token-usage-ledger'

export const OPERATOR_FLEET_STATUS_PATH = '/api/operator/fleet/status'
export const OPERATOR_FLEET_STATE_PATH = '/api/operator/fleet/state'
const CACHE_TTL_MILLIS = 10_000
const SPINE_SCHEMA_VERSION = 'openagents.pylon.agent_runner_status_event.v1'

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

type StatusRow = Readonly<{
  event_ref: string
  owner_agent_user_id: string
  runner_ref: string
  runner_kind: string
  pylon_ref: string | null
  assignment_ref: string | null
  state: string
  state_started_at: string
  updated_at: string
  retention_state: 'live' | 'retained'
  event_json: string
}>

type CacheEntry = Readonly<{
  expiresAt: number
  generatedAt: string
  response: unknown
}>

export type OperatorFleetReadScope =
  | Readonly<{ kind: 'admin' }>
  | Readonly<{ kind: 'agent'; userId: string }>

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

const boundedString = (value: unknown, maxLength: number): string | null =>
  typeof value === 'string' && value.length > 0
    ? value.slice(0, maxLength)
    : null

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

const normalizeState = (state: string): string =>
  state === 'working'
    ? 'running'
    : state === 'waiting'
      ? 'accepted'
      : state === 'done'
        ? 'proof_submitted'
        : state

const activeRunnerState = (state: string): boolean =>
  state === 'queued' ||
  state === 'working' ||
  state === 'waiting' ||
  state === 'blocked'

const parseEvent = (row: StatusRow): Record<string, unknown> =>
  parseJsonRecord(row.event_json) ?? {}

const eventRefs = (event: Record<string, unknown>): ReadonlyArray<string> => [
  ...parseJsonStringArray(JSON.stringify(event.refs ?? [])),
  ...parseJsonStringArray(JSON.stringify(event.capabilityRefs ?? [])),
]

export const readOperatorFleetStatusSnapshotFromSpine = async (
  db: D1Database,
  nowIso: string,
  scope: OperatorFleetReadScope,
  legacyStatusPath: boolean,
): Promise<unknown> => {
  const ownerClause = scope.kind === 'admin' ? '' : 'AND owner_agent_user_id = ?'
  const ownerBindings = scope.kind === 'admin' ? [] : [scope.userId]
  const rows = await safeAll<StatusRow>(
    db,
    `SELECT event_ref, owner_agent_user_id, runner_ref, runner_kind, pylon_ref,
            assignment_ref, state, state_started_at, updated_at,
            retention_state, event_json
       FROM pylon_agent_runner_status_events
      WHERE archived_at IS NULL
        ${ownerClause}
      ORDER BY updated_at DESC
      LIMIT 200`,
    ...ownerBindings,
  )

  const liveRows = rows.filter(row => row.retention_state === 'live')
  const activeRows = liveRows.filter(row => activeRunnerState(row.state))
  const byPylon = new Map<string, ReadonlyArray<StatusRow>>()
  for (const row of liveRows) {
    const pylonRef = row.pylon_ref ?? `pylon.unknown.${row.owner_agent_user_id}`
    byPylon.set(pylonRef, [...(byPylon.get(pylonRef) ?? []), row])
  }

  const fleetAccounts = [...byPylon.entries()].map(([pylonRef, pylonRows]) => {
    const refs = pylonRows.flatMap(row => eventRefs(parseEvent(row)))
    const latestUpdatedAt = pylonRows
      .map(row => row.updated_at)
      .sort()
      .at(-1) ?? null
    const availableFromRefs = metricFromRefs(refs, 'capacity.coding.codex.available=')
    const readyFromRefs = metricFromRefs(refs, 'capacity.coding.codex.ready=')
    const busyFromRefs = metricFromRefs(refs, 'load.coding.codex.busy=')
    const queuedFromRefs = metricFromRefs(refs, 'load.coding.codex.queued=')
    const activeForPylon = pylonRows.filter(row => activeRunnerState(row.state))
    const busySlots = Math.max(
      busyFromRefs,
      activeForPylon.filter(row => row.state === 'working').length,
    )
    const queuedSlots = Math.max(
      queuedFromRefs,
      activeForPylon.filter(row => row.state === 'queued' || row.state === 'waiting').length,
    )
    const readySlots = Math.max(readyFromRefs, availableFromRefs, activeForPylon.length)
    return {
      pylonRef,
      status: activeForPylon.length > 0 ? 'active' : 'idle',
      heartbeatFresh: latestUpdatedAt === null
        ? false
        : millisBetween(latestUpdatedAt, nowIso) <= 90_000,
      latestHeartbeatAt: latestUpdatedAt,
      activeSlots: Math.max(availableFromRefs, Math.max(readySlots - busySlots, 0)),
      readySlots,
      busySlots,
      queuedSlots,
      codexCapable: refs.some(ref => ref.includes('codex')) ||
        pylonRows.some(row => row.runner_kind.includes('codex')),
    }
  })

  const activeSlots = fleetAccounts.reduce((sum, account) => sum + account.activeSlots, 0)
  const readySlots = fleetAccounts.reduce((sum, account) => sum + account.readySlots, 0)
  const busySlots = fleetAccounts.reduce((sum, account) => sum + account.busySlots, 0)
  const queuedSlots = fleetAccounts.reduce((sum, account) => sum + account.queuedSlots, 0)
  const activeAssignments = activeRows.map(row => {
    const event = parseEvent(row)
    const blockerRefs = parseJsonStringArray(JSON.stringify(event.blockerRefs ?? []))
    const refs = parseJsonStringArray(JSON.stringify(event.refs ?? []))
    const phase = row.state === 'blocked'
      ? 'blocked'
      : row.state === 'waiting'
        ? 'waiting'
        : row.state === 'queued'
          ? 'queued'
          : 'running'
    return {
      assignmentRef: row.assignment_ref ?? row.runner_ref,
      pylonRef: row.pylon_ref ?? null,
      jobKind: row.runner_kind,
      state: normalizeState(row.state),
      elapsedMs: millisBetween(row.state_started_at, nowIso),
      lastUpdateAgeMs: millisBetween(row.updated_at, nowIso),
      phase,
      tokensSoFar: null,
      lastProgressEvent: boundedString(refs[0] ?? blockerRefs[0], 120),
      lastLog: boundedString(blockerRefs[0] ?? refs[0], 240),
      progressObservedAt: row.updated_at,
      progressAgeMs: millisBetween(row.updated_at, nowIso),
      tokenCountKind: null,
      updatedAt: row.updated_at,
      leaseExpiresAt: null,
    }
  })

  const dayBounds = todayAndYesterdayBoundsInTimezone(
    nowIso,
    PUBLIC_KHALA_TOKENS_SERVED_TIMEZONE,
  )
  const sourceRefs = ['d1:pylon_agent_runner_status_events']
  const targetFloor = 0

  return {
    schemaVersion: 'operator.fleet_status.v1',
    generatedAt: nowIso,
    ...(legacyStatusPath
      ? {
          deprecation: {
            deprecated: true,
            replacementPath: '/api/operator/pro/status',
            removalCondition: 'Remove after T11.1 mobile pairing/transport replaces the iOS fleet-status poll.',
            sourceOfTruth: 'operator_pro_status_spine',
          },
        }
      : {}),
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
      timezone: PUBLIC_KHALA_TOKENS_SERVED_TIMEZONE,
      activeAdjustedTokensPerMinute: 0,
      activeSessionTokenEstimate: {
        activeAssignmentCount: activeAssignments.length,
        assignments: [],
        inFlightTokens: 0,
        inFlightTokensPerMinute: 0,
        method: 'runner status spine only; token pace comes from exact token_usage_events projections outside this compat route',
        caveatRefs: ['caveat.public.operator_fleet_status.spine_status_only'],
        sourceRefs,
      },
      liveBurnRateTokensPerMinute: 0,
      paceToFloor: 'no_floor',
      todayTokens: 0,
      yesterdayTokens: 0,
      targetFloorTokens: targetFloor,
      ownCapacityCodex: {
        assignmentsWindow: 0,
        sourceRefs,
        tokensPerMinute: 0,
        tokensWindow: 0,
        turnsWindow: 0,
        windowSeconds: 600,
      },
      sourceRefs: [
        ...sourceRefs,
        `timezone.public.${PUBLIC_KHALA_TOKENS_SERVED_TIMEZONE}`,
        `window.public.today.${dayBounds.todayStartIso}`,
      ],
    },
    fleet: {
      activeSlots,
      readySlots,
      busySlots,
      queuedSlots,
      spread: fleetAccounts,
      activeAssignments,
      inFlightAssignments: activeAssignments.map(assignment => ({
        assignmentRef: assignment.assignmentRef,
        pylonRef: assignment.pylonRef,
        jobKind: assignment.jobKind,
        state: assignment.state,
        elapsedMs: assignment.elapsedMs,
        lastUpdateAgeMs: assignment.lastUpdateAgeMs,
        updatedAt: assignment.updatedAt,
        leaseExpiresAt: assignment.leaseExpiresAt,
      })),
      activeAssignmentCount: activeAssignments.length,
      sourceRefs,
    },
    accounts: {
      status: [],
      healthyCount: 0,
      limitedCount: 0,
      sourceRefs,
    },
    supervisor: {
      state: activeAssignments.length > 0 ? 'busy' : readySlots > 0 ? 'ready' : 'idle',
      desiredCodexSlots: readySlots,
      availableCodexSlots: activeSlots,
      queueDepth: queuedSlots,
      sourceRefs,
    },
    recentFailures: activeRows
      .filter(row => row.state === 'blocked')
      .slice(0, 10)
      .map(row => {
        const event = parseEvent(row)
        const blockerRefs = parseJsonStringArray(JSON.stringify(event.blockerRefs ?? []))
        return {
          scope: 'runner',
          ref: row.runner_ref,
          reasonCode: blockerRefs[0] ?? 'blocker.public.runner_status.blocked',
          observedAt: row.updated_at,
        }
      }),
    watchdog: {
      state: activeRows.some(row => row.state === 'blocked') ? 'STALLED' : 'HEALTHY',
      lastCronHeartbeatAt: null,
      activeLeases: activeAssignments.length,
      activeAlerts: [],
      sourceRefs,
    },
    servingRateMonitor: {
      state: 'HEALTHY',
      latestAlert: null,
      sourceRefs,
    },
    glm: {
      status: 'unknown',
      readyReplicas: 0,
      totalReplicas: 0,
      sourceRefs,
      caveatRefs: ['caveat.public.operator_fleet_status.glm_not_in_spine'],
    },
    brain: {
      loopHealth: activeRows.some(row => row.state === 'blocked') ? 'stalled' : 'healthy',
      currentGoals: [
        {
          goalRef: 'goal.public.artanis.serve_token_pace',
          state: 'no_floor',
        },
      ],
      recentDecisions: [],
      sourceRefs,
    },
    spine: {
      schemaVersion: SPINE_SCHEMA_VERSION,
      liveRunnerCount: liveRows.length,
      retainedRunnerCount: rows.filter(row => row.retention_state === 'retained').length,
      sourceRefs,
    },
  }
}

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

    const pathname = new URL(request.url).pathname
    const legacyStatusPath = pathname === OPERATOR_FLEET_STATUS_PATH
    const allowAgentToken = pathname === OPERATOR_FLEET_STATE_PATH
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
    const cacheKey = [
      'operator:fleet:status:spine:v1',
      legacyStatusPath ? 'legacy' : 'state',
      scope.kind === 'admin' ? 'admin' : `agent:${scope.userId}`,
    ].join(':')
    const cached = snapshotCache.get(cacheKey)
    const nowMs = Date.parse(nowIso)
    const headers = {
      'cache-control': 'private, max-age=10',
      ...(legacyStatusPath
        ? {
            deprecation: 'true',
            link: '</api/operator/pro/status>; rel="successor-version"',
            'x-openagents-deprecated': 'T10.3 compat until T11.1 mobile pairing replaces this poll',
          }
        : {}),
    }
    if (
      cached !== undefined &&
      Number.isFinite(nowMs) &&
      cached.expiresAt > nowMs
    ) {
      return jsonResponse(cached.response, {
        headers: {
          ...headers,
          'x-openagents-cache': 'hit',
        },
      })
    }

    const response = await readOperatorFleetStatusSnapshotFromSpine(
      openAgentsDatabase(env),
      nowIso,
      scope,
      legacyStatusPath,
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
        ...headers,
        'x-openagents-cache': 'miss',
      },
    })
  },
})
