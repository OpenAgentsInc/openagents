import { Effect, Schema as S, Stream } from "effect";
import {
  decodeKhalaRuntimeEvent,
  type KhalaRuntimeFinishReason,
  type KhalaRuntimeSource,
  type KhalaRuntimeUsage,
  type RuntimeInteractionDecision,
  type RuntimeInteractionPayload,
} from "@openagentsinc/agent-runtime-schema";
import type { AgentHarness, HarnessStartOptions } from "./adapter.ts";
import { HarnessStartError } from "./adapter.ts";
import { HarnessCapabilityUnsupported } from "./capability.ts";
import { type HarnessToolIdentity, toolIdentity } from "./common-tool.ts";
import { buildTextDelta, buildTurnFinished, buildTurnStarted } from "./event-builder.ts";
import type { HarnessContinuationState, HarnessResumeState } from "./lifecycle-state.ts";
import type { HarnessPermissionMode } from "./permission.ts";
import type {
  HarnessPromptControl,
  HarnessPromptTurnOptions,
  HarnessSession,
  HarnessTurnResult,
} from "./session.ts";
import { HarnessTurnError } from "./session.ts";
import { KhalaRuntimeEventSchemaLiteral } from "./stream.ts";
import type { HarnessStreamEvent } from "./stream.ts";

/**
 * Claude Code harness adapter (HW-02): the `@anthropic-ai/claude-agent-sdk`
 * `query()` runtime as an {@link AgentHarness}. The message drive loop, session
 * continuity, usage extraction, failure classification, and the AskUserQuestion
 * `canUseTool` answer path are PORTED from the monorepo's proven Claude local
 * runtime (`apps/openagents-desktop/src/claude-local-runtime.ts`, #8712) and
 * the Pylon Claude executor — generalized here onto the neutral
 * `KhalaRuntimeEvent` stream instead of the renderer `ClaudeLocalEvent`
 * envelope.
 *
 * The SDK itself is NOT a dependency: {@link ClaudeCodeQuery} is a minimal
 * structural mirror of the SDK surface this adapter consumes (the `query()`
 * async-iterable message shape, the init `session_id`, the `canUseTool`
 * callback, `options.resume`), injected exactly like a transport — so the
 * adapter stays dependency-free and hermetically testable, and a host wires the
 * real SDK's `query` in with zero adaptation.
 *
 * Host-resident posture (honest capability table):
 * - `promptTurn` drives one `query()` turn and projects its streamed messages
 *   onto `KhalaRuntimeEvent`.
 * - Session continuity: the init message's `session_id` is persisted and passed
 *   as `options.resume` on the next turn (ported from the desktop runtime's
 *   `sessionByThread` map). `instructions` are applied once, prepended to the
 *   first user message of a fresh session, never on a resumed one.
 * - `suspendTurn`/`continueTurn` take the contract-blessed DEGRADED rerun form:
 *   persist the SDK session id, abort the in-flight turn, re-drive with
 *   `resume` — the tail after the cursor is recomputed, so the continuation is
 *   declared `lossy: true` honestly, never claimed lossless.
 * - Approvals never ride the event stream: AskUserQuestion and `canUseTool`
 *   permission requests route through the durable `RuntimeInteraction` model
 *   (like the ACP adapter), via {@link makeClaudeCodeCanUseTool}.
 * - Isolation: the injected `configDir` is exported as `CLAUDE_CONFIG_DIR` and
 *   `settingSources` stays `[]`; the owner's live `~/.claude` home is REFUSED
 *   at session start (never the owner's live session by default).
 */

// ---------------------------------------------------------------------------
// Structural Claude Agent SDK seam
// ---------------------------------------------------------------------------

/** SDK `PermissionResult` allow branch: the tool runs with `updatedInput`. */
export interface ClaudeCodePermissionAllow {
  readonly behavior: "allow";
  readonly updatedInput: Record<string, unknown>;
}

/** SDK `PermissionResult` deny branch: the model sees `message` as the refusal. */
export interface ClaudeCodePermissionDeny {
  readonly behavior: "deny";
  readonly message: string;
}

export type ClaudeCodePermissionResult = ClaudeCodePermissionAllow | ClaudeCodePermissionDeny;

/**
 * SDK `canUseTool` handler shape. Every non-auto-allowed tool call lands here;
 * AskUserQuestion answers ride back as `allow + updatedInput.answers` (the
 * SDK-documented answer mechanism, receipted in the desktop runtime).
 */
export type ClaudeCodeCanUseTool = (
  toolName: string,
  toolInput: Record<string, unknown>,
  extra?: { readonly signal?: AbortSignal },
) => Promise<ClaudeCodePermissionResult>;

/**
 * The `query()` options subset this adapter drives. Mirrors the SDK `Options`
 * keys the ported desktop runtime uses; `permissionMode` stays `"default"`
 * because `"bypassPermissions"` would "Bypass all permission checks" — skipping
 * the `canUseTool` handler the AskUserQuestion flow parks on (sdk.d.ts receipt
 * carried over from the desktop runtime).
 */
export interface ClaudeCodeQueryOptions {
  readonly cwd?: string;
  readonly env?: Readonly<Record<string, string>>;
  readonly model?: string;
  /** SDK session id of a previous turn — the session-continuity seam. */
  readonly resume?: string;
  readonly abortController?: AbortController;
  readonly permissionMode?: string;
  readonly canUseTool?: ClaudeCodeCanUseTool;
  readonly settingSources?: ReadonlyArray<string>;
  readonly includePartialMessages?: boolean;
  readonly allowedTools?: ReadonlyArray<string>;
  readonly disallowedTools?: ReadonlyArray<string>;
  readonly maxTurns?: number;
  readonly pathToClaudeCodeExecutable?: string;
  readonly mcpServers?: unknown;
  readonly plugins?: ReadonlyArray<unknown>;
  readonly skills?: ReadonlyArray<string>;
}

export interface ClaudeCodeQueryParams {
  readonly prompt: string;
  readonly options: ClaudeCodeQueryOptions;
}

/** The injected `query()` seam: structurally the SDK's `query`, no SDK import. */
export type ClaudeCodeQuery = (params: ClaudeCodeQueryParams) => AsyncIterable<ClaudeCodeMessage>;

/**
 * SDK `system`/`init` message: carries the provider session id every later
 * `options.resume` needs, plus the effective model. Projects to NO neutral
 * event — identity is captured out-of-band by the adapter.
 */
export interface ClaudeCodeSystemInitMessage {
  readonly type: "system";
  readonly subtype: "init";
  readonly session_id: string;
  readonly model?: string;
}

/**
 * SDK partial `stream_event` (`includePartialMessages: true`): the streaming
 * text/thinking deltas the desktop runtime emits live. Only the
 * `content_block_delta` shapes this adapter reads are modelled.
 */
export interface ClaudeCodeStreamEventMessage {
  readonly type: "stream_event";
  readonly event: {
    readonly type: string;
    readonly delta?: {
      readonly type: string;
      readonly text?: string;
      readonly thinking?: string;
    };
  };
}

export interface ClaudeCodeTextBlock {
  readonly type: "text";
  readonly text: string;
}

export interface ClaudeCodeThinkingBlock {
  readonly type: "thinking";
  readonly thinking: string;
}

/**
 * SDK assistant `tool_use` block. The raw `input` payload is intentionally NOT
 * modelled — like the opencode adapter, only the public-safe replayable subset
 * crosses into a neutral event (`inputRef`, never raw args).
 */
export interface ClaudeCodeToolUseBlock {
  readonly type: "tool_use";
  readonly id: string;
  readonly name: string;
}

export type ClaudeCodeContentBlock =
  | ClaudeCodeTextBlock
  | ClaudeCodeThinkingBlock
  | ClaudeCodeToolUseBlock;

/** SDK complete assistant message: text/thinking/tool_use content blocks. */
export interface ClaudeCodeAssistantMessage {
  readonly type: "assistant";
  readonly message: {
    readonly id: string;
    readonly content: ReadonlyArray<ClaudeCodeContentBlock>;
  };
}

/**
 * SDK `tool_result` block inside a `user` message. Raw result `content` is not
 * modelled (public-safe subset only); `is_error` selects `tool.result` versus
 * `tool.error`.
 */
export interface ClaudeCodeToolResultBlock {
  readonly type: "tool_result";
  readonly tool_use_id: string;
  readonly is_error?: boolean;
}

export interface ClaudeCodeUserMessage {
  readonly type: "user";
  readonly message: {
    readonly content: ReadonlyArray<ClaudeCodeToolResultBlock>;
  };
}

/** SDK `result` usage: the exact cumulative token fields the ledger reads. */
export interface ClaudeCodeResultUsage {
  readonly input_tokens?: number;
  readonly output_tokens?: number;
  readonly cache_read_input_tokens?: number;
  readonly cache_creation_input_tokens?: number;
}

/**
 * SDK terminal `result` message: the provider-terminal authority (subtype +
 * `is_error` + exact usage). Claude Code can return `subtype: "success"` with
 * `is_error: true`; the useful typed fact then lives in `result` text.
 */
export interface ClaudeCodeResultMessage {
  readonly type: "result";
  readonly subtype: string;
  readonly is_error?: boolean;
  readonly session_id?: string;
  readonly result?: string;
  /** `usage: null` is a real wire shape (desktop fixture-pinned); treat like absent. */
  readonly usage?: ClaudeCodeResultUsage | null;
}

export type ClaudeCodeMessage =
  | ClaudeCodeSystemInitMessage
  | ClaudeCodeStreamEventMessage
  | ClaudeCodeAssistantMessage
  | ClaudeCodeUserMessage
  | ClaudeCodeResultMessage;

// ---------------------------------------------------------------------------
// Ported classification helpers
// ---------------------------------------------------------------------------

/**
 * Operator-facing failure classes for a Claude Code turn. Ported from the
 * desktop runtime's `classifyClaudeSdkResultFailure` — the account/access and
 * quota facts are preserved as typed classes so a supervisor can rotate
 * accounts immediately instead of degrading every refusal to an opaque error.
 */
export type ClaudeCodeFailureClass =
  | "account_reconnect_required"
  | "budget_exceeded"
  | "session_failed";

/** Classify a Claude SDK failure detail string (ported, desktop runtime). */
export const classifyClaudeCodeFailure = (detail: string): ClaudeCodeFailureClass => {
  const lower = detail.toLowerCase();
  if (
    lower.includes("failed to authenticate") ||
    lower.includes("oauth session expired") ||
    lower.includes("disabled claude subscription access") ||
    lower.includes("use an anthropic api key instead")
  ) {
    return "account_reconnect_required";
  }
  if (
    lower.includes("usage limit") ||
    lower.includes("quota") ||
    lower.includes("purchase more credits")
  ) {
    return "budget_exceeded";
  }
  return "session_failed";
};

/**
 * SDK result subtype/is_error -> neutral finish reason. Ported from the desktop
 * runtime's finalization ladder: `max_turns` is a length/budget stop, an error
 * subtype (or `is_error`) is an error finish, `success` is a normal stop.
 */
export const claudeCodeFinishReason = (
  subtype: string,
  isError: boolean,
): KhalaRuntimeFinishReason =>
  subtype.includes("max_turns")
    ? "length"
    : isError || subtype.startsWith("error")
      ? "error"
      : subtype === "success"
        ? "stop"
        : "unknown";

const finiteToken = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value) && value > 0 ? Math.trunc(value) : 0;

/**
 * Exact usage from the SDK `result` message (ported: `usageSplitFromResult`).
 * Returns `undefined` when no positive usage is present, so a missing or zero
 * usage never fabricates a token record.
 */
export const claudeCodeUsage = (
  usage: ClaudeCodeResultUsage | null | undefined,
  usageRef: string,
): KhalaRuntimeUsage | undefined => {
  // `usage: null` is a real SDK wire shape (openagents#9167 desktop fixture);
  // it means the same as absent and must never crash the projection.
  if (usage === undefined || usage === null) return undefined;
  const inputTokens = finiteToken(usage.input_tokens);
  const outputTokens = finiteToken(usage.output_tokens);
  const cacheReadInputTokens = finiteToken(usage.cache_read_input_tokens);
  const cacheWriteInputTokens = finiteToken(usage.cache_creation_input_tokens);
  const totalTokens = inputTokens + outputTokens + cacheReadInputTokens + cacheWriteInputTokens;
  if (totalTokens === 0) return undefined;
  return {
    usageRef,
    inputTokens,
    outputTokens,
    cacheReadInputTokens,
    cacheWriteInputTokens,
    totalTokens,
  };
};

// ---------------------------------------------------------------------------
// Projection: SDK messages -> KhalaRuntimeEvent
// ---------------------------------------------------------------------------

/** Context threaded through {@link claudeCodeMessageToKhalaEvents} while folding a stream. */
export interface ClaudeCodeProjectionContext {
  readonly source: KhalaRuntimeSource;
  readonly threadId: string;
  readonly turnId: string;
  /** Allocate the next session-global sequence number. */
  readonly nextSequence: () => number;
  /** tool_use id -> native tool name, populated by tool_use, read by tool_result. */
  readonly toolNames: Map<string, string>;
  /**
   * Dedup state for partial-versus-complete content. With
   * `includePartialMessages` the SDK streams `stream_event` deltas AND then
   * re-delivers the same content as complete assistant blocks; the desktop
   * runtime emits deltas from stream events and uses assistant blocks only as
   * the no-partial-events fallback ("a build that skips partial events still
   * yields the full reply"). Same authority order here.
   */
  readonly streamed: { text: boolean; thinking: boolean };
}

const base = (ctx: ClaudeCodeProjectionContext, sequence: number, eventSuffix: string) => ({
  schema: KhalaRuntimeEventSchemaLiteral,
  eventId: `evt.${ctx.turnId}.${sequence}.${eventSuffix}`,
  turnId: ctx.turnId,
  threadId: ctx.threadId,
  sequence,
  observedAt: "2026-07-20T00:00:00.000Z",
  source: ctx.source,
  visibility: "private",
  redactionClass: "private_ref",
  causalityRefs: [] as ReadonlyArray<string>,
});

/**
 * Provider-reported tool authority. Same stance as the ACP adapter: an SDK
 * tool event is REPORTED runtime state, never our authority decision — the
 * real authority decision (when one exists) lives in the durable
 * `RuntimeInteraction` the `canUseTool` seam routed through.
 */
const providerReportedAuthority = (toolCallId: string, wireName: string) => ({
  authorityRef: `authority.claude_code.${toolCallId}`,
  policyRef: "policy.claude_code_adapter",
  decisionRef: "decision.provider_reported_not_authority",
  toolRef: `toolref.claude_code.${wireName}`,
  status: "denied" as const,
  allowed: false,
  blockerRefs: ["blocker.provider_event_not_authority"],
});

const buildReasoningDelta = (
  ctx: ClaudeCodeProjectionContext,
  sequence: number,
  fields: { readonly messageId: string; readonly text: string },
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(ctx, sequence, "reasoning"),
    kind: "reasoning.delta",
    messageId: fields.messageId,
    chunkId: `chunk.${ctx.turnId}.${sequence}`,
    text: fields.text,
  });

const buildToolCall = (
  ctx: ClaudeCodeProjectionContext,
  sequence: number,
  toolCallId: string,
  identity: HarnessToolIdentity,
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(ctx, sequence, "toolcall"),
    kind: "tool.call",
    toolCallId,
    toolName: identity.wireName,
    inputRef: `input.claude_code.${toolCallId}`,
    authority: providerReportedAuthority(toolCallId, identity.wireName),
  });

const buildToolOutcome = (
  ctx: ClaudeCodeProjectionContext,
  sequence: number,
  toolCallId: string,
  identity: HarnessToolIdentity,
  ok: boolean,
): HarnessStreamEvent =>
  ok
    ? decodeKhalaRuntimeEvent({
        ...base(ctx, sequence, "toolresult"),
        kind: "tool.result",
        toolCallId,
        toolName: identity.wireName,
        resultRef: `result.claude_code.${toolCallId}`,
        authority: providerReportedAuthority(toolCallId, identity.wireName),
        providerExecuted: true,
      })
    : decodeKhalaRuntimeEvent({
        ...base(ctx, sequence, "toolerror"),
        kind: "tool.error",
        toolCallId,
        toolName: identity.wireName,
        errorRef: `error.claude_code.${toolCallId}`,
        messageSafe: "Claude Code tool reported failure",
        authority: providerReportedAuthority(toolCallId, identity.wireName),
        providerExecuted: true,
      });

/**
 * Pure projection of ONE Claude SDK message onto zero or more neutral
 * {@link HarnessStreamEvent}s. The mapping is the desktop runtime's drive loop
 * generalized: init carries identity only (no event), `stream_event` deltas are
 * the live text/thinking stream, complete assistant blocks are the no-partials
 * fallback plus the `tool_use` source, `user` tool_result blocks correlate to
 * the earlier call through {@link ClaudeCodeProjectionContext.toolNames}, and
 * the terminal `result` becomes `turn.finished` with exact usage.
 */
export const claudeCodeMessageToKhalaEvents = (
  message: ClaudeCodeMessage,
  ctx: ClaudeCodeProjectionContext,
): ReadonlyArray<HarnessStreamEvent> => {
  switch (message.type) {
    case "system":
      // Identity (session_id, model) is captured out-of-band by the adapter.
      return [];
    case "stream_event": {
      const delta = message.event.delta;
      if (message.event.type !== "content_block_delta" || delta === undefined) return [];
      if (delta.type === "text_delta" && typeof delta.text === "string" && delta.text.length > 0) {
        ctx.streamed.text = true;
        return [
          buildTextDelta({
            turnId: ctx.turnId,
            threadId: ctx.threadId,
            sequence: ctx.nextSequence(),
            source: ctx.source,
            messageId: `msg.${ctx.turnId}.stream`,
            text: delta.text,
          }),
        ];
      }
      if (
        delta.type === "thinking_delta" &&
        typeof delta.thinking === "string" &&
        delta.thinking.length > 0
      ) {
        ctx.streamed.thinking = true;
        return [
          buildReasoningDelta(ctx, ctx.nextSequence(), {
            messageId: `msg.${ctx.turnId}.stream`,
            text: delta.thinking,
          }),
        ];
      }
      return [];
    }
    case "assistant": {
      const events: Array<HarnessStreamEvent> = [];
      for (const block of message.message.content) {
        if (block.type === "text") {
          // Complete-block fallback only: stream deltas already carried it.
          if (ctx.streamed.text || block.text.length === 0) continue;
          events.push(
            buildTextDelta({
              turnId: ctx.turnId,
              threadId: ctx.threadId,
              sequence: ctx.nextSequence(),
              source: ctx.source,
              messageId: message.message.id,
              text: block.text,
            }),
          );
          continue;
        }
        if (block.type === "thinking") {
          if (ctx.streamed.thinking || block.thinking.length === 0) continue;
          events.push(
            buildReasoningDelta(ctx, ctx.nextSequence(), {
              messageId: message.message.id,
              text: block.thinking,
            }),
          );
          continue;
        }
        ctx.toolNames.set(block.id, block.name);
        const identity = toolIdentity(block.name, { providerExecuted: true });
        events.push(buildToolCall(ctx, ctx.nextSequence(), block.id, identity));
      }
      return events;
    }
    case "user": {
      const events: Array<HarnessStreamEvent> = [];
      for (const block of message.message.content) {
        if (block.type !== "tool_result") continue;
        const nativeName = ctx.toolNames.get(block.tool_use_id) ?? block.tool_use_id;
        const identity = toolIdentity(nativeName, { providerExecuted: true });
        events.push(
          buildToolOutcome(
            ctx,
            ctx.nextSequence(),
            block.tool_use_id,
            identity,
            block.is_error !== true,
          ),
        );
      }
      return events;
    }
    case "result": {
      const sequence = ctx.nextSequence();
      const usage = claudeCodeUsage(message.usage, `usage.claude_code.${ctx.turnId}.${sequence}`);
      return [
        buildTurnFinished({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence,
          source: ctx.source,
          finishReason: claudeCodeFinishReason(message.subtype, message.is_error === true),
          ...(usage === undefined ? {} : { usage }),
        }),
      ];
    }
    // The live SDK emits message types beyond the modeled set (for example
    // rate-limit and progress notices). Unknown types project to no neutral
    // event instead of crashing the stream (found by the live smoke).
    default:
      return [];
  }
};

// ---------------------------------------------------------------------------
// Permission routing: canUseTool / AskUserQuestion -> RuntimeInteraction
// ---------------------------------------------------------------------------

/** The one interactive tool answered through the question flow (ported). */
export const CLAUDE_CODE_QUESTION_TOOL = "AskUserQuestion";

/**
 * The durable interaction seam permission requests route through. The host
 * resolves each payload to a canonical `RuntimeInteractionDecision` (owner UI,
 * policy engine, or auto-deny) — the adapter never decides authority itself.
 */
export interface ClaudeCodeInteractionSeam {
  readonly requestInteraction: (
    payload: RuntimeInteractionPayload,
  ) => Promise<RuntimeInteractionDecision>;
}

/**
 * Project a non-question `canUseTool` request onto a canonical `tool_approval`
 * `RuntimeInteractionPayload` — the same durable, provider-neutral approval
 * model the ACP adapter routes through. The carried authority is
 * `operator_escalation_required` (owner decision pending), never a self-granted
 * allow.
 */
export const claudeCodePermissionToRuntimeInteractionPayload = (request: {
  readonly toolCallId: string;
  readonly toolName: string;
}): RuntimeInteractionPayload => {
  const identity = toolIdentity(request.toolName, { providerExecuted: true });
  return {
    kind: "tool_approval",
    displayText: `Allow Claude Code to run ${identity.wireName}?`,
    toolCallId: request.toolCallId,
    toolName: identity.wireName,
    authority: {
      ...providerReportedAuthority(request.toolCallId, identity.wireName),
      status: "operator_escalation_required",
      blockerRefs: ["blocker.owner_approval"],
    },
  };
};

/**
 * Parsed AskUserQuestion input (ported from `parseAskUserQuestions`): the
 * bounded questions plus the ref -> original-text/label maps so answers key
 * back to the SDK's EXACT question strings — the answers record is keyed by
 * question text, so ref indirection must never break that keying.
 */
export interface ParsedClaudeCodeQuestions {
  readonly payload: Extract<RuntimeInteractionPayload, { readonly kind: "provider_question" }>;
  readonly originalQuestionByRef: ReadonlyMap<string, string>;
  readonly labelByOptionRef: ReadonlyMap<string, string>;
}

const boundedText = (value: string, limit: number): string =>
  value.length > limit ? `${value.slice(0, limit - 1)}…` : value;

/**
 * Parse AskUserQuestion tool input onto a canonical `provider_question`
 * `RuntimeInteractionPayload`. Ported bounds: at most 4 questions of at most 4
 * labelled options each; malformed input returns `undefined` so the caller can
 * deny with the ported ask-again guidance.
 */
export const claudeCodeQuestionToRuntimeInteractionPayload = (
  rawInput: Record<string, unknown>,
  questionRefSeed: string,
): ParsedClaudeCodeQuestions | undefined => {
  const rawQuestions = rawInput.questions;
  if (!Array.isArray(rawQuestions) || rawQuestions.length === 0) return undefined;
  const questions: Array<{
    questionRef: string;
    displayText: string;
    options: Array<{ optionRef: string; label: string; description?: string }>;
    multiSelect: boolean;
  }> = [];
  const originalQuestionByRef = new Map<string, string>();
  const labelByOptionRef = new Map<string, string>();
  for (const [index, candidate] of rawQuestions.slice(0, 4).entries()) {
    if (candidate === null || typeof candidate !== "object") return undefined;
    const record = candidate as Record<string, unknown>;
    if (typeof record.question !== "string" || record.question.trim() === "") return undefined;
    const rawOptions = Array.isArray(record.options) ? record.options.slice(0, 4) : [];
    const options: Array<{ optionRef: string; label: string; description?: string }> = [];
    for (const [optionIndex, option] of rawOptions.entries()) {
      if (option === null || typeof option !== "object") continue;
      const optionRecord = option as Record<string, unknown>;
      if (typeof optionRecord.label !== "string" || optionRecord.label.trim() === "") continue;
      const optionRef = `option.${questionRefSeed}.${index}.${optionIndex}`;
      labelByOptionRef.set(optionRef, optionRecord.label);
      options.push({
        optionRef,
        label: boundedText(optionRecord.label, 160),
        ...(typeof optionRecord.description === "string" && optionRecord.description.length > 0
          ? { description: boundedText(optionRecord.description, 500) }
          : {}),
      });
    }
    if (options.length === 0) return undefined;
    const questionRef = `question.${questionRefSeed}.${index}`;
    originalQuestionByRef.set(questionRef, record.question);
    questions.push({
      questionRef,
      displayText: boundedText(record.question, 2_000),
      options,
      multiSelect: record.multiSelect === true,
    });
  }
  if (questions.length === 0) return undefined;
  const header =
    questions.length === 1 &&
    typeof (rawQuestions[0] as Record<string, unknown>).header === "string"
      ? ((rawQuestions[0] as Record<string, unknown>).header as string)
      : "";
  return {
    payload: {
      kind: "provider_question",
      displayTitle: boundedText(header.trim() === "" ? "Claude Code asks a question" : header, 160),
      questions,
    },
    originalQuestionByRef,
    labelByOptionRef,
  };
};

/** Configuration for {@link makeClaudeCodeCanUseTool}. */
export interface ClaudeCodeCanUseToolConfig {
  /**
   * The contract permission mode for built-in tools. `allow-all` is the
   * owner-local danger profile (auto-approve; questions still route),
   * `default` routes every non-question call through the interaction seam
   * (fail-closed deny when no seam is wired), `reject-all` denies everything.
   */
  readonly permissionMode?: HarnessPermissionMode;
  /** The durable interaction seam. Absent = fail-closed for gated requests. */
  readonly interaction?: ClaudeCodeInteractionSeam;
  /** Stable ref seed (turn ref) for minted toolCallId/questionRef values. */
  readonly refSeed?: string;
}

/**
 * Build the SDK `canUseTool` handler with every gated request routed through
 * the durable `RuntimeInteraction` model. AskUserQuestion resolves with the
 * SDK-documented answer mechanism (`allow + updatedInput.answers`, answers
 * keyed by ORIGINAL question text with multi-select labels comma-joined) —
 * ported from the desktop runtime's `awaitUserAnswer`. A seam failure or a
 * decision-kind mismatch is a fail-closed deny, never a silent allow.
 */
export const makeClaudeCodeCanUseTool = (
  config: ClaudeCodeCanUseToolConfig = {},
): ClaudeCodeCanUseTool => {
  const mode = config.permissionMode ?? "default";
  const refSeed = config.refSeed ?? "claude_code";
  const state = { calls: 0 };
  return async (toolName, toolInput) => {
    state.calls += 1;
    if (mode === "reject-all") {
      return {
        behavior: "deny",
        message: `Tool ${toolName} is denied by the session's reject-all permission mode.`,
      };
    }
    if (toolName === CLAUDE_CODE_QUESTION_TOOL) {
      const parsed = claudeCodeQuestionToRuntimeInteractionPayload(
        toolInput,
        `${refSeed}.${state.calls}`,
      );
      if (parsed === undefined) {
        return {
          behavior: "deny",
          message:
            "AskUserQuestion input was malformed; ask again with 1-4 questions, each with labeled options.",
        };
      }
      if (config.interaction === undefined) {
        return {
          behavior: "deny",
          message:
            "No interactive question path is wired. Make a reasonable judgment call and proceed without asking the user.",
        };
      }
      try {
        const decision = await config.interaction.requestInteraction(parsed.payload);
        if (decision.kind !== "provider_question") {
          return { behavior: "deny", message: "The question was not answered." };
        }
        const answers: Record<string, string> = {};
        for (const answer of decision.answers) {
          const original = parsed.originalQuestionByRef.get(answer.questionRef);
          if (original === undefined) continue;
          const labels = answer.optionRefs
            .map((optionRef) => parsed.labelByOptionRef.get(optionRef))
            .filter((label): label is string => label !== undefined && label.trim().length > 0);
          const text = labels.length > 0 ? labels.join(", ") : (answer.text ?? "").trim();
          if (text.length === 0) continue;
          answers[original] = text;
        }
        if (Object.keys(answers).length === 0) {
          return { behavior: "deny", message: "The answers did not match any asked question." };
        }
        return { behavior: "allow", updatedInput: { ...toolInput, answers } };
      } catch {
        return { behavior: "deny", message: "The question path failed; proceed without it." };
      }
    }
    if (mode === "allow-all") {
      return { behavior: "allow", updatedInput: toolInput };
    }
    // mode "default": the durable approval path decides; no seam = fail closed.
    if (config.interaction === undefined) {
      return {
        behavior: "deny",
        message: `Tool ${toolName} requires approval and no approval path is wired.`,
      };
    }
    const payload = claudeCodePermissionToRuntimeInteractionPayload({
      toolCallId: `toolcall.${refSeed}.${state.calls}`,
      toolName,
    });
    try {
      const decision = await config.interaction.requestInteraction(payload);
      if (decision.kind === "tool_approval" && decision.outcome === "approve") {
        return { behavior: "allow", updatedInput: toolInput };
      }
      return { behavior: "deny", message: `The owner denied ${toolName} for this call.` };
    } catch {
      return { behavior: "deny", message: `The approval path failed; ${toolName} stays denied.` };
    }
  };
};

// ---------------------------------------------------------------------------
// Isolation guard
// ---------------------------------------------------------------------------

/** An OS-home-shaped `.claude` directory (`/Users/<u>/.claude`, `/home/<u>/.claude`, `~/.claude`). */
const LIVE_CLAUDE_HOME_PATTERN = /^(?:\/(?:Users|home)\/[^/]+|~)\/\.claude$/;

/**
 * True when `configDir` is the owner's LIVE Claude Code home rather than an
 * isolated per-session/per-account home. The adapter treats a live-home value
 * the same as an omitted one: OWNER-LOCAL mode, reached by leaving
 * `CLAUDE_CONFIG_DIR` unset rather than pointing the runtime at an explicit
 * copy of the live home.
 */
export const isLiveClaudeHome = (configDir: string, homeDir?: string): boolean => {
  const normalized = configDir.replace(/\/+$/, "");
  if (homeDir !== undefined && normalized === `${homeDir.replace(/\/+$/, "")}/.claude`) {
    return true;
  }
  return LIVE_CLAUDE_HOME_PATTERN.test(normalized);
};

// ---------------------------------------------------------------------------
// Adapter factory
// ---------------------------------------------------------------------------

/**
 * Continuation prompt for the degraded rerun: the SDK has no attach-to-live
 * verb, so the continued slice re-drives the resumed session with this fixed
 * instruction and recomputes the tail (declared `lossy: true`).
 */
export const CLAUDE_CODE_CONTINUATION_PROMPT =
  "Continue the interrupted turn from where it stopped.";

/** Resume-state payload validated by {@link makeClaudeCodeHarnessAdapter}'s `lifecycleStateSchema`. */
export const ClaudeCodeResumeData = S.Struct({
  claudeSessionId: S.optionalKey(S.NonEmptyString),
});
export interface ClaudeCodeResumeData extends S.Schema.Type<typeof ClaudeCodeResumeData> {}

const ClaudeCodeContinuationData = S.Struct({
  claudeSessionId: S.optionalKey(S.NonEmptyString),
  prompt: S.String,
});

/** Configuration for {@link makeClaudeCodeHarnessAdapter}. */
export interface ClaudeCodeAdapterConfig {
  readonly harnessId?: string;
  /** The injected structural `query()` seam (the real SDK's `query` in production). */
  readonly query: ClaudeCodeQuery;
  /**
   * Claude settings/home directory, exported to the runtime as
   * `CLAUDE_CONFIG_DIR`. Omit it (or pass the live-home path) for OWNER-LOCAL
   * mode: CLAUDE_CONFIG_DIR is then left unset so the runtime uses the
   * developer's currently-authenticated default Claude home. The adapter only
   * drives query turns — never a login flow — so owner-local mode cannot
   * clobber the live session (owner decision 2026-07-22, openagents#9161).
   * Fleet and multi-account callers keep passing isolated homes.
   */
  readonly configDir?: string;
  /** The owner's home directory, for the live-home guard (injectable for tests). */
  readonly homeDir?: string;
  /** Working directory for the session's turns. */
  readonly cwd?: string;
  /** Requested model id (SDK `Options.model`). */
  readonly model?: string;
  /** Extra environment for the runtime (merged under the isolation env). */
  readonly env?: Readonly<Record<string, string>>;
  /** Durable interaction seam for AskUserQuestion/approval routing. */
  readonly interaction?: ClaudeCodeInteractionSeam;
  /**
   * Host-owned query-option overrides, merged LAST onto the adapter's
   * assembled options (openagents#9167 slice 2). The host owns whatever it
   * overrides — including `canUseTool` and `permissionMode`: overriding the
   * approval path bypasses the adapter's RuntimeInteraction routing, so the
   * host must provide its own durable approval handling in exchange.
   */
  readonly queryOverrides?: Readonly<Record<string, unknown>>;
  /**
   * Host raw-message observer (openagents#9167 slice 3), fired for EVERY SDK
   * message before neutral projection. The neutral core stream covers
   * text/reasoning/tool/turn; a host that renders richer, display-only events
   * (effective model, tool progress, sub-agent lifecycle, plan cards)
   * reconstructs them here from the raw items. Purely observational — it
   * never alters the neutral stream, cursor, or turn result.
   */
  readonly onRawMessage?: (message: ClaudeCodeMessage) => void;
}

interface ActiveClaudeTurn {
  readonly turnId: string;
  readonly prompt: string;
  readonly abort: AbortController;
}

interface TurnFinish {
  readonly finishReason: KhalaRuntimeFinishReason;
  readonly usage?: KhalaRuntimeUsage;
}

/**
 * Build the Claude Code {@link AgentHarness}. Host-resident: each `promptTurn`
 * drives one injected `query()` turn and lazily projects its streamed messages
 * onto the neutral event stream with session-global contiguous sequences;
 * `suspendTurn`/`continueTurn` are the DEGRADED rerun (abort + `resume` +
 * recomputed tail, `lossy: true`); `detach`/`stop` persist the SDK session id
 * as resume state; `compact` is refused honestly (the single-shot `query` seam
 * exposes no compaction trigger).
 */
export const makeClaudeCodeHarnessAdapter = (config: ClaudeCodeAdapterConfig): AgentHarness => {
  const harnessId = config.harnessId ?? "claude-code";

  const start = (options: HarnessStartOptions): Effect.Effect<HarnessSession, HarnessStartError> =>
    Effect.gen(function* () {
      const source: KhalaRuntimeSource = options.source;
      const sessionId = options.sessionId;

      // Owner-local mode: an omitted, empty, or live-home configDir means the
      // currently-authenticated default Claude home (CLAUDE_CONFIG_DIR unset).
      const effectiveConfigDir =
        config.configDir === undefined ||
        config.configDir.trim() === "" ||
        isLiveClaudeHome(config.configDir, config.homeDir)
          ? undefined
          : config.configDir;

      // Session-closure state. Mutable boxes (not Refs) because the projection
      // allocates sequences SYNCHRONOUSLY while the SDK iterable is consumed
      // lazily — the stream taps update them as events are actually delivered.
      const seedSequence = options.continueFrom?.cursor ?? -1;
      const sequence = { next: seedSequence + 1 };
      const cursor = { value: seedSequence };
      const claudeSession: { id: string | undefined } = { id: undefined };
      const active: { turn: ActiveClaudeTurn | undefined } = { turn: undefined };
      const isResume = options.resumeFrom !== undefined || options.continueFrom !== undefined;
      let instructionsApplied = isResume;

      if (options.resumeFrom !== undefined) {
        if (options.resumeFrom.harnessId !== harnessId) {
          return yield* Effect.fail(
            new HarnessStartError({
              harnessId,
              sessionId,
              failureClass: "cross_adapter_resume_state",
              detail: `resume state belongs to harness ${options.resumeFrom.harnessId}`,
            }),
          );
        }
        try {
          const data = S.decodeUnknownSync(ClaudeCodeResumeData)(options.resumeFrom.data);
          claudeSession.id = data.claudeSessionId;
        } catch (error) {
          return yield* Effect.fail(
            new HarnessStartError({
              harnessId,
              sessionId,
              failureClass: "invalid_resume_state",
              cause: error,
            }),
          );
        }
      }

      let pendingContinuation: { turnId: string; prompt: string } | undefined;
      if (options.continueFrom !== undefined) {
        try {
          const data = S.decodeUnknownSync(ClaudeCodeContinuationData)(options.continueFrom.data);
          claudeSession.id = data.claudeSessionId;
          pendingContinuation = { turnId: options.continueFrom.turnId, prompt: data.prompt };
        } catch (error) {
          return yield* Effect.fail(
            new HarnessStartError({
              harnessId,
              sessionId,
              failureClass: "invalid_continuation_state",
              cause: error,
            }),
          );
        }
      }

      const canUseTool = makeClaudeCodeCanUseTool({
        ...(options.permissionMode === undefined ? {} : { permissionMode: options.permissionMode }),
        ...(config.interaction === undefined ? {} : { interaction: config.interaction }),
        refSeed: `claude_code.${sessionId}`,
      });
      const allowedTools = options.builtinToolFiltering?.activeTools;
      const disallowedTools = options.builtinToolFiltering?.inactiveTools;

      /** Drive one `query()` turn (fresh prompt or degraded rerun). */
      const driveTurn = (params: {
        readonly turnId: string;
        readonly prompt: string;
        readonly rebaseSequence?: boolean;
      }): HarnessPromptControl => {
        const turnId = params.turnId;
        if (params.rebaseSequence === true) {
          // Degraded rerun: the recomputed tail attaches at cursor + 1 —
          // sequences allocated for never-delivered buffered events are
          // reclaimed, so no consumer ever sees a gap or a duplicate.
          sequence.next = cursor.value + 1;
        }
        const abort = new AbortController();
        active.turn = { turnId, prompt: params.prompt, abort };
        const finish: { value: TurnFinish | undefined } = { value: undefined };

        const ctx: ClaudeCodeProjectionContext = {
          source,
          threadId: sessionId,
          turnId,
          nextSequence: () => sequence.next++,
          toolNames: new Map<string, string>(),
          streamed: { text: false, thinking: false },
        };
        const turnStarted = buildTurnStarted({
          turnId,
          threadId: sessionId,
          sequence: ctx.nextSequence(),
          source,
        });

        const toTurnError = (error: unknown): HarnessTurnError => {
          const detail = error instanceof Error ? error.message : String(error);
          return new HarnessTurnError({
            harnessId,
            sessionId,
            turnId,
            failureClass: abort.signal.aborted ? "interrupted" : classifyClaudeCodeFailure(detail),
            detail,
            cause: error,
          });
        };

        const iterable = config.query({
          prompt: params.prompt,
          options: {
            ...(config.cwd === undefined ? {} : { cwd: config.cwd }),
            env: {
              ...(config.env ?? {}),
              ...(effectiveConfigDir === undefined
                ? {}
                : { CLAUDE_CONFIG_DIR: effectiveConfigDir }),
            },
            ...(config.model === undefined ? {} : { model: config.model }),
            abortController: abort,
            // Never "bypassPermissions": it would skip the canUseTool handler
            // the AskUserQuestion flow parks on (desktop runtime receipt).
            permissionMode: "default",
            canUseTool,
            settingSources: [],
            includePartialMessages: true,
            ...(allowedTools === undefined ? {} : { allowedTools }),
            ...(disallowedTools === undefined ? {} : { disallowedTools }),
            ...(claudeSession.id === undefined ? {} : { resume: claudeSession.id }),
            ...(config.queryOverrides ?? {}),
          },
        });

        const events: Stream.Stream<HarnessStreamEvent, HarnessTurnError> = Stream.fromIterable([
          turnStarted,
        ]).pipe(
          Stream.concat(
            Stream.fromAsyncIterable(iterable, toTurnError).pipe(
              Stream.tap((message) =>
                Effect.sync(() => {
                  if (
                    message.type === "system" &&
                    message.subtype === "init" &&
                    message.session_id.length > 0
                  ) {
                    claudeSession.id = message.session_id;
                  }
                  config.onRawMessage?.(message);
                }),
              ),
              Stream.flatMap((message) =>
                Stream.fromIterable(claudeCodeMessageToKhalaEvents(message, ctx)),
              ),
            ),
          ),
          Stream.tap((event) =>
            Effect.sync(() => {
              cursor.value = event.sequence;
              if (event.kind === "turn.finished") {
                finish.value = {
                  finishReason: event.finishReason,
                  ...(event.usage === undefined ? {} : { usage: event.usage }),
                };
                active.turn = undefined;
              }
            }),
          ),
        );

        const done: Effect.Effect<HarnessTurnResult, HarnessTurnError> = Effect.sync(() => ({
          turnId,
          finishReason: finish.value?.finishReason ?? "interrupted",
          ...(finish.value?.usage === undefined ? {} : { usage: finish.value.usage }),
          lastCursor: cursor.value,
        }));

        return {
          turnId,
          events,
          done,
          submitToolResult: () =>
            Effect.fail(
              new HarnessTurnError({
                harnessId,
                sessionId,
                turnId,
                failureClass: "no_active_tool_call",
                detail: "Claude Code executes its built-in tools in the runtime itself",
              }),
            ),
          submitToolApproval: () =>
            Effect.fail(
              new HarnessTurnError({
                harnessId,
                sessionId,
                turnId,
                failureClass: "no_active_tool_call",
                detail:
                  "Claude Code approvals route through RuntimeInteraction (canUseTool), not the native tool channel",
              }),
            ),
          submitUserMessage: () =>
            Effect.fail(
              new HarnessTurnError({
                harnessId,
                sessionId,
                turnId,
                failureClass: "mid_turn_user_message_unsupported",
                detail: "the single-prompt query seam cannot inject a mid-turn user message",
              }),
            ),
          interrupt: () => Effect.sync(() => abort.abort()),
        };
      };

      const promptTurn = (opts: HarnessPromptTurnOptions) =>
        Effect.sync(() => {
          const withInstructions =
            !instructionsApplied && opts.instructions !== undefined && opts.instructions.length > 0;
          instructionsApplied = true;
          return driveTurn({
            turnId: opts.turnId,
            prompt: withInstructions ? `${opts.instructions}\n\n${opts.prompt}` : opts.prompt,
          });
        });

      const continueTurn = () =>
        Effect.gen(function* () {
          const turn = active.turn ?? pendingContinuation;
          if (turn === undefined) {
            return yield* Effect.fail(
              new HarnessTurnError({
                harnessId,
                sessionId,
                turnId: "unknown",
                failureClass: "no_turn_to_continue",
              }),
            );
          }
          return driveTurn({
            turnId: turn.turnId,
            prompt: CLAUDE_CODE_CONTINUATION_PROMPT,
            rebaseSequence: true,
          });
        });

      const suspendTurn = (): Effect.Effect<
        HarnessContinuationState,
        HarnessCapabilityUnsupported
      > =>
        Effect.sync(() => {
          const turn = active.turn;
          // The degraded suspend: persist the SDK session id, abort the
          // in-flight turn, and record honestly that the continuation will be
          // a re-driven (lossy) tail rather than a lossless attach.
          turn?.abort.abort();
          return {
            harnessId,
            sessionId,
            turnId: turn?.turnId ?? "unknown",
            cursor: cursor.value,
            lossy: true,
            data: {
              ...(claudeSession.id === undefined ? {} : { claudeSessionId: claudeSession.id }),
              prompt: turn?.prompt ?? "",
            },
          };
        });

      const compact = () =>
        Effect.fail(
          new HarnessCapabilityUnsupported({
            harnessId,
            capability: "compact",
            detail: "the single-shot query seam exposes no compaction trigger",
          }),
        );

      const resumeState = (): HarnessResumeState => ({
        harnessId,
        sessionId,
        data: {
          ...(claudeSession.id === undefined ? {} : { claudeSessionId: claudeSession.id }),
        },
      });

      const detach = (): Effect.Effect<HarnessResumeState, HarnessCapabilityUnsupported> =>
        Effect.sync(resumeState);

      const stop = (): Effect.Effect<HarnessResumeState> =>
        Effect.sync(() => {
          active.turn?.abort.abort();
          active.turn = undefined;
          return resumeState();
        });

      const session: HarnessSession = {
        sessionId,
        isResume,
        ...(config.model === undefined ? {} : { modelId: config.model }),
        promptTurn,
        continueTurn,
        suspendTurn,
        compact,
        detach,
        stop,
        destroy: () =>
          Effect.sync(() => {
            active.turn?.abort.abort();
            active.turn = undefined;
          }),
      };
      return session;
    });

  return {
    specificationVersion: "agent-harness-v1",
    harnessId,
    harnessKind: "claude_code",
    adapterKind: "claude_code",
    // Claude Code's native PascalCase built-ins with their shared common names.
    builtinTools: [
      { nativeName: "Bash", commonName: "bash", description: "run a shell command" },
      { nativeName: "Read", commonName: "read", description: "read a file" },
      { nativeName: "Write", commonName: "write", description: "write a file" },
      { nativeName: "Edit", commonName: "edit", description: "edit a file" },
      { nativeName: "Glob", commonName: "glob", description: "match paths by glob" },
      { nativeName: "Grep", commonName: "grep", description: "search file contents" },
      { nativeName: "WebSearch", commonName: "webSearch", description: "search the web" },
      { nativeName: "WebFetch", description: "fetch a URL" },
    ],
    // Approvals route through RuntimeInteraction (canUseTool), never the
    // native submitToolApproval channel.
    supportsBuiltinToolApprovals: false,
    // The SDK filters built-ins natively (allowedTools/disallowedTools).
    supportsBuiltinToolFiltering: true,
    lifecycleStateSchema: ClaudeCodeResumeData,
    start,
  };
};
