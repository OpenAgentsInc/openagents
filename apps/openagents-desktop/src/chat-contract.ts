import { Exit, Schema } from "@effect-native/core/effect"
import type { LiveAgentGraphPresentation } from "./agent-graph-presentation.ts"

export const DesktopThreadsChannel = "openagents-desktop/threads" as const
export const DesktopNewThreadChannel = "openagents-desktop/thread-new" as const
export const DesktopOpenThreadChannel = "openagents-desktop/thread-open" as const
export const DesktopHydrateThreadChannel = "openagents-desktop/thread-hydrate" as const
export const DesktopChatTurnChannel = "openagents-desktop/chat-turn" as const

/**
 * Per-message host metadata (#8712, EP250: "if I click on the message, I see
 * the metadata of the message in the right sidebar"). Additive and optional:
 * only facts the host actually observed are recorded — the fable-local lane
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
export const DesktopToolTracePhaseSchema = Schema.Literals(["started", "ok", "failed"])
export type DesktopToolTracePhase = typeof DesktopToolTracePhaseSchema.Type

export const DesktopToolTraceSchema = Schema.Struct({
  toolName: Schema.String.check(Schema.isMaxLength(120)),
  phase: DesktopToolTracePhaseSchema,
  summary: Schema.String.check(Schema.isMaxLength(400)),
})
export type DesktopToolTrace = typeof DesktopToolTraceSchema.Type

export const DesktopMessageMetaSchema = Schema.Struct({
  lane: Schema.optional(Schema.String.check(Schema.isMaxLength(60))),
  model: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  accountRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  turnRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  requestId: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  totalTokens: Schema.optional(Schema.NullOr(Schema.Number)),
  durationMs: Schema.optional(Schema.Number),
  trace: Schema.optional(DesktopToolTraceSchema),
})
export type DesktopMessageMeta = typeof DesktopMessageMetaSchema.Type

/**
 * Interactive question card payload (EP250 scope addition: "make the question
 * UI too. Why not? proper effect native primitives and add some if needed.").
 * The question/option shapes mirror the FROZEN additive FableLocalEvent
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
  /** Absent on the frozen Fable-local bridge; canonical on Sync cards. */
  source: Schema.optional(Schema.Literal("runtime")),
  kind: Schema.optional(Schema.Literals(["provider_question", "tool_approval", "plan_review"])),
  decisionRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
  questions: Schema.Array(DesktopQuestionSchema),
})
export type DesktopQuestionCard = typeof DesktopQuestionCardSchema.Type

export const DesktopMessageSchema = Schema.Struct({
  key: Schema.String,
  role: Schema.Literals(["user", "assistant", "system"]),
  text: Schema.String,
  timestamp: Schema.String,
  meta: Schema.optional(DesktopMessageMetaSchema),
  /** Present only on interactive question notes (EP250 question cards). */
  question: Schema.optional(DesktopQuestionCardSchema),
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
export type DesktopThread = typeof DesktopThreadSchema.Type & Readonly<{
  /** Renderer-local projection of confirmed Runtime Gateway v8 graph data. */
  agentGraph?: LiveAgentGraphPresentation
}>

export const DesktopThreadRequestSchema = Schema.Struct({ id: Schema.String })
export const DesktopTurnRequestSchema = Schema.Struct({ id: Schema.String, message: Schema.String })
export type DesktopTurnRequest = typeof DesktopTurnRequestSchema.Type

export const decode = (schema: any, value: unknown): unknown | null => {
  const result = Schema.decodeUnknownExit(schema)(value)
  return Exit.isSuccess(result) ? result.value : null
}
