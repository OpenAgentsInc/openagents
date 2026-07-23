import {
  decodeKhalaRuntimeEvent,
  KhalaRuntimeEventSchemaLiteral,
  type KhalaRuntimeFinishReason,
  type KhalaRuntimeSource,
  type KhalaRuntimeUsage,
} from "@openagentsinc/agent-runtime-schema";
import type { HarnessStreamEvent } from "./stream.ts";

/**
 * Small builders for the common {@link HarnessStreamEvent} kinds, used by the
 * reference adapter and by adapter/runtime tests. Every builder validates the
 * constructed object through `decodeKhalaRuntimeEvent`, so a malformed event is
 * caught at construction rather than at the stream boundary. Deterministic:
 * `observedAt` defaults to a fixed timestamp so test output is stable.
 */

const DEFAULT_OBSERVED_AT = "2026-07-20T00:00:00.000Z";

interface BaseFields {
  readonly turnId: string;
  readonly threadId: string;
  readonly sequence: number;
  readonly source: KhalaRuntimeSource;
  readonly observedAt?: string;
}

const base = (fields: BaseFields, eventSuffix: string) => ({
  schema: KhalaRuntimeEventSchemaLiteral,
  eventId: `evt.${fields.turnId}.${fields.sequence}.${eventSuffix}`,
  turnId: fields.turnId,
  threadId: fields.threadId,
  sequence: fields.sequence,
  observedAt: fields.observedAt ?? DEFAULT_OBSERVED_AT,
  source: fields.source,
  visibility: "private",
  redactionClass: "private_ref",
  causalityRefs: [] as ReadonlyArray<string>,
});

export const buildTurnStarted = (fields: BaseFields): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(fields, "start"),
    kind: "turn.started",
  });

export const buildTextDelta = (
  fields: BaseFields & { readonly messageId: string; readonly text: string },
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(fields, "text"),
    kind: "text.delta",
    messageId: fields.messageId,
    chunkId: `chunk.${fields.turnId}.${fields.sequence}`,
    text: fields.text,
  });

export const buildTurnFinished = (
  fields: BaseFields & {
    readonly finishReason: KhalaRuntimeFinishReason;
    readonly usage?: KhalaRuntimeUsage;
  },
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(fields, "finish"),
    kind: "turn.finished",
    finishReason: fields.finishReason,
    ...(fields.usage === undefined ? {} : { usage: fields.usage }),
  });

export const buildCompactionRecorded = (
  fields: BaseFields & {
    readonly beforeContextRef: string;
    readonly afterContextRef: string;
  },
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(fields, "compaction"),
    kind: "compaction.recorded",
    beforeContextRef: fields.beforeContextRef,
    afterContextRef: fields.afterContextRef,
  });

/**
 * Owner-local host-tool authority. Host tools run under the owner-local
 * profile on desktop; the authority is allowed with refs derived from the
 * tool call id. This is honest for owner-local lanes — it is not a claim that
 * an untrusted or metered lane is unconditionally allowed.
 */
const hostToolAuthority = (toolCallId: string, toolName: string) => ({
  authorityRef: `authority.host_tool.${toolCallId}`,
  policyRef: "policy.host_tool.owner_local",
  decisionRef: `decision.host_tool.${toolCallId}`,
  toolRef: `toolref.host_tool.${toolName}`,
  status: "allowed" as const,
  allowed: true,
  blockerRefs: [] as ReadonlyArray<string>,
});

/** Build a `tool.call` event for a host-executed tool (RLM-03 re-entry). */
export const buildToolCall = (
  fields: BaseFields & {
    readonly toolCallId: string;
    readonly toolName: string;
    readonly inputRef?: string;
  },
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(fields, "toolcall"),
    kind: "tool.call",
    toolCallId: fields.toolCallId,
    toolName: fields.toolName,
    ...(fields.inputRef === undefined ? {} : { inputRef: fields.inputRef }),
    authority: hostToolAuthority(fields.toolCallId, fields.toolName),
  });

/**
 * Build a `tool.result` event for a host-executed tool. `providerExecuted` is
 * false for host tools (the host ran them, not the model provider runtime).
 * The result payload itself never rides the neutral stream — only `resultRef`.
 */
export const buildToolResult = (
  fields: BaseFields & {
    readonly toolCallId: string;
    readonly toolName: string;
    readonly resultRef: string;
    readonly providerExecuted?: boolean;
  },
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(fields, "toolresult"),
    kind: "tool.result",
    toolCallId: fields.toolCallId,
    toolName: fields.toolName,
    resultRef: fields.resultRef,
    authority: hostToolAuthority(fields.toolCallId, fields.toolName),
    providerExecuted: fields.providerExecuted ?? false,
  });

/** Build a `tool.error` event for a host-tool failure with a safe message. */
export const buildToolError = (
  fields: BaseFields & {
    readonly toolCallId: string;
    readonly toolName: string;
    readonly errorRef: string;
    readonly messageSafe: string;
    readonly providerExecuted?: boolean;
  },
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(fields, "toolerror"),
    kind: "tool.error",
    toolCallId: fields.toolCallId,
    toolName: fields.toolName,
    errorRef: fields.errorRef,
    messageSafe: fields.messageSafe,
    authority: hostToolAuthority(fields.toolCallId, fields.toolName),
    providerExecuted: fields.providerExecuted ?? false,
  });
