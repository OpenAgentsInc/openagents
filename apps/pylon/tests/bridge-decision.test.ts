import { describe, expect, test } from "bun:test"
import {
  applyExternalDecisionResolution,
  broadcastResolution,
  createDecisionRegistry,
  pendingForReplay,
  requestDecision,
  resolveOne,
} from "../src/node/bridge-decision"

const nowMs = 1_000

function registryWithDecision(overrides: Partial<{
  requestId: string
  actionRef: string
  expiresAtMs: number
}> = {}) {
  return requestDecision(createDecisionRegistry(), {
    requestId: "decision-1",
    actionRef: "action-1",
    expiresAtMs: nowMs + 10_000,
    ...overrides,
  })
}

describe("bridge decision registry", () => {
  test("request then resolve once is accepted", () => {
    const reg = registryWithDecision()

    const resolved = resolveOne(reg, { requestId: "decision-1", verb: "approve" }, nowMs)

    expect(resolved.outcome).toBe("accepted")
    expect(resolved.reg.decisions.get("decision-1")).toEqual({
      requestId: "decision-1",
      actionRef: "action-1",
      state: "resolved",
      resolvedVerb: "approve",
      expiresAtMs: nowMs + 10_000,
    })
    expect(reg.decisions.get("decision-1")?.state).toBe("pending")
  })

  test("second resolve is duplicate for the same answer and already_resolved for a different answer", () => {
    const reg = registryWithDecision()
    const first = resolveOne(reg, { requestId: "decision-1", verb: "approve" }, nowMs)

    const duplicate = resolveOne(first.reg, { requestId: "decision-1", verb: "approve" }, nowMs)
    const alreadyResolved = resolveOne(first.reg, { requestId: "decision-1", verb: "deny" }, nowMs)

    expect(duplicate.outcome).toBe("duplicate")
    expect(alreadyResolved.outcome).toBe("already_resolved")
  })

  test("expired resolve is expired and stores the expired record", () => {
    const reg = registryWithDecision({ expiresAtMs: nowMs + 1 })

    const resolved = resolveOne(reg, { requestId: "decision-1", verb: "approve" }, nowMs + 1)

    expect(resolved.outcome).toBe("expired")
    expect(resolved.reg.decisions.get("decision-1")?.state).toBe("expired")
  })

  test("cancelled decision broadcasts decision.cancelled and blocks resolve", () => {
    const reg = registryWithDecision()
    const cancelled = applyExternalDecisionResolution(reg, "decision-1", { state: "cancelled" })

    const broadcast = broadcastResolution(cancelled, "decision-1")
    const resolved = resolveOne(cancelled, { requestId: "decision-1", verb: "approve" }, nowMs)

    expect(broadcast.event).toEqual({ name: "decision.cancelled", requestId: "decision-1" })
    expect(resolved.outcome).toBe("cancelled")
    expect(resolved.reg.decisions.get("decision-1")?.state).toBe("cancelled")
  })

  test("resolved decision broadcasts decision.resolved", () => {
    const reg = registryWithDecision()
    const resolved = resolveOne(reg, { requestId: "decision-1", verb: "approve" }, nowMs)

    expect(broadcastResolution(resolved.reg, "decision-1").event).toEqual({
      name: "decision.resolved",
      requestId: "decision-1",
    })
  })

  test("pendingForReplay returns only still-pending requests", () => {
    let reg = createDecisionRegistry()
    reg = requestDecision(reg, {
      requestId: "pending-1",
      actionRef: "action-pending-1",
      expiresAtMs: nowMs + 10_000,
    })
    reg = requestDecision(reg, {
      requestId: "resolved-1",
      actionRef: "action-resolved-1",
      expiresAtMs: nowMs + 10_000,
    })
    reg = requestDecision(reg, {
      requestId: "cancelled-1",
      actionRef: "action-cancelled-1",
      expiresAtMs: nowMs + 10_000,
    })
    reg = requestDecision(reg, {
      requestId: "expired-1",
      actionRef: "action-expired-1",
      expiresAtMs: nowMs + 1,
    })

    reg = resolveOne(reg, { requestId: "resolved-1", verb: "approve" }, nowMs).reg
    reg = applyExternalDecisionResolution(reg, "cancelled-1", { state: "cancelled" })
    reg = resolveOne(reg, { requestId: "expired-1", verb: "deny" }, nowMs + 1).reg

    expect(pendingForReplay(reg).map((decision) => decision.requestId)).toEqual(["pending-1"])
  })

  test("broadcastResolution returns null for pending or unknown requests", () => {
    const reg = registryWithDecision()

    expect(broadcastResolution(reg, "decision-1").event).toBeNull()
    expect(broadcastResolution(reg, "missing").event).toBeNull()
  })
})
