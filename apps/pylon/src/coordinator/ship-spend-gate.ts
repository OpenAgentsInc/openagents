export type ShipSpendGateDecision = "allow" | "deny"

export type ShipSpendGateInput = {
  action: string
  budget: {
    spentSats: number
    budgetSats: number
    dailyCapSats: number
    perShipCapSats: number
    shipCostSats: number
    decidedAt: string
  }
}

export type ShipSpendGateReceipt = {
  schema: string
  action: string
  spentSats: number
  budgetSats: number
  remainingSats: number
  decidedAt: string
}

export type ShipSpendGateResult = {
  decision: ShipSpendGateDecision
  reasons: string[]
  receipt: ShipSpendGateReceipt
}

export const SHIP_SPEND_GATE_RECEIPT_SCHEMA =
  "openagents.pylon.ship_spend_gate.v1"

export function evaluateShipSpendGate(
  input: ShipSpendGateInput,
): ShipSpendGateResult {
  const { budget } = input
  const projectedSpentSats = budget.spentSats + budget.shipCostSats
  const reasons: string[] = []

  if (budget.budgetSats <= 0 || budget.spentSats >= budget.budgetSats) {
    reasons.push("budget_exhausted")
  }

  if (budget.shipCostSats > budget.perShipCapSats) {
    reasons.push("per_ship_cap_exceeded")
  }

  if (projectedSpentSats > budget.dailyCapSats) {
    reasons.push("daily_cap_exceeded")
  }

  if (projectedSpentSats > budget.budgetSats) {
    reasons.push("budget_exceeded")
  }

  const decision: ShipSpendGateDecision =
    reasons.length === 0 ? "allow" : "deny"
  const remainingSats =
    decision === "allow"
      ? budget.budgetSats - projectedSpentSats
      : budget.budgetSats - budget.spentSats

  return {
    decision,
    reasons,
    receipt: {
      schema: SHIP_SPEND_GATE_RECEIPT_SCHEMA,
      action: input.action,
      spentSats: budget.spentSats,
      budgetSats: budget.budgetSats,
      remainingSats,
      decidedAt: budget.decidedAt,
    },
  }
}
