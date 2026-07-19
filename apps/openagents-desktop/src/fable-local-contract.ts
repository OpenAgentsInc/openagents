/**
 * Fable local lane contract (#8712).
 *
 * In local (not-signed-in) mode the "Fable" harness chip must run a REAL
 * streaming Claude turn on this machine — never the legacy cloud gateway and
 * never another provider (the no-silent-substitution law). These channels
 * carry that lane across the Electron boundary:
 *
 * - `openagents:fable-local:availability` (invoke, no args): typed lane
 *   availability, probing the ordinary authenticated `~/.claude` session
 *   first and the isolated `~/.claude-pylon-*` homes used by the Pylon
 *   supervisor as fallback capacity.
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

import {
  DesktopThreadSchema,
  decode,
  type DesktopToolTrace,
} from "./chat-contract.ts"
import { LocalSkillInvocationSchema } from "./plugin-config-contract.ts"
import { WorkbenchItemSchema } from "./workbench-item-contract.ts"

export const FableLocalAvailabilityChannel = "openagents:fable-local:availability" as const
export const FableLocalStartChannel = "openagents:fable-local:start" as const
export const FableLocalInterruptChannel = "openagents:fable-local:interrupt" as const
export const FableLocalEventChannel = "openagents:fable-local:event" as const
/**
 * Image file picker (capability I1). The renderer invokes this to open the
 * native file dialog in MAIN (never a renderer filesystem read); main reads the
 * chosen images, bounds and base64-encodes them, and returns the decoded
 * attachments. Drop/paste `File` objects stay renderer-side (already in-memory).
 */
export const FableLocalPickImagesChannel = "openagents:fable-local:pick-images" as const
/**
 * Interactive question flow (EP250): the renderer answers a pending
 * AskUserQuestion via this invoke channel (`fableLocal.answerQuestion`).
 * Resolves `true` when a pending question with that turnRef/questionRef
 * accepted the answers; `false` is a typed rejection (unknown ref, already
 * settled, or no answer matched a question) and the question stays pending.
 */
export const FableLocalAnswerQuestionChannel = "openagents:fable-local:answer-question" as const
/**
 * Runtime-capability channels (EP250 wave-1 substrate; renderer UI is wave-2).
 * These carry additive control across the Electron boundary and default to
 * NO-OP behavior when the renderer never calls them, so current turn behavior
 * is unchanged.
 *
 * - `openagents:fable-local:steer-child` (invoke): steer/interrupt a running
 *   delegate child ({ turnRef, childRef, action, body? }).
 * - `openagents:fable-local:queue-followup` (invoke): enqueue a follow-up
 *   message while a turn for the thread is streaming
 *   ({ threadRef, message }); promoted at the next idle boundary.
 */
export const FableLocalSteerChildChannel = "openagents:fable-local:steer-child" as const
export const FableLocalQueueFollowupChannel = "openagents:fable-local:queue-followup" as const

/** Bound on a single streamed text-delta event payload. */
export const FABLE_LOCAL_DELTA_LIMIT = 2_000
/** Bound on tool summaries and failure detail crossing the boundary. */
export const FABLE_LOCAL_SUMMARY_LIMIT = 400
/** Bound on the persisted final assistant message text. */
export const FABLE_LOCAL_FINAL_TEXT_LIMIT = 32_000
/** Max plan/todo entries surfaced in one plan_updated event. */
export const FABLE_LOCAL_PLAN_ENTRY_LIMIT = 64
/** Max rate-limit windows carried per meter_updated event (T11 #8868;
 * Codex's `account/rateLimits/updated` reports at most `primary`+`secondary`). */
export const FABLE_LOCAL_RATE_LIMIT_WINDOW_LIMIT = 4
/** Max user-configured MCP servers accepted per turn (bounded passthrough). */
export const FABLE_LOCAL_MCP_SERVER_LIMIT = 16
/** Bound on a queued follow-up message crossing the boundary. */
export const FABLE_LOCAL_FOLLOWUP_MESSAGE_LIMIT = 8_000
/**
 * Image input bounds (capability I1). The renderer holds each attachment as
 * base64 (no `data:` prefix) and passes it across the start boundary; main
 * threads it into the SDK image content block (Fable) or writes it to the turn
 * workspace and passes `-i <path>` (Codex). The four media types are exactly
 * the Anthropic `Base64ImageSource` set (sdk.d.ts `media_type`).
 */
export const FABLE_LOCAL_IMAGE_COUNT_LIMIT = 8
/** Max decoded bytes per attachment (10 MB) — oversize is rejected honestly. */
export const FABLE_LOCAL_IMAGE_BYTES_LIMIT = 10 * 1024 * 1024
/**
 * Bound on the base64 string crossing the boundary. Base64 expands ~4/3 over
 * the raw bytes; this caps the encoded length with headroom so an oversize
 * blob fails schema decode instead of silently crossing.
 */
export const FABLE_LOCAL_IMAGE_DATA_LIMIT =
  Math.ceil((FABLE_LOCAL_IMAGE_BYTES_LIMIT * 4) / 3) + 1_024
/** Accepted image media types (Anthropic Base64ImageSource + codex -i). */
export const FABLE_LOCAL_IMAGE_MEDIA_TYPES = [
  "image/png",
  "image/jpeg",
  "image/webp",
  "image/gif",
] as const

export const FableLocalImageMediaTypeSchema = Schema.Literals(FABLE_LOCAL_IMAGE_MEDIA_TYPES)
export type FableLocalImageMediaType = typeof FableLocalImageMediaTypeSchema.Type

/**
 * One image attachment crossing the start boundary (capability I1). `data` is
 * raw base64 (no `data:` URL prefix); `name` is a bounded display label only
 * (never a filesystem path — the renderer never reads arbitrary files).
 */
export const FableLocalImageAttachmentSchema = Schema.Struct({
  mediaType: FableLocalImageMediaTypeSchema,
  data: Schema.String.check(
    Schema.isMinLength(1),
    Schema.isMaxLength(FABLE_LOCAL_IMAGE_DATA_LIMIT),
  ),
  name: Schema.optional(Schema.String.check(Schema.isMaxLength(256))),
})
export type FableLocalImageAttachment = typeof FableLocalImageAttachmentSchema.Type

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
  // Codex-local lane reasons (EP250 codex-first-class, additive): the direct
  // local Codex chat lane shares this envelope so the existing renderer
  // stream path renders codex turns identically. `account_reconnect_required`
  // is also emitted by Fable when Claude reports an expired/disabled account;
  // `no_codex_account` remains Codex-only.
  "no_codex_account",
  "account_reconnect_required",
  "incompatible_workflow",
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

/**
 * One AskUserQuestion question as projected to the renderer (EP250 question
 * flow). Mirrors the SDK's AskUserQuestionInput shape (question, short
 * header chip, 2-4 options with label + description, multiSelect) with every
 * string bounded/redacted before it crosses the boundary.
 */
export const FableLocalQuestionOptionSchema = Schema.Struct({
  label: Schema.String.check(Schema.isMaxLength(200)),
  description: Schema.optional(Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT))),
})
export type FableLocalQuestionOption = typeof FableLocalQuestionOptionSchema.Type

export const FableLocalQuestionSchema = Schema.Struct({
  question: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
  header: Schema.String.check(Schema.isMaxLength(120)),
  options: Schema.Array(FableLocalQuestionOptionSchema).check(Schema.isMaxLength(4)),
  multiSelect: Schema.Boolean,
})
export type FableLocalQuestion = typeof FableLocalQuestionSchema.Type

/**
 * One plan/todo entry (EP250 J2/J4). Sourced from the SDK's high-frequency
 * TodoWrite tool (`TodoWriteInput.todos[]` = `{ content, status, activeForm }`,
 * receipted sdk-tools.d.ts) — the 1,617-observation "update_plan"/todo signal
 * from the daily-coding audit. `step` is the todo `content` (bounded/redacted);
 * `status` mirrors the SDK's exact three-state enum.
 */
export const FableLocalPlanEntrySchema = Schema.Struct({
  step: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
  status: Schema.Literals(["pending", "in_progress", "completed"]),
})
export type FableLocalPlanEntry = typeof FableLocalPlanEntrySchema.Type

/**
 * One rolling rate-limit window from Codex's `account/rateLimits/updated`
 * (T11 #8868). `label` is bounded to the exact two window names the wire
 * reports (`primary`/`secondary`) rather than a free string — never invented
 * by this lane. `usedPercent`/`resetsAt`/`windowDurationMins` are the exact
 * wire values (`ServerNotification__RateLimitWindow`); a field the server
 * omitted stays absent here too.
 */
export const FableLocalRateLimitWindowSchema = Schema.Struct({
  label: Schema.Literals(["primary", "secondary"]),
  usedPercent: Schema.Number,
  resetsAt: Schema.optional(Schema.Number),
  windowDurationMins: Schema.optional(Schema.Number),
})
export type FableLocalRateLimitWindow = typeof FableLocalRateLimitWindowSchema.Type

export const FableLocalEventSchema = Schema.Union([
  Schema.Struct({
    kind: Schema.Literal("composer_admission"),
    state: Schema.Literals(["active_steerable", "active_nonsteerable", "interrupting", "repairing"]),
    activeTurnId: Schema.NullOr(Schema.String.check(Schema.isMaxLength(120))),
    reason: Schema.NullOr(Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT))),
  }),
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
    itemRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
    /** Typed item payload (#8859, additive): structured tool fields the
     * bounded summary string flattens. Absent on pre-#8859 emitters. */
    item: Schema.optional(WorkbenchItemSchema),
  }),
  Schema.Struct({
    kind: Schema.Literal("tool_progress"),
    toolName: Schema.String.check(Schema.isMaxLength(120)),
    summary: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
    itemRef: Schema.String.check(Schema.isMaxLength(120)),
    item: WorkbenchItemSchema,
  }),
  Schema.Struct({
    kind: Schema.Literal("tool_result"),
    toolName: Schema.String.check(Schema.isMaxLength(120)),
    ok: Schema.Boolean,
    summary: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
    itemRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
    /** Typed item payload (#8859, additive): completion-side structured
     * fields (exit code, duration, output tail, diffs, results). */
    item: Schema.optional(WorkbenchItemSchema),
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
  /**
   * Reasoning treatment (EP250 codex-first-class, additive): the codex-local
   * lane surfaces completed `reasoning` items as bounded summaries. The
   * renderer projects them as compact system trace lines ("Reasoning · …"),
   * the same treatment the runtime timeline gives reasoning items.
   */
  Schema.Struct({
    kind: Schema.Literal("reasoning"),
    text: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
  }),
  /**
   * Typed, VISIBLE lane notice (EP250 codex-first-class, additive): the
   * codex-local lane's account rotation announcements ("account X needs
   * reconnect — rotating…"). Rotation is never silent — the transcript
   * carries the notice as a compact system line.
   */
  Schema.Struct({
    kind: Schema.Literal("lane_notice"),
    text: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
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
    /** Exact provider child whose thread caused this child, absent for root children. */
    parentChildRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
    accountRef: Schema.optional(Schema.String),
    summary: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
    /** Exact bounded instruction sent to the child, for its transcript. */
    prompt: Schema.optional(Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_FINAL_TEXT_LIMIT))),
  }),
  Schema.Struct({
    kind: Schema.Literal("child_activity"),
    childRef: Schema.String.check(Schema.isMaxLength(120)),
    parentChildRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
    /**
     * "item": a completed child stream item. "account_reconnect_required":
     * an account with rejected credentials was skipped VISIBLY (typed event,
     * never a silent rotation) before the next candidate Codex home ran.
     * "pre_content_failure_rotated": a NON-auth pre-content failure was
     * rotated past (EP250 broadened rotation) — equally typed and visible.
     */
    activity: Schema.Literals([
      "item",
      "account_reconnect_required",
      "pre_content_failure_rotated",
    ]),
    accountRef: Schema.optional(Schema.String),
    summary: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
  }),
  Schema.Struct({
    kind: Schema.Literal("child_completed"),
    childRef: Schema.String.check(Schema.isMaxLength(120)),
    parentChildRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
    accountRef: Schema.String,
    summary: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
    /** Exact bounded child answer, rather than the compact card summary. */
    response: Schema.optional(Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_FINAL_TEXT_LIMIT))),
    usage: Schema.NullOr(FableChildUsageSchema),
    durationMs: Schema.Number,
  }),
  Schema.Struct({
    kind: Schema.Literal("child_failed"),
    childRef: Schema.String.check(Schema.isMaxLength(120)),
    parentChildRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
    accountRef: Schema.NullOr(Schema.String),
    reason: Schema.Literals([
      "account_reconnect_required",
      "no_codex_account",
      "child_timeout",
      "child_failed",
    ]),
    detail: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
  }),
  // -------------------------------------------------------------------------
  // Interactive question flow (EP250, owner scope change: "make the question
  // UI too") — additive. AskUserQuestion is a REAL affordance in this lane:
  // the runtime parks the tool call on the SDK canUseTool callback, emits
  // question_pending so the renderer can show a card, and resolves it when
  // the user answers via the answer-question channel (or on timeout/turn
  // end, honestly typed). Every string is bounded and path-redacted.
  // -------------------------------------------------------------------------
  Schema.Struct({
    kind: Schema.Literal("question_pending"),
    /** Stable per AskUserQuestion invocation within the turn. */
    questionRef: Schema.String.check(Schema.isMaxLength(120)),
    interactionKind: Schema.optional(Schema.Literals(["provider_question", "tool_approval", "plan_review"])),
    decisionRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
    questions: Schema.Array(FableLocalQuestionSchema).check(Schema.isMaxLength(4)),
  }),
  Schema.Struct({
    kind: Schema.Literal("question_resolved"),
    questionRef: Schema.String.check(Schema.isMaxLength(120)),
    /**
     * "answered": the user's selection flowed back to the model.
     * "timeout": no answer inside the question window — the tool was denied
     * gracefully (the model is told to proceed without the input).
     * "denied": the turn ended (interrupt/failure/dispose) before an answer.
     */
    outcome: Schema.Literals(["answered", "timeout", "denied"]),
  }),
  // -------------------------------------------------------------------------
  // Runtime-capability substrate (EP250 wave-1) — additive. The renderer that
  // draws these is a later (wave-2) lane; the runtime emits the typed events
  // and programmatic oracles assert them. Every string is bounded/redacted.
  // -------------------------------------------------------------------------
  /**
   * Plan/todo progress (J2/J4). Emitted whenever the model calls the SDK
   * TodoWrite tool; carries the full current todo list so the renderer can
   * replace-render it. Additive to the tool_use event the same call emits, so
   * transcripts still show the raw tool trace.
   */
  Schema.Struct({
    kind: Schema.Literal("plan_updated"),
    entries: Schema.Array(FableLocalPlanEntrySchema).check(
      Schema.isMaxLength(FABLE_LOCAL_PLAN_ENTRY_LIMIT),
    ),
    /**
     * Free-form plan narrative (T8 #8865 unification): additive so the
     * dropped `plan` ThreadItem (`{id, text, type: "plan"}` collaboration-mode
     * write-ups, which never carry structured entries) can ride the SAME
     * per-turn stable-key plan note instead of being silently discarded.
     */
    prose: Schema.optional(Schema.String.check(Schema.isMaxLength(4_000))),
  }),
  /**
   * Result of a steer-child control (G4). `interrupted`: the child's abort
   * was signaled. `unsupported`: neither a Codex exec child (non-interactive)
   * nor an SDK Agent subagent (no per-child message API) can receive a
   * mid-flight message — honest capability truth. `not_found`: no running
   * child matched the ref for that turn. `delivered` is reserved for a future
   * transport that can inject mid-flight input.
   */
  Schema.Struct({
    kind: Schema.Literal("child_steered"),
    childRef: Schema.String.check(Schema.isMaxLength(120)),
    action: Schema.Literals(["message", "interrupt"]),
    outcome: Schema.Literals(["interrupted", "delivered", "unsupported", "not_found"]),
    detail: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
  }),
  /**
   * A follow-up was enqueued while this turn was streaming (A3). Delivery
   * semantics are QUEUE-UNTIL-IDLE (not steer-at-boundary): the runtime holds
   * a single-string-prompt turn and cannot inject mid-stream, so the queued
   * message is promoted when the current turn ends. `position` is its 1-based
   * place in the thread queue.
   */
  Schema.Struct({
    kind: Schema.Literal("followup_queued"),
    queueRef: Schema.String.check(Schema.isMaxLength(120)),
    position: Schema.Number,
  }),
  /**
   * A queued follow-up is now ready to become the next turn (A3). Emitted on
   * the ending turn's stream at the idle boundary; the host/renderer starts a
   * fresh turn with `message`.
   */
  Schema.Struct({
    kind: Schema.Literal("followup_promoted"),
    queueRef: Schema.String.check(Schema.isMaxLength(120)),
    intentRef: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
    clientUserMessageId: Schema.optional(Schema.String.check(Schema.isMaxLength(120))),
    message: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_FOLLOWUP_MESSAGE_LIMIT)),
  }),
  /**
   * A user-configured MCP server (I2) could not be offered: either its config
   * failed bounded validation, or the SDK reported it failed/needs-auth/
   * disabled at init. The turn still completes — a bad server never crashes
   * the turn. `reason` is a bounded public-safe cause.
   */
  Schema.Struct({
    kind: Schema.Literal("mcp_server_unavailable"),
    name: Schema.String.check(Schema.isMaxLength(120)),
    reason: Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
  }),
  /**
   * Context/usage meter update (T11 #8868). Emitted ADDITIVELY alongside the
   * existing internal-accounting use of `thread/tokenUsage/updated`
   * (`codex-app-server-turn.ts` still tracks `CodexChildUsage` for its own
   * outcome classification) and from the previously-unconsumed
   * `account/rateLimits/updated` notification. Every field is optional and
   * carries the EXACT wire number — a field the server did not report in
   * this rolling update stays absent, never a synthesized `0`. At least one
   * of the token fields or `rateLimits` is present on any real emission.
   */
  Schema.Struct({
    kind: Schema.Literal("meter_updated"),
    inputTokens: Schema.optional(Schema.Number),
    cachedInputTokens: Schema.optional(Schema.Number),
    outputTokens: Schema.optional(Schema.Number),
    reasoningTokens: Schema.optional(Schema.Number),
    totalTokens: Schema.optional(Schema.Number),
    rateLimits: Schema.optional(
      Schema.Array(FableLocalRateLimitWindowSchema).check(
        Schema.isMaxLength(FABLE_LOCAL_RATE_LIMIT_WINDOW_LIMIT),
      ),
    ),
  }),
])
export type FableLocalEvent = typeof FableLocalEventSchema.Type

/**
 * Tracks whether a provider event creates a NEW visible timeline position.
 *
 * Several display-bearing event kinds are keyed updates: command progress and
 * completion update the invocation card, plan updates replace the turn's plan,
 * question resolution updates the pending question, and child lifecycle
 * updates replace the child card. Those events are visible, but they are not
 * new ordering boundaries. Splitting assistant text around them creates empty
 * visual seams because the card remains at its original position.
 *
 * Keep this state machine shared by the renderer and durable text persistence
 * so the live transcript cannot heal and then regress when the finalized
 * thread replaces it.
 */
export const makeTranscriptOrderingBoundaryTracker = (): ((event: FableLocalEvent) => boolean) => {
  const openToolsByRef = new Set<string>()
  const openToolsByName = new Map<string, number>()
  const runtimeKeys = new Set<string>()

  const openUnkeyedTool = (toolName: string): void => {
    openToolsByName.set(toolName, (openToolsByName.get(toolName) ?? 0) + 1)
  }
  const closeUnkeyedTool = (toolName: string): boolean => {
    const count = openToolsByName.get(toolName) ?? 0
    if (count === 0) return false
    if (count === 1) openToolsByName.delete(toolName)
    else openToolsByName.set(toolName, count - 1)
    return true
  }
  const upsert = (key: string): boolean => {
    if (runtimeKeys.has(key)) return false
    runtimeKeys.add(key)
    return true
  }

  return event => {
    switch (event.kind) {
      case "model_effective":
      case "reasoning":
      case "lane_notice":
        return true
      case "tool_use": {
        if (event.itemRef === undefined) {
          openUnkeyedTool(event.toolName)
          return true
        }
        const inserted = !openToolsByRef.has(event.itemRef)
        openToolsByRef.add(event.itemRef)
        return inserted
      }
      case "tool_progress": {
        if (event.itemRef === undefined) {
          if ((openToolsByName.get(event.toolName) ?? 0) > 0) return false
          openUnkeyedTool(event.toolName)
          return true
        }
        if (openToolsByRef.has(event.itemRef)) return false
        openToolsByRef.add(event.itemRef)
        return true
      }
      case "tool_result": {
        if (event.itemRef === undefined) return !closeUnkeyedTool(event.toolName)
        const updated = openToolsByRef.delete(event.itemRef)
        return !updated
      }
      case "question_pending":
        return upsert(`question:${event.questionRef}`)
      case "question_resolved":
        return false
      case "plan_updated":
        return upsert("plan")
      case "child_started":
      case "child_activity":
      case "child_completed":
      case "child_failed":
        return upsert(`child:${event.childRef}`)
      case "child_steered":
        return false
      case "followup_queued":
        return upsert(`queue:${event.queueRef}`)
      case "followup_promoted":
        runtimeKeys.delete(`queue:${event.queueRef}`)
        return false
      case "turn_started":
      case "composer_admission":
      case "text_delta":
      case "mcp_server_unavailable":
      case "meter_updated":
      case "turn_completed":
      case "turn_failed":
        return false
    }
  }
}

export const FableLocalEventEnvelopeSchema = Schema.Struct({
  turnRef: Schema.String,
  event: FableLocalEventSchema,
})
export type FableLocalEventEnvelope = typeof FableLocalEventEnvelopeSchema.Type

/**
 * Codex model ids are app-server catalog data, not an OpenAgents release
 * constant. Keep the wire value bounded and structurally Codex-scoped; the
 * main-owned control plane performs the exact visible-catalog admission.
 */
const CODEX_MODEL_ID_PATTERN = /^gpt-[a-z0-9][a-z0-9.-]{0,78}$/
export const CodexModelSchema = Schema.String.check(
  Schema.isMinLength(5),
  Schema.isMaxLength(80),
  Schema.isPattern(CODEX_MODEL_ID_PATTERN),
)
export type CodexModel = typeof CodexModelSchema.Type
export const CLAUDE_MODELS = ["claude-fable-5", "claude-opus-4-8", "claude-sonnet-5"] as const
export const ClaudeModelSchema = Schema.Literals(CLAUDE_MODELS)
export type ClaudeModel = typeof ClaudeModelSchema.Type
export const LocalModelSchema = Schema.Union([CodexModelSchema, ClaudeModelSchema])
export type LocalModel = typeof LocalModelSchema.Type
export const isCodexModel = (model: string): model is CodexModel =>
  CODEX_MODEL_ID_PATTERN.test(model)
export const isClaudeModel = (model: string): model is ClaudeModel =>
  model === "claude-fable-5" || model === "claude-opus-4-8" || model === "claude-sonnet-5"

export const LocalProviderTargetSchema = Schema.Struct({
  provider: Schema.Literals(["codex", "claude_agent"]),
  accountRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  model: LocalModelSchema,
})
export type LocalProviderTarget = typeof LocalProviderTargetSchema.Type

export const CODEX_REASONING_EFFORTS = ["low", "medium", "high", "xhigh", "max", "ultra"] as const
export const CodexReasoningEffortSchema = Schema.Literals(CODEX_REASONING_EFFORTS)
export type CodexReasoningEffort = typeof CodexReasoningEffortSchema.Type
export const isCodexReasoningEffort = (value: string): value is CodexReasoningEffort =>
  (CODEX_REASONING_EFFORTS as ReadonlyArray<string>).includes(value)

export const FableLocalStartRequestSchema = Schema.Struct({
  turnRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  threadRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  /** Durable queue admission identity; host validates this pair before provider dispatch. */
  queueRef: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120))),
  clientUserMessageId: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120))),
  // Widened from min-length 1 to max-length only (capability I1): a turn may
  // carry images with an empty message. This is a superset of the prior
  // contract — every previously valid request (non-empty message) still
  // decodes; images-only requests newly decode. Main still rejects a turn with
  // neither text nor images (`startRequestHasContent`).
  message: Schema.String.check(Schema.isMaxLength(8_000)),
  /**
   * Optional image attachments (capability I1). Bounded count; each bounded in
   * size by the schema. Additive — absent on every pre-I1 caller.
   */
  images: Schema.optional(
    Schema.Array(FableLocalImageAttachmentSchema).check(
      Schema.isMaxLength(FABLE_LOCAL_IMAGE_COUNT_LIMIT),
    ),
  ),
  /** Exact owner-selected target; absent preserves automatic health ordering. */
  target: Schema.optional(LocalProviderTargetSchema),
  /** Explicit `/skill` selection; main verifies it against the enabled host registry. */
  skill: Schema.optional(LocalSkillInvocationSchema),
  /** Explicit local authority posture; absent preserves owner-full default. */
  permissionMode: Schema.optional(Schema.Literals(["owner_full", "plan_only"])),
  /** Owner-selected Codex reasoning effort; ignored by the Claude lane. */
  reasoningEffort: Schema.optional(CodexReasoningEffortSchema),
  /** Exact owner-selected model. The runtime refuses provider substitution. */
  model: Schema.optional(LocalModelSchema),
  /** Reconciled Codex ecosystem identities. Main admits them before turn/start. */
  extensions: Schema.optional(Schema.Struct({
    skillIds: Schema.optional(Schema.Array(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120))).check(Schema.isMaxLength(32))),
    appIds: Schema.optional(Schema.Array(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120))).check(Schema.isMaxLength(32))),
    pluginIds: Schema.optional(Schema.Array(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120))).check(Schema.isMaxLength(32))),
  })),
  /**
   * Full Auto (#8852): when true on the Codex lane, the turn runs with
   * `approvalPolicy: "never"` (no mid-turn approval interruptions) and its
   * prompt is prefixed with the Full Auto instruction. Ignored by the Claude
   * lane. Absent/false preserves prior on-request approval behavior exactly.
   */
  fullAuto: Schema.optional(Schema.Boolean),
})
export type FableLocalStartRequest = typeof FableLocalStartRequestSchema.Type

/**
 * A start request carries content iff it has non-empty message text OR at
 * least one image. Main uses this to reject an empty turn now that the schema
 * permits an empty message (images-only turns are valid; empty+imageless is
 * not).
 */
export const startRequestHasContent = (
  request: Readonly<{ message: string; images?: ReadonlyArray<unknown> }>,
): boolean => request.message.trim() !== "" || (request.images?.length ?? 0) > 0

export const FableLocalInterruptRequestSchema = Schema.Struct({
  turnRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
})
export type FableLocalInterruptRequest = typeof FableLocalInterruptRequestSchema.Type

/**
 * Renderer answer to a pending question (EP250). One entry per answered
 * question: `question` is the question text exactly as the question_pending
 * event carried it (the runtime maps it back to the SDK's original text);
 * `labels` are the selected option labels — one for single-select, several
 * for multiSelect (the runtime joins them comma-separated, the SDK's
 * documented multi-select answer encoding).
 */
export const FableLocalQuestionAnswerSchema = Schema.Struct({
  question: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FABLE_LOCAL_SUMMARY_LIMIT)),
  labels: Schema.Array(Schema.String.check(Schema.isMaxLength(200))).check(Schema.isMaxLength(8)),
})
export type FableLocalQuestionAnswer = typeof FableLocalQuestionAnswerSchema.Type

export const FableLocalAnswerQuestionRequestSchema = Schema.Struct({
  turnRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  questionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  answers: Schema.Array(FableLocalQuestionAnswerSchema).check(Schema.isMaxLength(4)),
})
export type FableLocalAnswerQuestionRequest = typeof FableLocalAnswerQuestionRequestSchema.Type

export const decodeFableLocalStartRequest = (value: unknown): FableLocalStartRequest | null =>
  decode(FableLocalStartRequestSchema, value) as FableLocalStartRequest | null

/** Bounded array of picked image attachments (capability I1). */
export const FableLocalPickedImagesSchema = Schema.Array(FableLocalImageAttachmentSchema).check(
  Schema.isMaxLength(FABLE_LOCAL_IMAGE_COUNT_LIMIT),
)
export type FableLocalPickedImages = typeof FableLocalPickedImagesSchema.Type

export const FableLocalPickedImagesResultSchema = Schema.Struct({
  images: FableLocalPickedImagesSchema,
  rejection: Schema.NullOr(Schema.Literals(["wrong_type", "too_large", "count_limit", "unreadable"])),
})
export type FableLocalPickedImagesResult = typeof FableLocalPickedImagesResultSchema.Type

export const decodeFableLocalPickedImages = (
  value: unknown,
): FableLocalPickedImagesResult | null =>
  decode(FableLocalPickedImagesResultSchema, value) as FableLocalPickedImagesResult | null

export const decodeFableLocalInterruptRequest = (
  value: unknown,
): FableLocalInterruptRequest | null =>
  decode(FableLocalInterruptRequestSchema, value) as FableLocalInterruptRequest | null

export const decodeFableLocalAnswerQuestionRequest = (
  value: unknown,
): FableLocalAnswerQuestionRequest | null =>
  decode(FableLocalAnswerQuestionRequestSchema, value) as FableLocalAnswerQuestionRequest | null

// ---------------------------------------------------------------------------
// Child steering (G4) + follow-up queueing (A3) request contracts (EP250).
// ---------------------------------------------------------------------------
export const FableLocalSteerChildRequestSchema = Schema.Struct({
  turnRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  childRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  action: Schema.Literals(["message", "interrupt"]),
  /** Only meaningful for `action: "message"`; ignored for interrupt. */
  body: Schema.optional(Schema.String.check(Schema.isMaxLength(FABLE_LOCAL_FOLLOWUP_MESSAGE_LIMIT))),
})
export type FableLocalSteerChildRequest = typeof FableLocalSteerChildRequestSchema.Type

export const FableLocalQueueFollowupRequestSchema = Schema.Struct({
  threadRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  message: Schema.String.check(
    Schema.isMinLength(1),
    Schema.isMaxLength(FABLE_LOCAL_FOLLOWUP_MESSAGE_LIMIT),
  ),
  intentRef: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120))),
  clientUserMessageId: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120))),
  /** Required for steer; ignored by queue. Prevents stale-turn delivery. */
  expectedTurnId: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120))),
})
export type FableLocalQueueFollowupRequest = typeof FableLocalQueueFollowupRequestSchema.Type

export const decodeFableLocalSteerChildRequest = (
  value: unknown,
): FableLocalSteerChildRequest | null =>
  decode(FableLocalSteerChildRequestSchema, value) as FableLocalSteerChildRequest | null

export const decodeFableLocalQueueFollowupRequest = (
  value: unknown,
): FableLocalQueueFollowupRequest | null =>
  decode(FableLocalQueueFollowupRequestSchema, value) as FableLocalQueueFollowupRequest | null

// ===========================================================================
// FROZEN user-MCP-server config contract (I2) — EP250 wave-1.
//
// This is the schema the SEPARATE wave-2 settings-UI lane must build against.
// The desktop host reads a list of these (from settings/config) and hands it
// to the fable-local runtime, which merges the ENABLED ones into the SDK's
// `Options.mcpServers` alongside the internal `codex` delegate server. Their
// tools then surface to the model as `mcp__<name>__<tool>`.
//
// FROZEN FIELDS (do not repurpose; add new optional fields only):
// - name       server id; becomes the `mcp__<name>__…` tool prefix. Charset
//              is validated in `normalizeFableLocalMcpServers`
//              (`FABLE_LOCAL_MCP_NAME_PATTERN`); "codex" is RESERVED.
// - transport  "stdio" | "http" (SSE is not exposed in wave-1).
// - enabled    disabled entries are skipped entirely (default posture: none).
// - command    stdio: the executable to spawn (required for stdio).
// - args       stdio: bounded argv list.
// - env        stdio: bounded environment overrides.
// - url        http: the http(s) endpoint (required for http).
// - headers    http: bounded request headers (e.g. Authorization).
// ===========================================================================
/**
 * MCP server name charset: a bounded ID field (semantic route already chosen —
 * this is the deterministic ID validation the workspace contract allows). One
 * to 64 chars of letters/digits/underscore/hyphen, not starting/ending with a
 * separator so the `mcp__<name>__<tool>` prefix stays well-formed.
 */
export const FABLE_LOCAL_MCP_NAME_PATTERN = /^[A-Za-z0-9](?:[A-Za-z0-9_-]{0,62}[A-Za-z0-9])?$/

export const FableLocalMcpServerConfigSchema = Schema.Struct({
  name: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(64)),
  transport: Schema.Literals(["stdio", "http"]),
  enabled: Schema.Boolean,
  command: Schema.optional(Schema.String.check(Schema.isMaxLength(512))),
  args: Schema.optional(
    Schema.Array(Schema.String.check(Schema.isMaxLength(1_024))).check(Schema.isMaxLength(64)),
  ),
  env: Schema.optional(Schema.Record(Schema.String, Schema.String.check(Schema.isMaxLength(4_096)))),
  url: Schema.optional(Schema.String.check(Schema.isMaxLength(2_048))),
  headers: Schema.optional(
    Schema.Record(Schema.String, Schema.String.check(Schema.isMaxLength(4_096))),
  ),
})
export type FableLocalMcpServerConfig = typeof FableLocalMcpServerConfigSchema.Type

export const FableLocalMcpServerConfigsSchema = Schema.Array(FableLocalMcpServerConfigSchema).check(
  Schema.isMaxLength(FABLE_LOCAL_MCP_SERVER_LIMIT),
)

export const decodeFableLocalMcpServerConfigs = (
  value: unknown,
): ReadonlyArray<FableLocalMcpServerConfig> | null =>
  decode(FableLocalMcpServerConfigsSchema, value) as
    | ReadonlyArray<FableLocalMcpServerConfig>
    | null

/** One normalized, ready-to-pass SDK server (`mcpServers[name] = sdkConfig`). */
export type FableLocalNormalizedMcpServer = Readonly<{
  name: string
  /** A `McpStdioServerConfig` / `McpHttpServerConfig` shape (sdk.d.ts). */
  sdkConfig: Record<string, unknown>
}>

export type FableLocalMcpNormalizeResult = Readonly<{
  valid: ReadonlyArray<FableLocalNormalizedMcpServer>
  invalid: ReadonlyArray<{ name: string; reason: string }>
}>

/**
 * Pure, bounded normalization of user MCP configs into SDK server configs.
 * Skips disabled entries; rejects (into `invalid`, never a throw) bad names,
 * the reserved `codex` name, duplicates, and transport-specific missing
 * fields. A failed/invalid server NEVER blocks the turn — the runtime emits a
 * typed `mcp_server_unavailable` for each `invalid` entry and continues.
 */
export const normalizeFableLocalMcpServers = (
  configs: ReadonlyArray<FableLocalMcpServerConfig>,
): FableLocalMcpNormalizeResult => {
  const valid: Array<FableLocalNormalizedMcpServer> = []
  const invalid: Array<{ name: string; reason: string }> = []
  const seen = new Set<string>()
  for (const config of configs.slice(0, FABLE_LOCAL_MCP_SERVER_LIMIT)) {
    if (config.enabled !== true) continue
    const name = config.name.trim()
    if (!FABLE_LOCAL_MCP_NAME_PATTERN.test(name)) {
      invalid.push({ name, reason: "invalid server name (allowed: letters, digits, _ or -, 1-64 chars)" })
      continue
    }
    if (name === "codex") {
      invalid.push({ name, reason: "reserved server name (internal delegate server)" })
      continue
    }
    if (seen.has(name)) {
      invalid.push({ name, reason: "duplicate server name" })
      continue
    }
    if (config.transport === "stdio") {
      const command = (config.command ?? "").trim()
      if (command === "") {
        invalid.push({ name, reason: "stdio transport requires a command" })
        continue
      }
      seen.add(name)
      valid.push({
        name,
        sdkConfig: {
          type: "stdio",
          command,
          ...(config.args !== undefined && config.args.length > 0 ? { args: [...config.args] } : {}),
          ...(config.env !== undefined && Object.keys(config.env).length > 0
            ? { env: { ...config.env } }
            : {}),
        },
      })
      continue
    }
    const url = (config.url ?? "").trim()
    if (!/^https?:\/\//i.test(url)) {
      invalid.push({ name, reason: "http transport requires an http(s) url" })
      continue
    }
    seen.add(name)
    valid.push({
      name,
      sdkConfig: {
        type: "http",
        url,
        ...(config.headers !== undefined && Object.keys(config.headers).length > 0
          ? { headers: { ...config.headers } }
          : {}),
      },
    })
  }
  return { valid, invalid }
}

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
  event: Extract<FableLocalEvent, { kind: "tool_use" | "tool_progress" | "tool_result" }>,
): string => {
  const status = event.kind === "tool_use" ? "started"
    : event.kind === "tool_progress" ? "running"
      : event.ok ? "ok" : "failed"
  const summary = event.summary.trim() === "" ? "" : ` · ${event.summary.trim()}`
  return `${event.toolName} · ${status}${summary}`
}

/** The typed trace metadata carried on the same note (EP250 tool cards). */
export const fableLocalTraceNoteMeta = (
  event: Extract<FableLocalEvent, { kind: "tool_use" | "tool_progress" | "tool_result" }>,
): DesktopToolTrace => ({
  toolName: event.toolName,
  phase: event.kind === "tool_use" ? "started"
    : event.kind === "tool_progress" ? "progress"
      : event.ok ? "ok" : "failed",
  summary: event.summary.trim(),
  ...(event.itemRef === undefined ? {} : { itemRef: event.itemRef }),
  // The typed item (#8859) rides the same note so persisted transcripts
  // rebuild the same typed cards the live stream showed.
  ...(event.item === undefined ? {} : { item: event.item }),
})

/**
 * Deterministic inverse of `fableLocalTraceNoteText`, for persisted system
 * notes written before typed `meta.trace` existed. This parses only our own
 * bounded serialization format (`<toolName> · <status>[ · <summary>]`) — it
 * is a fallback for historical thread-store rows, not an intent router.
 */
export const parseFableLocalTraceNoteText = (text: string): DesktopToolTrace | null => {
  const match = /^([A-Za-z0-9_.:/-]{1,120}) · (started|running|ok|failed)(?: · ([\s\S]*))?$/.exec(text)
  if (match === null) return null
  return {
    toolName: match[1] ?? "",
    phase: match[2] === "running" ? "progress" : (match[2] ?? "started") as DesktopToolTrace["phase"],
    summary: (match[3] ?? "").slice(0, FABLE_LOCAL_SUMMARY_LIMIT),
  }
}

/**
 * Effective-model caption rendered as a transcript trace line above the
 * assistant reply (e.g. "Claude · claude-fable-5"). The model half is the
 * SDK-reported effective model, so the caption is capability-truthful even
 * though it also carries the lane brand.
 */
export const fableLocalModelNoteText = (model: string): string => `Claude · ${model}`

/** Renderer-facing copy for a typed lane failure — no provider text leaks. */
export const fableLocalFailureMessage = (
  reason: FableLocalFailureReason,
  detail: string,
): string => {
  const suffix = detail.trim() === "" ? "" : ` (${detail.trim()})`
  switch (reason) {
    case "no_claude_account":
      return "The local Claude lane is unavailable: no linked Claude account home found on this machine. No message was routed to any other lane."
    case "sdk_unavailable":
      return `The local Claude runtime could not start${suffix}. No message was routed to any other lane.`
    case "budget_exceeded":
      return "The local Claude turn hit its turn budget before finishing."
    case "interrupted":
      return "The local Claude turn was interrupted."
    case "timeout":
      return "The local Claude turn timed out."
    case "session_failed":
      return `The local Claude turn failed${suffix}.`
    case "model_substituted":
      return `The Claude lane refused a substituted model${suffix}. No substituted output was shown as Claude.`
    // Codex-local reasons never reach this fable-branded formatter in the
    // renderer (the codex lane formats through codexLocalFailureMessage), but
    // the switch stays exhaustive over the shared reason set.
    case "no_codex_account":
      return "No Codex account is registered on this machine. No message was routed to any other lane."
    case "account_reconnect_required":
      return `Every registered Codex account needs reconnect${suffix}. Reconnect in Settings — no message was routed to any other lane.`
    case "incompatible_workflow":
      return `The ProductSpec Codex workflow is incompatible${suffix}. No ambient skill or other lane was substituted.`
  }
}
