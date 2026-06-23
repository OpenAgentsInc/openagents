/**
 * Transport-agnostic Durable Streams core.
 *
 * Maps a normalized request to a normalized response over a single-stream
 * `StreamStore`, implementing the four-part conformance contract:
 *   1. offset-addressed replay (exact suffix, monotonic opaque offsets)
 *   2. resumability + EOF (Stream-Next-Offset / Stream-Closed)
 *   3. exactly-once writes ((producerId,epoch,seq) dedup/fence/gap)
 *   4. CDN-friendly fan-out (ETag/Cache-Control/Stream-Cursor)
 *
 * No Cloudflare or HTTP-framework import here — `core.ts` is pure and
 * Bun-unit-testable. The HTTP and SSE adapters (http.ts) translate Request /
 * Response to/from these normalized shapes.
 */
import * as P from "./protocol.ts"
import {
  isSentinel,
  OFFSET_BEGINNING,
  offsetForPosition,
  OFFSET_NOW,
  parseOffset,
  tailOffset,
} from "./offset.ts"
import type { ProducerState, StreamMeta, StreamStore } from "./store.ts"

export type Method = "GET" | "HEAD" | "PUT" | "POST" | "DELETE"

export interface CoreRequest {
  readonly method: Method
  readonly streamId: string
  /** Lowercased header map (single value per key). */
  readonly headers: Record<string, string | undefined>
  readonly query: Record<string, string | undefined>
  readonly body: Uint8Array
  readonly nowMs: number
}

export interface CoreResponse {
  readonly status: number
  readonly headers: Record<string, string>
  readonly body: Uint8Array | null
  /**
   * Set for SSE responses; the http adapter streams it. When present, `body`
   * is ignored and `status` is 200.
   */
  readonly sse?: SseDirective
}

export interface SseDirective {
  readonly contentType: string
  readonly base64: boolean
  readonly startPosition: number
}

const EMPTY = new Uint8Array(0)
const MAX_CHUNK_BYTES = 1 << 20 // 1 MiB server-defined chunk cap

const enc = new TextEncoder()

const h = (
  pairs: Record<string, string | number | boolean | undefined>,
): Record<string, string> => {
  const out: Record<string, string> = {}
  for (const [k, v] of Object.entries(pairs)) {
    if (v !== undefined) out[k] = String(v)
  }
  return out
}

const status = (code: number, headers: Record<string, string> = {}): CoreResponse => ({
  status: code,
  headers,
  body: null,
})

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

export const handle = (store: StreamStore, req: CoreRequest): CoreResponse => {
  switch (req.method) {
    case "PUT":
      return handlePut(store, req)
    case "POST":
      return handlePost(store, req)
    case "GET":
      return handleGet(store, req)
    case "HEAD":
      return handleHead(store, req)
    case "DELETE":
      return handleDelete(store, req)
    default:
      return status(405, h({ allow: "GET, HEAD, PUT, POST, DELETE" }))
  }
}

// ---------------------------------------------------------------------------
// PUT — create (§5.1)
// ---------------------------------------------------------------------------

const handlePut = (store: StreamStore, req: CoreRequest): CoreResponse => {
  const ttlRaw = req.headers[P.HDR_STREAM_TTL]
  const expiresRaw = req.headers[P.HDR_STREAM_EXPIRES_AT]
  if (ttlRaw !== undefined && expiresRaw !== undefined) {
    return status(400) // §5.1: cannot supply both
  }
  let ttlSeconds: number | null = null
  if (ttlRaw !== undefined) {
    const parsed = P.parseTtlSeconds(ttlRaw)
    if (parsed === null) return status(400)
    ttlSeconds = parsed
  }
  let expiresAtMs: number | null = null
  if (expiresRaw !== undefined) {
    const ms = Date.parse(expiresRaw)
    if (Number.isNaN(ms)) return status(400)
    expiresAtMs = ms
  }

  const contentType = req.headers[P.HDR_CONTENT_TYPE] ?? P.DEFAULT_CONTENT_TYPE
  const wantClosed = P.isStreamClosedTrue(req.headers[P.HDR_STREAM_CLOSED])

  const existing = store.getMeta()
  if (existing !== null) {
    // Idempotent create: 200 if config matches (incl. closure status), else 409.
    const configMatches =
      existing.contentType === contentType &&
      existing.ttlSeconds === ttlSeconds &&
      existing.expiresAtMs === expiresAtMs &&
      existing.closed === wantClosed
    if (!configMatches) return status(409)
    return status(200, h({
      [P.HDR_CONTENT_TYPE]: existing.contentType,
      [P.HDR_STREAM_NEXT_OFFSET]: tailOffset(store.byteLength()),
      [P.HDR_STREAM_CLOSED]: existing.closed ? P.STREAM_CLOSED_TRUE : undefined,
    }))
  }

  const meta: StreamMeta = {
    contentType,
    ttlSeconds,
    expiresAtMs,
    closed: wantClosed,
    createdAtMs: req.nowMs,
    streamId: req.streamId,
    lastStreamSeq: null,
  }
  store.create(meta)
  if (req.body.length > 0) {
    store.appendAtomic(req.body, null, null)
  }
  const tail = tailOffset(store.byteLength())
  return {
    status: 201,
    headers: h({
      [P.HDR_LOCATION]: req.streamId,
      [P.HDR_CONTENT_TYPE]: contentType,
      [P.HDR_STREAM_NEXT_OFFSET]: tail,
      [P.HDR_STREAM_CLOSED]: wantClosed ? P.STREAM_CLOSED_TRUE : undefined,
    }),
    body: null,
  }
}

// ---------------------------------------------------------------------------
// POST — append / close (§5.2, §5.2.1, §5.3)
// ---------------------------------------------------------------------------

const handlePost = (store: StreamStore, req: CoreRequest): CoreResponse => {
  const meta = store.getMeta()
  if (meta === null) return status(404)

  const wantClose = P.isStreamClosedTrue(req.headers[P.HDR_STREAM_CLOSED])
  const hasBody = req.body.length > 0

  // Empty body without close → 400 (§5.2 request body rules)
  if (!hasBody && !wantClose) return status(400)

  // Producer headers (§5.2.1) — all three or none.
  const producer = parseProducerHeaders(req.headers)
  if (producer === "invalid") return status(400)

  // ---- Closed-stream handling (error precedence: closed first) ----
  if (meta.closed) {
    if (!hasBody && wantClose) {
      // idempotent close-only on already-closed stream
      return status(204, h({
        [P.HDR_STREAM_NEXT_OFFSET]: tailOffset(store.byteLength()),
        [P.HDR_STREAM_CLOSED]: P.STREAM_CLOSED_TRUE,
      }))
    }
    // append (with or without close) to a closed stream
    if (producer !== null) {
      const st = store.getProducer(producer.id)
      if (
        st?.closingTuple &&
        st.closingTuple.epoch === producer.epoch &&
        st.closingTuple.seq === producer.seq
      ) {
        // duplicate of the closing append → idempotent success
        return status(204, h({
          [P.HDR_STREAM_NEXT_OFFSET]: tailOffset(store.byteLength()),
          [P.HDR_STREAM_CLOSED]: P.STREAM_CLOSED_TRUE,
          [P.HDR_PRODUCER_EPOCH]: producer.epoch,
          [P.HDR_PRODUCER_SEQ]: producer.seq,
        }))
      }
    }
    return status(409, h({
      [P.HDR_STREAM_CLOSED]: P.STREAM_CLOSED_TRUE,
      [P.HDR_STREAM_NEXT_OFFSET]: tailOffset(store.byteLength()),
    }))
  }

  // ---- Content-Type match (only enforced when a body is present) ----
  if (hasBody) {
    const ct = req.headers[P.HDR_CONTENT_TYPE]
    if (ct !== undefined && ct.split(";")[0]!.trim() !== meta.contentType.split(";")[0]!.trim()) {
      return status(409) // content type mismatch
    }
  }

  // ---- Idempotent producer validation (§5.2.1) ----
  let nextProducerState: ProducerState | null = null
  if (producer !== null) {
    const prior = store.getProducer(producer.id)
    const decision = validateProducer(prior, producer)
    switch (decision.kind) {
      case "stale-epoch":
        return status(403, h({ [P.HDR_PRODUCER_EPOCH]: decision.currentEpoch }))
      case "bad-new-epoch":
        return status(400)
      case "gap":
        return status(409, h({
          [P.HDR_PRODUCER_EXPECTED_SEQ]: decision.expected,
          [P.HDR_PRODUCER_RECEIVED_SEQ]: decision.received,
        }))
      case "duplicate":
        // already applied — idempotent success, do NOT re-append.
        return status(204, h({
          [P.HDR_STREAM_NEXT_OFFSET]: tailOffset(store.byteLength()),
          [P.HDR_PRODUCER_EPOCH]: producer.epoch,
          [P.HDR_PRODUCER_SEQ]: store.getProducer(producer.id)?.lastSeq ?? producer.seq,
          [P.HDR_STREAM_CLOSED]: meta.closed ? P.STREAM_CLOSED_TRUE : undefined,
        }))
      case "accept":
        nextProducerState = {
          epoch: producer.epoch,
          lastSeq: producer.seq,
          closingTuple: wantClose ? { epoch: producer.epoch, seq: producer.seq } : (prior?.closingTuple ?? null),
        }
        break
    }
  }

  // ---- Stream-Seq monotonicity (§5.2) ----
  const streamSeq = req.headers[P.HDR_STREAM_SEQ]
  if (streamSeq !== undefined) {
    if (meta.lastStreamSeq !== null && streamSeq <= meta.lastStreamSeq) {
      return status(409) // sequence regression
    }
  }

  // ---- Atomic append (+ producer state) ----
  if (hasBody) {
    store.appendAtomic(
      req.body,
      streamSeq ?? null,
      nextProducerState !== null ? { id: producer!.id, state: nextProducerState } : null,
    )
  } else if (nextProducerState !== null) {
    // close-only with producer headers: persist producer state without bytes
    store.appendAtomic(EMPTY, streamSeq ?? null, { id: producer!.id, state: nextProducerState })
  }

  // ---- Close transition ----
  if (wantClose) {
    store.putMeta({ ...store.getMeta()!, closed: true })
  }

  const tail = tailOffset(store.byteLength())
  const closed = store.getMeta()!.closed
  const okStatus = producer !== null ? 200 : 204
  return status(okStatus, h({
    [P.HDR_STREAM_NEXT_OFFSET]: tail,
    [P.HDR_STREAM_CLOSED]: closed ? P.STREAM_CLOSED_TRUE : undefined,
    [P.HDR_PRODUCER_EPOCH]: producer?.epoch,
    [P.HDR_PRODUCER_SEQ]: producer?.seq,
  }))
}

// ---------------------------------------------------------------------------
// GET — read (catch-up / long-poll / SSE) (§5.6–§5.8)
// ---------------------------------------------------------------------------

const handleGet = (store: StreamStore, req: CoreRequest): CoreResponse => {
  const meta = store.getMeta()
  if (meta === null) return status(404)

  const offsetRaw = req.query[P.QP_OFFSET]
  const parsed = parseOffset(offsetRaw)
  if (parsed === null) return status(400)

  const tailLen = store.byteLength()
  const live = req.query[P.QP_LIVE]

  // Resolve the start position.
  let startPos: number
  let isNow = false
  if (parsed.kind === "beginning") startPos = 0
  else if (parsed.kind === "now") {
    startPos = tailLen
    isNow = true
  } else startPos = parsed.position

  // SSE mode (§5.8)
  if (live === P.LIVE_SSE) {
    const base64 = !P.isTextLikeContentType(meta.contentType)
    return {
      status: 200,
      headers: h({
        [P.HDR_CONTENT_TYPE]: "text/event-stream",
        [P.HDR_CACHE_CONTROL]: P.NO_STORE,
        ...(base64 ? { [P.HDR_SSE_DATA_ENCODING]: "base64" } : {}),
      }),
      body: null,
      sse: { contentType: meta.contentType, base64, startPosition: startPos },
    }
  }

  // Long-poll mode (§5.7): we are a synchronous core. If data is available now
  // return it; otherwise return the "timeout" immediately (204) — the live wait
  // is the adapter's responsibility, and for a closed-at-tail stream the spec
  // requires an immediate 204 anyway.
  if (live === P.LIVE_LONG_POLL) {
    const hasData = startPos < tailLen
    if (hasData) {
      return readBody(meta, store, startPos, tailLen, req.nowMs, true)
    }
    // No data: 204 timeout (immediate for closed-at-tail; §5.7).
    return status(204, h({
      [P.HDR_STREAM_NEXT_OFFSET]: offsetForPosition(tailLen),
      [P.HDR_STREAM_UP_TO_DATE]: "true",
      [P.HDR_STREAM_CURSOR]: meta.closed ? undefined : P.makeCursor(req.nowMs, req.query[P.QP_CURSOR] ?? null),
      [P.HDR_STREAM_CLOSED]: meta.closed ? P.STREAM_CLOSED_TRUE : undefined,
    }))
  }

  // Catch-up mode (§5.6).
  if (isNow) {
    // §8: offset=now in catch-up → empty body, up-to-date, no-store, no ETag.
    return {
      status: 200,
      headers: h({
        [P.HDR_CONTENT_TYPE]: meta.contentType,
        [P.HDR_CACHE_CONTROL]: P.NO_STORE,
        [P.HDR_STREAM_NEXT_OFFSET]: offsetForPosition(tailLen),
        [P.HDR_STREAM_UP_TO_DATE]: "true",
        [P.HDR_STREAM_CLOSED]: meta.closed ? P.STREAM_CLOSED_TRUE : undefined,
      }),
      body: EMPTY,
    }
  }
  return readBody(meta, store, startPos, tailLen, req.nowMs, false)
}

/** Build a catch-up / long-poll data response from `startPos`. */
const readBody = (
  meta: StreamMeta,
  store: StreamStore,
  startPos: number,
  tailLen: number,
  nowMs: number,
  longPoll: boolean,
): CoreResponse => {
  const clampedStart = Math.min(startPos, tailLen)
  const chunk = store.readFrom(clampedStart, MAX_CHUNK_BYTES)
  const endPos = clampedStart + chunk.length
  const upToDate = endPos >= tailLen
  // §5.6: Stream-Closed present only when closed AND we've reached the final offset.
  const closedAtTail = meta.closed && upToDate
  const etag = P.makeETag(meta.streamId, offsetForPosition(clampedStart), offsetForPosition(endPos), closedAtTail)

  const headers = h({
    [P.HDR_CONTENT_TYPE]: meta.contentType,
    [P.HDR_CACHE_CONTROL]: P.CATCHUP_CACHE_CONTROL,
    [P.HDR_ETAG]: etag,
    [P.HDR_STREAM_NEXT_OFFSET]: offsetForPosition(endPos),
    [P.HDR_STREAM_UP_TO_DATE]: upToDate ? "true" : undefined,
    [P.HDR_STREAM_CLOSED]: closedAtTail ? P.STREAM_CLOSED_TRUE : undefined,
    [P.HDR_STREAM_CURSOR]: longPoll && !closedAtTail ? P.makeCursor(nowMs, null) : undefined,
  })
  return { status: 200, headers, body: chunk }
}

// ---------------------------------------------------------------------------
// HEAD — metadata (§5.5)
// ---------------------------------------------------------------------------

const handleHead = (store: StreamStore, req: CoreRequest): CoreResponse => {
  const meta = store.getMeta()
  if (meta === null) return status(404)
  void req
  return status(200, h({
    [P.HDR_CONTENT_TYPE]: meta.contentType,
    [P.HDR_STREAM_NEXT_OFFSET]: tailOffset(store.byteLength()),
    [P.HDR_STREAM_TTL]: meta.ttlSeconds !== null ? meta.ttlSeconds : undefined,
    [P.HDR_STREAM_EXPIRES_AT]: meta.expiresAtMs !== null ? new Date(meta.expiresAtMs).toISOString() : undefined,
    [P.HDR_STREAM_CLOSED]: meta.closed ? P.STREAM_CLOSED_TRUE : undefined,
    [P.HDR_CACHE_CONTROL]: P.NO_STORE,
  }))
}

// ---------------------------------------------------------------------------
// DELETE — (§5.4)
// ---------------------------------------------------------------------------

const handleDelete = (store: StreamStore, req: CoreRequest): CoreResponse => {
  void req
  if (store.getMeta() === null) return status(404)
  store.destroy()
  return status(204)
}

// ---------------------------------------------------------------------------
// Producer header parsing + validation (§5.2.1)
// ---------------------------------------------------------------------------

interface ProducerHeaders {
  readonly id: string
  readonly epoch: number
  readonly seq: number
}

const parseProducerHeaders = (
  headers: Record<string, string | undefined>,
): ProducerHeaders | null | "invalid" => {
  const id = headers[P.HDR_PRODUCER_ID]
  const epochRaw = headers[P.HDR_PRODUCER_EPOCH]
  const seqRaw = headers[P.HDR_PRODUCER_SEQ]
  const present = [id, epochRaw, seqRaw].filter((x) => x !== undefined).length
  if (present === 0) return null
  if (present !== 3) return "invalid" // all-or-none (§5.2.1)
  if (id === "" ) return "invalid"
  const epoch = toNonNegInt(epochRaw!)
  const seq = toNonNegInt(seqRaw!)
  if (epoch === null || seq === null) return "invalid"
  return { id: id!, epoch, seq }
}

const toNonNegInt = (raw: string): number | null => {
  if (!/^\d+$/.test(raw)) return null
  const n = Number(raw)
  if (!Number.isInteger(n) || n < 0 || n > Number.MAX_SAFE_INTEGER) return null
  return n
}

type ProducerDecision =
  | { kind: "accept" }
  | { kind: "duplicate" }
  | { kind: "stale-epoch"; currentEpoch: number }
  | { kind: "bad-new-epoch" }
  | { kind: "gap"; expected: number; received: number }

/** §5.2.1 validation logic. */
const validateProducer = (
  prior: ProducerState | null,
  req: ProducerHeaders,
): ProducerDecision => {
  if (prior === null) {
    // brand-new producer: must start at seq 0 of its declared epoch
    if (req.seq !== 0) return { kind: "gap", expected: 0, received: req.seq }
    return { kind: "accept" }
  }
  if (req.epoch < prior.epoch) {
    return { kind: "stale-epoch", currentEpoch: prior.epoch }
  }
  if (req.epoch > prior.epoch) {
    if (req.seq !== 0) return { kind: "bad-new-epoch" }
    return { kind: "accept" } // new epoch established
  }
  // same epoch
  if (req.seq <= prior.lastSeq) return { kind: "duplicate" }
  if (req.seq === prior.lastSeq + 1) return { kind: "accept" }
  return { kind: "gap", expected: prior.lastSeq + 1, received: req.seq }
}

// re-export for adapters/tests
export { isSentinel, OFFSET_BEGINNING, OFFSET_NOW }
export { enc }
