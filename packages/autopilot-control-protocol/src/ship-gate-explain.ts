export type ShipGateExplainInput = {
  spendDecision: "allow" | "deny"
  mode: "ota" | "rebuild"
  estimatedCostSats: number
}

export type ShipGateExplanation = {
  allowed: boolean
  headline: string
  detail: string
}

export function explainShipGate(input: ShipGateExplainInput): ShipGateExplanation {
  if (input.spendDecision === "deny") {
    return {
      allowed: false,
      headline: "Autonomous ship blocked",
      detail: `Spend gating denied this ${formatMode(input.mode)} ship, so it cannot proceed with an estimated ${input.estimatedCostSats} sats cost.`,
    }
  }

  if (input.mode === "ota") {
    return {
      allowed: true,
      headline: "Autonomous OTA ship allowed",
      detail: `Spend gating allowed this OTA ship. OTA updates are the lower-cost path, with an estimated ${input.estimatedCostSats} sats cost.`,
    }
  }

  return {
    allowed: true,
    headline: "Autonomous rebuild ship allowed",
    detail: `Spend gating allowed this rebuild ship. Native rebuilds are the higher-cost path, with an estimated ${input.estimatedCostSats} sats cost.`,
  }
}

function formatMode(mode: "ota" | "rebuild"): string {
  return mode === "ota" ? "OTA" : "rebuild"
}
