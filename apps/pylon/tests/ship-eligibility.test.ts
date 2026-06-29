import { describe, expect, test } from "bun:test"

import { decideShipEligibility } from "../src/coordinator/ship-eligibility"

describe("ship eligibility decision", () => {
  test("allows OTA ships when the spend gate allows", () => {
    expect(
      decideShipEligibility({
        mode: "ota",
        spendGate: { decision: "allow" },
      }),
    ).toEqual({
      eligible: true,
      mode: "ota",
      reason: "spend_gate_allowed",
    })
  })

  test("allows rebuild ships when the spend gate allows", () => {
    expect(
      decideShipEligibility({
        mode: "rebuild",
        spendGate: { decision: "allow" },
      }),
    ).toEqual({
      eligible: true,
      mode: "rebuild",
      reason: "spend_gate_allowed",
    })
  })

  test("denies OTA ships when the spend gate denies", () => {
    expect(
      decideShipEligibility({
        mode: "ota",
        spendGate: { decision: "deny" },
      }),
    ).toEqual({
      eligible: false,
      mode: "ota",
      reason: "spend_gate_denied",
    })
  })

  test("denies rebuild ships when the spend gate denies", () => {
    expect(
      decideShipEligibility({
        mode: "rebuild",
        spendGate: { decision: "deny" },
      }),
    ).toEqual({
      eligible: false,
      mode: "rebuild",
      reason: "spend_gate_denied",
    })
  })

  test("preserves the selected ship mode in the decision", () => {
    const otaDecision = decideShipEligibility({
      mode: "ota",
      spendGate: { decision: "allow" },
    })
    const rebuildDecision = decideShipEligibility({
      mode: "rebuild",
      spendGate: { decision: "deny" },
    })

    expect(otaDecision.mode).toBe("ota")
    expect(rebuildDecision.mode).toBe("rebuild")
  })
})
