export type IntentHistoryStep = {
  status: string
  observedAt: string
}

export type IntentHistoryView = {
  intentId: string
  status: string
  steps: IntentHistoryStep[]
  durationMs: number | null
}

type RawRecord = Record<string, unknown>

export function projectIntentHistory(raw: unknown): IntentHistoryView {
  const records = candidateRecords(raw)
  const steps = firstStatusHistory(records)
  const status = firstStringFrom(records, ["status", "state"]) ?? lastStepStatus(steps) ?? "unknown"

  return {
    intentId: firstStringFrom(records, [
      "intentId",
      "intent_id",
      "id",
      "ref",
    ]) ?? "",
    status,
    steps,
    durationMs: durationFromSteps(steps),
  }
}

function candidateRecords(raw: unknown): RawRecord[] {
  if (!isRecord(raw)) return []

  const records: RawRecord[] = [raw]
  appendRecord(records, raw.intent)
  appendRecord(records, raw.projection)
  appendRecord(records, raw.result)
  appendRecord(records, raw.summary)

  const result = readRecord(raw, "result")
  appendRecord(records, result?.intent)
  appendRecord(records, result?.projection)
  appendRecord(records, result?.summary)

  const projection = readRecord(raw, "projection") ?? readRecord(result, "projection")
  appendRecord(records, projection?.intent)
  appendRecord(records, projection?.summary)

  return records
}

function appendRecord(records: RawRecord[], value: unknown): void {
  if (isRecord(value) && !records.includes(value)) records.push(value)
}

function firstStatusHistory(records: readonly RawRecord[]): IntentHistoryStep[] {
  for (const record of records) {
    const history = readFirst(record, [
      "statusHistory",
      "status_history",
      "history",
    ])
    const steps = statusHistoryFrom(history)
    if (steps.length > 0) return steps
  }

  return []
}

function statusHistoryFrom(value: unknown): IntentHistoryStep[] {
  if (!Array.isArray(value)) return []

  const steps: IntentHistoryStep[] = []

  for (const item of value) {
    if (!isRecord(item)) continue

    const status = firstString(item.status, item.state)
    const observedAt = firstString(
      item.observedAt,
      item.observed_at,
      item.at,
      item.timestamp,
      item.createdAt,
      item.created_at,
    )

    if (status === undefined || observedAt === undefined) continue

    steps.push({ status, observedAt })
  }

  return steps
}

function durationFromSteps(steps: readonly IntentHistoryStep[]): number | null {
  if (steps.length < 2) return null

  const first = parseTimestampMs(steps[0]?.observedAt)
  const last = parseTimestampMs(steps[steps.length - 1]?.observedAt)
  if (first === null || last === null || last < first) return null

  return last - first
}

function parseTimestampMs(value: unknown): number | null {
  if (typeof value !== "string" || value.trim() === "") return null

  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : null
}

function lastStepStatus(steps: readonly IntentHistoryStep[]): string | undefined {
  return steps[steps.length - 1]?.status
}

function firstStringFrom(
  records: readonly RawRecord[],
  keys: readonly string[],
): string | undefined {
  for (const record of records) {
    const value = readFirst(record, keys)
    const parsed = readString(value)
    if (parsed !== undefined) return parsed
  }

  return undefined
}

function firstString(...values: unknown[]): string | undefined {
  for (const value of values) {
    const parsed = readString(value)
    if (parsed !== undefined) return parsed
  }

  return undefined
}

function readString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() !== "" ? value.trim() : undefined
}

function readFirst(record: RawRecord, keys: readonly string[]): unknown {
  for (const key of keys) {
    if (hasOwn(record, key)) return record[key]
  }

  return undefined
}

function readRecord(value: unknown, key: string): RawRecord | undefined {
  if (!isRecord(value)) return undefined
  const nested = value[key]
  return isRecord(nested) ? nested : undefined
}

function isRecord(value: unknown): value is RawRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function hasOwn(record: RawRecord, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(record, key)
}
