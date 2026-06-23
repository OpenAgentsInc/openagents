/**
 * producer/idempotent/* conformance subset — exactly-once writes via
 * (producerId, epoch, seq). Mirrors upstream
 * packages/client-conformance-tests/test-cases/producer/idempotent/.
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
const get = (url: string) => fetch(url, { method: "GET" })

const append = (
  url: string,
  body: string,
  producer: { id: string; epoch: number; seq: number },
  extra: Record<string, string> = {},
) =>
  fetch(url, {
    method: "POST",
    headers: {
      "content-type": "text/plain",
      [P.HDR_PRODUCER_ID]: producer.id,
      [P.HDR_PRODUCER_EPOCH]: String(producer.epoch),
      [P.HDR_PRODUCER_SEQ]: String(producer.seq),
      ...extra,
    },
    body,
  })

describe("producer/idempotent/sequence-validation", () => {
  test("first append (epoch0 seq0) → 200", async () => {
    const u = path("idem-first")
    await put(u, { headers: { "content-type": "text/plain" } })
    const res = await append(u, "a", { id: "p1", epoch: 0, seq: 0 })
    expect(res.status).toBe(200)
    expect(res.headers.get(P.HDR_PRODUCER_EPOCH)).toBe("0")
  })

  test("duplicate seq → 204 (idempotent), no double-append", async () => {
    const u = path("idem-dup")
    await put(u, { headers: { "content-type": "text/plain" } })
    await append(u, "a", { id: "p1", epoch: 0, seq: 0 })
    await append(u, "b", { id: "p1", epoch: 0, seq: 1 })
    const dup = await append(u, "b-again", { id: "p1", epoch: 0, seq: 1 })
    expect(dup.status).toBe(204)
    const read = await get(u)
    expect(await read.text()).toBe("ab") // not "abb-again"
  })

  test("sequence gap → 409 with expected/received", async () => {
    const u = path("idem-gap")
    await put(u, { headers: { "content-type": "text/plain" } })
    await append(u, "a", { id: "p1", epoch: 0, seq: 0 })
    const gap = await append(u, "c", { id: "p1", epoch: 0, seq: 5 })
    expect(gap.status).toBe(409)
    expect(gap.headers.get(P.HDR_PRODUCER_EXPECTED_SEQ)).toBe("1")
    expect(gap.headers.get(P.HDR_PRODUCER_RECEIVED_SEQ)).toBe("5")
  })

  test("brand-new producer must start at seq 0 (nonzero → 409 gap)", async () => {
    const u = path("idem-nonzero-start")
    await put(u, { headers: { "content-type": "text/plain" } })
    const res = await append(u, "x", { id: "p1", epoch: 0, seq: 3 })
    expect(res.status).toBe(409)
  })
})

describe("producer/idempotent/epoch-management", () => {
  test("higher epoch with seq0 → accept (new session)", async () => {
    const u = path("idem-epoch-up")
    await put(u, { headers: { "content-type": "text/plain" } })
    await append(u, "a", { id: "p1", epoch: 0, seq: 0 })
    const res = await append(u, "b", { id: "p1", epoch: 1, seq: 0 })
    expect(res.status).toBe(200)
  })

  test("higher epoch with seq != 0 → 400", async () => {
    const u = path("idem-epoch-badseq")
    await put(u, { headers: { "content-type": "text/plain" } })
    await append(u, "a", { id: "p1", epoch: 0, seq: 0 })
    const res = await append(u, "b", { id: "p1", epoch: 1, seq: 4 })
    expect(res.status).toBe(400)
  })

  test("stale epoch (zombie) → 403 with current Producer-Epoch", async () => {
    const u = path("idem-zombie")
    await put(u, { headers: { "content-type": "text/plain" } })
    await append(u, "a", { id: "p1", epoch: 0, seq: 0 })
    await append(u, "b", { id: "p1", epoch: 2, seq: 0 }) // bump to epoch 2
    const zombie = await append(u, "z", { id: "p1", epoch: 0, seq: 1 })
    expect(zombie.status).toBe(403)
    expect(zombie.headers.get(P.HDR_PRODUCER_EPOCH)).toBe("2")
  })
})

describe("producer/idempotent/multi-producer", () => {
  test("independent producers have independent sequence state", async () => {
    const u = path("idem-multi")
    await put(u, { headers: { "content-type": "text/plain" } })
    await append(u, "a", { id: "pA", epoch: 0, seq: 0 })
    await append(u, "b", { id: "pB", epoch: 0, seq: 0 })
    const a1 = await append(u, "c", { id: "pA", epoch: 0, seq: 1 })
    expect(a1.status).toBe(200)
    const read = await get(u)
    expect(await read.text()).toBe("abc")
  })
})

describe("producer/idempotent/error-handling (header validation)", () => {
  test("partial producer headers → 400", async () => {
    const u = path("idem-partial")
    await put(u, { headers: { "content-type": "text/plain" } })
    const res = await fetch(u, {
      method: "POST",
      headers: { "content-type": "text/plain", [P.HDR_PRODUCER_ID]: "p1" },
      body: "x",
    })
    expect(res.status).toBe(400)
  })

  test("non-integer epoch → 400", async () => {
    const u = path("idem-bad-epoch")
    await put(u, { headers: { "content-type": "text/plain" } })
    const res = await append(u, "x", { id: "p1", epoch: 0, seq: 0 }, { [P.HDR_PRODUCER_EPOCH]: "abc" })
    expect(res.status).toBe(400)
  })
})

describe("producer/idempotent close", () => {
  test("duplicate closing append by same tuple → 204 Stream-Closed", async () => {
    const u = path("idem-close")
    await put(u, { headers: { "content-type": "text/plain" } })
    await append(u, "a", { id: "p1", epoch: 0, seq: 0 })
    const close = await append(u, "final", { id: "p1", epoch: 0, seq: 1 }, { [P.HDR_STREAM_CLOSED]: "true" })
    expect(close.headers.get(P.HDR_STREAM_CLOSED)).toBe("true")
    const dup = await append(u, "final", { id: "p1", epoch: 0, seq: 1 }, { [P.HDR_STREAM_CLOSED]: "true" })
    expect(dup.status).toBe(204)
    expect(dup.headers.get(P.HDR_STREAM_CLOSED)).toBe("true")
    const read = await get(u)
    expect(await read.text()).toBe("afinal")
  })
})
