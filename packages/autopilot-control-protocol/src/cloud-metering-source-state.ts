export type CloudMeteringSourceReason =
  | "ok"
  | "no_feed"
  | "stale"
  | "malformed"

export type CloudMeteringSourceState = {
  available: boolean
  reason: CloudMeteringSourceReason
  observedAt: string | null
}

const FRESH_WINDOW_MS = 5 * 60 * 1000
const FUTURE_SKEW_MS = 60 * 1000

const FEED_KEYS = [
  "metering",
  "cloudMetering",
  "cloud_metering",
  "quota",
  "cloudQuota",
  "cloud_quota",
  "billing",
  "cost",
  "usage",
] as const

const USAGE_KEYS = [
  "usage",
  "used",
  "usageSats",
  "usage_sats",
  "usedSats",
  "used_sats",
  "spentSats",
  "spent_sats",
  "costSats",
  "cost_sats",
  "tokens",
  "tokensUsed",
  "tokens_used",
] as const

const OBSERVED_AT_KEYS = [
  "observedAt",
  "observed_at",
  "meteredAt",
  "metered_at",
  "updatedAt",
  "updated_at",
  "timestamp",
] as const

export function cloudMeteringState(raw: unknown): CloudMeteringSourceState {
  const records = candidateRecords(raw)
  if (records.length === 0) return unavailable("no_feed")

  const hasUsage = hasAny(records, USAGE_KEYS)
  const hasObservedAt = hasAny(records, OBSERVED_AT_KEYS)
  if (!hasUsage && !hasObservedAt) return unavailable("no_feed")

  const usage = readNumber(records, USAGE_KEYS)
  const observedAt = readObservedAt(records)

  if (usage === null || observedAt === null) {
    return unavailable("malformed")
  }

  if (!isFresh(observedAt.time)) {
    return {
      available: false,
      reason: "stale",
      observedAt: observedAt.iso,
    }
  }

  return {
    available: true,
    reason: "ok",
    observedAt: observedAt.iso,
  }
}

function unavailable(reason: "no_feed" | "malformed"): CloudMeteringSourceState {
  return {
    available: false,
    reason,
    observedAt: null,
  }
}

function candidateRecords(raw: unknown): Record<string, unknown>[] {
  if (!isPlainRecord(raw)) return []

  const records = [raw]
  for (const key of FEED_KEYS) {
    const value = raw[key]
    if (isPlainRecord(value)) records.push(value)
  }
  return records
}

function readNumber(
  records: readonly Record<string, unknown>[],
  keys: readonly string[],
): number | null {
  for (const record of records) {
    for (const key of keys) {
      if (!Object.hasOwn(record, key)) continue
      const value = record[key]
      return typeof value === "number" && Number.isFinite(value) ? value : null
    }
  }
  return null
}

function readObservedAt(
  records: readonly Record<string, unknown>[],
): { iso: string; time: number } | null {
  for (const record of records) {
    for (const key of OBSERVED_AT_KEYS) {
      if (!Object.hasOwn(record, key)) continue
      const value = record[key]
      if (typeof value !== "string" || value.trim() === "") return null

      const time = Date.parse(value)
      if (!Number.isFinite(time)) return null

      return {
        iso: new Date(time).toISOString(),
        time,
      }
    }
  }
  return null
}

function hasAny(
  records: readonly Record<string, unknown>[],
  keys: readonly string[],
): boolean {
  return records.some((record) => keys.some((key) => Object.hasOwn(record, key)))
}

function isFresh(time: number): boolean {
  const age = Date.now() - time
  return age >= -FUTURE_SKEW_MS && age <= FRESH_WINDOW_MS
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
