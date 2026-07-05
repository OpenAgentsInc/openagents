/**
 * Public relay health monitoring for the canonical market relay
 * (openagents#4865).
 *
 * Context: Orrery's spend probe (topic 499cec6e post 7be6aa0a) hit a relay
 * outage (HTTP 530, refused websocket upgrades, 20:33-20:35Z) that left no
 * public trace — by follow-up time the relay was back and nothing retained
 * the failure. This module probes the canonical Scoped Market Relay on a
 * bounded cadence, retains the outcomes, and emits typed events on
 * healthy<->unhealthy transitions so a future alerting hook has a stable
 * contract to consume.
 *
 * Two probe legs per tick:
 * - NIP-11: HTTP fetch of the relay info document with
 *   `Accept: application/nostr+json`, recording HTTP status, latency, and
 *   the advertised relay name.
 * - WebSocket round-trip: outbound Workers websocket via fetch upgrade
 *   (the same proven path as `workersFetchRelayConnector` in
 *   forum-work-request-live-publisher.ts), send a REQ with a tight filter,
 *   and await EOSE within a budget. This is a real round-trip, not a typed
 *   placeholder: workerd supports outbound websockets through fetch
 *   upgrade, and this repo already publishes events over that path.
 *
 * The canonical relay URL is read from the `MARKET_RELAY_URL` env override
 * with the shared `DefaultForumWorkRequestRelayUrl` constant as the
 * default, so the #4863 custom-domain cutover only has to change config.
 *
 * Authority boundary: read-only monitoring evidence. Probe records and
 * transition events grant no relay-mutation, payout, settlement, or
 * public-claim authority.
 */
import { Effect, Schema as S } from 'effect'

import {
  workersFetchRelayConnector,
  type ForumWorkRequestRelayConnector,
  type ForumWorkRequestRelaySocket,
} from './forum-work-request-live-publisher'
import { DefaultForumWorkRequestRelayUrl } from './forum-work-requests'
import { parseJsonUnknown, recordFromUnknown } from './json-boundary'
import { logWorkerRouteError, unwrapEffectTryPromiseCause } from './observability'
import {
  currentEpochMillis,
  epochMillisToIsoTimestamp,
  isoTimestampAfterIso,
} from './runtime-primitives'

/** Probe every 5 minutes; the worker cron fires every minute. */
export const RELAY_HEALTH_PROBE_INTERVAL_MINUTES = 5

/** Probe history retention: 7 days of 5-minute probes. */
export const RELAY_HEALTH_PROBE_RETENTION_MS = 7 * 24 * 60 * 60 * 1000

/** Transition retention: 30 days of status-change events. */
export const RELAY_HEALTH_TRANSITION_RETENTION_MS = 30 * 24 * 60 * 60 * 1000

const DEFAULT_NIP11_TIMEOUT_MS = 10_000
const DEFAULT_WS_EOSE_TIMEOUT_MS = 10_000

/**
 * Canonical market relay URL: `MARKET_RELAY_URL` env override, defaulting
 * to the shared workers.dev constant. #4863 may move the relay to
 * relay.openagents.com; that lands here as a config value, not a code edit.
 */
export const canonicalMarketRelayUrl = (environment: unknown): string => {
  const override = (environment as { MARKET_RELAY_URL?: string })
    .MARKET_RELAY_URL

  return typeof override === 'string' && override.length > 0
    ? override
    : DefaultForumWorkRequestRelayUrl
}

export const RelayHealthStatus = S.Literals([
  'healthy',
  'degraded',
  'unhealthy',
])
export type RelayHealthStatus = typeof RelayHealthStatus.Type

export const RelayNip11ProbeOutcome = S.Literals([
  'ok',
  'http_error',
  'fetch_failed',
  'invalid_body',
])
export type RelayNip11ProbeOutcome = typeof RelayNip11ProbeOutcome.Type

export const RelayWsProbeOutcome = S.Literals([
  'eose_received',
  'connect_failed',
  'timeout',
  'closed_before_eose',
])
export type RelayWsProbeOutcome = typeof RelayWsProbeOutcome.Type

export class RelayNip11ProbeLeg extends S.Class<RelayNip11ProbeLeg>(
  'RelayNip11ProbeLeg',
)({
  httpStatus: S.NullOr(S.Number),
  latencyMs: S.NullOr(S.Number),
  outcome: RelayNip11ProbeOutcome,
  relayName: S.NullOr(S.String),
}) {}

export class RelayWsProbeLeg extends S.Class<RelayWsProbeLeg>(
  'RelayWsProbeLeg',
)({
  latencyMs: S.NullOr(S.Number),
  outcome: RelayWsProbeOutcome,
}) {}

export class RelayHealthProbeRecord extends S.Class<RelayHealthProbeRecord>(
  'RelayHealthProbeRecord',
)({
  id: S.String,
  nip11: RelayNip11ProbeLeg,
  probedAt: S.String,
  relayUrl: S.String,
  status: RelayHealthStatus,
  ws: RelayWsProbeLeg,
}) {}

export const RelayHealthTransitionKind = S.Literals([
  'relay_health.transition.unhealthy',
  'relay_health.transition.degraded',
  'relay_health.transition.recovered',
])
export type RelayHealthTransitionKind = typeof RelayHealthTransitionKind.Type

/**
 * Typed status-transition event. Emitted exactly when consecutive retained
 * probes disagree on status; the kind names the destination so a future
 * alerting hook can subscribe to `relay_health.transition.unhealthy`
 * without re-deriving status pairs.
 */
export class RelayHealthTransitionEvent extends S.Class<RelayHealthTransitionEvent>(
  'RelayHealthTransitionEvent',
)({
  fromStatus: RelayHealthStatus,
  id: S.String,
  kind: RelayHealthTransitionKind,
  occurredAt: S.String,
  probeId: S.String,
  relayUrl: S.String,
  toStatus: RelayHealthStatus,
}) {}

/** Both legs ok -> healthy; one ok -> degraded; neither -> unhealthy. */
export const relayHealthStatusFromLegs = (
  nip11: RelayNip11ProbeLeg,
  ws: RelayWsProbeLeg,
): RelayHealthStatus => {
  const nip11Ok = nip11.outcome === 'ok'
  const wsOk = ws.outcome === 'eose_received'

  return nip11Ok && wsOk ? 'healthy' : nip11Ok || wsOk ? 'degraded' : 'unhealthy'
}

const transitionKindFor = (
  toStatus: RelayHealthStatus,
): RelayHealthTransitionKind =>
  toStatus === 'healthy'
    ? 'relay_health.transition.recovered'
    : toStatus === 'degraded'
      ? 'relay_health.transition.degraded'
      : 'relay_health.transition.unhealthy'

/**
 * Pure transition derivation: a typed event when the new probe's status
 * differs from the previous retained status, otherwise null. The first
 * probe ever recorded establishes a baseline without emitting.
 */
export const relayHealthTransitionEvent = (input: Readonly<{
  makeId: () => string
  previousStatus: RelayHealthStatus | null
  probe: RelayHealthProbeRecord
}>): RelayHealthTransitionEvent | null =>
  input.previousStatus === null || input.previousStatus === input.probe.status
    ? null
    : new RelayHealthTransitionEvent({
        fromStatus: input.previousStatus,
        id: input.makeId(),
        kind: transitionKindFor(input.probe.status),
        occurredAt: input.probe.probedAt,
        probeId: input.probe.id,
        relayUrl: input.probe.relayUrl,
        toStatus: input.probe.status,
      })

export type RelayHealthFetch = (
  url: string,
  init: Readonly<{ headers: Record<string, string>; signal?: AbortSignal }>,
) => Promise<Readonly<{ ok: boolean; status: number; text: () => Promise<string> }>>

const relayHttpUrl = (relayUrl: string): string =>
  relayUrl.replace(/^ws(s?):\/\//i, 'http$1://')

/**
 * NIP-11 info-document leg: GET the relay URL over HTTP with
 * `Accept: application/nostr+json`, recording HTTP status, latency, and
 * the advertised relay name.
 */
export const probeRelayNip11 = async (input: Readonly<{
  fetchFn: RelayHealthFetch
  nowMs: () => number
  relayUrl: string
  timeoutMs?: number | undefined
}>): Promise<RelayNip11ProbeLeg> => {
  const startedAt = input.nowMs()

  let response: Awaited<ReturnType<RelayHealthFetch>>

  try {
    response = await input.fetchFn(relayHttpUrl(input.relayUrl), {
      headers: { Accept: 'application/nostr+json' },
      signal: AbortSignal.timeout(input.timeoutMs ?? DEFAULT_NIP11_TIMEOUT_MS),
    })
  } catch {
    return new RelayNip11ProbeLeg({
      httpStatus: null,
      latencyMs: input.nowMs() - startedAt,
      outcome: 'fetch_failed',
      relayName: null,
    })
  }

  const latencyMs = input.nowMs() - startedAt

  if (!response.ok) {
    return new RelayNip11ProbeLeg({
      httpStatus: response.status,
      latencyMs,
      outcome: 'http_error',
      relayName: null,
    })
  }

  try {
    const body = recordFromUnknown(parseJsonUnknown(await response.text()))

    if (body === undefined) {
      return new RelayNip11ProbeLeg({
        httpStatus: response.status,
        latencyMs,
        outcome: 'invalid_body',
        relayName: null,
      })
    }

    const name = body['name']

    return new RelayNip11ProbeLeg({
      httpStatus: response.status,
      latencyMs,
      outcome: 'ok',
      relayName: typeof name === 'string' ? name.slice(0, 120) : null,
    })
  } catch {
    return new RelayNip11ProbeLeg({
      httpStatus: response.status,
      latencyMs,
      outcome: 'invalid_body',
      relayName: null,
    })
  }
}

/**
 * WebSocket round-trip leg: connect (Workers fetch upgrade), send a REQ
 * with a tight filter, and await the matching EOSE within the budget.
 */
export const probeRelayWebsocketEose = (input: Readonly<{
  connect: ForumWorkRequestRelayConnector
  nowMs: () => number
  relayUrl: string
  subscriptionId: string
  timeoutMs?: number | undefined
}>): Promise<RelayWsProbeLeg> => {
  const timeoutMs = input.timeoutMs ?? DEFAULT_WS_EOSE_TIMEOUT_MS
  const startedAt = input.nowMs()

  return input.connect(input.relayUrl).then(
    socket =>
      new Promise<RelayWsProbeLeg>(resolve => {
        let settled = false

        const settle = (outcome: RelayWsProbeOutcome) => {
          if (settled) {
            return
          }
          settled = true
          clearTimeout(timeout)
          try {
            socket.close()
          } catch {
            // socket already closed
          }
          resolve(
            new RelayWsProbeLeg({
              latencyMs: input.nowMs() - startedAt,
              outcome,
            }),
          )
        }

        const timeout = setTimeout(() => {
          settle('timeout')
        }, timeoutMs)

        socket.addEventListener('message', event => {
          try {
            const parsed = parseJsonUnknown(String(event.data))

            if (
              Array.isArray(parsed) &&
              parsed[0] === 'EOSE' &&
              parsed[1] === input.subscriptionId
            ) {
              settle('eose_received')
            }
          } catch {
            // non-JSON frame; keep waiting for EOSE within the budget
          }
        })
        socket.addEventListener('error', () => {
          settle('closed_before_eose')
        })
        socket.addEventListener('close', () => {
          settle('closed_before_eose')
        })

        sendProbeReq(socket, input.subscriptionId, settle)
      }),
    () =>
      new RelayWsProbeLeg({
        latencyMs: input.nowMs() - startedAt,
        outcome: 'connect_failed',
      }),
  )
}

const sendProbeReq = (
  socket: ForumWorkRequestRelaySocket,
  subscriptionId: string,
  settle: (outcome: RelayWsProbeOutcome) => void,
): void => {
  try {
    socket.send(JSON.stringify(['REQ', subscriptionId, { limit: 1 }]))
  } catch {
    settle('closed_before_eose')
  }
}

export type RelayHealthProbeDependencies = Readonly<{
  connect?: ForumWorkRequestRelayConnector | undefined
  fetchFn?: RelayHealthFetch | undefined
  makeId: () => string
  /** Latency clock; injectable for tests, defaults to the runtime primitive. */
  nowMs: () => number
  /** Probe timestamp authority — the scheduled controller event time. */
  probedAtIso: string
  relayUrl: string
  nip11TimeoutMs?: number | undefined
  wsTimeoutMs?: number | undefined
}>

/** Run both probe legs and derive the typed probe record. */
export const executeRelayHealthProbe = async (
  dependencies: RelayHealthProbeDependencies,
): Promise<RelayHealthProbeRecord> => {
  const probeId = dependencies.makeId()
  const probedAt = dependencies.probedAtIso
  const [nip11, ws] = await Promise.all([
    probeRelayNip11({
      fetchFn: dependencies.fetchFn ?? (fetch as unknown as RelayHealthFetch),
      nowMs: dependencies.nowMs,
      relayUrl: dependencies.relayUrl,
      timeoutMs: dependencies.nip11TimeoutMs,
    }),
    probeRelayWebsocketEose({
      connect: dependencies.connect ?? workersFetchRelayConnector,
      nowMs: dependencies.nowMs,
      relayUrl: dependencies.relayUrl,
      subscriptionId: `relay-health-${probeId.slice(0, 16)}`,
      timeoutMs: dependencies.wsTimeoutMs,
    }),
  ])

  return new RelayHealthProbeRecord({
    id: probeId,
    nip11,
    probedAt,
    relayUrl: dependencies.relayUrl,
    status: relayHealthStatusFromLegs(nip11, ws),
    ws,
  })
}

export type RelayHealthStore = Readonly<{
  insertProbe: (record: RelayHealthProbeRecord) => Promise<void>
  insertTransition: (event: RelayHealthTransitionEvent) => Promise<void>
  listRecentProbes: (
    relayUrl: string,
    limit: number,
  ) => Promise<ReadonlyArray<RelayHealthProbeRecord>>
  listRecentTransitions: (
    relayUrl: string,
    limit: number,
  ) => Promise<ReadonlyArray<RelayHealthTransitionEvent>>
  pruneProbesBefore: (beforeIso: string) => Promise<void>
  pruneTransitionsBefore: (beforeIso: string) => Promise<void>
  readLatestProbe: (
    relayUrl: string,
  ) => Promise<RelayHealthProbeRecord | null>
}>

type RelayHealthProbeRow = Readonly<{
  id: string
  relay_url: string
  probed_at: string
  nip11_outcome: string
  nip11_http_status: number | null
  nip11_latency_ms: number | null
  nip11_relay_name: string | null
  ws_outcome: string
  ws_latency_ms: number | null
  status: string
}>

type RelayHealthTransitionRow = Readonly<{
  id: string
  relay_url: string
  occurred_at: string
  kind: string
  from_status: string
  to_status: string
  probe_id: string
}>

const probeRecordFromRow = (row: RelayHealthProbeRow): RelayHealthProbeRecord =>
  S.decodeUnknownSync(RelayHealthProbeRecord)({
    id: row.id,
    nip11: {
      httpStatus: row.nip11_http_status,
      latencyMs: row.nip11_latency_ms,
      outcome: row.nip11_outcome,
      relayName: row.nip11_relay_name,
    },
    probedAt: row.probed_at,
    relayUrl: row.relay_url,
    status: row.status,
    ws: {
      latencyMs: row.ws_latency_ms,
      outcome: row.ws_outcome,
    },
  })

const transitionEventFromRow = (
  row: RelayHealthTransitionRow,
): RelayHealthTransitionEvent =>
  S.decodeUnknownSync(RelayHealthTransitionEvent)({
    fromStatus: row.from_status,
    id: row.id,
    kind: row.kind,
    occurredAt: row.occurred_at,
    probeId: row.probe_id,
    relayUrl: row.relay_url,
    toStatus: row.to_status,
  })

export const makeD1RelayHealthStore = (db: D1Database): RelayHealthStore => ({
  insertProbe: async record => {
    await db
      .prepare(
        `INSERT INTO relay_health_probes
          (id, relay_url, probed_at, nip11_outcome, nip11_http_status,
           nip11_latency_ms, nip11_relay_name, ws_outcome, ws_latency_ms,
           status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        record.id,
        record.relayUrl,
        record.probedAt,
        record.nip11.outcome,
        record.nip11.httpStatus,
        record.nip11.latencyMs,
        record.nip11.relayName,
        record.ws.outcome,
        record.ws.latencyMs,
        record.status,
        record.probedAt,
      )
      .run()
  },

  insertTransition: async event => {
    await db
      .prepare(
        `INSERT INTO relay_health_transitions
          (id, relay_url, occurred_at, kind, from_status, to_status,
           probe_id, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        event.id,
        event.relayUrl,
        event.occurredAt,
        event.kind,
        event.fromStatus,
        event.toStatus,
        event.probeId,
        event.occurredAt,
      )
      .run()
  },

  listRecentProbes: async (relayUrl, limit) => {
    const result = await db
      .prepare(
        `SELECT *
           FROM relay_health_probes
          WHERE relay_url = ?
          ORDER BY probed_at DESC
          LIMIT ?`,
      )
      .bind(relayUrl, limit)
      .all<RelayHealthProbeRow>()

    return (result.results ?? []).map(probeRecordFromRow)
  },

  listRecentTransitions: async (relayUrl, limit) => {
    const result = await db
      .prepare(
        `SELECT *
           FROM relay_health_transitions
          WHERE relay_url = ?
          ORDER BY occurred_at DESC
          LIMIT ?`,
      )
      .bind(relayUrl, limit)
      .all<RelayHealthTransitionRow>()

    return (result.results ?? []).map(transitionEventFromRow)
  },

  pruneProbesBefore: async beforeIso => {
    await db
      .prepare(`DELETE FROM relay_health_probes WHERE probed_at < ?`)
      .bind(beforeIso)
      .run()
  },

  pruneTransitionsBefore: async beforeIso => {
    await db
      .prepare(`DELETE FROM relay_health_transitions WHERE occurred_at < ?`)
      .bind(beforeIso)
      .run()
  },

  readLatestProbe: async relayUrl => {
    const row = await db
      .prepare(
        `SELECT *
           FROM relay_health_probes
          WHERE relay_url = ?
          ORDER BY probed_at DESC
          LIMIT 1`,
      )
      .bind(relayUrl)
      .first<RelayHealthProbeRow>()

    return row === null ? null : probeRecordFromRow(row)
  },
})

/**
 * Cadence guard: the worker cron fires every minute; the probe runs only
 * on minutes aligned to the probe interval.
 */
export const relayHealthProbeDue = (scheduledTimeMs: number): boolean =>
  Math.floor(scheduledTimeMs / 60_000) % RELAY_HEALTH_PROBE_INTERVAL_MINUTES ===
  0

export type RelayHealthTickResult = Readonly<{
  probe: RelayHealthProbeRecord | null
  skippedReasonRef: string | null
  transition: RelayHealthTransitionEvent | null
}>

/**
 * One scheduled tick: cadence guard, probe both legs, retain the record,
 * emit + retain the typed transition event when the status changed, and
 * prune beyond the retention bounds. Time comes from the scheduled
 * controller event, never from raw Date reads in this module.
 */
export const runRelayHealthProbeTick = async (input: Readonly<{
  connect?: ForumWorkRequestRelayConnector | undefined
  fetchFn?: RelayHealthFetch | undefined
  makeId: () => string
  nowMs?: (() => number) | undefined
  relayUrl: string
  scheduledTimeMs: number
  store: RelayHealthStore
  nip11TimeoutMs?: number | undefined
  wsTimeoutMs?: number | undefined
}>): Promise<RelayHealthTickResult> => {
  if (!relayHealthProbeDue(input.scheduledTimeMs)) {
    return {
      probe: null,
      skippedReasonRef: 'relay_health.skipped.cadence_not_due',
      transition: null,
    }
  }

  const previous = await input.store.readLatestProbe(input.relayUrl)
  const probe = await executeRelayHealthProbe({
    connect: input.connect,
    fetchFn: input.fetchFn,
    makeId: input.makeId,
    nip11TimeoutMs: input.nip11TimeoutMs,
    nowMs: input.nowMs ?? currentEpochMillis,
    probedAtIso: epochMillisToIsoTimestamp(input.scheduledTimeMs),
    relayUrl: input.relayUrl,
    wsTimeoutMs: input.wsTimeoutMs,
  })

  await input.store.insertProbe(probe)

  const transition = relayHealthTransitionEvent({
    makeId: input.makeId,
    previousStatus: previous?.status ?? null,
    probe,
  })

  if (transition !== null) {
    await input.store.insertTransition(transition)
  }

  // Two independent retention prunes on two different tables. Isolate them
  // with Effect structured concurrency instead of a bare `Promise.all`: one
  // table's prune failing must not mask whether the OTHER table's prune
  // succeeded (both are self-healing — retried next tick — but each
  // failure should still be individually logged rather than silently lost
  // inside one generic tick-level rejection).
  const probedAtIso = probe.probedAt
  const pruneOutcomes = await Effect.runPromise(
    Effect.forEach(
      [
        {
          name: 'probes' as const,
          run: () =>
            input.store.pruneProbesBefore(
              isoTimestampAfterIso(probedAtIso, -RELAY_HEALTH_PROBE_RETENTION_MS),
            ),
        },
        {
          name: 'transitions' as const,
          run: () =>
            input.store.pruneTransitionsBefore(
              isoTimestampAfterIso(
                probedAtIso,
                -RELAY_HEALTH_TRANSITION_RETENTION_MS,
              ),
            ),
        },
      ],
      prune =>
        Effect.result(Effect.tryPromise(prune.run)).pipe(
          Effect.map(outcome => ({ name: prune.name, outcome })),
        ),
      { concurrency: 'unbounded' },
    ),
  )

  for (const { name, outcome } of pruneOutcomes) {
    if (outcome._tag === 'Failure') {
      logWorkerRouteError(
        'relay_health_prune_failed',
        unwrapEffectTryPromiseCause(outcome.failure),
        { table: name },
      )
    }
  }

  return { probe, skippedReasonRef: null, transition }
}
