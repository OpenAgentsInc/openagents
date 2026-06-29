import type { FingerprintShipMode } from "./ship-mode-classify.js"
import type { ShipSpendGateDecision } from "./ship-spend-gate.js"

export type ShipEligibilityInput = {
  mode: FingerprintShipMode
  spendGate: {
    decision: ShipSpendGateDecision
  }
}

export type ShipEligibilityDecision = {
  eligible: boolean
  mode: FingerprintShipMode
  reason: string
}

export function decideShipEligibility(
  input: ShipEligibilityInput,
): ShipEligibilityDecision {
  if (input.spendGate.decision === "allow") {
    return {
      eligible: true,
      mode: input.mode,
      reason: "spend_gate_allowed",
    }
  }

  return {
    eligible: false,
    mode: input.mode,
    reason: "spend_gate_denied",
  }
}
