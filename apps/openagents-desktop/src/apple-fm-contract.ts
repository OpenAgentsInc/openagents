/**
 * Apple Foundation Models local-mode IPC contract (AFM-6, #9075).
 *
 * The Electron main process owns the Swift `foundation-bridge` sidecar
 * lifecycle and the in-process Pylon FM runtime authority. This file owns the
 * ONLY renderer-visible surface: three additive, schema-validated IPC channels
 * (status, start-turn, stop) whose payloads are bounded and public-safe.
 *
 * Security posture (§8.2 of the desktop plan): the renderer is never part of
 * the trusted boundary. It NEVER learns the raw bridge path, the loopback base
 * URL, a callback URL or token, a local workspace path, tool arguments, file
 * contents, or a raw transcript. It renders only: the supervisor state, a
 * health-derived readiness status, the selected local mode, bounded blocker
 * refs, honest usage truth, and — for one bounded read-only turn — a
 * length-capped assistant reply and token counts. The request carries a single
 * bounded prompt field; nothing beyond that crosses the line.
 */
import { Schema } from "effect"

/** Additive IPC channels (main ↔ renderer). Public-safe payloads only. */
export const AppleFmStatusChannel = "openagents-desktop/apple-fm-status" as const
export const AppleFmStartTurnChannel = "openagents-desktop/apple-fm-start-turn" as const
export const AppleFmStopChannel = "openagents-desktop/apple-fm-stop" as const

export const APPLE_FM_STATUS_SCHEMA_ID = "openagents.desktop.apple_fm.status.v1" as const
export const APPLE_FM_TURN_SCHEMA_ID = "openagents.desktop.apple_fm.turn.v1" as const
export const APPLE_FM_STOP_SCHEMA_ID = "openagents.desktop.apple_fm.stop.v1" as const

// ---------------------------------------------------------------------------
// Bounded vocabularies (never free text).
// ---------------------------------------------------------------------------

/** Typed supervisor lifecycle states owned by `apple-fm-host.ts`. */
export const appleFmHostStateValues = [
  "not_supported",
  "candidate",
  "helper_missing",
  "launching",
  "adopted",
  "running",
  "ready",
  "unavailable",
  "failed",
  "stopped",
] as const
export type AppleFmHostStateValue = (typeof appleFmHostStateValues)[number]

/** Health-derived readiness status (mirrors the Pylon FM client). */
export const appleFmReadinessValues = ["ready", "unavailable", "unsupported", "malformed", "unreachable"] as const
export type AppleFmReadinessValue = (typeof appleFmReadinessValues)[number]

/** Selected local execution mode. `none` means the sidecar is not owned/adopted. */
export const appleFmModeValues = ["local_launched", "local_adopted", "none"] as const
export type AppleFmModeValue = (typeof appleFmModeValues)[number]

/** Usage truth — honest by construction; the current bridge only estimates. */
export const appleFmUsageTruthValues = ["exact", "estimated", "unknown"] as const
export type AppleFmUsageTruthValue = (typeof appleFmUsageTruthValues)[number]

/** Turn outcome projected to the renderer. */
export const appleFmTurnOutcomeValues = ["completed", "refused_not_ready", "refused_unsupported", "failed"] as const
export type AppleFmTurnOutcomeValue = (typeof appleFmTurnOutcomeValues)[number]

/** A bounded lower-snake token (reason/failure-class/blocker), never free text. */
const BoundedToken = Schema.String.check(Schema.isMaxLength(120), Schema.isPattern(/^[a-z0-9_.]+$/))
/** A bounded model/profile identifier (public-safe). */
const BoundedRef = Schema.String.check(Schema.isMaxLength(120), Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._-]*$/))

// ---------------------------------------------------------------------------
// Status projection (get readiness/status).
// ---------------------------------------------------------------------------

export const AppleFmStatusSchema = Schema.Struct({
  schema: Schema.Literal(APPLE_FM_STATUS_SCHEMA_ID),
  supported: Schema.Boolean,
  state: Schema.Literals(appleFmHostStateValues),
  readiness: Schema.Literals(appleFmReadinessValues),
  ready: Schema.Boolean,
  mode: Schema.Literals(appleFmModeValues),
  model: Schema.NullOr(BoundedRef),
  profileId: Schema.NullOr(BoundedRef),
  usageTruth: Schema.Literals(appleFmUsageTruthValues),
  unavailableReason: Schema.NullOr(BoundedToken),
  blockerRefs: Schema.Array(BoundedToken).check(Schema.isMaxLength(8)),
})
export type AppleFmStatus = typeof AppleFmStatusSchema.Type

// ---------------------------------------------------------------------------
// Start turn (bounded read-only inference turn).
// ---------------------------------------------------------------------------

/** The single bounded prompt field. Nothing else crosses to main. */
export const AppleFmStartTurnRequestSchema = Schema.Struct({
  prompt: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(4000)),
})
export type AppleFmStartTurnRequest = typeof AppleFmStartTurnRequestSchema.Type

export const AppleFmTurnResultSchema = Schema.Struct({
  schema: Schema.Literal(APPLE_FM_TURN_SCHEMA_ID),
  ok: Schema.Boolean,
  outcome: Schema.Literals(appleFmTurnOutcomeValues),
  /** Length-capped assistant reply; never a raw transcript or tool output. */
  text: Schema.NullOr(Schema.String.check(Schema.isMaxLength(8192))),
  usageTruth: Schema.Literals(appleFmUsageTruthValues),
  promptTokens: Schema.NullOr(Schema.Number),
  completionTokens: Schema.NullOr(Schema.Number),
  totalTokens: Schema.NullOr(Schema.Number),
  failureClass: Schema.NullOr(BoundedToken),
})
export type AppleFmTurnResult = typeof AppleFmTurnResultSchema.Type

// ---------------------------------------------------------------------------
// Stop.
// ---------------------------------------------------------------------------

export const AppleFmStopResultSchema = Schema.Struct({
  schema: Schema.Literal(APPLE_FM_STOP_SCHEMA_ID),
  stopped: Schema.Boolean,
  state: Schema.Literals(appleFmHostStateValues),
})
export type AppleFmStopResult = typeof AppleFmStopResultSchema.Type

// ---------------------------------------------------------------------------
// Decoders (return null on invalid — the boundary never throws).
// ---------------------------------------------------------------------------

const decodeStatusExit = Schema.decodeUnknownExit(AppleFmStatusSchema)
const decodeStartTurnExit = Schema.decodeUnknownExit(AppleFmStartTurnRequestSchema)
const decodeTurnResultExit = Schema.decodeUnknownExit(AppleFmTurnResultSchema)
const decodeStopExit = Schema.decodeUnknownExit(AppleFmStopResultSchema)

export const decodeAppleFmStatus = (value: unknown): AppleFmStatus | null => {
  const decoded = decodeStatusExit(value)
  return decoded._tag === "Success" ? decoded.value : null
}

export const decodeAppleFmStartTurnRequest = (value: unknown): AppleFmStartTurnRequest | null => {
  const decoded = decodeStartTurnExit(value)
  return decoded._tag === "Success" ? decoded.value : null
}

export const decodeAppleFmTurnResult = (value: unknown): AppleFmTurnResult | null => {
  const decoded = decodeTurnResultExit(value)
  return decoded._tag === "Success" ? decoded.value : null
}

export const decodeAppleFmStopResult = (value: unknown): AppleFmStopResult | null => {
  const decoded = decodeStopExit(value)
  return decoded._tag === "Success" ? decoded.value : null
}

// ---------------------------------------------------------------------------
// Public-safe constructors used at the boundary fallbacks.
// ---------------------------------------------------------------------------

/** The safe default status: unsupported platform / no owned bridge. */
export const notSupportedAppleFmStatus = (): AppleFmStatus => ({
  schema: APPLE_FM_STATUS_SCHEMA_ID,
  supported: false,
  state: "not_supported",
  readiness: "unsupported",
  ready: false,
  mode: "none",
  model: null,
  profileId: null,
  usageTruth: "unknown",
  unavailableReason: "unsupported_hardware",
  blockerRefs: ["blocker.apple_fm.unsupported_platform"],
})

/** The safe default status when the host is unreachable or the response invalid. */
export const unavailableAppleFmStatus = (): AppleFmStatus => ({
  schema: APPLE_FM_STATUS_SCHEMA_ID,
  supported: true,
  state: "unavailable",
  readiness: "unreachable",
  ready: false,
  mode: "none",
  model: null,
  profileId: null,
  usageTruth: "unknown",
  unavailableReason: "bridge_unreachable",
  blockerRefs: ["blocker.apple_fm.status_unavailable"],
})

export const refusedNotReadyAppleFmTurn = (): AppleFmTurnResult => ({
  schema: APPLE_FM_TURN_SCHEMA_ID,
  ok: false,
  outcome: "refused_not_ready",
  text: null,
  usageTruth: "unknown",
  promptTokens: null,
  completionTokens: null,
  totalTokens: null,
  failureClass: "not_ready",
})

export const refusedUnsupportedAppleFmTurn = (): AppleFmTurnResult => ({
  schema: APPLE_FM_TURN_SCHEMA_ID,
  ok: false,
  outcome: "refused_unsupported",
  text: null,
  usageTruth: "unknown",
  promptTokens: null,
  completionTokens: null,
  totalTokens: null,
  failureClass: "unsupported_platform",
})

export const invalidAppleFmTurn = (): AppleFmTurnResult => ({
  schema: APPLE_FM_TURN_SCHEMA_ID,
  ok: false,
  outcome: "failed",
  text: null,
  usageTruth: "unknown",
  promptTokens: null,
  completionTokens: null,
  totalTokens: null,
  failureClass: "invalid_request",
})

export const unavailableAppleFmStopResult = (): AppleFmStopResult => ({
  schema: APPLE_FM_STOP_SCHEMA_ID,
  stopped: false,
  state: "unavailable",
})
