import { describe, expect, test } from "bun:test"

import {
  evaluateShipSpendGate,
  SHIP_SPEND_GATE_RECEIPT_SCHEMA,
  type ShipSpendGateInput,
} from "../src/coordinator/ship-spend-gate"

const baseInput = (
  overrides: Partial<ShipSpendGateInput["budget"]> = {},
): ShipSpendGateInput => ({
  action: "ship.autonomous.ota",
  budget: {
    spentSats: 1_000,
    budgetSats: 10_000,
    dailyCapSats: 5_000,
    perShipCapSats: 1_500,
    shipCostSats: 1_000,
    decidedAt: "2026-06-13T12:00:00.000Z",
    ...overrides,
  },
})

describe("CL-41 ship spend gate", () => {
  test("allows under-budget autonomous shipping", () => {
    expect(evaluateShipSpendGate(baseInput())).toEqual({
      decision: "allow",
      reasons: [],
      receipt: {
        schema: SHIP_SPEND_GATE_RECEIPT_SCHEMA,
        action: "ship.autonomous.ota",
        spentSats: 1_000,
        budgetSats: 10_000,
        remainingSats: 8_000,
        decidedAt: "2026-06-13T12:00:00.000Z",
      },
    })
  })

  test("denies when ship cost exceeds the per-ship cap", () => {
    const result = evaluateShipSpendGate(
      baseInput({
        perShipCapSats: 999,
      }),
    )

    expect(result.decision).toBe("deny")
    expect(result.reasons).toContain("per_ship_cap_exceeded")
  })

  test("denies when projected spend exceeds the daily cap", () => {
    const result = evaluateShipSpendGate(
      baseInput({
        dailyCapSats: 1_999,
      }),
    )

    expect(result.decision).toBe("deny")
    expect(result.reasons).toContain("daily_cap_exceeded")
  })

  test("denies when budget is exhausted", () => {
    const result = evaluateShipSpendGate(
      baseInput({
        spentSats: 10_000,
      }),
    )

    expect(result.decision).toBe("deny")
    expect(result.reasons).toContain("budget_exhausted")
  })

  test("denies zero-budget autonomous shipping", () => {
    const result = evaluateShipSpendGate(
      baseInput({
        spentSats: 0,
        budgetSats: 0,
        shipCostSats: 0,
      }),
    )

    expect(result.decision).toBe("deny")
    expect(result.reasons).toContain("budget_exhausted")
  })

  test("allows exact budget, daily, and per-ship cap boundaries", () => {
    expect(
      evaluateShipSpendGate(
        baseInput({
          spentSats: 4_000,
          budgetSats: 5_000,
          dailyCapSats: 5_000,
          perShipCapSats: 1_000,
          shipCostSats: 1_000,
        }),
      ),
    ).toEqual({
      decision: "allow",
      reasons: [],
      receipt: {
        schema: SHIP_SPEND_GATE_RECEIPT_SCHEMA,
        action: "ship.autonomous.ota",
        spentSats: 4_000,
        budgetSats: 5_000,
        remainingSats: 0,
        decidedAt: "2026-06-13T12:00:00.000Z",
      },
    })
  })
})
