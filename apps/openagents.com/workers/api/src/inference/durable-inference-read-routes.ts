// Durable read / resume route for streaming Khala completions (durable-stream
// Rank-1, #6058). Serves the persisted token stream of an in-flight or completed
// completion so a client that dropped mid-generation can reconnect and replay
// the suffix from its last offset, reconstructing the full completion.
//
//   GET /v1/chat/completions/durable/{requestId}?offset=<last-offset>
//
// THIS ROUTE NEVER METERS. It reads stored bytes only (`replayFromOffset`), so a
// reconnect, a multi-tab share, or a CDN catch-up hit is FREE. Settlement fired
// exactly once on the original upstream EOF (the producer drain in
// `chat-completions-routes.ts`); it can never fire here.
//
// The opaque request id carries no prompt/credential material, and the persisted
// frames are the same `chat.completion.chunk` SSE the client already received, so
// the durable read surface is safe to expose. `Stream-Closed` is the clean EOF.
//
// FAIL-SAFE: this route is dispatched only when the inference gateway flag is on
// AND a durable store factory is wired. Off / unwired => it returns `undefined`
// and the worker router falls through (404), with no behaviour change.

import {
  type DurableInferenceStreamStore,
  type DurableReplay,
  replayFromOffset,
} from './durable-inference-proxy'
import {
  type DurableStreamNamespace,
  replayFromOffsetDO,
} from './durable-inference-do-transport'

const DURABLE_PREFIX = '/v1/chat/completions/durable/'

export interface DurableInferenceReadDeps {
  // Whether the inference gateway is enabled (the durable read shares the gateway
  // flag — it is inert/404 when the gateway is off).
  readonly enabled: boolean
  // Per-request durable store factory (the SAME the chat route uses to persist).
  readonly durableStream: DurableInferenceStreamStore | undefined
  // Deterministic clock (epoch ms) for the durable log's TTL/offset bookkeeping.
  readonly nowEpochMillis: () => number
}

// Extract the request id from a durable read path, or undefined if not a durable
// read URL.
const requestIdFromPath = (pathname: string): string | undefined => {
  if (!pathname.startsWith(DURABLE_PREFIX)) {
    return undefined
  }
  const raw = pathname.slice(DURABLE_PREFIX.length)
  if (raw.length === 0 || raw.includes('/')) {
    return undefined
  }
  return decodeURIComponent(raw)
}

const jsonError = (error: string, status: number): Response =>
  new Response(JSON.stringify({ error }), {
    headers: { 'cache-control': 'no-store', 'content-type': 'application/json' },
    status,
  })

// Map a `DurableReplay` (from EITHER the sync store or the DO transport) to the
// resume Response. `undefined` (unknown request id) → 404; status 400 (malformed
// offset) → 400; otherwise the stored SSE suffix with the resume headers.
const replayToResponse = (replay: DurableReplay | undefined): Response => {
  if (replay === undefined) {
    return jsonError('not_found', 404)
  }
  if (replay.status === 400) {
    return jsonError('invalid_offset', 400)
  }
  // `Stream-Next-Offset` lets the client resume from exactly where it left off;
  // `Stream-Closed` signals the completion EOF.
  const headers: Record<string, string> = {
    'cache-control': 'no-store',
    'content-type': replay.contentType,
    'stream-next-offset': replay.nextOffset,
  }
  if (replay.upToDate) {
    headers['stream-up-to-date'] = 'true'
  }
  if (replay.streamClosed) {
    headers['stream-closed'] = 'true'
  }
  return new Response(replay.body, { headers, status: 200 })
}

// Parse a durable read request into `{ requestId, offset }`, or `undefined` when
// it is not a durable read URL. Shared by the sync and DO dispatchers.
export interface DurableReadMatch {
  readonly requestId: string
  readonly offset: string | undefined
}

export const matchDurableReadRequest = (
  request: Request,
): DurableReadMatch | undefined => {
  const url = new URL(request.url)
  const requestId = requestIdFromPath(url.pathname)
  if (requestId === undefined) {
    return undefined
  }
  return { offset: url.searchParams.get('offset') ?? undefined, requestId }
}

// Dispatch a durable inference read. Returns `undefined` when the request is not
// a durable read URL (the worker router falls through). When it IS a durable read
// URL but the gateway is off / unwired / the id is unknown, returns the matching
// 404/400/200 Response so the surface is honest.
export const routeDurableInferenceReadRequest = (
  request: Request,
  deps: DurableInferenceReadDeps,
): Response | undefined => {
  const matched = matchDurableReadRequest(request)
  if (matched === undefined) {
    return undefined
  }

  // Shares the gateway flag: inert/404 when the gateway is off.
  if (!deps.enabled || deps.durableStream === undefined) {
    return jsonError('not_found', 404)
  }

  if (request.method !== 'GET') {
    return jsonError('method_not_allowed', 405)
  }

  const store = deps.durableStream(matched.requestId)
  if (store === undefined) {
    return jsonError('not_found', 404)
  }

  const replay = replayFromOffset({
    nowMs: deps.nowEpochMillis(),
    offset: matched.offset,
    requestId: matched.requestId,
    store,
  })

  return replayToResponse(replay)
}

// PRODUCTION DURABLE READ DISPATCHER (#6058). The async DO-backed resume: reads
// the per-request Durable Object (`getByName(requestId)`) over the
// `/v1/stream/{id}` HTTP contract. Returns `undefined` when the request is not a
// durable read URL (router falls through). NEVER METERS — it reads stored bytes
// only, so a reconnect / multi-tab share / catch-up read is free. The chat
// route's producer drain already settled metering exactly once on the upstream
// EOF; it can never fire here.
export const routeDurableInferenceReadRequestDO = async (
  request: Request,
  deps: Readonly<{
    enabled: boolean
    namespace: DurableStreamNamespace | undefined
  }>,
): Promise<Response | undefined> => {
  const matched = matchDurableReadRequest(request)
  if (matched === undefined) {
    return undefined
  }

  if (!deps.enabled || deps.namespace === undefined) {
    return jsonError('not_found', 404)
  }

  if (request.method !== 'GET') {
    return jsonError('method_not_allowed', 405)
  }

  // A missing/empty stream is a graceful `undefined` (→ 404) from
  // `replayFromOffsetDO`, NOT a throw — the package's `SqliteStreamStore` reads
  // its single-row metadata tolerantly (a missing stream is zero rows, not an
  // "expected exactly one result" SQL error). A throw here can therefore only be
  // a genuine DO transport fault (binding unreachable, runtime error); map it to
  // a clean 502 instead of letting it surface as an unhandled 500 defect. This is
  // read-side fail-safe ONLY — it never masks a systematic write failure, which
  // is fixed at the source in the durable producer / DO SQL adapter.
  let replay: DurableReplay | undefined
  try {
    replay = await replayFromOffsetDO({
      namespace: deps.namespace,
      offset: matched.offset,
      requestId: matched.requestId,
    })
  } catch {
    return jsonError('durable_stream_unavailable', 502)
  }

  return replayToResponse(replay)
}
