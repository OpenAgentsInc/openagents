export type CloudQuotaFailoverState = "primary" | "failover" | "unknown"

export type CloudQuotaView = {
  usedSats: number | null
  capSats: number | null
  remainingSats: number | null
  percentUsed: number | null
  failoverState: CloudQuotaFailoverState
  blockers: string[]
}

type QuotaField = "usedSats" | "capSats"

const SATS_FIELD_ALIASES: Record<QuotaField, readonly string[]> = {
  usedSats: [
    "usedSats",
    "used_sats",
    "spentSats",
    "spent_sats",
    "costSats",
    "cost_sats",
    "usageSats",
    "usage_sats",
  ],
  capSats: [
    "capSats",
    "cap_sats",
    "limitSats",
    "limit_sats",
    "budgetSats",
    "budget_sats",
    "quotaSats",
    "quota_sats",
  ],
}

const MSATS_FIELD_ALIASES: Record<QuotaField, readonly string[]> = {
  usedSats: [
    "usedMsats",
    "used_msats",
    "spentMsats",
    "spent_msats",
    "costMsats",
    "cost_msats",
    "usageMsats",
    "usage_msats",
  ],
  capSats: [
    "capMsats",
    "cap_msats",
    "limitMsats",
    "limit_msats",
    "budgetMsats",
    "budget_msats",
    "quotaMsats",
    "quota_msats",
  ],
}

export function computeCloudQuota(
  usedSats: number | null,
  capSats: number | null,
): Pick<CloudQuotaView, "remainingSats" | "percentUsed"> {
  if (usedSats === null || capSats === null || capSats <= 0) {
    return {
      remainingSats: null,
      percentUsed: null,
    }
  }

  return {
    remainingSats: Math.max(0, capSats - usedSats),
    percentUsed: Math.min(100, (usedSats / capSats) * 100),
  }
}

export function projectCloudQuota(raw: unknown): CloudQuotaView {
  const blockers: string[] = []
  const records = candidateRecords(raw)

  if (records.length === 0) {
    return {
      usedSats: null,
      capSats: null,
      remainingSats: null,
      percentUsed: null,
      failoverState: "unknown",
      blockers: ["cloud_quota_payload_unavailable"],
    }
  }

  appendSourceBlockers(blockers, records)

  const usedSats = readSats("usedSats", records, blockers)
  const capSats = readSats("capSats", records, blockers)
  const computed = computeCloudQuota(usedSats, capSats)
  if (usedSats !== null && capSats !== null && capSats <= 0) {
    blockers.push("cap_sats_invalid")
  }

  const failoverState = readFailoverState(records, blockers)

  return {
    usedSats,
    capSats,
    remainingSats: computed.remainingSats,
    percentUsed: computed.percentUsed,
    failoverState,
    blockers: unique(blockers),
  }
}

function candidateRecords(raw: unknown): Record<string, unknown>[] {
  if (!isPlainRecord(raw)) return []

  const records = [raw]
  for (const key of ["quota", "cloudQuota", "cloud_quota", "billing", "cost", "usage"]) {
    const value = raw[key]
    if (isPlainRecord(value)) records.push(value)
  }
  return records
}

function readSats(
  field: QuotaField,
  records: readonly Record<string, unknown>[],
  blockers: string[],
): number | null {
  for (const record of records) {
    const satsValue = readFirst(record, SATS_FIELD_ALIASES[field])
    if (satsValue !== undefined) {
      const parsed = parseSats(satsValue)
      if (parsed !== null) return parsed
      blockers.push(`${toSnakeCase(field)}_invalid`)
      return null
    }

    const msatsValue = readFirst(record, MSATS_FIELD_ALIASES[field])
    if (msatsValue !== undefined) {
      const parsed = parseMsats(msatsValue)
      if (parsed !== null) return parsed
      blockers.push(`${toSnakeCase(field)}_invalid`)
      return null
    }
  }

  blockers.push(`${toSnakeCase(field)}_unknown`)
  return null
}

function readFailoverState(
  records: readonly Record<string, unknown>[],
  blockers: string[],
): CloudQuotaFailoverState {
  for (const record of records) {
    const value = readFirst(record, [
      "failoverState",
      "failover_state",
      "state",
      "mode",
      "providerState",
      "provider_state",
    ])
    const parsed = parseFailoverState(value)
    if (parsed !== "unknown") return parsed
  }

  blockers.push("failover_state_unknown")
  return "unknown"
}

function parseFailoverState(value: unknown): CloudQuotaFailoverState {
  if (typeof value === "boolean") return value ? "failover" : "primary"
  if (typeof value !== "string") return "unknown"

  switch (value.trim().toLowerCase()) {
    case "primary":
    case "normal":
    case "ok":
    case "healthy":
      return "primary"
    case "failover":
    case "fallback":
    case "degraded":
    case "secondary":
      return "failover"
    default:
      return "unknown"
  }
}

function appendSourceBlockers(
  blockers: string[],
  records: readonly Record<string, unknown>[],
): void {
  for (const record of records) {
    if (!Array.isArray(record.blockers)) continue

    for (const blocker of record.blockers) {
      if (typeof blocker === "string" && blocker.trim() !== "") {
        blockers.push(blocker.trim())
      }
    }
  }
}

function readFirst(
  record: Record<string, unknown>,
  keys: readonly string[],
): unknown {
  for (const key of keys) {
    if (Object.hasOwn(record, key)) return record[key]
  }
  return undefined
}

function parseSats(value: unknown): number | null {
  const n = parseFiniteInteger(value)
  if (n === null || n < 0 || !Number.isSafeInteger(n)) return null
  return n
}

function parseMsats(value: unknown): number | null {
  const n = parseFiniteInteger(value)
  if (n === null || n < 0 || !Number.isSafeInteger(n)) return null
  return Math.floor(n / 1000)
}

function parseFiniteInteger(value: unknown): number | null {
  if (typeof value === "number") {
    return Number.isFinite(value) && Number.isInteger(value) ? value : null
  }

  if (typeof value === "string" && /^\d+$/.test(value.trim())) {
    return Number(value)
  }

  return null
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function toSnakeCase(field: QuotaField): string {
  return field.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`)
}

function unique(values: readonly string[]): string[] {
  return Array.from(new Set(values))
}
