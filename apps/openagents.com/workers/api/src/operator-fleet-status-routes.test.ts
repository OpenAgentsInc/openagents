import { describe, expect, test } from 'vitest'

import {
  handleOperatorFleetStatusApi,
  readOperatorFleetStatusSnapshot,
} from './operator-fleet-status-routes'

type QueryResult = ReadonlyArray<Record<string, unknown>>

const nowMs = Date.parse('2026-06-27T18:00:00.000Z')

const fakeDb = (tables: {
  alerts?: QueryResult
  assignments?: QueryResult
  loops?: QueryResult
  pace10m?: number
  pace60m?: number
  pylons?: QueryResult
}): D1Database =>
  ({
    prepare: (query: string) => {
      const state: { args: ReadonlyArray<unknown> } = { args: [] }
      return {
        all: async () => {
          if (query.includes('FROM pylon_api_registrations')) {
            return { results: tables.pylons ?? [] }
          }
          if (query.includes('FROM pylon_api_assignments')) {
            return { results: tables.assignments ?? [] }
          }
          if (query.includes('FROM fleet_alerts')) {
            return { results: tables.alerts ?? [] }
          }
          if (query.includes('FROM artanis_loop_records')) {
            return { results: tables.loops ?? [] }
          }
          return { results: [] }
        },
        bind: (...args: ReadonlyArray<unknown>) => {
          state.args = args
          return {
            all: async () => {
              if (query.includes('FROM pylon_api_assignments')) {
                return { results: tables.assignments ?? [] }
              }
              return { results: [] }
            },
            first: async () => {
              const since = String(state.args[0] ?? '')
              return {
                total_tokens: since.includes('17:50')
                  ? (tables.pace10m ?? 0)
                  : (tables.pace60m ?? 0),
              }
            },
          }
        },
        first: async () => ({ total_tokens: 0 }),
      }
    },
  }) as unknown as D1Database

const env = (db: D1Database) =>
  ({
    FIREWORKS_API_KEY: '',
    HYDRALISK_GLM_52_REAP_504B_ENABLED: '',
    OPENAGENTS_DB: db,
    OPENROUTER_API_KEY: '',
    VERTEX_SA_KEY: '',
  }) as never

describe('GET /api/operator/fleet/status', () => {
  test('aggregates pace, fleet, watchdog, GLM, and Brain into one cached public-safe snapshot', async () => {
    const db = fakeDb({
      alerts: [
        {
          active_assignments: 1,
          alert_ref: 'alert.public.fleet.stall',
          classification: 'stalled',
          detected_at: '2026-06-27T17:58:00.000Z',
          queued_assignments: 1,
          reason_ref: 'reason.public.fleet.no_burn',
          recovered_lease_count: 0,
        },
      ],
      assignments: [
        {
          assignment_ref: 'assignment.public.issue6427',
          coding_assignment_json: JSON.stringify({ accountRef: 'codex-2' }),
          created_at: '2026-06-27T17:55:00.000Z',
          lease_expires_at: '2026-06-27T18:30:00.000Z',
          pylon_ref: 'pylon.public.owner-fleet',
          state: 'running',
          updated_at: '2026-06-27T17:56:00.000Z',
        },
      ],
      loops: [
        {
          public_projection_json: JSON.stringify({
            loopRef: 'loop.public.artanis.current',
          }),
          record_ref: 'loop.public.artanis.current',
          state: 'running',
          updated_at: '2026-06-27T17:59:00.000Z',
        },
      ],
      pace10m: 120,
      pace60m: 720,
      pylons: [
        {
          latest_capacity_refs_json: JSON.stringify([
            'capacity.coding.codex.available=2',
            'capacity.coding.codex.ready=3',
            'load.coding.codex.busy=1',
            'load.coding.codex.queued=0',
          ]),
          latest_heartbeat_at: '2026-06-27T17:59:30.000Z',
          pylon_ref: 'pylon.public.owner-fleet',
        },
      ],
    })

    const snapshot = await readOperatorFleetStatusSnapshot(env(db), nowMs)

    expect(snapshot.cache.maxAgeSeconds).toBe(10)
    expect(snapshot.pace.tokensLast10m).toBe(120)
    expect(snapshot.pace.tokensPerMinute10m).toBe(12)
    expect(snapshot.fleet.activeSlots).toBe(1)
    expect(snapshot.fleet.totalSlots).toBe(3)
    expect(snapshot.fleet.accountSpread).toEqual([
      { accountRef: 'codex-2', inFlight: 1 },
    ])
    expect(snapshot.fleet.inFlightAssignments[0]).toMatchObject({
      assignmentRef: 'assignment.public.issue6427',
      elapsedSeconds: 300,
      pylonRef: 'pylon.public.owner-fleet',
      state: 'running',
    })
    expect(snapshot.watchdog.state).toBe('STALLED')
    expect(snapshot.glm.readiness).toBe('unavailable')
    expect(snapshot.brain.loopHealth).toBe('active')

    const serialized = JSON.stringify(snapshot)
    expect(serialized).not.toContain('github:owner')
    expect(serialized).not.toContain('owner_agent_user_id')
    expect(serialized).not.toContain('rawEvents')
    expect(serialized).not.toContain('/Users/')
    expect(serialized).not.toContain('secret')
  })

  test('sets the required 10 second cache header', async () => {
    const response = await handleOperatorFleetStatusApi(
      new Request('https://openagents.com/api/operator/fleet/status'),
      env(fakeDb({})),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('public, max-age=10')
  })

  test('rejects non-GET methods without caching', async () => {
    const response = await handleOperatorFleetStatusApi(
      new Request('https://openagents.com/api/operator/fleet/status', {
        method: 'POST',
      }),
      env(fakeDb({})),
    )

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET')
    expect(response.headers.get('cache-control')).toBe('no-store')
  })
})
