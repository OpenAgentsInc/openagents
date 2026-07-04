import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { decodeFleetIntentRow } from '@openagentsinc/khala-sync-server'
import type {
  FleetIntentRow,
  ReadPendingFleetIntentsInput,
  SyncSql,
} from '@openagentsinc/khala-sync-server'

import {
  KHALA_SYNC_FLEET_INTENTS_PATH,
  KHALA_SYNC_FLEET_INTENTS_ROUTE_REF,
  handleKhalaSyncFleetIntents,
} from './khala-sync-fleet-intents-routes'

const FAKE_CONNECTION_STRING =
  'postgresql://user:secret@hyperdrive.local:5432/khala_sync_test'

const get = (query = '') =>
  new Request(
    `https://openagents.com${KHALA_SYNC_FLEET_INTENTS_PATH}${query}`,
  )

const intent = (id: number, overrides: Record<string, unknown> = {}): FleetIntentRow =>
  decodeFleetIntentRow({
    createdAt: '2026-07-04T15:20:11.412Z',
    desiredSlots: null,
    flagRef: null,
    id,
    intent: 'pause',
    mutationRef: `mutation:cg-1:c-1:${id}`,
    requestedByUserId: 'user-1',
    runId: 'fleet-run.pylon.supervisor.abc123',
    scope: 'scope.fleet_run.fleet-run.pylon.supervisor.abc123',
    workerId: null,
    ...overrides,
  })

const run = (
  input: Readonly<{
    authorized?: boolean
    binding?: { connectionString: string } | undefined
    request?: Request
    intents?: ReadonlyArray<FleetIntentRow>
    readError?: Error
    factoryError?: Error
  }> = {},
) => {
  const reads: Array<ReadPendingFleetIntentsInput> = []
  let ended = 0
  const response = Effect.runPromise(
    handleKhalaSyncFleetIntents(input.request ?? get(), {
      binding:
        'binding' in input
          ? input.binding
          : { connectionString: FAKE_CONNECTION_STRING },
      makeSqlClient: connectionString => {
        expect(connectionString).toBe(FAKE_CONNECTION_STRING)
        if (input.factoryError !== undefined) {
          return Promise.reject(input.factoryError)
        }
        return Promise.resolve({
          end: () => {
            ended += 1
            return Promise.resolve()
          },
          sql: {} as SyncSql,
        })
      },
      readPendingFleetIntents: (_sql, readInput) => {
        reads.push(readInput)
        if (input.readError !== undefined) {
          return Promise.reject(input.readError)
        }
        return Promise.resolve(input.intents ?? [])
      },
      requireOperator: () => Promise.resolve(input.authorized ?? true),
    }),
  )
  return { endedCount: () => ended, reads, response }
}

describe('handleKhalaSyncFleetIntents', () => {
  test('rejects non-GET methods', async () => {
    const { response } = run({
      request: new Request(
        `https://openagents.com${KHALA_SYNC_FLEET_INTENTS_PATH}`,
        { method: 'POST' },
      ),
    })
    expect((await response).status).toBe(405)
  })

  test('requires the admin bearer', async () => {
    const { reads, response } = run({ authorized: false })
    const result = await response
    expect(result.status).toBe(401)
    expect(reads).toHaveLength(0)
  })

  test('binding absent: honest ok:false enablement gap, no read attempted', async () => {
    const { reads, response } = run({ binding: undefined })
    const result = await response
    expect(result.status).toBe(200)
    const body = (await result.json()) as { ok: boolean; reason: string }
    expect(body.ok).toBe(false)
    expect(body.reason).toContain('KHALA_SYNC_DB')
    expect(reads).toHaveLength(0)
  })

  test('invalid scope / after / limit are typed 400s before any read', async () => {
    for (const query of [
      '?scope=scope.user.u1',
      '?scope=not-a-scope',
      '?after=-1',
      '?after=abc',
      '?limit=0',
      '?limit=nope',
    ]) {
      const { reads, response } = run({ request: get(query) })
      const result = await response
      expect(result.status).toBe(400)
      expect(reads).toHaveLength(0)
    }
  })

  test('returns intents oldest-first with a nextAfter watermark', async () => {
    const rows = [
      intent(11),
      intent(12, { desiredSlots: 4, intent: 'set_desired_slots' }),
      intent(13, { intent: 'pause_worker', workerId: 'dispatch-context.x.1' }),
    ]
    const { endedCount, reads, response } = run({
      intents: rows,
      request: get(
        '?scope=scope.fleet_run.fleet-run.pylon.supervisor.abc123&after=10&limit=3',
      ),
    })
    const result = await response
    expect(result.status).toBe(200)
    const body = (await result.json()) as {
      ok: boolean
      intents: Array<{ id: number; intent: string; workerId: string | null }>
      nextAfter: number
      upToDate: boolean
      routeRef: string
    }
    expect(body.ok).toBe(true)
    expect(body.routeRef).toBe(KHALA_SYNC_FLEET_INTENTS_ROUTE_REF)
    expect(body.intents.map(i => i.id)).toEqual([11, 12, 13])
    expect(body.intents[2]!.intent).toBe('pause_worker')
    expect(body.intents[2]!.workerId).toBe('dispatch-context.x.1')
    expect(body.nextAfter).toBe(13)
    // Page filled to the limit ⇒ there may be more.
    expect(body.upToDate).toBe(false)
    expect(reads).toEqual([
      {
        afterId: 10,
        limit: 3,
        scope: 'scope.fleet_run.fleet-run.pylon.supervisor.abc123',
      },
    ])
    expect(endedCount()).toBe(1)
  })

  test('empty page: nextAfter stays at the requested watermark, upToDate true', async () => {
    const { response } = run({ intents: [], request: get('?after=42') })
    const result = await response
    const body = (await result.json()) as {
      nextAfter: number
      upToDate: boolean
      ok: boolean
    }
    expect(body.ok).toBe(true)
    expect(body.nextAfter).toBe(42)
    expect(body.upToDate).toBe(true)
  })

  test('storage failure: 503 without echoing driver detail; client still torn down', async () => {
    const { endedCount, response } = run({
      readError: new Error(
        `connect ECONNREFUSED at ${FAKE_CONNECTION_STRING}`,
      ),
    })
    const result = await response
    expect(result.status).toBe(503)
    const text = await result.text()
    expect(text).not.toContain('secret')
    expect(text).not.toContain('hyperdrive.local')
    expect(endedCount()).toBe(1)
  })

  test('client factory failure: 503 without echoing driver detail', async () => {
    const { response } = run({
      factoryError: new Error(`auth failed for ${FAKE_CONNECTION_STRING}`),
    })
    const result = await response
    expect(result.status).toBe(503)
    expect(await result.text()).not.toContain('secret')
  })
})
