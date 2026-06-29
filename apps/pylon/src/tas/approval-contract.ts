export type ApprovalSurface = "headless" | "background" | "mobile" | "pylon" | "api"

export type ApprovalDecision = "allow" | "deny" | "ask"

export type ApprovalCapability = string

export type ApprovalRequest = {
  readonly actionRef: string
  readonly surface: ApprovalSurface
  readonly capability: ApprovalCapability
  readonly readOnly: boolean
}

export type ApprovalRuleDecision = ApprovalDecision

export type ApprovalRule = {
  readonly decision: ApprovalRuleDecision
  readonly capability: ApprovalCapability | "*"
  readonly reason: string
}

export type ApprovalPolicy = {
  readonly rules: readonly ApprovalRule[]
  readonly defaultDecision?: ApprovalDecision
  readonly defaultReason?: string
  readonly effectfulFallbackDecision?: "deny" | "ask"
}

export type ApprovalResult = {
  readonly decision: ApprovalDecision
  readonly reason: string
}

const DEFAULT_REASON = "no matching approval rule"
const EFFECTFUL_ALLOW_REASON = "effectful action cannot be allowed by read-only approval contract"

export function decideApproval(
  policy: ApprovalPolicy,
  request: ApprovalRequest,
): ApprovalResult {
  const matchedRule = findMatchingRule(policy.rules, request.capability)
  const baseDecision = matchedRule?.decision ?? policy.defaultDecision ?? "ask"
  const baseReason = matchedRule?.reason ?? policy.defaultReason ?? DEFAULT_REASON

  if (!request.readOnly && baseDecision === "allow") {
    return {
      decision: policy.effectfulFallbackDecision ?? "ask",
      reason: EFFECTFUL_ALLOW_REASON,
    }
  }

  return {
    decision: baseDecision,
    reason: baseReason,
  }
}

function findMatchingRule(
  rules: readonly ApprovalRule[],
  capability: ApprovalCapability,
): ApprovalRule | undefined {
  return (
    rules.find(
      (rule) => rule.decision === "deny" && matchesCapability(rule.capability, capability),
    ) ??
    rules.find(
      (rule) => rule.decision === "ask" && matchesCapability(rule.capability, capability),
    ) ??
    rules.find(
      (rule) => rule.decision === "allow" && matchesCapability(rule.capability, capability),
    )
  )
}

function matchesCapability(
  ruleCapability: ApprovalRule["capability"],
  capability: ApprovalCapability,
): boolean {
  return ruleCapability === "*" || ruleCapability === capability
}
