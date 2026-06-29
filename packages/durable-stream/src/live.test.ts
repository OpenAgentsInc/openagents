/**
 * consumer/read-longpoll + read-sse(+base64) conformance subset.
 *
 * Note: our core is synchronous (no held connection); long-poll returns
 * available data immediately, or a 204 timeout when no data is available. The
 * spec's *waiting* behavior is an adapter concern; the protocol's required
 * 204-shape, the immediate-204-on-closed-at-tail rule, and SSE framing are
 * covered here. This honest scope is documented in the README.
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

describe("consumer/read-longpoll", () => {
  test("data available → 200 with data + Stream-Cursor", async () => {
    const u = path("lp-data")
    await put(u, { headers: { "content-type": "text/plain" } })
    await post(u, { headers: { "content-type": "text/plain" }, body: "hi" })
    const res = await get(`${u}?offset=-1&live=long-poll`)
    expect(res.status).toBe(200)
    expect(await res.text()).toBe("hi")
    expect(res.headers.get(P.HDR_STREAM_CURSOR)).not.toBeNull()
  })

  test("no data → 204 with Stream-Up-To-Date + Stream-Cursor", async () => {
    const u = path("lp-empty")
    await put(u, { headers: { "content-type": "text/plain" } })
    const tail = (await post(u, { headers: { "content-type": "text/plain" }, body: "x" })).headers.get(
      P.HDR_STREAM_NEXT_OFFSET,
    )!
    const res = await get(`${u}?offset=${encodeURIComponent(tail)}&live=long-poll`)
    expect(res.status).toBe(204)
    expect(res.headers.get(P.HDR_STREAM_UP_TO_DATE)).toBe("true")
    expect(res.headers.get(P.HDR_STREAM_CURSOR)).not.toBeNull()
  })

  test("closed at tail → immediate 204 with Stream-Closed (no cursor)", async () => {
    const u = path("lp-closed")
    await put(u, { headers: { "content-type": "text/plain" } })
    const tail = (await post(u, { headers: { "content-type": "text/plain" }, body: "x" })).headers.get(
      P.HDR_STREAM_NEXT_OFFSET,
    )!
    await post(u, { headers: { [P.HDR_STREAM_CLOSED]: "true" } })
    const res = await get(`${u}?offset=${encodeURIComponent(tail)}&live=long-poll`)
    expect(res.status).toBe(204)
    expect(res.headers.get(P.HDR_STREAM_CLOSED)).toBe("true")
    expect(res.headers.get(P.HDR_STREAM_CURSOR)).toBeNull()
  })
})

describe("consumer/read-sse", () => {
  test("text stream: data event carries utf-8, control has streamNextOffset", async () => {
    const u = path("sse-text")
    await put(u, { headers: { "content-type": "text/plain" } })
    await post(u, { headers: { "content-type": "text/plain" }, body: "hello" })
    const res = await get(`${u}?offset=-1&live=sse`)
    expect(res.headers.get("content-type")).toContain("text/event-stream")
    const text = await res.text()
    expect(text).toContain("event: data")
    expect(text).toContain("data: hello")
    expect(text).toContain("event: control")
    expect(text).toContain("streamNextOffset")
  })

  test("closed stream SSE: control has streamClosed:true", async () => {
    const u = path("sse-closed")
    await put(u, { headers: { "content-type": "text/plain", [P.HDR_STREAM_CLOSED]: "true" }, body: "done" })
    const res = await get(`${u}?offset=-1&live=sse`)
    const text = await res.text()
    expect(text).toContain("streamClosed")
  })
})

describe("consumer/read-sse-base64", () => {
  test("binary stream: stream-sse-data-encoding header + base64 data", async () => {
    const u = path("sse-bin")
    await put(u, { headers: { "content-type": "application/octet-stream" } })
    // bytes [1,2,3,4,5,6]
    await post(u, {
      headers: { "content-type": "application/octet-stream" },
      body: new Uint8Array([1, 2, 3, 4, 5, 6]),
    })
    const res = await get(`${u}?offset=-1&live=sse`)
    expect(res.headers.get(P.HDR_SSE_DATA_ENCODING)).toBe("base64")
    const text = await res.text()
    // base64 of [1..6] is "AQIDBAUG"
    expect(text).toContain("AQIDBAUG")
  })
})
