// REAL Durable Object end-to-end test for the durable inference stream
// (#6154 tier 4 follow-up; reproduces + fixes the prod 500).
//
// THE GAP THIS CLOSES: `durable-inference-do-transport.test.ts` drives the
// production transport (`teeUpstreamToDurableDO` / `replayFromOffsetDO`) against
// a `MemoryStreamStore`-backed registry. `MemoryStreamStore.getMeta()` returns
// `null` for a missing stream, so the in-memory path can never hit Cloudflare's
// real `SqlStorageCursor.one()` THROW on zero rows — the exact crash that
// 500'd in prod ("Expected exactly one result from SQL query, but got no
// results") on a missing-stream read AND silently broke writes (PUT-create reads
// `getMeta()` on the not-yet-created stream first).
//
// This test drives the SAME production transport through a `DurableStreamNamespace`
// whose `getByName(name).fetch()` delegates to the REAL package DO handler
// (`handleDurableStreamFetch`) over a REAL `node:sqlite` database whose cursor
// reproduces Cloudflare's `.one()` throw-on-zero/multiple-rows semantics EXACTLY,
// keyed by name (so write and read hit the SAME DO instance — the prod key path).
// Pre-fix this suite fails with the prod error; post-fix it is green.

import {
  handleDurableStreamFetch,
  type DurableObjectStateLike,
  type SqlStorageLike,
} from '@openagentsinc/durable-stream'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import {
  type DurableStreamNamespace,
  replayFromOffsetDO,
  teeUpstreamToDurableDO,
} from './durable-inference-do-transport'
import {
  type InferenceStreamEvent,
  type InferenceStreamSource,
  type InferenceUsage,
} from './provider-adapter'

// A `SqlStorageLike` over `node:sqlite` that reproduces Cloudflare's
// `SqlStorageCursor` semantics — crucially `.one()` THROWS on zero rows, exactly
// like a live Durable Object's `state.storage.sql`.
const cloudflareSql = (db: DatabaseSync): SqlStorageLike => ({
  exec<T = Record<string, unknown>>(query: string, ...bindings: Array<unknown>) {
    const stmt = db.prepare(query)
    const rows = stmt.all(...(bindings as Array<never>)) as Array<T>
    // node:sqlite returns BLOB columns as Uint8Array; the package's `readFrom`
    // already tolerates both Uint8Array and ArrayBuffer, so no extra coercion.
    return {
      toArray: () => rows,
      one: (): T | undefined => {
        if (rows.length === 0) {
          throw new Error(
            'Expected exactly one result from SQL query, but got no results.',
          )
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

const fakeDoState = (db: DatabaseSync): DurableObjectStateLike => ({
  storage: {
    sql: cloudflareSql(db),
    setAlarm: () => {},
    deleteAlarm: () => {},
  },
  blockConcurrencyWhile: <T>(fn: () => Promise<T>): Promise<T> => fn(),
})

// A `DurableStreamNamespace` backed by real per-name SQLite DOs. One
// `DatabaseSync` per DO name == one DO instance, so a write and a later read of
// the SAME name share storage — the production `getByName(requestId)` key path.
const realSqliteNamespace = (): DurableStreamNamespace => {
  const dbs = new Map<string, DatabaseSync>()
  const stateFor = (name: string): DurableObjectStateLike => {
    let db = dbs.get(name)
    if (db === undefined) {
      db = new DatabaseSync(':memory:')
      dbs.set(name, db)
    }
    return fakeDoState(db)
  }
  return {
    getByName: (name: string) => ({
      fetch: (request: Request) =>
        handleDurableStreamFetch(stateFor(name), request),
    }),
  }
}

const usage: InferenceUsage = {
  completionTokens: 5,
  promptTokens: 9,
  totalTokens: 14,
}

const source = (
  script: ReadonlyArray<InferenceStreamEvent>,
  options: { readonly fault?: boolean } = {},
): InferenceStreamSource => ({
  frames: (async function* () {
    for (const event of script) {
      yield event
    }
    if (options.fault === true) {
      throw new Error('upstream faulted')
    }
  })(),
  terminal: () => ({
    finishReason: 'stop' as string | undefined,
    servedModel: 'served/m' as string | undefined,
    usage: usage as InferenceUsage | undefined,
  }),
})

const drive = async (input: {
  namespace: DurableStreamNamespace
  requestId: string
  src: InferenceStreamSource
}) => {
  const emitted: Array<string> = []
  let meterCount = 0
  const outcome = await teeUpstreamToDurableDO({
    emit: frame => emitted.push(frame),
    frameForDelta: delta => `data: {"delta":${JSON.stringify(delta)}}\n\n`,
    namespace: input.namespace,
    onEof: async (terminal, content) => {
      if (terminal.usage !== undefined) {
        meterCount += 1
      }
      return `data: {"done":true,"content":${JSON.stringify(content)}}\n\n`
    },
    requestId: input.requestId,
    source: input.src,
  })
  return { emitted, meterCount, outcome }
}

describe('REAL Durable Object (node:sqlite) — durable inference write + resume', () => {
  test('a completed turn PERSISTS to the real SQLite DO and a separate read resumes the FULL completion (the prod write-persistence regression)', async () => {
    const namespace = realSqliteNamespace()
    const requestId = 'onboarding:sess-abc:0'

    const { meterCount, outcome } = await drive({
      namespace,
      requestId,
      src: source([{ contentDelta: 'Hel' }, { contentDelta: 'lo' }]),
    })

    // The producer drain completed cleanly and metered exactly once.
    expect(outcome.faulted).toBe(false)
    expect(outcome.content).toBe('Hello')
    expect(meterCount).toBe(1)

    // A SEPARATE read (a later Worker invocation) by the SAME key reconstructs
    // the FULL log — proving the write actually persisted to the real DO and is
    // resumable by the key the reader uses. Pre-fix this 500'd / returned 404
    // because PUT-create threw on `.one()` and no frame ever persisted.
    const full = await replayFromOffsetDO({ namespace, offset: '0', requestId })
    expect(full).toBeDefined()
    expect(full!.body).toContain('"Hel"')
    expect(full!.body).toContain('"lo"')
    expect(full!.body).toContain('"done":true')
    expect(full!.streamClosed).toBe(true)
  })

  test('mid-offset resume returns only the suffix (streaming-equivalence) against the real DO', async () => {
    const namespace = realSqliteNamespace()
    const requestId = 'req-resume-real'

    const { emitted } = await drive({
      namespace,
      requestId,
      src: source([
        { contentDelta: 'AAA' },
        { contentDelta: 'BBB' },
        { contentDelta: 'CCC' },
      ]),
    })

    const firstReadBytes = new TextEncoder().encode(emitted[0]!).length
    const resume = await replayFromOffsetDO({
      namespace,
      offset: String(firstReadBytes),
      requestId,
    })
    expect(resume).toBeDefined()
    expect(resume!.body).not.toContain('"AAA"')
    expect(resume!.body).toContain('"BBB"')
    expect(resume!.body).toContain('"CCC"')
    expect(resume!.body).toContain('"done":true')
    expect(resume!.streamClosed).toBe(true)
  })

  test('metering NEVER fires on a resume read against the real DO', async () => {
    const namespace = realSqliteNamespace()
    const requestId = 'req-no-rebill-real'
    const first = await drive({
      namespace,
      requestId,
      src: source([{ contentDelta: 'x' }, { contentDelta: 'y' }]),
    })
    expect(first.meterCount).toBe(1)

    for (let i = 0; i < 4; i++) {
      const replay = await replayFromOffsetDO({ namespace, offset: '0', requestId })
      expect(replay).toBeDefined()
    }
    // Still exactly one settlement — the resume path has no metering hook.
    expect(first.meterCount).toBe(1)
  })
})

describe('REAL Durable Object (node:sqlite) — missing stream is a graceful 404 (the literal prod crash)', () => {
  test('replayFromOffsetDO on a NEVER-created stream returns undefined (→ 404), not a thrown "Expected exactly one result" 500', async () => {
    const namespace = realSqliteNamespace()
    // EXACT prod reproduction #1: GET .../durable/<random>?offset=0 on a stream
    // that was never created. Pre-fix the real DO threw the "no results" error
    // (→ 500); post-fix the read is a clean not-found (undefined → 404).
    const replay = await replayFromOffsetDO({
      namespace,
      offset: '0',
      requestId: 'never-created-real',
    })
    expect(replay).toBeUndefined()
  })
})
