import type { Adapter, SessionState, SessionSummary } from "./control.js"

export type CloudCoordinatorSessionListRequestInput = {
  pairingRef: string
  capabilityRef: string
  clientRequestId: string
}

export type CloudCoordinatorSessionListRequest = {
  verb: "cloud.session.list"
  pairingRef: string
  capabilityRef: string
  clientRequestId: string
  idempotencyKey: string
}

export type CloudCoordinatorDispatchRequestInput = {
  objective: string
  clientRequestId: string
}

export type CloudCoordinatorDispatchRequest = {
  verb: "cloud.dispatch"
  objective: string
  clientRequestId: string
  idempotencyKey: string
}

const ADAPTERS = new Set<Adapter>(["codex", "claude_agent"])
const SESSION_STATES = new Set<SessionState>([
  "queued",
  "running",
  "completed",
  "failed",
  "cancelled",
])

export function buildCloudSessionListRequest(
  input: CloudCoordinatorSessionListRequestInput,
): CloudCoordinatorSessionListRequest {
  return {
    verb: "cloud.session.list",
    pairingRef: input.pairingRef,
    capabilityRef: input.capabilityRef,
    clientRequestId: input.clientRequestId,
    idempotencyKey: input.clientRequestId,
  }
}

export function buildCloudDispatchRequest(
  input: CloudCoordinatorDispatchRequestInput,
): CloudCoordinatorDispatchRequest {
  return {
    verb: "cloud.dispatch",
    objective: input.objective,
    clientRequestId: input.clientRequestId,
    idempotencyKey: input.clientRequestId,
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function expectOptionalString(
  row: Record<string, unknown>,
  key: keyof SessionSummary,
): string | undefined {
  const value = row[key]
  if (value !== undefined && typeof value !== "string") {
    throw new TypeError(`Expected cloud session ${String(key)} to be a string`)
  }
  return value
}

function expectOptionalBoolean(
  row: Record<string, unknown>,
  key: keyof SessionSummary,
): boolean | undefined {
  const value = row[key]
  if (value !== undefined && typeof value !== "boolean") {
    throw new TypeError(`Expected cloud session ${String(key)} to be a boolean`)
  }
  return value
}

function parseCloudSessionSummary(raw: unknown): SessionSummary {
  if (!isRecord(raw)) throw new TypeError("Expected cloud session to be an object")
  if (typeof raw.sessionRef !== "string") throw new TypeError("Expected cloud session sessionRef to be a string")
  if (typeof raw.adapter !== "string" || !ADAPTERS.has(raw.adapter as Adapter)) {
    throw new TypeError("Expected cloud session adapter to be valid")
  }
  if (typeof raw.state !== "string" || !SESSION_STATES.has(raw.state as SessionState)) {
    throw new TypeError("Expected cloud session state to be valid")
  }
  if (raw.accountRefHash !== null && typeof raw.accountRefHash !== "string") {
    throw new TypeError("Expected cloud session accountRefHash to be a string or null")
  }
  if (typeof raw.updatedAt !== "string") throw new TypeError("Expected cloud session updatedAt to be a string")

  const objectiveRef = expectOptionalString(raw, "objectiveRef")
  const workspaceRef = expectOptionalString(raw, "workspaceRef")
  const lastProgressRef = expectOptionalString(raw, "lastProgressRef")
  const latestActivity = expectOptionalString(raw, "latestActivity")
  const parentRef = expectOptionalString(raw, "parentRef")
  const agentKind = expectOptionalString(raw, "agentKind")
  const pylonManaged = expectOptionalBoolean(raw, "pylonManaged")

  return {
    sessionRef: raw.sessionRef,
    adapter: raw.adapter as Adapter,
    state: raw.state as SessionState,
    ...(objectiveRef === undefined ? {} : { objectiveRef }),
    ...(workspaceRef === undefined ? {} : { workspaceRef }),
    accountRefHash: raw.accountRefHash,
    ...(lastProgressRef === undefined ? {} : { lastProgressRef }),
    ...(latestActivity === undefined ? {} : { latestActivity }),
    ...(parentRef === undefined ? {} : { parentRef }),
    ...(agentKind === undefined ? {} : { agentKind }),
    ...(pylonManaged === undefined ? {} : { pylonManaged }),
    updatedAt: raw.updatedAt,
  }
}

export function parseCloudSessionList(raw: unknown): SessionSummary[] {
  if (!Array.isArray(raw)) throw new TypeError("Expected cloud coordinator session list response to be an array")
  return raw.map((row) => parseCloudSessionSummary(row))
}
