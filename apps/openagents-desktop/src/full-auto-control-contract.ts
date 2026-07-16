import { Exit, Schema } from "effect"

import {
  CODEX_LOCAL_FULL_AUTO_DETAIL_LIMIT,
  CodexLocalFullAutoLiveStateSchema,
} from "./codex-local-contract.ts"
import {
  FULL_AUTO_BLOCKED_REASON_LIMIT,
  FullAutoDisabledBySchema,
} from "./full-auto-registry.ts"
import { LocalTurnDispositionSchema, LocalTurnPhaseSchema } from "./local-turn-journal.ts"

/**
 * FA-H13 (#8886): the request/response contract for the Phase 1 local Full
 * Auto control surface -- a loopback-only, opt-in, bearer-gated HTTP API in
 * Desktop main that lets a same-machine agent list/inspect/enable/disable/
 * continue Full Auto without clicking the composer toggle.
 *
 * Bounds mirror the existing IPC contract exactly (threadRef <= 120 like
 * CodexLocalFullAutoSetRequestSchema, workspaceRef <= 1024 like the registry's
 * durable binding, detail <= 300 like the live-state broadcast) so the HTTP
 * surface can never smuggle a wider value into the same durable stores.
 */
export const FULL_AUTO_CONTROL_SCHEMA = "openagents.desktop.full_auto_control.v1" as const
/** Opt-in flag: Desktop main starts the control server ONLY when this is "1". */
export const FULL_AUTO_CONTROL_ENV_FLAG = "OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL" as const
/** Optional env-pinned port; unset/invalid means an ephemeral loopback port. */
export const FULL_AUTO_CONTROL_PORT_ENV = "OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL_PORT" as const
/** Bounded recent-turn history served by the turns route. */
export const FULL_AUTO_CONTROL_TURNS_LIMIT = 20
/** Distinct attribution marker every programmatic mutation stamps on its receipt note. */
export const FULL_AUTO_CONTROL_CALLER = "control-api" as const

const ThreadRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120))
const TurnRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
const WorkspaceRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1024))
const LaneRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))
export const FullAutoControlInstanceIdSchema = Schema.String.check(
  Schema.isMinLength(16),
  Schema.isMaxLength(120),
)
const Count = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)

export const decodeFullAutoControlThreadRef = (value: unknown): string | null => {
  const decoded = Schema.decodeUnknownExit(ThreadRef)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

/** POST /v1/full-auto/{threadRef}/enable -- the caller MUST name the workspace
 * it expects; the server refuses (409) when the current resolution differs. */
export const FullAutoControlEnableRequestSchema = Schema.Struct({
  workspaceRef: WorkspaceRef,
  lane: Schema.optional(LaneRef),
})
export type FullAutoControlEnableRequest = typeof FullAutoControlEnableRequestSchema.Type
export const decodeFullAutoControlEnableRequest = (
  value: unknown,
): FullAutoControlEnableRequest | null => {
  const decoded = Schema.decodeUnknownExit(FullAutoControlEnableRequestSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

/** POST /v1/full-auto/start -- programmatic bootstrap: mint a brand-new local
 * thread, enable Full Auto on it, and schedule the first continuation in one
 * fail-closed operation. The caller MUST name the workspace it expects exactly
 * like enable; on mismatch nothing is created. */
export const FullAutoControlStartRequestSchema = Schema.Struct({
  workspaceRef: WorkspaceRef,
  title: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))),
  lane: Schema.optional(LaneRef),
})
export type FullAutoControlStartRequest = typeof FullAutoControlStartRequestSchema.Type
export const decodeFullAutoControlStartRequest = (
  value: unknown,
): FullAutoControlStartRequest | null => {
  const decoded = Schema.decodeUnknownExit(FullAutoControlStartRequestSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

/** Coarse live state riding alongside the durable record (FA-H4 vocabulary). */
export const FullAutoControlLiveSchema = Schema.Struct({
  state: CodexLocalFullAutoLiveStateSchema,
  turnRef: Schema.NullOr(TurnRef),
  detail: Schema.optional(Schema.String.check(
    Schema.isMinLength(1),
    Schema.isMaxLength(CODEX_LOCAL_FULL_AUTO_DETAIL_LIMIT),
  )),
})
export type FullAutoControlLive = typeof FullAutoControlLiveSchema.Type

/**
 * The public-safe projection of one registry record. Deliberately narrower
 * than the durable FullAutoRecord: profile material is reduced to the
 * accountRef alone (never model/effort/raw provider material), and lease/
 * failure internals surface only through blockedReason and the live state.
 */
export const FullAutoControlRecordSchema = Schema.Struct({
  threadRef: ThreadRef,
  enabled: Schema.Boolean,
  continuationCount: Count,
  updatedAt: Schema.String,
  workspaceRef: Schema.NullOr(WorkspaceRef),
  lane: LaneRef,
  accountRef: Schema.NullOr(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))),
  blockedReason: Schema.NullOr(Schema.String.check(
    Schema.isMinLength(1),
    Schema.isMaxLength(FULL_AUTO_BLOCKED_REASON_LIMIT),
  )),
  disabledBy: Schema.NullOr(FullAutoDisabledBySchema),
  disabledAt: Schema.NullOr(Schema.String),
  live: FullAutoControlLiveSchema,
})
export type FullAutoControlRecord = typeof FullAutoControlRecordSchema.Type

export const FullAutoControlListResponseSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_CONTROL_SCHEMA),
  serverInstanceId: FullAutoControlInstanceIdSchema,
  records: Schema.Array(FullAutoControlRecordSchema),
})
export type FullAutoControlListResponse = typeof FullAutoControlListResponseSchema.Type

export const FullAutoControlStatusResponseSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_CONTROL_SCHEMA),
  serverInstanceId: FullAutoControlInstanceIdSchema,
  record: FullAutoControlRecordSchema,
})
export type FullAutoControlStatusResponse = typeof FullAutoControlStatusResponseSchema.Type

export const FullAutoControlMutationResponseSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_CONTROL_SCHEMA),
  ok: Schema.Literal(true),
  record: FullAutoControlRecordSchema,
})
export type FullAutoControlMutationResponse = typeof FullAutoControlMutationResponseSchema.Type

/** continue-now schedules the shared serialized reconcile pass and returns
 * immediately -- it never reports dispatch outcome inline. */
export const FullAutoControlContinueNowResponseSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_CONTROL_SCHEMA),
  scheduled: Schema.Literal(true),
})
export type FullAutoControlContinueNowResponse = typeof FullAutoControlContinueNowResponseSchema.Type

/** Bounded turn-history projection: identity, phase, disposition, and
 * timestamps only -- never transcript text. */
export const FullAutoControlTurnSchema = Schema.Struct({
  turnRef: TurnRef,
  phase: LocalTurnPhaseSchema,
  disposition: Schema.NullOr(LocalTurnDispositionSchema),
  createdAt: Schema.String,
  updatedAt: Schema.String,
})
export type FullAutoControlTurn = typeof FullAutoControlTurnSchema.Type

export const FullAutoControlTurnsResponseSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_CONTROL_SCHEMA),
  threadRef: ThreadRef,
  turns: Schema.Array(FullAutoControlTurnSchema).check(
    Schema.isMaxLength(FULL_AUTO_CONTROL_TURNS_LIMIT),
  ),
})
export type FullAutoControlTurnsResponse = typeof FullAutoControlTurnsResponseSchema.Type

/** Machine-readable error tags every non-2xx response carries. */
export const FullAutoControlErrorTagSchema = Schema.Literals([
  "unauthorized",
  "not_found",
  "method_not_allowed",
  "invalid_request",
  "workspace_mismatch",
  "lane_not_eligible",
])
export type FullAutoControlErrorTag = typeof FullAutoControlErrorTagSchema.Type

export const FullAutoControlErrorSchema = Schema.Struct({
  error: FullAutoControlErrorTagSchema,
  message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(600)),
  /** workspace_mismatch only: what the caller named vs. what main resolved.
   * Local loopback surface -- the caller already knows its own paths. */
  expectedWorkspaceRef: Schema.optional(WorkspaceRef),
  resolvedWorkspaceRef: Schema.optional(WorkspaceRef),
})
export type FullAutoControlError = typeof FullAutoControlErrorSchema.Type

/**
 * The single route table both the HTTP server and the OpenAPI document are
 * built from -- the parity test in full-auto-control-server.test.ts asserts
 * doc <-> table equivalence in both directions so the served surface and the
 * published description can never drift.
 */
export const FULL_AUTO_CONTROL_ROUTES = [
  { method: "get", path: "/v1/openapi.json", operationId: "getOpenApiDocument" },
  { method: "get", path: "/v1/lanes", operationId: "listProviderLanes" },
  { method: "get", path: "/v1/full-auto", operationId: "listFullAuto" },
  { method: "post", path: "/v1/full-auto/start", operationId: "startFullAuto" },
  { method: "get", path: "/v1/full-auto/{threadRef}", operationId: "getFullAutoStatus" },
  { method: "post", path: "/v1/full-auto/{threadRef}/enable", operationId: "enableFullAuto" },
  { method: "post", path: "/v1/full-auto/{threadRef}/disable", operationId: "disableFullAuto" },
  { method: "post", path: "/v1/full-auto/{threadRef}/continue-now", operationId: "continueFullAutoNow" },
  { method: "get", path: "/v1/full-auto/{threadRef}/turns", operationId: "listFullAutoTurns" },
] as const
export type FullAutoControlRoute = (typeof FULL_AUTO_CONTROL_ROUTES)[number]

/** The connection-info file written under Electron userData (mode 0600) so a
 * local agent can discover the server: full-auto/control.json. */
export const FullAutoControlFileSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_CONTROL_SCHEMA),
  url: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(200)),
  token: Schema.String.check(Schema.isMinLength(16), Schema.isMaxLength(200)),
  scopes: Schema.Array(Schema.String),
  issuedAtIso: Schema.String,
  /** #8928: additive process-ownership guard. Optional so connection files
   * written by the earlier v1 server remain decodable; cleanup must refuse to
   * signal when either value is absent. Current writers always emit both. */
  pid: Schema.optional(Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0))),
  serverInstanceId: Schema.optional(FullAutoControlInstanceIdSchema),
})
export type FullAutoControlFile = typeof FullAutoControlFileSchema.Type
export const decodeFullAutoControlFile = (value: unknown): FullAutoControlFile | null => {
  const decoded = Schema.decodeUnknownExit(FullAutoControlFileSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}
