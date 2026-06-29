import { describe, expect, test } from "bun:test"

import { computeCoordinatorMetrics } from "../src/coordinator/coordinator-metrics"

describe("coordinator metrics", () => {
  test("counts intents by status and sessions by state", () => {
    expect(
      computeCoordinatorMetrics({
        intents: [
          { intentId: "i1", status: "received" },
          { intentId: "i2", status: "fanning_out" },
          { intentId: "i3", status: "fanning_out" },
          { intentId: "i4", status: "shipped" },
        ],
        sessions: [
          { sessionRef: "s1", state: "running", agentRef: "agent.a" },
          { sessionRef: "s2", state: "completed", agentRef: "agent.b" },
          { sessionRef: "s3", state: "failed", agentRef: "agent.c" },
          { sessionRef: "s4", state: "running", agentRef: "agent.d" },
        ],
      }),
    ).toEqual({
      intentsByStatus: { received: 1, fanning_out: 2, shipped: 1 },
      sessionsByState: { running: 2, completed: 1, failed: 1 },
      activeAgents: 2,
      totalIntents: 4,
    })
  })

  test("deduplicates active sessions for the same agent", () => {
    const metrics = computeCoordinatorMetrics({
      intents: [],
      sessions: [
        { sessionRef: "s1", state: "running", agentRef: "agent.same" },
        { sessionRef: "s2", state: "active", agentRef: "agent.same" },
        { sessionRef: "s3", state: "running", agentRef: "agent.other" },
        { sessionRef: "s4", state: "completed", agentRef: "agent.done" },
      ],
    })

    expect(metrics.activeAgents).toBe(2)
    expect(metrics.sessionsByState).toEqual({ running: 2, active: 1, completed: 1 })
  })

  test("counts active sessions without agent references individually", () => {
    const metrics = computeCoordinatorMetrics({
      intents: [],
      sessions: [
        { sessionRef: "s1", state: "running" },
        { sessionRef: "s2", state: "active", agentId: "agent.id" },
        { sessionRef: "s3", state: "active" },
      ],
    })

    expect(metrics.activeAgents).toBe(3)
  })

  test("ignores malformed records in grouped counts", () => {
    const metrics = computeCoordinatorMetrics({
      intents: [
        { status: "received" },
        { status: "" },
        { status: null },
        null,
        "not an intent",
        { status: "received" },
      ],
      sessions: [
        { state: "running", agentRef: "" },
        { state: "" },
        { state: 42 },
        undefined,
      ],
    })

    expect(metrics.intentsByStatus).toEqual({ received: 2 })
    expect(metrics.sessionsByState).toEqual({ running: 1 })
    expect(metrics.activeAgents).toBe(1)
    expect(metrics.totalIntents).toBe(6)
  })

  test("treats non-array collections as empty at runtime", () => {
    const metrics = computeCoordinatorMetrics({
      intents: null,
      sessions: { state: "running" },
    } as unknown as { intents: any[]; sessions: any[] })

    expect(metrics).toEqual({
      intentsByStatus: {},
      sessionsByState: {},
      activeAgents: 0,
      totalIntents: 0,
    })
  })

  test("does not mutate the input arrays or records", () => {
    const input = {
      intents: [{ status: "planning" }],
      sessions: [{ state: "running", agentRef: "agent.a" }],
    }

    const before = JSON.stringify(input)
    computeCoordinatorMetrics(input)

    expect(JSON.stringify(input)).toBe(before)
  })
})
