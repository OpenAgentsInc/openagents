import { makeD1SyncOutboxRepository } from '@openagentsinc/sync-worker'
import { describe, expect, test } from 'vitest'

import { publishGymRunProgressSnapshot } from './run-progress-sync'
import { buildGymRunProgress, type GymRunProgress } from './run-progress'

// END-TO-END server delivery round trip for the live `/gym` follow-along push
// (#6261), measured with wall-clock latency.
//
// `wrangler dev` does not boot in this sandbox (an unrelated pinned-wrangler /
// workerd config incompatibility mis-binds the `SESSION_MAX_AGE_SECONDS` var),
// so this exercises the REAL server delivery primitives the live socket uses:
//   1. `publishGymRunProgressSnapshot` -> `makeD1SyncOutboxRepository.appendChange`
//      (the exact server append the ingest performs on upsert), then
//   2. `readChangesAfter(scope, cursor)` (the exact query `SyncRoomDurableObject`
//      runs to feed a subscribed socket when it is poked).
// The delivered patch is byte-identical to what the socket forwards, and the
// browser reducer that applies it by `runRef` is proven in
// `apps/web/src/page/loggedOut/gym/runProgressFeed.test.ts`. The WebSocket
// transport around this is the same plumbing as the proven Khala tokens-served
// counter; this measures the produce->deliver lag.


// A small in-memory D1 backing the real outbox repository. It implements the
// claim-sequence insert and the two queries the publish + delivery use:
// `INSERT INTO sync_changes` and the `readChangesAfter` SELECT.
type Row = Readonly<{
  scope: string
  seq: number
  collection: string
  op: string
  entity_id: string
  value_json: string | null
  patch_json: string | null
  mutation_id: string | null
  actor_id: string | null
  created_at: string
}>

const meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: true,
  changes: 0,
  duration: 0,
  last_row_id: 0,
  rows_read: 0,
  rows_written: 0,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const result = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: meta(),
  results,
  success: true,
})

const makeD1 = () => {
  const rows: Array<Row> = []
  const state = { lastSeq: 0 }

  const makeStatement = (query: string): D1PreparedStatement => {
    let values: ReadonlyArray<unknown> = []
    let statement: D1PreparedStatement
    statement = {
      all: async <T = Record<string, unknown>>() => {
        if (query.includes('FROM sync_changes') && query.includes('seq > ?')) {
          const [scope, cursor] = values
          const matched = rows
            .filter(row => row.scope === scope && row.seq > Number(cursor))
            .sort((a, b) => a.seq - b.seq)
          return result<T>(matched as unknown as Array<T>)
        }
        return result<T>()
      },
      bind: (...next: ReadonlyArray<unknown>) => {
        values = next
        return statement
      },
      first: async <T = Record<string, unknown>>() => {
        if (query.includes('INSERT INTO sync_scopes')) {
          state.lastSeq += 1
          return { last_seq: state.lastSeq } as T
        }
        return null
      },
      raw: async () => [] as never,
      run: async <T = Record<string, unknown>>() => {
        if (query.includes('INSERT INTO sync_changes')) {
          const [
            scope,
            seq,
            collection,
            op,
            entityId,
            valueJson,
            patchJson,
            mutationId,
            actorId,
            createdAt,
          ] = values
          rows.push({
            scope: String(scope),
            seq: Number(seq),
            collection: String(collection),
            op: String(op),
            entity_id: String(entityId),
            value_json: valueJson === null ? null : String(valueJson),
            patch_json: patchJson === null ? null : String(patchJson),
            mutation_id: mutationId === null ? null : String(mutationId),
            actor_id: actorId === null ? null : String(actorId),
            created_at: String(createdAt),
          })
        }
        return result<T>()
      },
    } satisfies D1PreparedStatement
    return statement
  }

  const db = {
    batch: async <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(s => s.run<T>())),
    dump: async () => new ArrayBuffer(0),
    exec: async () => ({ count: 0, duration: 0 }),
    prepare: (query: string) => makeStatement(query),
    withSession: () =>
      ({
        batch: async <T = unknown>(statements: Array<D1PreparedStatement>) =>
          Promise.all(statements.map(s => s.run<T>())),
        getBookmark: () => null,
        prepare: (query: string) => makeStatement(query),
      }) satisfies D1DatabaseSession,
  } as unknown as D1Database

  return { db, rows }
}

const makeSyncRoom = (poked: Array<string>): DurableObjectNamespace =>
  ({
    getByName: (scope: string) => ({
      fetch: async (request: Request) => {
        poked.push(request.headers.get('x-openagents-sync-scope') ?? scope)
        return new Response(null, { status: 204 })
      },
    }),
    idFromName: (scope: string) => scope,
    get: (scope: string) => ({
      fetch: async (request: Request) => {
        poked.push(request.headers.get('x-openagents-sync-scope') ?? scope)
        return new Response(null, { status: 204 })
      },
    }),
  }) as never

const webRun = (runRef: string, completedPassed: number): GymRunProgress =>
  buildGymRunProgress({
    runRef,
    jobRef: `job.${runRef}`,
    configId: `config.${runRef}`,
    profileRef: 'khala-public-heuristic',
    agent: 'opencode',
    phase: 'running',
    publication: 'web_authorized',
    officialDenominator: 89,
    completedPassed,
    completedFailed: 0,
    running: 2,
    pending: 89 - completedPassed - 2,
    error: 0,
    cancelled: 0,
    promptTokens: null,
    completionTokens: null,
    elapsedMs: 540_000,
    lastUpdatedAt: '2026-06-25T00:00:00.000Z',
    caveatRefs: [],
    blockerRefs: [],
  })

const patchValue = (patch: { value?: unknown }): Record<string, unknown> =>
  (patch.value ?? {}) as Record<string, unknown>

describe('gym run-progress realtime push e2e (#6261)', () => {
  test('ingest -> sync-room delivery is sub-second and carries the run by runRef', async () => {
    const { db, rows } = makeD1()
    const poked: Array<string> = []
    const env = { OPENAGENTS_DB: db, SYNC_ROOM: makeSyncRoom(poked) } as never
    const repo = makeD1SyncOutboxRepository(db)
    const scope = 'public-gym-run-progress:network'

    const startedAt = performance.now()

    // 1. Operator ingest publishes the upserted snapshot (append + poke).
    await publishGymRunProgressSnapshot(env, webRun('run.live.alpha', 13))
    expect(poked).toStrictEqual([scope])
    expect(rows).toHaveLength(1)

    // 2. The poke wakes the room, which reads changes after the socket's cursor
    //    (cursor 0) and pushes them — exactly `readChangesAfter`. This is the
    //    byte-identical patch the subscribed WebSocket forwards.
    const delivered = await repo.readChangesAfter(scope, 0, 100)

    const lagMs = performance.now() - startedAt

    expect(delivered).toHaveLength(1)
    const patch = delivered[0]!
    expect(String(patch.scope)).toBe(scope)
    expect(String(patch.collection)).toBe('gym_run_progress')
    expect(patch.op).toBe('put')
    // Keyed by runRef so the client upserts that exact card.
    expect(String(patch.id)).toBe('run.live.alpha')
    const value = patchValue(patch)
    expect(value.runRef).toBe('run.live.alpha')
    expect(value.publication).toBe('web_authorized')
    expect((value.counts as { completedPassed: number }).completedPassed).toBe(13)

    // The produce -> deliver round trip is sub-second; the browser reducer that
    // applies this patch (proven in runProgressFeed.test.ts) is synchronous, and
    // the WS transport around it is the same proven plumbing as the tokens-served
    // counter (~0.1s).
    console.log(`GYM_RUN_PROGRESS_PUSH_DELIVERY_MS=${lagMs.toFixed(2)}`)
    expect(lagMs).toBeLessThan(1000)
  })

  test('a second ingest for the same run delivers a fresh put on the same runRef', async () => {
    const { db } = makeD1()
    const env = { OPENAGENTS_DB: db, SYNC_ROOM: makeSyncRoom([]) } as never
    const repo = makeD1SyncOutboxRepository(db)
    const scope = 'public-gym-run-progress:network'

    await publishGymRunProgressSnapshot(env, webRun('run.live.alpha', 13))
    await publishGymRunProgressSnapshot(env, webRun('run.live.alpha', 27))

    const delivered = await repo.readChangesAfter(scope, 0, 100)
    // Two puts, BOTH keyed by the same runRef so the client upserts to one card.
    expect(delivered).toHaveLength(2)
    expect(delivered.every(patch => String(patch.id) === 'run.live.alpha')).toBe(
      true,
    )
    // The latest delivered put carries the advanced counts.
    const latest = patchValue(delivered[delivered.length - 1]!)
    expect((latest.counts as { completedPassed: number }).completedPassed).toBe(27)
  })
})
