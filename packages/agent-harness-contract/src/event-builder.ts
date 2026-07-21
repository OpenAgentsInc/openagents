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
