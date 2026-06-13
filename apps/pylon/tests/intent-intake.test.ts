import { describe, expect, test } from "bun:test"
import {
  createIntentQueue,
  decodeSubmittedWorkIntent,
  transitionIntentStatus,
  type SubmittedWorkIntent,
} from "../src/node/intent-intake"

const baseIntent: SubmittedWorkIntent = {
  intentId: "intent-1",
  title: "Ship CL-35",
  body: "Private work request body with operator-only context.",
  scopeHint: "apps/pylon",
  submittedByClientRef: "client.remote-1",
  createdAt: "2026-06-13T00:00:00.000Z",
}

describe("intent intake schema", () => {
  test("decodes a submitted work intent", () => {
    const decoded = decodeSubmittedWorkIntent(baseIntent)

    expect(decoded.intentId).toBe("intent-1")
    expect(decoded.scopeHint).toBe("apps/pylon")
  })

  test("rejects malformed submitted work intent input", () => {
    expect(() =>
      decodeSubmittedWorkIntent({
        intentId: "intent-1",
        title: "Missing body",
        submittedByClientRef: "client.remote-1",
        createdAt: "2026-06-13T00:00:00.000Z",
      }),
    ).toThrow()
  })
})

describe("intent status machine", () => {
  test("accepts the full legal shipping path", () => {
    let status = transitionIntentStatus("received", "planning")
    status = transitionIntentStatus(status, "fanning_out")
    status = transitionIntentStatus(status, "shipping")
    status = transitionIntentStatus(status, "shipped")

    expect(status).toBe("shipped")
  })

  test("rejects an illegal transition", () => {
    expect(() => transitionIntentStatus("received", "shipping")).toThrow(/illegal intent status transition/)
    expect(() => transitionIntentStatus("shipped", "failed")).toThrow(/illegal intent status transition/)
  })
})

describe("intent queue", () => {
  test("is idempotent for duplicate intent ids", () => {
    const queue = createIntentQueue()
    const first = queue.enqueue(baseIntent)
    const duplicate = queue.enqueue({
      ...baseIntent,
      title: "Conflicting duplicate title",
      body: "Conflicting duplicate body.",
    })

    expect(duplicate).toEqual(first)
    expect(queue.list()).toHaveLength(1)
  })

  test("advances status append-only", () => {
    const queue = createIntentQueue()
    queue.enqueue(baseIntent)

    queue.advanceStatus("intent-1", "planning", "2026-06-13T00:01:00.000Z")
    queue.advanceStatus("intent-1", "fanning_out", "2026-06-13T00:02:00.000Z")
    queue.advanceStatus("intent-1", "shipping", "2026-06-13T00:03:00.000Z")
    const shipped = queue.advanceStatus("intent-1", "shipped", "2026-06-13T00:04:00.000Z")

    expect(shipped.status).toBe("shipped")
    expect(shipped.statusHistory.map((event) => event.status)).toEqual([
      "received",
      "planning",
      "fanning_out",
      "shipping",
      "shipped",
    ])
  })

  test("derived projections are refs-only", () => {
    const queue = createIntentQueue()
    const projection = queue.enqueue(baseIntent)
    const listed = queue.list()[0]
    const fetched = queue.get("intent-1")

    expect(projection.titleRef).toMatch(/^intent\.title\./)
    expect(projection.bodyRef).toMatch(/^intent\.body\./)
    expect(projection.scopeHintRef).toMatch(/^intent\.scope_hint\./)

    for (const view of [projection, listed, fetched]) {
      expect(JSON.stringify(view)).not.toContain(baseIntent.title)
      expect(JSON.stringify(view)).not.toContain(baseIntent.body)
      expect(JSON.stringify(view)).not.toContain(baseIntent.scopeHint)
    }
  })
})
