import { describe, expect, test } from "bun:test"

import { explainShipGate } from "./ship-gate-explain.js"

describe("ship gate explanation", () => {
  test("blocks an OTA ship when spend gating denies it", () => {
    expect(explainShipGate({
      spendDecision: "deny",
      mode: "ota",
      estimatedCostSats: 25,
    })).toEqual({
      allowed: false,
      headline: "Autonomous ship blocked",
      detail: "Spend gating denied this OTA ship, so it cannot proceed with an estimated 25 sats cost.",
    })
  })

  test("blocks a rebuild ship when spend gating denies it", () => {
    expect(explainShipGate({
      spendDecision: "deny",
      mode: "rebuild",
      estimatedCostSats: 2500,
    })).toEqual({
      allowed: false,
      headline: "Autonomous ship blocked",
      detail: "Spend gating denied this rebuild ship, so it cannot proceed with an estimated 2500 sats cost.",
    })
  })

  test("allows an OTA ship with a low cost note", () => {
    expect(explainShipGate({
      spendDecision: "allow",
      mode: "ota",
      estimatedCostSats: 5,
    })).toEqual({
      allowed: true,
      headline: "Autonomous OTA ship allowed",
      detail: "Spend gating allowed this OTA ship. OTA updates are the lower-cost path, with an estimated 5 sats cost.",
    })
  })

  test("allows a rebuild ship with a higher cost note", () => {
    expect(explainShipGate({
      spendDecision: "allow",
      mode: "rebuild",
      estimatedCostSats: 3000,
    })).toEqual({
      allowed: true,
      headline: "Autonomous rebuild ship allowed",
      detail: "Spend gating allowed this rebuild ship. Native rebuilds are the higher-cost path, with an estimated 3000 sats cost.",
    })
  })

  test("preserves a zero sat estimate in allowed OTA detail", () => {
    expect(explainShipGate({
      spendDecision: "allow",
      mode: "ota",
      estimatedCostSats: 0,
    })).toEqual({
      allowed: true,
      headline: "Autonomous OTA ship allowed",
      detail: "Spend gating allowed this OTA ship. OTA updates are the lower-cost path, with an estimated 0 sats cost.",
    })
  })
})
