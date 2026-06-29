export type PolicyDecision = "allow" | "deny"

export type PolicyDenialReason =
  | "team_not_allowed"
  | "repo_not_allowed"
  | "user_not_approved"
  | "provider_blocked"
  | "budget_exceeded"
  | "retention_not_allowed"
  | "telemetry_not_allowed"

export type PolicyAllowReason = "policy_allowed"

export type PolicyReason = PolicyAllowReason | PolicyDenialReason

export type PolicySnapshot = Readonly<{
  allowedTeams?: readonly string[]
  allowedRepos?: readonly string[]
  approvedUsers?: readonly string[]
  allowedProviders?: readonly string[]
  blockedProviders?: readonly string[]
  budget?: Readonly<{
    remainingTokens?: number
    remainingCostUsd?: number
  }>
  retention?: Readonly<{
    allowedClasses?: readonly string[]
    maxDays?: number
  }>
  telemetry?: Readonly<{
    allowedModes?: readonly string[]
  }>
}>

export type PolicyRequest = Readonly<{
  team: string
  repo: string
  user: string
  provider: string
  estimatedTokens?: number
  estimatedCostUsd?: number
  retentionClass?: string
  retentionDays?: number
  telemetryMode?: string
}>

export type PolicyEvaluation = Readonly<{
  decision: PolicyDecision
  reason: PolicyReason
}>

export function evaluatePolicy(
  snapshot: PolicySnapshot,
  request: PolicyRequest,
): PolicyEvaluation {
  if (isNotAllowed(snapshot.allowedTeams, request.team)) {
    return deny("team_not_allowed")
  }

  if (isNotAllowed(snapshot.allowedRepos, request.repo)) {
    return deny("repo_not_allowed")
  }

  if (isNotAllowed(snapshot.approvedUsers, request.user)) {
    return deny("user_not_approved")
  }

  if (
    includes(snapshot.blockedProviders, request.provider) ||
    isNotAllowed(snapshot.allowedProviders, request.provider)
  ) {
    return deny("provider_blocked")
  }

  if (exceedsBudget(snapshot, request)) {
    return deny("budget_exceeded")
  }

  if (violatesRetention(snapshot, request)) {
    return deny("retention_not_allowed")
  }

  if (violatesTelemetry(snapshot, request)) {
    return deny("telemetry_not_allowed")
  }

  return {
    decision: "allow",
    reason: "policy_allowed",
  }
}

function deny(reason: PolicyDenialReason): PolicyEvaluation {
  return {
    decision: "deny",
    reason,
  }
}

function isNotAllowed(
  allowedValues: readonly string[] | undefined,
  value: string,
): boolean {
  return allowedValues !== undefined && !allowedValues.includes(value)
}

function includes(
  values: readonly string[] | undefined,
  value: string,
): boolean {
  return values?.includes(value) ?? false
}

function exceedsBudget(
  snapshot: PolicySnapshot,
  request: PolicyRequest,
): boolean {
  const budget = snapshot.budget

  if (!budget) {
    return false
  }

  if (
    budget.remainingTokens !== undefined &&
    request.estimatedTokens !== undefined &&
    request.estimatedTokens > budget.remainingTokens
  ) {
    return true
  }

  return (
    budget.remainingCostUsd !== undefined &&
    request.estimatedCostUsd !== undefined &&
    request.estimatedCostUsd > budget.remainingCostUsd
  )
}

function violatesRetention(
  snapshot: PolicySnapshot,
  request: PolicyRequest,
): boolean {
  const retention = snapshot.retention

  if (!retention) {
    return false
  }

  if (
    retention.allowedClasses !== undefined &&
    (request.retentionClass === undefined ||
      !retention.allowedClasses.includes(request.retentionClass))
  ) {
    return true
  }

  return (
    retention.maxDays !== undefined &&
    request.retentionDays !== undefined &&
    request.retentionDays > retention.maxDays
  )
}

function violatesTelemetry(
  snapshot: PolicySnapshot,
  request: PolicyRequest,
): boolean {
  const allowedModes = snapshot.telemetry?.allowedModes

  return (
    allowedModes !== undefined &&
    (request.telemetryMode === undefined ||
      !allowedModes.includes(request.telemetryMode))
  )
}
