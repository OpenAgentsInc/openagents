/**
 * HTTP adapter: translate a Web `Request` into a `CoreRequest`, run the core
 * against a `StreamStore`, and translate the `CoreResponse` back into a Web
 * `Response` (including SSE streaming for live=sse).
 *
 * Stream URL scheme: `{base}/v1/stream/{path}` — matches the upstream server
 * conformance harness, which addresses streams at `/v1/stream/...`.
 */
import { handle } from "./core.ts"
import type { CoreRequest, CoreResponse, Method, SseDirective } from "./core.ts"
import * as P from "./protocol.ts"
import { offsetForPosition } from "./offset.ts"
import type { StreamStore } from "./store.ts"

export const STREAM_URL_PREFIX = "/v1/stream/"

/** Extract the stream id (path after the prefix) or null if not a stream URL. */
export const streamIdFromUrl = (url: string): string | null => {
  const u = new URL(url)
  if (!u.pathname.startsWith(STREAM_URL_PREFIX)) return null
  const id = u.pathname.slice(STREAM_URL_PREFIX.length)
  return id.length > 0 ? id : null
}

const lowerHeaders = (req: Request): Record<string, string | undefined> => {
  const out: Record<string, string | undefined> = {}
  req.headers.forEach((value, key) => {
    out[key.toLowerCase()] = value
  })
  return out
}

const queryOf = (url: string): Record<string, string | undefined> => {
  const out: Record<string, string | undefined> = {}
  new URL(url).searchParams.forEach((value, key) => {
    out[key] = value
  })
  return out
}

/**
 * Build a `CoreRequest` from a Web `Request`. Caller supplies the resolved
 * `streamId` (so the DO adapter can key by its own name) and `nowMs`.
 */
export const toCoreRequest = async (
  req: Request,
  streamId: string,
  nowMs: number,
): Promise<CoreRequest> => {
  const method = req.method.toUpperCase() as Method
  const body =
    method === "POST" || method === "PUT"
      ? new Uint8Array(await req.arrayBuffer())
      : new Uint8Array(0)
  return {
    method,
    streamId,
    headers: lowerHeaders(req),
    query: queryOf(req.url),
    body,
    nowMs,
  }
}

const toResponse = (store: StreamStore, cr: CoreRequest, res: CoreResponse): Response => {
  if (res.sse !== undefined) {
    return sseResponse(store, res.sse, res.headers, cr.nowMs)
  }
  return new Response(res.body as BodyInit | null, { status: res.status, headers: res.headers })
}

/**
 * One-shot handler: resolve a stream URL, run the core, return a Response.
 * Returns null if the URL is not a `/v1/stream/...` path (caller 404s/routes).
 */
export const handleRequest = async (
  store: StreamStore,
  req: Request,
  opts: { streamId: string; nowMs?: number },
): Promise<Response> => {
  const nowMs = opts.nowMs ?? Date.now()
  const cr = await toCoreRequest(req, opts.streamId, nowMs)
  const res = handle(store, cr)
  return toResponse(store, cr, res)
}

const td = new TextEncoder()

/**
 * SSE response (§5.8). We emit the currently-available suffix as `data` events,
 * then a final `control` event. If the stream is closed and the client is at
 * tail, we emit the closure control and close. A live server would keep the
 * connection open; this synchronous core delivers the catch-up portion plus the
 * up-to-date / closed control, which satisfies streaming-equivalence for the
 * stored data.
 */
const sseResponse = (
  store: StreamStore,
  sse: SseDirective,
  baseHeaders: Record<string, string>,
  nowMs: number,
): Response => {
  const meta = store.getMeta()!
  const tailLen = store.byteLength()
  const start = Math.min(sse.startPosition, tailLen)
  const chunk = store.readFrom(start, tailLen - start)
  const endPos = start + chunk.length
  const upToDate = endPos >= tailLen
  const closedAtTail = meta.closed && upToDate

  const parts: Array<string> = []
  if (chunk.length > 0) {
    const payload = sse.base64
      ? base64Encode(chunk)
      : new TextDecoder().decode(chunk)
    parts.push(formatSseData(payload))
  }
  const control: Record<string, unknown> = {
    streamNextOffset: offsetForPosition(endPos),
  }
  if (!closedAtTail) {
    control.streamCursor = P.makeCursor(nowMs, null)
  }
  if (closedAtTail) {
    control.streamClosed = true
  } else if (upToDate) {
    control.upToDate = true
  }
  parts.push(`event: control\ndata: ${JSON.stringify(control)}\n\n`)

  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(td.encode(parts.join("")))
      controller.close()
    },
  })
  return new Response(stream, { status: 200, headers: baseHeaders })
}

const formatSseData = (payload: string): string => {
  const lines = payload.split("\n").map((l) => `data: ${l}`)
  return `event: data\n${lines.join("\n")}\n\n`
}

const base64Encode = (bytes: Uint8Array): string => {
  let binary = ""
  for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]!)
  // btoa is available in Workers and Bun
  return btoa(binary)
}
