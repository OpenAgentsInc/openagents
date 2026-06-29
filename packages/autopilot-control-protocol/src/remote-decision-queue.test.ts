import { describe, expect, test } from "bun:test"
import { createRemoteDecisionQueue, type DecisionEvent } from "./remote-decision-queue.js"
import type { BridgeTransport } from "./bridge-transport.js"

// A minimal fake of the one transport method the queue uses. `behavior` lets a
// test return an ok body, throw a network error, or throw the node's typed
// rejection per requestId.
function fakeTransport(
  behavior: (input: { requestId: string; verb: string; answer?: string }) => Promise<unknown>,
): Pick<BridgeTransport, "resolveDecision"> & { calls: Array<{ requestId: string; verb: string }> } {
  const calls: Array<{ requestId: string; verb: string }> = []
  return {
    calls,
    resolveDecision: async (input) => {
      calls.push({ requestId: input.requestId, verb: input.verb })
      return behavior(input)
    },
  }
}

const requested = (requestId: string, extra: Partial<DecisionEvent> = {}): DecisionEvent => ({
  requestId,
  phase: "requested",
  actionRef: `action.${requestId}`,
  ...extra,
})

describe("createRemoteDecisionQueue — ingest (#5000 subscribe/history → queue)", () => {
  test("a requested event opens a pending decision", () => {
    const q = createRemoteDecisionQueue({ transport: fakeTransport(async () => ({ applied: true })) })
    q.ingest(requested("d1"), 1_000)
    expect(q.pending().map((e) => e.record.requestId)).toEqual(["d1"])
    expect(q.get("d1")?.record.state).toBe("pending")
    expect(q.get("d1")?.record.actionRef).toBe("action.d1")
  })

  test("re-delivery of the same requestId is idempotent (subscribe/history overlap)", () => {
    const q = createRemoteDecisionQueue({ transport: fakeTransport(async () => ({ applied: true })) })
    q.ingestMany([requested("d1"), requested("d1"), requested("d2")], 1_000)
    expect(q.pending().length).toBe(2)
  })

  test("a remote resolved event closes the local card (another client answered)", () => {
    const q = createRemoteDecisionQueue({ transport: fakeTransport(async () => ({ applied: true })) })
    q.ingest(requested("d1"), 1_000)
    q.ingest({ requestId: "d1", phase: "resolved", resolvedVerb: "approve" }, 2_000)
    expect(q.get("d1")?.record.state).toBe("resolved")
    expect(q.get("d1")?.record.resolvedVerb).toBe("approve")
    expect(q.pending().length).toBe(0)
  })

  test("a remote cancelled event disables the card", () => {
    const q = createRemoteDecisionQueue({ transport: fakeTransport(async () => ({ applied: true })) })
    q.ingest(requested("d1"), 1_000)
    q.ingest({ requestId: "d1", phase: "cancelled" }, 2_000)
    expect(q.get("d1")?.record.state).toBe("cancelled")
  })

  test("a close event for an unknown decision is ignored", () => {
    const q = createRemoteDecisionQueue({ transport: fakeTransport(async () => ({ applied: true })) })
    q.ingest({ requestId: "ghost", phase: "resolved" }, 1_000)
    expect(q.get("ghost")).toBeUndefined()
  })
})

describe("createRemoteDecisionQueue — resolve over the bridge (exactly-once)", () => {
  test("a clean ok relay applies the decision and records an applied receipt", async () => {
    const t = fakeTransport(async () => ({ applied: true }))
    const q = createRemoteDecisionQueue({ transport: t })
    q.ingest(requested("d1"), 1_000)
    const r = await q.resolve({ requestId: "d1", verb: "approve", nowMs: 1_500 })
    expect(r.outcome).toBe("applied")
    expect(r.entry.record.state).toBe("resolved")
    expect(r.entry.record.resolvedVerb).toBe("approve")
    expect(t.calls).toEqual([{ requestId: "d1", verb: "approve" }])
  })

  test("a duplicate body signal wins over a 2xx and never re-resolves", async () => {
    const t = fakeTransport(async () => ({ duplicate: true }))
    const q = createRemoteDecisionQueue({ transport: t })
    q.ingest(requested("d1"), 1_000)
    const r = await q.resolve({ requestId: "d1", verb: "approve", nowMs: 1_500 })
    expect(r.outcome).toBe("duplicate")
    expect(r.entry.record.state).toBe("resolved")
  })

  test("a second local resolve with the same verb is a duplicate and does NOT hit the wire", async () => {
    const t = fakeTransport(async () => ({ applied: true }))
    const q = createRemoteDecisionQueue({ transport: t })
    q.ingest(requested("d1"), 1_000)
    await q.resolve({ requestId: "d1", verb: "approve", nowMs: 1_500 })
    const second = await q.resolve({ requestId: "d1", verb: "approve", nowMs: 1_600 })
    expect(second.outcome).toBe("duplicate")
    expect(t.calls.length).toBe(1) // only the first attempt reached the node
  })

  test("a conflicting second local resolve is rejected as already_resolved (no wire call)", async () => {
    const t = fakeTransport(async () => ({ applied: true }))
    const q = createRemoteDecisionQueue({ transport: t })
    q.ingest(requested("d1"), 1_000)
    await q.resolve({ requestId: "d1", verb: "approve", nowMs: 1_500 })
    const second = await q.resolve({ requestId: "d1", verb: "deny", nowMs: 1_600 })
    expect(second.outcome).toBe("rejected")
    expect(second.reason).toBe("already_resolved")
    expect(t.calls.length).toBe(1)
  })

  test("resolving an externally-cancelled decision is rejected without a wire call", async () => {
    const t = fakeTransport(async () => ({ applied: true }))
    const q = createRemoteDecisionQueue({ transport: t })
    q.ingest(requested("d1"), 1_000)
    q.ingest({ requestId: "d1", phase: "cancelled" }, 1_200)
    const r = await q.resolve({ requestId: "d1", verb: "approve", nowMs: 1_500 })
    expect(r.outcome).toBe("rejected")
    expect(r.reason).toBe("cancelled")
    expect(t.calls.length).toBe(0)
  })

  test("resolving an expired decision transitions to expired and skips the wire", async () => {
    const t = fakeTransport(async () => ({ applied: true }))
    const q = createRemoteDecisionQueue({ transport: t })
    q.ingest(requested("d1", { expiresAtMs: 2_000 }), 1_000)
    const r = await q.resolve({ requestId: "d1", verb: "approve", nowMs: 3_000 })
    expect(r.outcome).toBe("rejected")
    expect(r.reason).toBe("expired")
    expect(r.entry.record.state).toBe("expired")
    expect(t.calls.length).toBe(0)
  })

  test("resolving an unknown decision is rejected", async () => {
    const t = fakeTransport(async () => ({ applied: true }))
    const q = createRemoteDecisionQueue({ transport: t })
    const r = await q.resolve({ requestId: "ghost", verb: "approve", nowMs: 1_000 })
    expect(r.outcome).toBe("rejected")
    expect(r.reason).toBe("unknown_request")
    expect(t.calls.length).toBe(0)
  })

  test("an answer is forwarded over the bridge", async () => {
    let seenAnswer: string | undefined
    const t: Pick<BridgeTransport, "resolveDecision"> = {
      resolveDecision: async (input) => {
        seenAnswer = input.answer
        return { applied: true }
      },
    }
    const q = createRemoteDecisionQueue({ transport: t })
    q.ingest(requested("d1"), 1_000)
    await q.resolve({ requestId: "d1", verb: "answer", answer: "use staging", nowMs: 1_500 })
    expect(seenAnswer).toBe("use staging")
  })
})

describe("createRemoteDecisionQueue — node rejections (capability/authority boundary)", () => {
  test("a node 403 (no answer_decision capability) surfaces as an unauthorized receipt", async () => {
    // The BridgeTransport throws on a non-ok envelope; the node's 403 message
    // is a typed rejection, not a network error, so it is terminal.
    const t = fakeTransport(async () => {
      throw new Error("capability not granted")
    })
    const q = createRemoteDecisionQueue({ transport: t })
    q.ingest(requested("d1"), 1_000)
    const r = await q.resolve({ requestId: "d1", verb: "approve", nowMs: 1_500 })
    expect(r.outcome).toBe("error") // terminal; record stays pending (not closed)
    expect(r.entry.record.state).toBe("pending")
    expect(r.entry.pendingOffline).toBe(false)
    expect(q.offlineSize()).toBe(0)
  })
})

describe("createRemoteDecisionQueue — offline queue + drain (#5002)", () => {
  test("a network failure queues the resolution and leaves the record pending", async () => {
    const t = fakeTransport(async () => {
      throw new TypeError("Failed to fetch")
    })
    const q = createRemoteDecisionQueue({ transport: t })
    q.ingest(requested("d1"), 1_000)
    const r = await q.resolve({ requestId: "d1", verb: "approve", nowMs: 1_500 })
    expect(r.outcome).toBe("queued")
    expect(r.entry.record.state).toBe("pending")
    expect(q.offlineSize()).toBe(1)
  })

  test("drain replays a queued resolution when the bridge returns", async () => {
    let online = false
    const t: Pick<BridgeTransport, "resolveDecision"> = {
      resolveDecision: async () => {
        if (!online) throw new TypeError("Failed to fetch")
        return { applied: true }
      },
    }
    const q = createRemoteDecisionQueue({ transport: t })
    q.ingest(requested("d1"), 1_000)
    await q.resolve({ requestId: "d1", verb: "approve", nowMs: 1_500 })
    expect(q.offlineSize()).toBe(1)

    online = true
    const drained = await q.drain(2_000)
    expect(drained).toEqual([{ requestId: "d1", outcome: "applied" }])
    expect(q.get("d1")?.record.state).toBe("resolved")
    expect(q.offlineSize()).toBe(0)
  })

  test("a still-offline drain re-enqueues for the next attempt", async () => {
    const t = fakeTransport(async () => {
      throw new TypeError("Failed to fetch")
    })
    const q = createRemoteDecisionQueue({ transport: t })
    q.ingest(requested("d1"), 1_000)
    await q.resolve({ requestId: "d1", verb: "approve", nowMs: 1_500 })
    const drained = await q.drain(2_000)
    expect(drained).toEqual([{ requestId: "d1", outcome: "offline" }])
    expect(q.offlineSize()).toBe(1) // re-queued
  })

  test("a queued resolution past its TTL is dropped as expired on drain", async () => {
    const t = fakeTransport(async () => {
      throw new TypeError("Failed to fetch")
    })
    const q = createRemoteDecisionQueue({ transport: t, offlineTtlMs: 1_000 })
    q.ingest(requested("d1", { expiresAtMs: 10_000_000 }), 1_000)
    await q.resolve({ requestId: "d1", verb: "approve", nowMs: 1_500 })
    // Drain well past the offline TTL.
    const drained = await q.drain(1_500 + 5_000)
    expect(drained).toEqual([{ requestId: "d1", outcome: "expired" }])
    expect(q.get("d1")?.receipt).toBe("expired")
    expect(q.offlineSize()).toBe(0)
  })
})

describe("createRemoteDecisionQueue — multi-client convergence", () => {
  test("a local resolve and the node's resolved broadcast converge to one resolved record", async () => {
    const t = fakeTransport(async () => ({ applied: true }))
    const q = createRemoteDecisionQueue({ transport: t })
    q.ingest(requested("d1"), 1_000)
    await q.resolve({ requestId: "d1", verb: "approve", nowMs: 1_500 })
    // The node then broadcasts the resolution back over subscribe; the external
    // merge is a no-op on an already-resolved record (no flip-flop).
    q.ingest({ requestId: "d1", phase: "resolved", resolvedVerb: "approve" }, 1_600)
    expect(q.get("d1")?.record.state).toBe("resolved")
    expect(q.get("d1")?.record.resolvedVerb).toBe("approve")
  })

  test("pending() lists only still-answerable decisions, oldest-first", () => {
    const t = fakeTransport(async () => ({ applied: true }))
    const q = createRemoteDecisionQueue({ transport: t })
    q.ingestMany([requested("d1"), requested("d2"), requested("d3")], 1_000)
    q.ingest({ requestId: "d2", phase: "resolved", resolvedVerb: "deny" }, 1_100)
    expect(q.pending().map((e) => e.record.requestId)).toEqual(["d1", "d3"])
    expect(q.all().length).toBe(3)
  })
})
