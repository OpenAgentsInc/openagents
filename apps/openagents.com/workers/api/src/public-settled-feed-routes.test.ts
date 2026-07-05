// KS-6.4 (#8414): route contract tests for GET /api/public/settled-feed.
// Covers both serving orders: the khala-sync projection (injected fake
// reader) and the fail-open legacy D1 sync-outbox fallback (a real D1
// fake driving the SAME `publishSettledFeedEvents` write path, then read
// back through the route's fallback), so the "no regression relative to
// today" guarantee has real test coverage, not just a happy-path stub.

import { Effect } from 'effect'
import { beforeEach, describe, expect, test } from 'vitest'

import {
  invalidateSettledFeedProjectionCacheForTests,
  SETTLED_FEED_PROJECTION_MAX_STALENESS_SECONDS,
} from './khala-sync-public-settled-feed'
import { handlePublicSettledFeedApi } from './public-settled-feed-routes'
import {
  buildSettledFeedEvents,
  publishSettledFeedEvents,
} from './tassadar-settled-feed-sync'

const nowIso = '2026-07-05T12:00:00.000Z'

beforeEach(() => {
  invalidateSettledFeedProjectionCacheForTests()
})

const getRequest = (query = ''): Request =>
  new Request(`https://openagents.com/api/public/settled-feed${query}`, {
    method: 'GET',
  })

describe('GET /api/public/settled-feed', () => {
  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handlePublicSettledFeedApi(
        new Request('https://openagents.com/api/public/settled-feed', {
          method: 'POST',
        }),
        {},
      ),
    )
    expect(response.status).toBe(405)
  })

  test('serves from the projection when it has events (rebuilt_on_transition staleness)', async () => {
    const projectedEvent = {
      amountSats: 5,
      challengeRef: 'challenge.tassadar.window.0001',
      contributorRef: 'pylon.worker.orrery',
      eventRef: 'settled.projected.0',
      party: 'worker' as const,
      runRef: 'run.tassadar.poc',
      settledAt: nowIso,
      totalSettledCount: 1,
      totalSettledSats: 5,
      windowRef: null,
    }
    const response = await Effect.runPromise(
      handlePublicSettledFeedApi(getRequest(), {
        KHALA_SYNC_DB: { connectionString: 'postgres://hyperdrive-fake' },
        nowIso: () => nowIso,
        projectionReadDeps: {
          makeSqlClient: async () => ({
            end: async () => undefined,
            sql: (() => undefined) as never,
          }),
          readProjection: async () => ({
            events: [projectedEvent],
            summary: {
              latestEventRef: projectedEvent.eventRef,
              latestSettledAt: nowIso,
              totalSettledCount: 1,
              totalSettledSats: 5,
              updatedAt: nowIso,
            },
          }),
        },
      }),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toContain('no-store')
    const body = (await response.json()) as Record<string, unknown>
    expect(body.schemaVersion).toBe('openagents.public_settled_feed.v1')
    expect(body.events).toEqual([projectedEvent])
    expect(body.summary).toMatchObject({ totalSettledCount: 1 })
    expect(body.staleness).toMatchObject({
      composition: 'rebuilt_on_transition',
      maxStalenessSeconds: SETTLED_FEED_PROJECTION_MAX_STALENESS_SECONDS,
    })
  })

  test('falls open to the legacy D1 sync-outbox snapshot when the projection is empty', async () => {
    const db = makeMemoryD1()
    const events = buildSettledFeedEvents({
      legs: [
        {
          amountSats: 5,
          challengeRef: 'challenge.tassadar.window.fallback',
          contributorRef: 'pylon.worker.orrery',
          party: 'worker',
          runRef: 'run.tassadar.poc',
          windowRef: 'window.tassadar.fallback',
        },
      ],
      priorCount: 0,
      priorSettledSats: 0,
      settledAt: nowIso,
    })

    // Seed the LEGACY D1 outbox the exact same way real settlement dispatch
    // does — no binding for the new khala-sync path, so the route MUST
    // fall back here (proving no availability regression).
    await publishSettledFeedEvents(
      { OPENAGENTS_DB: db, SYNC_ROOM: makeSyncRoom() },
      events,
    )

    const response = await Effect.runPromise(
      handlePublicSettledFeedApi(getRequest(), {
        OPENAGENTS_DB: db,
        nowIso: () => nowIso,
      }),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.staleness).toMatchObject({ composition: 'live_at_read' })
    const responseEvents = body.events as Array<Record<string, unknown>>
    expect(responseEvents).toHaveLength(1)
    expect(responseEvents[0]?.eventRef).toBe(events[0]?.eventRef)
    expect(body.summary).toMatchObject({ totalSettledCount: 1 })
  })

  test('serves an honest empty payload when neither path has data', async () => {
    const response = await Effect.runPromise(
      handlePublicSettledFeedApi(getRequest(), { nowIso: () => nowIso }),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.events).toEqual([])
    expect(body.summary).toBeNull()
  })
})

// ---------------------------------------------------------------------------
// Minimal in-memory D1 (append + snapshot read), same shape the legacy
// producer test file uses for `appendChange`, extended with `.all()` so
// `readSnapshot` (the fallback's read path) resolves real rows.
// ---------------------------------------------------------------------------

type StoredChange = Readonly<{
  actor_id: string | null
  collection: string
  created_at: string
  entity_id: string
  mutation_id: string | null
  op: 'put' | 'patch' | 'delete' | 'invalidate'
  patch_json: string | null
  scope: string
  seq: number
  value_json: string | null
}>

type MemoryD1 = D1Database & Readonly<{ changes: Array<StoredChange> }>

const d1Meta = (): D1Meta & Record<string, unknown> => ({
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

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

function rawD1Rows<T = unknown[]>(options: {
  columnNames: true
}): Promise<[Array<string>, ...Array<T>]>
function rawD1Rows<T = unknown[]>(options?: {
  columnNames?: false
}): Promise<Array<T>>
function rawD1Rows<T = unknown[]>(options?: {
  columnNames?: boolean
}): Promise<Array<T> | [Array<string>, ...Array<T>]> {
  return Promise.resolve(options?.columnNames === true ? [[]] : [])
}

const makeMemoryD1 = (): MemoryD1 => {
  const changes: Array<StoredChange> = []

  const makeStatement = (query: string): D1PreparedStatement => {
    let values: ReadonlyArray<unknown> = []
    let statement: D1PreparedStatement
    statement = {
      all: async <T = Record<string, unknown>>() => {
        if (query.includes('FROM sync_changes')) {
          const scope = String(values[0])
          const rows = changes
            .filter(change => change.scope === scope)
            .sort((a, b) => a.seq - b.seq)
          return makeResult<T>(rows as unknown as Array<T>)
        }
        return makeResult<T>()
      },
      bind: (...nextValues: ReadonlyArray<unknown>) => {
        values = nextValues
        return statement
      },
      first: async <T>() => {
        if (query.includes('INSERT INTO sync_scopes')) {
          return { last_seq: changes.length + 1 } as unknown as T
        }
        return null
      },
      raw: rawD1Rows,
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
          changes.push({
            actor_id: actorId === null ? null : String(actorId),
            collection: String(collection),
            created_at: String(createdAt),
            entity_id: String(entityId),
            mutation_id: mutationId === null ? null : String(mutationId),
            op: op as StoredChange['op'],
            patch_json: patchJson === null ? null : String(patchJson),
            scope: String(scope),
            seq: Number(seq),
            value_json: valueJson === null ? null : String(valueJson),
          })
        }
        return makeResult<T>()
      },
    } satisfies D1PreparedStatement
    return statement
  }

  return {
    changes,
    batch: async <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: async () => new ArrayBuffer(0),
    exec: async () => ({ count: 0, duration: 0 }),
    prepare: (query: string) => makeStatement(query),
    withSession: () =>
      ({
        batch: async <T = unknown>(statements: Array<D1PreparedStatement>) =>
          Promise.all(statements.map(statement => statement.run<T>())),
        getBookmark: () => null,
        prepare: (query: string) => makeStatement(query),
      }) satisfies D1DatabaseSession,
  } satisfies MemoryD1
}

const makeSyncRoom = (): DurableObjectNamespace =>
  // eslint-disable-next-line @typescript-eslint/consistent-type-assertions
  ({
    getByName: () => ({ fetch: async () => new Response(null, { status: 204 }) }),
    idFromName: (scope: string) => scope,
    get: () => ({ fetch: async () => new Response(null, { status: 204 }) }),
  }) as never
