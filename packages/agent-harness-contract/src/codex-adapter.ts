import { Effect, Option, Ref, Schema as S, Stream } from "effect";
import {
  decodeKhalaRuntimeEvent,
  KhalaRuntimeEventSchemaLiteral,
  type KhalaRuntimeFinishReason,
  type KhalaRuntimeSource,
  type KhalaRuntimeUsage,
  type RuntimeInteractionPayload,
} from "@openagentsinc/agent-runtime-schema";
import type { AgentHarness, HarnessStartOptions } from "./adapter.ts";
import { HarnessStartError } from "./adapter.ts";
import { HarnessCapabilityUnsupported } from "./capability.ts";
import { type HarnessToolIdentity, toolIdentity } from "./common-tool.ts";
import { buildTextDelta, buildTurnFinished, buildTurnStarted } from "./event-builder.ts";
import type { HarnessToolApprovalDecision } from "./host-tool.ts";
import type { HarnessContinuationState, HarnessResumeState } from "./lifecycle-state.ts";
import type {
  HarnessContinueTurnOptions,
  HarnessPromptControl,
  HarnessPromptTurnOptions,
  HarnessSession,
  HarnessTurnResult,
} from "./session.ts";
import { HarnessTurnError } from "./session.ts";
import type { HarnessStreamEvent } from "./stream.ts";

/**
 * Codex harness adapter (HW-01).
 *
 * A generalization of the OpenAgents monorepo's production Codex integration
 * into the neutral harness contract: the app-server turn loop
 * (`apps/openagents-desktop/src/codex-app-server-turn.ts` over
 * `codex-app-server-client.ts` / `codex-app-server-supervisor.ts`, speaking
 * the generated `@openagentsinc/codex-app-server-protocol` v2 vocabulary) and
 * the `codex exec --json` child lane
 * (`codex-local-runtime.ts` / `codex-child-runtime.ts`). The product-coupled
 * pieces (Electron IPC envelope, workbench items, supervisor leases) stay
 * behind the injected transport seam; the event translation and turn
 * lifecycle here mirror those sources and project onto `KhalaRuntimeEvent`.
 *
 * Two transport modes, both fixture-drivable through an injected seam so no
 * live `codex` binary is required by tests:
 *
 * - `app-server`: one long-lived supervised `codex app-server` process speaking
 *   JSON-RPC over stdio (the v2 `thread/*` + `turn/*` protocol — `initialize`,
 *   `thread/start` / `thread/resume`, `turn/start`, `turn/interrupt`). The
 *   preferred mode: turns attach to a live thread, native approval requests
 *   (`execCommandApproval` / `applyPatchApproval` server->client requests)
 *   route through the durable `RuntimeInteraction` model, and suspend/continue
 *   is lossless over the buffered projected turn.
 * - `exec`: `codex exec --json` spawned once per turn. Cross-turn continuity
 *   rides the Codex thread id (`codex exec resume <threadId>` / SDK
 *   `resumeThread`). Suspend/continue is an HONEST DEGRADED RERUN: the
 *   continuation resumes the thread with a bounded continue prompt and the
 *   tail after the cursor is recomputed, never replayed exactly (`lossy:
 *   true`). Approvals are refused — `codex exec` runs non-interactively under
 *   the approval policy fixed at spawn.
 *
 * The event vocabulary below is a self-contained LOCAL union covering the
 * public-safe, replayable subset of both wires. Each variant documents the
 * wire spelling it mirrors (app-server v2 camelCase notifications, exec JSONL
 * snake_case items). Raw command text, raw file contents, and provider
 * payloads are intentionally NOT modelled — they never cross into a neutral
 * event; only bounded display strings and refs do.
 *
 * CODEX_HOME is a required injected config value pointing at an ISOLATED
 * account home (for example `<pylon home>/accounts/codex/<ref>`), or, for
 * owner-local use, the developer's currently-authenticated default Codex home
 * (omit `codexHome`). The adapter runs exec/app-server turns only and NEVER
 * runs a login flow, so it cannot clobber the owner's session. The adapter
 * never defaults to `~/.codex` and refuses a config that points there, because
 * `codex login` clears `~/.codex/auth.json` and any accidental use of the
 * default home can destroy the owner's live Codex session.
 */

// ---------------------------------------------------------------------------
// Local Codex event vocabulary
// ---------------------------------------------------------------------------

/** Item lifecycle status shared by both wires (`in_progress`/`inProgress`, …). */
export type CodexItemStatus = "in_progress" | "completed" | "failed";

/**
 * One Codex thread item, normalized across the two wires: exec JSONL
 * `item.completed.item` (snake_case `item.type`) and app-server v2
 * `item/started` / `item/completed` (camelCase `item.type` — `agentMessage`,
 * `commandExecution`, `fileChange`, `mcpToolCall`, `webSearch`). The fields
 * kept are the public-safe subset each projection reads.
 */
export type CodexThreadItem =
  | CodexAgentMessageItem
  | CodexReasoningItem
  | CodexCommandExecutionItem
  | CodexFileChangeItem
  | CodexMcpToolCallItem
  | CodexWebSearchItem;

/** exec `agent_message` / app-server `agentMessage` — final assistant text. */
export interface CodexAgentMessageItem {
  readonly itemType: "agent_message";
  readonly id: string;
  readonly text: string;
}

/** exec `reasoning` / app-server `reasoning` — bounded reasoning summary text. */
export interface CodexReasoningItem {
  readonly itemType: "reasoning";
  readonly id: string;
  readonly text: string;
}

/**
 * exec `command_execution` / app-server `commandExecution`. `commandDisplay`
 * is a bounded display string, never the raw argv; raw aggregated output stays
 * on the wire side of the seam.
 */
export interface CodexCommandExecutionItem {
  readonly itemType: "command_execution";
  readonly id: string;
  readonly commandDisplay: string;
  readonly status: CodexItemStatus;
  readonly exitCode?: number;
}

/** exec `file_change` / app-server `fileChange` — per-path change summaries. */
export interface CodexFileChangeItem {
  readonly itemType: "file_change";
  readonly id: string;
  readonly status: CodexItemStatus;
  readonly changes: ReadonlyArray<{
    readonly path: string;
    readonly kind: "add" | "delete" | "update";
  }>;
}

/** exec `mcp_tool_call` / app-server `mcpToolCall`. */
export interface CodexMcpToolCallItem {
  readonly itemType: "mcp_tool_call";
  readonly id: string;
  readonly serverName: string;
  readonly toolName: string;
  readonly status: CodexItemStatus;
}

/** exec `web_search` / app-server `webSearch`. */
export interface CodexWebSearchItem {
  readonly itemType: "web_search";
  readonly id: string;
  readonly status: CodexItemStatus;
}

/** Exact token usage reported by Codex (`turn.completed.usage` / `thread/tokenUsage/updated`). */
export interface CodexTokenUsage {
  readonly inputTokens: number;
  readonly cachedInputTokens: number;
  readonly outputTokens: number;
  readonly reasoningOutputTokens: number;
}

/** exec `thread.started` / app-server `thread/started` — the runtime-native thread id. */
export interface CodexThreadStartedEvent {
  readonly type: "thread.started";
  readonly threadId: string;
}

/** exec `turn.started` / app-server `turn/started`. */
export interface CodexTurnStartedEvent {
  readonly type: "turn.started";
}

/** app-server `item/agentMessage/delta` — streamed assistant text (no exec equivalent). */
export interface CodexAgentMessageDeltaEvent {
  readonly type: "agent_message.delta";
  readonly itemId: string;
  readonly delta: string;
}

/** app-server `item/reasoning/textDelta` — streamed reasoning (no exec equivalent). */
export interface CodexReasoningDeltaEvent {
  readonly type: "reasoning.delta";
  readonly itemId: string;
  readonly delta: string;
}

/** app-server `item/started` — a thread item began (exec surfaces completions only). */
export interface CodexItemStartedEvent {
  readonly type: "item.started";
  readonly item: CodexThreadItem;
}

/** exec `item.completed` / app-server `item/completed` — a thread item settled. */
export interface CodexItemCompletedEvent {
  readonly type: "item.completed";
  readonly item: CodexThreadItem;
}

/**
 * App-server native approval request (`execCommandApproval` /
 * `applyPatchApproval` server->client JSON-RPC requests). `requestId` is the
 * JSON-RPC request id the transport answers through
 * {@link CodexAppServerTransport.respondToApproval}; `callId` correlates with
 * the `command_execution` / `file_change` item of the same call. Projects to
 * NO stream event — approvals route through the durable `RuntimeInteraction`
 * model via {@link codexApprovalToRuntimeInteractionPayload}.
 */
export interface CodexApprovalRequestedEvent {
  readonly type: "approval.requested";
  readonly requestId: string;
  readonly callId: string;
  readonly toolKind: "exec_command" | "apply_patch";
  /** Bounded public-safe display summary, never the raw command. */
  readonly displayText?: string;
}

/** app-server `thread/tokenUsage/updated` — remembered, surfaced on `turn.finished`. */
export interface CodexTokenUsageUpdatedEvent {
  readonly type: "token_usage.updated";
  readonly usage: CodexTokenUsage;
}

/**
 * exec `turn.completed` (carries exact usage inline) / app-server
 * `turn/completed` (usage arrives separately via `token_usage.updated`).
 */
export interface CodexTurnCompletedEvent {
  readonly type: "turn.completed";
  readonly status: "completed" | "failed" | "interrupted";
  readonly usage?: CodexTokenUsage;
}

/** exec `turn.failed` — the turn errored with a public-safe message. */
export interface CodexTurnFailedEvent {
  readonly type: "turn.failed";
  readonly messageSafe: string;
}

/** exec/app-server `error`. `willRetry: true` projects to no neutral event. */
export interface CodexErrorEvent {
  readonly type: "error";
  readonly messageSafe: string;
  readonly willRetry?: boolean;
}

/** The neutral local Codex event vocabulary this adapter consumes. */
export type CodexEvent =
  | CodexThreadStartedEvent
  | CodexTurnStartedEvent
  | CodexAgentMessageDeltaEvent
  | CodexReasoningDeltaEvent
  | CodexItemStartedEvent
  | CodexItemCompletedEvent
  | CodexApprovalRequestedEvent
  | CodexTokenUsageUpdatedEvent
  | CodexTurnCompletedEvent
  | CodexTurnFailedEvent
  | CodexErrorEvent;

// ---------------------------------------------------------------------------
// Transport seams (injected; fixtures drive them in tests)
// ---------------------------------------------------------------------------

/** Typed transport/spawn failure. `failureClass` carries the operator-facing
 * class (`account_exhausted`, `account_rate_limited`, `spawn_failed`, …) so
 * provider-capacity failures surface honestly instead of as generic errors. */
export class CodexTransportError extends S.TaggedErrorClass<CodexTransportError>()(
  "AgentHarness.CodexTransportError",
  {
    failureClass: S.String,
    detail: S.optionalKey(S.String),
    cause: S.optionalKey(S.Defect()),
  },
) {}

/** Decision vocabulary of the app-server approval response (`ReviewDecision`). */
export const CODEX_APPROVAL_DECISIONS = [
  "approved",
  "approved_for_session",
  "denied",
  "abort",
] as const;
export type CodexApprovalDecision = (typeof CODEX_APPROVAL_DECISIONS)[number];

/**
 * The app-server JSON-RPC seam: one long-lived supervised `codex app-server`
 * process. A live implementation owns the stdio JSONL framing, the
 * `initialize`/`initialized` handshake, `thread/start` / `thread/resume`,
 * `turn/start`, `turn/interrupt`, and answering the `execCommandApproval` /
 * `applyPatchApproval` server requests. Tests script it with fixtures.
 * `runTurn` resolves with the ordered turn events once the turn settles; the
 * adapter buffers them so suspend/continue stays cursor-exact.
 */
export interface CodexAppServerTransport {
  /** `thread/start` (fresh) or `thread/resume` (with `resumeThreadId`). */
  readonly startThread: (params: {
    readonly codexHome?: string;
    readonly workingDirectory?: string;
    readonly model?: string;
    readonly resumeThreadId?: string;
  }) => Effect.Effect<{ readonly threadId: string }, CodexTransportError>;
  /** `turn/start` — resolves with the ordered events of the settled turn. */
  readonly runTurn: (params: {
    readonly threadId: string;
    readonly prompt: string;
  }) => Effect.Effect<ReadonlyArray<CodexEvent>, CodexTransportError>;
  /**
   * LIVE-STREAMING variant of {@link runTurn} (openagents#9167). Returns a
   * {@link Stream} that emits each {@link CodexEvent} the instant the
   * app-server produces it and completes when the turn settles, instead of
   * buffering the whole turn and resolving once. When a transport provides
   * this, the adapter projects every event onto the neutral
   * {@link HarnessStreamEvent} stream AS IT ARRIVES — the exact live drive the
   * claude adapter gets from `Stream.fromAsyncIterable` — so text/reasoning
   * deltas and tool rows reach the renderer live rather than only after the
   * turn ends. A live implementation typically backs this with a
   * `Queue`/`Stream.fromQueue` fed by the JSON-RPC notification handler.
   *
   * Back-compat: OPTIONAL. When a transport omits it, the adapter uses the
   * batch {@link runTurn} above unchanged.
   *
   * The FULLY-CONSUMED ordered event set is identical to what {@link runTurn}
   * would have produced for the same wire, so the settled transcript stays
   * cursor-exact. Suspend/continue over a live turn is honestly DEGRADED
   * (`lossy: true`): a turn produced live has no already-computed tail to
   * replay from a mid-turn cursor, so the adapter never claims a lossless
   * remainder for the streaming path.
   */
  readonly runTurnStreaming?: (params: {
    readonly threadId: string;
    readonly prompt: string;
  }) => Stream.Stream<CodexEvent, CodexTransportError>;
  /** Answer a pending `execCommandApproval` / `applyPatchApproval` request. */
  readonly respondToApproval: (params: {
    readonly requestId: string;
    readonly decision: CodexApprovalDecision;
  }) => Effect.Effect<void, CodexTransportError>;
  /** `turn/interrupt` — ACK admits the interruption; completion still arrives as `turn/completed`. */
  readonly interruptTurn: (params: {
    readonly threadId: string;
    readonly turnId?: string;
  }) => Effect.Effect<void, CodexTransportError>;
  /** Optional `turn/steer` — mid-turn user-message injection where supported. */
  readonly steerTurn?: (params: {
    readonly threadId: string;
    readonly text: string;
  }) => Effect.Effect<void, CodexTransportError>;
  /** Tear the supervised process down. Idempotent. */
  readonly shutdown: () => Effect.Effect<void>;
}

/**
 * The `codex exec --json` seam: one bounded spawn per turn. A live
 * implementation spawns `codexBinaryPath exec --json --cd <workingDirectory>
 * [--model <model>] <prompt>` with `CODEX_HOME=<codexHome>` in the child env
 * (never the ambient process env), or resumes via the thread id when
 * `resumeThreadId` is present, and resolves with the parsed JSONL events.
 */
export interface CodexExecSpawner {
  readonly spawn: (params: {
    readonly codexBinaryPath: string;
    readonly codexHome?: string;
    readonly workingDirectory?: string;
    readonly model?: string;
    readonly prompt: string;
    readonly resumeThreadId?: string;
  }) => Effect.Effect<ReadonlyArray<CodexEvent>, CodexTransportError>;
}

// ---------------------------------------------------------------------------
// Projection
// ---------------------------------------------------------------------------

/** Bounded sanitizer onto the Khala safe-ref alphabet (`/` and spaces become `-`). */
const toSafeRef = (value: string, fallback: string): string => {
  const cleaned = value
    .replace(/[^A-Za-z0-9._:-]+/g, "-")
    .replace(/^[^A-Za-z0-9]+/, "")
    .slice(0, 200);
  return cleaned.length === 0 ? fallback : cleaned;
};

/** The neutral `toolCallId` for a Codex call/item id (shared with approvals). */
export const codexToolCallId = (nativeId: string): string =>
  `toolcall.codex.${toSafeRef(nativeId, "unknown")}`;

/** Context threaded through {@link codexEventToKhalaEvents} while folding a turn. */
export interface CodexProjectionContext {
  readonly source: KhalaRuntimeSource;
  /** The neutral session/thread id every event carries (NOT the Codex-native thread id). */
  readonly threadId: string;
  readonly turnId: string;
  /** Allocate the next session-global sequence number. */
  readonly nextSequence: () => number;
  /** item/call id -> normalized identity, populated by `item.started`. */
  readonly toolIdentities: Map<string, HarnessToolIdentity>;
  /** item ids whose text already streamed as deltas (skip the completed echo). */
  readonly deltaStreamedItems: Set<string>;
  /** neutral toolCallId -> JSON-RPC approval request id awaiting a decision. */
  readonly pendingApprovals: Map<string, string>;
  /** Last exact token usage reported mid-turn (`token_usage.updated`). */
  readonly usageBox: { value: CodexTokenUsage | undefined };
  /** Codex-native thread id captured from `thread.started`. */
  readonly threadBox: { value: string | undefined };
}

const base = (ctx: CodexProjectionContext, sequence: number, eventSuffix: string) => ({
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
 * Provider-reported Codex tool authority. Codex executes its own built-ins
 * under the sandbox/approval policy fixed at spawn; a Codex tool event is
 * REPORTED state, never this framework's authority decision, so it is recorded
 * as not-authority with the standard blocker ref (same stance as the ACP
 * adapter's bridge projector).
 */
const providerReportedAuthority = (toolCallId: string, wireName: string) => ({
  authorityRef: `authority.codex.${toSafeRef(toolCallId, "unknown")}`,
  policyRef: "policy.codex_runtime",
  decisionRef: "decision.provider_reported_not_authority",
  toolRef: `toolref.codex.${toSafeRef(wireName, "unknown")}`,
  status: "denied" as const,
  allowed: false,
  blockerRefs: ["blocker.provider_event_not_authority"],
});

const itemToolIdentity = (item: CodexThreadItem): HarnessToolIdentity | undefined => {
  switch (item.itemType) {
    case "command_execution":
      return toolIdentity("shell", { providerExecuted: true });
    case "file_change":
      return toolIdentity("apply_patch", { providerExecuted: true });
    case "mcp_tool_call":
      return toolIdentity(`${item.serverName}.${item.toolName}`, { providerExecuted: true });
    case "web_search":
      return toolIdentity("web_search", { providerExecuted: true });
    case "agent_message":
    case "reasoning":
      return undefined;
  }
};

const itemFailed = (item: CodexThreadItem): boolean => {
  if (item.itemType === "command_execution") {
    return item.status === "failed" || (item.exitCode !== undefined && item.exitCode !== 0);
  }
  return "status" in item && item.status === "failed";
};

const buildReasoningDelta = (
  ctx: CodexProjectionContext,
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
  ctx: CodexProjectionContext,
  sequence: number,
  identity: HarnessToolIdentity,
  toolCallId: string,
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(ctx, sequence, "toolcall"),
    kind: "tool.call",
    toolCallId,
    toolName: identity.wireName,
    inputRef: `input.codex.${ctx.turnId}.${sequence}`,
    authority: providerReportedAuthority(toolCallId, identity.wireName),
  });

const buildToolResult = (
  ctx: CodexProjectionContext,
  sequence: number,
  identity: HarnessToolIdentity,
  toolCallId: string,
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(ctx, sequence, "toolresult"),
    kind: "tool.result",
    toolCallId,
    toolName: identity.wireName,
    resultRef: `result.codex.${ctx.turnId}.${sequence}`,
    authority: providerReportedAuthority(toolCallId, identity.wireName),
    providerExecuted: true,
  });

const buildToolError = (
  ctx: CodexProjectionContext,
  sequence: number,
  identity: HarnessToolIdentity,
  toolCallId: string,
  messageSafe: string,
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(ctx, sequence, "toolerror"),
    kind: "tool.error",
    toolCallId,
    toolName: identity.wireName,
    errorRef: `error.codex.${ctx.turnId}.${sequence}`,
    messageSafe,
    authority: providerReportedAuthority(toolCallId, identity.wireName),
    providerExecuted: true,
  });

const buildFileChange = (
  ctx: CodexProjectionContext,
  sequence: number,
  itemId: string,
  index: number,
  change: { readonly path: string; readonly kind: "add" | "delete" | "update" },
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(ctx, sequence, "filechange"),
    kind: "file.change",
    fileChange: {
      fileChangeRef: `filechange.codex.${toSafeRef(itemId, "unknown")}.${index}`,
      pathRef: `path.${toSafeRef(change.path, "unknown")}`,
      op: change.kind === "add" ? "created" : change.kind === "delete" ? "deleted" : "modified",
    },
  });

const toKhalaUsage = (
  ctx: CodexProjectionContext,
  sequence: number,
  usage: CodexTokenUsage,
): KhalaRuntimeUsage => ({
  usageRef: `usage.codex.${ctx.turnId}.${sequence}`,
  inputTokens: usage.inputTokens,
  outputTokens: usage.outputTokens,
  reasoningTokens: usage.reasoningOutputTokens,
  cacheReadInputTokens: usage.cachedInputTokens,
  // Reasoning output tokens count into the total, matching the exact-only
  // Pylon/Codex accounting posture.
  totalTokens: usage.inputTokens + usage.outputTokens + usage.reasoningOutputTokens,
});

const turnStatusToFinishReason = (
  status: CodexTurnCompletedEvent["status"],
): KhalaRuntimeFinishReason =>
  status === "completed" ? "stop" : status === "interrupted" ? "interrupted" : "error";

const projectItemCompleted = (
  ctx: CodexProjectionContext,
  item: CodexThreadItem,
): ReadonlyArray<HarnessStreamEvent> => {
  switch (item.itemType) {
    case "agent_message": {
      // App-server mode already streamed this text as deltas; do not echo it.
      if (ctx.deltaStreamedItems.has(item.id)) return [];
      return [
        buildTextDelta({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence: ctx.nextSequence(),
          source: ctx.source,
          messageId: `msg.codex.${toSafeRef(item.id, "unknown")}`,
          text: item.text,
        }),
      ];
    }
    case "reasoning": {
      if (ctx.deltaStreamedItems.has(item.id)) return [];
      return [
        buildReasoningDelta(ctx, ctx.nextSequence(), {
          messageId: `msg.codex.${toSafeRef(item.id, "unknown")}`,
          text: item.text,
        }),
      ];
    }
    case "command_execution":
    case "file_change":
    case "mcp_tool_call":
    case "web_search": {
      const events: Array<HarnessStreamEvent> = [];
      const toolCallId = codexToolCallId(item.id);
      let identity = ctx.toolIdentities.get(item.id);
      if (identity === undefined) {
        // exec mode surfaces completions only: synthesize the paired call so
        // tool-call correlation holds on the neutral stream.
        identity = itemToolIdentity(item) ?? toolIdentity(item.itemType);
        ctx.toolIdentities.set(item.id, identity);
        events.push(buildToolCall(ctx, ctx.nextSequence(), identity, toolCallId));
      }
      if (itemFailed(item)) {
        const messageSafe =
          item.itemType === "command_execution" && item.exitCode !== undefined
            ? `Codex ${identity.wireName} exited with code ${item.exitCode}`
            : `Codex ${identity.wireName} reported failure`;
        events.push(buildToolError(ctx, ctx.nextSequence(), identity, toolCallId, messageSafe));
      } else {
        events.push(buildToolResult(ctx, ctx.nextSequence(), identity, toolCallId));
      }
      if (item.itemType === "file_change") {
        item.changes.forEach((change, index) => {
          events.push(buildFileChange(ctx, ctx.nextSequence(), item.id, index, change));
        });
      }
      return events;
    }
  }
};

/**
 * Pure projection of ONE Codex event onto zero or more neutral
 * {@link HarnessStreamEvent}s. Sequence numbers come from `ctx.nextSequence`
 * so a caller folds a whole turn while keeping session-global cursors
 * contiguous. Approval requests, mid-turn token usage, retryable errors, and
 * `thread.started` consume no sequence — they mutate the context (pending
 * approvals, usage box, thread box) instead of the transcript.
 */
export const codexEventToKhalaEvents = (
  event: CodexEvent,
  ctx: CodexProjectionContext,
): ReadonlyArray<HarnessStreamEvent> => {
  switch (event.type) {
    case "thread.started":
      ctx.threadBox.value = event.threadId;
      return [];
    case "turn.started":
      return [
        buildTurnStarted({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence: ctx.nextSequence(),
          source: ctx.source,
        }),
      ];
    case "agent_message.delta":
      ctx.deltaStreamedItems.add(event.itemId);
      return [
        buildTextDelta({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence: ctx.nextSequence(),
          source: ctx.source,
          messageId: `msg.codex.${toSafeRef(event.itemId, "unknown")}`,
          text: event.delta,
        }),
      ];
    case "reasoning.delta":
      ctx.deltaStreamedItems.add(event.itemId);
      return [
        buildReasoningDelta(ctx, ctx.nextSequence(), {
          messageId: `msg.codex.${toSafeRef(event.itemId, "unknown")}`,
          text: event.delta,
        }),
      ];
    case "item.started": {
      const identity = itemToolIdentity(event.item);
      if (identity === undefined) return [];
      ctx.toolIdentities.set(event.item.id, identity);
      return [buildToolCall(ctx, ctx.nextSequence(), identity, codexToolCallId(event.item.id))];
    }
    case "item.completed":
      return projectItemCompleted(ctx, event.item);
    case "approval.requested":
      // An approval is not a transcript item; it routes through
      // RuntimeInteraction and is answered via `submitToolApproval`.
      ctx.pendingApprovals.set(codexToolCallId(event.callId), event.requestId);
      return [];
    case "token_usage.updated":
      ctx.usageBox.value = event.usage;
      return [];
    case "turn.completed": {
      const sequence = ctx.nextSequence();
      const usage = event.usage ?? ctx.usageBox.value;
      return [
        buildTurnFinished({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence,
          source: ctx.source,
          finishReason: turnStatusToFinishReason(event.status),
          // Usage is exact-only: absent usage stays absent, never fabricated.
          ...(usage === undefined ? {} : { usage: toKhalaUsage(ctx, sequence, usage) }),
        }),
      ];
    }
    case "turn.failed":
      return [
        buildTurnFinished({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence: ctx.nextSequence(),
          source: ctx.source,
          finishReason: "error",
        }),
      ];
    case "error":
      if (event.willRetry === true) return [];
      return [
        buildTurnFinished({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence: ctx.nextSequence(),
          source: ctx.source,
          finishReason: "error",
        }),
      ];
  }
};

// ---------------------------------------------------------------------------
// Approval → RuntimeInteraction
// ---------------------------------------------------------------------------

/**
 * Project a Codex native approval request onto a canonical
 * `RuntimeInteractionPayload` of kind `tool_approval` — the durable,
 * provider-neutral approval model every harness approval routes through
 * (HARN-04 H4), exactly like the ACP adapter. The carried authority is
 * `operator_escalation_required` (owner decision pending), never a
 * self-granted allow. The eventual decision returns to the app-server through
 * `submitToolApproval` on the prompt control, which answers the pending
 * JSON-RPC request (`approved` / `approved_for_session` / `denied`).
 */
export const codexApprovalToRuntimeInteractionPayload = (
  event: CodexApprovalRequestedEvent,
): RuntimeInteractionPayload => {
  const identity = toolIdentity(event.toolKind === "exec_command" ? "shell" : "apply_patch", {
    providerExecuted: true,
  });
  const toolCallId = codexToolCallId(event.callId);
  return {
    kind: "tool_approval",
    displayText: event.displayText ?? `Allow Codex to run ${identity.wireName} in the workspace?`,
    toolCallId,
    toolName: identity.wireName,
    authority: {
      ...providerReportedAuthority(toolCallId, identity.wireName),
      status: "operator_escalation_required",
      blockerRefs: ["blocker.owner_approval"],
    },
  };
};

// ---------------------------------------------------------------------------
// Failure classification
// ---------------------------------------------------------------------------

/**
 * Classify a public-safe Codex failure detail into an operator-facing failure
 * class — the mandatory account-capacity vocabulary (`account_exhausted`,
 * `account_rate_limited`, auth-health) plus policy denial. Ported from the
 * desktop turn loop's `classifyFailure` keyword rules so provider capacity
 * failures surface honestly instead of as generic execution errors. Transport
 * implementations use it to mint `CodexTransportError.failureClass`; the
 * adapter passes the class through onto `HarnessTurnError` unchanged.
 */
export const classifyCodexFailureClass = (
  detail: string,
):
  | "account_reconnect_required"
  | "account_exhausted"
  | "account_rate_limited"
  | "policy_denied"
  | "execution_failed" => {
  const lower = detail.toLowerCase();
  if (
    lower.includes("unauthorized") ||
    lower.includes("authentication") ||
    lower.includes("login") ||
    lower.includes("credential") ||
    lower.includes("401")
  ) {
    return "account_reconnect_required";
  }
  if (
    lower.includes("denied by policy") ||
    lower.includes("policy denied") ||
    lower.includes("policy violation") ||
    lower.includes("approval policy")
  ) {
    return "policy_denied";
  }
  if (
    lower.includes("usage limit") ||
    lower.includes("quota") ||
    lower.includes("purchase more credits")
  ) {
    return "account_exhausted";
  }
  if (
    lower.includes("rate limit") ||
    lower.includes("429") ||
    lower.includes("too many requests")
  ) {
    return "account_rate_limited";
  }
  return "execution_failed";
};

/** Harness approval decision -> app-server `ReviewDecision`. */
const APPROVAL_DECISION_TO_CODEX: Readonly<
  Record<HarnessToolApprovalDecision, CodexApprovalDecision>
> = {
  "allow-once": "approved",
  "allow-session": "approved_for_session",
  deny: "denied",
};

// ---------------------------------------------------------------------------
// Adapter factory
// ---------------------------------------------------------------------------

/**
 * Config shared by both modes. `codexHome` selects the account: an isolated
 * account home for fleet use, or omitted for owner-local use of the
 * currently-authenticated default Codex home.
 */
export interface CodexAdapterCommonConfig {
  /** Stable kebab-case slug; defaults to `codex`. */
  readonly harnessId?: string;
  /** Path to the codex binary the transport/spawner should launch. */
  readonly codexBinaryPath: string;
  /**
   * CODEX_HOME for the session's account. Omit it (or pass a default-home
   * path) for OWNER-LOCAL mode: the spawner/transport must then leave
   * CODEX_HOME unset so the codex runtime uses the developer's
   * currently-authenticated default home. The adapter only runs
   * exec/app-server turns — never a login flow — so owner-local mode cannot
   * clobber the live session (owner decision 2026-07-22, #9161). Fleet and
   * multi-account callers keep passing explicit isolated homes.
   */
  readonly codexHome?: string;
  /** Working directory for the session (framework-created; `--cd` / `cwd`). */
  readonly workingDirectory?: string;
  /** Model override (`--model` / `turn/start.model`). */
  readonly model?: string;
}

/** App-server mode: long-lived supervised JSON-RPC process (preferred). */
export interface CodexAppServerAdapterConfig extends CodexAdapterCommonConfig {
  readonly mode: "app-server";
  readonly transport: CodexAppServerTransport;
}

/** Exec mode: `codex exec --json` per turn (degraded fallback). */
export interface CodexExecAdapterConfig extends CodexAdapterCommonConfig {
  readonly mode: "exec";
  readonly spawner: CodexExecSpawner;
}

export type CodexAdapterConfig = CodexAppServerAdapterConfig | CodexExecAdapterConfig;

/** Prompt used when exec mode re-drives an interrupted turn on resume. */
export const CODEX_EXEC_CONTINUE_PROMPT =
  "Continue the interrupted turn from where it stopped. Do not repeat completed work.";

/** Resume payload (`detach`/`stop` -> `start({ resumeFrom })`): the Codex thread id. */
export const CodexResumeData = S.Struct({
  threadId: S.optionalKey(S.NonEmptyString),
});
export interface CodexResumeData extends S.Schema.Type<typeof CodexResumeData> {}

const AppServerContinuationData = S.Struct({
  turnId: S.NonEmptyString,
  threadId: S.optionalKey(S.NonEmptyString),
  remaining: S.Array(S.Unknown),
});

const ExecContinuationData = S.Struct({
  turnId: S.NonEmptyString,
  threadId: S.optionalKey(S.NonEmptyString),
});

interface ActiveTurn {
  readonly turnId: string;
  readonly remaining: ReadonlyArray<HarnessStreamEvent>;
}

/**
 * Normalize the configured CODEX_HOME. `undefined`, empty, and default-home
 * shaped paths (`~/.codex`, any `/.codex` final segment) all select
 * OWNER-LOCAL mode (`undefined`): the currently-authenticated default home,
 * reached by leaving CODEX_HOME unset in the child environment.
 */
const normalizeCodexHome = (codexHome: string | undefined): string | undefined => {
  if (codexHome === undefined) return undefined;
  const trimmed = codexHome.trim();
  if (trimmed.length === 0) return undefined;
  const normalized = trimmed.replace(/\/+$/, "");
  if (normalized === "~/.codex" || normalized === ".codex" || normalized.endsWith("/.codex")) {
    return undefined;
  }
  return trimmed;
};

const makeControl = (params: {
  readonly harnessId: string;
  readonly sessionId: string;
  readonly turnId: string;
  readonly events: ReadonlyArray<HarnessStreamEvent>;
  readonly cursorRef: Ref.Ref<number>;
  readonly activeRef: Ref.Ref<Option.Option<ActiveTurn>>;
  readonly submitToolApproval: (
    toolCallId: string,
    decision: HarnessToolApprovalDecision,
  ) => Effect.Effect<void, HarnessTurnError | HarnessCapabilityUnsupported>;
  readonly submitUserMessage: (
    text: string,
  ) => Effect.Effect<void, HarnessTurnError | HarnessCapabilityUnsupported>;
  readonly interrupt: () => Effect.Effect<void>;
}): HarnessPromptControl => {
  const { turnId, events, cursorRef, activeRef } = params;

  const stream: Stream.Stream<HarnessStreamEvent, HarnessTurnError> = Stream.fromIterable(
    events,
  ).pipe(
    Stream.tap((event) =>
      Effect.gen(function* () {
        yield* Ref.set(cursorRef, event.sequence);
        yield* Ref.set(
          activeRef,
          Option.some({
            turnId,
            remaining: events.filter((e) => e.sequence > event.sequence),
          }),
        );
      }),
    ),
  );

  const done: Effect.Effect<HarnessTurnResult, HarnessTurnError> = Effect.gen(function* () {
    const cursor = yield* Ref.get(cursorRef);
    const active = yield* Ref.get(activeRef);
    const remaining = Option.match(active, {
      onNone: () => [] as ReadonlyArray<HarnessStreamEvent>,
      onSome: (a) => a.remaining,
    });
    return {
      turnId,
      finishReason: remaining.length === 0 ? "stop" : "interrupted",
      lastCursor: cursor,
    } satisfies HarnessTurnResult;
  });

  return {
    turnId,
    events: stream,
    done,
    // Codex executes its own built-ins; there is no host-tool channel here.
    submitToolResult: () =>
      Effect.fail(
        new HarnessTurnError({
          harnessId: params.harnessId,
          sessionId: params.sessionId,
          turnId,
          failureClass: "no_active_tool_call",
          detail: "codex runs its built-in tools natively; no host-tool result is awaited",
        }),
      ),
    submitToolApproval: params.submitToolApproval,
    submitUserMessage: params.submitUserMessage,
    interrupt: params.interrupt,
  };
};

/**
 * Build a Codex {@link AgentHarness} in one of the two modes.
 *
 * Capability honesty per mode:
 *
 * - `app-server`: `suspendTurn`/`continueTurn` are LOSSLESS and cursor-exact
 *   over the buffered projected turn (`lossy: false`); native approvals are
 *   supported (`supportsBuiltinToolApprovals: true`) and answered through the
 *   transport's pending JSON-RPC requests; `detach`/`stop` return the Codex
 *   thread id for `thread/resume`.
 * - `exec`: `suspendTurn` returns `lossy: true` and `continueTurn` is an
 *   honest DEGRADED RERUN — it resumes the Codex thread with a bounded
 *   continue prompt and recomputes the tail after the cursor; approvals are
 *   REFUSED (`CapabilityUnsupported("builtin_tool_approvals")`) because
 *   `codex exec` runs non-interactively; mid-turn user messages are refused.
 * - Both modes refuse `compact` (no client-driven compaction verb is modelled)
 *   and have no native built-in tool filtering.
 */
export const makeCodexHarnessAdapter = (config: CodexAdapterConfig): AgentHarness => {
  const harnessId = config.harnessId ?? "codex";

  const start = (options: HarnessStartOptions): Effect.Effect<HarnessSession, HarnessStartError> =>
    Effect.gen(function* () {
      const source: KhalaRuntimeSource = options.source;
      const sessionId = options.sessionId;

      // openagents#9167: app-server turns drive LIVE when the transport exposes
      // a streaming seam. Live turns project each event as it arrives and are
      // honestly lossy on suspend (no already-computed tail to replay).
      const appServerStreaming =
        config.mode === "app-server" && config.transport.runTurnStreaming !== undefined;

      // Owner-local mode: an omitted or default-home codexHome means the
      // currently-authenticated default Codex home (CODEX_HOME left unset).
      const effectiveCodexHome = normalizeCodexHome(config.codexHome);

      // Codex-native thread id for resume, seeded from durable lifecycle state.
      let seedThreadId: string | undefined;
      if (options.resumeFrom !== undefined) {
        const data = S.decodeUnknownSync(CodexResumeData)(options.resumeFrom.data);
        seedThreadId = data.threadId;
      }

      // Session-global monotonic sequence. A continuation-started session keeps
      // counting from where the export left off so cursors stay globally ordered.
      const seedSequence = options.continueFrom?.cursor ?? -1;
      const sequenceRef = yield* Ref.make(seedSequence + 1);
      const cursorRef = yield* Ref.make(seedSequence);
      const activeRef = yield* Ref.make<Option.Option<ActiveTurn>>(Option.none());
      const firstTurnRef = yield* Ref.make(true);
      const pendingApprovals = new Map<string, string>();

      let pendingContinuation: ActiveTurn | undefined;
      if (options.continueFrom !== undefined) {
        if (config.mode === "app-server") {
          const data = S.decodeUnknownSync(AppServerContinuationData)(options.continueFrom.data);
          seedThreadId = seedThreadId ?? data.threadId;
          pendingContinuation = {
            turnId: data.turnId,
            remaining: data.remaining as ReadonlyArray<HarnessStreamEvent>,
          };
        } else {
          const data = S.decodeUnknownSync(ExecContinuationData)(options.continueFrom.data);
          seedThreadId = seedThreadId ?? data.threadId;
          pendingContinuation = { turnId: data.turnId, remaining: [] };
        }
        yield* Ref.set(activeRef, Option.some(pendingContinuation));
      }

      const threadRef = yield* Ref.make<string | undefined>(seedThreadId);

      const isResume = options.resumeFrom !== undefined || options.continueFrom !== undefined;

      // App-server mode starts (or resumes) the live thread up front.
      if (config.mode === "app-server") {
        const started = yield* config.transport
          .startThread({
            ...(effectiveCodexHome === undefined ? {} : { codexHome: effectiveCodexHome }),
            ...(config.workingDirectory === undefined
              ? {}
              : { workingDirectory: config.workingDirectory }),
            ...(config.model === undefined ? {} : { model: config.model }),
            ...(seedThreadId === undefined ? {} : { resumeThreadId: seedThreadId }),
          })
          .pipe(
            Effect.mapError(
              (error) =>
                new HarnessStartError({
                  harnessId,
                  sessionId,
                  failureClass: error.failureClass,
                  ...(error.detail === undefined ? {} : { detail: error.detail }),
                }),
            ),
          );
        yield* Ref.set(threadRef, started.threadId);
      }

      const transportToTurnError = (turnId: string) => (error: CodexTransportError) =>
        new HarnessTurnError({
          harnessId,
          sessionId,
          turnId,
          failureClass: error.failureClass,
          ...(error.detail === undefined ? {} : { detail: error.detail }),
        });

      const makeProjectionContext = (
        turnId: string,
        nextSequence: () => number,
      ): CodexProjectionContext => ({
        source,
        threadId: sessionId,
        turnId,
        nextSequence,
        toolIdentities: new Map<string, HarnessToolIdentity>(),
        deltaStreamedItems: new Set<string>(),
        pendingApprovals,
        usageBox: { value: undefined },
        threadBox: { value: undefined },
      });

      const submitToolApproval = (
        turnId: string,
      ): ((
        toolCallId: string,
        decision: HarnessToolApprovalDecision,
      ) => Effect.Effect<void, HarnessTurnError | HarnessCapabilityUnsupported>) =>
        config.mode === "app-server"
          ? (toolCallId, decision) =>
              Effect.gen(function* () {
                const requestId = pendingApprovals.get(toolCallId);
                if (requestId === undefined) {
                  return yield* Effect.fail(
                    new HarnessTurnError({
                      harnessId,
                      sessionId,
                      turnId,
                      failureClass: "no_active_tool_call",
                      detail: `no pending Codex approval for ${toolCallId}`,
                    }),
                  );
                }
                yield* config.transport
                  .respondToApproval({
                    requestId,
                    decision: APPROVAL_DECISION_TO_CODEX[decision],
                  })
                  .pipe(Effect.mapError(transportToTurnError(turnId)));
                pendingApprovals.delete(toolCallId);
              })
          : () =>
              Effect.fail(
                new HarnessCapabilityUnsupported({
                  harnessId,
                  capability: "builtin_tool_approvals",
                  detail: "codex exec runs non-interactively; approvals are fixed at spawn",
                }),
              );

      const submitUserMessage = (
        turnId: string,
      ): ((text: string) => Effect.Effect<void, HarnessTurnError | HarnessCapabilityUnsupported>) =>
        config.mode === "app-server" && config.transport.steerTurn !== undefined
          ? (text) =>
              Effect.gen(function* () {
                const threadId = yield* Ref.get(threadRef);
                const steerTurn = config.transport.steerTurn;
                if (threadId === undefined || steerTurn === undefined) {
                  return yield* Effect.fail(
                    new HarnessTurnError({
                      harnessId,
                      sessionId,
                      turnId,
                      failureClass: "user_message_injection_unsupported",
                    }),
                  );
                }
                yield* steerTurn({ threadId, text }).pipe(
                  Effect.mapError(transportToTurnError(turnId)),
                );
              })
          : () =>
              Effect.fail(
                new HarnessTurnError({
                  harnessId,
                  sessionId,
                  turnId,
                  failureClass: "user_message_injection_unsupported",
                  detail:
                    config.mode === "exec"
                      ? "codex exec runs one bounded process per turn; mid-turn injection is impossible"
                      : "the configured app-server transport does not expose turn/steer",
                }),
              );

      const interrupt = (turnId: string): Effect.Effect<void> =>
        config.mode === "app-server"
          ? Effect.gen(function* () {
              const threadId = yield* Ref.get(threadRef);
              if (threadId === undefined) return;
              // ACK admits interruption only; the turn still settles through
              // its own turn/completed (mirrors the desktop turn loop).
              yield* config.transport.interruptTurn({ threadId, turnId }).pipe(Effect.ignore);
            })
          : Effect.void;

      const controlFor = (
        turnId: string,
        events: ReadonlyArray<HarnessStreamEvent>,
      ): HarnessPromptControl =>
        makeControl({
          harnessId,
          sessionId,
          turnId,
          events,
          cursorRef,
          activeRef,
          submitToolApproval: submitToolApproval(turnId),
          submitUserMessage: submitUserMessage(turnId),
          interrupt: () => interrupt(turnId),
        });

      /**
       * openagents#9167 live app-server control: consume the transport's
       * {@link CodexAppServerTransport.runTurnStreaming} lazily, projecting each
       * {@link CodexEvent} onto the neutral stream the instant it arrives (like
       * the claude adapter's live drive), while keeping the session cursors and
       * the Codex-native thread id in sync as events flow. The fully-consumed
       * ordered set is identical to the batch path; a mid-turn suspend is
       * honestly lossy (no already-computed tail to replay).
       */
      const liveAppServerControl = (params: {
        readonly turnId: string;
        readonly threadId: string;
        readonly prompt: string;
        readonly ctx: CodexProjectionContext;
        readonly advanceCounter: () => number;
        readonly runTurnStreaming: NonNullable<CodexAppServerTransport["runTurnStreaming"]>;
      }): HarnessPromptControl => {
        const { turnId, threadId, prompt, ctx, advanceCounter, runTurnStreaming } = params;
        const finishBox: { value: KhalaRuntimeFinishReason | undefined } = { value: undefined };

        const events: Stream.Stream<HarnessStreamEvent, HarnessTurnError> = runTurnStreaming({
          threadId,
          prompt,
        }).pipe(
          Stream.mapError(transportToTurnError(turnId)),
          // A native thread id can arrive on the live wire; keep resume state
          // current even though `thread.started` projects to no neutral event.
          Stream.tap((codexEvent) =>
            codexEvent.type === "thread.started"
              ? Ref.set(threadRef, codexEvent.threadId)
              : Effect.void,
          ),
          Stream.flatMap((codexEvent) =>
            Stream.fromIterable(codexEventToKhalaEvents(codexEvent, ctx)),
          ),
          Stream.tap((event) =>
            Effect.gen(function* () {
              yield* Ref.set(cursorRef, event.sequence);
              yield* Ref.set(sequenceRef, advanceCounter());
              if (ctx.threadBox.value !== undefined) {
                yield* Ref.set(threadRef, ctx.threadBox.value);
              }
              // Live turns carry no already-computed tail: the honest suspend
              // remainder is empty (see `suspendTurn`, `lossy: true`).
              yield* Ref.set(
                activeRef,
                Option.some({
                  turnId,
                  remaining: [] as ReadonlyArray<HarnessStreamEvent>,
                }),
              );
              if (event.kind === "turn.finished") {
                finishBox.value = event.finishReason;
              }
            }),
          ),
        );

        const done: Effect.Effect<HarnessTurnResult, HarnessTurnError> = Effect.gen(function* () {
          const cursor = yield* Ref.get(cursorRef);
          return {
            turnId,
            finishReason: finishBox.value ?? "interrupted",
            lastCursor: cursor,
          } satisfies HarnessTurnResult;
        });

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
                detail: "codex runs its built-in tools natively; no host-tool result is awaited",
              }),
            ),
          submitToolApproval: submitToolApproval(turnId),
          submitUserMessage: submitUserMessage(turnId),
          interrupt: () => interrupt(turnId),
        };
      };

      const promptTurn = (opts: HarnessPromptTurnOptions) =>
        Effect.gen(function* () {
          const turnId = opts.turnId;
          const isFirstTurn = yield* Ref.getAndSet(firstTurnRef, false);
          // Session instructions apply once, on the first turn of a FRESH session.
          const prompt =
            opts.instructions !== undefined && isFirstTurn && !isResume
              ? `${opts.instructions}\n\n${opts.prompt}`
              : opts.prompt;

          const startSequence = yield* Ref.get(sequenceRef);
          let counter = startSequence;
          const ctx = makeProjectionContext(turnId, () => counter++);

          let raw: ReadonlyArray<CodexEvent>;
          if (config.mode === "app-server") {
            const threadId = yield* Ref.get(threadRef);
            if (threadId === undefined) {
              return yield* Effect.fail(
                new HarnessTurnError({
                  harnessId,
                  sessionId,
                  turnId,
                  failureClass: "thread_not_started",
                }),
              );
            }
            // openagents#9167 live path: project each CodexEvent onto the
            // neutral stream AS IT ARRIVES so text/tool rows reach the renderer
            // live, mirroring the claude adapter's `Stream.fromAsyncIterable`
            // drive. The batch `runTurn` path below is unchanged for
            // transports that do not expose the streaming seam.
            if (config.transport.runTurnStreaming !== undefined) {
              return liveAppServerControl({
                turnId,
                threadId,
                prompt,
                ctx,
                advanceCounter: () => counter,
                runTurnStreaming: config.transport.runTurnStreaming,
              });
            }
            raw = yield* config.transport
              .runTurn({ threadId, prompt })
              .pipe(Effect.mapError(transportToTurnError(turnId)));
          } else {
            const resumeThreadId = yield* Ref.get(threadRef);
            raw = yield* config.spawner
              .spawn({
                codexBinaryPath: config.codexBinaryPath,
                ...(effectiveCodexHome === undefined ? {} : { codexHome: effectiveCodexHome }),
                ...(config.workingDirectory === undefined
                  ? {}
                  : { workingDirectory: config.workingDirectory }),
                ...(config.model === undefined ? {} : { model: config.model }),
                prompt,
                ...(resumeThreadId === undefined ? {} : { resumeThreadId }),
              })
              .pipe(Effect.mapError(transportToTurnError(turnId)));
          }

          const events = raw.flatMap((event) => codexEventToKhalaEvents(event, ctx));
          if (ctx.threadBox.value !== undefined) {
            yield* Ref.set(threadRef, ctx.threadBox.value);
          }
          yield* Ref.set(sequenceRef, counter);
          yield* Ref.set(activeRef, Option.some({ turnId, remaining: events }));
          return controlFor(turnId, events);
        });

      const continueTurn = (_options: HarnessContinueTurnOptions) =>
        Effect.gen(function* () {
          const active = yield* Ref.get(activeRef);
          const turn = Option.getOrUndefined(active) ?? pendingContinuation;
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
          if (config.mode === "app-server") {
            // Lossless: replay exactly the buffered remainder from cursor + 1.
            return controlFor(turn.turnId, turn.remaining);
          }
          // Degraded rerun: resume the Codex thread and recompute the tail.
          const resumeThreadId = yield* Ref.get(threadRef);
          if (resumeThreadId === undefined) {
            return yield* Effect.fail(
              new HarnessTurnError({
                harnessId,
                sessionId,
                turnId: turn.turnId,
                failureClass: "no_thread_to_continue",
                detail: "exec continuation requires a Codex thread id from a prior turn",
              }),
            );
          }
          const cursor = yield* Ref.get(cursorRef);
          let counter = cursor + 1;
          const ctx = makeProjectionContext(turn.turnId, () => counter++);
          const raw = yield* config.spawner
            .spawn({
              codexBinaryPath: config.codexBinaryPath,
              ...(effectiveCodexHome === undefined ? {} : { codexHome: effectiveCodexHome }),
              ...(config.workingDirectory === undefined
                ? {}
                : { workingDirectory: config.workingDirectory }),
              ...(config.model === undefined ? {} : { model: config.model }),
              prompt: CODEX_EXEC_CONTINUE_PROMPT,
              resumeThreadId,
            })
            .pipe(Effect.mapError(transportToTurnError(turn.turnId)));
          const events = raw.flatMap((event) => codexEventToKhalaEvents(event, ctx));
          if (ctx.threadBox.value !== undefined) {
            yield* Ref.set(threadRef, ctx.threadBox.value);
          }
          yield* Ref.set(sequenceRef, counter);
          yield* Ref.set(activeRef, Option.some({ turnId: turn.turnId, remaining: events }));
          return controlFor(turn.turnId, events);
        });

      const suspendTurn = (): Effect.Effect<
        HarnessContinuationState,
        HarnessCapabilityUnsupported
      > =>
        Effect.gen(function* () {
          const cursor = yield* Ref.get(cursorRef);
          const active = yield* Ref.get(activeRef);
          const turn = Option.getOrUndefined(active);
          const turnId = turn?.turnId ?? pendingContinuation?.turnId ?? "unknown";
          const threadId = yield* Ref.get(threadRef);
          if (config.mode === "app-server") {
            // Batch app-server buffers the whole settled turn, so its remainder
            // is a lossless replay. The live streaming path (openagents#9167)
            // has no already-computed tail — its remainder is empty and the
            // continuation is honestly lossy, matching the exec mode's stance.
            const remaining = appServerStreaming ? [] : (turn?.remaining ?? []);
            return {
              harnessId,
              sessionId,
              turnId,
              cursor,
              lossy: appServerStreaming,
              data: {
                turnId,
                ...(threadId === undefined ? {} : { threadId }),
                remaining,
              },
            };
          }
          // exec mode keeps no live turn: the continuation is honestly lossy.
          return {
            harnessId,
            sessionId,
            turnId,
            cursor,
            lossy: true,
            data: { turnId, ...(threadId === undefined ? {} : { threadId }) },
          };
        });

      const compact = () =>
        Effect.fail(
          new HarnessCapabilityUnsupported({
            harnessId,
            capability: "compact",
            detail: "no client-driven Codex compaction verb is modelled",
          }),
        );

      const resumeState = (): Effect.Effect<HarnessResumeState> =>
        Effect.gen(function* () {
          const threadId = yield* Ref.get(threadRef);
          return {
            harnessId,
            sessionId,
            data: (threadId === undefined ? {} : { threadId }) satisfies CodexResumeData,
          };
        });

      const detach = (): Effect.Effect<HarnessResumeState, HarnessCapabilityUnsupported> =>
        resumeState();

      const stop = (): Effect.Effect<HarnessResumeState> =>
        Effect.gen(function* () {
          if (config.mode === "app-server") {
            yield* config.transport.shutdown();
          }
          return yield* resumeState();
        });

      const destroy = (): Effect.Effect<void> =>
        config.mode === "app-server" ? config.transport.shutdown() : Effect.void;

      const session: HarnessSession = {
        sessionId,
        isResume,
        modelId: config.model ?? "codex",
        promptTurn,
        continueTurn,
        suspendTurn,
        compact,
        detach,
        stop,
        destroy,
      };
      return session;
    });

  return {
    specificationVersion: "agent-harness-v1",
    harnessId,
    harnessKind: "codex",
    adapterKind: "codex",
    // Codex native built-ins with their shared common names.
    builtinTools: [
      { nativeName: "shell", commonName: "bash", description: "run a shell command" },
      { nativeName: "apply_patch", commonName: "edit", description: "apply a file patch" },
      { nativeName: "web_search", commonName: "webSearch", description: "search the web" },
    ],
    supportsBuiltinToolApprovals: config.mode === "app-server",
    supportsBuiltinToolFiltering: false,
    lifecycleStateSchema: CodexResumeData,
    start,
  };
};
