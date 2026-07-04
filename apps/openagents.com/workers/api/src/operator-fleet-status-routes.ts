import { jsonResponse } from '@openagentsinc/sync-worker'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonRecord, parseJsonStringArray } from './json-boundary'
import {
  makeD1PylonAgentRunnerStatusReadStore,
  makePylonAgentRunnerStatusReadStoreForEnv,
  type PylonAgentRunnerStatusReadScope,
  type PylonAgentRunnerStatusReadStore,
  type PylonAgentRunnerStatusRow,
} from './pylon-agent-runner-status-store'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp, todayAndYesterdayBoundsInTimezone } from './runtime-primitives'
import { PUBLIC_KHALA_TOKENS_SERVED_TIMEZONE } from './token-usage-ledger'

export const OPERATOR_FLEET_STATUS_PATH = '/api/operator/fleet/status'
export const OPERATOR_FLEET_STATE_PATH = '/api/operator/fleet/state'
const CACHE_TTL_MILLIS = 10_000
const SPINE_SCHEMA_VERSION = 'openagents.pylon.agent_runner_status_event.v1'

type OperatorFleetStatusEnv = Readonly<{
  OPENAGENTS_DB: D1Database
  KHALA_SYNC_DB?: { connectionString: string } | undefined
  KHALA_SYNC_PYLON_DUAL_WRITE?: string | undefined
  KHALA_SYNC_PYLON_READS?: string | undefined
}>

type OperatorFleetStatusDependencies<Bindings extends OperatorFleetStatusEnv> =
  Readonly<{
    authenticateAgentToken?: (
      request: Request,
      env: Bindings,
    ) => Promise<{ userId: string } | undefined>
    currentIsoTimestamp?: () => string
    makeRunnerStatusReadStore?: (
      env: Bindings,
      db: D1Database,
    ) => PylonAgentRunnerStatusReadStore
    requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  }>

type CacheEntry = Readonly<{
  expiresAt: number
  generatedAt: string
  response: unknown
}>

export type OperatorFleetReadScope = PylonAgentRunnerStatusReadScope

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

const parseEvent = (
  row: PylonAgentRunnerStatusRow,
): Record<string, unknown> =>
  parseJsonRecord(row.event_json) ?? {}

const eventRefs = (event: Record<string, unknown>): ReadonlyArray<string> => [
  ...parseJsonStringArray(JSON.stringify(event.refs ?? [])),
  ...parseJsonStringArray(JSON.stringify(event.capabilityRefs ?? [])),
]

export const readOperatorFleetStatusSnapshotFromRunnerStatusReadStore = async (
  readStore: PylonAgentRunnerStatusReadStore,
  nowIso: string,
  scope: OperatorFleetReadScope,
  legacyStatusPath: boolean,
): Promise<unknown> => {
  const readResult = await readStore.listStatusRows({ limit: 200, scope })
  const rows = readResult.rows

  const liveRows = rows.filter(row => row.retention_state === 'live')
  const activeRows = liveRows.filter(row => activeRunnerState(row.state))
  const byPylon = new Map<string, ReadonlyArray<PylonAgentRunnerStatusRow>>()
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
  const sourceRefs = readResult.sourceRefs
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

export const readOperatorFleetStatusSnapshotFromSpine = async (
  db: D1Database,
  nowIso: string,
  scope: OperatorFleetReadScope,
  legacyStatusPath: boolean,
): Promise<unknown> =>
  readOperatorFleetStatusSnapshotFromRunnerStatusReadStore(
    makeD1PylonAgentRunnerStatusReadStore(db),
    nowIso,
    scope,
    legacyStatusPath,
  )

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
      env.KHALA_SYNC_PYLON_READS?.trim().toLowerCase() ?? 'd1',
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

    const db = openAgentsDatabase(env)
    const readStore =
      dependencies.makeRunnerStatusReadStore?.(env, db) ??
      makePylonAgentRunnerStatusReadStoreForEnv(env, db)
    const response = await readOperatorFleetStatusSnapshotFromRunnerStatusReadStore(
      readStore,
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
