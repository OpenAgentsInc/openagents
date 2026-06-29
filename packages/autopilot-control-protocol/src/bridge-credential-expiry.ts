export type BridgeCredentialExpiryInput = {
  expiresAt: string | null
  nowIso: string
  refreshWindowMs?: number
}

export type BridgeCredentialExpiryState = "valid" | "expiring" | "expired" | "none"

export type BridgeCredentialExpiryProjection = {
  state: BridgeCredentialExpiryState
  msRemaining: number | null
  shouldRefresh: boolean
}

const DEFAULT_REFRESH_WINDOW_MS = 5 * 60 * 1000

export function credentialExpiry(
  input: BridgeCredentialExpiryInput,
): BridgeCredentialExpiryProjection {
  if (typeof input.expiresAt !== "string" || input.expiresAt.length === 0) {
    return none()
  }

  const expiresAtMs = Date.parse(input.expiresAt)
  const nowMs = Date.parse(input.nowIso)
  if (!Number.isFinite(expiresAtMs) || !Number.isFinite(nowMs)) {
    return none()
  }

  const msRemaining = expiresAtMs - nowMs
  if (msRemaining < 0) {
    return {
      state: "expired",
      msRemaining,
      shouldRefresh: true,
    }
  }

  if (msRemaining <= refreshWindowMs(input.refreshWindowMs)) {
    return {
      state: "expiring",
      msRemaining,
      shouldRefresh: true,
    }
  }

  return {
    state: "valid",
    msRemaining,
    shouldRefresh: false,
  }
}

function refreshWindowMs(value: number | undefined): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    return DEFAULT_REFRESH_WINDOW_MS
  }

  return value
}

function none(): BridgeCredentialExpiryProjection {
  return {
    state: "none",
    msRemaining: null,
    shouldRefresh: false,
  }
}
