/**
 * Framed stdio protocol between Omega Rust supervisor and omega-effectd.
 *
 * Schema: `openagents.omega.effectd.v1`
 * Transport: one JSON object per line on stdin/stdout.
 * Generation fencing: every request carries the supervisor generation;
 * stale generations are refused.
 */

export const OMEGA_EFFECTD_PROTOCOL_SCHEMA = "openagents.omega.effectd.v1" as const
export const OMEGA_EFFECTD_SERVICE_VERSION = "0.1.0" as const
export const OMEGA_EFFECTD_PROTOCOL_VERSION = 1 as const

export type OmegaEffectdProtocolErrorCode =
  | "stale_generation"
  | "unknown_method"
  | "invalid_request"
  | "not_running"
  | "run_not_found"
  | "internal"

export type OmegaEffectdProtocolError = Readonly<{
  code: OmegaEffectdProtocolErrorCode
  message: string
}>

export type OmegaEffectdInitializeParams = Readonly<{
  generation: number
  client?: string
}>

export type OmegaEffectdInitializeResult = Readonly<{
  schema: typeof OMEGA_EFFECTD_PROTOCOL_SCHEMA
  protocolVersion: typeof OMEGA_EFFECTD_PROTOCOL_VERSION
  serviceVersion: typeof OMEGA_EFFECTD_SERVICE_VERSION
  generation: number
  capabilities: ReadonlyArray<
    | "health"
    | "list_runs"
    | "get_run"
    | "start"
    | "pause"
    | "resume"
    | "stop"
    | "retry"
  >
  dataRoot: string
  activeRunLimit: number
}>

/** Redacted durable run projection for list/monitor (no objective/transcript). */
export type OmegaEffectdRunSnapshot = Readonly<{
  runRef: string
  threadRef: string | null
  state: string
  title: string
  updatedAt: string
}>

/** Owner-local run detail for the GPUI launcher/monitor (FA-03). */
export type OmegaEffectdRunDetail = Readonly<{
  runRef: string
  threadRef: string | null
  state: string
  title: string
  objective: string
  doneCondition: string
  workspaceRef: string | null
  lane: string | null
  turnCap: number
  successfulAttempts: number
  failedAttempts: number
  stallCause: string | null
  recoveryAction: string
  terminalReason: string | null
  updatedAt: string
  turns: ReadonlyArray<{
    turnRef: string
    lane: string
    outcomeSummary: string
    createdAt: string
  }>
}>

export type OmegaEffectdStartParams = Readonly<{
  workspaceRef: string
  title: string
  objective: string
  doneCondition: string
  lane?: string
  model?: string
  turnCap?: number
  autonomy?: boolean
}>

export type OmegaEffectdHealthResult = Readonly<{
  ok: true
  status: "running" | "stopped"
  generation: number
  dataRoot: string
  activeRunCount: number
}>

export type OmegaEffectdRequest = Readonly<{
  schema: typeof OMEGA_EFFECTD_PROTOCOL_SCHEMA
  kind: "request"
  id: string
  generation: number
  method: string
  params?: unknown
}>

export type OmegaEffectdResponse = Readonly<{
  schema: typeof OMEGA_EFFECTD_PROTOCOL_SCHEMA
  kind: "response"
  id: string
  generation: number
  ok: boolean
  result?: unknown
  error?: OmegaEffectdProtocolError
}>

export type OmegaEffectdEvent = Readonly<{
  schema: typeof OMEGA_EFFECTD_PROTOCOL_SCHEMA
  kind: "event"
  generation: number
  name: string
  payload?: unknown
}>

export type OmegaEffectdFrame = OmegaEffectdRequest | OmegaEffectdResponse | OmegaEffectdEvent

export const isOmegaEffectdRequest = (value: unknown): value is OmegaEffectdRequest => {
  if (value === null || typeof value !== "object") return false
  const frame = value as Record<string, unknown>
  return (
    frame.schema === OMEGA_EFFECTD_PROTOCOL_SCHEMA &&
    frame.kind === "request" &&
    typeof frame.id === "string" &&
    typeof frame.generation === "number" &&
    typeof frame.method === "string"
  )
}

export const redactDiagnosticText = (text: string): string =>
  text
    .replace(/Bearer\s+[A-Za-z0-9._\-]+/gi, "Bearer [redacted]")
    .replace(/\/Users\/[^/\s]+/g, "/Users/[redacted]")
    .replace(/\/home\/[^/\s]+/g, "/home/[redacted]")
    .replace(/[A-Za-z0-9+/]{32,}={0,2}/g, "[redacted-token]")
