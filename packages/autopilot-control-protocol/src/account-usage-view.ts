type RawRecord = Record<string, unknown>

const PROVIDER_ALIASES = [
  "provider",
  "providerId",
  "provider_id",
  "accountProvider",
  "account_provider",
  "name",
  "id",
] as const

const USED_PERCENT_ALIASES = [
  "usedPercent",
  "used_percent",
  "percentUsed",
  "percent_used",
  "usagePercent",
  "usage_percent",
  "quotaPercent",
  "quota_percent",
] as const

const RESET_AT_ALIASES = [
  "resetAt",
  "reset_at",
  "quotaResetAt",
  "quota_reset_at",
  "usageResetAt",
  "usage_reset_at",
  "renewsAt",
  "renews_at",
] as const

const USED_ALIASES = [
  "used",
  "usage",
  "requestsUsed",
  "requests_used",
  "tokensUsed",
  "tokens_used",
  "unitsUsed",
  "units_used",
] as const

const LIMIT_ALIASES = [
  "limit",
  "quota",
  "cap",
  "requestsLimit",
  "requests_limit",
  "tokenLimit",
  "token_limit",
  "unitsLimit",
  "units_limit",
] as const

export function projectAccountUsage(raw: unknown): {
  provider: string
  usedPercent: number | null
  resetAt: string | null
  limited: boolean
  blockers: string[]
}[] {
  const rows = accountRows(raw)
  const projected: {
    provider: string
    usedPercent: number | null
    resetAt: string | null
    limited: boolean
    blockers: string[]
  }[] = []

  for (const row of rows) {
    if (!isRecord(row)) continue
    projected.push(projectRow(row))
  }

  return projected
}

function accountRows(raw: unknown): unknown[] {
  if (Array.isArray(raw)) return raw
  if (!isRecord(raw)) return []

  for (const key of ["accounts", "accountUsage", "account_usage", "usage"]) {
    const value = raw[key]
    if (Array.isArray(value)) return value
  }

  return []
}

function projectRow(row: RawRecord): {
  provider: string
  usedPercent: number | null
  resetAt: string | null
  limited: boolean
  blockers: string[]
} {
  const blockers = readBlockers(row)
  const provider = readProvider(row, blockers)
  const usedPercent = readUsedPercent(row, blockers)
  const resetAt = readResetAt(row, blockers)
  const limited = readLimited(row, usedPercent, blockers)

  return {
    provider,
    usedPercent,
    resetAt,
    limited,
    blockers: unique(blockers),
  }
}

function readProvider(row: RawRecord, blockers: string[]): string {
  const value = readFirst(row, PROVIDER_ALIASES)

  if (typeof value === "string" && value.trim() !== "") {
    return value.trim()
  }

  blockers.push("provider_unknown")
  return "unknown"
}

function readUsedPercent(row: RawRecord, blockers: string[]): number | null {
  const percentValue = readFirst(row, USED_PERCENT_ALIASES)
  if (percentValue !== undefined) {
    const parsed = parsePercent(percentValue)
    if (parsed !== null) return parsed
    blockers.push("used_percent_invalid")
    return null
  }

  const used = parseNonNegativeNumber(readFirst(row, USED_ALIASES))
  const limit = parseNonNegativeNumber(readFirst(row, LIMIT_ALIASES))

  if (used !== null && limit !== null) {
    if (limit <= 0) {
      blockers.push("usage_limit_invalid")
      return null
    }

    return Math.min(100, (used / limit) * 100)
  }

  blockers.push("used_percent_unknown")
  return null
}

function readResetAt(row: RawRecord, blockers: string[]): string | null {
  const value = readFirst(row, RESET_AT_ALIASES)
  if (value === undefined || value === null || value === "") return null

  if (typeof value === "string") {
    const trimmed = value.trim()
    if (trimmed !== "") return trimmed
  }

  blockers.push("reset_at_invalid")
  return null
}

function readLimited(
  row: RawRecord,
  usedPercent: number | null,
  blockers: string[],
): boolean {
  for (const key of [
    "limited",
    "isLimited",
    "is_limited",
    "quotaLimited",
    "quota_limited",
    "rateLimited",
    "rate_limited",
    "blocked",
  ]) {
    const value = row[key]
    if (typeof value === "boolean") return value
  }

  if (typeof row.status === "string") {
    const status = row.status.trim().toLowerCase()
    if (
      status === "limited" ||
      status === "rate_limited" ||
      status === "quota_limited" ||
      status === "blocked" ||
      status === "exhausted"
    ) {
      return true
    }
    if (status === "ok" || status === "ready" || status === "available") return false
  }

  if (usedPercent !== null) return usedPercent >= 100

  blockers.push("limited_status_unknown")
  return false
}

function readBlockers(row: RawRecord): string[] {
  const value = row.blockers
  if (!Array.isArray(value)) return []

  const blockers: string[] = []
  for (const blocker of value) {
    if (typeof blocker === "string" && blocker.trim() !== "") {
      blockers.push(blocker.trim())
    }
  }

  return blockers
}

function readFirst(row: RawRecord, keys: readonly string[]): unknown {
  for (const key of keys) {
    if (hasOwn(row, key)) return row[key]
  }
  return undefined
}

function parsePercent(value: unknown): number | null {
  const n = parseNonNegativeNumber(value)
  if (n === null) return null
  return Math.min(100, n)
}

function parseNonNegativeNumber(value: unknown): number | null {
  if (typeof value === "number") {
    return Number.isFinite(value) && value >= 0 ? value : null
  }

  if (typeof value === "string" && /^\d+(?:\.\d+)?$/.test(value.trim())) {
    const parsed = Number(value)
    return Number.isFinite(parsed) ? parsed : null
  }

  return null
}

function isRecord(value: unknown): value is RawRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function hasOwn(record: RawRecord, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(record, key)
}

function unique(values: readonly string[]): string[] {
  return Array.from(new Set(values))
}
