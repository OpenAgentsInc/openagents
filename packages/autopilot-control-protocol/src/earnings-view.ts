export type EarningsView = {
  balanceSats: number | null
  pendingSats: number | null
  lifetimeSats: number | null
  online: boolean
  blockers: string[]
}

type SatsField = "balanceSats" | "pendingSats" | "lifetimeSats"

const FIELD_ALIASES: Record<SatsField, readonly string[]> = {
  balanceSats: [
    "balanceSats",
    "balance_sats",
    "availableSats",
    "available_sats",
    "spendableSats",
    "spendable_sats",
  ],
  pendingSats: [
    "pendingSats",
    "pending_sats",
    "heldSats",
    "held_sats",
    "pendingPayoutSats",
    "pending_payout_sats",
  ],
  lifetimeSats: [
    "lifetimeSats",
    "lifetime_sats",
    "earnedSats",
    "earned_sats",
    "totalEarnedSats",
    "total_earned_sats",
  ],
}

const MSATS_FIELD_ALIASES: Record<SatsField, readonly string[]> = {
  balanceSats: [
    "balanceMsats",
    "balance_msats",
    "availableMsats",
    "available_msats",
    "spendableMsats",
    "spendable_msats",
  ],
  pendingSats: [
    "pendingMsats",
    "pending_msats",
    "heldMsats",
    "held_msats",
    "pendingPayoutMsats",
    "pending_payout_msats",
  ],
  lifetimeSats: [
    "lifetimeMsats",
    "lifetime_msats",
    "earnedMsats",
    "earned_msats",
    "totalEarnedMsats",
    "total_earned_msats",
  ],
}

export function projectEarnings(raw: unknown): EarningsView {
  const blockers: string[] = []
  const records = candidateRecords(raw)

  if (records.length === 0) {
    return {
      balanceSats: null,
      pendingSats: null,
      lifetimeSats: null,
      online: false,
      blockers: ["earnings_payload_unavailable"],
    }
  }

  appendSourceBlockers(blockers, records)

  const balanceSats = readSats("balanceSats", records, blockers)
  const pendingSats = readSats("pendingSats", records, blockers)
  const lifetimeSats = readSats("lifetimeSats", records, blockers)
  const online = readOnline(records, blockers)

  return {
    balanceSats,
    pendingSats,
    lifetimeSats,
    online,
    blockers: unique(blockers),
  }
}

export function formatSats(n: number): string {
  if (!Number.isFinite(n)) return "unknown sats"

  return `${Math.trunc(n).toLocaleString("en-US")} sats`
}

function candidateRecords(raw: unknown): Record<string, unknown>[] {
  if (!isPlainRecord(raw)) return []

  const records = [raw]
  for (const key of ["earnings", "wallet", "balance", "payments"]) {
    const value = raw[key]
    if (isPlainRecord(value)) records.push(value)
  }
  return records
}

function readSats(
  field: SatsField,
  records: readonly Record<string, unknown>[],
  blockers: string[],
): number | null {
  for (const record of records) {
    const satsValue = readFirst(record, FIELD_ALIASES[field])
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

function readOnline(
  records: readonly Record<string, unknown>[],
  blockers: string[],
): boolean {
  for (const record of records) {
    if (typeof record.online === "boolean") return record.online
    if (typeof record.connected === "boolean") return record.connected
    if (typeof record.status === "string") {
      const status = record.status.toLowerCase()
      if (status === "online" || status === "connected" || status === "ready") {
        return true
      }
      if (status === "offline" || status === "disconnected" || status === "down") {
        return false
      }
    }
  }

  blockers.push("online_status_unknown")
  return false
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

function toSnakeCase(field: SatsField): string {
  return field.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`)
}

function unique(values: readonly string[]): string[] {
  return Array.from(new Set(values))
}
