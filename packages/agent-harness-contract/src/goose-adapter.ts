import { Effect } from "effect";
import type {
  KhalaRuntimeUsage,
  ModelFailureClass,
  RuntimeInteractionPayload,
} from "@openagentsinc/agent-runtime-schema";
import type { AgentHarness, HarnessStartOptions } from "./adapter.ts";
import { HarnessStartError } from "./adapter.ts";
import {
  type AcpAdapterEvent,
  acpPermissionToRuntimeInteractionPayload,
  makeAcpHarnessAdapter,
} from "./acp-adapter.ts";
import type { HarnessBuiltinTool } from "./common-tool.ts";
import type { HarnessToolApprovalDecision } from "./host-tool.ts";

/**
 * Goose harness adapter (HW-06).
 *
 * SURFACE CHOICE — ACP, configured through the existing ACP factory.
 *
 * The block/goose reference clone (`~/work/projects/repos/goose`, version
 * 1.43.0) is ACP-FIRST. Its only programmatic agent surface is the Agent
 * Client Protocol:
 * - `goose acp` runs an ACP agent server over stdio, and `goose serve` runs the
 *   SAME agent over HTTP/WebSocket at `/acp` (`crates/goose-cli/src/cli.rs`,
 *   `crates/goose/src/acp/`). The generic `serve<R, W>(agent, read, write)` in
 *   `acp/server.rs` is transport-agnostic over any AsyncRead/AsyncWrite, so a
 *   host injects its own duplex — a real, hermetic transport seam.
 * - The legacy `goosed` HTTP/SSE `/reply` daemon is GONE; there is no bespoke
 *   event-stream endpoint left to adapt.
 * - The CLI `goose run --output-format stream-json` emits WHOLE `Message`
 *   objects and can only confirm tools INTERACTIVELY on a TTY — it exposes no
 *   programmatic approve/deny channel — so it is disqualified for an adapter
 *   that must drive tool policy.
 *
 * Goose's ACP `SessionUpdate` vocabulary (`AgentMessageChunk`,
 * `AgentThoughtChunk`, `ToolCall`, `ToolCallUpdate{status}`, `UsageUpdate`),
 * its `session/request_permission` request, and its `PromptResponse{StopReason}`
 * terminator are the SAME shapes the neutral ACP adapter
 * (`makeAcpHarnessAdapter`) already projects onto `KhalaRuntimeEvent`. So Goose
 * becomes an `AgentHarness` by CONFIGURING that factory with a Goose peer
 * profile — exactly the Cursor-adapter approach — rather than by writing a
 * bespoke lane. This module contributes only the Goose-specific pieces the
 * generic factory cannot know:
 *
 * 1. `gooseUpdateToAcpEvents` — the honest projection from Goose's own
 *    `SessionUpdate` wire vocabulary onto the ACP adapter's neutral
 *    `AcpAdapterEvent` input, so a live `goose serve` / `goose acp` transport
 *    (or a hermetic fixture) drives the same factory.
 * 2. An isolated goose-home START GUARD: the adapter refuses to run against the
 *    owner's live goose config/data home (`~/.config/goose`,
 *    `~/.local/share/goose`, or a `~/.goose` tree), the same posture the Codex
 *    (`~/.codex`) and Pi (`~/.pi`) adapters take, because those trees hold the
 *    owner's live sessions, credentials, and settings.
 * 3. Approval routing through the canonical `RuntimeInteraction` model
 *    (`goosePermissionToRuntimeInteractionPayload`, reusing the ACP helper),
 *    plus the Goose `PermissionOptionKind` decision mapping.
 * 4. Error mapping onto the shared neutral `ModelFailureClass` vocabulary
 *    (`gooseModelFailureClass`), which a live Goose ACP transport uses to mint
 *    a typed turn failure from an ACP prompt error (`credits_exhausted`, auth,
 *    rate-limit) instead of a generic execution error.
 *
 * SANDBOX POSTURE. Goose owns its own tool execution and permission model; a
 * Goose tool event is provider-REPORTED state, never this framework's authority
 * decision, so it projects as not-authority (the ACP adapter's stance). Every
 * tool approval routes through the durable `RuntimeInteraction` model rather
 * than a self-granted allow. Isolation comes from the injected per-account
 * goose home plus the workspace boundary — never the owner's live home — so the
 * adapter is safe on the owner-local lane without claiming authority over
 * untrusted or metered capacity.
 */

// ---------------------------------------------------------------------------
// Goose ACP wire vocabulary (the injected/fixtured input)
// ---------------------------------------------------------------------------

/** Goose `ToolCallStatus` (`acp/server/tool_notifications.rs`). */
export type GooseToolCallStatus = "pending" | "in_progress" | "completed" | "failed";

/** `SessionUpdate::AgentMessageChunk(ContentChunk)` — a fragment of assistant text. */
export interface GooseAgentMessageChunk {
  readonly type: "agent_message_chunk";
  readonly text: string;
  /** Groups fragments of one assistant message; defaults to a per-turn key. */
  readonly messageKey?: string;
}

/** `SessionUpdate::AgentThoughtChunk` — a fragment of model reasoning/thinking. */
export interface GooseAgentThoughtChunk {
  readonly type: "agent_thought_chunk";
  readonly text: string;
  readonly messageKey?: string;
}

/** `SessionUpdate::ToolCall(ToolCall)` — the runtime invoked a tool. */
export interface GooseToolCall {
  readonly type: "tool_call";
  readonly toolCallId: string;
  /** Goose tool id, extension-prefixed (`developer__shell`, `developer__text_editor`). */
  readonly toolName: string;
  readonly inputRef?: string;
}

/**
 * `SessionUpdate::ToolCallUpdate(ToolCallUpdate{ fields.status })`. Only the
 * terminal `completed`/`failed` states cross onto the neutral stream (as
 * `tool.result` / `tool.error`); `pending`/`in_progress` are progress and
 * project to nothing, matching the ACP bridge's stance.
 */
export interface GooseToolCallUpdate {
  readonly type: "tool_call_update";
  readonly toolCallId: string;
  readonly toolName: string;
  readonly status: GooseToolCallStatus;
  /** Public-safe failure summary for the `tool.error` projection. */
  readonly messageSafe?: string;
}

/**
 * The `session/request_permission` request Goose issues to the client. Routes
 * through {@link goosePermissionToRuntimeInteractionPayload}, NOT the event
 * stream — an approval is not a transcript item.
 */
export interface GoosePermissionRequest {
  readonly type: "permission_request";
  readonly toolCallId: string;
  readonly toolName: string;
  readonly displayText?: string;
  /** True when the framework raised the request for an inactive built-in. */
  readonly inactiveBuiltin?: boolean;
}

/**
 * `SessionUpdate::UsageUpdate(UsageUpdate)` / the goose custom
 * `MessageUsage` notification. Carries exact token counts. Like the generic ACP
 * lane, usage does NOT ride the neutral turn stream — it is recorded through the
 * durable usage accountant — so this projects to no stream event; a live
 * transport reads it with {@link gooseUsageUpdateToKhalaUsage}.
 */
export interface GooseUsageUpdate {
  readonly type: "usage_update";
  readonly inputTokens?: number;
  readonly outputTokens?: number;
  readonly totalTokens?: number;
  readonly cacheReadTokens?: number;
  readonly cacheWriteTokens?: number;
}

/**
 * The `session/prompt` response terminator — Goose `StopReason::{EndTurn,
 * Cancelled}` (`acp/server.rs`). `end_turn` / `cancelled` map onto the neutral
 * finish reason through the ACP adapter's own stop-reason mapping.
 */
export interface GoosePromptStop {
  readonly type: "prompt_stop";
  readonly stopReason: "end_turn" | "cancelled" | "max_tokens" | "refusal";
}

/** The Goose ACP session-update vocabulary this adapter consumes. */
export type GooseSessionUpdate =
  | GooseAgentMessageChunk
  | GooseAgentThoughtChunk
  | GooseToolCall
  | GooseToolCallUpdate
  | GoosePermissionRequest
  | GooseUsageUpdate
  | GoosePromptStop;

// ---------------------------------------------------------------------------
// Projection: Goose SessionUpdate -> neutral ACP adapter input
// ---------------------------------------------------------------------------

/**
 * Project ONE Goose `SessionUpdate` onto zero or more {@link AcpAdapterEvent}s,
 * the neutral input the ACP factory folds onto `KhalaRuntimeEvent`. Non-terminal
 * tool updates, permission requests, and usage updates consume no transcript
 * event (they route through progress / RuntimeInteraction / the usage
 * accountant), exactly as the ACP bridge treats them.
 */
export const gooseUpdateToAcpEvents = (
  update: GooseSessionUpdate,
): ReadonlyArray<AcpAdapterEvent> => {
  switch (update.type) {
    case "agent_message_chunk":
      return [
        {
          type: "acp_text_delta",
          text: update.text,
          ...(update.messageKey === undefined ? {} : { messageKey: update.messageKey }),
        },
      ];
    case "agent_thought_chunk":
      return [
        {
          type: "acp_thought_delta",
          text: update.text,
          ...(update.messageKey === undefined ? {} : { messageKey: update.messageKey }),
        },
      ];
    case "tool_call":
      return [
        {
          type: "acp_tool_call",
          toolCallId: update.toolCallId,
          toolName: update.toolName,
          ...(update.inputRef === undefined ? {} : { inputRef: update.inputRef }),
        },
      ];
    case "tool_call_update":
      // Only terminal states cross the neutral boundary; progress is dropped.
      if (update.status !== "completed" && update.status !== "failed") return [];
      return [
        {
          type: "acp_tool_result",
          toolCallId: update.toolCallId,
          toolName: update.toolName,
          ok: update.status === "completed",
          ...(update.messageSafe === undefined ? {} : { messageSafe: update.messageSafe }),
        },
      ];
    case "permission_request":
      // Approvals are not transcript items; they route through RuntimeInteraction.
      return [];
    case "usage_update":
      // Usage rides the durable accountant, not the neutral turn stream.
      return [];
    case "prompt_stop":
      return [{ type: "acp_turn_stop", stopReason: update.stopReason }];
  }
};

/**
 * Project a whole Goose turn onto the ACP factory `script`: a synthesized
 * `acp_turn_started` boundary (Goose emits no explicit turn-start
 * `SessionUpdate` — `session/prompt` opens the turn) followed by the projected
 * updates. The Goose script MUST end with a `prompt_stop`, which yields the
 * single `turn.finished`.
 */
export const gooseScriptToAcpScript = (
  script: ReadonlyArray<GooseSessionUpdate>,
): ReadonlyArray<AcpAdapterEvent> => [
  { type: "acp_turn_started" },
  ...script.flatMap((update) => gooseUpdateToAcpEvents(update)),
];

/**
 * Map a Goose `UsageUpdate` onto the neutral {@link KhalaRuntimeUsage}. Absent
 * counters stay absent — usage is exact-only and is never fabricated.
 */
export const gooseUsageUpdateToKhalaUsage = (
  update: GooseUsageUpdate,
  usageRef: string,
): KhalaRuntimeUsage => ({
  usageRef,
  ...(update.inputTokens === undefined ? {} : { inputTokens: update.inputTokens }),
  ...(update.outputTokens === undefined ? {} : { outputTokens: update.outputTokens }),
  ...(update.cacheReadTokens === undefined ? {} : { cacheReadInputTokens: update.cacheReadTokens }),
  ...(update.cacheWriteTokens === undefined
    ? {}
    : { cacheWriteInputTokens: update.cacheWriteTokens }),
  ...(update.totalTokens === undefined ? {} : { totalTokens: update.totalTokens }),
});

// ---------------------------------------------------------------------------
// Approval -> RuntimeInteraction + Goose permission-option vocabulary
// ---------------------------------------------------------------------------

/**
 * Project a Goose permission request onto the canonical `tool_approval`
 * `RuntimeInteractionPayload`, reusing the ACP adapter's approval helper so
 * every harness approval — Goose included — routes through the ONE durable
 * interaction model. The carried authority is `operator_escalation_required`
 * (owner decision pending), never a self-granted allow.
 */
export const goosePermissionToRuntimeInteractionPayload = (
  request: GoosePermissionRequest,
): RuntimeInteractionPayload =>
  acpPermissionToRuntimeInteractionPayload({
    type: "acp_permission_request",
    toolCallId: request.toolCallId,
    toolName: request.toolName,
    ...(request.displayText === undefined ? {} : { displayText: request.displayText }),
    ...(request.inactiveBuiltin === undefined ? {} : { inactiveBuiltin: request.inactiveBuiltin }),
  });

/** Goose `PermissionOptionKind` (`acp/server.rs` permission options). */
export const GOOSE_PERMISSION_OPTION_KINDS = [
  "allow_always",
  "allow_once",
  "reject_once",
  "reject_always",
] as const;
export type GoosePermissionOptionKind = (typeof GOOSE_PERMISSION_OPTION_KINDS)[number];

/**
 * A harness approval decision -> the Goose `PermissionOptionKind` the client
 * selects in its `RequestPermissionOutcome`. `allow-session` picks
 * `allow_always` (remember for the session); `deny` picks `reject_once` (a
 * single denial, never the harder `reject_always`, which is an owner policy
 * choice the harness does not infer).
 */
export const gooseOptionForApprovalDecision: Readonly<
  Record<HarnessToolApprovalDecision, GoosePermissionOptionKind>
> = {
  "allow-once": "allow_once",
  "allow-session": "allow_always",
  deny: "reject_once",
};

/** The inverse: a Goose selected `PermissionOptionKind` -> harness decision. */
export const harnessDecisionForGooseOption = (
  kind: GoosePermissionOptionKind,
): HarnessToolApprovalDecision => {
  switch (kind) {
    case "allow_always":
      return "allow-session";
    case "allow_once":
      return "allow-once";
    case "reject_once":
    case "reject_always":
      return "deny";
  }
};

// ---------------------------------------------------------------------------
// Failure classification
// ---------------------------------------------------------------------------

/**
 * Classify a public-safe Goose ACP prompt-error tag/detail onto the shared
 * neutral {@link ModelFailureClass} vocabulary
 * (`@openagentsinc/agent-runtime-schema` `model-failure.ts`). Goose surfaces
 * prompt errors such as `credits_exhausted` and ACP auth/rate-limit errors; a
 * live Goose transport uses this to mint a typed turn failure so provider
 * capacity failures surface honestly instead of as a generic execution error.
 * Anything without an honest semantic match is `unknown`.
 */
export const gooseModelFailureClass = (detail: string): ModelFailureClass => {
  const lower = detail.toLowerCase();
  if (
    lower.includes("credits_exhausted") ||
    lower.includes("quota") ||
    lower.includes("usage limit")
  ) {
    return "account_exhausted";
  }
  if (
    lower.includes("rate limit") ||
    lower.includes("rate_limit") ||
    lower.includes("429") ||
    lower.includes("too many requests")
  ) {
    return "account_rate_limited";
  }
  if (
    lower.includes("unauthorized") ||
    lower.includes("unauthenticated") ||
    lower.includes("authentication") ||
    lower.includes("credential") ||
    lower.includes("401") ||
    lower.includes("forbidden")
  ) {
    return "auth_required";
  }
  return "unknown";
};

// ---------------------------------------------------------------------------
// Isolated goose-home guard
// ---------------------------------------------------------------------------

/**
 * True when the configured goose home is refused: empty, the owner's live XDG
 * config/data homes (`.../.config/goose`, `.../.local/share/goose`), or any
 * path with a `.goose` segment. Isolated per-account homes must use a distinct
 * directory, mirroring the Codex `~/.codex` and Pi `~/.pi` refusals.
 */
export const isRefusedGooseHome = (home: string): boolean => {
  const trimmed = home.trim();
  if (trimmed.length === 0) return true;
  const normalized = trimmed.replace(/\\+/g, "/").replace(/\/+$/, "");
  const segments = normalized.split("/");
  if (segments.includes(".goose")) return true;
  return (
    normalized.endsWith("/.config/goose") ||
    normalized.endsWith("/.local/share/goose") ||
    normalized === ".config/goose" ||
    normalized === ".local/share/goose"
  );
};

// ---------------------------------------------------------------------------
// Adapter factory
// ---------------------------------------------------------------------------

/**
 * Goose's core `developer` extension tools, declared with their common-name
 * mapping so the framework can normalize tool events and route built-in
 * filtering/approval. Goose tool ids are extension-prefixed
 * (`<extension>__<tool>`), so the native names are kept verbatim.
 */
export const GOOSE_BUILTIN_TOOLS: ReadonlyArray<HarnessBuiltinTool> = [
  { nativeName: "developer__shell", commonName: "bash", description: "run a shell command" },
  {
    nativeName: "developer__text_editor",
    commonName: "edit",
    description: "view, write, or str-replace a file",
  },
  { nativeName: "developer__list_windows", description: "list open windows" },
];

/** Configuration for {@link makeGooseHarnessAdapter}. */
export interface GooseHarnessAdapterConfig {
  /**
   * Isolated per-account goose config/data home. REQUIRED and validated at
   * `start`: the owner's live `~/.config/goose` / `~/.local/share/goose` /
   * `~/.goose` trees are refused (they hold live sessions, credentials, and
   * settings — the goose equivalent of clobbering `~/.codex`).
   */
  readonly gooseHome: string;
  /** Stable kebab-case slug; defaults to `goose`. */
  readonly harnessId?: string;
  /**
   * Scripted Goose `SessionUpdate` sequence replayed for each prompt turn (no
   * live goose peer). MUST end with a `prompt_stop`. Defaults to a
   * representative developer-tool turn.
   */
  readonly script?: ReadonlyArray<GooseSessionUpdate>;
  readonly supportsSuspend?: boolean;
  readonly supportsContinue?: boolean;
  readonly supportsCompact?: boolean;
  readonly supportsDetach?: boolean;
}

/**
 * A representative Goose turn: a thought, assistant text, a shell tool call
 * gated by a permission request, its completed result, exact usage, and a
 * finished turn.
 */
export const DEFAULT_GOOSE_SCRIPT: ReadonlyArray<GooseSessionUpdate> = [
  { type: "agent_thought_chunk", text: "Reviewing the failing test." },
  { type: "agent_message_chunk", text: "Reading the file." },
  { type: "tool_call", toolCallId: "toolcall.goose.1", toolName: "developer__shell" },
  {
    type: "permission_request",
    toolCallId: "toolcall.goose.1",
    toolName: "developer__shell",
    displayText: "Allow goose to run a shell command?",
  },
  {
    type: "tool_call_update",
    toolCallId: "toolcall.goose.1",
    toolName: "developer__shell",
    status: "completed",
  },
  { type: "usage_update", inputTokens: 40, outputTokens: 12, totalTokens: 52 },
  { type: "agent_message_chunk", text: " Done." },
  { type: "prompt_stop", stopReason: "end_turn" },
];

/**
 * Build a Goose {@link AgentHarness} by configuring the neutral ACP factory
 * with a Goose peer profile (the Cursor-adapter approach), then wrapping its
 * `start` with the isolated goose-home guard.
 *
 * Capability honesty:
 * - Approvals do NOT ride the native `submitToolApproval` channel — Goose
 *   permission requests route through the durable `RuntimeInteraction` model
 *   (`goosePermissionToRuntimeInteractionPayload`), so
 *   `supportsBuiltinToolApprovals` is false.
 * - No native built-in tool filtering (`supportsBuiltinToolFiltering` false).
 * - `suspend`/`continue` are LOSSLESS (the ACP attach model replays the buffered
 *   projected tail); `compact` and `detach` are supported (Goose has
 *   summarization and `session/load`), each overridable through the config.
 */
export const makeGooseHarnessAdapter = (config: GooseHarnessAdapterConfig): AgentHarness => {
  const harnessId = config.harnessId ?? "goose";
  const gooseScript = config.script ?? DEFAULT_GOOSE_SCRIPT;

  const acpAdapter = makeAcpHarnessAdapter({
    harnessId,
    // Goose has no dedicated member in the harness-kind vocabulary yet.
    harnessKind: "custom",
    adapterKind: "agent_client_protocol",
    script: gooseScriptToAcpScript(gooseScript),
    builtinTools: GOOSE_BUILTIN_TOOLS,
    // Goose approvals ride RuntimeInteraction, not the native channel.
    supportsBuiltinToolApprovals: false,
    supportsBuiltinToolFiltering: false,
    supportsSuspend: config.supportsSuspend ?? true,
    supportsContinue: config.supportsContinue ?? true,
    supportsCompact: config.supportsCompact ?? true,
    supportsDetach: config.supportsDetach ?? true,
    // ACP attach continuation replays the buffered tail losslessly.
    continueIsLossy: false,
  });

  const start = (options: HarnessStartOptions) =>
    isRefusedGooseHome(config.gooseHome)
      ? Effect.fail(
          new HarnessStartError({
            harnessId,
            sessionId: options.sessionId,
            failureClass: "goose_home_not_isolated",
            detail:
              "gooseHome must be an isolated per-account home; the owner's live ~/.config/goose, ~/.local/share/goose, or ~/.goose tree is refused",
          }),
        )
      : acpAdapter.start(options);

  return { ...acpAdapter, harnessId, start };
};
