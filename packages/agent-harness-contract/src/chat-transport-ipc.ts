import { Schema as S, Stream } from "effect";
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
 * STREAM-03 desktop IPC Layer helpers. Pure schema + codecs — no Electron
 * imports. The host wires these onto the existing renderer channels; the
 * bespoke `ClaudeLocalEvent` envelope stays a behavior-contract surface and
 * becomes ONE projection of the same `UiMessageChunk` stream (parallel-run,
 * not a delete).
 *
 * Channel naming follows the desktop turn IPC style
 * (`openagents:turn:*` in `desktop-turn-ipc.ts`).
 */

/** Main → renderer push of one UI chunk for an active turn. */
export const DesktopChatTransportChunkChannel = "openagents:chat-transport:chunk" as const;

/** Renderer → main: start a turn through ChatTransport.sendMessages. */
export const DesktopChatTransportSendChannel = "openagents:chat-transport:send" as const;

/** Renderer → main: reconnect at a durable cursor. */
export const DesktopChatTransportReconnectChannel = "openagents:chat-transport:reconnect" as const;

/** Pushed frame: one chunk plus the turn it belongs to. */
export const DesktopChatTransportChunkFrame = S.Struct({
  turnId: S.NonEmptyString,
  chunk: S.Unknown, // validated via decodeUiMessageChunk on both ends
});
export interface DesktopChatTransportChunkFrame extends S.Schema.Type<
  typeof DesktopChatTransportChunkFrame
> {}

export const encodeDesktopChunkFrame = (
  turnId: string,
  chunk: UiMessageChunk,
): DesktopChatTransportChunkFrame => ({
  turnId,
  chunk: encodeUiMessageChunk(chunk),
});

export const decodeDesktopChunkFrame = (
  value: unknown,
): { readonly turnId: string; readonly chunk: UiMessageChunk } => {
  const frame = S.decodeUnknownSync(DesktopChatTransportChunkFrame)(value);
  return {
    turnId: frame.turnId,
    chunk: decodeUiMessageChunk(frame.chunk),
  };
};

/**
 * Project a chunk stream into IPC frames for a fixed turn. The host pushes
 * each frame over `DesktopChatTransportChunkChannel`.
 */
export const uiChunkStreamToIpcFrames = (
  turnId: string,
  chunks: Stream.Stream<UiMessageChunk, ChatTransportError>,
): Stream.Stream<DesktopChatTransportChunkFrame, ChatTransportError> =>
  chunks.pipe(Stream.map((chunk) => encodeDesktopChunkFrame(turnId, chunk)));

/**
 * Desktop ChatTransport: delegates to an underlying (event-log) transport and
 * exposes frame codecs for the Electron host. Electron wiring is host-owned.
 */
export interface DesktopIpcChatTransport extends ChatTransport {
  readonly chunkChannel: typeof DesktopChatTransportChunkChannel;
  readonly sendChannel: typeof DesktopChatTransportSendChannel;
  readonly reconnectChannel: typeof DesktopChatTransportReconnectChannel;
  readonly framesFor: (
    turnId: string,
    chunks: Stream.Stream<UiMessageChunk, ChatTransportError>,
  ) => Stream.Stream<DesktopChatTransportChunkFrame, ChatTransportError>;
}

export const makeDesktopIpcChatTransport = (inner: ChatTransport): DesktopIpcChatTransport => ({
  sendMessages: (options: ChatTransportSendMessagesOptions) => inner.sendMessages(options),
  reconnectToStream: (options: ChatTransportReconnectOptions) => inner.reconnectToStream(options),
  chunkChannel: DesktopChatTransportChunkChannel,
  sendChannel: DesktopChatTransportSendChannel,
  reconnectChannel: DesktopChatTransportReconnectChannel,
  framesFor: uiChunkStreamToIpcFrames,
});

/**
 * Inverse projection note for the desktop host: given a
 * `ClaudeLocalEvent` stream already projected to `HarnessStreamEvent`
 * (see `apps/openagents-desktop/src/harness-projection.ts`), map each event
 * through `khalaEventToUiChunks` and push the resulting IPC frames. The
 * frozen ClaudeLocalEvent envelope continues to ship on its own channel —
 * this transport does not replace it in this packet.
 */
export const DESKTOP_IPC_PARALLEL_PROJECTION_NOTE =
  "ClaudeLocalEvent remains a parallel projection; UiMessageChunk is the transport vocabulary." as const;
