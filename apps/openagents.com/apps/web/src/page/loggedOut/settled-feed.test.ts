import {
  ChangelogEntry,
  EntityId as KhalaSyncEntityId,
  EntityType,
  SyncScope as KhalaSyncScope,
  SyncVersion,
} from '@openagentsinc/khala-sync'
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

import { initSettledFeedModel } from './model'
import {
  SETTLED_FEED_EVENTS_COLLECTION,
  SETTLED_FEED_SCOPE,
  SETTLED_FEED_SUMMARY_COLLECTION,
  applySettledFeedPatch,
  latestSettledEvent,
  settledFeedAfterCursorGap,
  settledFeedAfterSnapshot,
  settledFeedFailed,
  settledFeedOpen,
  settledFeedPatchFromChangelogEntry,
  settledFeedStreamSnapshotSettled,
} from './settled-feed'

const eventPatch = (
  input: Readonly<{
    amountSats: number
    contributorRef: string
    eventRef: string
    party: 'validator' | 'worker'
    seq: number
    totalSettledCount: number
    totalSettledSats: number
  }>,
): SyncPatch =>
  new SyncPatch({
    scope: SyncScope.make(SETTLED_FEED_SCOPE),
    seq: SyncSequence.make(input.seq),
    collection: CollectionName.make(SETTLED_FEED_EVENTS_COLLECTION),
    op: 'put',
    id: EntityId.make(input.eventRef),
    value: {
      amountSats: input.amountSats,
      challengeRef: 'challenge.tassadar.window.0001',
      contributorRef: input.contributorRef,
      eventRef: input.eventRef,
      party: input.party,
      runRef: 'run.tassadar.poc',
      settledAt: '2026-06-17T00:00:00.000Z',
      totalSettledCount: input.totalSettledCount,
      totalSettledSats: input.totalSettledSats,
      windowRef: 'window.tassadar.0001',
    },
    serverTime: IsoTimestamp.make('2026-06-17T00:00:00.000Z'),
  })

describe('applySettledFeedPatch', () => {
  test('a streamed settled event advances totals and prepends to the feed', () => {
    const initial = initSettledFeedModel()

    const afterFirst = applySettledFeedPatch(
      initial,
      eventPatch({
        amountSats: 5,
        contributorRef: 'pylon.worker.orrery',
        eventRef: 'settled.0',
        party: 'worker',
        seq: 1,
        totalSettledCount: 1,
        totalSettledSats: 5,
      }),
    )

    expect(afterFirst.totalSettledSats).toBe(5)
    expect(afterFirst.totalSettledCount).toBe(1)
    expect(afterFirst.cursor).toBe(1)
    expect(afterFirst.events).toHaveLength(1)

    const afterSecond = applySettledFeedPatch(
      afterFirst,
      eventPatch({
        amountSats: 5,
        contributorRef: 'pylon.validator.whitefang',
        eventRef: 'settled.1',
        party: 'validator',
        seq: 2,
        totalSettledCount: 2,
        totalSettledSats: 10,
      }),
    )

    expect(afterSecond.totalSettledSats).toBe(10)
    expect(afterSecond.totalSettledCount).toBe(2)
    expect(afterSecond.cursor).toBe(2)
    expect(afterSecond.events).toHaveLength(2)
    // Latest first.
    expect(latestSettledEvent(afterSecond)?.eventRef).toBe('settled.1')
  })

  test('a repeated event ref does not double-count the rendered feed', () => {
    const initial = initSettledFeedModel()
    const patch = eventPatch({
      amountSats: 5,
      contributorRef: 'pylon.worker.orrery',
      eventRef: 'settled.0',
      party: 'worker',
      seq: 1,
      totalSettledCount: 1,
      totalSettledSats: 5,
    })

    const once = applySettledFeedPatch(initial, patch)
    const twice = applySettledFeedPatch(once, patch)

    expect(twice.events).toHaveLength(1)
  })

  test('a summary patch refreshes the running totals', () => {
    const initial = initSettledFeedModel()
    const patch = new SyncPatch({
      scope: SyncScope.make(SETTLED_FEED_SCOPE),
      seq: SyncSequence.make(3),
      collection: CollectionName.make(SETTLED_FEED_SUMMARY_COLLECTION),
      op: 'put',
      id: EntityId.make('summary'),
      value: { totalSettledCount: 7, totalSettledSats: 35 },
      serverTime: IsoTimestamp.make('2026-06-17T00:00:00.000Z'),
    })

    const updated = applySettledFeedPatch(initial, patch)

    expect(updated.totalSettledSats).toBe(35)
    expect(updated.totalSettledCount).toBe(7)
    expect(updated.cursor).toBe(3)
  })

  test('an unparseable value is ignored but still advances the cursor', () => {
    const initial = initSettledFeedModel()
    const patch = new SyncPatch({
      scope: SyncScope.make(SETTLED_FEED_SCOPE),
      seq: SyncSequence.make(4),
      collection: CollectionName.make(SETTLED_FEED_EVENTS_COLLECTION),
      op: 'put',
      id: EntityId.make('settled.bad'),
      value: { not: 'a settled event' },
      serverTime: IsoTimestamp.make('2026-06-17T00:00:00.000Z'),
    })

    const updated = applySettledFeedPatch(initial, patch)

    expect(updated.events).toHaveLength(0)
    expect(updated.cursor).toBe(4)
  })
})

describe('settledFeedAfterSnapshot', () => {
  test('seeds totals + cursor from the cold-read snapshot summary', () => {
    const updated = settledFeedAfterSnapshot(initSettledFeedModel(), {
      cursor: 12,
      summary: { totalSettledCount: 4, totalSettledSats: 20 },
    })

    expect(updated.cursor).toBe(12)
    expect(updated.totalSettledCount).toBe(4)
    expect(updated.totalSettledSats).toBe(20)
  })

  test('a missing summary keeps current totals (graceful fallback)', () => {
    const updated = settledFeedAfterSnapshot(initSettledFeedModel(), {
      cursor: 0,
      summary: null,
    })

    expect(updated.totalSettledSats).toBe(0)
    expect(updated.totalSettledCount).toBe(0)
  })
})

describe('connection state', () => {
  test('open then failed flips the connection without losing totals', () => {
    const seeded = settledFeedAfterSnapshot(initSettledFeedModel(), {
      cursor: 5,
      summary: { totalSettledCount: 2, totalSettledSats: 10 },
    })

    const open = settledFeedOpen(seeded)
    expect(open.connection).toBe('open')

    const failed = settledFeedFailed(open)
    expect(failed.connection).toBe('failed')
    expect(failed.totalSettledSats).toBe(10)
  })

  test('a cursor gap advances the cursor for replay', () => {
    const gap = new CursorGap({
      scope: SyncScope.make(SETTLED_FEED_SCOPE),
      expectedSeq: SyncSequence.make(3),
      receivedSeq: SyncSequence.make(9),
    })

    const updated = settledFeedAfterCursorGap(initSettledFeedModel(), gap)

    expect(updated.cursor).toBe(9)
  })
})

describe('settledFeedAfterSnapshot / settledFeedStreamSnapshotSettled (KS-6.4, #8414)', () => {
  test('a successful snapshot flips snapshotLoaded so the live-tail socket may open', () => {
    const updated = settledFeedAfterSnapshot(initSettledFeedModel(), {
      cursor: 12,
      summary: { totalSettledCount: 4, totalSettledSats: 20 },
    })

    expect(updated.snapshotLoaded).toBe(true)
  })

  test('a failed snapshot also flips snapshotLoaded (degraded: opens at cursor 0)', () => {
    expect(initSettledFeedModel().snapshotLoaded).toBe(false)

    const settled = settledFeedStreamSnapshotSettled(initSettledFeedModel())

    expect(settled.snapshotLoaded).toBe(true)
    expect(settled.cursor).toBe(0)
  })

  test('settledFeedStreamSnapshotSettled is idempotent and never rewinds an already-seeded cursor', () => {
    const seeded = settledFeedAfterSnapshot(initSettledFeedModel(), {
      cursor: 42,
      summary: null,
    })

    const settled = settledFeedStreamSnapshotSettled(seeded)

    expect(settled).toEqual(seeded)
  })
})

// KS-6.4 (#8414) cutover: `settledFeedPatchFromChangelogEntry` adapts one
// khala-sync `ChangelogEntry` (the `GET /api/sync/log` catch-up + `WS
// /api/sync/connect` `DeltaFrame` wire shape) into the legacy `SyncPatch`
// shape `applySettledFeedPatch` already understands, so the reducer above
// needed no changes for the engine cutover.
describe('settledFeedPatchFromChangelogEntry', () => {
  const entry = (
    input: Readonly<{
      entityId: string
      entityType: string
      op: 'upsert' | 'delete'
      postImageJson?: string
      version: number
    }>,
  ): ChangelogEntry =>
    new ChangelogEntry({
      scope: KhalaSyncScope.make(SETTLED_FEED_SCOPE),
      version: SyncVersion.make(input.version),
      entityType: EntityType.make(input.entityType),
      entityId: KhalaSyncEntityId.make(input.entityId),
      op: input.op,
      committedAt: '2026-07-05T00:00:00.000Z',
      ...(input.postImageJson === undefined
        ? {}
        : { postImageJson: input.postImageJson }),
    })

  test('adapts a settled_feed_event upsert into a settled_events put patch', () => {
    const patch = settledFeedPatchFromChangelogEntry(
      entry({
        entityId: 'settled.0',
        entityType: 'settled_feed_event',
        op: 'upsert',
        postImageJson: JSON.stringify({
          amountSats: 5,
          challengeRef: 'challenge.tassadar.window.0001',
          contributorRef: 'pylon.worker.orrery',
          eventRef: 'settled.0',
          party: 'worker',
          runRef: 'run.tassadar.poc',
          settledAt: '2026-07-05T00:00:00.000Z',
          totalSettledCount: 1,
          totalSettledSats: 5,
          windowRef: null,
        }),
        version: 1,
      }),
    )

    expect(patch).toBeDefined()
    expect(patch?.collection).toBe(SETTLED_FEED_EVENTS_COLLECTION)
    expect(patch?.op).toBe('put')
    expect(patch?.seq).toBe(1)
    expect(patch?.id).toBe('settled.0')
    expect(patch?.value).toMatchObject({ amountSats: 5, totalSettledCount: 1 })

    // Round-trips through the existing reducer unchanged.
    const updated = applySettledFeedPatch(initSettledFeedModel(), patch!)
    expect(updated.totalSettledSats).toBe(5)
    expect(updated.events).toHaveLength(1)
  })

  test('adapts a settled_feed_summary upsert into a settled_summary put patch', () => {
    const patch = settledFeedPatchFromChangelogEntry(
      entry({
        entityId: 'summary',
        entityType: 'settled_feed_summary',
        op: 'upsert',
        postImageJson: JSON.stringify({
          latestEventRef: 'settled.0',
          latestSettledAt: '2026-07-05T00:00:00.000Z',
          totalSettledCount: 7,
          totalSettledSats: 35,
          updatedAt: '2026-07-05T00:00:00.000Z',
        }),
        version: 3,
      }),
    )

    expect(patch?.collection).toBe(SETTLED_FEED_SUMMARY_COLLECTION)
    expect(patch?.value).toMatchObject({
      totalSettledCount: 7,
      totalSettledSats: 35,
    })
  })

  test('adapts a delete op with no value', () => {
    const patch = settledFeedPatchFromChangelogEntry(
      entry({
        entityId: 'settled.stale',
        entityType: 'settled_feed_event',
        op: 'delete',
        version: 5,
      }),
    )

    expect(patch?.op).toBe('delete')
    expect(patch?.value).toBeUndefined()
  })

  test('returns undefined for an unparseable upsert post-image', () => {
    const patch = settledFeedPatchFromChangelogEntry(
      entry({
        entityId: 'settled.bad',
        entityType: 'settled_feed_event',
        op: 'upsert',
        postImageJson: '{not json',
        version: 6,
      }),
    )

    expect(patch).toBeUndefined()
  })
})
