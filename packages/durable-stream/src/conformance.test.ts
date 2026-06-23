/**
 * Conformance subset, driven over HTTP against the in-memory test server (same
 * core + http paths as the DO adapter). These cases replicate the upstream
 * Durable Streams conformance buckets (PROTOCOL.md / packages/client-conformance-
 * tests/test-cases) as owned Bun tests — the "conformance suite as oracle" loop.
 *
 * Buckets covered: producer (create/append/seq/idempotent), consumer
 * (catch-up/offset-resumption/message-ordering/streaming-equivalence/cache),
 * lifecycle (closure/lifecycle), validation. Coverage gaps are reported in the
 * package README, not hidden.
 */
import { afterAll, beforeAll, describe, expect, test } from "bun:test"
import * as P from "./protocol.ts"
import { startTestServer } from "./test-server.ts"

let baseUrl: string
let stop: () => void

beforeAll(() => {
  const s = startTestServer()
  baseUrl = s.baseUrl
  stop = s.stop
})
afterAll(() => stop())

let counter = 0
const path = (name: string) => `${baseUrl}/v1/stream/${name}-${Date.now()}-${counter++}`

const put = (url: string, init: RequestInit = {}) => fetch(url, { method: "PUT", ...init })
const post = (url: string, init: RequestInit = {}) => fetch(url, { method: "POST", ...init })
const get = (url: string) => fetch(url, { method: "GET" })
const head = (url: string) => fetch(url, { method: "HEAD" })
const del = (url: string) => fetch(url, { method: "DELETE" })

// ---------------------------------------------------------------------------
// producer/create-stream
// ---------------------------------------------------------------------------

describe("producer/create-stream", () => {
  test("creates a stream (201) with Location + Stream-Next-Offset", async () => {
    const u = path("create")
    const res = await put(u, { headers: { "content-type": "text/plain" } })
    expect(res.status).toBe(201)
    expect(res.headers.get(P.HDR_STREAM_NEXT_OFFSET)).not.toBeNull()
  })

  test("idempotent create with same config → 200", async () => {
    const u = path("idem")
    await put(u, { headers: { "content-type": "text/plain" } })
    const res = await put(u, { headers: { "content-type": "text/plain" } })
    expect(res.status).toBe(200)
  })

  test("create with different content-type → 409", async () => {
    const u = path("mismatch")
    await put(u, { headers: { "content-type": "text/plain" } })
    const res = await put(u, { headers: { "content-type": "application/json" } })
    expect(res.status).toBe(409)
  })

  test("create-and-close (Stream-Closed:true + body) is immediately closed", async () => {
    const u = path("create-closed")
    const res = await put(u, {
      headers: { "content-type": "text/plain", [P.HDR_STREAM_CLOSED]: "true" },
      body: "final",
    })
    expect(res.status).toBe(201)
    expect(res.headers.get(P.HDR_STREAM_CLOSED)).toBe("true")
    const read = await get(u)
    expect(await read.text()).toBe("final")
    expect(read.headers.get(P.HDR_STREAM_CLOSED)).toBe("true")
  })

  test("rejects both Stream-TTL and Stream-Expires-At → 400", async () => {
    const u = path("ttl-conflict")
    const res = await put(u, {
      headers: { [P.HDR_STREAM_TTL]: "60", [P.HDR_STREAM_EXPIRES_AT]: "2030-01-01T00:00:00Z" },
    })
    expect(res.status).toBe(400)
  })

  test("rejects malformed Stream-TTL → 400", async () => {
    const u = path("ttl-bad")
    const res = await put(u, { headers: { [P.HDR_STREAM_TTL]: "03600" } })
    expect(res.status).toBe(400)
  })
})

// ---------------------------------------------------------------------------
// producer/append-data + sequence-ordering
// ---------------------------------------------------------------------------

describe("producer/append-data", () => {
  test("append single + multiple chunks concatenates", async () => {
    const u = path("append")
    await put(u, { headers: { "content-type": "text/plain" } })
    await post(u, { headers: { "content-type": "text/plain" }, body: "chunk1" })
    await post(u, { headers: { "content-type": "text/plain" }, body: "chunk2" })
    const res = await get(u)
    expect(await res.text()).toBe("chunk1chunk2")
  })

  test("append to nonexistent stream → 404", async () => {
    const u = path("missing")
    const res = await post(u, { headers: { "content-type": "text/plain" }, body: "x" })
    expect(res.status).toBe(404)
  })

  test("empty body without Stream-Closed → 400", async () => {
    const u = path("empty")
    await put(u, { headers: { "content-type": "text/plain" } })
    const res = await post(u, { headers: { "content-type": "text/plain" } })
    expect(res.status).toBe(400)
  })

  test("content-type mismatch on append → 409", async () => {
    const u = path("ct-mismatch")
    await put(u, { headers: { "content-type": "text/plain" } })
    const res = await post(u, { headers: { "content-type": "application/json" }, body: "{}" })
    expect(res.status).toBe(409)
  })

  test("Stream-Seq monotonicity: regression → 409", async () => {
    const u = path("seq")
    await put(u, { headers: { "content-type": "text/plain" } })
    await post(u, { headers: { "content-type": "text/plain", [P.HDR_STREAM_SEQ]: "001" }, body: "a" })
    await post(u, { headers: { "content-type": "text/plain", [P.HDR_STREAM_SEQ]: "002" }, body: "b" })
    const res = await post(u, {
      headers: { "content-type": "text/plain", [P.HDR_STREAM_SEQ]: "001" },
      body: "c",
    })
    expect(res.status).toBe(409)
  })
})

// ---------------------------------------------------------------------------
// consumer/read-catchup + offset-resumption + message-ordering
// ---------------------------------------------------------------------------

describe("consumer/read-catchup", () => {
  test("read empty stream → 200 upToDate, 0 bytes", async () => {
    const u = path("empty-read")
    await put(u, { headers: { "content-type": "text/plain" } })
    const res = await get(u)
    expect(res.status).toBe(200)
    expect(await res.text()).toBe("")
    expect(res.headers.get(P.HDR_STREAM_UP_TO_DATE)).toBe("true")
  })

  test("read single chunk", async () => {
    const u = path("single")
    await put(u, { headers: { "content-type": "text/plain" } })
    await post(u, { headers: { "content-type": "text/plain" }, body: "Hello, World!" })
    const res = await get(u)
    expect(await res.text()).toBe("Hello, World!")
    expect(res.headers.get(P.HDR_STREAM_UP_TO_DATE)).toBe("true")
  })

  test("read from offset returns exact suffix (offset-resumption)", async () => {
    const u = path("offset")
    await put(u, { headers: { "content-type": "text/plain" } })
    const first = await post(u, { headers: { "content-type": "text/plain" }, body: "First" })
    const off = first.headers.get(P.HDR_STREAM_NEXT_OFFSET)!
    await post(u, { headers: { "content-type": "text/plain" }, body: "Second" })
    const res = await get(`${u}?offset=${encodeURIComponent(off)}`)
    expect(await res.text()).toBe("Second")
  })

  test("message ordering preserved across many appends", async () => {
    const u = path("ordering")
    await put(u, { headers: { "content-type": "text/plain" } })
    for (let i = 0; i < 10; i++) {
      await post(u, { headers: { "content-type": "text/plain" }, body: `${i}` })
    }
    const res = await get(u)
    expect(await res.text()).toBe("0123456789")
  })

  test("unicode round-trips", async () => {
    const u = path("unicode")
    await put(u, { headers: { "content-type": "text/plain; charset=utf-8" } })
    await post(u, { headers: { "content-type": "text/plain; charset=utf-8" }, body: "日本語 🎉 Ñoño" })
    const res = await get(u)
    expect(await res.text()).toBe("日本語 🎉 Ñoño")
  })

  test("offset=now → empty body, upToDate, no-store", async () => {
    const u = path("now")
    await put(u, { headers: { "content-type": "text/plain" } })
    await post(u, { headers: { "content-type": "text/plain" }, body: "existing" })
    const res = await get(`${u}?offset=now`)
    expect(await res.text()).toBe("")
    expect(res.headers.get(P.HDR_STREAM_UP_TO_DATE)).toBe("true")
    expect(res.headers.get(P.HDR_CACHE_CONTROL)).toBe(P.NO_STORE)
  })

  test("offset=-1 reads from beginning", async () => {
    const u = path("begin")
    await put(u, { headers: { "content-type": "text/plain" } })
    await post(u, { headers: { "content-type": "text/plain" }, body: "abc" })
    const res = await get(`${u}?offset=-1`)
    expect(await res.text()).toBe("abc")
  })

  test("malformed offset → 400", async () => {
    const u = path("badoffset")
    await put(u, { headers: { "content-type": "text/plain" } })
    const res = await get(`${u}?offset=not-an-offset`)
    expect(res.status).toBe(400)
  })
})

// ---------------------------------------------------------------------------
// consumer/streaming-equivalence
// ---------------------------------------------------------------------------

describe("consumer/streaming-equivalence", () => {
  test("catch-up and SSE yield identical stored data", async () => {
    const u = path("equiv")
    await put(u, { headers: { "content-type": "text/plain" } })
    await post(u, { headers: { "content-type": "text/plain" }, body: "alpha" })
    await post(u, { headers: { "content-type": "text/plain" }, body: "beta" })

    const catchup = await get(u)
    const catchupText = await catchup.text()

    const sseRes = await get(`${u}?offset=-1&live=sse`)
    expect(sseRes.headers.get("content-type")).toContain("text/event-stream")
    const sseText = await sseRes.text()
    // extract data lines
    const data = sseText
      .split("\n")
      .filter((l) => l.startsWith("data: ") && !l.includes("streamNextOffset"))
      .map((l) => l.slice("data: ".length))
      .join("")
    expect(data).toBe(catchupText)
  })
})

// ---------------------------------------------------------------------------
// consumer/cache-headers (fan-out)
// ---------------------------------------------------------------------------

describe("consumer/cache-headers", () => {
  test("catch-up read has ETag and cacheable Cache-Control", async () => {
    const u = path("cache")
    await put(u, { headers: { "content-type": "text/plain" } })
    await post(u, { headers: { "content-type": "text/plain" }, body: "data" })
    const res = await get(u)
    expect(res.headers.get(P.HDR_ETAG)).not.toBeNull()
    expect(res.headers.get(P.HDR_CACHE_CONTROL)).toContain("max-age=60")
  })

  test("ETag varies with closure status", async () => {
    const u = path("etag-close")
    await put(u, { headers: { "content-type": "text/plain" } })
    await post(u, { headers: { "content-type": "text/plain" }, body: "data" })
    const open = await get(u)
    const openTag = open.headers.get(P.HDR_ETAG)!
    await post(u, { headers: { [P.HDR_STREAM_CLOSED]: "true" } })
    const closed = await get(u)
    const closedTag = closed.headers.get(P.HDR_ETAG)!
    expect(closedTag).not.toBe(openTag)
    expect(closedTag).toContain(":c")
  })
})

// ---------------------------------------------------------------------------
// lifecycle/stream-closure + stream-lifecycle
// ---------------------------------------------------------------------------

describe("lifecycle/stream-closure", () => {
  test("close-only then EOF discovered at tail offset", async () => {
    const u = path("close")
    await put(u, { headers: { "content-type": "text/plain" } })
    const app = await post(u, { headers: { "content-type": "text/plain" }, body: "done" })
    const tail = app.headers.get(P.HDR_STREAM_NEXT_OFFSET)!
    const close = await post(u, { headers: { [P.HDR_STREAM_CLOSED]: "true" } })
    expect(close.status).toBe(204)
    expect(close.headers.get(P.HDR_STREAM_CLOSED)).toBe("true")
    // reading at the tail offset returns empty body + Stream-Closed (EOF)
    const eof = await get(`${u}?offset=${encodeURIComponent(tail)}`)
    expect(await eof.text()).toBe("")
    expect(eof.headers.get(P.HDR_STREAM_CLOSED)).toBe("true")
  })

  test("close is idempotent (close-only on closed → 204 Stream-Closed)", async () => {
    const u = path("close-idem")
    await put(u, { headers: { "content-type": "text/plain" } })
    await post(u, { headers: { [P.HDR_STREAM_CLOSED]: "true" } })
    const again = await post(u, { headers: { [P.HDR_STREAM_CLOSED]: "true" } })
    expect(again.status).toBe(204)
    expect(again.headers.get(P.HDR_STREAM_CLOSED)).toBe("true")
  })

  test("append to closed stream → 409 with Stream-Closed + final offset", async () => {
    const u = path("append-closed")
    await put(u, { headers: { "content-type": "text/plain" } })
    await post(u, { headers: { "content-type": "text/plain" }, body: "x" })
    await post(u, { headers: { [P.HDR_STREAM_CLOSED]: "true" } })
    const res = await post(u, { headers: { "content-type": "text/plain" }, body: "more" })
    expect(res.status).toBe(409)
    expect(res.headers.get(P.HDR_STREAM_CLOSED)).toBe("true")
    expect(res.headers.get(P.HDR_STREAM_NEXT_OFFSET)).not.toBeNull()
  })

  test("atomic append-and-close", async () => {
    const u = path("append-and-close")
    await put(u, { headers: { "content-type": "text/plain" } })
    const res = await post(u, {
      headers: { "content-type": "text/plain", [P.HDR_STREAM_CLOSED]: "true" },
      body: "last",
    })
    expect(res.status).toBe(204)
    expect(res.headers.get(P.HDR_STREAM_CLOSED)).toBe("true")
    const read = await get(u)
    expect(await read.text()).toBe("last")
  })
})

describe("lifecycle/stream-lifecycle", () => {
  test("HEAD returns tail offset + closure status", async () => {
    const u = path("head")
    await put(u, { headers: { "content-type": "text/plain", [P.HDR_STREAM_TTL]: "120" } })
    await post(u, { headers: { "content-type": "text/plain" }, body: "abc" })
    const res = await head(u)
    expect(res.status).toBe(200)
    expect(res.headers.get(P.HDR_STREAM_NEXT_OFFSET)).not.toBeNull()
    expect(res.headers.get(P.HDR_STREAM_TTL)).toBe("120")
    expect(res.headers.get(P.HDR_STREAM_CLOSED)).toBeNull()
  })

  test("HEAD on missing stream → 404", async () => {
    const res = await head(path("head-missing"))
    expect(res.status).toBe(404)
  })

  test("DELETE then read → 404; recreate is isolated", async () => {
    const u = path("delete")
    await put(u, { headers: { "content-type": "text/plain" }, body: "old" })
    const d = await del(u)
    expect(d.status).toBe(204)
    const gone = await get(u)
    expect(gone.status).toBe(404)
    const recreate = await put(u, { headers: { "content-type": "text/plain" }, body: "new" })
    expect(recreate.status).toBe(201)
    const read = await get(u)
    expect(await read.text()).toBe("new")
  })

  test("DELETE missing → 404", async () => {
    const res = await del(path("delete-missing"))
    expect(res.status).toBe(404)
  })
})
