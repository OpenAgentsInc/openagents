import { CursorGap, SyncPatch } from '@openagentsinc/sync-schema'
import { Array as Arr, Option, Schema as S } from 'effect'

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
export const SETTLED_FEED_SCOPE = `public-settled-feed:${SETTLED_FEED_ID}`
export const SETTLED_FEED_EVENTS_COLLECTION = 'settled_events'
export const SETTLED_FEED_SUMMARY_COLLECTION = 'settled_summary'

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
// just keeps current totals.
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
  })

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
