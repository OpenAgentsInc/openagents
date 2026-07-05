import { readFileSync } from 'node:fs'
import { describe, expect, test } from 'vitest'

import type {
  ForumWorkRequestRelayConnector,
  ForumWorkRequestRelaySocket,
} from './forum-work-request-live-publisher'
import {
  RELAY_HEALTH_PROBE_INTERVAL_MINUTES,
  RELAY_HEALTH_PROBE_RETENTION_MS,
  RELAY_HEALTH_TRANSITION_RETENTION_MS,
  RelayHealthProbeRecord,
  RelayNip11ProbeLeg,
  RelayWsProbeLeg,
  canonicalMarketRelayUrl,
  probeRelayNip11,
  probeRelayWebsocketEose,
  relayHealthProbeDue,
  relayHealthStatusFromLegs,
  relayHealthTransitionEvent,
  runRelayHealthProbeTick,
  type RelayHealthFetch,
  type RelayHealthStore,
  type RelayHealthTransitionEvent,
} from './relay-health'

const RELAY_URL = 'wss://relay.openagents.com'

const tickingClock = (startMs = 1000, stepMs = 25) => {
  let now = startMs

  return () => {
    now += stepMs

    return now
  }
}

const sequentialIds = (prefix: string) => {
  let counter = 0

  return () => `${prefix}-${(counter += 1)}`
}

const nip11Leg = (outcome: RelayNip11ProbeLeg['outcome']) =>
  new RelayNip11ProbeLeg({
    httpStatus: outcome === 'ok' ? 200 : outcome === 'http_error' ? 530 : null,
    latencyMs: 12,
    outcome,
    relayName: outcome === 'ok' ? 'Scoped Market Relay' : null,
  })

const wsLeg = (outcome: RelayWsProbeLeg['outcome']) =>
  new RelayWsProbeLeg({
    latencyMs: 34,
    outcome,
  })

const probeRecord = (
  status: RelayHealthProbeRecord['status'],
  overrides: Partial<{ id: string; probedAt: string }> = {},
) =>
  new RelayHealthProbeRecord({
    id: overrides.id ?? 'probe-1',
    nip11: nip11Leg(status === 'healthy' ? 'ok' : 'http_error'),
    probedAt: overrides.probedAt ?? '2026-06-12T20:35:00.000Z',
    relayUrl: RELAY_URL,
    status,
    ws: wsLeg(status === 'unhealthy' ? 'connect_failed' : 'eose_received'),
  })

const okNip11Fetch: RelayHealthFetch = async () => ({
  ok: true,
  status: 200,
  text: async () => JSON.stringify({ name: 'Scoped Market Relay' }),
})

type FakeSocketBehavior = 'eose' | 'silent' | 'close_after_req'

const makeFakeRelayConnector = (
  behavior: FakeSocketBehavior,
  sentFrames: Array<string> = [],
): ForumWorkRequestRelayConnector => {
  const listeners: Record<
    'close' | 'error' | 'message',
    Array<(event: { data?: unknown }) => void>
  > = { close: [], error: [], message: [] }

  const socket: ForumWorkRequestRelaySocket = {
    addEventListener: (type, handler) => {
      listeners[type].push(handler)
    },
    close: () => {},
    send: data => {
      sentFrames.push(data)
      const parsed = JSON.parse(data) as ReadonlyArray<unknown>

      if (behavior === 'eose') {
        queueMicrotask(() => {
          for (const handler of listeners.message) {
            handler({ data: JSON.stringify(['EOSE', parsed[1]]) })
          }
        })
      }

      if (behavior === 'close_after_req') {
        queueMicrotask(() => {
          for (const handler of listeners.close) {
            handler({})
          }
        })
      }
    },
  }

  return async () => socket
}

const makeMemoryStore = () => {
  const probes: Array<RelayHealthProbeRecord> = []
  const transitions: Array<RelayHealthTransitionEvent> = []
  const pruneCalls: Array<{ kind: string; beforeIso: string }> = []
  const listLimits: Array<{ kind: string; limit: number }> = []

  const store: RelayHealthStore = {
    insertProbe: async record => {
      probes.unshift(record)
    },
    insertTransition: async event => {
      transitions.unshift(event)
    },
    listRecentProbes: async (_relayUrl, limit) => {
      listLimits.push({ kind: 'probes', limit })

      return probes.slice(0, limit)
    },
    listRecentTransitions: async (_relayUrl, limit) => {
      listLimits.push({ kind: 'transitions', limit })

      return transitions.slice(0, limit)
    },
    pruneProbesBefore: async beforeIso => {
      pruneCalls.push({ beforeIso, kind: 'probes' })
    },
    pruneTransitionsBefore: async beforeIso => {
      pruneCalls.push({ beforeIso, kind: 'transitions' })
    },
    readLatestProbe: async () => probes[0] ?? null,
  }

  return { listLimits, probes, pruneCalls, store, transitions }
}

// 2026-06-12T20:35:00.000Z is minute-aligned to the 5-minute cadence.
const DUE_SCHEDULED_TIME_MS = Date.parse('2026-06-12T20:35:00.000Z')

describe('relay health migration shape', () => {
  test('migration 0176 creates the probe and transition tables with indexes', () => {
    const migration = readFileSync(
      new URL('../migrations/0176_relay_health_probes.sql', import.meta.url),
      'utf8',
    )

    expect(migration).toContain(
      'CREATE TABLE IF NOT EXISTS relay_health_probes',
    )
    expect(migration).toContain(
      'CREATE TABLE IF NOT EXISTS relay_health_transitions',
    )
    expect(migration).toContain('idx_relay_health_probes_relay_probed_at')
    expect(migration).toContain(
      'idx_relay_health_transitions_relay_occurred_at',
    )
    expect(migration).toContain('nip11_outcome TEXT NOT NULL')
    expect(migration).toContain('ws_outcome TEXT NOT NULL')
    expect(migration).toContain('status TEXT NOT NULL')
  })
})

describe('canonicalMarketRelayUrl', () => {
  test('defaults to the shared workers.dev market relay constant', () => {
    expect(canonicalMarketRelayUrl({})).toBe(RELAY_URL)
  })

  test('reads the MARKET_RELAY_URL override so #4863 lands as config', () => {
    expect(
      canonicalMarketRelayUrl({
        MARKET_RELAY_URL: 'wss://relay.openagents.com',
      }),
    ).toBe('wss://relay.openagents.com')
  })

  test('ignores an empty override', () => {
    expect(canonicalMarketRelayUrl({ MARKET_RELAY_URL: '' })).toBe(RELAY_URL)
  })
})

describe('relayHealthStatusFromLegs', () => {
  test('both legs ok is healthy', () => {
    expect(relayHealthStatusFromLegs(nip11Leg('ok'), wsLeg('eose_received'))).toBe(
      'healthy',
    )
  })

  test('one failing leg is degraded', () => {
    expect(
      relayHealthStatusFromLegs(nip11Leg('http_error'), wsLeg('eose_received')),
    ).toBe('degraded')
    expect(relayHealthStatusFromLegs(nip11Leg('ok'), wsLeg('timeout'))).toBe(
      'degraded',
    )
  })

  test('both legs failing is unhealthy', () => {
    expect(
      relayHealthStatusFromLegs(nip11Leg('fetch_failed'), wsLeg('connect_failed')),
    ).toBe('unhealthy')
  })
})

describe('relayHealthTransitionEvent', () => {
  test('first probe establishes a baseline without emitting', () => {
    expect(
      relayHealthTransitionEvent({
        makeId: sequentialIds('transition'),
        previousStatus: null,
        probe: probeRecord('healthy'),
      }),
    ).toBeNull()
  })

  test('unchanged status emits nothing', () => {
    expect(
      relayHealthTransitionEvent({
        makeId: sequentialIds('transition'),
        previousStatus: 'healthy',
        probe: probeRecord('healthy'),
      }),
    ).toBeNull()
  })

  test('healthy to unhealthy emits the typed unhealthy transition', () => {
    const probe = probeRecord('unhealthy', { id: 'probe-down' })
    const event = relayHealthTransitionEvent({
      makeId: () => 'transition-1',
      previousStatus: 'healthy',
      probe,
    })

    expect(event).not.toBeNull()
    expect(event?.kind).toBe('relay_health.transition.unhealthy')
    expect(event?.fromStatus).toBe('healthy')
    expect(event?.toStatus).toBe('unhealthy')
    expect(event?.probeId).toBe('probe-down')
    expect(event?.occurredAt).toBe(probe.probedAt)
  })

  test('recovery emits the typed recovered transition', () => {
    const event = relayHealthTransitionEvent({
      makeId: () => 'transition-2',
      previousStatus: 'unhealthy',
      probe: probeRecord('healthy'),
    })

    expect(event?.kind).toBe('relay_health.transition.recovered')
  })

  test('partial failure emits the typed degraded transition', () => {
    const event = relayHealthTransitionEvent({
      makeId: () => 'transition-3',
      previousStatus: 'healthy',
      probe: probeRecord('degraded'),
    })

    expect(event?.kind).toBe('relay_health.transition.degraded')
  })
})

describe('probeRelayNip11', () => {
  test('records status, latency, and relay name on success', async () => {
    const requested: Array<{ headers: Record<string, string>; url: string }> =
      []
    const fetchFn: RelayHealthFetch = async (url, init) => {
      requested.push({ headers: init.headers, url })

      return okNip11Fetch(url, init)
    }

    const leg = await probeRelayNip11({
      fetchFn,
      nowMs: tickingClock(),
      relayUrl: RELAY_URL,
    })

    expect(leg.outcome).toBe('ok')
    expect(leg.httpStatus).toBe(200)
    expect(leg.relayName).toBe('Scoped Market Relay')
    expect(leg.latencyMs).toBeGreaterThan(0)
    expect(requested[0]?.url).toBe(
      'https://relay.openagents.com',
    )
    expect(requested[0]?.headers).toEqual({
      Accept: 'application/nostr+json',
    })
  })

  test('records the HTTP status on an error response (the Orrery 530 case)', async () => {
    const leg = await probeRelayNip11({
      fetchFn: async () => ({
        ok: false,
        status: 530,
        text: async () => 'error code: 530',
      }),
      nowMs: tickingClock(),
      relayUrl: RELAY_URL,
    })

    expect(leg.outcome).toBe('http_error')
    expect(leg.httpStatus).toBe(530)
    expect(leg.relayName).toBeNull()
  })

  test('records fetch_failed when the request throws', async () => {
    const leg = await probeRelayNip11({
      fetchFn: async () => {
        throw new Error('connection refused')
      },
      nowMs: tickingClock(),
      relayUrl: RELAY_URL,
    })

    expect(leg.outcome).toBe('fetch_failed')
    expect(leg.httpStatus).toBeNull()
  })

  test('records invalid_body when the info document is not a JSON record', async () => {
    const leg = await probeRelayNip11({
      fetchFn: async () => ({
        ok: true,
        status: 200,
        text: async () => 'not json',
      }),
      nowMs: tickingClock(),
      relayUrl: RELAY_URL,
    })

    expect(leg.outcome).toBe('invalid_body')
    expect(leg.httpStatus).toBe(200)
  })
})

describe('probeRelayWebsocketEose', () => {
  test('sends a tight REQ and resolves on the matching EOSE', async () => {
    const sentFrames: Array<string> = []

    const leg = await probeRelayWebsocketEose({
      connect: makeFakeRelayConnector('eose', sentFrames),
      nowMs: tickingClock(),
      relayUrl: RELAY_URL,
      subscriptionId: 'relay-health-test',
    })

    expect(leg.outcome).toBe('eose_received')
    expect(leg.latencyMs).toBeGreaterThan(0)
    expect(JSON.parse(sentFrames[0] ?? '')).toEqual([
      'REQ',
      'relay-health-test',
      { limit: 1 },
    ])
  })

  test('records connect_failed when the upgrade is refused', async () => {
    const leg = await probeRelayWebsocketEose({
      connect: async () => {
        throw new Error('relay refused websocket upgrade')
      },
      nowMs: tickingClock(),
      relayUrl: RELAY_URL,
      subscriptionId: 'relay-health-test',
    })

    expect(leg.outcome).toBe('connect_failed')
  })

  test('records timeout when no EOSE arrives within the budget', async () => {
    const leg = await probeRelayWebsocketEose({
      connect: makeFakeRelayConnector('silent'),
      nowMs: tickingClock(),
      relayUrl: RELAY_URL,
      subscriptionId: 'relay-health-test',
      timeoutMs: 10,
    })

    expect(leg.outcome).toBe('timeout')
  })

  test('records closed_before_eose when the relay drops the socket', async () => {
    const leg = await probeRelayWebsocketEose({
      connect: makeFakeRelayConnector('close_after_req'),
      nowMs: tickingClock(),
      relayUrl: RELAY_URL,
      subscriptionId: 'relay-health-test',
    })

    expect(leg.outcome).toBe('closed_before_eose')
  })
})

describe('relayHealthProbeDue', () => {
  test('is due exactly on interval-aligned minutes', () => {
    expect(RELAY_HEALTH_PROBE_INTERVAL_MINUTES).toBe(5)
    expect(relayHealthProbeDue(DUE_SCHEDULED_TIME_MS)).toBe(true)
    expect(relayHealthProbeDue(DUE_SCHEDULED_TIME_MS + 60_000)).toBe(false)
    expect(relayHealthProbeDue(DUE_SCHEDULED_TIME_MS + 4 * 60_000)).toBe(false)
    expect(relayHealthProbeDue(DUE_SCHEDULED_TIME_MS + 5 * 60_000)).toBe(true)
  })
})

describe('runRelayHealthProbeTick', () => {
  test('skips with a typed reason when the cadence is not due', async () => {
    const { probes, store } = makeMemoryStore()

    const result = await runRelayHealthProbeTick({
      connect: makeFakeRelayConnector('eose'),
      fetchFn: okNip11Fetch,
      makeId: sequentialIds('id'),
      nowMs: tickingClock(),
      relayUrl: RELAY_URL,
      scheduledTimeMs: DUE_SCHEDULED_TIME_MS + 60_000,
      store,
    })

    expect(result.skippedReasonRef).toBe(
      'relay_health.skipped.cadence_not_due',
    )
    expect(result.probe).toBeNull()
    expect(probes).toHaveLength(0)
  })

  test('records a healthy baseline probe without a transition and prunes retention', async () => {
    const { probes, pruneCalls, store, transitions } = makeMemoryStore()

    const result = await runRelayHealthProbeTick({
      connect: makeFakeRelayConnector('eose'),
      fetchFn: okNip11Fetch,
      makeId: sequentialIds('id'),
      nowMs: tickingClock(),
      relayUrl: RELAY_URL,
      scheduledTimeMs: DUE_SCHEDULED_TIME_MS,
      store,
    })

    expect(result.probe?.status).toBe('healthy')
    expect(result.probe?.probedAt).toBe('2026-06-12T20:35:00.000Z')
    expect(result.transition).toBeNull()
    expect(probes).toHaveLength(1)
    expect(transitions).toHaveLength(0)
    expect(pruneCalls).toEqual([
      {
        beforeIso: new Date(
          DUE_SCHEDULED_TIME_MS - RELAY_HEALTH_PROBE_RETENTION_MS,
        ).toISOString(),
        kind: 'probes',
      },
      {
        beforeIso: new Date(
          DUE_SCHEDULED_TIME_MS - RELAY_HEALTH_TRANSITION_RETENTION_MS,
        ).toISOString(),
        kind: 'transitions',
      },
    ])
  })

  // Regression coverage for the #8282 Promise.all landmine audit
  // (docs/2026-07-05-promise-all-cron-landmine-audit.md, Lane 1 #2): one
  // retention table's prune failing must not mask whether the OTHER
  // table's prune succeeded, and must not make the whole tick throw.
  test('one prune table failing does not abort the tick or the sibling prune', async () => {
    const { pruneCalls, store } = makeMemoryStore()
    const guardedStore: RelayHealthStore = {
      ...store,
      pruneProbesBefore: async () => {
        throw new Error('D1 prune failure (simulated)')
      },
    }

    const result = await runRelayHealthProbeTick({
      connect: makeFakeRelayConnector('eose'),
      fetchFn: okNip11Fetch,
      makeId: sequentialIds('id'),
      nowMs: tickingClock(),
      relayUrl: RELAY_URL,
      scheduledTimeMs: DUE_SCHEDULED_TIME_MS,
      store: guardedStore,
    })

    // Must not throw: the probes-table prune failure is isolated and
    // logged, not left to abort the whole tick.
    expect(result.probe?.status).toBe('healthy')
    // The sibling transitions-table prune still ran.
    expect(pruneCalls).toEqual([
      {
        beforeIso: new Date(
          DUE_SCHEDULED_TIME_MS - RELAY_HEALTH_TRANSITION_RETENTION_MS,
        ).toISOString(),
        kind: 'transitions',
      },
    ])
  })

  test('emits and retains typed transitions on failure and recovery', async () => {
    const { store, transitions } = makeMemoryStore()
    const makeId = sequentialIds('id')

    await runRelayHealthProbeTick({
      connect: makeFakeRelayConnector('eose'),
      fetchFn: okNip11Fetch,
      makeId,
      nowMs: tickingClock(),
      relayUrl: RELAY_URL,
      scheduledTimeMs: DUE_SCHEDULED_TIME_MS,
      store,
    })

    const outage = await runRelayHealthProbeTick({
      connect: async () => {
        throw new Error('relay refused websocket upgrade')
      },
      fetchFn: async () => ({
        ok: false,
        status: 530,
        text: async () => 'error code: 530',
      }),
      makeId,
      nowMs: tickingClock(),
      relayUrl: RELAY_URL,
      scheduledTimeMs: DUE_SCHEDULED_TIME_MS + 5 * 60_000,
      store,
    })

    expect(outage.probe?.status).toBe('unhealthy')
    expect(outage.transition?.kind).toBe('relay_health.transition.unhealthy')
    expect(outage.transition?.fromStatus).toBe('healthy')

    const recovery = await runRelayHealthProbeTick({
      connect: makeFakeRelayConnector('eose'),
      fetchFn: okNip11Fetch,
      makeId,
      nowMs: tickingClock(),
      relayUrl: RELAY_URL,
      scheduledTimeMs: DUE_SCHEDULED_TIME_MS + 10 * 60_000,
      store,
    })

    expect(recovery.transition?.kind).toBe('relay_health.transition.recovered')
    expect(recovery.transition?.fromStatus).toBe('unhealthy')
    expect(recovery.transition?.toStatus).toBe('healthy')
    expect(transitions).toHaveLength(2)
  })
})
