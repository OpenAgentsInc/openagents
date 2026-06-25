import {
  CollectionName,
  CursorGap,
  EntityId,
  IsoTimestamp,
  SyncPatch,
  SyncScope,
  SyncSequence,
} from '@openagentsinc/sync-schema'
import { describe, expect, test } from 'vitest'

import {
  KHALA_TOKENS_SERVED_DELTAS_COLLECTION,
  KHALA_TOKENS_SERVED_SCOPE,
  applyKhalaTokensServedPatch,
  khalaTokensServedStreamAfterCursorGap,
  khalaTokensServedStreamFailed,
  khalaTokensServedStreamOpen,
} from './khala-tokens-served-feed'
import {
  type PublicKhalaTokensServedModel,
  IdlePublicKhalaTokensServed,
  LoadedPublicKhalaTokensServed,
  initKhalaTokensServedStreamModel,
} from './model'

const deltaPatch = (
  input: Readonly<{
    eventRef: string
    observedAt?: string
    seq: number
    tokensServedDelta: number
  }>,
): SyncPatch =>
  new SyncPatch({
    scope: SyncScope.make(KHALA_TOKENS_SERVED_SCOPE),
    seq: SyncSequence.make(input.seq),
    collection: CollectionName.make(KHALA_TOKENS_SERVED_DELTAS_COLLECTION),
    op: 'put',
    id: EntityId.make(input.eventRef),
    value: {
      eventRef: input.eventRef,
      observedAt: input.observedAt ?? '2026-06-24T00:00:01.000Z',
      tokensServedDelta: input.tokensServedDelta,
    },
    serverTime: IsoTimestamp.make('2026-06-24T00:00:01.000Z'),
  })

const seeded = (tokensServed: number): PublicKhalaTokensServedModel =>
  LoadedPublicKhalaTokensServed({
    served: { tokensServed, generatedAt: '2026-06-24T00:00:00.000Z' },
  })

describe('applyKhalaTokensServedPatch', () => {
  test('a pushed delta rolls the seeded total up and advances the cursor', () => {
    const result = applyKhalaTokensServedPatch({
      counter: seeded(1000),
      patch: deltaPatch({ eventRef: 'event.a', seq: 1, tokensServedDelta: 42 }),
      stream: initKhalaTokensServedStreamModel(),
    })

    expect(result.counter._tag).toBe('PublicKhalaTokensServedLoaded')
    expect(
      result.counter._tag === 'PublicKhalaTokensServedLoaded'
        ? result.counter.served.tokensServed
        : null,
    ).toBe(1042)
    expect(result.stream.cursor).toBe(1)
    expect(result.stream.appliedEventRefs).toEqual(['event.a'])
  })

  test('two distinct deltas accumulate', () => {
    const first = applyKhalaTokensServedPatch({
      counter: seeded(0),
      patch: deltaPatch({ eventRef: 'event.a', seq: 1, tokensServedDelta: 10 }),
      stream: initKhalaTokensServedStreamModel(),
    })
    const second = applyKhalaTokensServedPatch({
      counter: first.counter,
      patch: deltaPatch({ eventRef: 'event.b', seq: 2, tokensServedDelta: 5 }),
      stream: first.stream,
    })

    expect(
      second.counter._tag === 'PublicKhalaTokensServedLoaded'
        ? second.counter.served.tokensServed
        : null,
    ).toBe(15)
    expect(second.stream.cursor).toBe(2)
    expect(second.stream.appliedEventRefs).toEqual(['event.a', 'event.b'])
  })

  test('a replayed delta (same event ref) does not double-count', () => {
    const stream = initKhalaTokensServedStreamModel()
    const first = applyKhalaTokensServedPatch({
      counter: seeded(100),
      patch: deltaPatch({ eventRef: 'event.a', seq: 1, tokensServedDelta: 42 }),
      stream,
    })
    // A reconnect/cursor-replay re-delivers the SAME event ref (lower or equal
    // seq): the total must NOT move again.
    const replayed = applyKhalaTokensServedPatch({
      counter: first.counter,
      patch: deltaPatch({ eventRef: 'event.a', seq: 1, tokensServedDelta: 42 }),
      stream: first.stream,
    })

    expect(
      replayed.counter._tag === 'PublicKhalaTokensServedLoaded'
        ? replayed.counter.served.tokensServed
        : null,
    ).toBe(142)
    expect(replayed.stream.appliedEventRefs).toEqual(['event.a'])
  })

  test('a delta before the counter is seeded only advances the cursor (no loss)', () => {
    // Before the seed lands the counter is Idle; the delta must not invent a
    // total, but the cursor + applied ref still advance so a later replay of the
    // SAME event after seeding does not double-count.
    const beforeSeed = applyKhalaTokensServedPatch({
      counter: IdlePublicKhalaTokensServed(),
      patch: deltaPatch({ eventRef: 'event.a', seq: 1, tokensServedDelta: 42 }),
      stream: initKhalaTokensServedStreamModel(),
    })

    expect(beforeSeed.counter._tag).toBe('PublicKhalaTokensServedIdle')
    expect(beforeSeed.stream.cursor).toBe(1)
    expect(beforeSeed.stream.appliedEventRefs).toEqual(['event.a'])

    // The seed reads the authoritative SUM (which already includes event.a).
    // A replay of event.a after seeding must be a no-op.
    const afterSeedReplay = applyKhalaTokensServedPatch({
      counter: seeded(1000),
      patch: deltaPatch({ eventRef: 'event.a', seq: 1, tokensServedDelta: 42 }),
      stream: beforeSeed.stream,
    })

    expect(
      afterSeedReplay.counter._tag === 'PublicKhalaTokensServedLoaded'
        ? afterSeedReplay.counter.served.tokensServed
        : null,
    ).toBe(1000)
  })

  test('an unknown collection / non-put op only advances the cursor', () => {
    const result = applyKhalaTokensServedPatch({
      counter: seeded(500),
      patch: new SyncPatch({
        scope: SyncScope.make(KHALA_TOKENS_SERVED_SCOPE),
        seq: SyncSequence.make(7),
        collection: CollectionName.make('something_else'),
        op: 'put',
        id: EntityId.make('x'),
        value: { tokensServedDelta: 99 },
        serverTime: IsoTimestamp.make('2026-06-24T00:00:01.000Z'),
      }),
      stream: initKhalaTokensServedStreamModel(),
    })

    expect(
      result.counter._tag === 'PublicKhalaTokensServedLoaded'
        ? result.counter.served.tokensServed
        : null,
    ).toBe(500)
    expect(result.stream.cursor).toBe(7)
    expect(result.stream.appliedEventRefs).toEqual([])
  })
})

describe('khalaTokensServedStreamAfterCursorGap', () => {
  test('advances the cursor to the received seq', () => {
    const gapped = khalaTokensServedStreamAfterCursorGap(
      initKhalaTokensServedStreamModel(),
      new CursorGap({
        scope: SyncScope.make(KHALA_TOKENS_SERVED_SCOPE),
        expectedSeq: SyncSequence.make(0),
        receivedSeq: SyncSequence.make(12),
      }),
    )

    expect(gapped.cursor).toBe(12)
  })
})

describe('khala tokens served stream connection state', () => {
  test('open / failed update the connection tag', () => {
    const open = khalaTokensServedStreamOpen(initKhalaTokensServedStreamModel())
    expect(open.connection).toBe('open')
    expect(khalaTokensServedStreamFailed(open).connection).toBe('failed')
  })
})
