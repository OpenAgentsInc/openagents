/**
 * CX-5 (#8549): mobile Claude account readiness projection.
 *
 * Claude connect is paste-token (`CLAUDE_CODE_OAUTH_TOKEN` from
 * `claude setup-token`), not a device-login state machine. Readiness still
 * mirrors Codex labels so Settings rows stay consistent across providers.
 */

export type KhalaMobileClaudeAccountFailure = "account_exhausted" | "account_rate_limited"

export type KhalaMobileClaudeReadiness =
  | "account_exhausted"
  | "account_rate_limited"
  | "pending"
  | "ready"
  | "requires_reauth"
  | "unavailable"

export type KhalaMobileClaudeQuotaState =
  | "available"
  | "exhausted"
  | "rate_limited"
  | "unknown"

export type KhalaMobileClaudeAccountView = Readonly<{
  accountLabel: string | null
  health: "healthy" | "requires_reauth" | "unhealthy" | "unknown"
  lastStatusAt: string
  planType: string | null
  providerAccountRef: string
  quotaState: KhalaMobileClaudeQuotaState
  readiness: KhalaMobileClaudeReadiness
  status: "connected" | "denied" | "disconnected" | "expired" | "pending" | "unhealthy"
}>

export const claudeReadinessForAccount = (
  input: Readonly<{
    failure?: KhalaMobileClaudeAccountFailure | null
    health: KhalaMobileClaudeAccountView["health"]
    quotaState?: KhalaMobileClaudeQuotaState | null
    status: KhalaMobileClaudeAccountView["status"]
  }>,
): KhalaMobileClaudeReadiness => {
  if (input.failure === "account_exhausted" || input.quotaState === "exhausted") return "account_exhausted"
  if (input.failure === "account_rate_limited" || input.quotaState === "rate_limited") return "account_rate_limited"
  if (input.status === "pending") return "pending"
  if (input.status !== "connected") return "unavailable"
  if (input.health === "requires_reauth") return "requires_reauth"
  if (input.health === "unhealthy") return "unavailable"
  return "ready"
}

export const claudeReadinessLabel = (readiness: KhalaMobileClaudeReadiness): string => {
  switch (readiness) {
    case "account_exhausted":
      return "Exhausted"
    case "account_rate_limited":
      return "Rate limited"
    case "pending":
      return "Pending"
    case "ready":
      return "Ready"
    case "requires_reauth":
      return "Reconnect"
    case "unavailable":
      return "Unavailable"
  }
}

export const claudeQuotaLabel = (state: KhalaMobileClaudeQuotaState): string => {
  switch (state) {
    case "available":
      return "Quota available"
    case "exhausted":
      return "Quota exhausted"
    case "rate_limited":
      return "Cooling down"
    case "unknown":
      return "Quota unknown"
  }
}

export const claudeAccountTitle = (account: KhalaMobileClaudeAccountView): string =>
  account.accountLabel?.trim() ||
  account.planType?.trim() ||
  account.providerAccountRef

/**
 * A Claude account is visible in the mobile list only if it is connected (or
 * a rare in-progress pending row). Terminal residue is never rendered.
 * Mirror of Codex's isVisibleCodexAccount for the Claude Settings section.
 */
export const isVisibleClaudeAccount = (account: KhalaMobileClaudeAccountView): boolean =>
  account.status === "connected" || account.status === "pending"

export const visibleClaudeAccounts = (
  accounts: ReadonlyArray<KhalaMobileClaudeAccountView>,
): ReadonlyArray<KhalaMobileClaudeAccountView> => accounts.filter(isVisibleClaudeAccount)
