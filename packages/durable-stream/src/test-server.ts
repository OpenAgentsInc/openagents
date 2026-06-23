/**
 * A minimal Bun-hostable Durable Streams server backed by per-stream in-memory
 * stores. This is the local target for the conformance subset tests (the
 * "conformance suite as oracle" loop) — it exercises the SAME `core.ts` +
 * `http.ts` code paths the Cloudflare DO adapter uses, with `MemoryStreamStore`
 * standing in for the DO's SQLite store.
 *
 * Each `/v1/stream/{path}` is keyed to its own `MemoryStreamStore`, exactly as
 * the DO model keys one DO per stream path.
 */
import { handleRequest, streamIdFromUrl } from "./http.ts"
import { MemoryStreamStore } from "./store.ts"
import type { StreamStore } from "./store.ts"

export class TestStreamRegistry {
  private readonly stores = new Map<string, StreamStore>()

  storeFor(streamId: string): StreamStore {
    let s = this.stores.get(streamId)
    if (s === undefined) {
      s = new MemoryStreamStore()
      this.stores.set(streamId, s)
    }
    return s
  }

  async fetch(request: Request, nowMs?: number): Promise<Response> {
    const streamId = streamIdFromUrl(request.url)
    if (streamId === null) {
      return new Response("not a stream url", { status: 404 })
    }
    const store = this.storeFor(streamId)
    return handleRequest(store, request, { streamId, ...(nowMs !== undefined ? { nowMs } : {}) })
  }
}

/** Spin up a Bun HTTP server; returns the base URL and a stop function. */
export const startTestServer = (
  registry: TestStreamRegistry = new TestStreamRegistry(),
): { baseUrl: string; stop: () => void; registry: TestStreamRegistry } => {
  const server = Bun.serve({
    port: 0,
    fetch: (req) => registry.fetch(req),
  })
  return {
    baseUrl: `http://${server.hostname}:${server.port}`,
    stop: () => server.stop(true),
    registry,
  }
}
