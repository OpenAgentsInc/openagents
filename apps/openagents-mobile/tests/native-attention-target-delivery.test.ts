import { describe, expect, test } from "vite-plus/test"

import type { ConfirmedRuntimeAttentionSnapshot } from "@openagentsinc/khala-sync-client"

import { MobileAttentionTargetSchemaVersion } from "../src/attention/mobile-attention-target"
import { openNativeAttentionTargetDelivery } from "../src/attention/native-attention-target-delivery"

const target = {
  schema: MobileAttentionTargetSchemaVersion,
  attentionRef: "interaction.1",
  threadRef: "thread.1",
  turnRef: "turn.1",
} as const

const snapshot = (phase: ConfirmedRuntimeAttentionSnapshot["status"]["phase"]): ConfirmedRuntimeAttentionSnapshot => ({
  status: { phase, cursor: phase === "live" ? 4 : null },
  pending: phase === "live" ? [{
    schema: "openagents.runtime_attention.v1",
    attentionRef: target.attentionRef,
    ownerUserId: "owner.1",
    interactionRef: target.attentionRef,
    threadRef: target.threadRef,
    turnRef: target.turnRef,
    kind: "tool_approval",
    status: "pending",
    requestedAt: "2026-07-17T12:00:00.000Z",
    expiresAt: "2026-07-17T13:00:00.000Z",
    updatedAt: "2026-07-17T12:00:00.000Z",
  }] : [],
  terminal: [],
  issues: [],
})

describe("native attention target delivery", () => {
  test("waits for live authority and then delivers the exact target once", async () => {
    let current: ConfirmedRuntimeAttentionSnapshot | null = snapshot("catching_up")
    const delivered: unknown[] = []
    const delivery = openNativeAttentionTargetDelivery({
      snapshot: () => current,
      deliver: value => { delivered.push(value); return true },
    })
    delivery.enqueue({ source: "notification", payload: target })
    await delivery.flush()
    expect(delivery.pendingCount()).toBe(1)
    expect(delivered).toEqual([])
    current = snapshot("live")
    await delivery.flush()
    expect(delivery.pendingCount()).toBe(0)
    expect(delivered).toEqual([target])
  })

  test("drops terminally invalid candidates without navigation", async () => {
    const rejected: string[] = []
    const delivery = openNativeAttentionTargetDelivery({
      snapshot: () => snapshot("live"),
      deliver: () => { throw new Error("must not deliver") },
      rejected: result => rejected.push(result.reason),
    })
    delivery.enqueue({ source: "notification", payload: { ...target, prompt: "private" } })
    await delivery.flush()
    expect(delivery.pendingCount()).toBe(0)
    expect(rejected).toEqual(["invalid_target"])
  })
})
