/**
 * Fable local lane contract (#8712).
 *
 * In local (not-signed-in) mode the "Fable" harness chip must run a REAL
 * streaming Claude turn on this machine — never the legacy cloud gateway and
 * never another provider (the no-silent-substitution law). These channels
 * carry that lane across the Electron boundary:
 *
 * - `openagents:fable-local:availability` (invoke, no args): typed lane
 *   availability, probed from the same isolated `~/.claude-pylon-*` sibling
 *   account homes the Pylon supervisor dispatches to. Never the default
 *   `~/.claude` home.
 * - `openagents:fable-local:start` (invoke): starts one turn. Main appends
 *   the user message to its own thread store and reads prior history from
 *   that store — the renderer cannot inject synthetic history. Resolves with
 *   the same `{ ok, thread?, error? }` shape as the legacy chat channel once
 *   the turn finishes.
 * - `openagents:fable-local:event` (webContents.send): typed incremental
 *   events `{ turnRef, event }` while the turn streams. Every payload is
 *   bounded and path-redacted before it crosses this boundary.
 * - `openagents:fable-local:interrupt` (invoke): aborts a running turn.
 */
import { Schema } from "@effect-native/core/effect"

import { DesktopThreadSchema, decode } from "./chat-contract.ts"

export const FableLocalAvailabilityChannel = "openagents:fable-local:availability" as const
export const FableLocalStartChannel = "openagents:fable-local:start" as const
export const FableLocalInterruptChannel = "openagents:fable-local:interrupt" as const
export const FableLocalEventChannel = "openagents:fable-local:event" as const

/** Bound on a single streamed text-delta event payload. */
export const FABLE_LOCAL_DELTA_LIMIT = 2_000
/** Bound on tool summaries and failure detail crossing the boundary. */
export const FABLE_LOCAL_SUMMARY_LIMIT = 400
/** Bound on the persisted final assistant message text. */
export const FABLE_LOCAL_FINAL_TEXT_LIMIT = 32_000

export const FableLocalAvailabilitySchema = Schema.Union([
  Schema.Struct({
    state: Schema.Literal("available"),
    accountRef: Schema.String,
  }),
  Schema.Struct({
    state: Schema.Literal("unavailable"),
    reason: Schema.Literals(["no_claude_account", "sdk_unavailable"]),
  }),
])
export type FableLocalAvailability = typeof FableLocalAvailabilitySchema.Type

export const FableLocalFailureReasonSchema = Schema.Literals([
  "no_claude_account",
  "sdk_unavailable",
  "budget_exceeded",
  "interrupted",
  "timeout",
  "session_failed",
  /**
   * The SDK init reported an effective model outside the Fable family
   * ("IT HAS TO BE FABLE"): the turn fails typed with requested vs effective
   * in the detail, no substituted output is ever streamed as Fable, and the
   * lane never rotates accounts on it (rotation is for account/session
   * failures before content only).
   */
  "model_substituted",
])
export type FableLocalFailureReason = typeof FableLocalFailureReasonSchema.Type

/**
 * Exact token split (#8712 Lane C) shared by turn_completed (SDK result
 * usage) and child_completed (codex `turn.completed` usage; total = input +
 * output + reasoning, cached reported separately).
 */
export const FableChildUsageSchema = Schema.Struct({
  inputTokens: Schema.Number,
  cachedInputTokens: Schema.Number,
  outputTokens: Schema.Number,
  reasoningTokens: Schema.Number,
  totalTokens: Schema.Number,
})
export type FableChildUsage = typeof FableChildUsageSchema.Type

export const FableLocalEventSchema = Schema.Union([
  Schema.Struct({
    kind: Schema.Literal("turn_started"),
    /**
     * The persisted thread snapshot with the user message already appended
     * (main attaches it before forwarding the runtime's turn_started), so the
     * renderer can project progressive updates without a second round trip.
     */
    thread: Schema.optional(DesktopThreadSchema),
  }),
  Schema.Struct({
    kind: Schema.Literal("text_delta"),
    text: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_DELTA_LIMIT)),
  }),
  Schema.Struct({
    kind: Schema.Literal("tool_use"),
    toolName: Schema.String.check(Schema.isMaxLength(120)),
    summary: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
  }),
  Schema.Struct({
    kind: Schema.Literal("tool_result"),
    toolName: Schema.String.check(Schema.isMaxLength(120)),
    ok: Schema.Boolean,
    summary: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
  }),
  /**
   * Effective-model visibility: the model the SDK init actually reported for
   * this turn. Capability-truthful — the renderer shows model identity from
   * this event, never from the "Fable" brand alone.
   */
  Schema.Struct({
    kind: Schema.Literal("model_effective"),
    model: Schema.String.check(Schema.isMaxLength(120)),
  }),
  Schema.Struct({
    kind: Schema.Literal("turn_completed"),
    totalTokens: Schema.NullOr(Schema.Number),
    /**
     * Additive (#8712 Lane C): the account the completed turn ran on and the
     * exact SDK usage split, so the session usage ledger can attribute exact
     * tokens per account. Both optional — older emitters stay schema-valid.
     */
    accountRef: Schema.optional(Schema.String),
    usage: Schema.optional(FableChildUsageSchema),
  }),
  Schema.Struct({
    kind: Schema.Literal("turn_failed"),
    reason: FableLocalFailureReasonSchema,
    detail: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
  }),
  // -------------------------------------------------------------------------
  // Codex sub-agent (child) lifecycle (#8712 Lane C) — additive. The renderer
  // needs no new components today (the delegate tool's tool_use/tool_result
  // lines already render); these events exist so the UI can project child
  // cards later and so main can feed the session usage ledger. Every field is
  // bounded and public-safe (account refs are already renderer-visible via
  // the fleet projection; no paths, prompts, or credentials).
  // -------------------------------------------------------------------------
  Schema.Struct({
    kind: Schema.Literal("child_started"),
    childRef: Schema.String.check(Schema.isMaxLength(120)),
    accountRef: Schema.optional(Schema.String),
    summary: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
  }),
  Schema.Struct({
    kind: Schema.Literal("child_activity"),
    childRef: Schema.String.check(Schema.isMaxLength(120)),
    /**
     * "item": a completed child stream item. "account_reconnect_required":
     * an account with revoked credentials was skipped VISIBLY (typed event,
     * never a silent rotation) before the next registered Codex home ran.
     */
    activity: Schema.Literals(["item", "account_reconnect_required"]),
    accountRef: Schema.optional(Schema.String),
    summary: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
  }),
  Schema.Struct({
    kind: Schema.Literal("child_completed"),
    childRef: Schema.String.check(Schema.isMaxLength(120)),
    accountRef: Schema.String,
    summary: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
    usage: Schema.NullOr(FableChildUsageSchema),
    durationMs: Schema.Number,
  }),
  Schema.Struct({
    kind: Schema.Literal("child_failed"),
    childRef: Schema.String.check(Schema.isMaxLength(120)),
    accountRef: Schema.NullOr(Schema.String),
    reason: Schema.Literals([
      "account_reconnect_required",
      "no_codex_account",
      "child_timeout",
      "child_failed",
    ]),
    detail: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
  }),
])
export type FableLocalEvent = typeof FableLocalEventSchema.Type

export const FableLocalEventEnvelopeSchema = Schema.Struct({
  turnRef: Schema.String,
  event: FableLocalEventSchema,
})
export type FableLocalEventEnvelope = typeof FableLocalEventEnvelopeSchema.Type

export const FableLocalStartRequestSchema = Schema.Struct({
  turnRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  threadRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(8_000)),
})
export type FableLocalStartRequest = typeof FableLocalStartRequestSchema.Type

export const FableLocalInterruptRequestSchema = Schema.Struct({
  turnRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
})
export type FableLocalInterruptRequest = typeof FableLocalInterruptRequestSchema.Type

export const decodeFableLocalStartRequest = (value: unknown): FableLocalStartRequest | null =>
  decode(FableLocalStartRequestSchema, value) as FableLocalStartRequest | null

export const decodeFableLocalInterruptRequest = (
  value: unknown,
): FableLocalInterruptRequest | null =>
  decode(FableLocalInterruptRequestSchema, value) as FableLocalInterruptRequest | null

export const decodeFableLocalEventEnvelope = (value: unknown): FableLocalEventEnvelope | null =>
  decode(FableLocalEventEnvelopeSchema, value) as FableLocalEventEnvelope | null

export const decodeFableLocalAvailability = (value: unknown): FableLocalAvailability | null =>
  decode(FableLocalAvailabilitySchema, value) as FableLocalAvailability | null

/**
 * One compact trace line per tool event — the SAME text in the renderer's
 * live stream and in the persisted thread notes main appends, so the
 * transcript does not change shape when the turn finalizes.
 */
export const fableLocalTraceNoteText = (
  event: Extract<FableLocalEvent, { kind: "tool_use" | "tool_result" }>,
): string => {
  const status = event.kind === "tool_use" ? "started" : event.ok ? "ok" : "failed"
  const summary = event.summary.trim() === "" ? "" : ` · ${event.summary.trim()}`
  return `${event.toolName} · ${status}${summary}`
}

/**
 * Effective-model caption rendered as a transcript trace line above the
 * assistant reply (e.g. "Fable · claude-fable-5"). The model half is the
 * SDK-reported effective model, so the caption is capability-truthful even
 * though it also carries the lane brand.
 */
export const fableLocalModelNoteText = (model: string): string => `Fable · ${model}`

/** Renderer-facing copy for a typed lane failure — no provider text leaks. */
export const fableLocalFailureMessage = (
  reason: FableLocalFailureReason,
  detail: string,
): string => {
  const suffix = detail.trim() === "" ? "" : ` (${detail.trim()})`
  switch (reason) {
    case "no_claude_account":
      return "Fable is unavailable: no linked Claude account home found on this machine. No message was routed to any other lane."
    case "sdk_unavailable":
      return `The local Claude runtime could not start${suffix}. No message was routed to any other lane.`
    case "budget_exceeded":
      return "The local Fable turn hit its turn budget before finishing."
    case "interrupted":
      return "The local Fable turn was interrupted."
    case "timeout":
      return "The local Fable turn timed out."
    case "session_failed":
      return `The local Fable turn failed${suffix}.`
    case "model_substituted":
      return `Fable refused a substituted model${suffix}. No substituted output was shown as Fable.`
  }
}
