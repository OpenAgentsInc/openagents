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
  KHALA_TOKENS_SERVED_SUMMARY_COLLECTION,
  applyKhalaTokensServedPatch,
  khalaTokensServedAfterScalarSeed,
  khalaTokensServedAfterSnapshot,
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

// Each streamed event carries the AUTHORITATIVE running total AFTER that event
// (`tokensServedTotal` = the live ledger SUM), so the client advances the counter
// monotonically (`max`) and converges exactly to the ledger with no double-count.
const deltaPatch = (
  input: Readonly<{
    eventRef: string
    observedAt?: string
    seq: number
    tokensServedDelta: number
    tokensServedTotal: number
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
      tokensServedTotal: input.tokensServedTotal,
    },
    serverTime: IsoTimestamp.make('2026-06-24T00:00:01.000Z'),
  })

const summaryPatch = (
  input: Readonly<{ seq: number; tokensServedTotal: number }>,
): SyncPatch =>
  new SyncPatch({
    scope: SyncScope.make(KHALA_TOKENS_SERVED_SCOPE),
    seq: SyncSequence.make(input.seq),
    collection: CollectionName.make(KHALA_TOKENS_SERVED_SUMMARY_COLLECTION),
    op: 'put',
    id: EntityId.make('summary'),
    value: {
      observedAt: '2026-06-24T00:00:01.000Z',
      tokensServedTotal: input.tokensServedTotal,
    },
    serverTime: IsoTimestamp.make('2026-06-24T00:00:01.000Z'),
  })

const seeded = (tokensServed: number): PublicKhalaTokensServedModel =>
  LoadedPublicKhalaTokensServed({
    served: { tokensServed, generatedAt: '2026-06-24T00:00:00.000Z' },
  })

const tokensServed = (counter: PublicKhalaTokensServedModel): number | null =>
  counter._tag === 'PublicKhalaTokensServedLoaded'
    ? counter.served.tokensServed
    : null

describe('khalaTokensServedAfterSnapshot', () => {
  test('seeds the running total + cursor from the summary record', () => {
    const result = khalaTokensServedAfterSnapshot({
      counter: IdlePublicKhalaTokensServed(),
      cursor: 5,
      stream: initKhalaTokensServedStreamModel(),
      summary: { observedAt: '2026-06-24T00:00:00.000Z', tokensServedTotal: 1590000 },
    })

    expect(tokensServed(result.counter)).toBe(1590000)
    expect(result.stream.cursor).toBe(5)
  })

  test('a missing summary only advances the cursor (scalar fallback seeds the total)', () => {
    const result = khalaTokensServedAfterSnapshot({
      counter: IdlePublicKhalaTokensServed(),
      cursor: 3,
      stream: initKhalaTokensServedStreamModel(),
      summary: null,
    })

    expect(result.counter._tag).toBe('PublicKhalaTokensServedIdle')
    expect(result.stream.cursor).toBe(3)
  })

  test('never lowers an already-higher displayed total', () => {
    const result = khalaTokensServedAfterSnapshot({
      counter: seeded(2000000),
      cursor: 9,
      stream: initKhalaTokensServedStreamModel(),
      summary: { observedAt: '2026-06-24T00:00:00.000Z', tokensServedTotal: 1590000 },
    })

    expect(tokensServed(result.counter)).toBe(2000000)
    expect(result.stream.cursor).toBe(9)
  })
})

describe('applyKhalaTokensServedPatch', () => {
  test('a pushed event raises the total to its authoritative running total', () => {
    const result = applyKhalaTokensServedPatch({
      counter: seeded(1000),
      patch: deltaPatch({
        eventRef: 'event.a',
        seq: 1,
        tokensServedDelta: 42,
        tokensServedTotal: 1042,
      }),
      stream: initKhalaTokensServedStreamModel(),
    })

    expect(tokensServed(result.counter)).toBe(1042)
    expect(result.stream.cursor).toBe(1)
    expect(result.stream.appliedEventRefs).toEqual(['event.a'])
  })

  test('successive events advance monotonically to each running total', () => {
    const first = applyKhalaTokensServedPatch({
      counter: seeded(0),
      patch: deltaPatch({
        eventRef: 'event.a',
        seq: 1,
        tokensServedDelta: 10,
        tokensServedTotal: 10,
      }),
      stream: initKhalaTokensServedStreamModel(),
    })
    const second = applyKhalaTokensServedPatch({
      counter: first.counter,
      patch: deltaPatch({
        eventRef: 'event.b',
        seq: 2,
        tokensServedDelta: 5,
        tokensServedTotal: 15,
      }),
      stream: first.stream,
    })

    expect(tokensServed(second.counter)).toBe(15)
    expect(second.stream.cursor).toBe(2)
    expect(second.stream.appliedEventRefs).toEqual(['event.a', 'event.b'])
  })

  test('a replayed event (same running total) never double-counts', () => {
    const first = applyKhalaTokensServedPatch({
      counter: seeded(100),
      patch: deltaPatch({
        eventRef: 'event.a',
        seq: 1,
        tokensServedDelta: 42,
        tokensServedTotal: 142,
      }),
      stream: initKhalaTokensServedStreamModel(),
    })
    // A reconnect/cursor-replay re-delivers the SAME event: the authoritative
    // total is already applied, so the counter does NOT move (max is idempotent),
    // and even if it were an additive delta the de-dupe ledger guards it.
    const replayed = applyKhalaTokensServedPatch({
      counter: first.counter,
      patch: deltaPatch({
        eventRef: 'event.a',
        seq: 1,
        tokensServedDelta: 42,
        tokensServedTotal: 142,
      }),
      stream: first.stream,
    })

    expect(tokensServed(replayed.counter)).toBe(142)
    expect(replayed.stream.appliedEventRefs).toEqual(['event.a'])
  })

  test('an event whose total is below the displayed value never moves it backward', () => {
    // An out-of-order / stale event carrying a lower running total must NOT pull
    // the counter down — monotonicity holds regardless of delivery order.
    const result = applyKhalaTokensServedPatch({
      counter: seeded(2000000),
      patch: deltaPatch({
        eventRef: 'event.stale',
        seq: 1,
        tokensServedDelta: 5,
        tokensServedTotal: 1590000,
      }),
      stream: initKhalaTokensServedStreamModel(),
    })

    expect(tokensServed(result.counter)).toBe(2000000)
    expect(result.stream.cursor).toBe(1)
  })

  test('a summary put raises the total monotonically', () => {
    const result = applyKhalaTokensServedPatch({
      counter: seeded(1000),
      patch: summaryPatch({ seq: 4, tokensServedTotal: 1234 }),
      stream: initKhalaTokensServedStreamModel(),
    })

    expect(tokensServed(result.counter)).toBe(1234)
    expect(result.stream.cursor).toBe(4)
  })

  test('an event before the counter is seeded only advances the cursor + ref', () => {
    const beforeSeed = applyKhalaTokensServedPatch({
      counter: IdlePublicKhalaTokensServed(),
      patch: deltaPatch({
        eventRef: 'event.a',
        seq: 1,
        tokensServedDelta: 42,
        tokensServedTotal: 1042,
      }),
      stream: initKhalaTokensServedStreamModel(),
    })

    expect(beforeSeed.counter._tag).toBe('PublicKhalaTokensServedIdle')
    expect(beforeSeed.stream.cursor).toBe(1)
    expect(beforeSeed.stream.appliedEventRefs).toEqual(['event.a'])
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
        value: { tokensServedTotal: 99 },
        serverTime: IsoTimestamp.make('2026-06-24T00:00:01.000Z'),
      }),
      stream: initKhalaTokensServedStreamModel(),
    })

    expect(tokensServed(result.counter)).toBe(500)
    expect(result.stream.cursor).toBe(7)
    expect(result.stream.appliedEventRefs).toEqual([])
  })
})

describe('khalaTokensServedAfterScalarSeed', () => {
  test('establishes the displayed total when the counter is un-seeded', () => {
    const result = khalaTokensServedAfterScalarSeed(
      IdlePublicKhalaTokensServed(),
      { generatedAt: '2026-06-24T00:00:00.000Z', tokensServed: 1590000 },
    )

    expect(tokensServed(result)).toBe(1590000)
  })

  test('MONOTONE: a stale-low scalar value never clobbers a higher live total', () => {
    // This is the original backward-jump: the live (snapshot + stream) total is
    // 2_000_000 and a stale-cached scalar reconcile returns 1_590_000. The seed
    // must hold the higher value, not drop to the stale one.
    const result = khalaTokensServedAfterScalarSeed(seeded(2000000), {
      generatedAt: '2026-06-24T00:00:00.000Z',
      tokensServed: 1590000,
    })

    expect(tokensServed(result)).toBe(2000000)
  })

  test('raises the displayed total when the scalar is higher', () => {
    const result = khalaTokensServedAfterScalarSeed(seeded(1000), {
      generatedAt: '2026-06-24T00:00:05.000Z',
      tokensServed: 1500,
    })

    expect(tokensServed(result)).toBe(1500)
  })
})

describe('seed + cursor-replay + reconnect counts each event exactly once', () => {
  // The whole point of the fix: seeding from the snapshot summary + cursor, then
  // applying ONLY events after that cursor, then a reconnect that replays from
  // the last cursor, must converge to the true ledger total with NO double-count
  // and NO backward step at any point.
  test('end-to-end monotonic convergence to the ledger total', () => {
    const samples: Array<number> = []
    const record = (counter: PublicKhalaTokensServedModel): void => {
      const value = tokensServed(counter)
      if (value !== null) {
        samples.push(value)
      }
    }

    // 1) Seed from the snapshot: authoritative total 1_000_000 at cursor 3 (events
    //    1..3 are already baked into this total).
    const afterSnapshot = khalaTokensServedAfterSnapshot({
      counter: IdlePublicKhalaTokensServed(),
      cursor: 3,
      stream: initKhalaTokensServedStreamModel(),
      summary: {
        observedAt: '2026-06-24T00:00:00.000Z',
        tokensServedTotal: 1_000_000,
      },
    })
    record(afterSnapshot.counter)
    expect(afterSnapshot.stream.cursor).toBe(3)

    // 2) A cursor-replay re-delivers event 3 (already in the seed). It must NOT
    //    re-add — the authoritative total is <= displayed, so no movement.
    const replayOld = applyKhalaTokensServedPatch({
      counter: afterSnapshot.counter,
      patch: deltaPatch({
        eventRef: 'event.3',
        seq: 3,
        tokensServedDelta: 500,
        tokensServedTotal: 1_000_000,
      }),
      stream: afterSnapshot.stream,
    })
    record(replayOld.counter)

    // 3) New live events after the cursor advance the total to each running total.
    const live4 = applyKhalaTokensServedPatch({
      counter: replayOld.counter,
      patch: deltaPatch({
        eventRef: 'event.4',
        seq: 4,
        tokensServedDelta: 100,
        tokensServedTotal: 1_000_100,
      }),
      stream: replayOld.stream,
    })
    record(live4.counter)

    const live5 = applyKhalaTokensServedPatch({
      counter: live4.counter,
      patch: deltaPatch({
        eventRef: 'event.5',
        seq: 5,
        tokensServedDelta: 250,
        tokensServedTotal: 1_000_350,
      }),
      stream: live4.stream,
    })
    record(live5.counter)

    // 4) A reconnect replays from the last cursor (4..5). Both events are already
    //    applied; the authoritative totals are <= displayed → no double-count.
    const reconnect4 = applyKhalaTokensServedPatch({
      counter: live5.counter,
      patch: deltaPatch({
        eventRef: 'event.4',
        seq: 4,
        tokensServedDelta: 100,
        tokensServedTotal: 1_000_100,
      }),
      stream: live5.stream,
    })
    record(reconnect4.counter)
    const reconnect5 = applyKhalaTokensServedPatch({
      counter: reconnect4.counter,
      patch: deltaPatch({
        eventRef: 'event.5',
        seq: 5,
        tokensServedDelta: 250,
        tokensServedTotal: 1_000_350,
      }),
      stream: reconnect4.stream,
    })
    record(reconnect5.counter)

    // 5) A stale scalar reconcile (cache lag) returns the OLD pre-live total. The
    //    monotone scalar seed must hold the higher live total — no backward jump.
    const afterStaleScalar = khalaTokensServedAfterScalarSeed(reconnect5.counter, {
      generatedAt: '2026-06-24T00:00:02.000Z',
      tokensServed: 1_000_000,
    })
    record(afterStaleScalar)

    // Converged to the true ledger total.
    expect(tokensServed(afterStaleScalar)).toBe(1_000_350)
    // MONOTONIC: every sampled value is >= the previous one. Never backward.
    samples.forEach((value, index) => {
      if (index > 0) {
        expect(value).toBeGreaterThanOrEqual(samples[index - 1] as number)
      }
    })
    // And it never over-counted: the max equals the true ledger total (no 2M).
    expect(Math.max(...samples)).toBe(1_000_350)
  })

  // Property proof: over many randomized interleavings of seed + live events +
  // cursor-replays + reconnects + stale scalar reconciles, the displayed counter
  // is ALWAYS monotonically non-decreasing AND converges EXACTLY to the true
  // ledger total — never the over-count, never a backward step. This is the
  // burst-of-completions + reconnect + cache-lag scenario from the bug report,
  // sampled deterministically rather than against a live socket.
  test('randomized interleavings stay monotonic and converge to the ledger total', () => {
    // A small seeded PRNG so the proof is deterministic + reproducible.
    let prngState = 0x2545f4914f6cdd1d
    const rng = (): number => {
      prngState = (prngState * 6364136223846793005 + 1442695040888963407) % 2 ** 53
      return prngState / 2 ** 53
    }
    const pick = <T>(items: ReadonlyArray<T>): T =>
      items[Math.floor(rng() * items.length)] as T

    for (let trial = 0; trial < 200; trial = trial + 1) {
      // A ground-truth ledger: a sequence of events each with a positive delta;
      // the running total after event i is the SUM of deltas 0..i. This is the
      // SAME authoritative total the server reads + stamps on every event.
      const eventCount = 4 + Math.floor(rng() * 10)
      const deltas = Array.from(
        { length: eventCount },
        () => 1 + Math.floor(rng() * 5000),
      )
      const runningTotals = deltas.reduce<Array<number>>((acc, delta, index) => {
        acc.push((index === 0 ? 0 : (acc[index - 1] as number)) + delta)
        return acc
      }, [])
      const ledgerTotal = runningTotals[eventCount - 1] as number

      // Seed from a snapshot at a random cursor: events 0..seedCursor-1 are baked
      // into the seeded total (the server's summary = ledger SUM at that point).
      const seedCursor = Math.floor(rng() * eventCount)
      const seedTotal =
        seedCursor === 0 ? 0 : (runningTotals[seedCursor - 1] as number)

      let result = khalaTokensServedAfterSnapshot({
        counter: IdlePublicKhalaTokensServed(),
        cursor: seedCursor,
        stream: initKhalaTokensServedStreamModel(),
        summary:
          seedCursor === 0
            ? null
            : { observedAt: '2026-06-24T00:00:00.000Z', tokensServedTotal: seedTotal },
      })
      // If the snapshot had no summary (brand-new scope), the scalar seed
      // establishes the (zero) displayed total — the socket-down fallback path.
      if (seedCursor === 0) {
        result = {
          counter: khalaTokensServedAfterScalarSeed(result.counter, {
            generatedAt: '2026-06-24T00:00:00.000Z',
            tokensServed: 0,
          }),
          stream: result.stream,
        }
      }

      let last = tokensServed(result.counter) ?? 0
      const assertMonotone = (): void => {
        const value = tokensServed(result.counter) ?? 0
        expect(value).toBeGreaterThanOrEqual(last)
        last = value
      }

      // The live events still to be delivered (those after the seed cursor).
      const pending = Array.from(
        { length: eventCount - seedCursor },
        (_unused, index) => seedCursor + index,
      )

      // Walk forward, randomly: deliver the next pending event, OR replay an
      // already-delivered event (reconnect/cursor-replay), OR fire a stale scalar
      // reconcile carrying an OLD total (cache lag). Every step must stay
      // monotonic and must never exceed the ledger total (no over-count).
      const delivered: Array<number> = []
      while (pending.length > 0) {
        const action = pick(['deliver', 'replay', 'staleScalar', 'deliver'])

        if (action === 'deliver') {
          const eventIndex = pending.shift() as number
          delivered.push(eventIndex)
          result = applyKhalaTokensServedPatch({
            counter: result.counter,
            patch: deltaPatch({
              eventRef: `event.${eventIndex}`,
              seq: eventIndex + 1,
              tokensServedDelta: deltas[eventIndex] as number,
              tokensServedTotal: runningTotals[eventIndex] as number,
            }),
            stream: result.stream,
          })
        } else if (action === 'replay' && delivered.length > 0) {
          const eventIndex = pick(delivered)
          result = applyKhalaTokensServedPatch({
            counter: result.counter,
            patch: deltaPatch({
              eventRef: `event.${eventIndex}`,
              seq: eventIndex + 1,
              tokensServedDelta: deltas[eventIndex] as number,
              tokensServedTotal: runningTotals[eventIndex] as number,
            }),
            stream: result.stream,
          })
        } else {
          // A stale scalar reconcile returns the seed-era (lower) total.
          result = {
            counter: khalaTokensServedAfterScalarSeed(result.counter, {
              generatedAt: '2026-06-24T00:00:02.000Z',
              tokensServed: seedTotal,
            }),
            stream: result.stream,
          }
        }

        assertMonotone()
        expect(tokensServed(result.counter) ?? 0).toBeLessThanOrEqual(ledgerTotal)
      }

      // Converged EXACTLY to the ledger total — not the over-count, not a stale low.
      expect(tokensServed(result.counter)).toBe(ledgerTotal)
    }
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
