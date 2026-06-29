export type CloudSelection = "byo_key" | "credits"

export type CloudSessionState = "queued" | "running" | "completed" | "failed" | "cancelled"

export type CloudSession = {
  cloudSessionRef: string
  origin: "cloud"
  state: CloudSessionState
  region?: string
  costRef?: string
}

export type DeployCloudRequest = {
  type: "cloud.deploy"
  objective: string
  selection: CloudSelection
}

export type CloudListRequest = {
  type: "cloud.list"
}

export function buildDeployCloudRequest(input: {
  objective: string
  selection: CloudSelection
}): DeployCloudRequest {
  return {
    type: "cloud.deploy",
    objective: input.objective,
    selection: input.selection,
  }
}

export function buildCloudListRequest(): CloudListRequest {
  return { type: "cloud.list" }
}

const CLOUD_SESSION_STATES = new Set<CloudSessionState>([
  "queued",
  "running",
  "completed",
  "failed",
  "cancelled",
])

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function parseCloudSession(raw: unknown): CloudSession {
  if (!isRecord(raw)) throw new TypeError("Expected cloud session to be an object")
  if (typeof raw.cloudSessionRef !== "string") throw new TypeError("Expected cloudSessionRef to be a string")
  if (raw.origin !== "cloud") throw new TypeError("Expected cloud session origin to be cloud")
  if (typeof raw.state !== "string" || !CLOUD_SESSION_STATES.has(raw.state as CloudSessionState)) {
    throw new TypeError("Expected cloud session state to be valid")
  }
  if (raw.region !== undefined && typeof raw.region !== "string") {
    throw new TypeError("Expected cloud session region to be a string")
  }
  if (raw.costRef !== undefined && typeof raw.costRef !== "string") {
    throw new TypeError("Expected cloud session costRef to be a string")
  }

  return {
    cloudSessionRef: raw.cloudSessionRef,
    origin: "cloud",
    state: raw.state as CloudSessionState,
    ...(raw.region === undefined ? {} : { region: raw.region }),
    ...(raw.costRef === undefined ? {} : { costRef: raw.costRef }),
  }
}

export function parseCloudList(raw: unknown): CloudSession[] {
  if (!Array.isArray(raw)) throw new TypeError("Expected cloud session list response to be an array")
  return raw.map((row) => parseCloudSession(row))
}
