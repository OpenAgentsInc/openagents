export type BridgeRenewAction = "none" | "renew" | "repair"

export type BridgeRenewPlanInput = {
  expiresAt: string | null
  nowIso: string
}

export type BridgeRenewPlan = {
  action: BridgeRenewAction
  reason: string
}

const RENEW_WINDOW_MS = 24 * 60 * 60 * 1000

export function planBridgeRenewal(input: BridgeRenewPlanInput): BridgeRenewPlan {
  if (input.expiresAt === null) {
    return { action: "repair", reason: "credential_missing_expiry" }
  }

  const expiresAtMs = Date.parse(input.expiresAt)
  const nowMs = Date.parse(input.nowIso)

  if (!Number.isFinite(expiresAtMs) || !Number.isFinite(nowMs)) {
    return { action: "repair", reason: "credential_expiry_invalid" }
  }

  if (expiresAtMs < nowMs) {
    return { action: "repair", reason: "credential_expired_repair_pairing" }
  }

  if (expiresAtMs - nowMs <= RENEW_WINDOW_MS) {
    return { action: "renew", reason: "credential_expiring_within_24h" }
  }

  return { action: "none", reason: "credential_valid" }
}
