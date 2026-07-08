export type KhalaMobileCodexAccountFailure = "account_exhausted" | "account_rate_limited"

export type KhalaMobileCodexDeviceAuthState =
  | "connected"
  | "denied"
  | "expired"
  | "pending"
  | "revoked"

export type KhalaMobileCodexReadiness =
  | "account_exhausted"
  | "account_rate_limited"
  | "pending"
  | "ready"
  | "requires_reauth"
  | "unavailable"

export type KhalaMobileCodexQuotaState =
  | "available"
  | "exhausted"
  | "rate_limited"
  | "unknown"

export type KhalaMobileCodexAccountView = Readonly<{
  accountLabel: string | null
  health: "healthy" | "requires_reauth" | "unhealthy" | "unknown"
  lastStatusAt: string
  planType: string | null
  providerAccountRef: string
  quotaState: KhalaMobileCodexQuotaState
  readiness: KhalaMobileCodexReadiness
  status: "connected" | "denied" | "disconnected" | "expired" | "pending" | "unhealthy"
}>

export const codexDeviceAuthStateFromAttempt = (
  status: "connected" | "denied" | "expired" | "failed" | "pending",
): KhalaMobileCodexDeviceAuthState =>
  status === "connected"
    ? "connected"
    : status === "denied" || status === "failed"
      ? "denied"
      : status === "expired"
        ? "expired"
        : "pending"

export const codexDeviceAuthStateFromAccount = (
  status: KhalaMobileCodexAccountView["status"],
): KhalaMobileCodexDeviceAuthState =>
  status === "connected"
    ? "connected"
    : status === "denied"
      ? "denied"
      : status === "expired"
        ? "expired"
        : status === "disconnected"
          ? "revoked"
          : "pending"

export const codexReadinessForAccount = (
  input: Readonly<{
    failure?: KhalaMobileCodexAccountFailure | null
    health: KhalaMobileCodexAccountView["health"]
    quotaState?: KhalaMobileCodexQuotaState | null
    status: KhalaMobileCodexAccountView["status"]
  }>,
): KhalaMobileCodexReadiness => {
  if (input.failure === "account_exhausted" || input.quotaState === "exhausted") return "account_exhausted"
  if (input.failure === "account_rate_limited" || input.quotaState === "rate_limited") return "account_rate_limited"
  if (input.status === "pending") return "pending"
  if (input.status !== "connected") return "unavailable"
  if (input.health === "requires_reauth") return "requires_reauth"
  if (input.health === "unhealthy") return "unavailable"
  return "ready"
}

export const codexReadinessLabel = (readiness: KhalaMobileCodexReadiness): string => {
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

export const codexQuotaLabel = (state: KhalaMobileCodexQuotaState): string => {
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

export const codexAccountTitle = (account: KhalaMobileCodexAccountView): string =>
  account.accountLabel?.trim() ||
  account.planType?.trim() ||
  account.providerAccountRef
