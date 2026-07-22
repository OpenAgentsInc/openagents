import { Exit, Schema } from "@effect-native/core/effect"

/**
 * FA-UX-01 (#8974): the renderer<->main IPC channel set for the dedicated
 * Full Auto launcher + read-only run view. This is a SECOND thin transport
 * over the exact same main-owned action functions
 * (`full-auto-run-actions.ts`) the opt-in HTTP control server already uses
 * (`full-auto-control-server.ts`) -- never a parallel state machine. Every
 * mutation here uses `actor: "owner_ui"` so the durable transition/system-note
 * history always distinguishes an owner click from a programmatic CLI/MCP
 * caller. Follows the `codex-local-contract.ts` channel-constant +
 * Schema-decode pattern exactly.
 *
 * RENDERER-BOUNDARY NOTE: this file is imported from BOTH main (main.ts,
 * preload.cts) and the renderer (full-auto-workspace.ts, shell.ts). Every
 * schema below is therefore a SELF-CONTAINED, node-builtin-free duplicate of
 * the durable/control-API shapes defined in full-auto-run-registry.ts,
 * full-auto-liveness.ts, full-auto-control-contract.ts, full-auto-run-report.ts,
 * and full-auto-provider-handoff.ts -- those files unconditionally import
 * node:fs/node:path/node:crypto for their own store-opening logic, and even a
 * type-only reimport of one of their exported Schema VALUES pulls that whole
 * module (and its node:fs) into the renderer bundle (proven by
 * tests/build.test.ts and tests/electron-boundary.test.ts, which fail loudly
 * on exactly this leak). Keep it that way: add a new field here by copying
 * its shape, never by importing from those main-process-only modules.
 */
export const FullAutoRunListChannel = "openagents:full-auto-run:list" as const
export const FullAutoRunStartChannel = "openagents:full-auto-run:start" as const
export const FullAutoRunGetChannel = "openagents:full-auto-run:get" as const
export const FullAutoRunPauseChannel = "openagents:full-auto-run:pause" as const
export const FullAutoRunResumeChannel = "openagents:full-auto-run:resume" as const
export const FullAutoRunStopChannel = "openagents:full-auto-run:stop" as const
export const FullAutoRunRetryNowChannel = "openagents:full-auto-run:retry-now" as const
export const FullAutoRunHandoffChannel = "openagents:full-auto-run:handoff" as const
export const FullAutoRunReportChannel = "openagents:full-auto-run:report" as const
export const FullAutoRunReceiptChannel = "openagents:full-auto-run:receipt" as const

const FULL_AUTO_RUN_TITLE_LIMIT = 120
const FULL_AUTO_RUN_OBJECTIVE_LIMIT = 4000
const FULL_AUTO_RUN_DONE_CONDITION_LIMIT = 2000
const FULL_AUTO_RUN_REASON_LIMIT = 400

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
const ModelRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
const WorkspaceRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1024))
const LaneRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120))
const Count = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0))

/** { runRef } -- the shape every single-run route beyond start/list needs. */
export const FullAutoRunRefRequestSchema = Schema.Struct({ runRef: Ref })
export type FullAutoRunRefRequest = typeof FullAutoRunRefRequestSchema.Type
export const decodeFullAutoRunRefRequest = (value: unknown): FullAutoRunRefRequest | null => {
  const decoded = Schema.decodeUnknownExit(FullAutoRunRefRequestSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

/**
 * FA-WIRE-01 (#8996): SELF-CONTAINED duplicates of the durable routing
 * candidate / guardrail shapes (full-auto-registry.ts) -- see the
 * RENDERER-BOUNDARY NOTE above for why these are copied, never imported.
 */
export const FullAutoRunRoutingCandidateSchema = Schema.Struct({
  lane: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  accountRef: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))),
})
export type FullAutoRunRoutingCandidate = typeof FullAutoRunRoutingCandidateSchema.Type
/** Mirrors FULL_AUTO_ROUTING_POLICY_LIMIT in full-auto-registry.ts. */
export const FULL_AUTO_RUN_ROUTING_POLICY_LIMIT = 8
export const FullAutoRunRoutingPolicySchema = Schema.Array(FullAutoRunRoutingCandidateSchema).check(
  Schema.isMinLength(1),
  Schema.isMaxLength(FULL_AUTO_RUN_ROUTING_POLICY_LIMIT),
)

const PositiveGuardrailCount = Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0))
/** Mirrors FullAutoGuardrailsSchema in full-auto-registry.ts. */
export const FullAutoRunGuardrailsSchema = Schema.Struct({
  maxWallClockMs: Schema.optional(PositiveGuardrailCount),
  maxTurns: Schema.optional(PositiveGuardrailCount),
  maxPerTurnFailures: Schema.optional(PositiveGuardrailCount),
  tokenBudgetRef: Schema.optional(Ref),
})
export type FullAutoRunGuardrails = typeof FullAutoRunGuardrailsSchema.Type

/** Mirrors full-auto-control-contract.ts's FullAutoControlRunStartRequestSchema. */
export const FullAutoRunStartRequestSchema = Schema.Struct({
  workspaceRef: WorkspaceRef,
  title: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_RUN_TITLE_LIMIT)),
  objective: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_RUN_OBJECTIVE_LIMIT)),
  doneCondition: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_RUN_DONE_CONDITION_LIMIT)),
  lane: Schema.optional(LaneRef),
  model: Schema.optional(ModelRef),
  turnCap: Schema.optional(Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0), Schema.isLessThanOrEqualTo(1000))),
  /** FA-WIRE-01 (#8996): optional ordered routing policy (order = rotation
   * priority) and owner guardrails, validated fail-closed main-side. */
  routingPolicy: Schema.optional(FullAutoRunRoutingPolicySchema),
  guardrails: Schema.optional(FullAutoRunGuardrailsSchema),
})
export type FullAutoRunStartRequest = typeof FullAutoRunStartRequestSchema.Type
export const decodeFullAutoRunStartRequest = (value: unknown): FullAutoRunStartRequest | null => {
  const decoded = Schema.decodeUnknownExit(FullAutoRunStartRequestSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

export const FullAutoRunHandoffIpcRequestSchema = Schema.Struct({
  runRef: Ref,
  targetLaneRef: LaneRef,
  model: Schema.optional(ModelRef),
  reason: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_RUN_REASON_LIMIT))),
})
export type FullAutoRunHandoffIpcRequest = typeof FullAutoRunHandoffIpcRequestSchema.Type
export const decodeFullAutoRunHandoffIpcRequest = (value: unknown): FullAutoRunHandoffIpcRequest | null => {
  const decoded = Schema.decodeUnknownExit(FullAutoRunHandoffIpcRequestSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

// -- Mirrors full-auto-run-registry.ts's state/actor/transition vocabulary --
const FullAutoRunStateSchema = Schema.Literals([
  "draft", "running", "pausing", "paused", "retrying", "stalled",
  "completed", "failed", "stopped", "cap_reached",
])
const FullAutoRunActorSchema = Schema.Literals([
  "owner_ui", "control_api", "cli", "mcp", "workspace_guard", "continuation_cap",
  "dispatch_failure_limit", "turn_resolution", "thread_state_sync", "legacy_migration", "liveness_monitor",
  // FA-GD-01 (#8991): guardrail stops (wall clock / turn / failure budgets).
  "guardrail",
  // MOB-FA-02 (#8994): a typed Pause/Resume/Stop intent dispatched from
  // OpenAgents mobile and applied by Desktop's control-intent consumer.
  "mobile",
])
const FullAutoRunObjectiveSourceSchema = Schema.Literals([
  "user", "control_caller", "legacy_migration",
  // HANDS-1 (#9172): a host-proposed, owner-endorsed objective.
  "system_selected",
])
const FullAutoRunTransitionRecordSchema = Schema.Struct({
  from: FullAutoRunStateSchema,
  to: FullAutoRunStateSchema,
  actor: FullAutoRunActorSchema,
  at: Schema.String,
  reason: Schema.String,
  correlationRef: Schema.optional(Ref),
})
// -- Mirrors full-auto-liveness.ts's stall/recovery vocabulary --
const FullAutoStallCauseSchema = Schema.Literals([
  "host_thread_missing", "provider_session_missing", "workspace_mismatch",
  "auth_admission_failure", "stale_lease", "app_offline", "dispatch_overdue",
  "account_exhausted", "rate_limited", "provider_error", "unknown_error",
])
const FullAutoRecoveryActionSchema = Schema.Literals(["retry_now", "stop_only", "none"])

/** Mirrors full-auto-control-contract.ts's FullAutoControlRunSchema (the
 * public-safe run projection every route/UI reads). */
export const FullAutoRunProjectionSchema = Schema.Struct({
  runRef: Ref,
  threadRef: Schema.NullOr(Ref),
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
  predecessorRunRef: Schema.NullOr(Ref),
  migratedFrom: Schema.NullOr(Schema.Literal("legacy_registry")),
  createdAt: Schema.String,
  startedAt: Schema.NullOr(Schema.String),
  lastProgressAt: Schema.NullOr(Schema.String),
  pausedAt: Schema.NullOr(Schema.String),
  stoppedAt: Schema.NullOr(Schema.String),
  completedAt: Schema.NullOr(Schema.String),
  transitions: Schema.Array(FullAutoRunTransitionRecordSchema),
  stallCause: Schema.NullOr(FullAutoStallCauseSchema),
  nextRetryAt: Schema.NullOr(Schema.String),
  recoveryAction: FullAutoRecoveryActionSchema,
})
export type FullAutoRunProjection = typeof FullAutoRunProjectionSchema.Type

/** Only `error`/`message` are decode-validated; refusal responses carry
 * additional typed fields (e.g. `activeRunRef`, `stallCause`) that the UI
 * does not currently branch on, so they pass through untyped rather than
 * requiring this boundary to track every action's exact refusal shape. */
const FullAutoControlErrorSchema = Schema.Struct({
  error: Schema.String,
  message: Schema.String,
})
export type FullAutoControlError = typeof FullAutoControlErrorSchema.Type & Readonly<Record<string, unknown>>

/** Every mutation/read result is this same discriminated shape on the wire,
 * decoded renderer-side so a stale/mismatched main build cannot silently
 * hand the renderer an untyped payload. */
export type FullAutoRunIpcOutcome<T> =
  | Readonly<{ ok: true; value: T }>
  | Readonly<{ ok: false; status: number; error: FullAutoControlError }>

const outcomeSchema = <A, I>(
  value: Schema.Codec<A, I>,
): Schema.Codec<FullAutoRunIpcOutcome<A>, unknown> =>
  Schema.Union([
    Schema.Struct({ ok: Schema.Literal(true), value }),
    Schema.Struct({ ok: Schema.Literal(false), status: Schema.Number, error: FullAutoControlErrorSchema }),
  ]) as unknown as Schema.Codec<FullAutoRunIpcOutcome<A>, unknown>

/** `resolvedWorkspaceRef` is the SAME value `startFullAutoRunAction` checks
 * `body.workspaceRef` against (main's `resolveWorkspaceRef()`) -- the
 * launcher pre-fills from this, never from the unrelated Files-workspace
 * `workingDirectory` bridge, so a smoke/fixture-mode divergence between the
 * two can never manifest as a false "workspace_mismatch" refusal. */
export const FullAutoRunListResultSchema = Schema.Struct({
  runs: Schema.Array(FullAutoRunProjectionSchema),
  resolvedWorkspaceRef: Schema.NullOr(WorkspaceRef),
})
export type FullAutoRunListResult = typeof FullAutoRunListResultSchema.Type
export const decodeFullAutoRunListResult = (value: unknown): FullAutoRunListResult | null => {
  const decoded = Schema.decodeUnknownExit(FullAutoRunListResultSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

export const decodeFullAutoRunOutcome = (value: unknown): FullAutoRunIpcOutcome<FullAutoRunProjection> | null => {
  const decoded = Schema.decodeUnknownExit(outcomeSchema(FullAutoRunProjectionSchema))(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

/**
 * Report/receipt/handoff-transition payloads are large, evolving schema
 * graphs owned by full-auto-run-report.ts / full-auto-provider-handoff.ts
 * (both node:fs-touching, main-only). Rather than duplicate that whole graph
 * here, this boundary validates only the outer discriminated envelope and
 * a minimal read-only display shape -- these values never cross a real
 * trust boundary (same-process IPC from main's own action functions), and
 * the renderer only ever reads bounded string/array fields off them for
 * display, never branches product logic on an un-narrowed field.
 */
export type FullAutoRunReportView = Readonly<{
  turns: ReadonlyArray<Readonly<{ turnRef: string; lane: string; outcomeSummary: string; createdAt: string; updatedAt: string }>>
  providerTransitions: ReadonlyArray<Readonly<{ handoffRef: string; from: string; to: string; disposition: string; truncated: boolean; reason: string }>>
}>
const FullAutoRunReportViewSchema = Schema.Struct({
  turns: Schema.Array(Schema.Struct({
    turnRef: Schema.String, lane: Schema.String, outcomeSummary: Schema.String, createdAt: Schema.String, updatedAt: Schema.String,
  })),
  providerTransitions: Schema.Array(Schema.Struct({
    handoffRef: Schema.String, from: Schema.String, to: Schema.String, disposition: Schema.String, truncated: Schema.Boolean, reason: Schema.String,
  })),
})
export const decodeFullAutoRunReportOutcome = (value: unknown): FullAutoRunIpcOutcome<FullAutoRunReportView> | null => {
  const decoded = Schema.decodeUnknownExit(outcomeSchema(FullAutoRunReportViewSchema))(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

export type FullAutoRunReceiptView = Readonly<Record<string, unknown>>
export const decodeFullAutoRunReceiptOutcome = (value: unknown): FullAutoRunIpcOutcome<FullAutoRunReceiptView> | null => {
  const decoded = Schema.decodeUnknownExit(outcomeSchema(Schema.Record(Schema.String, Schema.Unknown)))(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

export type FullAutoRunHandoffResult = Readonly<{
  run: FullAutoRunProjection
  transition: Readonly<{ handoffRef: string; from: string; to: string; disposition: string; truncated: boolean; reason: string }>
}>
const FullAutoRunHandoffResultSchema = Schema.Struct({
  run: FullAutoRunProjectionSchema,
  transition: Schema.Struct({
    handoffRef: Schema.String, from: Schema.String, to: Schema.String, disposition: Schema.String, truncated: Schema.Boolean, reason: Schema.String,
  }),
})
export const decodeFullAutoRunHandoffOutcome = (
  value: unknown,
): FullAutoRunIpcOutcome<FullAutoRunHandoffResult> | null => {
  const decoded = Schema.decodeUnknownExit(outcomeSchema(FullAutoRunHandoffResultSchema))(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

export const unavailableFullAutoRunOutcome = <T,>(): FullAutoRunIpcOutcome<T> => ({
  ok: false,
  status: 503,
  error: { error: "invalid_request", message: "Full Auto run control is unavailable in this environment." },
})

/**
 * Renderer-facing capability shape `boot.ts` wires and `full-auto-workspace.ts`
 * consumes. Mirrors the `XxxBridge` convention used throughout this app
 * (e.g. `FleetAccountsBridge`): every method returns the RAW bridge payload
 * (`Promise<unknown>`), never a pre-decoded value -- the renderer-side
 * workspace module is the one place that decodes it with this contract's
 * `decodeFullAutoRun*Outcome` helpers, so an untyped or stale-main payload
 * can never silently masquerade as a valid typed result.
 */
export type FullAutoRunRendererHost = Readonly<{
  list: () => Promise<unknown>
  start: (request: FullAutoRunStartRequest) => Promise<unknown>
  get: (runRef: string) => Promise<unknown>
  pause: (runRef: string) => Promise<unknown>
  resume: (runRef: string) => Promise<unknown>
  stop: (runRef: string) => Promise<unknown>
  retryNow: (runRef: string) => Promise<unknown>
  handoff: (request: FullAutoRunHandoffIpcRequest) => Promise<unknown>
  report: (runRef: string) => Promise<unknown>
  receipt: (runRef: string) => Promise<unknown>
}>

export const unavailableFullAutoRunRendererHost: FullAutoRunRendererHost = {
  list: () => Promise.resolve({ runs: [], resolvedWorkspaceRef: null }),
  start: () => Promise.resolve(unavailableFullAutoRunOutcome()),
  get: () => Promise.resolve(unavailableFullAutoRunOutcome()),
  pause: () => Promise.resolve(unavailableFullAutoRunOutcome()),
  resume: () => Promise.resolve(unavailableFullAutoRunOutcome()),
  stop: () => Promise.resolve(unavailableFullAutoRunOutcome()),
  retryNow: () => Promise.resolve(unavailableFullAutoRunOutcome()),
  handoff: () => Promise.resolve(unavailableFullAutoRunOutcome()),
  report: () => Promise.resolve(unavailableFullAutoRunOutcome()),
  receipt: () => Promise.resolve(unavailableFullAutoRunOutcome()),
}
