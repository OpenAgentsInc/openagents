import { PublicKhalaTokensServedAggregate } from '@openagentsinc/sync-schema'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { handleOperatorFleetStatusApi } from './operator-fleet-status-routes'

const json = async <A>(response: Response): Promise<A> =>
  (await response.json()) as A

const okLedger = {
  ingestEvent: () => Effect.die('unused'),
  readAggregates: () => Effect.die('unused'),
  readInferenceAnalytics: () => Effect.die('unused'),
  readLeaderboardPreference: () => Effect.die('unused'),
  readLeaderboards: () => Effect.die('unused'),
  readPublicTokensServed: () =>
    Effect.succeed(PublicKhalaTokensServedAggregate.make({ tokensServed: 1200 })),
  readPublicTokensServedHistory: () =>
    Effect.succeed({
      bucket: 'day' as const,
      series: [
        { day: '2026-06-26', tokensServed: 100 },
        { day: '2026-06-27', tokensServed: 400 },
      ],
      timezone: 'America/Chicago',
      window: '7d' as const,
    }),
  readPublicTokensServedModelMix: () => Effect.die('unused'),
  updateLeaderboardPreference: () => Effect.die('unused'),
}

type PreparedResult = Readonly<{
  all?: () => Promise<{ results: ReadonlyArray<unknown> }>
  first?: () => Promise<unknown>
}>

const db = (onPrepare?: () => void): D1Database =>
  ({
    prepare: (sql: string) => {
      onPrepare?.()
      const result: PreparedResult = sql.includes('FROM pylon_api_registrations')
        ? {
            all: async () => ({
              results: [
                {
                  latest_heartbeat_at: '2026-06-27T12:29:00.000Z',
                  public_projection_json: JSON.stringify({
                    codingCapacity: [
                      {
                        available: 35,
                        busy: 13,
                        queued: 1,
                        ready: 48,
                        service: 'codex',
                      },
                    ],
                  }),
                  pylon_ref: 'pylon.public.owner',
                  status: 'active',
                },
              ],
            }),
          }
        : sql.includes('FROM pylon_api_assignments') && sql.includes('COUNT')
          ? { first: async () => ({ count: 1 }) }
          : sql.includes('FROM pylon_api_assignments')
            ? {
                all: async () => ({
                  results: [
                    {
                      assignment_ref: 'assignment.public.issue6427',
                      lease_expires_at: '2026-06-27T12:45:00.000Z',
                      pylon_ref: 'pylon.public.owner',
                      state: 'running',
                      updated_at: '2026-06-27T12:20:00.000Z',
                    },
                  ],
                }),
              }
            : sql.includes('FROM pylon_api_events')
              ? { first: async () => ({ created_at: '2026-06-27T12:29:30.000Z' }) }
              : sql.includes('FROM artanis_loop_records')
                ? {
                    all: async () => ({
                      results: [
                        {
                          kind: 'work_routing_proposal',
                          record_ref: 'proposal.public.artanis.issue6427',
                          state: 'active',
                          updated_at: '2026-06-27T12:28:00.000Z',
                        },
                      ],
                    }),
                  }
                : { all: async () => ({ results: [] }), first: async () => null }
      return {
        all: result.all ?? (async () => ({ results: [] })),
        bind: () => ({ all: result.all, first: result.first }),
        first: result.first ?? (async () => null),
      }
    },
  }) as unknown as D1Database

describe('GET /api/operator/fleet/status', () => {
  test('requires the operator token when configured', async () => {
    const response = await Effect.runPromise(
      handleOperatorFleetStatusApi(
        new Request('https://openagents.com/api/operator/fleet/status'),
        {
          requireAdminApiToken: async () => false,
        },
      ),
    )

    expect(response.status).toBe(401)
  })

  test('aggregates pace, fleet, watchdog, GLM, and brain blocks', async () => {
    const response = await Effect.runPromise(
      handleOperatorFleetStatusApi(
        new Request('https://openagents.com/api/operator/fleet/status'),
        {
          OPENAGENTS_DB: db(),
          glmStatusLoader: async () => ({
            readyReplicas: 2,
            status: 'ready',
            totalReplicas: 3,
            warmReplicas: 2,
          }),
          ledger: okLedger,
          nowIso: () => '2026-06-27T12:30:00.000Z',
          nowUnixMs: () => Date.parse('2026-06-27T12:30:00.000Z'),
          requireAdminApiToken: async () => true,
        },
      ),
    )

    expect(response.status).toBe(200)
    const body = await json<{
      brain: { available: boolean; lastDecisions: ReadonlyArray<unknown> }
      cache: { status: string }
      fleet: {
        activeSlots: { available: number; busy: number; queued: number; ready: number }
        inFlightAssignments: ReadonlyArray<{ assignmentRef: string }>
      }
      glm: { readyReplicas: number; status: string }
      pace: { allTimeTokensServed: number; pace: { todayTokens: number } }
      schemaVersion: string
      watchdog: { activeLeases: number; state: string }
    }>(response)

    expect(body.schemaVersion).toBe('openagents.operator_fleet_status.v1')
    expect(body.cache.status).toBe('miss')
    expect(body.pace.allTimeTokensServed).toBe(1200)
    expect(body.pace.pace.todayTokens).toBe(400)
    expect(body.fleet.activeSlots).toEqual({
      available: 35,
      busy: 13,
      queued: 1,
      ready: 48,
    })
    expect(body.fleet.inFlightAssignments[0]?.assignmentRef).toBe(
      'assignment.public.issue6427',
    )
    expect(body.watchdog).toMatchObject({ activeLeases: 1, state: 'HEALTHY' })
    expect(body.glm).toMatchObject({ readyReplicas: 2, status: 'ready' })
    expect(body.brain.available).toBe(true)
    expect(body.brain.lastDecisions).toHaveLength(1)
  })

  test('caches D1-backed snapshots for 10 seconds', async () => {
    let prepares = 0
    const input = {
      OPENAGENTS_DB: db(() => {
        prepares += 1
      }),
      glmStatusLoader: async () => ({
        readyReplicas: 1,
        status: 'ready',
        totalReplicas: 1,
        warmReplicas: 1,
      }),
      nowIso: () => '2026-06-27T12:30:00.000Z',
      nowUnixMs: () => Date.parse('2026-06-27T12:30:00.000Z'),
      requireAdminApiToken: async () => true,
    }

    const first = await Effect.runPromise(
      handleOperatorFleetStatusApi(
        new Request('https://openagents.com/api/operator/fleet/status'),
        input,
      ),
    )
    const second = await Effect.runPromise(
      handleOperatorFleetStatusApi(
        new Request('https://openagents.com/api/operator/fleet/status'),
        input,
      ),
    )

    expect(first.status).toBe(200)
    expect(second.status).toBe(200)
    expect((await json<{ cache: { status: string } }>(first)).cache.status).toBe(
      'miss',
    )
    expect((await json<{ cache: { status: string } }>(second)).cache.status).toBe(
      'hit',
    )
    expect(prepares).toBeGreaterThan(0)
  })
})
