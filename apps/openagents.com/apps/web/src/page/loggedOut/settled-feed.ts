import {
  SETTLED_FEED_CHANNEL_ID,
  SETTLED_FEED_EVENT_ENTITY_TYPE,
  SETTLED_FEED_SUMMARY_ENTITY_TYPE,
  type ChangelogEntry,
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
import { Array as Arr, Option, Schema as S } from 'effect'

import { parseJsonRecord } from '../../json-boundary'
import {
  type PublicSettledFeedEvent,
  type SettledFeedModel,
  PublicSettledFeedEvent as PublicSettledFeedEventSchema,
  SettledFeedModel as SettledFeedModelSchema,
} from './model'

// Live settled feed (openagents #5311). The homepage subscribes to ONE public,
// read-only sync room scope and renders the settled total / latest settlement /
// feed count live as real Bitcoin settlements stream. This module owns the pure
// reducers; update.ts wires them to messages and subscriptions.ts opens the
// socket.

export const SETTLED_FEED_ID = 'tassadar'
// KS-6.4 (#8414) cutover: the scope is now the khala-sync engine's
// `scope.public.<channel>` id (`packages/khala-sync/src/settled-feed.ts`),
// read via `GET/WS /api/sync/log|connect` under the KS-8.x anonymous-read
// exception for `scope.public.*`. This REPLACES the legacy
// `public-settled-feed:tassadar` room scope — the legacy producer
// (`notifySyncScopes` in `tassadar-settled-feed-sync.ts`) has been retired
// now that this path is live.
export const SETTLED_FEED_SCOPE = `scope.public.${SETTLED_FEED_CHANNEL_ID}`
export const SETTLED_FEED_EVENTS_COLLECTION = 'settled_events'
export const SETTLED_FEED_SUMMARY_COLLECTION = 'settled_summary'

const ENTITY_TYPE_TO_COLLECTION: Readonly<Record<string, string>> = {
  [SETTLED_FEED_EVENT_ENTITY_TYPE]: SETTLED_FEED_EVENTS_COLLECTION,
  [SETTLED_FEED_SUMMARY_ENTITY_TYPE]: SETTLED_FEED_SUMMARY_COLLECTION,
}

// Adapt one khala-sync `ChangelogEntry` (the KS-6.4 cutover wire shape used by
// both `GET /api/sync/log` catch-up pages and `WS /api/sync/connect` live-tail
// `DeltaFrame`s) into the legacy `SyncPatch` shape the existing reducers below
// already understand — no reducer rewrite needed for the engine cutover.
// Returns `undefined` only if an upsert's post-image fails to parse as JSON
// (defensive; the server never emits this in practice).
export const settledFeedPatchFromChangelogEntry = (
  entry: ChangelogEntry,
): SyncPatch | undefined => {
  const collection = ENTITY_TYPE_TO_COLLECTION[entry.entityType] ?? entry.entityType

  let value: unknown
  if (entry.op === 'upsert') {
    if (entry.postImageJson === undefined) {
      return undefined
    }
    const record = parseJsonRecord(entry.postImageJson)
    if (record === undefined) {
      return undefined
    }
    value = record
  }

  return new SyncPatch({
    scope: SyncScope.make(entry.scope),
    seq: SyncSequence.make(entry.version),
    collection: CollectionName.make(collection),
    op: entry.op === 'upsert' ? 'put' : 'delete',
    id: EntityId.make(entry.entityId),
    ...(value === undefined ? {} : { value }),
    serverTime: IsoTimestamp.make(entry.committedAt),
  })
}

const MAX_RENDERED_EVENTS = 50

const decodeEvent = (value: unknown): Option.Option<PublicSettledFeedEvent> =>
  S.decodeUnknownOption(PublicSettledFeedEventSchema)(value)

const SettledFeedSummary = S.Struct({
  totalSettledCount: S.Number,
  totalSettledSats: S.Number,
})

export type SettledFeedSummary = typeof SettledFeedSummary.Type

const decodeSummary = (value: unknown): Option.Option<SettledFeedSummary> =>
  S.decodeUnknownOption(SettledFeedSummary)(value)

const withConnection = (
  model: SettledFeedModel,
  connection: SettledFeedModel['connection'],
): SettledFeedModel => SettledFeedModelSchema({ ...model, connection })

export const settledFeedConnecting = (
  model: SettledFeedModel,
): SettledFeedModel => withConnection(model, 'connecting')

export const settledFeedOpen = (model: SettledFeedModel): SettledFeedModel =>
  withConnection(model, 'open')

export const settledFeedClosed = (model: SettledFeedModel): SettledFeedModel =>
  withConnection(model, 'closed')

export const settledFeedFailed = (model: SettledFeedModel): SettledFeedModel =>
  withConnection(model, 'failed')

// Apply the public-safe snapshot summary (running totals + cursor) without
// reordering rendered events. Falls back gracefully — a missing/invalid summary
// just keeps current totals. Also flips `snapshotLoaded` (KS-6.4, #8414
// cutover) so the live-tail socket subscription — gated on that flag in
// subscriptions.ts — opens exactly once, at this seeded cursor, instead of at
// 0 (which would replay the scope's entire historical event log).
export const settledFeedAfterSnapshot = (
  model: SettledFeedModel,
  input: Readonly<{
    cursor: number
    summary: SettledFeedSummary | null
  }>,
): SettledFeedModel =>
  SettledFeedModelSchema({
    ...model,
    cursor: input.cursor,
    totalSettledCount: input.summary?.totalSettledCount ?? model.totalSettledCount,
    totalSettledSats: input.summary?.totalSettledSats ?? model.totalSettledSats,
    snapshotLoaded: true,
  })

// Mark the snapshot phase complete so the live-tail socket may open (KS-6.4,
// #8414 cutover; same #6324 race the tokens-served stream already guards
// against), WITHOUT moving the cursor. Used ONLY on a snapshot-load FAILURE:
// the success path flips this via `settledFeedAfterSnapshot` AFTER seeding a
// real cursor; a failure never learned one, so the socket falls back to
// opening at 0 — tolerable because this is the rare degraded path, not the
// common case, and matches the pre-existing "no seed => no live totals until
// reload" fallback posture.
export const settledFeedStreamSnapshotSettled = (
  model: SettledFeedModel,
): SettledFeedModel =>
  model.snapshotLoaded
    ? model
    : SettledFeedModelSchema({ ...model, snapshotLoaded: true })

const insertEvent = (
  events: ReadonlyArray<PublicSettledFeedEvent>,
  event: PublicSettledFeedEvent,
): ReadonlyArray<PublicSettledFeedEvent> => {
  const withoutDuplicate = events.filter(
    existing => existing.eventRef !== event.eventRef,
  )

  return [event, ...withoutDuplicate].slice(0, MAX_RENDERED_EVENTS)
}

// Apply one streamed sync patch. Settled-event puts prepend to the live feed
// and advance the running totals (the event carries cumulative totals);
// summary puts refresh totals directly. Deletes/invalidations and unknown
// collections are ignored. The cursor always advances to the patch seq.
export const applySettledFeedPatch = (
  model: SettledFeedModel,
  patch: SyncPatch,
): SettledFeedModel => {
  const cursor = Math.max(model.cursor, patch.seq)

  if (patch.op !== 'put' && patch.op !== 'patch') {
    return SettledFeedModelSchema({ ...model, cursor })
  }

  if (patch.collection === SETTLED_FEED_EVENTS_COLLECTION) {
    return Option.match(decodeEvent(patch.value), {
      onNone: () => SettledFeedModelSchema({ ...model, cursor }),
      onSome: event =>
        SettledFeedModelSchema({
          ...model,
          cursor,
          events: insertEvent(model.events, event),
          totalSettledCount: Math.max(
            model.totalSettledCount,
            event.totalSettledCount,
          ),
          totalSettledSats: Math.max(
            model.totalSettledSats,
            event.totalSettledSats,
          ),
        }),
    })
  }

  if (patch.collection === SETTLED_FEED_SUMMARY_COLLECTION) {
    return Option.match(decodeSummary(patch.value), {
      onNone: () => SettledFeedModelSchema({ ...model, cursor }),
      onSome: summary =>
        SettledFeedModelSchema({
          ...model,
          cursor,
          totalSettledCount: Math.max(
            model.totalSettledCount,
            summary.totalSettledCount,
          ),
          totalSettledSats: Math.max(
            model.totalSettledSats,
            summary.totalSettledSats,
          ),
        }),
    })
  }

  return SettledFeedModelSchema({ ...model, cursor })
}

// A cursor gap means we may have missed changes; advance the cursor to the
// received seq so the next reconnect replays from there. The non-realtime
// totals the homepage already fetches remain the safe fallback.
export const settledFeedAfterCursorGap = (
  model: SettledFeedModel,
  gap: CursorGap,
): SettledFeedModel =>
  SettledFeedModelSchema({
    ...model,
    cursor: Math.max(model.cursor, gap.receivedSeq),
  })

export const latestSettledEvent = (
  model: SettledFeedModel,
): PublicSettledFeedEvent | undefined => Arr.head(model.events).pipe(
  Option.getOrUndefined,
)
