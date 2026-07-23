import { Schema as S } from "effect";
import type { AgentRuntimeVisibility } from "@openagentsinc/agent-runtime-schema";
import { KhalaRuntimeFinishReason, KhalaRuntimeSafeRef } from "@openagentsinc/agent-runtime-schema";
import { HarnessToolIdentity, toolIdentity } from "./common-tool.ts";
import type { HarnessStreamEvent } from "./stream.ts";

/**
 * The renderer-facing UI chunk vocabulary (STREAM-02, the emission twin of the
 * ingestion-side `KhalaRuntimeAiSdkTextStreamPart`). Ideas re-derived from the
 * AI SDK stage-3 wire protocol `UIMessageChunk`
 * (`packages/ai/src/ui-message-stream/ui-message-chunks.ts` in the reference
 * clone) — no upstream code is vendored. Divergences from the AI SDK union are
 * deliberate and bounded:
 *
 * - The chunk carries only already-safe `KhalaRuntimeEvent` material: refs
 *   (`inputRef`, `resultRef`, `errorRef`), the safe error text
 *   (`messageSafe`), and text/reasoning deltas. There is no raw tool
 *   input/output payload on the wire — the AI SDK `input`/`output` unknowns
 *   are replaced by refs so the projection can never widen redaction.
 * - Every chunk may carry the durable replay `cursor` (the source event
 *   `sequence`, HARN-02), so a renderer can resume with the harness event log
 *   `attach` at its last chunk (the STREAM-03 transport seam).
 * - The AI SDK data-part `transient` flag is generalized: any chunk may be
 *   `transient`, meaning renderer-only — observed live, never folded into the
 *   persisted `UiMessage` (mirrors the transient bypass in
 *   `packages/ai/src/ui/process-ui-message-stream.ts`).
 * - The AI SDK three-chunk `tool-input-start`/`tool-input-delta` pair is
 *   collapsed into one self-starting `tool-input-streaming` chunk, because the
 *   Khala runtime vocabulary has no separate tool-input-start event.
 */

/** Fields shared by every chunk variant. */
const UiMessageChunkBase = {
  /**
   * Durable replay cursor: the `sequence` of the source `HarnessStreamEvent`.
   * Optional on the wire (a transport may synthesize chunks), always set by
   * {@link khalaEventToUiChunks}.
   */
  cursor: S.optionalKey(S.Number),
  /**
   * Renderer-only chunk. A transient chunk is observed live but never folded
   * into the persisted `UiMessage` (the reducer skips it).
   */
  transient: S.optionalKey(S.Boolean),
} as const;

/** Assistant message opened (from `turn.started`). */
export const UiMessageStartChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("message-start"),
  messageId: S.optionalKey(KhalaRuntimeSafeRef),
});
export interface UiMessageStartChunk extends S.Schema.Type<typeof UiMessageStartChunk> {}

/** Assistant message finished (from `turn.finished`). */
export const UiMessageFinishChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("message-finish"),
  finishReason: KhalaRuntimeFinishReason,
});
export interface UiMessageFinishChunk extends S.Schema.Type<typeof UiMessageFinishChunk> {}

/** Assistant message aborted mid-turn (from `turn.interrupted`). */
export const UiMessageAbortChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("message-abort"),
  reasonRef: S.optionalKey(KhalaRuntimeSafeRef),
});
export interface UiMessageAbortChunk extends S.Schema.Type<typeof UiMessageAbortChunk> {}

/** Step boundary opened (from `step.started`). */
export const UiStepStartChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("step-start"),
  stepId: S.optionalKey(KhalaRuntimeSafeRef),
});
export interface UiStepStartChunk extends S.Schema.Type<typeof UiStepStartChunk> {}

/** Step boundary closed (from `step.finished`). */
export const UiStepFinishChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("step-finish"),
  stepId: S.optionalKey(KhalaRuntimeSafeRef),
  finishReason: S.optionalKey(KhalaRuntimeFinishReason),
});
export interface UiStepFinishChunk extends S.Schema.Type<typeof UiStepFinishChunk> {}

/**
 * Text part opened. The Khala projection never emits this (the runtime
 * vocabulary has no `text.start` event — `text-delta` is self-starting in the
 * reducer), but the wire vocabulary keeps it so a non-Khala emitter can use
 * the AI SDK-shaped start/delta/end protocol.
 */
export const UiTextStartChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("text-start"),
  id: S.NonEmptyString,
});
export interface UiTextStartChunk extends S.Schema.Type<typeof UiTextStartChunk> {}

/** Incremental text (from `text.delta`, id-keyed by `messageId`). */
export const UiTextDeltaChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("text-delta"),
  id: S.NonEmptyString,
  delta: S.String,
});
export interface UiTextDeltaChunk extends S.Schema.Type<typeof UiTextDeltaChunk> {}

/** Text part closed (from `text.completed`). */
export const UiTextEndChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("text-end"),
  id: S.NonEmptyString,
});
export interface UiTextEndChunk extends S.Schema.Type<typeof UiTextEndChunk> {}

/** Reasoning part opened (wire-completeness twin of {@link UiTextStartChunk}). */
export const UiReasoningStartChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("reasoning-start"),
  id: S.NonEmptyString,
});
export interface UiReasoningStartChunk extends S.Schema.Type<typeof UiReasoningStartChunk> {}

/** Incremental reasoning text (from `reasoning.delta`). */
export const UiReasoningDeltaChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("reasoning-delta"),
  id: S.NonEmptyString,
  delta: S.String,
});
export interface UiReasoningDeltaChunk extends S.Schema.Type<typeof UiReasoningDeltaChunk> {}

/** Reasoning part closed (from `reasoning.completed`). */
export const UiReasoningEndChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("reasoning-end"),
  id: S.NonEmptyString,
});
export interface UiReasoningEndChunk extends S.Schema.Type<typeof UiReasoningEndChunk> {}

/**
 * Streamed partial tool input (from `tool.input.delta`). Self-starting: the
 * first chunk for a `toolCallId` opens the tool part in `input-streaming`.
 */
export const UiToolInputStreamingChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("tool-input-streaming"),
  toolCallId: S.NonEmptyString,
  tool: HarnessToolIdentity,
  inputTextDelta: S.String,
});
export interface UiToolInputStreamingChunk extends S.Schema.Type<
  typeof UiToolInputStreamingChunk
> {}

/**
 * Tool input complete, the call is placed (from `tool.call`). Mirrors the AI
 * SDK `tool-input-available`, with the raw `input` replaced by the safe
 * `inputRef`.
 */
export const UiToolInputAvailableChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("tool-input-available"),
  toolCallId: S.NonEmptyString,
  tool: HarnessToolIdentity,
  inputRef: S.optionalKey(KhalaRuntimeSafeRef),
});
export interface UiToolInputAvailableChunk extends S.Schema.Type<
  typeof UiToolInputAvailableChunk
> {}

/**
 * Streamed PRELIMINARY (partial) tool output (STREAM-07 #9135). The Effect v4
 * Toolkit handler context exposes `HandlerContext.preliminary`
 * (`effect/dist/unstable/ai/Toolkit.d.ts`), and every streamed
 * `Tool.HandlerResult` carries a `preliminary` flag
 * (`effect/dist/unstable/ai/Tool.d.ts`): "Preliminary results represent
 * progress updates; only the final result should be used as the authoritative
 * output." This chunk is that progress update on the wire — the raw partial
 * output is replaced by the safe `resultRef`, exactly like
 * {@link UiToolOutputAvailableChunk}. The reducer never folds it into the
 * persisted `UiMessage` tool state machine (progress is never authoritative
 * output); renderers that want live progress observe the chunk stream.
 */
export const UiToolOutputPreliminaryChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("tool-output-preliminary"),
  toolCallId: S.NonEmptyString,
  tool: HarnessToolIdentity,
  resultRef: KhalaRuntimeSafeRef,
});
export interface UiToolOutputPreliminaryChunk extends S.Schema.Type<
  typeof UiToolOutputPreliminaryChunk
> {}

/**
 * Tool result available (from `tool.result`). Mirrors the AI SDK
 * `tool-output-available`, with the raw `output` replaced by the safe
 * `resultRef`.
 */
export const UiToolOutputAvailableChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("tool-output-available"),
  toolCallId: S.NonEmptyString,
  tool: HarnessToolIdentity,
  resultRef: KhalaRuntimeSafeRef,
});
export interface UiToolOutputAvailableChunk extends S.Schema.Type<
  typeof UiToolOutputAvailableChunk
> {}

/** Tool failed (from `tool.error`). `errorText` is the event's safe text. */
export const UiToolOutputErrorChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("tool-output-error"),
  toolCallId: S.NonEmptyString,
  tool: HarnessToolIdentity,
  errorText: S.String,
  errorRef: S.optionalKey(KhalaRuntimeSafeRef),
});
export interface UiToolOutputErrorChunk extends S.Schema.Type<typeof UiToolOutputErrorChunk> {}

/**
 * Stream-level error. The Khala projection does not emit it (tool failures
 * are `tool-output-error`); a transport injects it with masked text, mirroring
 * the AI SDK `error` chunk whose text passes an `onError` mask.
 */
export const UiErrorChunk = S.Struct({
  ...UiMessageChunkBase,
  type: S.Literal("error"),
  errorText: S.String,
});
export interface UiErrorChunk extends S.Schema.Type<typeof UiErrorChunk> {}

export const UI_MESSAGE_CHUNK_TYPES = [
  "message-start",
  "message-finish",
  "message-abort",
  "step-start",
  "step-finish",
  "text-start",
  "text-delta",
  "text-end",
  "reasoning-start",
  "reasoning-delta",
  "reasoning-end",
  "tool-input-streaming",
  "tool-input-available",
  "tool-output-preliminary",
  "tool-output-available",
  "tool-output-error",
  "error",
] as const;
export type UiMessageChunkType = (typeof UI_MESSAGE_CHUNK_TYPES)[number];

/** The full renderer wire vocabulary, discriminated on `type`. */
export const UiMessageChunk = S.Union([
  UiMessageStartChunk,
  UiMessageFinishChunk,
  UiMessageAbortChunk,
  UiStepStartChunk,
  UiStepFinishChunk,
  UiTextStartChunk,
  UiTextDeltaChunk,
  UiTextEndChunk,
  UiReasoningStartChunk,
  UiReasoningDeltaChunk,
  UiReasoningEndChunk,
  UiToolInputStreamingChunk,
  UiToolInputAvailableChunk,
  UiToolOutputPreliminaryChunk,
  UiToolOutputAvailableChunk,
  UiToolOutputErrorChunk,
  UiErrorChunk,
]);
export type UiMessageChunk = typeof UiMessageChunk.Type;

export const decodeUiMessageChunk = S.decodeUnknownSync(UiMessageChunk);
export const encodeUiMessageChunk = S.encodeUnknownSync(UiMessageChunk);

/**
 * Options for {@link khalaEventToUiChunks}. Send flags re-derive the AI SDK
 * `toUIMessageChunk` gating (`sendReasoning`/`sendStart`/`sendFinish`,
 * `packages/ai/src/ui-message-stream/to-ui-message-chunk.ts`); the visibility
 * options map the event's `visibility` onto gated (dropped) or transient
 * (renderer-only) emission, which is how the AI SDK `transient` flag lands on
 * the Khala redaction model.
 */
export interface KhalaEventToUiChunksOptions {
  /** Emit `message-start` from `turn.started`. Default `true`. */
  readonly sendStart?: boolean;
  /** Emit `message-finish` from `turn.finished`. Default `true`. */
  readonly sendFinish?: boolean;
  /** Emit reasoning chunks from `reasoning.*`. Default `true`. */
  readonly sendReasoning?: boolean;
  /**
   * Visibilities admitted to the renderer at all. An event whose `visibility`
   * is not listed projects to `[]`. Default: all three (an owner-local
   * renderer sees the full stream); a public projection passes `["public"]`.
   */
  readonly admitVisibilities?: ReadonlyArray<AgentRuntimeVisibility>;
  /**
   * Visibilities whose chunks are marked `transient` (renderer-only, never
   * persisted into the durable `UiMessage`). Default: `["operator"]`.
   */
  readonly transientVisibilities?: ReadonlyArray<AgentRuntimeVisibility>;
}

const ALL_VISIBILITIES: ReadonlyArray<AgentRuntimeVisibility> = ["public", "operator", "private"];

/**
 * The pure, redaction-aware layer-1 projection `KhalaRuntimeEvent →
 * UiMessageChunk` (audit §5.2). It consumes ONLY already-safe event fields
 * (refs, `messageSafe`, deltas) and never widens redaction. Khala kinds that
 * are desktop-display-only telemetry with no chunk equivalent
 * (`agent.child.*`, `usage.recorded`, `provider.metadata`, `file.change`,
 * `writeback.recorded`, `compaction.recorded`, `raw.sidecar_ref`, and
 * `tool.input.completed`, which `tool.call` supersedes) project to `[]`.
 */
export const khalaEventToUiChunks = (
  event: HarnessStreamEvent,
  options?: KhalaEventToUiChunksOptions,
): ReadonlyArray<UiMessageChunk> => {
  const admit = options?.admitVisibilities ?? ALL_VISIBILITIES;
  if (!admit.includes(event.visibility)) return [];

  const transientVisibilities = options?.transientVisibilities ?? ["operator"];
  const base = {
    cursor: event.sequence,
    ...(transientVisibilities.includes(event.visibility) ? { transient: true as const } : {}),
  };

  switch (event.kind) {
    case "turn.started":
      return (options?.sendStart ?? true)
        ? [{ ...base, type: "message-start", messageId: event.turnId }]
        : [];
    case "turn.finished":
      return (options?.sendFinish ?? true)
        ? [{ ...base, type: "message-finish", finishReason: event.finishReason }]
        : [];
    case "turn.interrupted":
      return [
        {
          ...base,
          type: "message-abort",
          ...(event.reasonRef === undefined ? {} : { reasonRef: event.reasonRef }),
        },
      ];
    case "step.started":
      return [{ ...base, type: "step-start", stepId: event.stepId }];
    case "step.finished":
      return [
        { ...base, type: "step-finish", stepId: event.stepId, finishReason: event.finishReason },
      ];
    case "text.delta":
      return [{ ...base, type: "text-delta", id: event.messageId, delta: event.text }];
    case "text.completed":
      return [{ ...base, type: "text-end", id: event.messageId }];
    case "reasoning.delta":
      return (options?.sendReasoning ?? true)
        ? [{ ...base, type: "reasoning-delta", id: event.messageId, delta: event.text }]
        : [];
    case "reasoning.completed":
      return (options?.sendReasoning ?? true)
        ? [{ ...base, type: "reasoning-end", id: event.messageId }]
        : [];
    case "tool.input.delta":
      return [
        {
          ...base,
          type: "tool-input-streaming",
          toolCallId: event.toolCallId,
          tool: toolIdentity(event.toolName),
          inputTextDelta: event.inputDelta,
        },
      ];
    case "tool.call":
      return [
        {
          ...base,
          type: "tool-input-available",
          toolCallId: event.toolCallId,
          tool: toolIdentity(event.toolName),
          ...(event.inputRef === undefined ? {} : { inputRef: event.inputRef }),
        },
      ];
    case "tool.result":
      return [
        {
          ...base,
          type: "tool-output-available",
          toolCallId: event.toolCallId,
          tool: toolIdentity(
            event.toolName,
            event.providerExecuted === undefined
              ? undefined
              : { providerExecuted: event.providerExecuted },
          ),
          resultRef: event.resultRef,
        },
      ];
    case "tool.error":
      return [
        {
          ...base,
          type: "tool-output-error",
          toolCallId: event.toolCallId,
          tool: toolIdentity(
            event.toolName,
            event.providerExecuted === undefined
              ? undefined
              : { providerExecuted: event.providerExecuted },
          ),
          errorText: event.messageSafe,
          errorRef: event.errorRef,
        },
      ];
    case "tool.input.completed":
    case "agent.child.started":
    case "agent.child.progress":
    case "agent.child.finished":
    case "usage.recorded":
    case "provider.metadata":
    case "file.change":
    case "writeback.recorded":
    case "compaction.recorded":
    case "raw.sidecar_ref":
      return [];
  }
  // Exhaustive: every `KhalaRuntimeEvent` kind is handled above.
  return event satisfies never;
};
