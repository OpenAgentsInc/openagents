import { describe, expect, test } from "bun:test"
import { buildFleetPlan } from "../src/backlog.js"

describe("buildFleetPlan", () => {
  test("preserves deterministic fleet summary counts", () => {
    const plan = buildFleetPlan([
      { ref: "codex", readiness: "ready" },
      { ref: "codex-2", readiness: "credentials-missing" },
      { ref: "codex-3", readiness: "ready" },
    ])

    expect(plan.summary).toEqual({
      total: 3,
      ready: 2,
      needsAttention: 1,
    })
  })

  test("classifies each account risk for the demo report", () => {
    const plan = buildFleetPlan([
      { ref: "codex", readiness: "ready" },
      { ref: "codex-2", readiness: "credentials-missing" },
    ])

    expect(plan.rows).toEqual([
      {
        ref: "codex",
        ordinal: 1,
        readiness: "ready",
        canRunCodex: true,
        riskLevel: "low",
      },
      {
        ref: "codex-2",
        ordinal: 2,
        readiness: "credentials-missing",
        canRunCodex: false,
        riskLevel: "needs-attention",
      },
    ])
  })
})
