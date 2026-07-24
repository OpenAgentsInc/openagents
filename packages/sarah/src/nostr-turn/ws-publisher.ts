/**
 * WebSocket publisher for signed Nostr events (NIP-01 EVENT frames).
 * Works against a local Node relay (nostr-effect/relay/node) or any NIP-01 WS.
 */
import type { SarahNostrSignedEvent } from "../nostr-identity/types.ts";
import type { SarahRelayPublisher } from "./consumer.ts";

export type WsPublisherHandle = {
  readonly publish: SarahRelayPublisher;
  readonly close: () => Promise<void>;
  readonly waitOpen: () => Promise<void>;
};

/**
 * Create a publisher that opens one WebSocket and sends ["EVENT", event].
 * Uses global WebSocket (Node 24 / undici or ws polyfill in tests).
 */
export const createWebSocketRelayPublisher = (input: {
  readonly url: string;
  readonly WebSocketImpl?: typeof WebSocket;
  readonly openTimeoutMs?: number;
}): WsPublisherHandle => {
  const WS = input.WebSocketImpl ?? WebSocket;
  const openTimeoutMs = input.openTimeoutMs ?? 5_000;
  let ws: WebSocket | null = null;
  let openPromise: Promise<void> | null = null;

  const ensureOpen = (): Promise<void> => {
    if (ws !== null && ws.readyState === WS.OPEN) {
      return Promise.resolve();
    }
    if (openPromise) return openPromise;
    openPromise = new Promise<void>((resolve, reject) => {
      const socket = new WS(input.url);
      ws = socket as WebSocket;
      const timer = setTimeout(() => {
        reject(new Error(`ws_open_timeout: ${input.url}`));
      }, openTimeoutMs);
      socket.addEventListener("open", () => {
        clearTimeout(timer);
        resolve();
      });
      socket.addEventListener("error", () => {
        clearTimeout(timer);
        reject(new Error(`ws_error: ${input.url}`));
      });
    });
    return openPromise;
  };

  return {
    waitOpen: ensureOpen,
    publish: async (event: SarahNostrSignedEvent) => {
      await ensureOpen();
      if (ws === null || ws.readyState !== WS.OPEN) {
        throw new Error("ws_not_open");
      }
      ws.send(JSON.stringify(["EVENT", event]));
    },
    close: async () => {
      if (ws !== null && ws.readyState === WS.OPEN) {
        ws.close();
      }
      ws = null;
      openPromise = null;
    },
  };
};
