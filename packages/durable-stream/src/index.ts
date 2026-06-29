/**
 * `@openagentsinc/durable-stream` — an Effect + Effect Schema durable
 * append-only offset-log primitive on Cloudflare Durable Objects with SQLite
 * storage.
 *
 * Implements the four-part Durable Streams conformance contract
 * (PROTOCOL.md, ElectricSQL — ported, not vendored):
 *   1. offset-addressed replay (exact suffix, monotonic opaque offsets)
 *   2. resumability + EOF (Stream-Next-Offset / Stream-Closed)
 *   3. exactly-once writes ((producerId, epoch, seq) dedup / fence / gap)
 *   4. CDN-friendly fan-out (ETag / Cache-Control / Stream-Cursor)
 *
 * Layout:
 *   - offset.ts        offset codec (branded Effect Schema; lexicographic)
 *   - protocol.ts      wire header/param constants + helpers
 *   - core.ts          transport-agnostic state machine (pure; Bun-testable)
 *   - store.ts         StreamStore port + in-memory impl
 *   - http.ts          Web Request/Response adapter (+ SSE)
 *   - durable-object.ts  Cloudflare DO + SQLite adapter
 */
export const DURABLE_STREAM_VERSION = "0.1.0"

export * from "./offset.ts"
export * from "./protocol.ts"
export * from "./store.ts"
export * from "./core.ts"
export * from "./http.ts"
export * from "./durable-object.ts"
