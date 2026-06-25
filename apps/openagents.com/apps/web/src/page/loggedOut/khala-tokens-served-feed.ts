import { CursorGap, SyncPatch } from '@openagentsinc/sync-schema'
import { Array as Arr, Option, Schema as S } from 'effect'

import {
  type KhalaTokensServedStreamModel,
  type PublicKhalaTokensServedModel,
  KhalaTokensServedStreamModel as KhalaTokensServedStreamModelSchema,
  LoadedPublicKhalaTokensServed,
} from './model'

// Live "Khala Tokens Served" delta stream (#6231). The homepage subscribes to
// ONE public, read-only sync room scope and rolls the odometer up instantly as
// each served completion pushes a public-safe delta — no per-second poll/SUM.
// This module owns the pure reducers; update.ts wires them to messages and
// subscriptions.ts opens the socket. Mirrors `settled-feed.ts`.

export const KHALA_TOKENS_SERVED_ID = 'network'
export const KHALA_TOKENS_SERVED_SCOPE = `public-khala-tokens-served:${KHALA_TOKENS_SERVED_ID}`
export const KHALA_TOKENS_SERVED_DELTAS_COLLECTION = 'tokens_served_deltas'

// Bound the de-dupe ledger so a long-lived session can't grow it without limit.
// Far more than any plausible cursor-replay batch; older refs age out FIFO.
const MAX_APPLIED_EVENT_REFS = 256

const PublicKhalaTokensServedDelta = S.Struct({
  eventRef: S.String,
  observedAt: S.String,
  tokensServedDelta: S.Number,
})
type PublicKhalaTokensServedDelta = typeof PublicKhalaTokensServedDelta.Type

const decodeDelta = (
  value: unknown,
): Option.Option<PublicKhalaTokensServedDelta> =>
  S.decodeUnknownOption(PublicKhalaTokensServedDelta)(value)

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

// Apply one streamed delta to BOTH slices at once: advance the stream cursor +
// record the applied event ref (so a reconnect/cursor-replay never re-applies
// it), and increment the running total on the scalar counter model. A delta for
// an already-applied event ref, an undecodable patch, a non-put op, or an
// unknown collection only advances the cursor — it never moves the total. A
// delta that arrives before the counter has been seeded (Loaded) is dropped
// (cursor still advances): the seed fetch + this stream both read the same
// authoritative SUM, so the next seed reconcile catches any pre-seed delta.
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
      const increment = Math.max(0, Math.trunc(delta.tokensServedDelta))

      if (alreadyApplied || increment <= 0) {
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

      // Only roll up a counter that has been seeded (Loaded). Before the seed
      // lands, drop the increment (the cursor + ref still advance) — the seed
      // reconcile reads the same authoritative SUM and catches it.
      if (input.counter._tag !== 'PublicKhalaTokensServedLoaded') {
        return { counter: input.counter, stream: streamWithRef }
      }

      return {
        counter: LoadedPublicKhalaTokensServed({
          served: {
            tokensServed: input.counter.served.tokensServed + increment,
            generatedAt: delta.observedAt,
          },
        }),
        stream: streamWithRef,
      }
    },
  })
}

// A cursor gap means we may have missed deltas; advance the cursor to the
// received seq so the next reconnect replays from there. The slow reconcile poll
// (and the seed) re-read the authoritative SUM, so a gap is self-healing.
export const khalaTokensServedStreamAfterCursorGap = (
  model: KhalaTokensServedStreamModel,
  gap: CursorGap,
): KhalaTokensServedStreamModel =>
  KhalaTokensServedStreamModelSchema({
    ...model,
    cursor: Math.max(model.cursor, gap.receivedSeq),
  })
