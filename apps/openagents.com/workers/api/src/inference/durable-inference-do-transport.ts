// DO-fetch transport for the durable inference stream (durable-stream Rank-1,
// #6058, EPIC #6056). This is the PRODUCTION wiring of the durable proxy: it
// speaks the EXACT `/v1/stream/{id}` HTTP contract the
// `@openagentsinc/durable-stream` Durable Object (`DurableInferenceStreamObject`)
// serves (`handleDurableStreamFetch`), so the per-request DO is the single
// authoritative durable log.
//
// WHY A SEPARATE ASYNC TRANSPORT (not the sync `StreamStore` proxy):
// `durable-inference-proxy.ts` drives a SYNCHRONOUS `StreamStore` — which models
// the inside of ONE stream's DO running its SQLite synchronously. A remote DO is
// reached over an ASYNC `stub.fetch()`, so the sync port cannot back it. This
// module rebuilds the SAME wire requests (PUT create, POST append with the
// `(producer-id, epoch, seq)` idempotency tuple + optional `stream-closed`, GET
// replay with `?offset=`) and sends them via `fetch()` to the DO stub. The DO
// then runs the package's sync core internally, so the offset/idempotency/EOF
// guarantees are byte-identical to the unit-tested in-memory path.
//
// ┌─────────────────── METERING EXACTLY-ONCE (the money path) ────────────────┐
// │ Metering lives ENTIRELY in the route's `onEof` callback (the producer      │
// │ drain), NOT in this transport and NOT in the DO. `teeUpstreamToDurableDO`  │
// │ calls `onEof` EXACTLY ONCE on a clean upstream drain and NEVER on the fault │
// │ path. `replayFromOffsetDO` reads stored bytes ONLY — it has no metering     │
// │ hook. Replays + catch-up reads are therefore free, exactly as the          │
// │ in-memory proxy guarantees.                                                 │
// └────────────────────────────────────────────────────────────────────────────┘
//
// FAIL-SAFE: a DO-fetch failure in the producer must NOT break the live client
// completion. Persist errors stop the durable mirror but the client keeps
// receiving its tokens and metering still settles on EOF — a broken durable
// substrate degrades to today's non-durable pass-through behaviour.
import {
  DURABLE_INFERENCE_CONTENT_TYPE,
  DURABLE_INFERENCE_TTL_SECONDS,
  type DurableProducerOutcome,
  type DurableReplay,
} from './durable-inference-proxy'
import {
  type InferenceStreamEvent,
  type InferenceStreamSource,
} from './provider-adapter'

// Structural typing for the DO namespace + stub surface, matching the package's
// own convention of typing the Cloudflare runtime locally rather than depending
// on `@cloudflare/workers-types` here. `getByName` is the repo's modern DO API
// (runtime.ts), returning a stub whose `.fetch()` is the DO's `fetch()`.
export interface DurableStreamStub {
  fetch(request: Request): Promise<Response>
}
export interface DurableStreamNamespace {
  getByName(name: string): DurableStreamStub
}

// The stable idempotent-producer id for a request's durable stream (must match
// the sync proxy's `PRODUCER_ID` so the wire format is identical).
const PRODUCER_ID = 'khala-gateway'

// The DO addresses streams at `/v1/stream/{streamId}` (durable-stream `http.ts`
// `STREAM_URL_PREFIX`). The host is arbitrary — the stub routes by name, not by
// host — so we use an opaque internal origin.
const streamUrl = (streamId: string): string =>
  `https://durable-inference-stream/v1/stream/${encodeURIComponent(streamId)}`

const encoder = new TextEncoder()

// PUT-create the per-request durable stream (idempotent: re-create with the same
// config returns 200; first create 201). Returns false when the DO rejects
// creation (treated as "durable unavailable" → caller fails safe).
const ensureStreamDO = async (
  stub: DurableStreamStub,
  streamId: string,
): Promise<boolean> => {
  const res = await stub.fetch(
    new Request(streamUrl(streamId), {
      method: 'PUT',
      headers: {
        'content-type': DURABLE_INFERENCE_CONTENT_TYPE,
        'stream-ttl': String(DURABLE_INFERENCE_TTL_SECONDS),
      },
    }),
  )
  return res.status === 200 || res.status === 201
}

// POST one persisted frame as the producer's `seq`-th record (exactly-once via
// the `(producer-id, epoch, seq)` tuple the DO dedups on). When `close` is set
// the append also closes the stream (the completion EOF).
const appendFrameDO = async (
  stub: DurableStreamStub,
  streamId: string,
  seq: number,
  frame: string,
  close: boolean,
): Promise<void> => {
  const headers: Record<string, string> = {
    'content-type': DURABLE_INFERENCE_CONTENT_TYPE,
    'producer-epoch': '0',
    'producer-id': PRODUCER_ID,
    'producer-seq': String(seq),
  }
  if (close) {
    headers['stream-closed'] = 'true'
  }
  await stub.fetch(
    new Request(streamUrl(streamId), {
      method: 'POST',
      headers,
      body: encoder.encode(frame),
    }),
  )
}

// Close the durable stream at the producer's `seq`-th record without bytes (the
// EOF for an empty/terminal-only or faulted completion).
const closeStreamDO = async (
  stub: DurableStreamStub,
  streamId: string,
  seq: number,
): Promise<void> => {
  await stub.fetch(
    new Request(streamUrl(streamId), {
      method: 'POST',
      headers: {
        'producer-epoch': '0',
        'producer-id': PRODUCER_ID,
        'producer-seq': String(seq),
        'stream-closed': 'true',
      },
    }),
  )
}

export const seedDurableInferenceStreamDO = async (input: {
  readonly close?: boolean
  readonly frames: ReadonlyArray<string>
  readonly namespace: DurableStreamNamespace
  readonly requestId: string
}): Promise<boolean> => {
  const stub = input.namespace.getByName(input.requestId)
  const created = await ensureStreamDO(stub, input.requestId).catch(() => false)
  if (!created) {
    return false
  }

  for (let index = 0; index < input.frames.length; index += 1) {
    await appendFrameDO(
      stub,
      input.requestId,
      index,
      input.frames[index]!,
      input.close === true && index === input.frames.length - 1,
    )
  }

  if (input.close === true && input.frames.length === 0) {
    await closeStreamDO(stub, input.requestId, 0)
  }

  return true
}

// Tee the upstream `InferenceStreamSource` into the DO durable log AND to the
// live client. The DO-fetch analogue of `teeUpstreamToDurable`: each upstream
// content delta is (1) persisted to the DO as a durable frame and (2) emitted to
// the client. On clean drain the terminal frame is persisted and the stream is
// CLOSED (EOF) and metering fires ONCE via `onEof`. On an upstream fault the
// partial content stays durable and the stream is closed WITHOUT a terminal
// frame (no metering — receipt-first).
//
// PERSIST-BEFORE-EMIT: each frame is awaited into the DO before it is emitted to
// the client, so the durable log is always ahead of (or equal to) what the
// client saw — a resume can only ever replay MORE, never less.
//
// FAIL-SAFE: if a DO persist throws (transport fault), we STOP persisting but
// keep draining + emitting to the client so the completion still lands and
// metering still settles. The durable mirror is best-effort; the live stream is
// authoritative for the client.
export const teeUpstreamToDurableDO = async (input: {
  readonly namespace: DurableStreamNamespace
  readonly requestId: string
  readonly frameForDelta: (delta: string) => string
  readonly frameForEvent?: ((event: InferenceStreamEvent) => string) | undefined
  readonly onEof: (
    terminal: ReturnType<InferenceStreamSource['terminal']>,
    content: string,
  ) => Promise<string>
  readonly emit: (frame: string) => void
  readonly source: InferenceStreamSource
}): Promise<DurableProducerOutcome> => {
  const {
    emit,
    frameForDelta,
    frameForEvent,
    namespace,
    onEof,
    requestId,
    source,
  } = input

  const stub = namespace.getByName(requestId)
  let durable = await ensureStreamDO(stub, requestId).catch(() => false)

  const contentParts: Array<string> = []
  let seq = 0

  // Persist a frame, degrading `durable` to false (and swallowing the error) on
  // any DO transport fault so the live client stream is never broken.
  const persist = async (frame: string, close: boolean): Promise<void> => {
    if (!durable) {
      return
    }
    try {
      await appendFrameDO(stub, requestId, seq, frame, close)
    } catch {
      durable = false
    }
  }

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
        await persist(frame, false)
        if (durable) {
          seq += 1
        }
        emit(frame)
      }
    }
  } catch {
    // Upstream faulted mid-flight. Close the durable stream WITHOUT a terminal
    // frame so metering does NOT settle (receipt-first). Best-effort: a close
    // failure is swallowed.
    if (durable) {
      await closeStreamDO(stub, requestId, seq).catch(() => {})
    }
    return {
      content: contentParts.join(''),
      faulted: true,
      terminal: undefined,
    }
  }

  // Clean drain: the SINGLE real EOF. Run the route's metering-once + receipt
  // builder, persist the terminal frame, close the stream (Stream-Closed), and
  // emit the terminal frame to the live client. Metering happens inside `onEof`
  // regardless of whether the durable mirror is still healthy.
  const terminal = source.terminal()
  const terminalFrame = await onEof(terminal, contentParts.join(''))
  await persist(terminalFrame, true)
  emit(terminalFrame)

  return {
    content: contentParts.join(''),
    faulted: false,
    terminal,
  }
}

// Resume/replay read of a durable completion from `offset`, via the DO. The
// DO-fetch analogue of `replayFromOffset`: returns the stored SSE byte suffix,
// the next offset to resume from, and whether the stream is closed (EOF). THIS
// PATH NEVER METERS — it issues a GET to the DO and reads stored bytes only.
// Returns `undefined` when the DO reports the stream does not exist (404).
export const replayFromOffsetDO = async (input: {
  readonly namespace: DurableStreamNamespace
  readonly requestId: string
  readonly offset: string | undefined
}): Promise<DurableReplay | undefined> => {
  const { namespace, offset, requestId } = input

  const stub = namespace.getByName(requestId)
  const url = new URL(streamUrl(requestId))
  if (offset !== undefined) {
    url.searchParams.set('offset', offset)
  }

  const res = await stub.fetch(new Request(url.toString(), { method: 'GET' }))

  // Unknown request id → no durable stream for it.
  if (res.status === 404) {
    return undefined
  }

  const bodyText = await res.text()
  const headerOffset = res.headers.get('stream-next-offset')
  return {
    body: bodyText,
    contentType:
      res.headers.get('content-type') ?? DURABLE_INFERENCE_CONTENT_TYPE,
    nextOffset: headerOffset ?? offset ?? '',
    status: res.status,
    streamClosed: res.headers.get('stream-closed') === 'true',
    upToDate: res.headers.get('stream-up-to-date') === 'true',
  }
}
