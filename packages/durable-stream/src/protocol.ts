/**
 * Durable Streams protocol header names, content types, and the conformance
 * contract constants. Ported from PROTOCOL.md (ElectricSQL, read-only ref) into
 * owned code — names match the wire protocol so the conformance suite (and any
 * upstream client) can drive this server.
 */

export const DURABLE_STREAM_SCHEMA_VERSION = "openagents.durable_stream.v1"

// ---------------------------------------------------------------------------
// Header names (wire protocol — case-insensitive on read; canonical on write)
// ---------------------------------------------------------------------------

export const HDR_CONTENT_TYPE = "content-type"
export const HDR_STREAM_TTL = "stream-ttl"
export const HDR_STREAM_EXPIRES_AT = "stream-expires-at"
export const HDR_STREAM_CLOSED = "stream-closed"
export const HDR_STREAM_SEQ = "stream-seq"
export const HDR_STREAM_NEXT_OFFSET = "stream-next-offset"
export const HDR_STREAM_UP_TO_DATE = "stream-up-to-date"
export const HDR_STREAM_CURSOR = "stream-cursor"
export const HDR_ETAG = "etag"
export const HDR_IF_NONE_MATCH = "if-none-match"
export const HDR_CACHE_CONTROL = "cache-control"
export const HDR_LOCATION = "location"
export const HDR_SSE_DATA_ENCODING = "stream-sse-data-encoding"

// Idempotent producer headers (PROTOCOL.md §5.2.1)
export const HDR_PRODUCER_ID = "producer-id"
export const HDR_PRODUCER_EPOCH = "producer-epoch"
export const HDR_PRODUCER_SEQ = "producer-seq"
export const HDR_PRODUCER_EXPECTED_SEQ = "producer-expected-seq"
export const HDR_PRODUCER_RECEIVED_SEQ = "producer-received-seq"

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

export const QP_OFFSET = "offset"
export const QP_LIVE = "live"
export const QP_CURSOR = "cursor"

export const LIVE_LONG_POLL = "long-poll"
export const LIVE_SSE = "sse"

// ---------------------------------------------------------------------------
// Defaults / values
// ---------------------------------------------------------------------------

export const DEFAULT_CONTENT_TYPE = "application/octet-stream"
export const STREAM_CLOSED_TRUE = "true"

/**
 * Catch-up reads are cacheable; we use `private` because durable streams may
 * carry user-specific / confidential data (e.g. inference output). PROTOCOL.md
 * §10.1.
 */
export const CATCHUP_CACHE_CONTROL = "private, max-age=60, stale-while-revalidate=300"
export const NO_STORE = "no-store"

/**
 * Content types whose SSE `data` events carry UTF-8 text directly; everything
 * else is base64-encoded with `stream-sse-data-encoding: base64`. §5.8.
 */
export const isTextLikeContentType = (ct: string): boolean => {
  const base = ct.split(";")[0]!.trim().toLowerCase()
  return base.startsWith("text/") || base === "application/json"
}

/** §4.1: header is "present" only when its value is exactly `true` (ci). */
export const isStreamClosedTrue = (raw: string | null | undefined): boolean =>
  raw !== null && raw !== undefined && raw.trim().toLowerCase() === STREAM_CLOSED_TRUE

/**
 * Validate a `Stream-TTL` value per §5.1: non-negative integer, decimal, no
 * leading zeros / plus / decimal / scientific. Returns the parsed seconds or
 * `null` if malformed.
 */
export const parseTtlSeconds = (raw: string): number | null => {
  if (!/^(0|[1-9]\d*)$/.test(raw)) return null
  const n = Number(raw)
  return Number.isSafeInteger(n) ? n : null
}

/** ETag format: `{streamId}:{startOffset}:{endOffset}` (+ `:c` when closed). §10.1 */
export const makeETag = (
  streamId: string,
  startOffset: string,
  endOffset: string,
  closed: boolean,
): string => `"${streamId}:${startOffset}:${endOffset}${closed ? ":c" : ""}"`

/**
 * Cursor for live modes (§10.1): interval number since 2024-10-09T00:00:00Z in
 * 20s intervals, as a decimal string. Must strictly advance when the client
 * echoes a cursor >= the current interval (jitter), to prevent CDN cache loops.
 */
const CURSOR_EPOCH_MS = Date.UTC(2024, 9, 9, 0, 0, 0)
const CURSOR_INTERVAL_MS = 20_000

export const makeCursor = (nowMs: number, clientCursor: string | null): string => {
  const interval = Math.floor((nowMs - CURSOR_EPOCH_MS) / CURSOR_INTERVAL_MS)
  const client = clientCursor !== null && /^\d+$/.test(clientCursor) ? Number(clientCursor) : -1
  if (client >= interval) {
    // jitter 1..3600 to guarantee strict monotonic progression
    const jitter = 1 + Math.floor((nowMs % 3600))
    return String(client + jitter)
  }
  return String(interval)
}
