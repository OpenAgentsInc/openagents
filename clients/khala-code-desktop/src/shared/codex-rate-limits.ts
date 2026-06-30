export type KhalaCodexRateLimitWindow = {
  readonly usedPercent: number
  readonly remainingPercent: number
  readonly windowMinutes: number
  readonly resetsAtIso: string | null
  readonly resetDescription: string | null
}

export type KhalaCodexRateLimitResetCredits = {
  readonly availableCount: number
  readonly totalEarnedCount?: number
  readonly nextExpiresAtIso: string | null
  readonly credits?: readonly {
    readonly status: string
    readonly expiresAtIso: string | null
    readonly grantedAtIso: string | null
  }[]
}

export type KhalaCodexRateLimitProviderStatus = {
  readonly provider: "codex"
  readonly session: KhalaCodexRateLimitWindow | null
  readonly weekly: KhalaCodexRateLimitWindow | null
  readonly rateLimitResetCredits?: KhalaCodexRateLimitResetCredits | null
  readonly updatedAtIso: string
  readonly error: string | null
  readonly status: "ok" | "error" | "unavailable"
}

export type KhalaCodexRateLimitResetOutcome =
  | "reset"
  | "nothingToReset"
  | "noCredit"
  | "alreadyRedeemed"
