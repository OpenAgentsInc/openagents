import { Context, Effect, Layer } from "effect";

import {
  decodeSignedWorkroomDeliveryRequest,
  type SignedWorkroomActivity,
  type SignedWorkroomDeliveryAttempt,
  type SignedWorkroomDeliveryResult,
} from "./generated.ts";
import {
  deliverSignedWorkroomActivity,
  SIGNED_WORKROOM_DELIVERY_CAPABILITY,
  SignedWorkroomError,
  SignedWorkroomStateStore,
  validateSignedWorkroomState,
} from "./signed-workroom-authority.ts";
import { signedWorkroomNostrTemplate } from "./signed-workroom-nostr.ts";

const DEFAULT_RECEIPT_TIMEOUT_MS = 10_000;
const MAX_RELAY_FRAME_BYTES = 8_192;

type RelayEvent = Readonly<{ data?: unknown }>;
type RelayListener = (event: RelayEvent) => void;

export interface SignedWorkroomRelaySocket {
  readonly addEventListener: (
    type: "open" | "message" | "error" | "close",
    listener: RelayListener,
    options?: Readonly<{ once?: boolean }>,
  ) => void;
  readonly send: (data: string) => void;
  readonly close: () => void;
}

export type SignedWorkroomRelaySocketConstructor = new (
  relayUrl: string,
) => SignedWorkroomRelaySocket;

export interface SignedWorkroomRelayPublisherShape {
  readonly publish: (
    activity: SignedWorkroomActivity,
    relayUrl: string,
  ) => Effect.Effect<SignedWorkroomDeliveryAttempt>;
}

export class SignedWorkroomRelayPublisher extends Context.Service<
  SignedWorkroomRelayPublisher,
  SignedWorkroomRelayPublisherShape
>()("SignedWorkroomAuthority.RelayPublisher") {}

const normalizedRelayUrl = (value: string): string | null => {
  try {
    const url = new URL(value);
    if (
      url.protocol !== "wss:" ||
      url.username !== "" ||
      url.password !== "" ||
      url.search !== "" ||
      url.hash !== "" ||
      url.hostname === ""
    ) {
      return null;
    }
    return value;
  } catch {
    return null;
  }
};

const relayFrameText = (input: unknown): string | null => {
  if (typeof input === "string") {
    return Buffer.byteLength(input) <= MAX_RELAY_FRAME_BYTES ? input : null;
  }
  if (input instanceof ArrayBuffer) {
    return input.byteLength <= MAX_RELAY_FRAME_BYTES ? Buffer.from(input).toString("utf8") : null;
  }
  if (ArrayBuffer.isView(input)) {
    return input.byteLength <= MAX_RELAY_FRAME_BYTES
      ? Buffer.from(input.buffer, input.byteOffset, input.byteLength).toString("utf8")
      : null;
  }
  return null;
};

const relayEvent = (activity: SignedWorkroomActivity) => ({
  ...signedWorkroomNostrTemplate(activity),
  id: activity.nostrEventId,
  sig: activity.signature,
});

export const makeSignedWorkroomRelayPublisherLayer = (
  options?:
    | Readonly<{
        WebSocketImpl?: SignedWorkroomRelaySocketConstructor;
        now?: () => Date;
        receiptTimeoutMs?: number;
      }>
    | undefined,
): Layer.Layer<SignedWorkroomRelayPublisher> => {
  const WebSocketImpl =
    options?.WebSocketImpl ??
    (Reflect.get(globalThis, "WebSocket") as SignedWorkroomRelaySocketConstructor | undefined);
  const now = options?.now ?? (() => new Date());
  const receiptTimeoutMs = options?.receiptTimeoutMs ?? DEFAULT_RECEIPT_TIMEOUT_MS;

  return Layer.succeed(
    SignedWorkroomRelayPublisher,
    SignedWorkroomRelayPublisher.of({
      publish: (activity, relayUrl) =>
        Effect.promise(
          (signal) =>
            new Promise<SignedWorkroomDeliveryAttempt>((resolve) => {
              const attemptedAt = now().toISOString();
              const normalized = normalizedRelayUrl(relayUrl);
              if (normalized === null || WebSocketImpl === undefined) {
                resolve({
                  relayUrl,
                  outcome: "unreachable",
                  attemptedAt,
                  detail:
                    normalized === null
                      ? "configured relay URL is not canonical WSS"
                      : "WebSocket transport is unavailable",
                });
                return;
              }

              let socket: SignedWorkroomRelaySocket;
              try {
                socket = new WebSocketImpl(normalized);
              } catch {
                resolve({
                  relayUrl,
                  outcome: "unreachable",
                  attemptedAt,
                  detail: "relay connection could not start",
                });
                return;
              }

              let settled = false;
              const finish = (
                outcome: SignedWorkroomDeliveryAttempt["outcome"],
                detail: string | null,
              ) => {
                if (settled) return;
                settled = true;
                clearTimeout(timer);
                socket.close();
                resolve({ relayUrl, outcome, attemptedAt, detail });
              };
              const timer = setTimeout(
                () => finish("unreachable", "relay acknowledgement timed out"),
                receiptTimeoutMs,
              );
              signal.addEventListener(
                "abort",
                () => finish("unreachable", "relay publication interrupted"),
                { once: true },
              );
              socket.addEventListener(
                "open",
                () => {
                  try {
                    socket.send(JSON.stringify(["EVENT", relayEvent(activity)]));
                  } catch {
                    finish("unreachable", "relay event send failed");
                  }
                },
                { once: true },
              );
              socket.addEventListener("message", (message) => {
                const text = relayFrameText(message.data);
                if (text === null) return;
                let frame: unknown;
                try {
                  frame = JSON.parse(text);
                } catch {
                  return;
                }
                if (
                  !Array.isArray(frame) ||
                  frame.length < 3 ||
                  frame[0] !== "OK" ||
                  frame[1] !== activity.nostrEventId ||
                  typeof frame[2] !== "boolean"
                ) {
                  return;
                }
                finish(
                  frame[2] ? "accepted" : "rejected",
                  frame[2] ? null : "relay rejected signed Workroom event",
                );
              });
              socket.addEventListener(
                "error",
                () => finish("unreachable", "relay connection failed"),
                { once: true },
              );
              socket.addEventListener(
                "close",
                () => finish("unreachable", "relay closed before acknowledgement"),
                { once: true },
              );
            }),
        ),
    }),
  );
};

export interface PublishSignedWorkroomOutboxRequest {
  readonly idempotencyKey: string;
  readonly eventRef: string;
  readonly effectivePrincipalRef: string;
  readonly capabilityRef: string;
}

export const publishSignedWorkroomOutbox = (
  request: PublishSignedWorkroomOutboxRequest,
): Effect.Effect<
  SignedWorkroomDeliveryResult,
  SignedWorkroomError,
  SignedWorkroomStateStore | SignedWorkroomRelayPublisher
> =>
  Effect.gen(function* () {
    const store = yield* SignedWorkroomStateStore;
    const publisher = yield* SignedWorkroomRelayPublisher;
    const state = yield* store.load;
    if (state === null) {
      return yield* new SignedWorkroomError({
        reason: "outbox_not_found",
        detail: request.eventRef,
      });
    }
    yield* validateSignedWorkroomState(state);
    const record = state.ledger.outbox.find(
      (candidate) => candidate.activity.eventRef === request.eventRef,
    );
    if (record === undefined) {
      return yield* new SignedWorkroomError({
        reason: "outbox_not_found",
        detail: request.eventRef,
      });
    }
    if (
      request.capabilityRef !== SIGNED_WORKROOM_DELIVERY_CAPABILITY ||
      request.effectivePrincipalRef !== record.activity.actorRef
    ) {
      return yield* new SignedWorkroomError({
        reason: "forbidden",
        detail: "effective principal lacks the signed Workroom delivery capability",
      });
    }
    if (["accepted", "superseded", "revoked"].includes(record.state)) {
      return yield* new SignedWorkroomError({
        reason: "invalid_delivery",
        detail: `outbox record is terminal in ${record.state}`,
      });
    }
    const unresolvedRelayUrls = record.relayUrls.filter(
      (relayUrl) => !record.acceptedRelayUrls.includes(relayUrl),
    );
    if (unresolvedRelayUrls.length === 0) {
      return yield* new SignedWorkroomError({
        reason: "invalid_delivery",
        detail: "outbox record has no unresolved relay targets",
      });
    }
    const attempts = yield* Effect.forEach(
      unresolvedRelayUrls,
      (relayUrl) => publisher.publish(record.activity, relayUrl),
      { concurrency: 4 },
    );
    return yield* deliverSignedWorkroomActivity(
      decodeSignedWorkroomDeliveryRequest({
        ...request,
        expectedRevision: state.ledger.revision,
        attempts,
      }),
    );
  });
