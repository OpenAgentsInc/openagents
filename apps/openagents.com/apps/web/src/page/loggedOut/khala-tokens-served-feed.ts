import { CursorGap, SyncPatch } from '@openagentsinc/sync-schema'
import { Array as Arr, Option, Schema as S } from 'effect'

import {
  type KhalaTokensServedStreamModel,
  type PublicKhalaTokensServedModel,
  KhalaTokensServedStreamModel as KhalaTokensServedStreamModelSchema,
  LoadedPublicKhalaTokensServed,
} from './model'

// Live "Khala Tokens Served" feed (#6231 + follow-up). The homepage subscribes
// to ONE public, read-only sync room scope and rolls the odometer up instantly
// as each served completion pushes a public-safe event — no per-second poll/SUM.
// This module owns the pure reducers; update.ts wires them to messages and
// subscriptions.ts opens the socket. Mirrors `settled-feed.ts`.
//
// SINGLE SOURCE OF TRUTH / MONOTONICITY. The running total is AUTHORITATIVE on
// the server: every event AND the snapshot summary carry `tokensServedTotal`
// (the live ledger SUM after that row). The client seeds the total + cursor from
// ONE snapshot read (`khalaTokensServedAfterSnapshot`) and applies only events
// after that cursor, taking `max(displayed, total)`. So the counter never
// double-counts a pre-seed event and never moves backward — it converges exactly
// to the ledger SUM with no periodic scalar reconcile clobbering it back down.

export const KHALA_TOKENS_SERVED_ID = 'network'
export const KHALA_TOKENS_SERVED_SCOPE = `public-khala-tokens-served:${KHALA_TOKENS_SERVED_ID}`
export const KHALA_TOKENS_SERVED_DELTAS_COLLECTION = 'tokens_served_deltas'
export const KHALA_TOKENS_SERVED_SUMMARY_COLLECTION = 'tokens_served_summary'

// Bound the de-dupe ledger so a long-lived session can't grow it without limit.
// Far more than any plausible cursor-replay batch; older refs age out FIFO. With
// the authoritative-total `max` reducer this is belt-and-suspenders, but it keeps
// a replayed event from re-adding to `appliedEventRefs` redundantly.
const MAX_APPLIED_EVENT_REFS = 256

const PublicKhalaTokensServedDelta = S.Struct({
  eventRef: S.String,
  observedAt: S.String,
  tokensServedDelta: S.Number,
  tokensServedTotal: S.Number,
})
type PublicKhalaTokensServedDelta = typeof PublicKhalaTokensServedDelta.Type

const PublicKhalaTokensServedSummary = S.Struct({
  observedAt: S.String,
  tokensServedTotal: S.Number,
})
type PublicKhalaTokensServedSummary =
  typeof PublicKhalaTokensServedSummary.Type

const decodeDelta = (
  value: unknown,
): Option.Option<PublicKhalaTokensServedDelta> =>
  S.decodeUnknownOption(PublicKhalaTokensServedDelta)(value)

const decodeSummary = (
  value: unknown,
): Option.Option<PublicKhalaTokensServedSummary> =>
  S.decodeUnknownOption(PublicKhalaTokensServedSummary)(value)

const withConnection = (
  model: KhalaTokensServedStreamModel,
  connection: KhalaTokensServedStreamModel['connection'],
): KhalaTokensServedStreamModel =>
  KhalaTokensServedStreamModelSchema({ ...model, connection })

export const khalaTokensServedStreamConnecting = (
  model: KhalaTokensServedStreamModel,
): KhalaTokensServedStreamModel => withConnection(model, 'connecting')

export const khalaTokensServedStreamOpen = (
  model: KhalaTokensServedStreamModel,
): KhalaTokensServedStreamModel => withConnection(model, 'open')

export const khalaTokensServedStreamClosed = (
  model: KhalaTokensServedStreamModel,
): KhalaTokensServedStreamModel => withConnection(model, 'closed')

export const khalaTokensServedStreamFailed = (
  model: KhalaTokensServedStreamModel,
): KhalaTokensServedStreamModel => withConnection(model, 'failed')

// Raise the displayed total to `total` if it is higher, never lower. Monotonic by
// construction: an authoritative total can only advance the counter. A counter
// that has not been seeded yet (not Loaded) is left untouched — the snapshot seed
// (or the scalar fallback) establishes the first authoritative value.
const raiseCounterTo = (
  counter: PublicKhalaTokensServedModel,
  input: Readonly<{ generatedAt: string; total: number }>,
): PublicKhalaTokensServedModel => {
  if (counter._tag !== 'PublicKhalaTokensServedLoaded') {
    return counter
  }

  if (input.total <= counter.served.tokensServed) {
    return counter
  }

  return LoadedPublicKhalaTokensServed({
    served: {
      tokensServed: input.total,
      generatedAt: input.generatedAt,
    },
  })
}

// Seed the displayed total from the scalar SUM endpoint (the socket-down
// fallback / brand-new-scope seed). MONOTONE: if the counter is already Loaded it
// is only ever raised, never lowered — so a stale-low cached scalar value can
// never clobber a higher live total back down (the original oscillation). An
// un-seeded counter is established at the scalar value.
export const khalaTokensServedAfterScalarSeed = (
  counter: PublicKhalaTokensServedModel,
  served: Readonly<{ generatedAt: string; tokensServed: number }>,
): PublicKhalaTokensServedModel => {
  const total = Math.max(0, Math.trunc(served.tokensServed))

  if (counter._tag !== 'PublicKhalaTokensServedLoaded') {
    return LoadedPublicKhalaTokensServed({
      served: { tokensServed: total, generatedAt: served.generatedAt },
    })
  }

  return raiseCounterTo(counter, { generatedAt: served.generatedAt, total })
}

// Seed the running total + cursor from ONE public snapshot read (the room's
// `summary` record carries the authoritative running ledger total; the snapshot
// carries the cursor). Subscribing strictly from this cursor means events ALREADY
// baked into the seeded total are never replayed-and-re-added, eliminating the
// seed-vs-replay double count. A missing/invalid summary just advances the cursor
// and leaves the current total (the scalar fallback seed still applies).
export const khalaTokensServedAfterSnapshot = (
  input: Readonly<{
    counter: PublicKhalaTokensServedModel
    cursor: number
    stream: KhalaTokensServedStreamModel
    summary: PublicKhalaTokensServedSummary | null
  }>,
): Readonly<{
  counter: PublicKhalaTokensServedModel
  stream: KhalaTokensServedStreamModel
}> => {
  const seededStream = KhalaTokensServedStreamModelSchema({
    ...input.stream,
    cursor: Math.max(input.stream.cursor, input.cursor),
  })

  if (input.summary === null) {
    return { counter: input.counter, stream: seededStream }
  }

  const total = Math.max(0, Math.trunc(input.summary.tokensServedTotal))
  const seededCounter =
    input.counter._tag === 'PublicKhalaTokensServedLoaded'
      ? raiseCounterTo(input.counter, {
          generatedAt: input.summary.observedAt,
          total,
        })
      : LoadedPublicKhalaTokensServed({
          served: {
            tokensServed: total,
            generatedAt: input.summary.observedAt,
          },
        })

  return { counter: seededCounter, stream: seededStream }
}

// Apply one streamed patch to BOTH slices at once: advance the stream cursor +
// record the applied event ref, and raise the running total to the event's
// AUTHORITATIVE `tokensServedTotal` (never below the current displayed value, so
// the counter is monotonic). A patch for an already-applied event ref, an
// undecodable value, a non-put op, or an unknown collection only advances the
// cursor. A summary put refreshes the total the same monotonic way. Because the
// total is authoritative (not an additive delta), a replayed/duplicate event is
// inherently safe; the de-dupe ledger is a redundant guard.
export const applyKhalaTokensServedPatch = (
  input: Readonly<{
    counter: PublicKhalaTokensServedModel
    patch: SyncPatch
    stream: KhalaTokensServedStreamModel
  }>,
): Readonly<{
  counter: PublicKhalaTokensServedModel
  stream: KhalaTokensServedStreamModel
}> => {
  const cursor = Math.max(input.stream.cursor, input.patch.seq)
  const advancedStream = KhalaTokensServedStreamModelSchema({
    ...input.stream,
    cursor,
  })
  const unchanged = { counter: input.counter, stream: advancedStream }

  if (input.patch.op !== 'put' && input.patch.op !== 'patch') {
    return unchanged
  }

  if (input.patch.collection === KHALA_TOKENS_SERVED_SUMMARY_COLLECTION) {
    return Option.match(decodeSummary(input.patch.value), {
      onNone: () => unchanged,
      onSome: summary => ({
        counter: raiseCounterTo(input.counter, {
          generatedAt: summary.observedAt,
          total: Math.max(0, Math.trunc(summary.tokensServedTotal)),
        }),
        stream: advancedStream,
      }),
    })
  }

  if (input.patch.collection !== KHALA_TOKENS_SERVED_DELTAS_COLLECTION) {
    return unchanged
  }

  return Option.match(decodeDelta(input.patch.value), {
    onNone: () => unchanged,
    onSome: delta => {
      const alreadyApplied = Arr.contains(
        input.stream.appliedEventRefs,
        delta.eventRef,
      )

      if (alreadyApplied) {
        return unchanged
      }

      const appliedEventRefs = [
        ...input.stream.appliedEventRefs,
        delta.eventRef,
      ].slice(-MAX_APPLIED_EVENT_REFS)
      const streamWithRef = KhalaTokensServedStreamModelSchema({
        ...advancedStream,
        appliedEventRefs,
      })

      return {
        counter: raiseCounterTo(input.counter, {
          generatedAt: delta.observedAt,
          total: Math.max(0, Math.trunc(delta.tokensServedTotal)),
        }),
        stream: streamWithRef,
      }
    },
  })
}

// A cursor gap means we may have missed deltas; advance the cursor to the
// received seq so the next reconnect replays from there. The authoritative total
// on each event (and the summary) is self-healing: the next event raises the
// counter to the true running total regardless of any skipped intermediate event.
export const khalaTokensServedStreamAfterCursorGap = (
  model: KhalaTokensServedStreamModel,
  gap: CursorGap,
): KhalaTokensServedStreamModel =>
  KhalaTokensServedStreamModelSchema({
    ...model,
    cursor: Math.max(model.cursor, gap.receivedSeq),
  })
