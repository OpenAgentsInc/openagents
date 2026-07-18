import { Exit, Schema } from "effect"

import {
  CODEX_LOCAL_FULL_AUTO_DETAIL_LIMIT,
  CodexLocalFullAutoLiveStateSchema,
} from "./codex-local-contract.ts"
import {
  FULL_AUTO_BLOCKED_REASON_LIMIT,
  FULL_AUTO_DECISION_HISTORY_LIMIT,
  FULL_AUTO_ROTATION_HISTORY_LIMIT,
  FULL_AUTO_ROUTING_POLICY_LIMIT,
  FullAutoContinuationDecisionSchema,
  FullAutoDisabledBySchema,
  FullAutoGuardrailsSchema,
  FullAutoRotationRecordSchema,
  FullAutoRoutingCandidateSchema,
} from "./full-auto-registry.ts"
import {
  FullAutoRecoveryActionSchema,
  FullAutoStallCauseSchema,
} from "./full-auto-liveness.ts"
import {
  FULL_AUTO_RUN_DONE_CONDITION_LIMIT,
  FULL_AUTO_RUN_ACTIVE_LIMIT,
  FULL_AUTO_RUN_OBJECTIVE_LIMIT,
  FULL_AUTO_RUN_REASON_LIMIT,
  FULL_AUTO_RUN_TITLE_LIMIT,
  FullAutoRunObjectiveSourceSchema,
  FullAutoRunStateSchema,
  FullAutoRunTransitionRecordSchema,
} from "./full-auto-run-registry.ts"
import {
  ProviderHandoffRefusalReasonSchema,
  ProviderHandoffTransitionRecordSchema,
} from "./full-auto-provider-handoff.ts"
import { LocalTurnDispositionSchema, LocalTurnPhaseSchema } from "./local-turn-journal.ts"
import { FullAutoRunReceiptSchema, FullAutoRunReportSchema } from "./full-auto-run-report.ts"

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

/**
 * FA-WIRE-01 (#8996): the optional ordered routing policy + guardrails a
 * start/enable caller may bind in the same request. The candidate/guardrail
 * shapes are the EXACT durable registry schemas (full-auto-registry.ts), so
 * the HTTP surface can never smuggle a wider value into the durable store;
 * lane admission is separately validated fail-closed via
 * validateFullAutoRoutingPolicy before anything is written.
 */
export const FullAutoControlRoutingPolicySchema = Schema.Array(FullAutoRoutingCandidateSchema).check(
  Schema.isMinLength(1),
  Schema.isMaxLength(FULL_AUTO_ROUTING_POLICY_LIMIT),
)

/** POST /v1/full-auto/{threadRef}/enable -- the caller MUST name the workspace
 * it expects; the server refuses (409) when the current resolution differs. */
export const FullAutoControlEnableRequestSchema = Schema.Struct({
  workspaceRef: WorkspaceRef,
  lane: Schema.optional(LaneRef),
  /** FA-WIRE-01 (#8996): optional ordered multi-lane routing policy (order =
   * rotation priority). Validated fail-closed before binding. */
  routingPolicy: Schema.optional(FullAutoControlRoutingPolicySchema),
  /** FA-WIRE-01 (#8996): optional owner-configured guardrails. Invalid
   * shapes (non-positive limits) fail decode -> 400 invalid_request. */
  guardrails: Schema.optional(FullAutoGuardrailsSchema),
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
  /** FA-WIRE-01 (#8996): see FullAutoControlEnableRequestSchema. */
  routingPolicy: Schema.optional(FullAutoControlRoutingPolicySchema),
  guardrails: Schema.optional(FullAutoGuardrailsSchema),
})
export type FullAutoControlStartRequest = typeof FullAutoControlStartRequestSchema.Type
export const decodeFullAutoControlStartRequest = (
  value: unknown,
): FullAutoControlStartRequest | null => {
  const decoded = Schema.decodeUnknownExit(FullAutoControlStartRequestSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

const RunRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
const ModelRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
export const decodeFullAutoControlRunRef = (value: unknown): string | null => {
  const decoded = Schema.decodeUnknownExit(RunRef)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

/**
 * FA-RUN-01 (#8969): POST /v1/full-auto/runs/start -- the run-level
 * bootstrap. Unlike the thread-level /v1/full-auto/start (kept unchanged),
 * this route requires an explicit title/objective/doneCondition (FA-AC-38)
 * and mints a distinct runRef/threadRef for independent concurrent admission
 * (FA-AC-39 rev 13).
 */
export const FullAutoControlRunStartRequestSchema = Schema.Struct({
  workspaceRef: WorkspaceRef,
  title: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_RUN_TITLE_LIMIT)),
  objective: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_RUN_OBJECTIVE_LIMIT)),
  doneCondition: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_RUN_DONE_CONDITION_LIMIT)),
  lane: Schema.optional(LaneRef),
  /** Optional exact provider model, admitted against the selected lane before
   * the run is minted and retained in the durable execution profile. */
  model: Schema.optional(ModelRef),
  turnCap: Schema.optional(Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0), Schema.isLessThanOrEqualTo(1000))),
  /** FA-WIRE-01 (#8996): optional ordered routing policy + guardrails, bound
   * onto the run's thread-level record after the run mints (validated
   * fail-closed BEFORE anything is minted). */
  routingPolicy: Schema.optional(FullAutoControlRoutingPolicySchema),
  guardrails: Schema.optional(FullAutoGuardrailsSchema),
})
export type FullAutoControlRunStartRequest = typeof FullAutoControlRunStartRequestSchema.Type
export const decodeFullAutoControlRunStartRequest = (
  value: unknown,
): FullAutoControlRunStartRequest | null => {
  const decoded = Schema.decodeUnknownExit(FullAutoControlRunStartRequestSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

/** The public-safe projection of one `FullAutoRun`. Same trust tier as the
 * rest of this loopback, bearer-gated surface (which already exposes
 * workspaceRef/accountRef) -- objective/doneCondition are included here so a
 * local control-API caller can see the mission it is driving, but this
 * module NEVER writes objective/doneCondition text into `auditLog` or any
 * other routine log line (see the privacy note in full-auto-run-registry.ts). */
export const FullAutoControlRunSchema = Schema.Struct({
  runRef: RunRef,
  threadRef: Schema.NullOr(ThreadRef),
  title: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_RUN_TITLE_LIMIT)),
  objective: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_RUN_OBJECTIVE_LIMIT)),
  objectiveSource: FullAutoRunObjectiveSourceSchema,
  doneCondition: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_RUN_DONE_CONDITION_LIMIT)),
  workspaceRef: Schema.NullOr(WorkspaceRef),
  lane: Schema.NullOr(LaneRef),
  turnCap: Count,
  successfulAttempts: Count,
  failedAttempts: Count,
  state: FullAutoRunStateSchema,
  stateRevision: Count,
  terminalReason: Schema.NullOr(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_RUN_REASON_LIMIT))),
  predecessorRunRef: Schema.NullOr(RunRef),
  migratedFrom: Schema.NullOr(Schema.Literal("legacy_registry")),
  createdAt: Schema.String,
  startedAt: Schema.NullOr(Schema.String),
  lastProgressAt: Schema.NullOr(Schema.String),
  pausedAt: Schema.NullOr(Schema.String),
  stoppedAt: Schema.NullOr(Schema.String),
  completedAt: Schema.NullOr(Schema.String),
  transitions: Schema.Array(FullAutoRunTransitionRecordSchema),
  /**
   * FA-RUN-03 (#8971): the main-owned liveness projection, always computed
   * fresh against the current state (never a stale cached field). `state`
   * above already reflects `stallCause`/`nextRetryAt` when a liveness settle
   * pass has run -- these three fields are the "why" and "what can I do"
   * that a generic `state: "stalled"` alone cannot express (AC "Sidebar/run
   * view and control API return the same typed state and retry deadline").
   */
  stallCause: Schema.NullOr(FullAutoStallCauseSchema),
  nextRetryAt: Schema.NullOr(Schema.String),
  recoveryAction: FullAutoRecoveryActionSchema,
})
export type FullAutoControlRun = typeof FullAutoControlRunSchema.Type

export const FullAutoControlRunListResponseSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_CONTROL_SCHEMA),
  serverInstanceId: FullAutoControlInstanceIdSchema,
  runs: Schema.Array(FullAutoControlRunSchema),
})
export type FullAutoControlRunListResponse = typeof FullAutoControlRunListResponseSchema.Type

export const FullAutoControlRunStatusResponseSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_CONTROL_SCHEMA),
  serverInstanceId: FullAutoControlInstanceIdSchema,
  run: FullAutoControlRunSchema,
})
export type FullAutoControlRunStatusResponse = typeof FullAutoControlRunStatusResponseSchema.Type

export const FullAutoControlRunMutationResponseSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_CONTROL_SCHEMA),
  ok: Schema.Literal(true),
  run: FullAutoControlRunSchema,
})
export type FullAutoControlRunMutationResponse = typeof FullAutoControlRunMutationResponseSchema.Type

/**
 * FA-HO-01 (#8975): POST /v1/full-auto/runs/{runRef}/handoff -- a manual
 * provider switch legal ONLY while the run is `paused` (FA-AC-58). The
 * caller names the target lane; the server re-validates its admission/auth/
 * capability eligibility (FA-AC-59) before rebinding the run's execution
 * profile, so a refusal leaves the run's current lane/profile untouched
 * (rollback, never a partial state change).
 */
export const FullAutoControlRunHandoffRequestSchema = Schema.Struct({
  targetLaneRef: LaneRef,
  /** Optional exact target-lane model. Absent preserves the existing
   * target-lane default behavior. */
  model: Schema.optional(ModelRef),
  reason: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_RUN_REASON_LIMIT))),
})
export type FullAutoControlRunHandoffRequest = typeof FullAutoControlRunHandoffRequestSchema.Type
export const decodeFullAutoControlRunHandoffRequest = (
  value: unknown,
): FullAutoControlRunHandoffRequest | null => {
  const decoded = Schema.decodeUnknownExit(FullAutoControlRunHandoffRequestSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

export const FullAutoControlRunHandoffResponseSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_CONTROL_SCHEMA),
  ok: Schema.Literal(true),
  run: FullAutoControlRunSchema,
  transition: ProviderHandoffTransitionRecordSchema,
})
export type FullAutoControlRunHandoffResponse = typeof FullAutoControlRunHandoffResponseSchema.Type

/**
 * FA-RUN-04 (#8972): GET /v1/full-auto/runs/{runRef}/report -- the freshly
 * synced, bounded, PRIVATE `FullAutoRunReport` aggregating lifecycle
 * transitions (FA-RUN-01 #8969), liveness/stall observations (FA-RUN-03
 * #8971), provider-handoff transitions (FA-HO-01 #8975), and turn outcomes
 * for exactly this run. Same authenticated loopback trust tier as the rest
 * of this surface -- the existing `FullAutoControlRunSchema` already returns
 * raw objective/doneCondition text to any bearer holder, so this route adds
 * no new category of exposure, only more history for the SAME caller.
 */
export const FullAutoControlRunReportResponseSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_CONTROL_SCHEMA),
  report: FullAutoRunReportSchema,
})
export type FullAutoControlRunReportResponse = typeof FullAutoControlRunReportResponseSchema.Type

/**
 * FA-RUN-04 (#8972): GET /v1/full-auto/runs/{runRef}/receipt -- the derived
 * PUBLIC-SAFE `FullAutoRunReceipt`: identities, digests, dispositions,
 * counts, and artifact refs only, provably redacted of objective/
 * doneCondition/transcript/reason/path/account text (see the adversarial
 * tests in full-auto-run-report.test.ts). Safe to attach to a public
 * dogfood issue or export outside the loopback boundary.
 */
export const FullAutoControlRunReceiptResponseSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_CONTROL_SCHEMA),
  receipt: FullAutoRunReceiptSchema,
})
export type FullAutoControlRunReceiptResponse = typeof FullAutoControlRunReceiptResponseSchema.Type

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
  /**
   * FA-RT-01 (#8987): bounded lane-rotation history, most recent last.
   * OPTIONAL and additive so existing server projections keep decoding
   * unchanged; the control-server projection (owned separately) populates it
   * from `projectFullAutoRotationHistory` in full-auto-registry.ts. Entries
   * are public-safe by construction: lane refs, a typed reason, and an ISO
   * timestamp only -- never prompts, models, paths, or secrets.
   */
  rotationHistory: Schema.optional(Schema.Array(FullAutoRotationRecordSchema).check(
    Schema.isMaxLength(FULL_AUTO_ROTATION_HISTORY_LIMIT),
  )),
  /**
   * FA-WIRE-01 (#8996): bounded continuation-decision history, most recent
   * last, projected via projectFullAutoDecisionHistory. OPTIONAL and additive
   * like rotationHistory; the server projection always populates it.
   */
  decisionHistory: Schema.optional(Schema.Array(FullAutoContinuationDecisionSchema).check(
    Schema.isMaxLength(FULL_AUTO_DECISION_HISTORY_LIMIT),
  )),
  /** FA-WIRE-01 (#8996): the record's bound owner-configured guardrails, or
   * null when none are bound. */
  guardrails: Schema.optional(Schema.NullOr(FullAutoGuardrailsSchema)),
  /** FA-WIRE-01 (#8996): the ordered bound routing policy, or null. */
  routingPolicy: Schema.optional(Schema.NullOr(FullAutoControlRoutingPolicySchema)),
  /** FA-GD-01/FA-WIRE-01: non-null exactly when the record is durably paused
   * by the low-confidence gate; cleared only by an explicit resume. */
  pausedReason: Schema.optional(Schema.NullOr(Schema.String.check(
    Schema.isMinLength(1),
    Schema.isMaxLength(FULL_AUTO_BLOCKED_REASON_LIMIT),
  ))),
  pausedAt: Schema.optional(Schema.NullOr(Schema.String)),
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

/**
 * FA-WIRE-01 (#8996): the typed routing-policy refusal vocabulary, mirrored
 * from full-auto-routing.ts (type-only import keeps that module's
 * capability-registry dependencies out of this contract). The two compile-time
 * parity assertions below fail the build if either side drifts.
 */
export const FullAutoControlRoutingPolicyRefusalReasonSchema = Schema.Literals([
  "policy_empty",
  "policy_too_long",
  "duplicate_candidate",
  "lane_unknown",
  "lane_not_admitted",
  "lane_not_full_auto_eligible",
])
type _RoutingRefusalCovers =
  typeof FullAutoControlRoutingPolicyRefusalReasonSchema.Type extends
    import("./full-auto-routing.ts").FullAutoRoutingPolicyRefusalReason ? true : never
type _RoutingRefusalComplete =
  import("./full-auto-routing.ts").FullAutoRoutingPolicyRefusalReason extends
    typeof FullAutoControlRoutingPolicyRefusalReasonSchema.Type ? true : never
const _routingRefusalParity: readonly [_RoutingRefusalCovers, _RoutingRefusalComplete] = [true, true]
void _routingRefusalParity

/** Machine-readable error tags every non-2xx response carries. */
export const FullAutoControlErrorTagSchema = Schema.Literals([
  "unauthorized",
  "not_found",
  "method_not_allowed",
  "invalid_request",
  "workspace_mismatch",
  "lane_not_eligible",
  "model_not_eligible",
  /** Legacy pre-rev-13 compatibility tag. New starts no longer emit it. */
  "active_run_conflict",
  /** The bounded local concurrent-run capacity is full. */
  "active_run_limit_reached",
  /** FA-AC-43: the requested run-lifecycle transition is not legal from the
   * run's current state (for example Resume from a non-Paused state). */
  "illegal_transition",
  /** FA-HO-01/FA-AC-59: the target lane failed admission/auth/capability
   * re-validation; the run's current lane/profile is unchanged (rollback,
   * never a partial state change). See `handoffRefusalReason` on the error
   * body for the exact reason. */
  "handoff_refused",
  /** FA-RUN-03 (#8971): retry-now was requested on a run that is not
   * Stalled, or whose current classified cause fails closed (AC "Nonrecoverable
   * states fail closed and present one safe action"). `stallCause` on the
   * error body names the cause; the safe action is always Stop. */
  "not_recoverable",
  /** FA-WIRE-01 (#8996): a submitted routingPolicy failed
   * validateFullAutoRoutingPolicy -- nothing was written or minted. See
   * `routingPolicyRefusalReason`/`lane` on the error body. */
  "routing_policy_refused",
  /** FA-WIRE-01 (#8996): resume was requested for a record that exists but
   * is not currently paused -- resume never re-enables a disabled record. */
  "not_paused",
])
export type FullAutoControlErrorTag = typeof FullAutoControlErrorTagSchema.Type

export const FullAutoControlErrorSchema = Schema.Struct({
  error: FullAutoControlErrorTagSchema,
  message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(600)),
  /** workspace_mismatch only: what the caller named vs. what main resolved.
   * Local loopback surface -- the caller already knows its own paths. */
  expectedWorkspaceRef: Schema.optional(WorkspaceRef),
  resolvedWorkspaceRef: Schema.optional(WorkspaceRef),
  /** Legacy active_run_conflict only; retained for decoder compatibility. */
  activeRunRef: Schema.optional(RunRef),
  /** active_run_limit_reached only: bounded local capacity evidence. */
  activeRunCount: Schema.optional(Count),
  activeRunLimit: Schema.optional(Schema.Literal(FULL_AUTO_RUN_ACTIVE_LIMIT)),
  /** illegal_transition only: the exact refused edge. */
  fromState: Schema.optional(FullAutoRunStateSchema),
  toState: Schema.optional(FullAutoRunStateSchema),
  /** handoff_refused only: the exact typed refusal reason (FA-AC-59). */
  handoffRefusalReason: Schema.optional(ProviderHandoffRefusalReasonSchema),
  /** not_recoverable only: the classified cause that failed closed. */
  stallCause: Schema.optional(FullAutoStallCauseSchema),
  /** routing_policy_refused only: the exact typed refusal reason from
   * validateFullAutoRoutingPolicy, plus the first offending lane when the
   * refusal is lane-scoped. */
  routingPolicyRefusalReason: Schema.optional(FullAutoControlRoutingPolicyRefusalReasonSchema),
  lane: Schema.optional(LaneRef),
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
  // FA-WIRE-01 (#8996): the explicit resume command for a low-confidence
  // pause (FA-GD-01) -- wires the exported resumeFullAuto, never a new
  // dispatch mechanism.
  { method: "post", path: "/v1/full-auto/{threadRef}/resume", operationId: "resumeFullAutoThread" },
  { method: "get", path: "/v1/full-auto/{threadRef}/turns", operationId: "listFullAutoTurns" },
  // FA-RUN-01 (#8969): the durable FullAutoRun lifecycle surface. Distinct
  // from the thread-level routes above (kept unchanged) -- these operate on
  // runRef identity, support independently active runs, and route every
  // mutation through the single typed transition
  // function in full-auto-run-registry.ts.
  { method: "get", path: "/v1/full-auto/runs", operationId: "listFullAutoRuns" },
  { method: "post", path: "/v1/full-auto/runs/start", operationId: "startFullAutoRun" },
  { method: "get", path: "/v1/full-auto/runs/{runRef}", operationId: "getFullAutoRunStatus" },
  { method: "post", path: "/v1/full-auto/runs/{runRef}/pause", operationId: "pauseFullAutoRun" },
  { method: "post", path: "/v1/full-auto/runs/{runRef}/resume", operationId: "resumeFullAutoRun" },
  { method: "post", path: "/v1/full-auto/runs/{runRef}/stop", operationId: "stopFullAutoRun" },
  // FA-HO-01 (#8975): manual cross-provider handoff, legal only while paused.
  { method: "post", path: "/v1/full-auto/runs/{runRef}/handoff", operationId: "handoffFullAutoRun" },
  // FA-RUN-03 (#8971): AC-48's owner-actionable recovery affordance -- legal
  // only from Stalled, and only when the freshly classified cause is
  // recoverable; a nonrecoverable cause refuses with `not_recoverable`.
  { method: "post", path: "/v1/full-auto/runs/{runRef}/retry-now", operationId: "retryFullAutoRunNow" },
  // FA-RUN-04 (#8972): the bounded private aggregate report and its derived
  // public-safe receipt projection.
  { method: "get", path: "/v1/full-auto/runs/{runRef}/report", operationId: "getFullAutoRunReport" },
  { method: "get", path: "/v1/full-auto/runs/{runRef}/receipt", operationId: "getFullAutoRunReceipt" },
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
