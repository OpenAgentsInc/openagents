export const OPENAGENTS_DESKTOP_ACCOUNT_POLL_INTERVAL_MS = 15_000

export type DesktopCodexAccountReadiness =
  | "ready"
  | "credentials_missing"
  | "credentials_revoked"
  | "disabled_by_config"
  | "platform_unsupported"
  | "rate_limited"
  | "sdk_missing"
  | "usage_limited"
  | "unknown"

export type DesktopCodexAccountWindow = {
  readonly label: string
  readonly resetAt: string | null
  readonly usedPercent: number | null
  readonly windowMinutes: number | null
}

export type DesktopCodexAccount = {
  readonly accountRef: string | null
  readonly accountRefHash: string
  readonly blockerRefs: readonly string[]
  readonly manualResetsRemaining: number | null
  readonly readiness: DesktopCodexAccountReadiness
  readonly resetAt: string | null
  readonly resetAvailable: boolean
  readonly resetSupported: boolean
  readonly totalTokens: number | null
  readonly usedPercent: number | null
  readonly windows: readonly DesktopCodexAccountWindow[]
}

export type CodexAccountStatusResult =
  | {
      readonly ok: true
      readonly accounts: readonly DesktopCodexAccount[]
      readonly observedAt: string
      readonly source: "local-pylon" | "openagents"
      readonly notice?: string
    }
  | {
      readonly ok: false
      readonly accounts: readonly DesktopCodexAccount[]
      readonly error: string
      readonly observedAt: string
      readonly source: "local-pylon" | "openagents"
    }

export type CodexAccountResetResult =
  | {
      readonly ok: true
      readonly account: DesktopCodexAccount | null
      readonly accounts: readonly DesktopCodexAccount[]
      readonly observedAt: string
    }
  | {
      readonly ok: false
      readonly error: string
      readonly observedAt: string
    }

const asRecord = (value: unknown): Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {}

const asArray = (value: unknown): readonly unknown[] =>
  Array.isArray(value) ? value : []

const stringValue = (value: unknown, fallback = ""): string =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : fallback

const nullableString = (value: unknown): string | null =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : null

const numberValue = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null

const resetAtFrom = (value: unknown): string | null => {
  if (typeof value === "string" && value.trim() !== "") return value.trim()
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) {
    return null
  }
  const millis = value < 10_000_000_000 ? value * 1000 : value
  const date = new Date(millis)
  return Number.isNaN(date.getTime()) ? null : date.toISOString()
}

const readinessFrom = (value: unknown): DesktopCodexAccountReadiness => {
  const raw = stringValue(asRecord(value).state ?? value).trim()
  switch (raw) {
    case "ready":
    case "credentials_missing":
    case "credentials_revoked":
    case "disabled_by_config":
    case "platform_unsupported":
    case "rate_limited":
    case "sdk_missing":
    case "usage_limited":
      return raw
    case "auth_error":
      return "credentials_revoked"
    default:
      return "unknown"
  }
}

const readinessForAccount = (
  account: Record<string, unknown>,
  limited: boolean,
): DesktopCodexAccountReadiness => {
  const readiness = readinessFrom(account.readiness ?? account.status)
  if (limited && (readiness === "ready" || readiness === "unknown")) {
    return "rate_limited"
  }
  return readiness
}

const windowFromLocal = (value: unknown): DesktopCodexAccountWindow | null => {
  const window = asRecord(value)
  if (Object.keys(window).length === 0) return null
  return {
    label: stringValue(window.label, "usage"),
    resetAt: nullableString(window.resetsAtIso) ?? resetAtFrom(window.reset_at),
    usedPercent: numberValue(window.usedPercent ?? window.used_percent),
    windowMinutes: numberValue(window.windowMinutes ?? window.window_minutes),
  }
}

const windowFromOperator = (
  label: string,
  usage: unknown,
  cap: unknown,
): DesktopCodexAccountWindow | null => {
  const used = numberValue(usage)
  const limit = numberValue(cap)
  if (used === null && limit === null) return null
  return {
    label,
    resetAt: null,
    usedPercent:
      used === null || limit === null || limit <= 0
        ? null
        : Math.round((used / limit) * 10_000) / 100,
    windowMinutes: label === "hourly" ? 60 : label === "weekly" ? 10_080 : null,
  }
}

const highestUsedPercent = (
  windows: readonly DesktopCodexAccountWindow[],
): number | null => {
  const values = windows
    .map(window => window.usedPercent)
    .filter((value): value is number => value !== null)
  return values.length === 0 ? null : Math.max(...values)
}

const localAccountFrom = (value: unknown): DesktopCodexAccount | null => {
  const account = asRecord(value)
  if (account.provider !== "codex") return null
  const quota = asRecord(account.quota)
  const capacity = asRecord(account.capacity)
  const usage = asRecord(account.usage)
  const manualReset = asRecord(account.manualReset)
  const windows = [
    windowFromLocal(capacity.hourly),
    windowFromLocal(capacity.weekly),
    ...asArray(capacity.windows).map(windowFromLocal),
  ].filter((window): window is DesktopCodexAccountWindow => window !== null)
  const uniqueWindows = windows.filter((window, index) =>
    windows.findIndex(candidate =>
      candidate.label === window.label &&
      candidate.windowMinutes === window.windowMinutes &&
      candidate.resetAt === window.resetAt &&
      candidate.usedPercent === window.usedPercent,
    ) === index,
  )
  const limited = quota.state === "limited"
  const manualResetsRemaining = numberValue(
    quota.manualResetsRemaining ?? manualReset.manualResetsRemaining,
  )
  const accountRef = nullableString(account.accountRef)
  return {
    accountRef,
    accountRefHash: stringValue(account.accountRefHash, "unknown-account"),
    blockerRefs: asArray(account.blockerRefs).filter(
      (item): item is string => typeof item === "string",
    ),
    manualResetsRemaining,
    readiness: readinessForAccount(account, limited),
    resetAt: nullableString(quota.cooldownExpiresAt),
    resetAvailable:
      accountRef !== null &&
      limited &&
      manualResetsRemaining !== null &&
      manualResetsRemaining > 0,
    resetSupported: accountRef !== null,
    totalTokens: numberValue(usage.totalTokens),
    usedPercent: highestUsedPercent(uniqueWindows),
    windows: uniqueWindows,
  }
}

const operatorAccountFrom = (value: unknown): DesktopCodexAccount | null => {
  const account = asRecord(value)
  if (account.provider !== "codex") return null
  const windows = [
    windowFromOperator("hourly", account.hourlyUsage, account.hourlyCap),
    windowFromOperator("weekly", account.weeklyUsage, account.weeklyCap),
  ].filter((window): window is DesktopCodexAccountWindow => window !== null)
  const limited = account.isRateLimited === true
  return {
    accountRef: nullableString(account.accountRef),
    accountRefHash: stringValue(account.accountRefHash, "unknown-account"),
    blockerRefs: [],
    manualResetsRemaining: numberValue(account.manualResetsRemaining),
    readiness: readinessForAccount(account, limited),
    resetAt: nullableString(account.cooldownExpiresAt),
    resetAvailable: false,
    resetSupported: false,
    totalTokens: null,
    usedPercent: highestUsedPercent(windows),
    windows,
  }
}

export const codexAccountsFromPylonStatus = (
  input: unknown,
): readonly DesktopCodexAccount[] =>
  asArray(asRecord(input).accounts)
    .map(localAccountFrom)
    .filter((account): account is DesktopCodexAccount => account !== null)

export const codexAccountsFromOperatorStatus = (
  input: unknown,
): readonly DesktopCodexAccount[] =>
  asArray(asRecord(input).accounts)
    .map(operatorAccountFrom)
    .filter((account): account is DesktopCodexAccount => account !== null)
