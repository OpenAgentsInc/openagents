import { describe, expect, test } from "bun:test"

import { evaluateBudget, type Budget, type UsageRef } from "../src/tas/budget"

const usage = (overrides: Partial<UsageRef> = {}): UsageRef => ({
  usageRef: "usage.fixture.run",
  totalTokens: 0,
  ...overrides,
})

describe("tas budget stop core", () => {
  test("continues under configured token and cost budget", () => {
    const budget: Budget = {
      maxTokens: 1_000,
      maxCostUsd: 2,
    }

    expect(
      evaluateBudget(
        budget,
        usage({
          totalTokens: 999,
          totalCostUsd: 1.99,
        }),
      ),
    ).toEqual({
      decision: "continue",
      reason: "within_budget",
    })
  })

  test("stops at or over budget with a usage-limit blocker ref", () => {
    expect(
      evaluateBudget(
        { maxTokens: 1_000 },
        usage({
          totalTokens: 1_000,
          usageLimitBlockerRef: "blocker.fixture.token_budget",
        }),
      ),
    ).toEqual({
      decision: "stop",
      reason: "token_budget_reached",
      blocker: {
        kind: "usage_limit",
        blockerRef: "blocker.fixture.token_budget",
        usageRef: "usage.fixture.run",
        limit: "tokens",
      },
    })

    expect(
      evaluateBudget(
        { maxCostUsd: 2 },
        usage({
          totalTokens: 1_250,
          totalCostUsd: 2.01,
          usageLimitBlockerRef: "blocker.fixture.cost_budget",
        }),
      ),
    ).toEqual({
      decision: "stop",
      reason: "cost_budget_reached",
      blocker: {
        kind: "usage_limit",
        blockerRef: "blocker.fixture.cost_budget",
        usageRef: "usage.fixture.run",
        limit: "cost_usd",
      },
    })
  })

  test("continues own-Pylon zero-credit runs when no budget is configured", () => {
    expect(evaluateBudget({}, usage())).toEqual({
      decision: "continue",
      reason: "no_budget_configured",
      note: "No token or cost budget is configured; own-Pylon zero-credit runs continue without a spend stop.",
    })
  })
})
