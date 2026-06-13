export type ShipMode = "ota" | "rebuild" | "none"

export type ShipAuthorizationDecision = "auto" | "escalate"

export type ShipAuthorizationInput = {
  shipMode: ShipMode
  estimatedCostUsd?: number
  targetsLiveDevice: boolean
}

export type ShipAuthorization = {
  decision: ShipAuthorizationDecision
  reason: string
}

export const REBUILD_SPEND_ESCALATION_THRESHOLD_USD = 0

export function decideShipAuthorization(
  input: ShipAuthorizationInput,
): ShipAuthorization {
  if (input.shipMode === "none") {
    return {
      decision: "auto",
      reason: "No ship action requested; autonomous loop may continue.",
    }
  }

  if (input.shipMode === "ota") {
    if (input.targetsLiveDevice) {
      return {
        decision: "auto",
        reason:
          "OTA update targets a live device but does not create new spend; autonomous loop may push and record receipt.",
      }
    }

    return {
      decision: "auto",
      reason:
        "OTA update does not create new spend; autonomous loop may push and record receipt.",
    }
  }

  if (
    typeof input.estimatedCostUsd === "number" &&
    input.estimatedCostUsd > REBUILD_SPEND_ESCALATION_THRESHOLD_USD
  ) {
    return {
      decision: "escalate",
      reason:
        "Rebuild is a cost-bearing EAS build with estimated spend above the autonomous threshold; owner spend-enable review required.",
    }
  }

  return {
    decision: "escalate",
    reason:
      "Rebuild is a cost-bearing EAS build; owner spend-enable review required before autonomous shipping.",
  }
}
