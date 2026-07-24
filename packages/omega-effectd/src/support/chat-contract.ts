import { Exit, Schema } from "@effect-native/core/effect"
import type { LiveAgentGraphPresentation } from "./agent-graph-presentation.ts"
import { WorkbenchItemSchema } from "./workbench-item-contract.ts"

/**
 * Canonical conversation ordering: newest-created first across every host.
 * Legacy projections without a creation timestamp fall back to the timestamp
 * they already carry, while the id tie-break keeps the order deterministic.
 */
export const compareDesktopThreadsByCreatedAt = (left: DesktopThread, right: DesktopThread): number =>
  (right.createdAt ?? right.updatedAt).localeCompare(left.createdAt ?? left.updatedAt) ||
  left.id.localeCompare(right.id)

export const DesktopThreadsChannel = "openagents-desktop/threads" as const
export const DesktopNewThreadChannel = "openagents-desktop/thread-new" as const
export const DesktopOpenThreadChannel = "openagents-desktop/thread-open" as const
export const DesktopHydrateThreadChannel = "openagents-desktop/thread-hydrate" as const
export const DesktopChatTurnChannel = "openagents-desktop/chat-turn" as const
export const DesktopLocalTurnRecoveryUpdateChannel = "openagents-desktop/local-turn-recovery-update" as const
export const DesktopLocalThreadsChannel = "openagents-desktop/history-local-threads" as const
export const DesktopResumeLocalThreadChannel = "openagents-desktop/history-resume-local-thread" as const
export const DesktopForkHistoryThreadChannel = "openagents-desktop/history-fork-thread" as const
export const DesktopRenameLocalThreadChannel = "openagents-desktop/history-rename-local-thread" as const

/**
 * Per-message host metadata (#8712, EP250: "if I click on the message, I see
 * the metadata of the message in the right sidebar"). Additive and optional:
 * only facts the host actually observed are recorded — the claude-local lane
 * stamps the SDK-reported effective model, the lane name, the account ref
 * used, the turn ref, the exact reported token total, and the wall-clock
 * duration. Bounded public-safe strings only; never prompts, paths, tokens,
 * or provider payloads.
 */
/**
 * Typed tool-trace facts for a system trace note (EP250, #8712: "improve the
 * UI of those tool calls so it's not just JSON stuff"). Carried alongside the
 * existing compact trace text so the renderer can build typed tool cards
 * without re-parsing display strings; the summary stays the same bounded,
 * redacted payload the text line carries.
 */
export const DesktopToolTracePhaseSchema = Schema.Literals(["started", "progress", "ok", "failed"])
export type DesktopToolTracePhase = typeof DesktopToolTracePhaseSchema.Type

export const DesktopToolTraceSchema = Schema.Struct({
  toolName: Schema.String.check(Schema.isMaxLength(120)),
  /** Provider invocation identity; lets concurrent same-name tools reconcile exactly. */
  itemRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  phase: DesktopToolTracePhaseSchema,
  summary: Schema.String.check(Schema.isMaxLength(400)),
  /**
   * Typed item payload (#8859): the structured fields the string summary
   * flattens (command cwd/exit/duration/output tail, per-file diffs, tool
   * args/results). Additive — absent on every pre-#8859 note, and the
   * summary stays populated so existing renderers keep working.
   */
  item: Schema.optional(WorkbenchItemSchema),
})
export type DesktopToolTrace = typeof DesktopToolTraceSchema.Type

export const DesktopMessageMetaSchema = Schema.Struct({
  lane: Schema.optional(Schema.String.check(Schema.isMaxLength(60))),
  model: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  accountRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  turnRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  requestId: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  // AFS-03 (#9081): the effective route disclosure for a kernel-driven turn —
  // selected/effective provider, lane placement, data destination, and usage
  // truth. Bounded public-safe labels only; never a helper path, url, or token.
  provider: Schema.optional(Schema.String.check(Schema.isMaxLength(60))),
  placement: Schema.optional(Schema.String.check(Schema.isMaxLength(60))),
  dataDestination: Schema.optional(Schema.String.check(Schema.isMaxLength(60))),
  usageTruth: Schema.optional(Schema.String.check(Schema.isMaxLength(60))),
  totalTokens: Schema.optional(Schema.NullOr(Schema.Number)),
  durationMs: Schema.optional(Schema.Number),
  trace: Schema.optional(DesktopToolTraceSchema),
  recovery: Schema.optional(Schema.Struct({
    state: Schema.Literals(["recovering", "interrupted", "completed"]),
    disposition: Schema.optional(Schema.Literals(["resumed_after_restart", "interrupted_by_restart"])),
    generation: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  })),
})
export type DesktopMessageMeta = typeof DesktopMessageMetaSchema.Type

/**
 * Interactive question card payload (EP250 scope addition: "make the question
 * UI too. Why not? proper effect native primitives and add some if needed.").
 * The question/option shapes mirror the FROZEN additive ClaudeLocalEvent
 * question_pending contract; the card status tracks the resolved outcome.
 */
export const DesktopQuestionOptionSchema = Schema.Struct({
  /** Canonical option identity for durable runtime interactions. */
  optionRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  label: Schema.String.check(Schema.isMaxLength(200)),
  description: Schema.optional(Schema.String.check(Schema.isMaxLength(400))),
})
export type DesktopQuestionOption = typeof DesktopQuestionOptionSchema.Type

export const DesktopQuestionSchema = Schema.Struct({
  /** Canonical question identity for durable runtime interactions. */
  questionRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  question: Schema.String.check(Schema.isMaxLength(2_000)),
  header: Schema.String.check(Schema.isMaxLength(120)),
  options: Schema.Array(DesktopQuestionOptionSchema),
  multiSelect: Schema.Boolean,
})
export type DesktopQuestion = typeof DesktopQuestionSchema.Type

export const DesktopQuestionCardStatusSchema = Schema.Literals([
  "pending",
  "answered",
  "timeout",
  "denied",
  "resolved",
  "expired",
  "revoked",
])
export type DesktopQuestionCardStatus = typeof DesktopQuestionCardStatusSchema.Type

export const DesktopQuestionCardSchema = Schema.Struct({
  turnRef: Schema.String.check(Schema.isMaxLength(120)),
  threadRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  questionRef: Schema.String.check(Schema.isMaxLength(120)),
  status: DesktopQuestionCardStatusSchema,
  /** Absent on the frozen Claude-local bridge; canonical on Sync cards. */
  source: Schema.optional(Schema.Literal("runtime")),
  kind: Schema.optional(Schema.Literals(["provider_question", "tool_approval", "plan_review"])),
  decisionRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  questions: Schema.Array(DesktopQuestionSchema),
})
export type DesktopQuestionCard = typeof DesktopQuestionCardSchema.Type

/**
 * Runtime-capability transcript card payloads (EP250 wave-2, #8712). A system
 * note may carry ONE of these typed `runtime` payloads so the renderer draws a
 * plan/todo checklist (J2/J4), a delegate-child lifecycle card with an
 * Interrupt control (G4), or a queued-follow-up chip (A3) — projected from the
 * frozen additive ClaudeLocalEvent stream, never raw JSON. The glyph/model
 * vocabulary lives in `renderer/runtime-cards.ts`; the View render in
 * `renderer/shell.ts`, exactly like the tool/question cards.
 */
export const DesktopRuntimePlanEntrySchema = Schema.Struct({
  step: Schema.String.check(Schema.isMaxLength(400)),
  status: Schema.Literals(["pending", "in_progress", "completed"]),
})
const DESKTOP_RUNTIME_TRANSCRIPT_TEXT_LIMIT = 32_000
const DesktopRuntimeTranscriptActivityStatusSchema = Schema.Literals(["running", "completed", "failed"])
export const DesktopRuntimeTranscriptActivitySchema = Schema.Union([
  Schema.Struct({
    kind: Schema.Literal("command"),
    label: Schema.String.check(Schema.isMaxLength(120)),
    status: DesktopRuntimeTranscriptActivityStatusSchema,
    outputByteCount: Schema.optional(Schema.Number),
  }),
  Schema.Struct({
    kind: Schema.Literal("file_change"),
    label: Schema.String.check(Schema.isMaxLength(120)),
    status: DesktopRuntimeTranscriptActivityStatusSchema,
    fileChangeCount: Schema.Number,
    outputByteCount: Schema.optional(Schema.Number),
  }),
  Schema.Struct({
    kind: Schema.Literal("tool"),
    label: Schema.String.check(Schema.isMaxLength(120)),
    status: DesktopRuntimeTranscriptActivityStatusSchema,
    outputByteCount: Schema.optional(Schema.Number),
  }),
  Schema.Struct({
    kind: Schema.Literal("reasoning"),
    label: Schema.String.check(Schema.isMaxLength(120)),
    status: DesktopRuntimeTranscriptActivityStatusSchema,
  }),
  Schema.Struct({
    kind: Schema.Literal("notice"),
    label: Schema.String.check(Schema.isMaxLength(120)),
    status: DesktopRuntimeTranscriptActivityStatusSchema,
  }),
])
export type DesktopRuntimeTranscriptActivity = typeof DesktopRuntimeTranscriptActivitySchema.Type
export const DesktopRuntimeTranscriptEntrySchema = Schema.Struct({
  /** Stable safe-chain identity. It is absent on cards from older builds. */
  entryRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  role: Schema.Literals(["user", "assistant", "system", "tool"]),
  text: Schema.String.check(Schema.isMaxLength(DESKTOP_RUNTIME_TRANSCRIPT_TEXT_LIMIT)),
  /** Typed compact work metadata. It never contains commands, paths, or raw output. */
  activity: Schema.optional(DesktopRuntimeTranscriptActivitySchema),
})
export type DesktopRuntimeTranscriptEntry = typeof DesktopRuntimeTranscriptEntrySchema.Type
export const DesktopRuntimeCardSchema = Schema.Union([
  Schema.Struct({
    kind: Schema.Literal("plan"),
    entries: Schema.Array(DesktopRuntimePlanEntrySchema).check(Schema.isMaxLength(64)),
    /**
     * Free-form plan narrative (T8 #8865 unification): Codex's `plan`
     * ThreadItem variant (`{id, text, type: "plan"}`, collaboration-mode
     * write-ups) carries prose instead of a structured step list. Optional so
     * older persisted plan cards without it keep decoding.
     */
    prose: Schema.optional(Schema.String.check(Schema.isMaxLength(4_000))),
  }),
  Schema.Struct({
    kind: Schema.Literal("child"),
    turnRef: Schema.String.check(Schema.isMaxLength(120)),
    childRef: Schema.String.check(Schema.isMaxLength(120)),
    /** Provider-native parent identity for durable nested topology. */
    parentChildRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
    status: Schema.Literals(["running", "completed", "failed"]),
    title: Schema.String.check(Schema.isMaxLength(400)),
    detail: Schema.String.check(Schema.isMaxLength(400)),
    /** Optional for backward compatibility with already-persisted child cards. */
    transcript: Schema.optional(
      Schema.Array(DesktopRuntimeTranscriptEntrySchema).check(Schema.isMaxLength(128)),
    ),
    steered: Schema.NullOr(
      Schema.Struct({
        action: Schema.Literals(["message", "interrupt"]),
        outcome: Schema.Literals(["interrupted", "delivered", "unsupported", "not_found"]),
        detail: Schema.String.check(Schema.isMaxLength(400)),
      }),
    ),
  }),
  Schema.Struct({
    kind: Schema.Literal("queue"),
    turnRef: Schema.String.check(Schema.isMaxLength(120)),
    queueRef: Schema.String.check(Schema.isMaxLength(120)),
    position: Schema.Number,
  }),
])
export type DesktopRuntimeCard = typeof DesktopRuntimeCardSchema.Type

export const DesktopMessageSchema = Schema.Struct({
  key: Schema.String,
  role: Schema.Literals(["user", "assistant", "system"]),
  text: Schema.String,
  timestamp: Schema.String,
  meta: Schema.optional(DesktopMessageMetaSchema),
  /** Present only on interactive question notes (EP250 question cards). */
  question: Schema.optional(DesktopQuestionCardSchema),
  /** Present only on runtime-capability cards (EP250 wave-2 plan/child/queue). */
  runtime: Schema.optional(DesktopRuntimeCardSchema),
})
export type DesktopMessage = typeof DesktopMessageSchema.Type

export const DesktopThreadSchema = Schema.Struct({
  id: Schema.String,
  title: Schema.String,
  createdAt: Schema.optional(Schema.String),
  updatedAt: Schema.String,
  cwd: Schema.optional(Schema.String),
  model: Schema.optional(Schema.String),
  notes: Schema.Array(DesktopMessageSchema),
})

/**
 * Renderer-local live context/usage meter snapshot (T11 #8868). Mirrors the
 * exact wire fields from `thread/tokenUsage/updated` (context-window
 * composition) and `account/rateLimits/updated` (rolling rate-limit
 * windows) — never a synthesized value. Carried on `DesktopThread` the same
 * way `agentGraph` is: a renderer-local projection field, not part of the
 * `DesktopThreadSchema` IPC boundary (the boundary-crossing shape lives on
 * the `meter_updated` ClaudeLocalEvent in `claude-local-contract.ts`).
 */
export type DesktopMeterRateLimitWindow = Readonly<{
  label: "primary" | "secondary"
  usedPercent: number
  resetsAt?: number
  windowDurationMins?: number
}>
export type DesktopMeterSnapshot = Readonly<{
  inputTokens?: number
  cachedInputTokens?: number
  outputTokens?: number
  reasoningTokens?: number
  totalTokens?: number
  rateLimits?: ReadonlyArray<DesktopMeterRateLimitWindow>
}>

export type DesktopThread = typeof DesktopThreadSchema.Type & Readonly<{
  /** Renderer-local projection of confirmed Runtime Gateway v8 graph data. */
  agentGraph?: LiveAgentGraphPresentation
  /** Renderer-local live meter snapshot for the header/rail ContextMeter mount. */
  meter?: DesktopMeterSnapshot
}>

export const DesktopThreadRequestSchema = Schema.Struct({ id: Schema.String })
export const DesktopTurnRequestSchema = Schema.Struct({ id: Schema.String, message: Schema.String })
export type DesktopTurnRequest = typeof DesktopTurnRequestSchema.Type

/** H1: select an existing app-local Claude thread. Reusing its exact ref is
 * what reaches claude-local-runtime's existing per-thread SDK resume map. */
export const DesktopResumeLocalThreadRequestSchema = Schema.Struct({
  threadRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
})
export type DesktopResumeLocalThreadRequest = typeof DesktopResumeLocalThreadRequestSchema.Type

export const DesktopRenameLocalThreadRequestSchema = Schema.Struct({
  threadRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  title: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
})
export type DesktopRenameLocalThreadRequest = typeof DesktopRenameLocalThreadRequestSchema.Type

export const decodeDesktopRenameLocalThreadRequest = (
  value: unknown,
): DesktopRenameLocalThreadRequest | null => {
  const decoded = decode(DesktopRenameLocalThreadRequestSchema, value) as DesktopRenameLocalThreadRequest | null
  if (decoded === null) return null
  const title = decoded.title.trim()
  return title === "" ? null : { ...decoded, title }
}

export const DesktopRenameLocalThreadResultSchema = Schema.Union([
  Schema.Struct({ ok: Schema.Literal(true), thread: DesktopThreadSchema }),
  Schema.Struct({ ok: Schema.Literal(false), error: Schema.String.check(Schema.isMaxLength(160)) }),
])
export type DesktopRenameLocalThreadResult = typeof DesktopRenameLocalThreadResultSchema.Type

/** H2: refs-only fork request. Main re-reads provider history and constructs
 * the bounded seed; renderer transcript text is never mutation authority. */
export const DesktopForkHistoryThreadRequestSchema = Schema.Struct({
  sourceThreadRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(256)),
  throughSequence: Schema.NullOr(
    Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  ),
})
export type DesktopForkHistoryThreadRequest = typeof DesktopForkHistoryThreadRequestSchema.Type

export const decode = (schema: any, value: unknown): unknown | null => {
  const result = Schema.decodeUnknownExit(schema)(value)
  return Exit.isSuccess(result) ? result.value : null
}
