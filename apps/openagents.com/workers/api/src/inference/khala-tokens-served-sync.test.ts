import { describe, expect, test } from 'vitest'

import {
  KHALA_TOKENS_SERVED_SUMMARY_COLLECTION,
  KHALA_TOKENS_SERVED_SUMMARY_ENTITY_ID,
  KHALA_TOKENS_SERVED_SYNC_COLLECTION,
  buildKhalaTokensServedDelta,
  publishKhalaTokensServedDelta,
} from './khala-tokens-served-sync'

// Minimal in-memory D1 + sync room (mirrors tassadar-settled-feed-sync.test.ts)
// that records the rows appended to the outbox and the scopes poked, so the
// publisher's public-safe-and-fail-soft behavior is exercised against the REAL
// outbox append path.

type StoredChange = Readonly<{
  collection: string
  entity_id: string
  op: string
  scope: string
  seq: number
  value_json: string | null
}>

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

type MemoryD1 = D1Database & Readonly<{ changes: Array<StoredChange> }>

const makeStatement = (
  state: Pick<MemoryD1, 'changes'> & { lastSeq: number; tokensServed: number },
  query: string,
): D1PreparedStatement => {
  let values: ReadonlyArray<unknown> = []
  let statement: D1PreparedStatement

  statement = {
    all: async <T = Record<string, unknown>>() => makeResult<T>(),
    bind: (...nextValues: ReadonlyArray<unknown>) => {
      values = nextValues

      return statement
    },
    first: async <T = Record<string, unknown>>() => {
      if (query.includes('INSERT INTO sync_scopes')) {
        state.lastSeq = state.lastSeq + 1

        return JSON.parse(JSON.stringify({ last_seq: state.lastSeq })) as T
      }

      // The authoritative ledger SUM read used to fill `tokensServedTotal`.
      if (query.includes('AS tokens_served')) {
        return JSON.parse(
          JSON.stringify({ tokens_served: state.tokensServed }),
        ) as T
      }

      return null
    },
    raw: async () => [] as never,
    run: async <T = Record<string, unknown>>() => {
      if (query.includes('INSERT INTO sync_changes')) {
        const [scope, seq, collection, op, entityId, valueJson] = values

        state.changes.push({
          collection: String(collection),
          entity_id: String(entityId),
          op: String(op),
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

const makeMemoryD1 = (tokensServed = 0): MemoryD1 => {
  const state = {
    changes: [] as Array<StoredChange>,
    lastSeq: 0,
    tokensServed,
  }

  return {
    changes: state.changes,
    batch: async <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: async () => new ArrayBuffer(0),
    exec: async () => ({ count: 0, duration: 0 }),
    prepare: (query: string) => makeStatement(state, query),
    withSession: () =>
      ({
        batch: async <T = unknown>(statements: Array<D1PreparedStatement>) =>
          Promise.all(statements.map(statement => statement.run<T>())),
        getBookmark: () => null,
        prepare: (query: string) => makeStatement(state, query),
      }) satisfies D1DatabaseSession,
  } as unknown as MemoryD1
}

const makeSyncRoom = (
  notifiedScopes: Array<string>,
): DurableObjectNamespace =>
  ({
    getByName: (scope: string) => ({
      fetch: async (request: Request) => {
        notifiedScopes.push(
          request.headers.get('x-openagents-sync-scope') ?? scope,
        )

        return new Response(null, { status: 204 })
      },
    }),
    idFromName: (scope: string) => scope,
    get: (scope: string) => ({
      fetch: async (request: Request) => {
        notifiedScopes.push(
          request.headers.get('x-openagents-sync-scope') ?? scope,
        )

        return new Response(null, { status: 204 })
      },
    }),
  }) as never

describe('buildKhalaTokensServedDelta', () => {
  test('carries only a clamped integer delta, event ref, and timestamp', () => {
    const delta = buildKhalaTokensServedDelta({
      eventRef: 'event.inference.served-tokens.chatcmpl-1',
      observedAt: '2026-06-24T00:00:00.000Z',
      tokensServedDelta: 42.9,
    })

    expect(delta).toStrictEqual({
      eventRef: 'event.inference.served-tokens.chatcmpl-1',
      observedAt: '2026-06-24T00:00:00.000Z',
      tokensServedDelta: 42,
    })
    // Public-safe: a bare integer + refs, nothing user/team/provider/secret.
    expect(Object.keys(delta).sort()).toEqual([
      'eventRef',
      'observedAt',
      'tokensServedDelta',
    ])
  })

  test('clamps a negative delta to zero', () => {
    expect(
      buildKhalaTokensServedDelta({
        eventRef: 'event.x',
        observedAt: '2026-06-24T00:00:00.000Z',
        tokensServedDelta: -10,
      }).tokensServedDelta,
    ).toBe(0)
  })
})

describe('publishKhalaTokensServedDelta', () => {
  test('writes the event + an authoritative running-total summary and pokes the room', async () => {
    // The authoritative ledger SUM (1_000_042) is the running total AFTER this
    // 42-token row; both the event and the summary must carry it.
    const db = makeMemoryD1(1_000_042)
    const notifiedScopes: Array<string> = []

    await publishKhalaTokensServedDelta(
      { OPENAGENTS_DB: db, SYNC_ROOM: makeSyncRoom(notifiedScopes) },
      buildKhalaTokensServedDelta({
        eventRef: 'event.inference.served-tokens.chatcmpl-1',
        observedAt: '2026-06-24T00:00:00.000Z',
        tokensServedDelta: 42,
      }),
    )

    expect(db.changes).toHaveLength(2)

    const event = db.changes.find(
      change => change.collection === KHALA_TOKENS_SERVED_SYNC_COLLECTION,
    )
    const summary = db.changes.find(
      change => change.collection === KHALA_TOKENS_SERVED_SUMMARY_COLLECTION,
    )

    expect(event?.scope).toBe('public-khala-tokens-served:network')
    expect(event?.op).toBe('put')
    expect(event?.entity_id).toBe('event.inference.served-tokens.chatcmpl-1')
    expect(JSON.parse(event?.value_json ?? '{}')).toMatchObject({
      eventRef: 'event.inference.served-tokens.chatcmpl-1',
      tokensServedDelta: 42,
      tokensServedTotal: 1_000_042,
    })

    expect(summary?.op).toBe('put')
    expect(summary?.entity_id).toBe(KHALA_TOKENS_SERVED_SUMMARY_ENTITY_ID)
    expect(JSON.parse(summary?.value_json ?? '{}')).toMatchObject({
      tokensServedTotal: 1_000_042,
    })

    expect(notifiedScopes).toEqual(['public-khala-tokens-served:network'])
  })

  test('a zero/negative delta is a no-op (nothing written, room not poked)', async () => {
    const db = makeMemoryD1()
    const notifiedScopes: Array<string> = []

    await publishKhalaTokensServedDelta(
      { OPENAGENTS_DB: db, SYNC_ROOM: makeSyncRoom(notifiedScopes) },
      buildKhalaTokensServedDelta({
        eventRef: 'event.zero',
        observedAt: '2026-06-24T00:00:00.000Z',
        tokensServedDelta: 0,
      }),
    )

    expect(db.changes).toHaveLength(0)
    expect(notifiedScopes).toHaveLength(0)
  })

  test('a payload that smuggles secret-shaped material is rejected before write', async () => {
    const db = makeMemoryD1()
    const notifiedScopes: Array<string> = []

    // An (impossible by construction) event ref carrying a 64-hex secret-shaped
    // value must be filtered by the public projection guard before any write.
    await publishKhalaTokensServedDelta(
      { OPENAGENTS_DB: db, SYNC_ROOM: makeSyncRoom(notifiedScopes) },
      {
        eventRef:
          'sk-0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
        observedAt: '2026-06-24T00:00:00.000Z',
        tokensServedDelta: 42,
      },
    )

    expect(db.changes).toHaveLength(0)
    expect(notifiedScopes).toHaveLength(0)
  })
})
