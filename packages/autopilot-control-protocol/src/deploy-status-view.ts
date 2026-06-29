export type DeployStatusState =
  | "queued"
  | "building"
  | "deployed"
  | "failed"
  | "unknown"

export type DeployStatusView = {
  state: DeployStatusState
  url: string | null
  deployedAt: string | null
  message: string
}

type RawRecord = Record<string, unknown>

const STATE_KEYS = [
  "deployState",
  "deploy_state",
  "deploymentState",
  "deployment_state",
  "state",
  "status",
  "phase",
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

const DEPLOYED_AT_KEYS = [
  "deployedAt",
  "deployed_at",
  "completedAt",
  "completed_at",
  "finishedAt",
  "finished_at",
  "liveAt",
  "live_at",
] as const

const MESSAGE_KEYS = [
  "message",
  "statusText",
  "status_text",
  "detail",
  "reason",
  "error",
] as const

export function projectDeployStatus(raw: unknown): DeployStatusView {
  const records = candidateRecords(raw)

  if (records.length === 0) {
    return {
      state: "unknown",
      url: null,
      deployedAt: null,
      message: defaultMessage("unknown", false),
    }
  }

  const state = firstDeployState(records)

  return {
    state,
    url: firstUrl(records),
    deployedAt: firstStringFrom(records, DEPLOYED_AT_KEYS),
    message: firstMessage(records) ?? defaultMessage(state, true),
  }
}

function candidateRecords(raw: unknown): RawRecord[] {
  if (!isRecord(raw)) return []

  const records: RawRecord[] = [raw]
  for (const key of [
    "deploy",
    "deployment",
    "deployStatus",
    "deploy_status",
    "status",
    "result",
    "cloud",
    "session",
  ]) {
    appendRecord(records, raw[key])
  }

  const result = readRecord(raw, "result")
  appendRecord(records, result?.deploy)
  appendRecord(records, result?.deployment)
  appendRecord(records, result?.status)

  const cloud = readRecord(raw, "cloud")
  appendRecord(records, cloud?.deploy)
  appendRecord(records, cloud?.deployment)
  appendRecord(records, cloud?.status)

  return records
}

function appendRecord(records: RawRecord[], value: unknown): void {
  if (isRecord(value) && !records.includes(value)) records.push(value)
}

function firstDeployState(records: readonly RawRecord[]): DeployStatusState {
  for (const record of records) {
    const parsed = parseDeployState(readFirst(record, STATE_KEYS))
    if (parsed !== "unknown") return parsed
  }

  return "unknown"
}

function parseDeployState(value: unknown): DeployStatusState {
  if (typeof value !== "string") return "unknown"

  switch (value.trim().toLowerCase()) {
    case "queued":
    case "queue":
    case "pending":
    case "scheduled":
    case "created":
    case "accepted":
      return "queued"

    case "building":
    case "build":
    case "running":
    case "in_progress":
    case "in-progress":
    case "deploying":
    case "uploading":
    case "provisioning":
      return "building"

    case "deployed":
    case "complete":
    case "completed":
    case "success":
    case "succeeded":
    case "ready":
    case "live":
      return "deployed"

    case "failed":
    case "failure":
    case "error":
    case "errored":
    case "cancelled":
    case "canceled":
      return "failed"

    default:
      return "unknown"
  }
}

function firstUrl(records: readonly RawRecord[]): string | null {
  for (const record of records) {
    const value = firstStringFrom([record], URL_KEYS)
    if (value !== null && /^https?:\/\/\S+$/i.test(value)) return value
  }

  return null
}

function firstMessage(records: readonly RawRecord[]): string | null {
  for (const record of records) {
    const direct = firstStringFrom([record], MESSAGE_KEYS)
    if (direct !== null) return direct

    const error = readRecord(record, "error")
    const errorMessage = firstStringFrom(error === undefined ? [] : [error], [
      "message",
      "detail",
      "reason",
    ])
    if (errorMessage !== null) return errorMessage
  }

  return null
}

function firstStringFrom(
  records: readonly RawRecord[],
  keys: readonly string[],
): string | null {
  for (const record of records) {
    const value = readFirst(record, keys)
    if (typeof value === "string" && value.trim() !== "") return value.trim()
  }

  return null
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

function defaultMessage(state: DeployStatusState, hadPayload: boolean): string {
  switch (state) {
    case "queued":
      return "Deployment queued"
    case "building":
      return "Deployment building"
    case "deployed":
      return "Deployment deployed"
    case "failed":
      return "Deployment failed"
    case "unknown":
      return hadPayload ? "Deployment status unknown" : "Deployment status unavailable"
  }
}
