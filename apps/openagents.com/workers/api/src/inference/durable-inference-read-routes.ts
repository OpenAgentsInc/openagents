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
  replayFromOffset,
} from './durable-inference-proxy'

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

// Dispatch a durable inference read. Returns `undefined` when the request is not
// a durable read URL (the worker router falls through). When it IS a durable read
// URL but the gateway is off / unwired / the id is unknown, returns the matching
// 404/400/200 Response so the surface is honest.
export const routeDurableInferenceReadRequest = (
  request: Request,
  deps: DurableInferenceReadDeps,
): Response | undefined => {
  const url = new URL(request.url)
  const requestId = requestIdFromPath(url.pathname)
  if (requestId === undefined) {
    return undefined
  }

  // Shares the gateway flag: inert/404 when the gateway is off.
  if (!deps.enabled || deps.durableStream === undefined) {
    return new Response(JSON.stringify({ error: 'not_found' }), {
      headers: { 'cache-control': 'no-store', 'content-type': 'application/json' },
      status: 404,
    })
  }

  if (request.method !== 'GET') {
    return new Response(JSON.stringify({ error: 'method_not_allowed' }), {
      headers: { 'cache-control': 'no-store', 'content-type': 'application/json' },
      status: 405,
    })
  }

  const store = deps.durableStream(requestId)
  if (store === undefined) {
    return new Response(JSON.stringify({ error: 'not_found' }), {
      headers: { 'cache-control': 'no-store', 'content-type': 'application/json' },
      status: 404,
    })
  }

  const replay = replayFromOffset({
    nowMs: deps.nowEpochMillis(),
    offset: url.searchParams.get('offset') ?? undefined,
    requestId,
    store,
  })

  // Unknown request id (no durable stream) → 404.
  if (replay === undefined) {
    return new Response(JSON.stringify({ error: 'not_found' }), {
      headers: { 'cache-control': 'no-store', 'content-type': 'application/json' },
      status: 404,
    })
  }

  // Malformed offset → 400.
  if (replay.status === 400) {
    return new Response(JSON.stringify({ error: 'invalid_offset' }), {
      headers: { 'cache-control': 'no-store', 'content-type': 'application/json' },
      status: 400,
    })
  }

  // Replay the stored SSE suffix. `Stream-Next-Offset` lets the client resume
  // from exactly where it left off; `Stream-Closed` signals the completion EOF.
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
