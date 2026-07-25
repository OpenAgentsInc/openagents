import type {
  SarahNostrSignedEvent,
  SarahNostrSigner,
  SarahOwnerAuthTag,
} from "../nostr-identity/types.ts";
import { buildAttestedAuthTemplate, verifySignedEvent } from "../nostr-identity/crypto.ts";
import type { SarahRelayPublisher } from "./consumer.ts";

type RelayFrame = ReadonlyArray<unknown>;

export type SarahRelaySubscription = Readonly<{
  readonly close: () => Promise<void>;
}>;

export type WsPublisherHandle = {
  readonly publish: SarahRelayPublisher;
  readonly subscribe: (input: {
    readonly subscriptionId: string;
    readonly filters: ReadonlyArray<Readonly<Record<string, unknown>>>;
    readonly onEvent: (event: SarahNostrSignedEvent) => Promise<void> | void;
    readonly onError: (error: Error) => void;
  }) => Promise<SarahRelaySubscription>;
  readonly close: () => Promise<void>;
  readonly waitAuthenticated: () => Promise<void>;
  readonly waitOpen: () => Promise<void>;
};

type PendingReceipt = Readonly<{
  resolve: () => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}>;

type ActiveSubscription = Readonly<{
  filters: ReadonlyArray<Readonly<Record<string, unknown>>>;
  onEvent: (event: SarahNostrSignedEvent) => Promise<void> | void;
  onError: (error: Error) => void;
}>;

const relayError = (reason: string): Error => new Error(`sarah_relay: ${reason}`);

const parseFrame = (data: unknown): RelayFrame | null => {
  const text =
    typeof data === "string"
      ? data
      : data instanceof ArrayBuffer
        ? new TextDecoder().decode(data)
        : ArrayBuffer.isView(data)
          ? new TextDecoder().decode(data)
          : null;
  if (text === null) return null;
  try {
    const value = JSON.parse(text) as unknown;
    return Array.isArray(value) ? value : null;
  } catch {
    return null;
  }
};

const decodeSignedEvent = (value: unknown): SarahNostrSignedEvent | null => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return null;
  const record = value as Readonly<Record<string, unknown>>;
  const tags = record["tags"];
  if (
    typeof record["id"] !== "string" ||
    typeof record["pubkey"] !== "string" ||
    typeof record["created_at"] !== "number" ||
    !Number.isSafeInteger(record["created_at"]) ||
    typeof record["kind"] !== "number" ||
    !Number.isSafeInteger(record["kind"]) ||
    !Array.isArray(tags) ||
    !tags.every((tag) => Array.isArray(tag) && tag.every((part) => typeof part === "string")) ||
    typeof record["content"] !== "string" ||
    typeof record["sig"] !== "string"
  ) {
    return null;
  }
  const event: SarahNostrSignedEvent = {
    id: record["id"],
    pubkey: record["pubkey"],
    created_at: record["created_at"],
    kind: record["kind"],
    tags,
    content: record["content"],
    sig: record["sig"],
  };
  return verifySignedEvent(event) ? event : null;
};

/**
 * Create one authenticated NIP-01/NIP-42 relay session. The admitted relay
 * must send its AUTH challenge on connection, before protected EVENT or REQ
 * frames. A publish resolves only after an affirmative `OK` for the exact id.
 */
export const createWebSocketRelayPublisher = (input: {
  readonly url: string;
  readonly signer: SarahNostrSigner;
  readonly ownerAuthTag: SarahOwnerAuthTag;
  readonly WebSocketImpl?: typeof WebSocket;
  readonly openTimeoutMs?: number;
  readonly receiptTimeoutMs?: number;
}): WsPublisherHandle => {
  const WebSocketConstructor = input.WebSocketImpl ?? WebSocket;
  const openTimeoutMs = input.openTimeoutMs ?? 5_000;
  const receiptTimeoutMs = input.receiptTimeoutMs ?? 10_000;
  let socket: WebSocket | null = null;
  let openPromise: Promise<void> | null = null;
  let authenticatedPromise: Promise<void> | null = null;
  let resolveAuthenticated: (() => void) | null = null;
  let rejectAuthenticated: ((error: Error) => void) | null = null;
  let authEventId: string | null = null;
  let closed = false;
  const pendingReceipts = new Map<string, PendingReceipt>();
  const subscriptions = new Map<string, ActiveSubscription>();

  const rejectPending = (error: Error): void => {
    for (const receipt of pendingReceipts.values()) {
      clearTimeout(receipt.timer);
      receipt.reject(error);
    }
    pendingReceipts.clear();
    rejectAuthenticated?.(error);
    resolveAuthenticated = null;
    rejectAuthenticated = null;
  };

  const send = (frame: ReadonlyArray<unknown>): void => {
    if (socket === null || socket.readyState !== WebSocketConstructor.OPEN) {
      throw relayError("socket_not_open");
    }
    socket.send(JSON.stringify(frame));
  };

  const handleFrame = (frame: RelayFrame): void => {
    const type = frame[0];
    if (type === "AUTH" && typeof frame[1] === "string") {
      const authEvent = input.signer.signEvent(
        buildAttestedAuthTemplate({
          challenge: frame[1],
          relayUrl: input.url,
          ownerAuthTag: input.ownerAuthTag,
        }),
      );
      authEventId = authEvent.id;
      send(["AUTH", authEvent]);
      return;
    }
    if (type === "OK" && typeof frame[1] === "string" && typeof frame[2] === "boolean") {
      const eventId = frame[1];
      const accepted = frame[2];
      const detail = typeof frame[3] === "string" ? frame[3] : "relay_rejected";
      if (eventId === authEventId) {
        if (accepted) resolveAuthenticated?.();
        else rejectAuthenticated?.(relayError(`authentication_rejected:${detail}`));
        resolveAuthenticated = null;
        rejectAuthenticated = null;
        return;
      }
      const receipt = pendingReceipts.get(eventId);
      if (receipt !== undefined) {
        pendingReceipts.delete(eventId);
        clearTimeout(receipt.timer);
        if (accepted) receipt.resolve();
        else receipt.reject(relayError(`publish_rejected:${detail}`));
      }
      return;
    }
    if (
      type === "EVENT" &&
      typeof frame[1] === "string" &&
      frame[2] !== null &&
      typeof frame[2] === "object"
    ) {
      const subscription = subscriptions.get(frame[1]);
      if (subscription !== undefined) {
        const event = decodeSignedEvent(frame[2]);
        if (event === null) {
          subscription.onError(relayError("invalid_subscription_event"));
          return;
        }
        void Promise.resolve(subscription.onEvent(event)).catch((error: unknown) => {
          subscription.onError(
            error instanceof Error ? error : relayError("subscription_handler_failed"),
          );
        });
      }
      return;
    }
    if (type === "CLOSED" && typeof frame[1] === "string") {
      const subscription = subscriptions.get(frame[1]);
      subscriptions.delete(frame[1]);
      subscription?.onError(
        relayError(typeof frame[2] === "string" ? `subscription_closed:${frame[2]}` : "subscription_closed"),
      );
    }
  };

  const ensureOpen = (): Promise<void> => {
    if (closed) return Promise.reject(relayError("client_closed"));
    if (socket !== null && socket.readyState === WebSocketConstructor.OPEN) {
      return Promise.resolve();
    }
    if (openPromise !== null) return openPromise;
    openPromise = new Promise<void>((resolve, reject) => {
      const nextSocket = new WebSocketConstructor(input.url);
      socket = nextSocket as WebSocket;
      const timer = setTimeout(() => {
        nextSocket.close();
        if (socket === nextSocket) socket = null;
        openPromise = null;
        reject(relayError("open_timeout"));
      }, openTimeoutMs);
      nextSocket.addEventListener("open", () => {
        clearTimeout(timer);
        resolve();
      });
      nextSocket.addEventListener("message", (event) => {
        const frame = parseFrame(event.data);
        if (frame !== null) handleFrame(frame);
      });
      nextSocket.addEventListener("error", () => {
        clearTimeout(timer);
        const error = relayError("socket_error");
        reject(error);
        rejectPending(error);
      });
      nextSocket.addEventListener("close", () => {
        const error = relayError("socket_closed");
        rejectPending(error);
        for (const subscription of subscriptions.values()) subscription.onError(error);
        subscriptions.clear();
        socket = null;
        openPromise = null;
        authenticatedPromise = null;
        authEventId = null;
      });
    });
    return openPromise;
  };

  const waitAuthenticated = async (): Promise<void> => {
    if (authenticatedPromise === null) {
      authenticatedPromise = new Promise<void>((resolve, reject) => {
        const timer = setTimeout(() => {
          if (rejectAuthenticated === wrappedReject) {
            reject(relayError("authentication_timeout"));
            resolveAuthenticated = null;
            rejectAuthenticated = null;
          }
        }, receiptTimeoutMs);
        const wrappedResolve = (): void => {
          clearTimeout(timer);
          resolve();
        };
        const wrappedReject = (error: Error): void => {
          clearTimeout(timer);
          reject(error);
        };
        resolveAuthenticated = wrappedResolve;
        rejectAuthenticated = wrappedReject;
      });
    }
    await ensureOpen();
    return authenticatedPromise;
  };

  const publish = async (event: SarahNostrSignedEvent): Promise<void> => {
    await waitAuthenticated();
    if (pendingReceipts.has(event.id)) throw relayError("duplicate_pending_event");
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        pendingReceipts.delete(event.id);
        reject(relayError("publish_receipt_timeout"));
      }, receiptTimeoutMs);
      pendingReceipts.set(event.id, { resolve, reject, timer });
      try {
        send(["EVENT", event]);
      } catch (error) {
        clearTimeout(timer);
        pendingReceipts.delete(event.id);
        reject(error instanceof Error ? error : relayError("publish_failed"));
      }
    });
  };

  return {
    waitOpen: ensureOpen,
    waitAuthenticated,
    publish,
    subscribe: async ({ subscriptionId, filters, onEvent, onError }) => {
      if (subscriptions.has(subscriptionId)) {
        throw relayError("duplicate_subscription_id");
      }
      await waitAuthenticated();
      subscriptions.set(subscriptionId, { filters, onEvent, onError });
      send(["REQ", subscriptionId, ...filters]);
      return {
        close: async () => {
          if (
            subscriptions.delete(subscriptionId) &&
            socket?.readyState === WebSocketConstructor.OPEN
          ) {
            send(["CLOSE", subscriptionId]);
          }
        },
      };
    },
    close: async () => {
      closed = true;
      const error = relayError("client_closed");
      rejectPending(error);
      subscriptions.clear();
      if (socket !== null && socket.readyState < WebSocketConstructor.CLOSING) socket.close();
      socket = null;
      openPromise = null;
      authenticatedPromise = null;
    },
  };
};
