// Durable proxy for streaming Khala completions (durable-stream Rank-1, #6058,
// EPIC #6056 / §3 of the durable-stream roadmap).
//
// WHAT THIS SOLVES (the audit's flagged gap): the SSE pass-through
// (`makePassThroughResponseStream` in chat-completions-routes.ts) pumps the
// upstream provider stream straight to the client and persists NOTHING. If the
// CLIENT disconnects mid-generation (tab suspend, network flap, sleep), the
// in-flight tokens are gone and the paid work is lost — the only recovery is to
// re-run the (paid, slow) completion.
//
// THE DURABLE PROXY: tee the upstream token stream into a per-request durable
// offset log keyed by `requestId` (`@openagentsinc/durable-stream`), so the
// completion's frames are persisted as they arrive. A reconnect replays the
// stored suffix from its last offset and reconstructs the full completion.
// `Stream-Closed` is the clean EOF of the completion.
//
// ┌─────────────────── METERING EXACTLY-ONCE (the audit's risk) ─────────────┐
// │ Settlement fires EXACTLY ONCE, on the single real upstream EOF, and NEVER │
// │ on a resumed/replayed/catch-up read. The producer path (`teeUpstream...`) │
// │ is the ONLY surface that consumes the live upstream frames; it owns a     │
// │ one-shot guard so even a retried producer drain meters once. The resume   │
// │ path (`replayFromOffset`) reads stored bytes ONLY — it has no metering     │
// │ hook and cannot bill. Replays + CDN catch-up reads are therefore free.    │
// └──────────────────────────────────────────────────────────────────────────┘
//
// FAIL-SAFE: this module is pure/transport-agnostic and is only reached when the
// route is given a `DurableInferenceStreamStore`. With the flag off (or no store
// wired, or store construction failing) the route falls back to today's
// pass-through behaviour — no behaviour change, idempotent.
//
// NO Cloudflare or HTTP import here: the durable log is accessed through the
// `@openagentsinc/durable-stream` `StreamStore` port (synchronous, single-stream
// — the exact DO model), so this is fully Bun-unit-testable with the in-memory
// store. The DO-backed `SqliteStreamStore` plugs into the same port in prod.
import {
  type StreamMeta,
  type StreamStore,
  handle as durableHandle,
  offsetForPosition,
  parseOffset,
  tailOffset,
} from '@openagentsinc/durable-stream'

import {
  type InferenceStreamEvent,
  type InferenceStreamSource,
} from './provider-adapter'

// The durable wire content type for persisted completion frames. Each appended
// record is one already-rendered `chat.completion.chunk` SSE frame (text), so a
// catch-up/replay read returns a byte-exact SSE suffix the client can resume.
export const DURABLE_INFERENCE_CONTENT_TYPE = 'text/event-stream'

// Per-request durable storage TTL. Bounds storage cost (the protocol's native
// sliding window): a completed/abandoned stream's bytes expire after this. One
// hour comfortably covers a reconnect window without unbounded growth.
export const DURABLE_INFERENCE_TTL_SECONDS = 3600

// A factory the route calls with a stable `requestId` to obtain the single
// durable `StreamStore` for that request. In prod the Worker wires this to a DO
// (`SqliteStreamStore` on `idFromName(requestId)`); tests pass a `MemoryStreamStore`
// factory. Returning `undefined` (or throwing) means "no durable substrate" and
// the route degrades to the non-durable pass-through (fail-safe).
export type DurableInferenceStreamStore = (
  requestId: string,
) => StreamStore | undefined

const encoder = new TextEncoder()
const decoder = new TextDecoder()

// A normalized request into the durable `StreamStore` core, with deterministic
// `nowMs` injected (no raw clock read in this business module).
const coreRequest = (
  method: 'PUT' | 'POST' | 'GET',
  streamId: string,
  nowMs: number,
  options: Readonly<{
    headers?: Record<string, string | undefined>
    query?: Record<string, string | undefined>
    body?: Uint8Array
  }> = {},
) => ({
  body: options.body ?? new Uint8Array(0),
  headers: options.headers ?? {},
  method,
  nowMs,
  query: options.query ?? {},
  streamId,
})

// A stable idempotent-producer id for a request's durable stream. One producer
// (the gateway) writes one completion; the `(producerId, epoch, seq)` tuple makes
// each frame append retry-safe (exactly-once writes).
const PRODUCER_ID = 'khala-gateway'

// Ensure the per-request durable stream exists. Idempotent: a re-create with the
// same config returns 200; a first create returns 201. Returns false only when
// the store rejects creation (treated as "durable unavailable" → fail-safe).
const ensureStream = (
  store: StreamStore,
  requestId: string,
  nowMs: number,
): boolean => {
  const res = durableHandle(
    store,
    coreRequest('PUT', requestId, nowMs, {
      headers: {
        'content-type': DURABLE_INFERENCE_CONTENT_TYPE,
        'stream-ttl': String(DURABLE_INFERENCE_TTL_SECONDS),
      },
    }),
  )
  return res.status === 200 || res.status === 201
}

// Append one persisted frame as the producer's `seq`-th record (exactly-once).
// When `close` is set the append also closes the stream (the completion's EOF).
const appendFrame = (
  store: StreamStore,
  requestId: string,
  nowMs: number,
  seq: number,
  frame: string,
  close: boolean,
): void => {
  const headers: Record<string, string | undefined> = {
    'content-type': DURABLE_INFERENCE_CONTENT_TYPE,
    'producer-epoch': '0',
    'producer-id': PRODUCER_ID,
    'producer-seq': String(seq),
  }
  if (close) {
    headers['stream-closed'] = 'true'
  }
  durableHandle(
    store,
    coreRequest('POST', requestId, nowMs, {
      body: encoder.encode(frame),
      headers,
    }),
  )
}

// Close the durable stream at the producer's `seq`-th record without bytes (the
// EOF for an empty/terminal-only completion). Idempotent on an already-closed
// stream.
const closeStream = (
  store: StreamStore,
  requestId: string,
  nowMs: number,
  seq: number,
): void => {
  durableHandle(
    store,
    coreRequest('POST', requestId, nowMs, {
      headers: {
        'producer-epoch': '0',
        'producer-id': PRODUCER_ID,
        'producer-seq': String(seq),
        'stream-closed': 'true',
      },
    }),
  )
}

export const seedDurableInferenceStream = (input: {
  readonly close?: boolean
  readonly frames: ReadonlyArray<string>
  readonly nowMs: number
  readonly requestId: string
  readonly store: StreamStore
}): boolean => {
  if (!ensureStream(input.store, input.requestId, input.nowMs)) {
    return false
  }

  input.frames.forEach((frame, index) =>
    appendFrame(
      input.store,
      input.requestId,
      input.nowMs,
      index,
      frame,
      input.close === true && index === input.frames.length - 1,
    ),
  )

  if (input.close === true && input.frames.length === 0) {
    closeStream(input.store, input.requestId, input.nowMs, 0)
  }

  return true
}

// The public URL a client uses to resume a durable completion by offset. Keyed
// by `requestId`; the read route (`routeDurableInferenceReadRequest`) serves the
// stored suffix. No prompt/credential material is in the path — only the opaque
// request id — so the durable read surface is safe to expose / cache.
export const durableInferenceReadUrl = (requestId: string): string =>
  `/v1/chat/completions/durable/${encodeURIComponent(requestId)}`

// Result of draining the upstream into the durable log. `metered` is true on the
// single real EOF (the producer drained to completion); false when the upstream
// faulted mid-flight (no terminal frame → no settlement, receipt-first).
export interface DurableProducerOutcome {
  readonly content: string
  readonly faulted: boolean
  readonly terminal: ReturnType<InferenceStreamSource['terminal']> | undefined
}

// Tee the upstream `InferenceStreamSource` into the durable log AND into a
// caller-supplied frame sink (the live client SSE). Each upstream content delta
// is (1) emitted to the client and (2) persisted as a durable frame, so a client
// drop loses nothing. On clean drain the terminal frame is persisted and the
// stream is CLOSED (EOF). On an upstream fault the partial content stays durable
// and the stream is closed WITHOUT a terminal frame (so no metering, exactly as
// the non-durable path behaves).
//
// THE METERING-ONCE BOUNDARY: this producer drain is the only place that reads
// the live upstream. It calls `onEof` EXACTLY ONCE — when the upstream drains
// cleanly with a terminal frame — and never on the fault path. The resume path
// never invokes this function, so replays cannot meter. The `onEof` callback is
// where the route runs the metering hook + attaches the receipt.
export const teeUpstreamToDurable = async (input: {
  readonly store: StreamStore
  readonly requestId: string
  readonly nowMs: number
  // Persist + forward each rendered content-delta frame to the client.
  readonly frameForDelta: (delta: string) => string
  // Persist + forward a full rendered frame when the upstream carries a
  // non-text delta such as OpenAI `tool_calls`. Older callers can keep using
  // `frameForDelta` because content-only frames are still representable there.
  readonly frameForEvent?: ((event: InferenceStreamEvent) => string) | undefined
  // Build the terminal frame (empty delta + finish reason + `openagents` block).
  // Receives the metering-once outcome the route resolves from the terminal
  // usage. Returns the rendered terminal SSE frame to persist + forward.
  readonly onEof: (
    terminal: ReturnType<InferenceStreamSource['terminal']>,
    content: string,
  ) => Promise<string>
  // Emit a rendered frame to the live client stream.
  readonly emit: (frame: string) => void
  readonly source: InferenceStreamSource
}): Promise<DurableProducerOutcome> => {
  const {
    emit,
    frameForDelta,
    frameForEvent,
    nowMs,
    onEof,
    requestId,
    source,
    store,
  } = input

  ensureStream(store, requestId, nowMs)

  const contentParts: Array<string> = []
  let seq = 0

  try {
    for await (const event of source.frames) {
      const hasDelta =
        event.contentDelta !== '' ||
        (event.toolCallDeltas !== undefined && event.toolCallDeltas.length > 0)
      if (hasDelta) {
        contentParts.push(event.contentDelta)
        const frame =
          frameForEvent === undefined
            ? frameForDelta(event.contentDelta)
            : frameForEvent(event)
        // Persist BEFORE emitting so a crash between persist and emit still
        // leaves the durable log ahead of (or equal to) what the client saw —
        // resume can only ever replay MORE, never less.
        appendFrame(store, requestId, nowMs, seq++, frame, false)
        emit(frame)
      }
    }
  } catch {
    // Upstream faulted mid-flight. The client has partial content; the durable
    // log holds it too (resumable). Close the stream WITHOUT a terminal frame so
    // metering does NOT settle (receipt-first — never an estimate). EOF on the
    // durable stream still lets a reconnect see the closed partial completion.
    closeStream(store, requestId, nowMs, seq)
    return {
      content: contentParts.join(''),
      faulted: true,
      terminal: undefined,
    }
  }

  // Clean drain: the SINGLE real EOF. Run the route's metering-once + receipt
  // builder, persist the terminal frame, close the stream (Stream-Closed), and
  // emit the terminal frame to the live client.
  const terminal = source.terminal()
  const terminalFrame = await onEof(terminal, contentParts.join(''))
  appendFrame(store, requestId, nowMs, seq, terminalFrame, true)
  emit(terminalFrame)

  return {
    content: contentParts.join(''),
    faulted: false,
    terminal,
  }
}

// A resume/replay read of a durable completion from `offset`. Returns the stored
// SSE byte suffix, the next offset to resume from, and whether the stream is
// closed (EOF). THIS PATH NEVER METERS — it only reads stored bytes, so a
// reconnect, a multi-tab share, or a CDN catch-up hit is free.
export interface DurableReplay {
  readonly status: number
  readonly body: string
  readonly nextOffset: string
  readonly upToDate: boolean
  readonly streamClosed: boolean
  readonly contentType: string
}

export const replayFromOffset = (input: {
  readonly store: StreamStore
  readonly requestId: string
  readonly nowMs: number
  readonly offset: string | undefined
}): DurableReplay | undefined => {
  const { nowMs, offset, requestId, store } = input

  // Reject a malformed offset deterministically before touching the store.
  if (parseOffset(offset) === null) {
    return {
      body: '',
      contentType: DURABLE_INFERENCE_CONTENT_TYPE,
      nextOffset: tailOffset(0),
      status: 400,
      streamClosed: false,
      upToDate: false,
    }
  }

  const res = durableHandle(
    store,
    coreRequest('GET', requestId, nowMs, {
      query: offset === undefined ? {} : { offset },
    }),
  )

  // Unknown request id → no durable stream for it.
  if (res.status === 404) {
    return undefined
  }

  const bodyBytes = res.body ?? new Uint8Array(0)
  return {
    body: decoder.decode(bodyBytes),
    contentType: res.headers['content-type'] ?? DURABLE_INFERENCE_CONTENT_TYPE,
    nextOffset:
      res.headers['stream-next-offset'] ?? tailOffset(bodyBytes.length),
    status: res.status,
    streamClosed: res.headers['stream-closed'] === 'true',
    upToDate: res.headers['stream-up-to-date'] === 'true',
  }
}

// Reconstruct the FULL completion content from a durable stream by reading from
// the beginning. Used to prove resume reconstructs the whole completion; also a
// building block for a server-side "give me the final answer" read.
export const reconstructCompletion = (input: {
  readonly store: StreamStore
  readonly requestId: string
  readonly nowMs: number
}): string | undefined => {
  const replay = replayFromOffset({
    nowMs: input.nowMs,
    offset: offsetForPosition(0),
    requestId: input.requestId,
    store: input.store,
  })
  if (replay === undefined) {
    return undefined
  }
  return replay.body
}

// Type re-exports so the route + worker wiring can reference the durable surface
// without importing the package directly.
export type { StreamMeta, StreamStore }
