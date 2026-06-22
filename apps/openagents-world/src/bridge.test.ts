import { describe, expect, test } from "bun:test"

import {
  bridgeHealthRow,
  decodePublicBridgeRows,
  dedupeBridgeRows,
  planBridgeRetry,
  projectionCursorRow,
  projectionRowMetadata,
  projectionRowRef,
  rowsFromTassadarRunSummary,
} from "./bridge"

const observedAt = "2026-06-22T00:00:00.000Z"
const sourceRef = "https://openagents.com/api/public/tassadar-run-summary"

describe("world bridge projection helpers", () => {
  test("maps Tassadar summaries to deterministic public rows", () => {
    const summary = {
      runRef: "run.cs336.a1.demo",
      label: "CS336 A1 Demo",
      state: "accepted",
      pylons: [
        { pylonRef: "pylon.tassadar.executor", label: "Executor", status: "working", position: { x: 1, y: 0, z: 2 } },
      ],
      entities: [
        { entityRef: "entity.executor", label: "Executor", entityKind: "agent" },
      ],
      proofRefs: [
        { proofRef: "proof.public.1", label: "Proof", url: "https://openagents.com/proofs/1" },
      ],
      settlementRefs: [
        { settlementRef: "settlement.public.1", label: "Settlement", amountSats: 1000 },
      ],
      events: [
        { eventKind: "accepted", text: "Run accepted" },
        { eventKind: "accepted", text: "Run accepted" },
      ],
    }

    const first = rowsFromTassadarRunSummary(summary, observedAt, sourceRef)
    const second = rowsFromTassadarRunSummary(summary, observedAt, sourceRef)

    expect(first.map(projectionRowRef)).toEqual(second.map(projectionRowRef))
    expect(first.filter(row => row.kind === "world_event")).toHaveLength(2)
    expect(new Set(first.map(projectionRowRef)).size).toBe(first.length)
    for (const row of first) {
      expect(row.safety.publicProjectionAllowed).toBe(true)
      expect(row.safety.sourceRefs.map(String)).toEqual([sourceRef])
    }
  })

  test("dedupes event rows by deterministic key on replay", () => {
    const rows = rowsFromTassadarRunSummary({
      runRef: "run.demo",
      events: [
        { eventRef: "event.public.1", eventKind: "status", text: "one" },
      ],
    }, observedAt, sourceRef)
    const replay = dedupeBridgeRows([...rows, ...rows])

    expect(replay.map(projectionRowRef)).toEqual(rows.map(projectionRowRef))
  })

  test("keeps proof and settlement rows source-ref only and metadata indexed", () => {
    const rows = rowsFromTassadarRunSummary({
      runRef: "run.demo",
      proofRefs: [{ label: "Proof", url: "https://openagents.com/proofs/1" }],
      settlementRefs: [{ label: "Settlement", amountSats: 7 }],
    }, observedAt, sourceRef)
    const proof = rows.find(row => row.kind === "proof_ref")
    const settlement = rows.find(row => row.kind === "settlement_ref")

    expect(proof?.safety.sourceRefs.map(String)).toEqual([sourceRef])
    expect(settlement?.safety.sourceRefs.map(String)).toEqual([sourceRef])
    expect(proof === undefined ? null : projectionRowMetadata(proof).runRef).toBe("run.demo")
    expect(settlement === undefined ? null : projectionRowMetadata(settlement).runRef).toBe("run.demo")
  })

  test("records bridge health and cursor rows without fabricated projection data", () => {
    const health = bridgeHealthRow({
      sourceRef,
      status: "failed",
      observedAt,
      diagnosticRefs: ["diagnostic.public.bridge"],
    })
    const cursor = projectionCursorRow({
      sourceRef,
      cursor: "cursor.bridge.7",
      observedAt,
    })

    expect(health.kind).toBe("bridge_health")
    expect(health.safety.sourceRefs.map(String)).toEqual([sourceRef])
    expect(cursor.kind).toBe("projection_cursor")
    expect(projectionRowMetadata(cursor).cursor).toBe("cursor.bridge.7")
  })

  test("rejects unsafe bridge payload rows before persistence", () => {
    expect(() => decodePublicBridgeRows([{
      kind: "world_event",
      eventRef: "event.private.1",
      regionRef: "region.run.demo",
      runRef: "run.demo",
      eventKind: "raw_prompt",
      text: "raw_prompt private body",
      createdAt: observedAt,
      sourceRefs: [sourceRef],
      safety: {
        publicProjectionAllowed: true,
        sourceRefs: [sourceRef],
        blockerRefs: [],
        caveatRefs: [],
      },
    }])).toThrow()
  })

  test("classifies retriable and terminal queue bridge work", () => {
    expect(planBridgeRetry({
      reason: "temporary D1 timeout",
      attempt: 1,
      maxAttempts: 3,
    })).toEqual({
      kind: "retry",
      reason: "temporary D1 timeout",
      attempt: 1,
      maxAttempts: 3,
    })
    expect(planBridgeRetry({
      reason: "raw_prompt private payload",
      attempt: 3,
      maxAttempts: 3,
    })).toEqual({
      kind: "terminal",
      reason: "redacted private payload",
      attempt: 3,
      maxAttempts: 3,
    })
  })
})
