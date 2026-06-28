import { Effect, Schema as S } from 'effect'

import { type ArtanisGlmFleetStatus } from './artanis-operator-tools'
import {
  ARTANIS_TOKEN_PACE_TIMEZONE,
  computeArtanisTokenPaceBlock,
} from './artanis-token-pace'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { isRecord } from './json-boundary'
import {
  currentEpochMillis,
  currentIsoTimestamp,
} from './runtime-primitives'
import {
  type TokenUsageLedgerShape,
  makeD1TokenUsageLedger,
} from './token-usage-ledger'

const STATUS_CACHE_TTL_MS = 10_000
const ACTIVE_ASSIGNMENT_STATES = new Set([
  'accepted',
  'blocked',
  'offered',
  'proof_submitted',
  'running',
])
const WATCHDOG_HEALTHY_CRON_WINDOW_MS = 10 * 60 * 1000

export const OperatorFleetStatusResponse = S.Struct({
  schemaVersion: S.Literal('openagents.operator_fleet_status.v1'),
  generatedAt: S.String,
  cache: S.Struct({
    cachedAt: S.String,
    maxAgeSeconds: S.Int,
    status: S.Literals(['hit', 'miss']),
  }),
  pace: S.Unknown,
  fleet: S.Unknown,
  watchdog: S.Unknown,
  glm: S.Unknown,
  brain: S.Unknown,
})
export type OperatorFleetStatusResponse = typeof OperatorFleetStatusResponse.Type

type OperatorFleetStatusRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  glmStatusLoader?: (() => Promise<ArtanisGlmFleetStatus>) | undefined
  ledger?: TokenUsageLedgerShape | undefined
  nowIso?: (() => string) | undefined
  nowUnixMs?: (() => number) | undefined
  requireAdminApiToken?: ((request: Request) => Promise<boolean>) | undefined
}>

type CachedStatus = Readonly<{
  atMs: number
  cachedAt: string
  payload: Omit<OperatorFleetStatusResponse, 'cache' | 'generatedAt'>
}>

let statusCache: CachedStatus | undefined

const intFromUnknown = (value: unknown): number =>
  typeof value === 'number' && Number.isFinite(value)
    ? Math.max(0, Math.trunc(value))
    : 0

const stringFromUnknown = (value: unknown): string | null =>
  typeof value === 'string' && value.trim() !== '' ? value : null

const parseJsonRecord = (value: string | null): Record<string, unknown> => {
  if (value === null || value.trim() === '') return {}
  try {
    const parsed = JSON.parse(value)
    return isRecord(parsed) ? parsed : {}
  } catch {
    return {}
  }
}

const msSince = (iso: string | null, nowMs: number): number | null => {
  if (iso === null) return null
  const then = Date.parse(iso)
  return Number.isFinite(then) ? Math.max(0, nowMs - then) : null
}

const blockUnavailable = (reason: string) => ({
  available: false,
  reason,
})

const readPaceBlock = (
  ledger: TokenUsageLedgerShape,
  nowIso: string,
) =>
  Effect.gen(function* () {
    const [aggregate, history] = yield* Effect.all([
      ledger.readPublicTokensServed(),
      ledger.readPublicTokensServedHistory({
        bucket: 'day',
        timezone: ARTANIS_TOKEN_PACE_TIMEZONE,
        window: '7d',
      }),
    ])
    const pace = computeArtanisTokenPaceBlock({
      nowIso,
      series: history.series,
      timezone: ARTANIS_TOKEN_PACE_TIMEZONE,
    })
    return {
      available: true,
      allTimeTokensServed: aggregate.tokensServed,
      pace,
      timezone: ARTANIS_TOKEN_PACE_TIMEZONE,
    }
  }).pipe(
    Effect.catch(() =>
      Effect.succeed(blockUnavailable('token_usage_ledger_unavailable')),
    ),
  )

type PylonRegistrationRow = Readonly<{
  latest_heartbeat_at: string | null
  public_projection_json: string | null
  pylon_ref: string
  status: string
}>

type PylonAssignmentRow = Readonly<{
  assignment_ref: string
  lease_expires_at: string | null
  pylon_ref: string
  state: string
  updated_at: string
}>

const readFleetBlock = (db: D1Database, nowMs: number, nowIso: string) =>
  Effect.tryPromise({
    catch: error => error,
    try: async () => {
      const registrations = await db
        .prepare(
          `SELECT pylon_ref, status, latest_heartbeat_at, public_projection_json
             FROM pylon_api_registrations
            WHERE archived_at IS NULL
            ORDER BY updated_at DESC
            LIMIT 200`,
        )
        .all<PylonRegistrationRow>()
      const assignments = await db
        .prepare(
          `SELECT assignment_ref, pylon_ref, state, lease_expires_at, updated_at
             FROM pylon_api_assignments
            WHERE archived_at IS NULL
              AND state IN ('accepted', 'blocked', 'offered', 'proof_submitted', 'running')
            ORDER BY updated_at DESC
            LIMIT 100`,
        )
        .all<PylonAssignmentRow>()

      const activePylons = (registrations.results ?? []).filter(row => row.status === 'active')
      const services = new Map<string, { available: number; busy: number; queued: number; ready: number }>()
      for (const row of activePylons) {
        const projection = parseJsonRecord(row.public_projection_json)
        const capacities = Array.isArray(projection.codingCapacity)
          ? projection.codingCapacity
          : []
        for (const capacity of capacities) {
          if (!isRecord(capacity)) continue
          const service = stringFromUnknown(capacity.service) ?? 'unknown'
          const current = services.get(service) ?? { available: 0, busy: 0, queued: 0, ready: 0 }
          current.available += intFromUnknown(capacity.available)
          current.busy += intFromUnknown(capacity.busy)
          current.queued += intFromUnknown(capacity.queued)
          current.ready += intFromUnknown(capacity.ready)
          services.set(service, current)
        }
      }

      const inFlightAssignments = (assignments.results ?? [])
        .filter(row => ACTIVE_ASSIGNMENT_STATES.has(row.state))
        .map(row => ({
          assignmentRef: row.assignment_ref,
          elapsedMs: msSince(row.updated_at, nowMs),
          leaseExpiresAt: row.lease_expires_at,
          pylonRef: row.pylon_ref,
          state: row.state,
          updatedAt: row.updated_at,
        }))

      const activeSlots = [...services.values()].reduce(
        (sum, service) => ({
          available: sum.available + service.available,
          busy: sum.busy + service.busy,
          queued: sum.queued + service.queued,
          ready: sum.ready + service.ready,
        }),
        { available: 0, busy: 0, queued: 0, ready: 0 },
      )

      return {
        activeSlots,
        available: true,
        generatedAt: nowIso,
        inFlightAssignments,
        perService: Object.fromEntries(services),
        pylonCount: activePylons.length,
      }
    },
  }).pipe(
    Effect.catch(() =>
      Effect.succeed(blockUnavailable('pylon_projection_unavailable')),
    ),
  )

const readWatchdogBlock = (db: D1Database, nowMs: number) =>
  Effect.tryPromise({
    catch: error => error,
    try: async () => {
      const row = await db
        .prepare(
          `SELECT created_at
             FROM pylon_api_events
            WHERE archived_at IS NULL
              AND event_kind = 'heartbeat'
            ORDER BY created_at DESC
            LIMIT 1`,
        )
        .first<{ created_at: string | null }>()
      const leases = await db
        .prepare(
          `SELECT COUNT(*) AS count
             FROM pylon_api_assignments
            WHERE archived_at IS NULL
              AND state IN ('accepted', 'blocked', 'offered', 'proof_submitted', 'running')`,
        )
        .first<{ count: number | null }>()
      const ageMs = msSince(row?.created_at ?? null, nowMs)
      const state =
        ageMs === null
          ? 'STALLED'
          : ageMs <= WATCHDOG_HEALTHY_CRON_WINDOW_MS
            ? 'HEALTHY'
            : 'RECOVERING'
      return {
        activeAlerts: state === 'HEALTHY' ? [] : ['alert.public.operator_fleet.watchdog_heartbeat_stale'],
        activeLeases: Math.max(0, Math.trunc(leases?.count ?? 0)),
        available: true,
        lastCronHeartbeatAt: row?.created_at ?? null,
        state,
      }
    },
  }).pipe(
    Effect.catch(() =>
      Effect.succeed(blockUnavailable('watchdog_projection_unavailable')),
    ),
  )

const readBrainBlock = (db: D1Database) =>
  Effect.tryPromise({
    catch: error => error,
    try: async () => {
      const rows = await db
        .prepare(
          `SELECT 'loop_record' AS kind, record_ref, state, updated_at
             FROM artanis_loop_records
            UNION ALL
           SELECT 'loop_tick' AS kind, record_ref, state, updated_at
             FROM artanis_loop_ticks
            UNION ALL
           SELECT 'work_routing_proposal' AS kind, record_ref, state, updated_at
             FROM artanis_work_routing_proposals
            ORDER BY updated_at DESC
            LIMIT 12`,
        )
        .all<{
          kind: string
          record_ref: string
          state: string
          updated_at: string
        }>()
      const records = rows.results ?? []
      return {
        available: true,
        currentGoals: records
          .filter(row => row.kind.includes('goal') || row.kind.includes('work_routing'))
          .slice(0, 3)
          .map(row => ({
            recordRef: row.record_ref,
            state: row.state,
            updatedAt: row.updated_at,
          })),
        lastDecisions: records
          .slice(0, 3)
          .map(row => ({
            kind: row.kind,
            recordRef: row.record_ref,
            state: row.state,
            updatedAt: row.updated_at,
          })),
        loopHealth: records.length > 0 ? 'observed' : 'no_recent_records',
      }
    },
  }).pipe(
    Effect.catch(() =>
      Effect.succeed(blockUnavailable('artanis_persistence_unavailable')),
    ),
  )

const readGlmBlock = (loader: (() => Promise<ArtanisGlmFleetStatus>) | undefined) =>
  loader === undefined
    ? Effect.succeed(blockUnavailable('glm_status_loader_unavailable'))
    : Effect.tryPromise({
        catch: error => error,
        try: async () => {
          const status = await loader()
          return {
            available: true,
            readyReplicas: status.readyReplicas,
            replicaCounts: {
              ready: status.readyReplicas,
              total: status.totalReplicas,
              warm: status.warmReplicas,
            },
            status: status.status,
          }
        },
      }).pipe(
        Effect.catch(() =>
          Effect.succeed(blockUnavailable('glm_status_loader_failed')),
        ),
      )

const readSnapshotPayload = (
  input: OperatorFleetStatusRouteInput,
  nowIso: string,
  nowMs: number,
) => {
  const db = input.OPENAGENTS_DB
  const ledger = input.ledger ?? (db === undefined ? undefined : makeD1TokenUsageLedger(db))

  return Effect.all(
    {
      brain: db === undefined ? Effect.succeed(blockUnavailable('d1_binding_missing')) : readBrainBlock(db),
      fleet: db === undefined ? Effect.succeed(blockUnavailable('d1_binding_missing')) : readFleetBlock(db, nowMs, nowIso),
      glm: readGlmBlock(input.glmStatusLoader),
      pace: ledger === undefined ? Effect.succeed(blockUnavailable('token_usage_ledger_missing')) : readPaceBlock(ledger, nowIso),
      watchdog: db === undefined ? Effect.succeed(blockUnavailable('d1_binding_missing')) : readWatchdogBlock(db, nowMs),
    },
    { concurrency: 5 },
  ).pipe(
    Effect.map(blocks => ({
      schemaVersion: 'openagents.operator_fleet_status.v1' as const,
      ...blocks,
    })),
  )
}

export const handleOperatorFleetStatusApi = (
  request: Request,
  input: OperatorFleetStatusRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const nowMs = input.nowUnixMs?.() ?? currentEpochMillis()
  const nowIso = input.nowIso?.() ?? currentIsoTimestamp()
  const cacheable = input.ledger === undefined
  const cached = cacheable ? statusCache : undefined
  if (cached !== undefined && nowMs - cached.atMs < STATUS_CACHE_TTL_MS) {
    return Effect.succeed(
      noStoreJsonResponse({
        ...cached.payload,
        cache: {
          cachedAt: cached.cachedAt,
          maxAgeSeconds: STATUS_CACHE_TTL_MS / 1000,
          status: 'hit',
        },
        generatedAt: nowIso,
      } satisfies OperatorFleetStatusResponse),
    )
  }

  return Effect.gen(function* () {
    if (input.requireAdminApiToken !== undefined) {
      const allowed = yield* Effect.tryPromise({
        catch: () => false,
        try: () => input.requireAdminApiToken!(request),
      }).pipe(Effect.catch(() => Effect.succeed(false)))
      if (!allowed) return unauthorized()
    }

    const payload = yield* readSnapshotPayload(input, nowIso, nowMs)
    if (cacheable) {
      statusCache = { atMs: nowMs, cachedAt: nowIso, payload }
    }
    return noStoreJsonResponse({
      ...payload,
      cache: {
        cachedAt: nowIso,
        maxAgeSeconds: STATUS_CACHE_TTL_MS / 1000,
        status: 'miss',
      },
      generatedAt: nowIso,
    } satisfies OperatorFleetStatusResponse)
  })
}
