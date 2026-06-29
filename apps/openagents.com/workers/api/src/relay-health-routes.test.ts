import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  RelayHealthProbeRecord,
  RelayHealthTransitionEvent,
  RelayNip11ProbeLeg,
  RelayWsProbeLeg,
  type RelayHealthStore,
} from './relay-health'
import { handlePublicRelayHealthApi } from './relay-health-routes'

const RELAY_URL = 'wss://relay.openagents.com'
const NOW_ISO = '2026-06-12T20:40:00.000Z'

const probeRecord = (
  status: RelayHealthProbeRecord['status'],
  probedAt: string,
  id = `probe-${probedAt}`,
) =>
  new RelayHealthProbeRecord({
    id,
    nip11: new RelayNip11ProbeLeg({
      httpStatus: status === 'healthy' ? 200 : 530,
      latencyMs: 42,
      outcome: status === 'healthy' ? 'ok' : 'http_error',
      relayName: status === 'healthy' ? 'Scoped Market Relay' : null,
    }),
    probedAt,
    relayUrl: RELAY_URL,
    status,
    ws: new RelayWsProbeLeg({
      latencyMs: status === 'healthy' ? 88 : null,
      outcome: status === 'healthy' ? 'eose_received' : 'connect_failed',
    }),
  })

const transitionEvent = (occurredAt: string) =>
  new RelayHealthTransitionEvent({
    fromStatus: 'healthy',
    id: `transition-${occurredAt}`,
    kind: 'relay_health.transition.unhealthy',
    occurredAt,
    probeId: `probe-${occurredAt}`,
    relayUrl: RELAY_URL,
    toStatus: 'unhealthy',
  })

const makeStore = (
  probes: ReadonlyArray<RelayHealthProbeRecord>,
  transitions: ReadonlyArray<RelayHealthTransitionEvent> = [],
) => {
  const listLimits: Array<{ kind: string; limit: number }> = []

  const store: RelayHealthStore = {
    insertProbe: async () => {},
    insertTransition: async () => {},
    listRecentProbes: async (_relayUrl, limit) => {
      listLimits.push({ kind: 'probes', limit })

      return probes.slice(0, limit)
    },
    listRecentTransitions: async (_relayUrl, limit) => {
      listLimits.push({ kind: 'transitions', limit })

      return transitions.slice(0, limit)
    },
    pruneProbesBefore: async () => {},
    pruneTransitionsBefore: async () => {},
    readLatestProbe: async () => probes[0] ?? null,
  }

  return { listLimits, store }
}

const getRelayHealth = async (
  store: RelayHealthStore,
  method = 'GET',
): Promise<{ body: Record<string, unknown>; status: number }> => {
  const response = await Effect.runPromise(
    handlePublicRelayHealthApi(
      new Request('https://openagents.com/api/public/relay-health', {
        method,
      }),
      {
        nowIso: () => NOW_ISO,
        relayUrl: RELAY_URL,
        store,
      },
    ),
  )

  return {
    body:
      response.status === 405
        ? {}
        : ((await response.json()) as Record<string, unknown>),
    status: response.status,
  }
}

describe('GET /api/public/relay-health', () => {
  test('rejects non-GET methods', async () => {
    const { store } = makeStore([])

    const { status } = await getRelayHealth(store, 'POST')

    expect(status).toBe(405)
  })

  test('serves current status, bounded history, transitions, and the staleness contract', async () => {
    const { listLimits, store } = makeStore(
      [
        probeRecord('healthy', '2026-06-12T20:35:00.000Z'),
        probeRecord('unhealthy', '2026-06-12T20:30:00.000Z'),
      ],
      [transitionEvent('2026-06-12T20:30:00.000Z')],
    )

    const { body, status } = await getRelayHealth(store)

    expect(status).toBe(200)
    expect(body['relayUrl']).toBe(RELAY_URL)
    expect(body['status']).toBe('healthy')
    expect(body['generatedAt']).toBe(NOW_ISO)
    expect(body['publicSafe']).toBe(true)
    expect(body['probeCadenceMinutes']).toBe(5)

    const staleness = body['staleness'] as Record<string, unknown>

    expect(staleness['composition']).toBe('stored_snapshot')
    expect(staleness['maxStalenessSeconds']).toBe(420)
    expect(staleness['rebuildsOn']).toEqual([
      'relay_health_probe_recorded',
      'relay_health_status_transition',
    ])

    // Latest probe is 5 minutes old at NOW_ISO: within the 7-minute bound.
    expect(body['staleExceeded']).toBe(false)

    const current = body['current'] as Record<string, unknown>

    expect(current['status']).toBe('healthy')
    expect(current['dataAgeSeconds']).toBe(300)
    expect(
      (current['nip11'] as Record<string, unknown>)['relayName'],
    ).toBe('Scoped Market Relay')
    expect((current['ws'] as Record<string, unknown>)['outcome']).toBe(
      'eose_received',
    )

    const history = body['history'] as Record<string, unknown>
    const historyProbes = history['probes'] as ReadonlyArray<unknown>

    expect(historyProbes).toHaveLength(2)
    expect(history['retentionPolicyRef']).toBe(
      'retention.relay_health.probes_7d_transitions_30d',
    )

    const transitions = body['transitions'] as ReadonlyArray<
      Record<string, unknown>
    >

    expect(transitions).toHaveLength(1)
    expect(transitions[0]?.['kind']).toBe('relay_health.transition.unhealthy')

    // Bounded reads: 24h of 5-minute probes, 50 transitions.
    expect(listLimits).toEqual([
      { kind: 'probes', limit: 288 },
      { kind: 'transitions', limit: 50 },
    ])
  })

  test('flags itself stale when the newest probe exceeds the declared bound', async () => {
    const { store } = makeStore([
      probeRecord('healthy', '2026-06-12T20:00:00.000Z'),
    ])

    const { body } = await getRelayHealth(store)

    expect(body['staleExceeded']).toBe(true)
  })

  test('serves an honest unknown state before the first probe', async () => {
    const { store } = makeStore([])

    const { body, status } = await getRelayHealth(store)

    expect(status).toBe(200)
    expect(body['status']).toBe('unknown')
    expect(body['current']).toBeNull()
    expect(body['staleExceeded']).toBe(false)
  })
})
