import { describe, expect, test } from 'vitest'

import {
  type PublicSettledFeedEvent,
  SETTLED_FEED_SUMMARY_COLLECTION,
  SETTLED_FEED_SUMMARY_ENTITY_ID,
  SETTLED_FEED_SYNC_COLLECTION,
  assertSettledFeedPayloadPublicSafe,
  buildSettledFeedEvents,
  publishSettledFeedEvents,
  settledFeedSummaryFromEvents,
} from './tassadar-settled-feed-sync'

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

type StoredScope = Readonly<{
  created_at: string
  last_seq: number
  scope: string
  updated_at: string
}>

type MemoryD1 = D1Database &
  Readonly<{
    changes: Array<StoredChange>
    scopes: Map<string, StoredScope>
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

const storedChangeOp = (value: unknown): StoredChange['op'] => {
  if (
    value === 'put' ||
    value === 'patch' ||
    value === 'delete' ||
    value === 'invalidate'
  ) {
    return value
  }

  throw new Error(`Invalid stored change operation: ${String(value)}`)
}

function rawD1Rows<T = unknown[]>(options: {
  columnNames: true
}): Promise<[Array<string>, ...Array<T>]>
function rawD1Rows<T = unknown[]>(options?: {
  columnNames?: false
}): Promise<Array<T>>
function rawD1Rows<T = unknown[]>(options?: {
  columnNames?: boolean
}): Promise<Array<T> | [Array<string>, ...Array<T>]> {
  if (options?.columnNames === true) {
    return Promise.resolve([[]])
  }

  return Promise.resolve([])
}

const makeStatement = (
  state: Pick<MemoryD1, 'changes' | 'scopes'>,
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
        const [scope, createdAt, updatedAt] = values
        const key = String(scope)
        const previous = state.scopes.get(key)
        const next = {
          created_at: previous?.created_at ?? String(createdAt),
          last_seq: (previous?.last_seq ?? 0) + 1,
          scope: key,
          updated_at: String(updatedAt),
        }

        state.scopes.set(key, next)

        const decoded: T = JSON.parse(
          JSON.stringify({ last_seq: next.last_seq }),
        )

        return decoded
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

        state.changes.push({
          actor_id: actorId === null ? null : String(actorId),
          collection: String(collection),
          created_at: String(createdAt),
          entity_id: String(entityId),
          mutation_id: mutationId === null ? null : String(mutationId),
          op: storedChangeOp(op),
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

const makeMemoryD1 = (): MemoryD1 => {
  const state: Pick<MemoryD1, 'changes' | 'scopes'> = {
    changes: [],
    scopes: new Map<string, StoredScope>(),
  }

  return {
    ...state,
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
  } satisfies MemoryD1
}

const makeSyncRoom = (
  notifiedScopes: Array<string>,
): DurableObjectNamespace =>
  // eslint-disable-next-line @typescript-eslint/consistent-type-assertions
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

const settledLeg = (
  overrides: Partial<{
    amountSats: number
    challengeRef: string
    contributorRef: string
    party: 'validator' | 'worker'
    runRef: string
    windowRef: string | null
  }> = {},
) => ({
  amountSats: overrides.amountSats ?? 5,
  challengeRef: overrides.challengeRef ?? 'challenge.tassadar.window.0001',
  contributorRef: overrides.contributorRef ?? 'pylon.worker.orrery',
  party: overrides.party ?? ('worker' as const),
  runRef: overrides.runRef ?? 'run.tassadar.poc',
  windowRef: overrides.windowRef ?? 'window.tassadar.0001',
})

describe('buildSettledFeedEvents', () => {
  test('threads running settled total/count across legs', () => {
    const events = buildSettledFeedEvents({
      legs: [
        settledLeg({ amountSats: 5, party: 'worker' }),
        settledLeg({
          amountSats: 5,
          contributorRef: 'pylon.validator.whitefang',
          party: 'validator',
        }),
      ],
      priorCount: 3,
      priorSettledSats: 15,
      settledAt: '2026-06-17T00:00:00.000Z',
    })

    expect(events).toHaveLength(2)
    expect(events[0]).toMatchObject({
      amountSats: 5,
      party: 'worker',
      totalSettledCount: 4,
      totalSettledSats: 20,
    })
    expect(events[1]).toMatchObject({
      amountSats: 5,
      party: 'validator',
      totalSettledCount: 5,
      totalSettledSats: 25,
    })
    expect(events[0]?.eventRef).not.toEqual(events[1]?.eventRef)
  })
})

describe('settledFeedSummaryFromEvents', () => {
  test('summarizes the latest cumulative totals', () => {
    const events = buildSettledFeedEvents({
      legs: [settledLeg({ amountSats: 5 })],
      priorCount: 0,
      priorSettledSats: 0,
      settledAt: '2026-06-17T00:00:00.000Z',
    })
    const summary = settledFeedSummaryFromEvents(events)

    expect(summary).toMatchObject({
      latestSettledAt: '2026-06-17T00:00:00.000Z',
      totalSettledCount: 1,
      totalSettledSats: 5,
    })
    expect(summary.latestEventRef).toEqual(events[0]?.eventRef)
  })
})

describe('assertSettledFeedPayloadPublicSafe (redaction)', () => {
  const safe: PublicSettledFeedEvent = {
    amountSats: 5,
    challengeRef: 'challenge.tassadar.window.0001',
    contributorRef: 'pylon.worker.orrery',
    eventRef: 'settled.challenge_tassadar_window_0001.worker.0',
    party: 'worker',
    runRef: 'run.tassadar.poc',
    settledAt: '2026-06-17T00:00:00.000Z',
    totalSettledCount: 1,
    totalSettledSats: 5,
    windowRef: 'window.tassadar.0001',
  }

  test('accepts a public-safe event', () => {
    expect(() =>
      assertSettledFeedPayloadPublicSafe('event', safe),
    ).not.toThrow()
  })

  test('rejects a raw spark destination', () => {
    expect(() =>
      assertSettledFeedPayloadPublicSafe('event', {
        ...safe,
        contributorRef: 'spark1qexamplerawdestinationxyz0123456789',
      }),
    ).toThrow()
  })

  test('rejects a raw lightning invoice', () => {
    expect(() =>
      assertSettledFeedPayloadPublicSafe('event', {
        ...safe,
        runRef: 'lnbc2500u1pvjluezpp5example',
      }),
    ).toThrow()
  })

  test('rejects a raw preimage', () => {
    expect(() =>
      assertSettledFeedPayloadPublicSafe('event', {
        ...safe,
        contributorRef: 'preimage:abc123',
      }),
    ).toThrow()
  })

  test('rejects a 64-hex secret (preimage/hash shape)', () => {
    expect(() =>
      assertSettledFeedPayloadPublicSafe('event', {
        ...safe,
        challengeRef:
          '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
      }),
    ).toThrow()
  })

  test('rejects a raw on-chain bech32 address', () => {
    expect(() =>
      assertSettledFeedPayloadPublicSafe('event', {
        ...safe,
        contributorRef: 'bc1qexampleonchainaddressxyz0123456789',
      }),
    ).toThrow()
  })
})

describe('publishSettledFeedEvents', () => {
  test('writes public-safe events + summary to the public scope and pokes the room', async () => {
    const db = makeMemoryD1()
    const notifiedScopes: Array<string> = []
    const events = buildSettledFeedEvents({
      legs: [
        settledLeg({ amountSats: 5, party: 'worker' }),
        settledLeg({
          amountSats: 5,
          contributorRef: 'pylon.validator.whitefang',
          party: 'validator',
        }),
      ],
      priorCount: 0,
      priorSettledSats: 0,
      settledAt: '2026-06-17T00:00:00.000Z',
    })

    await publishSettledFeedEvents(
      {
        OPENAGENTS_DB: db,
        SYNC_ROOM: makeSyncRoom(notifiedScopes),
      },
      events,
    )

    const eventChanges = db.changes.filter(
      change => change.collection === SETTLED_FEED_SYNC_COLLECTION,
    )
    const summaryChanges = db.changes.filter(
      change => change.collection === SETTLED_FEED_SUMMARY_COLLECTION,
    )

    expect(eventChanges).toHaveLength(2)
    expect(eventChanges.every(c => c.scope === 'public-settled-feed:tassadar'))
      .toBe(true)
    expect(summaryChanges).toHaveLength(1)
    expect(summaryChanges[0]?.entity_id).toEqual(
      SETTLED_FEED_SUMMARY_ENTITY_ID,
    )
    expect(JSON.parse(summaryChanges[0]?.value_json ?? '{}')).toMatchObject({
      totalSettledCount: 2,
      totalSettledSats: 10,
    })
    expect(notifiedScopes).toEqual(['public-settled-feed:tassadar'])
  })

  test('no raw spark/invoice/preimage material ever lands in the outbox', async () => {
    const db = makeMemoryD1()
    const notifiedScopes: Array<string> = []
    // An (impossible by construction, but defensively guarded) event that smuggles
    // a raw spark destination must be filtered out before it can be written.
    const unsafeEvent: PublicSettledFeedEvent = {
      amountSats: 5,
      challengeRef: 'challenge.tassadar.window.0001',
      contributorRef: 'spark1qrawdestination0123456789abcdef',
      eventRef: 'settled.unsafe.0',
      party: 'worker',
      runRef: 'run.tassadar.poc',
      settledAt: '2026-06-17T00:00:00.000Z',
      totalSettledCount: 1,
      totalSettledSats: 5,
      windowRef: 'window.tassadar.0001',
    }

    await publishSettledFeedEvents(
      {
        OPENAGENTS_DB: db,
        SYNC_ROOM: makeSyncRoom(notifiedScopes),
      },
      [unsafeEvent],
    )

    expect(db.changes).toHaveLength(0)
    const serialized = JSON.stringify(db.changes)
    expect(serialized).not.toContain('spark1')
    expect(serialized).not.toMatch(/lnbc|preimage/i)
  })

  test('no-ops with no events', async () => {
    const db = makeMemoryD1()
    const notifiedScopes: Array<string> = []

    await publishSettledFeedEvents(
      {
        OPENAGENTS_DB: db,
        SYNC_ROOM: makeSyncRoom(notifiedScopes),
      },
      [],
    )

    expect(db.changes).toHaveLength(0)
    expect(notifiedScopes).toHaveLength(0)
  })
})
