import { Effect, Fiber, Schema as S, Stream, SubscriptionRef } from "effect";
import { KhalaRuntimeFinishReason, KhalaRuntimeSafeRef } from "@openagentsinc/agent-runtime-schema";
import { HarnessToolIdentity } from "./common-tool.ts";
import type { UiMessageChunk } from "./ui-message-chunk.ts";

/**
 * The layer-3 progressive reducer (STREAM-02): `Stream<UiMessageChunk>` folded
 * into progressively-more-complete `UiMessage` snapshots held in a
 * `SubscriptionRef`, so a renderer reads the current value and the change
 * stream from one source. Ideas re-derived from the AI SDK
 * `processUIMessageStream` reducer and its `StreamingUIMessageState`
 * (`packages/ai/src/ui/process-ui-message-stream.ts` in the reference clone)
 * and from `readUIMessageStream`'s snapshot-per-update semantics — no upstream
 * code is vendored. Deliberate divergences:
 *
 * - The fold is immutable: every applied chunk produces a fresh `UiMessage`
 *   value (the AI SDK mutates one message and `structuredClone`s per emit).
 * - There are no separate active-part maps: a part's own `state`
 *   (`streaming`/`done`) carries what the AI SDK tracks in
 *   `activeTextParts`/`activeReasoningParts`/`partialToolCalls`.
 * - `text-delta`/`reasoning-delta` are self-starting (the Khala runtime
 *   vocabulary has no `text.start` event); the AI SDK instead throws on a
 *   delta without a start. A delta after `*-end`, and any tool state-machine
 *   regression, still fails with the tagged {@link UiMessageReducerError} —
 *   a malformed sequence never silently corrupts a snapshot.
 * - `transient` chunks bypass the persisted message entirely, mirroring the
 *   AI SDK transient data-part bypass.
 */

/** A malformed chunk sequence. The reducer fails closed, never corrupts. */
export class UiMessageReducerError extends S.TaggedErrorClass<UiMessageReducerError>()(
  "AgentHarness.UiMessageReducerError",
  {
    chunkType: S.String,
    chunkId: S.optionalKey(S.String),
    detail: S.String,
  },
) {}

export const UI_STREAM_PART_STATES = ["streaming", "done"] as const;
export type UiStreamPartState = (typeof UI_STREAM_PART_STATES)[number];
export const UiStreamPartStateSchema = S.Literals(UI_STREAM_PART_STATES);

/** A streamed text part, id-keyed (the Khala `messageId`). */
export const UiTextPart = S.Struct({
  type: S.Literal("text"),
  id: S.NonEmptyString,
  text: S.String,
  state: UiStreamPartStateSchema,
});
export interface UiTextPart extends S.Schema.Type<typeof UiTextPart> {}

/** A streamed reasoning part, id-keyed. */
export const UiReasoningPart = S.Struct({
  type: S.Literal("reasoning"),
  id: S.NonEmptyString,
  text: S.String,
  state: UiStreamPartStateSchema,
});
export interface UiReasoningPart extends S.Schema.Type<typeof UiReasoningPart> {}

export const UI_TOOL_STATES = [
  "input-streaming",
  "input-available",
  "output-available",
  "output-error",
] as const;
export type UiToolStateName = (typeof UI_TOOL_STATES)[number];
export const UiToolStateSchema = S.Literals(UI_TOOL_STATES);

const UiToolPartBase = {
  type: S.Literal("tool"),
  toolCallId: S.NonEmptyString,
  tool: HarnessToolIdentity,
  /** Concatenated streamed input text so far (may be a partial JSON prefix). */
  inputText: S.String,
} as const;

/**
 * The tool-call state machine as an Effect Schema discriminated union on
 * `state` (audit S1): `input-streaming → input-available → output-available |
 * output-error`. Decoded fail-closed at the boundary.
 */
export const UiToolPart = S.Union([
  S.Struct({ ...UiToolPartBase, state: S.Literal("input-streaming") }),
  S.Struct({
    ...UiToolPartBase,
    state: S.Literal("input-available"),
    inputRef: S.optionalKey(KhalaRuntimeSafeRef),
  }),
  S.Struct({
    ...UiToolPartBase,
    state: S.Literal("output-available"),
    inputRef: S.optionalKey(KhalaRuntimeSafeRef),
    resultRef: KhalaRuntimeSafeRef,
  }),
  S.Struct({
    ...UiToolPartBase,
    state: S.Literal("output-error"),
    inputRef: S.optionalKey(KhalaRuntimeSafeRef),
    errorRef: S.optionalKey(KhalaRuntimeSafeRef),
    errorText: S.String,
  }),
]);
export type UiToolPart = typeof UiToolPart.Type;

/** A step boundary marker (mirrors the AI SDK `step-start` part). */
export const UiStepStartPart = S.Struct({ type: S.Literal("step-start") });
export interface UiStepStartPart extends S.Schema.Type<typeof UiStepStartPart> {}

export const UiMessagePart = S.Union([UiTextPart, UiReasoningPart, UiToolPart, UiStepStartPart]);
export type UiMessagePart = typeof UiMessagePart.Type;

export const UI_MESSAGE_STATUSES = ["streaming", "complete", "aborted"] as const;
export type UiMessageStatus = (typeof UI_MESSAGE_STATUSES)[number];
export const UiMessageStatusSchema = S.Literals(UI_MESSAGE_STATUSES);

/**
 * The progressively-reconstructed message a renderer binds to. Every reducer
 * snapshot is one more-complete value of this schema.
 */
export const UiMessage = S.Struct({
  id: S.NonEmptyString,
  role: S.Literals(["user", "assistant", "system"]),
  parts: S.Array(UiMessagePart),
  status: UiMessageStatusSchema,
  finishReason: S.optionalKey(KhalaRuntimeFinishReason),
  errorText: S.optionalKey(S.String),
});
export interface UiMessage extends S.Schema.Type<typeof UiMessage> {}

export const decodeUiMessage = S.decodeUnknownSync(UiMessage);

export interface InitialUiMessageOptions {
  /** Message id before a `message-start` chunk names one. */
  readonly messageId?: string;
}

/** The empty streaming assistant message the fold starts from. */
export const initialUiMessage = (options?: InitialUiMessageOptions): UiMessage => ({
  id: options?.messageId ?? "ui.message.pending",
  role: "assistant",
  parts: [],
  status: "streaming",
});

const malformed = (chunk: UiMessageChunk, chunkId: string | undefined, detail: string): never => {
  throw new UiMessageReducerError({
    chunkType: chunk.type,
    ...(chunkId === undefined ? {} : { chunkId }),
    detail,
  });
};

const replacePart = (
  parts: ReadonlyArray<UiMessagePart>,
  index: number,
  part: UiMessagePart,
): ReadonlyArray<UiMessagePart> => [...parts.slice(0, index), part, ...parts.slice(index + 1)];

/** Apply a text/reasoning delta or end chunk to the id-keyed streamed part. */
const applyStreamedTextChunk = (
  message: UiMessage,
  chunk: UiMessageChunk & { readonly id: string },
  partType: "text" | "reasoning",
  action: "start" | "delta" | "end",
  delta: string,
): UiMessage => {
  const index = message.parts.findIndex((part) => part.type === partType && part.id === chunk.id);
  const candidate = index === -1 ? undefined : message.parts[index];
  const existing =
    candidate !== undefined && (candidate.type === "text" || candidate.type === "reasoning")
      ? candidate
      : undefined;

  const makePart = (text: string, state: UiStreamPartState): UiTextPart | UiReasoningPart =>
    partType === "text"
      ? { type: "text", id: chunk.id, text, state }
      : { type: "reasoning", id: chunk.id, text, state };

  switch (action) {
    case "start": {
      if (existing !== undefined) {
        return malformed(chunk, chunk.id, `duplicate ${partType} part id`);
      }
      return { ...message, parts: [...message.parts, makePart("", "streaming")] };
    }
    case "delta": {
      if (existing === undefined) {
        // Self-starting: the Khala vocabulary has no text/reasoning start event.
        return { ...message, parts: [...message.parts, makePart(delta, "streaming")] };
      }
      if (existing.state === "done") {
        return malformed(chunk, chunk.id, `${partType} delta after ${partType} end`);
      }
      return {
        ...message,
        parts: replacePart(message.parts, index, { ...existing, text: existing.text + delta }),
      };
    }
    case "end": {
      if (existing === undefined) {
        // A completion with zero deltas is a legal empty part.
        return { ...message, parts: [...message.parts, makePart("", "done")] };
      }
      if (existing.state === "done") {
        return malformed(chunk, chunk.id, `duplicate ${partType} end`);
      }
      return {
        ...message,
        parts: replacePart(message.parts, index, { ...existing, state: "done" }),
      };
    }
  }
};

const findToolPart = (
  message: UiMessage,
  toolCallId: string,
): { readonly index: number; readonly part: UiToolPart | undefined } => {
  const index = message.parts.findIndex(
    (part) => part.type === "tool" && part.toolCallId === toolCallId,
  );
  const candidate = index === -1 ? undefined : message.parts[index];
  return {
    index,
    part: candidate !== undefined && candidate.type === "tool" ? candidate : undefined,
  };
};

const toolInputRef = (part: UiToolPart): { readonly inputRef?: KhalaRuntimeSafeRef } =>
  part.state !== "input-streaming" && part.inputRef !== undefined
    ? { inputRef: part.inputRef }
    : {};

/**
 * The pure progressive fold: one chunk applied to one snapshot yields the
 * next snapshot. Exported for direct conformance testing. Throws
 * {@link UiMessageReducerError} on a malformed sequence (state-machine
 * regression, delta after end, output for an unknown tool call) — use
 * {@link reduceUiMessageStream} for the Effect error channel.
 */
export const applyUiChunk = (message: UiMessage, chunk: UiMessageChunk): UiMessage => {
  if (chunk.transient === true) return message;

  switch (chunk.type) {
    case "message-start":
      return {
        ...message,
        ...(chunk.messageId === undefined ? {} : { id: chunk.messageId }),
      };
    case "message-finish":
      return { ...message, status: "complete", finishReason: chunk.finishReason };
    case "message-abort":
      return { ...message, status: "aborted" };
    case "step-start":
      return { ...message, parts: [...message.parts, { type: "step-start" }] };
    case "step-finish":
      // Close still-streaming text/reasoning parts — the immutable equivalent
      // of the AI SDK resetting its active-part maps on `finish-step`.
      return {
        ...message,
        parts: message.parts.map((part) =>
          (part.type === "text" || part.type === "reasoning") && part.state === "streaming"
            ? { ...part, state: "done" }
            : part,
        ),
      };
    case "text-start":
      return applyStreamedTextChunk(message, chunk, "text", "start", "");
    case "text-delta":
      return applyStreamedTextChunk(message, chunk, "text", "delta", chunk.delta);
    case "text-end":
      return applyStreamedTextChunk(message, chunk, "text", "end", "");
    case "reasoning-start":
      return applyStreamedTextChunk(message, chunk, "reasoning", "start", "");
    case "reasoning-delta":
      return applyStreamedTextChunk(message, chunk, "reasoning", "delta", chunk.delta);
    case "reasoning-end":
      return applyStreamedTextChunk(message, chunk, "reasoning", "end", "");
    case "tool-input-streaming": {
      const { index, part } = findToolPart(message, chunk.toolCallId);
      if (part === undefined) {
        return {
          ...message,
          parts: [
            ...message.parts,
            {
              type: "tool",
              toolCallId: chunk.toolCallId,
              tool: chunk.tool,
              inputText: chunk.inputTextDelta,
              state: "input-streaming",
            },
          ],
        };
      }
      if (part.state !== "input-streaming") {
        return malformed(chunk, chunk.toolCallId, `tool input delta after state "${part.state}"`);
      }
      return {
        ...message,
        parts: replacePart(message.parts, index, {
          ...part,
          inputText: part.inputText + chunk.inputTextDelta,
        }),
      };
    }
    case "tool-input-available": {
      const { index, part } = findToolPart(message, chunk.toolCallId);
      const next: UiToolPart = {
        type: "tool",
        toolCallId: chunk.toolCallId,
        tool: chunk.tool,
        inputText: part?.inputText ?? "",
        state: "input-available",
        ...(chunk.inputRef === undefined ? {} : { inputRef: chunk.inputRef }),
      };
      if (part === undefined) {
        return { ...message, parts: [...message.parts, next] };
      }
      if (part.state !== "input-streaming" && part.state !== "input-available") {
        return malformed(
          chunk,
          chunk.toolCallId,
          `tool input available after state "${part.state}"`,
        );
      }
      return { ...message, parts: replacePart(message.parts, index, next) };
    }
    case "tool-output-preliminary":
      // STREAM-07 (#9135): a preliminary tool output is live progress, never
      // authoritative output (Effect v4 `Tool.HandlerResult.preliminary`,
      // `effect/dist/unstable/ai/Tool.d.ts`: "only the final result should be
      // used as the authoritative output"). The persisted tool state machine
      // is unchanged; a renderer that wants live progress observes the chunk
      // stream directly.
      return message;
    case "tool-output-available": {
      const { index, part } = findToolPart(message, chunk.toolCallId);
      if (part === undefined) {
        return malformed(chunk, chunk.toolCallId, "no tool part for tool call id");
      }
      if (part.state === "output-error") {
        return malformed(chunk, chunk.toolCallId, 'tool output after state "output-error"');
      }
      return {
        ...message,
        parts: replacePart(message.parts, index, {
          type: "tool",
          toolCallId: chunk.toolCallId,
          tool: chunk.tool,
          inputText: part.inputText,
          state: "output-available",
          resultRef: chunk.resultRef,
          ...toolInputRef(part),
        }),
      };
    }
    case "tool-output-error": {
      const { index, part } = findToolPart(message, chunk.toolCallId);
      if (part === undefined) {
        return malformed(chunk, chunk.toolCallId, "no tool part for tool call id");
      }
      if (part.state === "output-error") {
        return malformed(chunk, chunk.toolCallId, "duplicate tool output error");
      }
      return {
        ...message,
        parts: replacePart(message.parts, index, {
          type: "tool",
          toolCallId: chunk.toolCallId,
          tool: chunk.tool,
          inputText: part.inputText,
          state: "output-error",
          errorText: chunk.errorText,
          ...(chunk.errorRef === undefined ? {} : { errorRef: chunk.errorRef }),
          ...toolInputRef(part),
        }),
      };
    }
    case "error":
      return { ...message, errorText: chunk.errorText };
  }
  return chunk satisfies never;
};

export interface ReduceUiMessageStreamOptions extends InitialUiMessageOptions {}

/**
 * Handle over a running reduction: `ref` publishes every snapshot (current
 * value plus change stream from one source), `done` resolves with the final
 * message when the chunk stream ends, or fails with the reducer/stream error.
 */
export interface UiMessageStreamHandle<E = never> {
  readonly ref: SubscriptionRef.SubscriptionRef<UiMessage>;
  readonly done: Effect.Effect<UiMessage, UiMessageReducerError | E>;
}

/**
 * Fold a chunk stream into a `SubscriptionRef<UiMessage>` of progressive
 * snapshots. The consuming fiber is forked as a child of the caller's fiber
 * (`Effect.forkChild`); on a malformed sequence the fold stops, `done` fails
 * with the tagged error, and `ref` keeps the last good snapshot.
 */
export const reduceUiMessageStream = <E = never>(
  stream: Stream.Stream<UiMessageChunk, E>,
  options?: ReduceUiMessageStreamOptions,
): Effect.Effect<UiMessageStreamHandle<E>> =>
  Effect.gen(function* () {
    const ref = yield* SubscriptionRef.make(initialUiMessage(options));
    const fiber = yield* Effect.forkChild(
      Stream.runForEach(stream, (chunk) =>
        SubscriptionRef.updateEffect(ref, (message) =>
          Effect.try({
            try: () => applyUiChunk(message, chunk),
            catch: (cause) =>
              cause instanceof UiMessageReducerError
                ? cause
                : new UiMessageReducerError({
                    chunkType: chunk.type,
                    detail: `unexpected reducer failure: ${String(cause)}`,
                  }),
          }),
        ),
      ).pipe(Effect.flatMap(() => SubscriptionRef.get(ref))),
    );
    return { ref, done: Fiber.join(fiber) };
  });
