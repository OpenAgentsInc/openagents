import { Option, Schema as S, Stream } from "effect";
import {
  ChatTransportError,
  type ChatTransport,
  type ChatTransportReconnectOptions,
  type ChatTransportSendMessagesOptions,
} from "./chat-transport.ts";
import {
  decodeUiMessageChunk,
  encodeUiMessageChunk,
  type UiMessageChunk,
} from "./ui-message-chunk.ts";

/**
 * STREAM-03 web Layer helpers: encode a `Stream<UiMessageChunk>` as
 * `text/event-stream` frames for Cloud Run, and decode frames back into
 * chunks. The durable transport underneath stays the harness event log —
 * this module is the wire artifact only. Google Cloud is the sole production
 * infrastructure authority (no Durable Objects).
 *
 * Wire shape (one event per chunk):
 * ```
 * data: <json UiMessageChunk>\n\n
 * ```
 * Terminal stream: a final `data: [DONE]\n\n` frame (mirrors the existing
 * Khala chat SSE convention used by `POST /api/khala/chat`).
 */

export const SSE_CONTENT_TYPE = "text/event-stream; charset=utf-8" as const;
export const SSE_DONE_PAYLOAD = "[DONE]" as const;

/** Encode one chunk as a single SSE `data:` event (UTF-8 string). */
export const encodeSseChunk = (chunk: UiMessageChunk): string => {
  const json = JSON.stringify(encodeUiMessageChunk(chunk));
  return `data: ${json}\n\n`;
};

/** Encode the terminal DONE frame. */
export const encodeSseDone = (): string => `data: ${SSE_DONE_PAYLOAD}\n\n`;

/**
 * Project a chunk stream to SSE frames, appending `[DONE]` when the stream
 * completes cleanly. Failures surface as `ChatTransportError` without a DONE
 * frame so the client can distinguish clean end from mid-stream failure.
 */
export const uiChunkStreamToSse = <E>(
  chunks: Stream.Stream<UiMessageChunk, E>,
): Stream.Stream<string, E | ChatTransportError> =>
  chunks.pipe(Stream.map(encodeSseChunk), Stream.concat(Stream.make(encodeSseDone())));

/**
 * Parse a complete SSE body (or a growing buffer of complete events) into
 * chunks. Incomplete trailing data is returned in `rest` for the next read.
 * The `[DONE]` frame yields no chunk and sets `done: true`.
 */
export const parseSseBuffer = (
  buffer: string,
): {
  readonly chunks: ReadonlyArray<UiMessageChunk>;
  readonly rest: string;
  readonly done: boolean;
} => {
  const chunks: Array<UiMessageChunk> = [];
  let done = false;
  let rest = buffer;
  // Split on blank-line event boundaries; keep an incomplete tail.
  while (true) {
    const boundary = rest.indexOf("\n\n");
    if (boundary < 0) break;
    const event = rest.slice(0, boundary);
    rest = rest.slice(boundary + 2);
    const dataLines = event
      .split("\n")
      .filter((line) => line.startsWith("data:"))
      .map((line) => line.slice(5).replace(/^ /, ""));
    if (dataLines.length === 0) continue;
    const payload = dataLines.join("\n");
    if (payload === SSE_DONE_PAYLOAD) {
      done = true;
      continue;
    }
    try {
      chunks.push(decodeUiMessageChunk(JSON.parse(payload)));
    } catch (cause) {
      throw new ChatTransportError({
        operation: "sse.decode",
        detail: "malformed UiMessageChunk SSE payload",
        cause,
      });
    }
  }
  return { chunks, rest, done };
};

/** Decode a full finite SSE string into chunks (throws on malformed JSON). */
export const decodeSseBody = (body: string): ReadonlyArray<UiMessageChunk> => {
  const { chunks, rest, done: _done } = parseSseBuffer(body);
  if (rest.trim().length > 0) {
    throw new ChatTransportError({
      operation: "sse.decode",
      detail: "incomplete SSE frame at end of body",
    });
  }
  return chunks;
};

/**
 * A thin web ChatTransport that:
 * - `sendMessages` / `reconnectToStream` delegate to an underlying transport
 *   (usually the event-log core)
 * - exposes {@link streamAsSse} for Cloud Run handlers to encode the result
 *
 * The HTTP route itself is host-owned (Cloud Run monolith); this Layer is the
 * Effect surface the route provides into.
 */
export interface WebSseChatTransport extends ChatTransport {
  /** Encode the chunk stream returned by send/reconnect as SSE frames. */
  readonly streamAsSse: (
    chunks: Stream.Stream<UiMessageChunk, ChatTransportError>,
  ) => Stream.Stream<string, ChatTransportError>;
}

export const makeWebSseChatTransport = (inner: ChatTransport): WebSseChatTransport => ({
  sendMessages: (options: ChatTransportSendMessagesOptions) => inner.sendMessages(options),
  reconnectToStream: (options: ChatTransportReconnectOptions) => inner.reconnectToStream(options),
  streamAsSse: (chunks) => uiChunkStreamToSse(chunks),
});

// Keep Schema re-export surface for wire tests (frame envelope).
export const SseFrameSchema = S.Struct({
  data: S.String,
});
export type SseFrameSchema = typeof SseFrameSchema.Type;

/** Reconnect HTTP helper: map Option.none to an empty body signal (HTTP 204). */
export const reconnectHttpStatus = (
  stream: Option.Option<Stream.Stream<UiMessageChunk, ChatTransportError>>,
): "stream" | "no_content" => (Option.isSome(stream) ? "stream" : "no_content");
