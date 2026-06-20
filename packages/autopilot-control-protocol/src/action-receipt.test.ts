import { describe, expect, test } from "bun:test"
import {
  classifyActionOutcome,
  createActionQueue,
  isRetryableOutcome,
  type ActionOutcome,
} from "./action-receipt.js"

describe("classifyActionOutcome (#5002)", () => {
  test("network failure → offline (retryable)", () => {
    expect(classifyActionOutcome({ networkError: true })).toBe("offline")
    expect(isRetryableOutcome("offline")).toBe(true)
  })

  test("body signals win over a 2xx applied", () => {
    expect(classifyActionOutcome({ ok: true, status: 200, body: { duplicate: true } })).toBe("duplicate")
    expect(classifyActionOutcome({ ok: true, status: 200, body: { revoked: true } })).toBe("revoked")
    expect(classifyActionOutcome({ ok: true, status: 200, body: { stale: true } })).toBe("stale")
  })

  test("status codes map to typed outcomes", () => {
    const cases: Array<[number, ActionOutcome]> = [
      [401, "unauthorized"],
      [403, "unauthorized"],
      [404, "unsupported"],
      [501, "unsupported"],
      [409, "stale"],
      [410, "expired"],
      [429, "overloaded"],
      [503, "overloaded"],
      [500, "error"],
    ]
    for (const [status, outcome] of cases) {
      expect(classifyActionOutcome({ ok: false, status })).toBe(outcome)
    }
    expect(isRetryableOutcome("overloaded")).toBe(true)
    expect(isRetryableOutcome("unauthorized")).toBe(false)
  })

  test("clean 2xx → applied; applied:false → error", () => {
    expect(classifyActionOutcome({ ok: true, status: 200, body: { applied: true } })).toBe("applied")
    expect(classifyActionOutcome({ ok: true, status: 200, body: null })).toBe("applied")
    expect(classifyActionOutcome({ ok: true, status: 200, body: { applied: false } })).toBe("error")
  })
})

describe("createActionQueue (#5002)", () => {
  test("enqueue + drain returns live entries oldest-first and drops expired", () => {
    const q = createActionQueue<{ verb: string }>({ ttlMs: 1000 })
    q.enqueue({ id: "a", action: { verb: "approve" }, nowMs: 0 })
    q.enqueue({ id: "b", action: { verb: "cancel" }, nowMs: 500 })
    expect(q.size()).toBe(2)

    // At t=1200, "a" (expires at 1000) is expired; "b" (expires at 1500) is live.
    const drained = q.drain(1200)
    expect(drained.ready.map((e) => e.id)).toEqual(["b"])
    expect(drained.expired.map((e) => e.id)).toEqual(["a"])
    expect(q.size()).toBe(0)
  })

  test("re-enqueue by id replaces the prior entry; maxSize bounds the queue", () => {
    const q = createActionQueue<number>({ ttlMs: 10_000, maxSize: 2 })
    q.enqueue({ id: "x", action: 1, nowMs: 0 })
    q.enqueue({ id: "x", action: 2, nowMs: 1 })
    expect(q.size()).toBe(1)
    q.enqueue({ id: "y", action: 3, nowMs: 2 })
    q.enqueue({ id: "z", action: 4, nowMs: 3 })
    // maxSize 2: oldest ("x") dropped.
    const drained = q.drain(4)
    expect(drained.ready.map((e) => e.id)).toEqual(["y", "z"])
  })
})
