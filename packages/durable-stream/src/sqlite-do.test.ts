// REAL SQLite Durable-Object adapter tests (#6154 tier 4 follow-up).
//
// WHY THIS FILE EXISTS — THE PROD GAP IT CLOSES:
// The conformance + idempotent + live suites all drive `MemoryStreamStore`,
// whose `getMeta()`/`getProducer()` return `null`/`undefined` for a row that
// does not exist. The REAL Cloudflare `SqliteStreamStore` instead reads those
// rows through `SqlStorageCursor.one()`, and Cloudflare's `.one()` THROWS
// ("Expected exactly one result from SQL query, but got no results") when the
// query returns zero rows. So the in-memory tests can never observe the prod
// failure: a missing-stream read 500'd in production while every unit test was
// green, and PUT-create itself 500'd (it calls `getMeta()` on the not-yet-
// existing stream first), so durable writes never persisted at all.
//
// This suite backs the REAL `SqliteStreamStore` with a real `bun:sqlite`
// database wrapped in a `SqlStorageLike` whose `.one()` reproduces Cloudflare's
// throw-on-zero/multiple-rows semantics EXACTLY. It then drives the REAL
// `handleDurableStreamFetch` (the DO `fetch` body) end-to-end, so the prod
// failure is reproducible here and the fix is proven against real SQLite — not
// a re-implementation.

import { Database } from 'bun:sqlite'
import { describe, expect, test } from 'bun:test'

import {
  handleDurableStreamFetch,
  SqliteStreamStore,
  type DurableObjectStateLike,
  type SqlStorageLike,
} from './durable-object.ts'

// A `SqlStorageLike` over `bun:sqlite` that faithfully reproduces Cloudflare's
// `SqlStorageCursor` semantics — most importantly `.one()` THROWS on zero rows
// (and on >1 row), exactly like `state.storage.sql` in a live Durable Object.
const FAITHFUL_ONE_ERROR =
  'Expected exactly one result from SQL query, but got no results.'

const cloudflareSql = (db: Database): SqlStorageLike => ({
  exec<T = Record<string, unknown>>(query: string, ...bindings: Array<unknown>) {
    const stmt = db.query(query)
    const rows = stmt.all(...(bindings as Array<never>)) as Array<T>
    return {
      toArray: () => rows,
      // Cloudflare's `.one()`: returns the single row, THROWS otherwise.
      one: (): T | undefined => {
        if (rows.length === 0) {
          throw new Error(FAITHFUL_ONE_ERROR)
        }
        if (rows.length > 1) {
          throw new Error(
            'Expected exactly one result from SQL query, but got multiple results.',
          )
        }
        return rows[0]
      },
    }
  },
})

// A minimal `DurableObjectStateLike` over the faithful SQL surface. Alarms are
// captured (not scheduled) so the TTL-refresh path is exercised without a real
// timer.
const fakeDoState = (db: Database): DurableObjectStateLike => {
  const sql = cloudflareSql(db)
  return {
    storage: {
      sql,
      setAlarm: () => {},
      deleteAlarm: () => {},
    },
    blockConcurrencyWhile: <T>(fn: () => Promise<T>): Promise<T> => fn(),
  }
}

const streamUrl = (id: string): string =>
  `https://do-internal/v1/stream/${encodeURIComponent(id)}`

describe('real SQLite Durable Object — missing-stream reads are graceful (not a thrown SQL error)', () => {
  test('GET a never-created stream returns 404, never throwing "Expected exactly one result"', async () => {
    const db = new Database(':memory:')
    const state = fakeDoState(db)

    // This is the EXACT prod reproduction: a durable read of a stream that was
    // never created. Pre-fix, `SqliteStreamStore.getMeta()` calls `.one()` on
    // zero `ds_meta` rows and THROWS, surfacing as the 500 the gateway mapped to
    // `internal_server_error`. Post-fix it must be a clean 404.
    const res = await handleDurableStreamFetch(
      state,
      new Request(`${streamUrl('never-created')}?offset=0`, { method: 'GET' }),
    )
    expect(res.status).toBe(404)
  })

  test('HEAD / POST / DELETE on a missing stream are all 404 (no thrown SQL error)', async () => {
    const db = new Database(':memory:')
    const state = fakeDoState(db)

    const head = await handleDurableStreamFetch(
      state,
      new Request(streamUrl('absent'), { method: 'HEAD' }),
    )
    expect(head.status).toBe(404)

    const post = await handleDurableStreamFetch(
      state,
      new Request(streamUrl('absent'), {
        method: 'POST',
        headers: {
          'content-type': 'text/event-stream',
          'producer-id': 'p',
          'producer-epoch': '0',
          'producer-seq': '0',
        },
        body: new TextEncoder().encode('data: x\n\n'),
      }),
    )
    expect(post.status).toBe(404)

    const del = await handleDurableStreamFetch(
      state,
      new Request(streamUrl('absent'), { method: 'DELETE' }),
    )
    expect(del.status).toBe(404)
  })
})

describe('real SQLite Durable Object — create + append + resume round-trip', () => {
  test('PUT-create succeeds against real SQLite (regression: create reads getMeta on the empty stream first)', async () => {
    const db = new Database(':memory:')
    const state = fakeDoState(db)

    // Pre-fix this 500'd: `handlePut` calls `store.getMeta()` BEFORE creating,
    // and on the not-yet-existing stream `.one()` threw — so `ensureStreamDO`
    // saw a non-2xx and degraded `durable=false`, and NO frames ever persisted.
    const put = await handleDurableStreamFetch(
      state,
      new Request(streamUrl('s1'), {
        method: 'PUT',
        headers: { 'content-type': 'text/event-stream', 'stream-ttl': '3600' },
      }),
    )
    expect(put.status).toBe(201)
  })

  test('write deltas + terminal frame, then a separate GET resumes the FULL log (streaming-equivalent) and a mid-offset resume returns only the suffix', async () => {
    const db = new Database(':memory:')
    const state = fakeDoState(db)
    const id = 'completion-1'

    // 1) PUT-create.
    expect(
      (
        await handleDurableStreamFetch(
          state,
          new Request(streamUrl(id), {
            method: 'PUT',
            headers: {
              'content-type': 'text/event-stream',
              'stream-ttl': '3600',
            },
          }),
        )
      ).status,
    ).toBe(201)

    // 2) Append two delta frames as the producer (seq 0, 1).
    const frames = ['data: {"delta":"Hel"}\n\n', 'data: {"delta":"lo"}\n\n']
    const offsets: Array<string> = []
    for (let seq = 0; seq < frames.length; seq++) {
      const res = await handleDurableStreamFetch(
        state,
        new Request(streamUrl(id), {
          method: 'POST',
          headers: {
            'content-type': 'text/event-stream',
            'producer-id': 'khala-gateway',
            'producer-epoch': '0',
            'producer-seq': String(seq),
          },
          body: new TextEncoder().encode(frames[seq]!),
        }),
      )
      expect(res.status).toBe(200)
      offsets.push(res.headers.get('stream-next-offset')!)
    }

    // 3) Append the terminal frame + close (seq 2, stream-closed).
    const terminal = 'data: {"done":true}\n\n'
    const closeRes = await handleDurableStreamFetch(
      state,
      new Request(streamUrl(id), {
        method: 'POST',
        headers: {
          'content-type': 'text/event-stream',
          'producer-id': 'khala-gateway',
          'producer-epoch': '0',
          'producer-seq': '2',
          'stream-closed': 'true',
        },
        body: new TextEncoder().encode(terminal),
      }),
    )
    expect(closeRes.status).toBe(200)
    expect(closeRes.headers.get('stream-closed')).toBe('true')

    // 4) FULL resume from offset 0 (a fresh Worker invocation / new DO read):
    //    reconstructs the entire completion.
    const full = await handleDurableStreamFetch(
      state,
      new Request(`${streamUrl(id)}?offset=0`, { method: 'GET' }),
    )
    expect(full.status).toBe(200)
    const fullBody = await full.text()
    expect(fullBody).toContain('"Hel"')
    expect(fullBody).toContain('"lo"')
    expect(fullBody).toContain('"done":true')
    expect(full.headers.get('stream-closed')).toBe('true')

    // 5) MID-OFFSET resume (client kept the first frame, dropped after): the
    //    suffix carries "lo" + terminal but NOT "Hel" — streaming-equivalence.
    const resume = await handleDurableStreamFetch(
      state,
      new Request(`${streamUrl(id)}?offset=${offsets[0]}`, { method: 'GET' }),
    )
    expect(resume.status).toBe(200)
    const resumeBody = await resume.text()
    expect(resumeBody).not.toContain('"Hel"')
    expect(resumeBody).toContain('"lo"')
    expect(resumeBody).toContain('"done":true')
    expect(resume.headers.get('stream-closed')).toBe('true')
  })
})

describe('real SQLite SqliteStreamStore — getMeta / getProducer tolerate zero rows', () => {
  test('getMeta() returns null (not a throw) on an empty store', () => {
    const db = new Database(':memory:')
    const store = new SqliteStreamStore(cloudflareSql(db))
    expect(store.getMeta()).toBeNull()
  })

  test('getProducer() returns null (not a throw) for an unknown producer', () => {
    const db = new Database(':memory:')
    const store = new SqliteStreamStore(cloudflareSql(db))
    expect(store.getProducer('unknown')).toBeNull()
  })

  test('byteLength() is 0 (not a throw) on an empty store', () => {
    const db = new Database(':memory:')
    const store = new SqliteStreamStore(cloudflareSql(db))
    expect(store.byteLength()).toBe(0)
  })
})
