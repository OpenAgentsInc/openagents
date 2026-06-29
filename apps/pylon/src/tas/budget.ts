export type Budget = {
  readonly maxTokens?: number
  readonly maxCostUsd?: number
}

export type UsageRef = {
  readonly usageRef: string
  readonly totalTokens: number
  readonly totalCostUsd?: number
  readonly rateLimitBlockerRef?: string
  readonly usageLimitBlockerRef?: string
}

export type BudgetStopLimit = "tokens" | "cost_usd"

export type BudgetBlocker = {
  readonly kind: "usage_limit" | "rate_limit"
  readonly blockerRef: string
  readonly usageRef: string
  readonly limit?: BudgetStopLimit
}

export type BudgetDecision =
  | {
      readonly decision: "continue"
      readonly reason: string
      readonly note?: string
    }
  | {
      readonly decision: "stop"
      readonly reason: string
      readonly blocker: BudgetBlocker
    }

const OWN_PYLON_ZERO_CREDIT_NOTE =
  "No token or cost budget is configured; own-Pylon zero-credit runs continue without a spend stop."

export function evaluateBudget(budget: Budget, usage: UsageRef): BudgetDecision {
  if (usage.rateLimitBlockerRef) {
    return {
      decision: "stop",
      reason: "rate_limit_blocked",
      blocker: {
        kind: "rate_limit",
        blockerRef: usage.rateLimitBlockerRef,
        usageRef: usage.usageRef,
      },
    }
  }

  if (budget.maxTokens !== undefined && usage.totalTokens >= budget.maxTokens) {
    return {
      decision: "stop",
      reason: "token_budget_reached",
      blocker: {
        kind: "usage_limit",
        blockerRef:
          usage.usageLimitBlockerRef ??
          `${usage.usageRef}.blocker.token_budget_reached`,
        usageRef: usage.usageRef,
        limit: "tokens",
      },
    }
  }

  if (
    budget.maxCostUsd !== undefined &&
    usage.totalCostUsd !== undefined &&
    usage.totalCostUsd >= budget.maxCostUsd
  ) {
    return {
      decision: "stop",
      reason: "cost_budget_reached",
      blocker: {
        kind: "usage_limit",
        blockerRef:
          usage.usageLimitBlockerRef ??
          `${usage.usageRef}.blocker.cost_budget_reached`,
        usageRef: usage.usageRef,
        limit: "cost_usd",
      },
    }
  }

  if (budget.maxTokens === undefined && budget.maxCostUsd === undefined) {
    return {
      decision: "continue",
      reason: "no_budget_configured",
      note: OWN_PYLON_ZERO_CREDIT_NOTE,
    }
  }

  return {
    decision: "continue",
    reason: "within_budget",
  }
}
