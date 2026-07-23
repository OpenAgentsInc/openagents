import { Effect, Option, Ref, Schema as S, Stream } from "effect";
import {
  decodeKhalaRuntimeEvent,
  type KhalaRuntimeFinishReason,
  type KhalaRuntimeSource,
  type KhalaRuntimeUsage,
  type RuntimeInteractionPayload,
} from "@openagentsinc/agent-runtime-schema";
import type { AgentHarness, HarnessStartOptions } from "./adapter.ts";
import { HarnessStartError } from "./adapter.ts";
import { HarnessCapabilityUnsupported } from "./capability.ts";
import { type HarnessToolIdentity, toolIdentity } from "./common-tool.ts";
import type { HarnessToolApprovalDecision } from "./host-tool.ts";
import { KhalaRuntimeEventSchemaLiteral } from "./stream.ts";
import { buildTextDelta, buildTurnFinished, buildTurnStarted } from "./event-builder.ts";
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
 * opencode harness adapter (HW-03): a LIVE runtime adapter for the opencode
 * HTTP control plane, promoted from the earlier fixture-only projection.
 *
 * The opencode server (`~/work/projects/repos/opencode`) exposes a REST + SSE
 * control plane: `POST /session` creates a session, `POST
 * /session/{sessionID}/message` sends a prompt and drives the model, and
 * `GET /event` is the global Server-Sent-Events bus carrying every
 * `session.next.*` notification (`packages/schema/src/session-event.ts`).
 * Permission requests ride the same bus as `permission.v2.asked` and are
 * answered with `POST /api/session/{sessionID}/permission/{permissionID}/reply`
 * (`packages/schema/src/permission.ts`).
 *
 * All of that transport machinery — the fetch calls, the SSE decode, filtering
 * the global bus down to this session's events, and correlating a pending
 * permission — lives behind the injected {@link OpencodeTransport} seam. The
 * adapter itself is a pure PROJECTION from opencode's event shapes onto the
 * neutral {@link HarnessStreamEvent} ({@link KhalaRuntimeEvent}). Fixture tests
 * script the seam so CI needs no live opencode server, and a host wires a real
 * `fetch`/`EventSource` transport in production with zero adaptation.
 *
 * The local {@link OpencodeEvent} union is the public-safe, replayable subset
 * the projection reads. opencode's live-only stream fragments (`text.started`,
 * `tool.input.delta`, …), raw tool `input`/`content` payloads, and raw file
 * contents are intentionally NOT modelled — they never cross into a neutral
 * event. {@link decodeOpencodeSessionEvent} maps a raw SSE wire event onto this
 * subset, so the live transport and the fixture both feed the same projection.
 */

// ---------------------------------------------------------------------------
// Local opencode event vocabulary (public-safe projection subset)
// ---------------------------------------------------------------------------

export type OpencodeEvent =
  | OpencodeTextDelta
  | OpencodeReasoningDelta
  | OpencodeToolCalled
  | OpencodeToolSuccess
  | OpencodeToolFailed
  | OpencodeStepEnded
  | OpencodeStepFailed
  | OpencodePermissionAsked
  | OpencodeSessionIdle;

/** opencode `session.next.text.delta` — a fragment of assistant output text. */
export interface OpencodeTextDelta {
  readonly type: "session.next.text.delta";
  readonly assistantMessageID: string;
  readonly textID: string;
  readonly delta: string;
}

/** opencode `session.next.reasoning.delta` — a fragment of model reasoning. */
export interface OpencodeReasoningDelta {
  readonly type: "session.next.reasoning.delta";
  readonly assistantMessageID: string;
  readonly reasoningID: string;
  readonly delta: string;
}

/**
 * opencode `session.next.tool.called` — the runtime invoked a tool. `tool` is
 * the opencode tool id (`bash`, `read`, `edit`, `webfetch`, …). opencode's wire
 * carries `provider: { executed }`; the live transport flattens it onto
 * {@link providerExecuted} here. The raw `input` record is intentionally
 * dropped at the seam and never crosses into a neutral event.
 */
export interface OpencodeToolCalled {
  readonly type: "session.next.tool.called";
  readonly assistantMessageID: string;
  readonly callID: string;
  readonly tool: string;
  readonly providerExecuted: boolean;
}

/**
 * opencode `session.next.tool.success`. opencode's success event carries only
 * `callID` (not the tool id), so the projection correlates the id from the
 * earlier `tool.called` through the projection context. `providerExecuted`
 * mirrors opencode `provider.executed`.
 */
export interface OpencodeToolSuccess {
  readonly type: "session.next.tool.success";
  readonly callID: string;
  readonly providerExecuted: boolean;
}

/**
 * opencode `session.next.tool.failed` — a tool call that errored. opencode's
 * wire carries `error: { type: "unknown", message }`; the transport extracts a
 * bounded public-safe `messageSafe`, never the raw payload.
 */
export interface OpencodeToolFailed {
  readonly type: "session.next.tool.failed";
  readonly callID: string;
  readonly messageSafe: string;
  readonly providerExecuted: boolean;
}

/**
 * opencode `session.next.step.ended` — the model step settled with a finish
 * reason and token usage (opencode `tokens: { input, output, reasoning,
 * cache: { read, write } }`). This is the neutral turn-finish carrier: opencode
 * has no explicit "turn started/finished" event, so the adapter synthesizes
 * `turn.started` and derives `turn.finished` (+usage) from this step boundary.
 */
export interface OpencodeStepEnded {
  readonly type: "session.next.step.ended";
  readonly assistantMessageID: string;
  readonly finish: string;
  readonly tokens: {
    readonly input: number;
    readonly output: number;
    readonly reasoning: number;
    readonly cache: { readonly read: number; readonly write: number };
  };
}

/**
 * opencode `session.next.step.failed` — the model step errored before it
 * settled. There is no usage on a failed step, so this projects to a
 * `turn.finished` with an `error` finish reason and no fabricated usage.
 */
export interface OpencodeStepFailed {
  readonly type: "session.next.step.failed";
  readonly assistantMessageID: string;
  readonly messageSafe: string;
}

/**
 * opencode `permission.v2.asked` — a pending permission request the runtime
 * needs answered before it runs a tool (`packages/schema/src/permission.ts`).
 * `permissionId` is the `per_…` id the reply endpoint answers; `callID`
 * correlates with the `tool.called` of the same call. Projects to NO neutral
 * stream event — approvals route through the durable `RuntimeInteraction` model
 * ({@link opencodePermissionToRuntimeInteractionPayload}) and the eventual
 * decision returns through `submitToolApproval` on the prompt control.
 */
export interface OpencodePermissionAsked {
  readonly type: "permission.v2.asked";
  readonly permissionId: string;
  readonly callID: string;
  /** Bounded public-safe action string (opencode `action`), never raw args. */
  readonly action: string;
}

/**
 * opencode session-idle boundary (transport-level end-of-activity). The neutral
 * turn boundary is already carried by `step.ended`, so this projects to no
 * neutral event.
 */
export interface OpencodeSessionIdle {
  readonly type: "session.idle";
}

// ---------------------------------------------------------------------------
// Tool identity + finish-reason normalization
// ---------------------------------------------------------------------------

/**
 * opencode tool ids are lowercase (`bash`, `read`, `write`, `edit`, `glob`,
 * `grep`, `websearch`, `webfetch`, `apply_patch`, …), but the shared
 * {@link commonToolName} map in `common-tool.ts` keys Claude PascalCase and
 * Codex snake_case. This adapter-local alias resolves the opencode id onto a
 * name the shared map recognizes, so `toolIdentity` still owns the
 * native->common vocabulary and a future shared-map addition wins automatically.
 * opencode ids with no shared equivalent (`webfetch`, `task`, `todo`) are left
 * unaliased and forwarded as-is with no common name.
 */
const OPENCODE_TO_SHARED_NATIVE: Readonly<Record<string, string>> = {
  bash: "Bash",
  read: "Read",
  write: "Write",
  edit: "Edit",
  glob: "Glob",
  grep: "Grep",
  websearch: "WebSearch",
  apply_patch: "apply_patch",
};

/**
 * Normalized tool identity for an opencode tool id: the shared common name when
 * one exists, with the true opencode native id always preserved as `nativeName`.
 */
const opencodeToolIdentity = (
  opencodeToolId: string,
  options?: { readonly providerExecuted?: boolean },
): HarnessToolIdentity => {
  const sharedNative = OPENCODE_TO_SHARED_NATIVE[opencodeToolId] ?? opencodeToolId;
  const identity = toolIdentity(sharedNative, options);
  // Preserve opencode's real lowercase id rather than the shared-map alias.
  return { ...identity, nativeName: opencodeToolId };
};

/** Bounded sanitizer onto the Khala safe-ref alphabet (`/` and spaces become `-`). */
const toSafeRef = (value: string, fallback: string): string => {
  const cleaned = value
    .replace(/[^A-Za-z0-9._:-]+/g, "-")
    .replace(/^[^A-Za-z0-9]+/, "")
    .slice(0, 200);
  return cleaned.length === 0 ? fallback : cleaned;
};

/**
 * The neutral `toolCallId` for an opencode call id. Derived from the opencode
 * `callID` (NOT a per-event sequence) so the `tool.call`, `tool.result` /
 * `tool.error`, and the `permission.v2.asked` of the same call all correlate to
 * one stable id — opencode's success/failed events carry only the `callID`.
 */
export const opencodeToolCallId = (callID: string): string =>
  `toolcall.opencode.${toSafeRef(callID, "unknown")}`;

/** opencode finish string -> neutral {@link KhalaRuntimeFinishReason}. */
const OPENCODE_FINISH_TO_KHALA: Readonly<Record<string, KhalaRuntimeFinishReason>> = {
  stop: "stop",
  length: "length",
  "tool-calls": "tool-calls",
  "content-filter": "content-filter",
  error: "error",
  cancelled: "cancelled",
  interrupted: "interrupted",
};

const mapFinishReason = (finish: string): KhalaRuntimeFinishReason =>
  OPENCODE_FINISH_TO_KHALA[finish] ?? "unknown";

// ---------------------------------------------------------------------------
// Projection
// ---------------------------------------------------------------------------

/** Context threaded through {@link opencodeEventToKhalaEvents} while folding a stream. */
export interface OpencodeProjectionContext {
  readonly source: KhalaRuntimeSource;
  readonly threadId: string;
  readonly turnId: string;
  /** Allocate the next session-global sequence number. */
  readonly nextSequence: () => number;
  /** callID -> opencode tool id, populated by `tool.called`, read by success/failed. */
  readonly toolNames: Map<string, string>;
  /**
   * neutral toolCallId -> opencode permission id awaiting a decision, populated
   * by `permission.v2.asked`. Shared across a session's turns so an approval can
   * be answered on the prompt control. Optional so the pure projection stays
   * usable standalone in unit tests.
   */
  readonly pendingApprovals?: Map<string, string>;
}

const base = (ctx: OpencodeProjectionContext, sequence: number, eventSuffix: string) => ({
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

/** Default allow-authority for a projected opencode tool event. */
const toolAuthority = (
  identity: HarnessToolIdentity,
  ctx: OpencodeProjectionContext,
  sequence: number,
) => ({
  authorityRef: `authority.opencode.${identity.wireName}`,
  policyRef: `policy.opencode.${identity.wireName}`,
  decisionRef: `decision.opencode.${ctx.turnId}.${sequence}`,
  toolRef: `tool.opencode.${identity.wireName}`,
  status: "allowed" as const,
  allowed: true,
  blockerRefs: [] as ReadonlyArray<string>,
});

const buildReasoningDelta = (
  ctx: OpencodeProjectionContext,
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
  ctx: OpencodeProjectionContext,
  sequence: number,
  identity: HarnessToolIdentity,
  toolCallId: string,
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(ctx, sequence, "toolcall"),
    kind: "tool.call",
    toolCallId,
    toolName: identity.wireName,
    inputRef: `input.opencode.${ctx.turnId}.${sequence}`,
    authority: toolAuthority(identity, ctx, sequence),
  });

const buildToolResult = (
  ctx: OpencodeProjectionContext,
  sequence: number,
  identity: HarnessToolIdentity,
  toolCallId: string,
  providerExecuted: boolean,
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(ctx, sequence, "toolresult"),
    kind: "tool.result",
    toolCallId,
    toolName: identity.wireName,
    resultRef: `result.opencode.${ctx.turnId}.${sequence}`,
    authority: toolAuthority(identity, ctx, sequence),
    providerExecuted,
  });

const buildToolError = (
  ctx: OpencodeProjectionContext,
  sequence: number,
  identity: HarnessToolIdentity,
  toolCallId: string,
  messageSafe: string,
  providerExecuted: boolean,
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(ctx, sequence, "toolerror"),
    kind: "tool.error",
    toolCallId,
    toolName: identity.wireName,
    errorRef: `error.opencode.${ctx.turnId}.${sequence}`,
    messageSafe,
    authority: toolAuthority(identity, ctx, sequence),
    providerExecuted,
  });

const stepUsage = (
  ctx: OpencodeProjectionContext,
  sequence: number,
  ev: OpencodeStepEnded,
): KhalaRuntimeUsage => ({
  usageRef: `usage.opencode.${ctx.turnId}.${sequence}`,
  inputTokens: ev.tokens.input,
  outputTokens: ev.tokens.output,
  reasoningTokens: ev.tokens.reasoning,
  cacheReadInputTokens: ev.tokens.cache.read,
  cacheWriteInputTokens: ev.tokens.cache.write,
  totalTokens: ev.tokens.input + ev.tokens.output + ev.tokens.reasoning,
});

/**
 * Pure projection of ONE opencode event onto zero or more neutral
 * {@link HarnessStreamEvent}s. Sequence numbers come from `ctx.nextSequence`, so
 * a caller folds a whole opencode stream while keeping session-global cursors
 * contiguous. Tool ids are normalized through {@link opencodeToolIdentity} so
 * `bash`/`read`/… land on the shared common vocabulary, and the neutral
 * `toolCallId` is derived from the opencode `callID` so a call and its later
 * success/failed (which carry only the `callID`) correlate to one id.
 * `permission.v2.asked` consumes no sequence — it records the pending approval
 * in the context instead of the transcript.
 */
export const opencodeEventToKhalaEvents = (
  event: OpencodeEvent,
  ctx: OpencodeProjectionContext,
): ReadonlyArray<HarnessStreamEvent> => {
  switch (event.type) {
    case "session.next.text.delta":
      return [
        buildTextDelta({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence: ctx.nextSequence(),
          source: ctx.source,
          messageId: event.assistantMessageID,
          text: event.delta,
        }),
      ];
    case "session.next.reasoning.delta":
      return [
        buildReasoningDelta(ctx, ctx.nextSequence(), {
          messageId: event.assistantMessageID,
          text: event.delta,
        }),
      ];
    case "session.next.tool.called": {
      ctx.toolNames.set(event.callID, event.tool);
      const identity = opencodeToolIdentity(event.tool, {
        providerExecuted: event.providerExecuted,
      });
      return [buildToolCall(ctx, ctx.nextSequence(), identity, opencodeToolCallId(event.callID))];
    }
    case "session.next.tool.success": {
      const opencodeToolId = ctx.toolNames.get(event.callID) ?? event.callID;
      const identity = opencodeToolIdentity(opencodeToolId, {
        providerExecuted: event.providerExecuted,
      });
      return [
        buildToolResult(
          ctx,
          ctx.nextSequence(),
          identity,
          opencodeToolCallId(event.callID),
          event.providerExecuted,
        ),
      ];
    }
    case "session.next.tool.failed": {
      const opencodeToolId = ctx.toolNames.get(event.callID) ?? event.callID;
      const identity = opencodeToolIdentity(opencodeToolId, {
        providerExecuted: event.providerExecuted,
      });
      return [
        buildToolError(
          ctx,
          ctx.nextSequence(),
          identity,
          opencodeToolCallId(event.callID),
          event.messageSafe,
          event.providerExecuted,
        ),
      ];
    }
    case "session.next.step.ended": {
      const sequence = ctx.nextSequence();
      return [
        buildTurnFinished({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence,
          source: ctx.source,
          finishReason: mapFinishReason(event.finish),
          usage: stepUsage(ctx, sequence, event),
        }),
      ];
    }
    case "session.next.step.failed":
      // No usage on a failed step: honest error finish, never fabricated tokens.
      return [
        buildTurnFinished({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence: ctx.nextSequence(),
          source: ctx.source,
          finishReason: "error",
        }),
      ];
    case "permission.v2.asked":
      // Not a transcript item: record the pending approval so the eventual
      // decision on the prompt control can answer opencode's reply endpoint.
      ctx.pendingApprovals?.set(opencodeToolCallId(event.callID), event.permissionId);
      return [];
    case "session.idle":
      // The neutral turn boundary is carried by `step.ended`; idle is transport-level.
      return [];
  }
};

// ---------------------------------------------------------------------------
// Wire decode: raw SSE event -> local OpencodeEvent
// ---------------------------------------------------------------------------

/**
 * A raw opencode SSE event as delivered on `GET /event`. The event bus wraps
 * each `session.next.*` notification's schema fields under `properties`
 * (durable projections use `data`); this permissive structural type reads
 * whichever is present so the live transport can decode without importing the
 * opencode schema package.
 */
export interface OpencodeWireEvent {
  readonly type: string;
  readonly properties?: Record<string, unknown>;
  readonly data?: Record<string, unknown>;
}

const boundedText = (value: unknown, limit: number, fallback: string): string => {
  if (typeof value !== "string" || value.length === 0) return fallback;
  return value.length > limit ? `${value.slice(0, limit - 1)}…` : value;
};

const readString = (record: Record<string, unknown>, key: string): string => {
  const value = record[key];
  return typeof value === "string" ? value : "";
};

const readProviderExecuted = (record: Record<string, unknown>): boolean => {
  const provider = record.provider;
  return (
    provider !== null &&
    typeof provider === "object" &&
    (provider as Record<string, unknown>).executed === true
  );
};

/**
 * Map ONE raw opencode SSE wire event onto the local {@link OpencodeEvent}
 * subset, or `undefined` for a live-only / unmodelled event (`text.started`,
 * `tool.input.delta`, `step.started`, `session.next.retried`, …) that has no
 * neutral projection. This is the single decode boundary the live transport
 * uses; the fixture path feeds {@link OpencodeEvent}s directly, so both paths
 * share {@link opencodeEventToKhalaEvents}. Raw tool `input`/`content`, raw
 * file contents, and unbounded error text never cross this boundary — only the
 * bounded public-safe subset does.
 */
export const decodeOpencodeSessionEvent = (wire: OpencodeWireEvent): OpencodeEvent | undefined => {
  const p = (wire.properties ?? wire.data ?? {}) as Record<string, unknown>;
  switch (wire.type) {
    case "session.next.text.delta":
      return {
        type: "session.next.text.delta",
        assistantMessageID: readString(p, "assistantMessageID"),
        textID: readString(p, "textID"),
        delta: readString(p, "delta"),
      };
    case "session.next.reasoning.delta":
      return {
        type: "session.next.reasoning.delta",
        assistantMessageID: readString(p, "assistantMessageID"),
        reasoningID: readString(p, "reasoningID"),
        delta: readString(p, "delta"),
      };
    case "session.next.tool.called":
      return {
        type: "session.next.tool.called",
        assistantMessageID: readString(p, "assistantMessageID"),
        callID: readString(p, "callID"),
        tool: readString(p, "tool"),
        providerExecuted: readProviderExecuted(p),
      };
    case "session.next.tool.success":
      return {
        type: "session.next.tool.success",
        callID: readString(p, "callID"),
        providerExecuted: readProviderExecuted(p),
      };
    case "session.next.tool.failed": {
      const error = (p.error ?? {}) as Record<string, unknown>;
      return {
        type: "session.next.tool.failed",
        callID: readString(p, "callID"),
        messageSafe: boundedText(error.message, 500, "opencode reported a tool failure"),
        providerExecuted: readProviderExecuted(p),
      };
    }
    case "session.next.step.ended": {
      const tokens = (p.tokens ?? {}) as Record<string, unknown>;
      const cache = (tokens.cache ?? {}) as Record<string, unknown>;
      const num = (value: unknown): number => (typeof value === "number" ? value : 0);
      return {
        type: "session.next.step.ended",
        assistantMessageID: readString(p, "assistantMessageID"),
        finish: readString(p, "finish"),
        tokens: {
          input: num(tokens.input),
          output: num(tokens.output),
          reasoning: num(tokens.reasoning),
          cache: { read: num(cache.read), write: num(cache.write) },
        },
      };
    }
    case "session.next.step.failed": {
      const error = (p.error ?? {}) as Record<string, unknown>;
      return {
        type: "session.next.step.failed",
        assistantMessageID: readString(p, "assistantMessageID"),
        messageSafe: boundedText(error.message, 500, "opencode reported a step failure"),
      };
    }
    case "permission.v2.asked": {
      const source = (p.source ?? {}) as Record<string, unknown>;
      return {
        type: "permission.v2.asked",
        permissionId: readString(p, "id"),
        callID: readString(source, "callID"),
        action: boundedText(p.action, 160, "run a tool"),
      };
    }
    case "session.next.idle":
    case "session.idle":
      return { type: "session.idle" };
    default:
      return undefined;
  }
};

// ---------------------------------------------------------------------------
// Approval → RuntimeInteraction
// ---------------------------------------------------------------------------

/** opencode permission reply vocabulary (`packages/schema/src/permission.ts`). */
export type OpencodePermissionReply = "once" | "always" | "reject";

/** Harness approval decision -> opencode `PermissionV2.Reply`. */
export const OPENCODE_APPROVAL_DECISION_TO_REPLY: Readonly<
  Record<HarnessToolApprovalDecision, OpencodePermissionReply>
> = {
  "allow-once": "once",
  "allow-session": "always",
  deny: "reject",
};

/**
 * Project an opencode permission request onto a canonical
 * `RuntimeInteractionPayload` of kind `tool_approval` — the durable,
 * provider-neutral approval model every harness approval routes through (like
 * the ACP and Codex adapters). The carried authority is
 * `operator_escalation_required` (owner decision pending), never a self-granted
 * allow. The eventual decision returns through `submitToolApproval`, which
 * answers opencode's `permission/reply` endpoint.
 */
export const opencodePermissionToRuntimeInteractionPayload = (
  event: OpencodePermissionAsked,
  toolNames?: ReadonlyMap<string, string>,
): RuntimeInteractionPayload => {
  const opencodeToolId = toolNames?.get(event.callID);
  const identity =
    opencodeToolId === undefined
      ? undefined
      : opencodeToolIdentity(opencodeToolId, { providerExecuted: true });
  const wireName = identity?.wireName ?? "a tool";
  const toolCallId = opencodeToolCallId(event.callID);
  return {
    kind: "tool_approval",
    displayText: `Allow opencode to ${event.action} (${wireName})?`,
    toolCallId,
    toolName: wireName,
    authority: {
      authorityRef: `authority.opencode.${toSafeRef(event.callID, "unknown")}`,
      policyRef: "policy.opencode_runtime",
      decisionRef: "decision.provider_reported_not_authority",
      toolRef: `toolref.opencode.${toSafeRef(wireName, "unknown")}`,
      status: "operator_escalation_required",
      allowed: false,
      blockerRefs: ["blocker.owner_approval"],
    },
  };
};

// ---------------------------------------------------------------------------
// Transport seam (injected; fixtures script it in tests)
// ---------------------------------------------------------------------------

/** Typed transport failure. `failureClass` carries the operator-facing class. */
export class OpencodeTransportError extends S.TaggedErrorClass<OpencodeTransportError>()(
  "AgentHarness.OpencodeTransportError",
  {
    failureClass: S.String,
    detail: S.optionalKey(S.String),
    cause: S.optionalKey(S.Defect()),
  },
) {}

/**
 * The opencode HTTP + SSE control-plane seam. A live implementation owns the
 * REST calls and the SSE decode; fixtures script it. `prompt` resolves with the
 * ordered {@link OpencodeEvent}s of the settled turn (decoded from the raw SSE
 * stream through {@link decodeOpencodeSessionEvent}), buffered so the adapter's
 * suspend/continue stays cursor-exact.
 *
 * LIVE SMOKE RECIPE (never runs in CI; see the skipped test in
 * `opencode-adapter.test.ts`):
 *   1. Start a local server: `opencode serve --port 4096` (prints its URL).
 *   2. `createSession` -> `POST {baseUrl}/session` with `{ directory }`; read
 *      the returned `id` (the opencode-native session id).
 *   3. Subscribe to `GET {baseUrl}/event` (SSE). Filter each event to
 *      `properties.sessionID === sessionId`, decode it through
 *      `decodeOpencodeSessionEvent`, and buffer the non-undefined results.
 *   4. `prompt` -> `POST {baseUrl}/session/{sessionId}/message` with
 *      `{ parts: [{ type: "text", text: prompt }], system, model }`. Resolve
 *      the buffered events once `session.next.step.ended` (or `step.failed`)
 *      for that assistant message arrives.
 *   5. `replyToPermission` -> `POST
 *      {baseUrl}/api/session/{sessionId}/permission/{requestId}/reply` with
 *      `{ reply }` (`once` | `always` | `reject`).
 *   6. `shutdown` -> close the SSE stream.
 */
export interface OpencodeTransport {
  /** `POST /session` — create the opencode-native session. */
  readonly createSession: (params: {
    readonly directory?: string;
    readonly model?: string;
    readonly title?: string;
  }) => Effect.Effect<{ readonly sessionId: string }, OpencodeTransportError>;
  /** `POST /session/{sessionID}/message` + SSE — the ordered events of the settled turn. */
  readonly prompt: (params: {
    readonly sessionId: string;
    readonly prompt: string;
    readonly system?: string;
    readonly model?: string;
  }) => Effect.Effect<ReadonlyArray<OpencodeEvent>, OpencodeTransportError>;
  /** Answer a pending `permission.v2.asked` via `.../permission/{id}/reply`. */
  readonly replyToPermission: (params: {
    readonly sessionId: string;
    readonly requestId: string;
    readonly reply: OpencodePermissionReply;
  }) => Effect.Effect<void, OpencodeTransportError>;
  /** Close the SSE stream / release the transport. Idempotent. */
  readonly shutdown: () => Effect.Effect<void>;
}

// ---------------------------------------------------------------------------
// Adapter factory
// ---------------------------------------------------------------------------

/** Configuration for {@link makeOpencodeAdapter}. */
export interface OpencodeAdapterConfig {
  readonly harnessId?: string;
  /**
   * The LIVE HTTP + SSE transport driving a real opencode server. When present,
   * every turn runs through it. Absent, the adapter synthesizes an internal
   * scripted transport from {@link script} so unit tests need no live server.
   */
  readonly transport?: OpencodeTransport;
  /**
   * The scripted opencode event stream each prompt turn replays (fixture, no
   * live server). Ignored when {@link transport} is set.
   */
  readonly script?: ReadonlyArray<OpencodeEvent>;
  /** Working directory passed to `createSession` (live transport). */
  readonly directory?: string;
  /** Model id passed to `createSession` / `prompt` (live transport). */
  readonly model?: string;
  readonly supportsSuspend?: boolean;
  readonly supportsContinue?: boolean;
  readonly supportsCompact?: boolean;
  readonly supportsDetach?: boolean;
  /** When true, `continueTurn` reports the continuation as a re-driven (lossy) tail. */
  readonly continueIsLossy?: boolean;
}

/** Default fixture stream when neither a transport nor a script is supplied. */
const DEFAULT_SCRIPT: ReadonlyArray<OpencodeEvent> = [
  {
    type: "session.next.text.delta",
    assistantMessageID: "msg_opencode_1",
    textID: "text_1",
    delta: "Hello ",
  },
  {
    type: "session.next.text.delta",
    assistantMessageID: "msg_opencode_1",
    textID: "text_1",
    delta: "world",
  },
  {
    type: "session.next.step.ended",
    assistantMessageID: "msg_opencode_1",
    finish: "stop",
    tokens: { input: 12, output: 5, reasoning: 0, cache: { read: 0, write: 0 } },
  },
];

/**
 * An internal transport that replays a fixed {@link OpencodeEvent} script. It
 * keeps the legacy fixture path (`makeOpencodeAdapter({ script })`) working
 * unchanged: the adapter always drives through a transport, and a scripted
 * config synthesizes this one. `createSession` returns a synthetic id and
 * `replyToPermission` is a no-op — the script has no live permission to answer.
 */
const makeScriptedTransport = (script: ReadonlyArray<OpencodeEvent>): OpencodeTransport => ({
  createSession: () => Effect.succeed({ sessionId: "opencode-scripted" }),
  prompt: () => Effect.succeed(script),
  replyToPermission: () => Effect.void,
  shutdown: () => Effect.void,
});

/** Resume payload (`detach`/`stop` -> `start({ resumeFrom })`): the opencode session id. */
export const OpencodeResumeData = S.Struct({
  opencodeSessionId: S.optionalKey(S.NonEmptyString),
});
export interface OpencodeResumeData extends S.Schema.Type<typeof OpencodeResumeData> {}

interface ContinuationData {
  readonly turnId: string;
  readonly remaining: ReadonlyArray<HarnessStreamEvent>;
}

const ContinuationDataSchema = S.Struct({
  turnId: S.NonEmptyString,
  remaining: S.Array(S.Unknown),
});

interface ActiveTurn {
  readonly turnId: string;
  readonly remaining: ReadonlyArray<HarnessStreamEvent>;
}

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

  const notActive = () =>
    new HarnessTurnError({
      harnessId: params.harnessId,
      sessionId: params.sessionId,
      turnId,
      failureClass: "no_active_tool_call",
      detail: "opencode adapter has no tool call awaiting a result",
    });

  return {
    turnId,
    events: stream,
    done,
    // opencode executes its own built-ins; there is no host-tool result channel.
    submitToolResult: () => Effect.fail(notActive()),
    submitToolApproval: params.submitToolApproval,
    submitUserMessage: () => Effect.void,
    interrupt: () => Effect.void,
  };
};

/**
 * Build an opencode {@link AgentHarness}. Each turn runs through the injected
 * {@link OpencodeTransport} (live) or an internal scripted transport (fixture),
 * projecting opencode's `session.next.*` stream through
 * {@link opencodeEventToKhalaEvents} as
 * `turn.started -> <projected events> -> turn.finished` with session-global
 * contiguous sequence numbers. Suspend/continue is lossless and cursor-exact,
 * mirroring the reference adapter. Permission requests route through the durable
 * `RuntimeInteraction` model and are answered through `submitToolApproval` ->
 * the transport's `replyToPermission`. Capabilities are declared honestly and a
 * refused verb fails with {@link HarnessCapabilityUnsupported}.
 */
export const makeOpencodeAdapter = (config: OpencodeAdapterConfig = {}): AgentHarness => {
  const harnessId = config.harnessId ?? "opencode";
  const isLive = config.transport !== undefined;
  const transport: OpencodeTransport =
    config.transport ?? makeScriptedTransport(config.script ?? DEFAULT_SCRIPT);
  const supportsSuspend = config.supportsSuspend ?? true;
  const supportsContinue = config.supportsContinue ?? true;
  const supportsCompact = config.supportsCompact ?? true;
  const supportsDetach = config.supportsDetach ?? true;
  const continueIsLossy = config.continueIsLossy ?? false;

  const start = (options: HarnessStartOptions): Effect.Effect<HarnessSession, HarnessStartError> =>
    Effect.gen(function* () {
      const source: KhalaRuntimeSource = options.source;
      const sessionId = options.sessionId;

      // Session-global monotonic sequence. A resumed session keeps counting from
      // where the export left off so cursors stay globally ordered.
      const seedSequence = options.continueFrom?.cursor ?? -1;
      const sequenceRef = yield* Ref.make(seedSequence + 1);
      const cursorRef = yield* Ref.make(seedSequence);
      const activeRef = yield* Ref.make<Option.Option<ActiveTurn>>(Option.none());
      // Pending approvals live for the whole session so an approval asked during
      // a turn can be answered on that turn's prompt control.
      const pendingApprovals = new Map<string, string>();
      // callID -> opencode tool id, shared so a permission payload can name the tool.
      const toolNames = new Map<string, string>();

      // opencode-native session id, seeded from durable resume state where present.
      let seedOpencodeSessionId: string | undefined;
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
        const data = S.decodeUnknownSync(OpencodeResumeData)(options.resumeFrom.data);
        seedOpencodeSessionId = data.opencodeSessionId;
      }

      // A continuation-started session pre-loads the remaining turn script.
      let pendingContinuation: ActiveTurn | undefined;
      if (options.continueFrom !== undefined) {
        const data = S.decodeUnknownSync(ContinuationDataSchema)(
          options.continueFrom.data,
        ) as unknown as ContinuationData;
        pendingContinuation = {
          turnId: options.continueFrom.turnId,
          remaining: data.remaining as ReadonlyArray<HarnessStreamEvent>,
        };
        yield* Ref.set(activeRef, Option.some(pendingContinuation));
      }

      // Create (or reuse) the opencode session. A continuation replays buffered
      // events and never re-prompts, so it needs no server session.
      let opencodeSessionId = seedOpencodeSessionId;
      if (opencodeSessionId === undefined && options.continueFrom === undefined) {
        const created = yield* transport
          .createSession({
            ...(config.directory === undefined ? {} : { directory: config.directory }),
            ...(config.model === undefined ? {} : { model: config.model }),
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
        opencodeSessionId = created.sessionId;
      }
      const opencodeSessionRef = yield* Ref.make<string | undefined>(opencodeSessionId);

      const transportToTurnError = (turnId: string) => (error: OpencodeTransportError) =>
        new HarnessTurnError({
          harnessId,
          sessionId,
          turnId,
          failureClass: error.failureClass,
          ...(error.detail === undefined ? {} : { detail: error.detail }),
        });

      const submitToolApproval = (turnId: string) =>
        isLive
          ? (toolCallId: string, decision: HarnessToolApprovalDecision) =>
              Effect.gen(function* () {
                const requestId = pendingApprovals.get(toolCallId);
                const opencodeSession = yield* Ref.get(opencodeSessionRef);
                if (requestId === undefined || opencodeSession === undefined) {
                  return yield* Effect.fail(
                    new HarnessTurnError({
                      harnessId,
                      sessionId,
                      turnId,
                      failureClass: "no_active_tool_call",
                      detail: `no pending opencode permission for ${toolCallId}`,
                    }),
                  );
                }
                yield* transport
                  .replyToPermission({
                    sessionId: opencodeSession,
                    requestId,
                    reply: OPENCODE_APPROVAL_DECISION_TO_REPLY[decision],
                  })
                  .pipe(Effect.mapError(transportToTurnError(turnId)));
                pendingApprovals.delete(toolCallId);
              })
          : () =>
              Effect.fail(
                new HarnessCapabilityUnsupported({
                  harnessId,
                  capability: "builtin_tool_approvals",
                  detail: "the scripted opencode transport surfaces no live permission requests",
                }),
              );

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
        });

      const promptTurn = (opts: HarnessPromptTurnOptions) =>
        Effect.gen(function* () {
          const turnId = opts.turnId;
          const opencodeSession = yield* Ref.get(opencodeSessionRef);
          if (opencodeSession === undefined) {
            return yield* Effect.fail(
              new HarnessTurnError({
                harnessId,
                sessionId,
                turnId,
                failureClass: "session_not_started",
                detail: "opencode session was not created before promptTurn",
              }),
            );
          }

          const events: Array<HarnessStreamEvent> = [];

          // opencode has no explicit turn-started event; synthesize it.
          const s0 = yield* Ref.getAndUpdate(sequenceRef, (n) => n + 1);
          events.push(buildTurnStarted({ turnId, threadId: sessionId, sequence: s0, source }));

          // Drive the turn through the transport, then fold its opencode events
          // through the pure projection with contiguous session-global sequences.
          const raw = yield* transport
            .prompt({
              sessionId: opencodeSession,
              prompt: opts.prompt,
              ...(opts.instructions === undefined ? {} : { system: opts.instructions }),
              ...(config.model === undefined ? {} : { model: config.model }),
            })
            .pipe(Effect.mapError(transportToTurnError(turnId)));

          const seqBox = { value: yield* Ref.get(sequenceRef) };
          const ctx: OpencodeProjectionContext = {
            source,
            threadId: sessionId,
            turnId,
            nextSequence: () => seqBox.value++,
            toolNames,
            pendingApprovals,
          };
          for (const opencodeEvent of raw) {
            events.push(...opencodeEventToKhalaEvents(opencodeEvent, ctx));
          }
          yield* Ref.set(sequenceRef, seqBox.value);

          yield* Ref.set(activeRef, Option.some({ turnId, remaining: events }));
          return controlFor(turnId, events);
        });

      const continueTurn = (_options: HarnessContinueTurnOptions) =>
        supportsContinue
          ? Effect.gen(function* () {
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
              return controlFor(turn.turnId, turn.remaining);
            })
          : Effect.fail(
              new HarnessCapabilityUnsupported({ harnessId, capability: "continue_turn" }),
            );

      const suspendTurn = (): Effect.Effect<
        HarnessContinuationState,
        HarnessCapabilityUnsupported
      > =>
        supportsSuspend
          ? Effect.gen(function* () {
              const cursor = yield* Ref.get(cursorRef);
              const active = yield* Ref.get(activeRef);
              const turn = Option.getOrUndefined(active);
              const turnId = turn?.turnId ?? "unknown";
              const remaining = turn?.remaining ?? [];
              return {
                harnessId,
                sessionId,
                turnId,
                cursor,
                lossy: continueIsLossy,
                data: { turnId, remaining } satisfies ContinuationData,
              };
            })
          : Effect.fail(
              new HarnessCapabilityUnsupported({ harnessId, capability: "suspend_turn" }),
            );

      const compact = () =>
        supportsCompact
          ? Effect.void
          : Effect.fail(new HarnessCapabilityUnsupported({ harnessId, capability: "compact" }));

      const resumeState = (): Effect.Effect<HarnessResumeState> =>
        Effect.gen(function* () {
          const opencodeSession = yield* Ref.get(opencodeSessionRef);
          return {
            harnessId,
            sessionId,
            data: (opencodeSession === undefined
              ? {}
              : { opencodeSessionId: opencodeSession }) satisfies OpencodeResumeData,
          };
        });

      const detach = (): Effect.Effect<HarnessResumeState, HarnessCapabilityUnsupported> =>
        supportsDetach
          ? resumeState()
          : Effect.fail(new HarnessCapabilityUnsupported({ harnessId, capability: "detach" }));

      const stop = (): Effect.Effect<HarnessResumeState> =>
        Effect.gen(function* () {
          yield* transport.shutdown();
          return yield* resumeState();
        });

      const session: HarnessSession = {
        sessionId,
        isResume: options.resumeFrom !== undefined || options.continueFrom !== undefined,
        modelId: config.model ?? "opencode",
        promptTurn,
        continueTurn,
        suspendTurn,
        compact,
        detach,
        stop,
        destroy: () => transport.shutdown(),
      };
      return session;
    });

  return {
    specificationVersion: "agent-harness-v1",
    harnessId,
    harnessKind: "opencode",
    adapterKind: "opencode",
    // opencode's native tool ids (lowercase) with their shared common names.
    builtinTools: [
      { nativeName: "bash", commonName: "bash", description: "run a shell command" },
      { nativeName: "read", commonName: "read", description: "read a file" },
      { nativeName: "write", commonName: "write", description: "write a file" },
      { nativeName: "edit", commonName: "edit", description: "edit a file" },
      { nativeName: "glob", commonName: "glob", description: "match paths by glob" },
      { nativeName: "grep", commonName: "grep", description: "search file contents" },
      { nativeName: "websearch", commonName: "webSearch", description: "search the web" },
      { nativeName: "webfetch", description: "fetch a URL" },
    ],
    // Live transport surfaces permission.v2.asked; the scripted fixture does not.
    supportsBuiltinToolApprovals: isLive,
    supportsBuiltinToolFiltering: false,
    lifecycleStateSchema: OpencodeResumeData,
    start,
  };
};
