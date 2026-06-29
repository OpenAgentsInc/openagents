export type DeployHistoryEntry = {
  ref: string
  state: string
  at: string
  url: string | null
}

export type DeployHistoryView = {
  entries: DeployHistoryEntry[]
  latest: { ref: string; state: string } | null
  total: number
}

type RawRecord = Record<string, unknown>

const HISTORY_KEYS = [
  "deployHistory",
  "deploy_history",
  "deploymentHistory",
  "deployment_history",
  "deployments",
  "deploys",
  "history",
  "entries",
] as const

const REF_KEYS = [
  "ref",
  "id",
  "deployRef",
  "deploy_ref",
  "deploymentRef",
  "deployment_ref",
  "deploymentId",
  "deployment_id",
] as const

const STATE_KEYS = [
  "state",
  "status",
  "phase",
  "deployState",
  "deploy_state",
  "deploymentState",
  "deployment_state",
] as const

const AT_KEYS = [
  "at",
  "timestamp",
  "createdAt",
  "created_at",
  "deployedAt",
  "deployed_at",
  "completedAt",
  "completed_at",
  "finishedAt",
  "finished_at",
] as const

const URL_KEYS = [
  "url",
  "deployUrl",
  "deploy_url",
  "deploymentUrl",
  "deployment_url",
  "publicUrl",
  "public_url",
  "previewUrl",
  "preview_url",
] as const

export function projectDeployHistory(raw: unknown): DeployHistoryView {
  const entries = candidateEntries(raw)
    .map(projectEntry)
    .filter((entry): entry is DeployHistoryEntry => entry !== null)
    .sort(compareEntriesByAtDesc)

  return {
    entries,
    latest: entries[0] === undefined
      ? null
      : {
          ref: entries[0].ref,
          state: entries[0].state,
        },
    total: entries.length,
  }
}

function candidateEntries(raw: unknown): unknown[] {
  const entries: unknown[] = []

  appendEntries(entries, raw)

  for (const record of candidateRecords(raw)) {
    for (const key of HISTORY_KEYS) {
      appendEntries(entries, record[key])
    }
  }

  return entries
}

function candidateRecords(raw: unknown): RawRecord[] {
  if (!isRecord(raw)) return []

  const records: RawRecord[] = [raw]
  for (const key of [
    "deploy",
    "deployment",
    "deployStatus",
    "deploy_status",
    "result",
    "cloud",
    "session",
  ]) {
    appendRecord(records, raw[key])
  }

  const result = readRecord(raw, "result")
  appendRecord(records, result?.deploy)
  appendRecord(records, result?.deployment)
  appendRecord(records, result?.cloud)

  const cloud = readRecord(raw, "cloud") ?? readRecord(result, "cloud")
  appendRecord(records, cloud?.deploy)
  appendRecord(records, cloud?.deployment)

  return records
}

function appendEntries(entries: unknown[], value: unknown): void {
  if (!Array.isArray(value)) return
  for (const item of value) entries.push(item)
}

function appendRecord(records: RawRecord[], value: unknown): void {
  if (isRecord(value) && !records.includes(value)) records.push(value)
}

function projectEntry(value: unknown): DeployHistoryEntry | null {
  if (!isRecord(value)) return null

  const ref = firstStringFrom(value, REF_KEYS)
  const state = firstStringFrom(value, STATE_KEYS)
  const at = firstStringFrom(value, AT_KEYS)
  if (ref === null || state === null || at === null) return null

  return {
    ref,
    state,
    at,
    url: firstUrl(value),
  }
}

function compareEntriesByAtDesc(
  left: DeployHistoryEntry,
  right: DeployHistoryEntry,
): number {
  const leftMs = parseTimestampMs(left.at)
  const rightMs = parseTimestampMs(right.at)

  if (leftMs !== rightMs) return rightMs - leftMs
  return right.at.localeCompare(left.at)
}

function parseTimestampMs(value: string): number {
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : Number.NEGATIVE_INFINITY
}

function firstUrl(record: RawRecord): string | null {
  const value = firstStringFrom(record, URL_KEYS)
  return value !== null && /^https?:\/\/\S+$/i.test(value) ? value : null
}

function firstStringFrom(
  record: RawRecord,
  keys: readonly string[],
): string | null {
  for (const key of keys) {
    if (!hasOwn(record, key)) continue

    const value = record[key]
    if (typeof value === "string" && value.trim() !== "") return value.trim()
  }

  return null
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
