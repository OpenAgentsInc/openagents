// HARN-03: project the desktop `ClaudeLocalEvent` envelope onto the neutral
// harness stream (`KhalaRuntimeEvent`). This is the "one shared projection" the
// harvest analysis calls for: the frozen `ClaudeLocalEvent` envelope stays the
// renderer surface, but it becomes ONE projection of the harness stream rather
// than a second source of truth. Codex and the ACP lanes already hand-map their
// runtimes onto `ClaudeLocalEvent`; this module gives the inverse — the neutral,
// durable, cursor-carrying representation the HARN-02 event log and Full Auto
// liveness read.
//
// Scope: the CORE common kinds — turn lifecycle, text, reasoning, tool
// call/result, and usage. `ClaudeLocalEvent` is the richer, desktop-specific
// superset (plan/meter/question/child/followup display events have no neutral
// `KhalaRuntimeEvent` origin); those project to no core event and are left to the
// renderer envelope. The subset covered here is exactly the vocabulary a
// transcript, a usage accountant, and cursor-exact replay need.

import {
  buildTextDelta,
  buildTurnFinished,
  buildTurnStarted,
  type HarnessStreamEvent,
} from "@openagentsinc/agent-harness-contract";
import { toolIdentity } from "@openagentsinc/agent-harness-contract";
import {
  decodeKhalaRuntimeEvent,
  KhalaRuntimeEventSchemaLiteral,
  type KhalaRuntimeSource,
  type KhalaRuntimeUsage,
} from "@openagentsinc/agent-runtime-schema";
import type { ClaudeLocalEvent } from "./claude-local-contract.ts";

/** Context a projector needs: the turn/thread identity and the event source label. */
export interface HarnessProjectionContext {
  readonly turnId: string;
  readonly threadId: string;
  readonly source: KhalaRuntimeSource;
  /** First sequence number to assign. Defaults to 0. */
  readonly startSequence?: number;
}

const SAFE_REF = /[^A-Za-z0-9._:-]/g;

/** Coerce an arbitrary string into a `KhalaRuntimeSafeRef`-valid token. */
const safeRef = (value: string, fallback: string): string => {
  const cleaned = value.replace(SAFE_REF, "-");
  const trimmed = cleaned.replace(/^[^A-Za-z0-9]+/, "");
  return trimmed.length > 0 ? trimmed : fallback;
};

const baseFields = (ctx: HarnessProjectionContext, sequence: number, suffix: string) => ({
  schema: KhalaRuntimeEventSchemaLiteral,
  eventId: `evt.${safeRef(ctx.turnId, "turn")}.${sequence}.${suffix}`,
  turnId: ctx.turnId,
  threadId: ctx.threadId,
  sequence,
  observedAt: "1970-01-01T00:00:00.000Z",
  source: ctx.source,
  visibility: "private" as const,
  redactionClass: "private_ref" as const,
  causalityRefs: [] as ReadonlyArray<string>,
});

/**
 * An owner-local tool authority. The native desktop lanes run under the
 * owner-local danger profile where every built-in tool is allowed, so a
 * projected tool event carries an `allowed` authority with refs derived from the
 * tool call id. This is honest for those lanes — it is NOT a claim that an
 * untrusted or metered lane is unconditionally allowed.
 */
const ownerLocalAuthority = (toolCallId: string, toolRef: string) => ({
  authorityRef: `authority.local.${toolCallId}`,
  policyRef: "policy.owner-local.allow-all",
  decisionRef: `decision.local.${toolCallId}`,
  toolRef,
  status: "allowed" as const,
  allowed: true,
  blockerRefs: [] as ReadonlyArray<string>,
});

const usageOf = (
  ctx: HarnessProjectionContext,
  sequence: number,
  event: Extract<ClaudeLocalEvent, { kind: "turn_completed" }>,
): KhalaRuntimeUsage | undefined => {
  const usage = event.usage;
  const total = event.totalTokens;
  if (usage === undefined && (total === null || total === undefined)) {
    return undefined;
  }
  const usageRef = `usage.${safeRef(ctx.turnId, "turn")}.${sequence}`;
  return {
    usageRef,
    ...(usage?.inputTokens === undefined ? {} : { inputTokens: usage.inputTokens }),
    ...(usage?.outputTokens === undefined ? {} : { outputTokens: usage.outputTokens }),
    ...(usage?.reasoningTokens === undefined ? {} : { reasoningTokens: usage.reasoningTokens }),
    ...(total === null || total === undefined ? {} : { totalTokens: total }),
  };
};

/**
 * Build a stateful projector for one turn. Each call maps a `ClaudeLocalEvent`
 * to zero or more `HarnessStreamEvent`s, allocating contiguous sequence numbers
 * so the output is a valid, cursor-carrying harness stream (the HARN-02 event
 * log appends these directly).
 */
export const makeClaudeLocalHarnessProjector = (
  ctx: HarnessProjectionContext,
): ((event: ClaudeLocalEvent) => ReadonlyArray<HarnessStreamEvent>) => {
  let sequence = ctx.startSequence ?? 0;
  const next = () => sequence++;
  const messageId = `msg.${safeRef(ctx.turnId, "turn")}`;

  return (event: ClaudeLocalEvent): ReadonlyArray<HarnessStreamEvent> => {
    switch (event.kind) {
      case "turn_started":
        return [
          buildTurnStarted({
            turnId: ctx.turnId,
            threadId: ctx.threadId,
            sequence: next(),
            source: ctx.source,
          }),
        ];
      case "text_delta":
        return [
          buildTextDelta({
            turnId: ctx.turnId,
            threadId: ctx.threadId,
            sequence: next(),
            source: ctx.source,
            messageId,
            text: event.text,
          }),
        ];
      case "reasoning": {
        const seq = next();
        return [
          decodeKhalaRuntimeEvent({
            ...baseFields(ctx, seq, "reasoning"),
            kind: "reasoning.delta",
            messageId,
            chunkId: `chunk.${safeRef(ctx.turnId, "turn")}.${seq}`,
            text: event.text,
          }),
        ];
      }
      case "tool_use": {
        const seq = next();
        const toolCallId = safeRef(event.itemRef ?? `tool.${seq}`, `tool.${seq}`);
        const identity = toolIdentity(event.toolName);
        return [
          decodeKhalaRuntimeEvent({
            ...baseFields(ctx, seq, "toolcall"),
            kind: "tool.call",
            toolCallId,
            toolName: identity.wireName,
            authority: ownerLocalAuthority(toolCallId, identity.nativeName),
          }),
        ];
      }
      case "tool_result": {
        const seq = next();
        const toolCallId = safeRef(event.itemRef ?? `tool.${seq}`, `tool.${seq}`);
        const identity = toolIdentity(event.toolName);
        return [
          decodeKhalaRuntimeEvent({
            ...baseFields(ctx, seq, "toolresult"),
            kind: "tool.result",
            toolCallId,
            toolName: identity.wireName,
            resultRef: `result.${safeRef(ctx.turnId, "turn")}.${seq}`,
            authority: ownerLocalAuthority(toolCallId, identity.nativeName),
            providerExecuted: true,
          }),
        ];
      }
      case "turn_completed": {
        const seq = next();
        const usage = usageOf(ctx, seq, event);
        return [
          buildTurnFinished({
            turnId: ctx.turnId,
            threadId: ctx.threadId,
            sequence: seq,
            source: ctx.source,
            finishReason: "stop",
            ...(usage === undefined ? {} : { usage }),
          }),
        ];
      }
      case "turn_failed": {
        const seq = next();
        return [
          decodeKhalaRuntimeEvent({
            ...baseFields(ctx, seq, "interrupted"),
            kind: "turn.interrupted",
            reasonRef: safeRef(event.reason, "failed"),
          }),
        ];
      }
      default:
        // Desktop-display-only kinds (plan_updated, meter_updated, question_*,
        // child_*, followup_*, lane_notice, mcp_server_unavailable,
        // model_effective, composer_admission, available/unavailable,
        // tool_progress, child_steered) have no neutral core event. They remain
        // on the renderer envelope and project to nothing here.
        return [];
    }
  };
};
