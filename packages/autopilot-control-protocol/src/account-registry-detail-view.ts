type RawRecord = Record<string, unknown>

export type AccountRegistryDetailView = {
  accounts: {
    provider: string
    identityLabel: string
    ready: boolean
    exhausted: boolean
    homeState: string
    capacity: {
      usedPct: number | null
      limited: boolean
    } | null
    blockerRefs: string[]
  }[]
  readyCount: number
  exhaustedCount: number
  total: number
}

const PROVIDER_ALIASES = [
  "provider",
  "providerId",
  "provider_id",
  "accountProvider",
  "account_provider",
] as const

const ACCOUNT_REF_ALIASES = [
  "accountRef",
  "account_ref",
  "ref",
  "id",
  "accountId",
  "account_id",
] as const

const EMAIL_ALIASES = ["email", "emailHash", "email_hash"] as const
const HOME_ALIASES = ["home", "homeState", "home_state", "homeRegion", "home_region"] as const
const READY_ALIASES = ["ready", "isReady", "is_ready", "available", "enabled"] as const
const BLOCKER_ALIASES = ["blockerRefs", "blocker_refs", "blockers", "reasons"] as const
const USED_PERCENT_ALIASES = [
  "usedPct",
  "used_pct",
  "usedPercent",
  "used_percent",
  "percentUsed",
  "percent_used",
  "usagePercent",
  "usage_percent",
  "quotaPercent",
  "quota_percent",
] as const
const LIMITED_ALIASES = [
  "limited",
  "isLimited",
  "is_limited",
  "quotaLimited",
  "quota_limited",
  "rateLimited",
  "rate_limited",
  "exhausted",
] as const

const EXHAUSTED_REF = /limit|exhaust|quota|rate/i

export function projectAccountRegistryDetail(raw: unknown): AccountRegistryDetailView {
  const accounts = accountRows(raw).flatMap((row) => {
    if (!isRecord(row)) return []
    return [projectRow(row)]
  })

  return {
    accounts,
    readyCount: accounts.filter((account) => account.ready).length,
    exhaustedCount: accounts.filter((account) => account.exhausted).length,
    total: accounts.length,
  }
}

function accountRows(raw: unknown): unknown[] {
  if (Array.isArray(raw)) return raw
  if (!isRecord(raw)) return []

  for (const key of ["accounts", "accountRegistry", "account_registry", "registry"]) {
    const value = raw[key]
    if (Array.isArray(value)) return value
  }

  return []
}

function projectRow(row: RawRecord): AccountRegistryDetailView["accounts"][number] {
  const provider = readStringAlias(row, PROVIDER_ALIASES) ?? "unknown"
  const homeState = readStringAlias(row, HOME_ALIASES) ?? "unknown"
  const blockerRefs = unique(readStringListAlias(row, BLOCKER_ALIASES))
  const capacity = readCapacity(row)
  const ready = readReady(row)
  const exhausted = blockerRefs.some((ref) => EXHAUSTED_REF.test(ref)) || capacity?.limited === true

  return {
    provider,
    identityLabel: readIdentityLabel(row, homeState),
    ready,
    exhausted,
    homeState,
    capacity,
    blockerRefs,
  }
}

function readIdentityLabel(row: RawRecord, homeState: string): string {
  return (
    readStringAlias(row, ACCOUNT_REF_ALIASES) ??
    readStringAlias(row, EMAIL_ALIASES) ??
    (homeState === "unknown" ? "unknown" : homeState)
  )
}

function readReady(row: RawRecord): boolean {
  const value = readFirst(row, READY_ALIASES)
  if (typeof value === "boolean") return value

  if (typeof row.status === "string") {
    const status = row.status.trim().toLowerCase()
    if (status === "ready" || status === "ok" || status === "available") return true
    if (
      status === "limited" ||
      status === "rate_limited" ||
      status === "quota_limited" ||
      status === "blocked" ||
      status === "exhausted" ||
      status === "disabled"
    ) {
      return false
    }
  }

  return false
}

function readCapacity(row: RawRecord): AccountRegistryDetailView["accounts"][number]["capacity"] {
  const capacityValue = row.capacity
  if (isRecord(capacityValue)) {
    return {
      usedPct: readUsedPct(capacityValue),
      limited: readLimited(capacityValue),
    }
  }

  const usedPct = readUsedPct(row)
  const limited = readLimited(row)
  if (usedPct === null && limited === false && !hasAny(row, [...USED_PERCENT_ALIASES, ...LIMITED_ALIASES])) {
    return null
  }

  return { usedPct, limited }
}

function readUsedPct(row: RawRecord): number | null {
  const value = readFirst(row, USED_PERCENT_ALIASES)
  if (value === undefined) return null
  return parsePercent(value)
}

function readLimited(row: RawRecord): boolean {
  const value = readFirst(row, LIMITED_ALIASES)
  if (typeof value === "boolean") return value

  if (typeof row.status === "string") {
    const status = row.status.trim().toLowerCase()
    return (
      status === "limited" ||
      status === "rate_limited" ||
      status === "quota_limited" ||
      status === "blocked" ||
      status === "exhausted"
    )
  }

  const usedPct = readUsedPct(row)
  return usedPct !== null && usedPct >= 100
}

function readStringAlias(row: RawRecord, keys: readonly string[]): string | null {
  const value = readFirst(row, keys)
  if (typeof value !== "string") return null

  const trimmed = value.trim()
  return trimmed === "" ? null : trimmed
}

function readStringListAlias(row: RawRecord, keys: readonly string[]): string[] {
  const value = readFirst(row, keys)
  if (!Array.isArray(value)) return []

  const strings: string[] = []
  for (const item of value) {
    if (typeof item !== "string") continue
    const trimmed = item.trim()
    if (trimmed !== "") strings.push(trimmed)
  }
  return strings
}

function readFirst(row: RawRecord, keys: readonly string[]): unknown {
  for (const key of keys) {
    if (hasOwn(row, key)) return row[key]
  }
  return undefined
}

function parsePercent(value: unknown): number | null {
  if (typeof value === "number") {
    return Number.isFinite(value) && value >= 0 ? Math.min(100, value) : null
  }

  if (typeof value === "string" && /^\d+(?:\.\d+)?$/.test(value.trim())) {
    const parsed = Number(value)
    return Number.isFinite(parsed) ? Math.min(100, parsed) : null
  }

  return null
}

function hasAny(row: RawRecord, keys: readonly string[]): boolean {
  return keys.some((key) => hasOwn(row, key))
}

function hasOwn(record: RawRecord, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(record, key)
}

function isRecord(value: unknown): value is RawRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function unique(values: readonly string[]): string[] {
  return Array.from(new Set(values))
}
