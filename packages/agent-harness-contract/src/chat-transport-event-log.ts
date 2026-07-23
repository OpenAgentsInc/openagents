import { Effect, Option, Stream } from "effect";
import type { HarnessEventLog } from "./event-log.ts";
import { HarnessEventLogError } from "./event-log-store.ts";
import type { HarnessStreamEvent } from "./stream.ts";
import {
  ChatTransportError,
  type ChatTransport,
  type ChatTransportReconnectOptions,
  type ChatTransportSendMessagesOptions,
} from "./chat-transport.ts";
import {
  khalaEventToUiChunks,
  type KhalaEventToUiChunksOptions,
  type UiMessageChunk,
} from "./ui-message-chunk.ts";

const DEFAULT_CONSUMER = "renderer";

/**
 * A producer the host hands to the event-log transport. It must append every
 * `HarnessStreamEvent` for `turnId` through the same {@link HarnessEventLog}
 * the transport attaches to. The transport forks this effect under the stream
 * scope so interruption of the consumer cancels the producer.
 */
export type ChatTransportTurnProducer = (params: {
  readonly turnId: string;
  readonly threadId: string;
  readonly messages: ChatTransportSendMessagesOptions["messages"];
  readonly log: HarnessEventLog;
}) => Effect.Effect<void, ChatTransportError>;

export interface EventLogChatTransportOptions {
  readonly log: HarnessEventLog;
  /**
   * Runs once per `sendMessages` call. When omitted, `sendMessages` only
   * attaches to the log (useful when a separate host path already produces
   * events, e.g. an existing desktop lane).
   */
  readonly produce?: ChatTransportTurnProducer;
  /** Projection options for {@link khalaEventToUiChunks}. */
  readonly project?: KhalaEventToUiChunksOptions;
}

const mapLogError =
  (operation: string, turnId: string) =>
  (error: HarnessEventLogError): ChatTransportError =>
    new ChatTransportError({
      operation,
      turnId,
      detail: error.detail ?? error.operation,
      cause: error,
    });

const isTerminalEvent = (event: HarnessStreamEvent): boolean =>
  event.kind === "turn.finished" || event.kind === "turn.interrupted";

const projectEventStream = (
  events: Stream.Stream<HarnessStreamEvent, HarnessEventLogError>,
  project: KhalaEventToUiChunksOptions | undefined,
  turnId: string,
): Stream.Stream<UiMessageChunk, ChatTransportError> =>
  events.pipe(
    Stream.mapError(mapLogError("attach", turnId)),
    Stream.flatMap((event) => Stream.fromIterable(khalaEventToUiChunks(event, project))),
  );

/**
 * Finite replay of a completed turn. Prefer this over live attach whenever the
 * terminal event is already persisted — attach's live tail never completes on
 * its own (HARN-02 tests interrupt it explicitly).
 */
const replayChunks = (
  log: HarnessEventLog,
  turnId: string,
  fromCursor: number,
  project: KhalaEventToUiChunksOptions | undefined,
): Stream.Stream<UiMessageChunk, ChatTransportError> =>
  projectEventStream(log.replay({ turnId, fromCursor }), project, turnId);

/**
 * Live attach for an in-flight turn. Ends at the first terminal harness event
 * so the consumer does not hang waiting for a live tail that will never come
 * after `turn.finished` / `turn.interrupted`.
 */
const attachChunks = (
  log: HarnessEventLog,
  turnId: string,
  fromCursor: number,
  consumerClass: string,
  project: KhalaEventToUiChunksOptions | undefined,
): Stream.Stream<UiMessageChunk, ChatTransportError> =>
  projectEventStream(
    log.attach({ turnId, fromCursor, consumerClass }).pipe(Stream.takeUntil(isTerminalEvent)),
    project,
    turnId,
  );

/** True when the last stored event for the turn is terminal. */
const turnIsComplete = (
  log: HarnessEventLog,
  turnId: string,
): Effect.Effect<boolean, ChatTransportError> =>
  Stream.runCollect(log.replay({ turnId, fromCursor: -1 })).pipe(
    Effect.mapError(mapLogError("replay", turnId)),
    Effect.map((events) => {
      if (events.length === 0) return false;
      return isTerminalEvent(events[events.length - 1]!);
    }),
  );

/**
 * Build a {@link ChatTransport} over a {@link HarnessEventLog}. This is the
 * shared core both the desktop IPC Layer and the web SSE Layer use: the
 * durable resume primitive is the shipped HARN-02 `attach` / `replay`.
 */
export const makeEventLogChatTransport = (options: EventLogChatTransportOptions): ChatTransport => {
  const { log, produce, project } = options;

  const sendMessages = (
    input: ChatTransportSendMessagesOptions,
  ): Stream.Stream<UiMessageChunk, ChatTransportError> => {
    const consumerClass = input.consumerClass ?? DEFAULT_CONSUMER;

    // When a producer is provided it runs to completion first, then the
    // transport streams via finite replay. Live mid-turn attach is the
    // reconnect path (and HARN-02 attach tests): hosts that interleave
    // production with consumption call append themselves and use
    // reconnectToStream / sendMessages without `produce`.
    return Stream.unwrap(
      Effect.gen(function* () {
        if (produce !== undefined) {
          yield* produce({
            turnId: input.turnId,
            threadId: input.threadId,
            messages: input.messages,
            log,
          });
        }
        const complete = yield* turnIsComplete(log, input.turnId);
        return complete
          ? replayChunks(log, input.turnId, -1, project)
          : attachChunks(log, input.turnId, -1, consumerClass, project);
      }),
    );
  };

  const reconnectToStream = (
    input: ChatTransportReconnectOptions,
  ): Effect.Effect<
    Option.Option<Stream.Stream<UiMessageChunk, ChatTransportError>>,
    ChatTransportError
  > =>
    Effect.gen(function* () {
      const consumerClass = input.consumerClass ?? DEFAULT_CONSUMER;
      const last = yield* log
        .lastCursor({ turnId: input.turnId })
        .pipe(Effect.mapError(mapLogError("lastCursor", input.turnId)));

      // No events ever stored for this turn → nothing to reconnect to.
      if (last < 0) {
        return Option.none();
      }

      // Fully consumed: nothing after the renderer's cursor.
      if (input.fromCursor >= last) {
        return Option.none();
      }

      const complete = yield* turnIsComplete(log, input.turnId);
      const stream = complete
        ? replayChunks(log, input.turnId, input.fromCursor, project)
        : attachChunks(log, input.turnId, input.fromCursor, consumerClass, project);
      return Option.some(stream);
    });

  return { sendMessages, reconnectToStream };
};
