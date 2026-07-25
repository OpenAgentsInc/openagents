import { describe, expect, it } from "vite-plus/test";
import WebSocket, { WebSocketServer } from "ws";

import {
  generateSarahNostrSigner,
  generateSecretKeyBytes,
  signOwnerAuthTag,
  verifySignedEvent,
} from "../nostr-identity/index.ts";
import { createWebSocketRelayPublisher } from "./ws-publisher.ts";

const WebSocketImpl = WebSocket as unknown as typeof globalThis.WebSocket;

const listen = async (
  onConnection: (socket: WebSocket) => void,
): Promise<Readonly<{ url: string; close: () => Promise<void> }>> => {
  const server = new WebSocketServer({ host: "127.0.0.1", port: 0 });
  await new Promise<void>((resolve, reject) => {
    server.once("listening", resolve);
    server.once("error", reject);
  });
  server.on("connection", onConnection);
  const address = server.address();
  if (address === null) throw new Error("relay did not bind");
  if (typeof address === "string") throw new Error("expected TCP relay address");
  return {
    url: `ws://127.0.0.1:${address.port}`,
    close: () =>
      new Promise<void>((resolve, reject) => {
        server.close((error) => (error === undefined ? resolve() : reject(error)));
      }),
  };
};

describe("authenticated WebSocket relay publisher", () => {
  it("authenticates on the connect challenge and awaits exact positive OK receipts", async () => {
    const signer = generateSarahNostrSigner();
    const ownerAuthTag = signOwnerAuthTag({
      agentPubkey: signer.getPublicKey(),
      conditions: "owner-private Sarah relay",
      ownerSeckeyHex: Buffer.from(generateSecretKeyBytes()).toString("hex"),
    });
    const receivedKinds: number[] = [];
    const subscribedEvent = signer.signEvent({
      kind: 1059,
      created_at: 1_700_000_000,
      tags: [["p", signer.getPublicKey()]],
      content: "encrypted gift wrap",
    });
    const relay = await listen((socket) => {
      socket.send(JSON.stringify(["AUTH", "challenge-on-connect"]));
      socket.on("message", (bytes) => {
        const frame = JSON.parse(bytes.toString()) as ReadonlyArray<unknown>;
        if (frame[0] === "REQ") {
          socket.send(JSON.stringify(["EVENT", frame[1], subscribedEvent]));
          return;
        }
        const event = frame[1] as { readonly id: string; readonly kind: number };
        receivedKinds.push(event.kind);
        socket.send(JSON.stringify(["OK", event.id, true, "accepted"]));
      });
    });
    const client = createWebSocketRelayPublisher({
      url: relay.url,
      signer,
      ownerAuthTag,
      WebSocketImpl,
    });
    try {
      await client.waitAuthenticated();
      const received = new Promise<string>((resolve, reject) => {
        void client
          .subscribe({
            subscriptionId: "owner-private",
            filters: [{ kinds: [1059], "#p": [signer.getPublicKey()] }],
            onEvent: (event) => resolve(event.id),
            onError: reject,
          })
          .catch(reject);
      });
      await expect(received).resolves.toBe(subscribedEvent.id);
      const event = signer.signEvent({
        kind: 44300,
        created_at: 1_700_000_000,
        tags: [],
        content: "encrypted",
      });
      await client.publish(event);
      expect(receivedKinds).toEqual([22242, 44300]);
      expect(verifySignedEvent(event)).toBe(true);
    } finally {
      await client.close();
      await relay.close();
    }
  });

  it("rejects a negative relay OK instead of treating send as success", async () => {
    const signer = generateSarahNostrSigner();
    const ownerAuthTag = signOwnerAuthTag({
      agentPubkey: signer.getPublicKey(),
      conditions: "owner-private Sarah relay",
      ownerSeckeyHex: Buffer.from(generateSecretKeyBytes()).toString("hex"),
    });
    const relay = await listen((socket) => {
      socket.send(JSON.stringify(["AUTH", "challenge-on-connect"]));
      socket.on("message", (bytes) => {
        const frame = JSON.parse(bytes.toString()) as ReadonlyArray<unknown>;
        const event = frame[1] as { readonly id: string; readonly kind: number };
        socket.send(
          JSON.stringify([
            "OK",
            event.id,
            event.kind === 22242,
            event.kind === 22242 ? "accepted" : "blocked: policy",
          ]),
        );
      });
    });
    const client = createWebSocketRelayPublisher({
      url: relay.url,
      signer,
      ownerAuthTag,
      WebSocketImpl,
    });
    try {
      await expect(
        client.publish(
          signer.signEvent({ kind: 44300, created_at: 1, tags: [], content: "encrypted" }),
        ),
      ).rejects.toThrow(/publish_rejected/);
    } finally {
      await client.close();
      await relay.close();
    }
  });

  it("fails closed when the admitted relay does not challenge on connect", async () => {
    const signer = generateSarahNostrSigner();
    const ownerAuthTag = signOwnerAuthTag({
      agentPubkey: signer.getPublicKey(),
      conditions: "owner-private Sarah relay",
      ownerSeckeyHex: Buffer.from(generateSecretKeyBytes()).toString("hex"),
    });
    const relay = await listen(() => undefined);
    const client = createWebSocketRelayPublisher({
      url: relay.url,
      signer,
      ownerAuthTag,
      WebSocketImpl,
      receiptTimeoutMs: 20,
    });
    try {
      await expect(client.waitAuthenticated()).rejects.toThrow(/authentication_timeout/);
    } finally {
      await client.close();
      await relay.close();
    }
  });
});
