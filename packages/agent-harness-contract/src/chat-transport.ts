import { Context, Effect, Option, Schema as S, Stream } from "effect";
import type { UiMessageChunk } from "./ui-message-chunk.ts";

/**
 * STREAM-03 — Effect `ChatTransport` over the harness event log.
 *
 * Ideas re-derived from the AI SDK `ChatTransport` surface
 * (`packages/ai/src/ui/chat-transport.ts` in the reference clone):
 * `sendMessages → stream` and `reconnectToStream → stream | null`. No
 * upstream code is vendored. The AI SDK `null`-versus-stream return maps
 * onto "no active turn" versus "attach at cursor N" on the HARN-02 event
 * log (`attach` is single-flight, gap-free, and dup-free).
 *
 * Two Layers implement this service:
 * - desktop IPC — {@link makeDesktopIpcChatTransport} / IPC envelope codecs
 * - web SSE — {@link makeWebSseChatTransport} / SSE encoder over Cloud Run
 *
 * Both share the event-log-backed core
 * {@link makeEventLogChatTransport}.
 */

/** Typed failure of a transport verb. */
export class ChatTransportError extends S.TaggedErrorClass<ChatTransportError>()(
  "AgentHarness.ChatTransportError",
  {
    operation: S.String,
    turnId: S.optionalKey(S.String),
    detail: S.optionalKey(S.String),
    cause: S.optionalKey(S.Defect()),
  },
) {}

/** One user message a renderer may submit on a new turn. */
export const ChatTransportUserMessage = S.Struct({
  id: S.optionalKey(S.NonEmptyString),
  role: S.Literal("user"),
  text: S.NonEmptyString,
});
export interface ChatTransportUserMessage extends S.Schema.Type<typeof ChatTransportUserMessage> {}

/** Input for {@link ChatTransport.sendMessages}. */
export interface ChatTransportSendMessagesOptions {
  /** Host-assigned turn id. Must be unique per turn. */
  readonly turnId: string;
  /** Owning thread id (for producers that need it). */
  readonly threadId: string;
  /** Fresh user messages that start this turn. */
  readonly messages: ReadonlyArray<ChatTransportUserMessage>;
  /**
   * Consumer class for the HARN-02 single-flight attach key
   * (`${turnId}:${consumerClass}`). Defaults to `"renderer"`.
   */
  readonly consumerClass?: string;
}

/** Input for {@link ChatTransport.reconnectToStream}. */
export interface ChatTransportReconnectOptions {
  readonly turnId: string;
  /**
   * Last durable cursor the renderer applied. Events with
   * `sequence > fromCursor` are delivered; pass `-1` to attach from the
   * start of the turn.
   */
  readonly fromCursor: number;
  readonly consumerClass?: string;
}

/**
 * Effect service: one contract from a new turn (or a reconnect) to the
 * renderer-facing `UiMessageChunk` stream. Layers differ only in how the
 * chunks cross the process/network boundary (IPC vs SSE).
 */
export interface ChatTransport {
  /**
   * Start a new turn and return the live chunk stream. Completes when the
   * turn ends (message-finish / message-abort, or producer completion with
   * no more events). Fails with {@link ChatTransportError} on transport or
   * producer failure.
   */
  readonly sendMessages: (
    options: ChatTransportSendMessagesOptions,
  ) => Stream.Stream<UiMessageChunk, ChatTransportError>;

  /**
   * Re-attach to an in-flight or completed turn at `fromCursor`. Returns
   * `Option.none` when there is no active or replayable turn (the AI SDK
   * `null` path). Returns `Option.some(stream)` when the harness event log
   * can attach — including a fully completed turn, so a reloading renderer
   * can rebuild the progressive message from the durable tail.
   */
  readonly reconnectToStream: (
    options: ChatTransportReconnectOptions,
  ) => Effect.Effect<
    Option.Option<Stream.Stream<UiMessageChunk, ChatTransportError>>,
    ChatTransportError
  >;
}

/**
 * Effect service tag for {@link ChatTransport}. Yield with
 * `yield* ChatTransportService` inside an Effect that provides a Layer.
 */
export class ChatTransportService extends Context.Service<ChatTransportService, ChatTransport>()(
  "@openagentsinc/agent-harness-contract/ChatTransport",
) {}

export const ChatTransportTag = ChatTransportService;
