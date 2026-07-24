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
    | "get_capacity"
    | "decide_attention"
    | "get_report"
    | "get_receipt"
    | "apply_control_intent"
    | "get_sync_status"
    | "publish_projection"
    | "get_native_binding"
    | "assess_native_boundary"
  >
  dataRoot: string
  activeRunLimit: number
}>

/** FA-06: native project/worktree join (public-safe digests only). */
export type OmegaEffectdNativeEvidence = Readonly<{
  projectRef: string
  worktreeRef: string
  worktreePathDigest: string | null
  gitHead: string | null
}>

export type OmegaEffectdNativeBinding = Readonly<{
  runRef: string
  workspaceRef: string
  projectRef: string
  worktreeRef: string
  worktreePathDigest: string | null
  gitHead: string | null
  rebaseUnsafe: boolean
  boundAt: string
}>

/** FA-05: honest Sync availability (Omega OA-04 session may be absent). */
export type OmegaEffectdSyncStatus = Readonly<{
  available: boolean
  publishBlocksDispatch: false
  reason: string
}>

export type OmegaEffectdPublishProjectionResult = Readonly<{
  ok: boolean
  status: "published" | "sync_unavailable" | "run_not_found"
  reason?: string
}>
/** Per-lane capacity ledger (FA-04). Public-safe: lane refs and typed states only. */
export type OmegaEffectdLaneCapacity = Readonly<{
  lane: string
  state: "available" | "busy" | "cooling" | "exhausted" | "unavailable"
  activeRuns: number
  reason: string
}>

export type OmegaEffectdCapacityResult = Readonly<{
  activeRunLimit: number
  activeRunCount: number
  lanes: ReadonlyArray<OmegaEffectdLaneCapacity>
  nonOverridableGuardrails: ReadonlyArray<string>
  ownerConfigurableGuardrails: ReadonlyArray<string>
  enabledThreadsNeverEvicted: true
}>

/** Redacted attention decision (FA-04). Never carries objective/transcript. */
export type OmegaEffectdAttentionResult = Readonly<{
  notify: boolean
  dedupKey: string
  title: string
  body: string
} | null>

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
  nativeEvidence: OmegaEffectdNativeEvidence | null
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
  projectRef?: string
  worktreeRef?: string
  worktreeAbsolutePath?: string
  gitHead?: string
  rebaseUnsafe?: boolean
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
