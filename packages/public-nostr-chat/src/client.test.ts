import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { Schema as S } from "effect";
import { finalizeEvent, generateSecretKey } from "nostr-effect/pure";
import { describe, expect, it, vi } from "vitest";

import {
  NostrEvent,
  PUBLIC_CHAT_GROUP_ID,
  PublicChatParityFixture,
  makePublicChatRelayClient,
} from "./index.js";

const parityFixture = S.decodeUnknownSync(PublicChatParityFixture)(
  JSON.parse(
    readFileSync(resolve(import.meta.dirname, "../fixtures/agent-chat-parity.v1.json"), "utf8"),
  ),
);

const required = <T>(value: T | undefined, label: string): T => {
  if (value === undefined) throw new Error(`Missing ${label}`);
  return value;
};

class TestSocket {
  readonly sent: string[] = [];
  readyState = 0;
  private readonly listeners = new Map<string, Array<(event: { data?: unknown }) => void>>();

  addEventListener(
    type: "open" | "message" | "close" | "error",
    listener: (event: { data?: unknown }) => void,
  ): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }
  close(): void {
    this.readyState = 3;
    this.fire("close");
  }
  send(data: string): void {
    this.sent.push(data);
  }
  fire(type: string, data?: unknown): void {
    if (type === "open") this.readyState = 1;
    for (const listener of this.listeners.get(type) ?? []) listener({ data });
  }
}

describe("public chat relay client", () => {
  it("replays with an overlap, deduplicates, reaches current at EOSE, and maps OK", async () => {
    vi.useFakeTimers();
    const socket = new TestSocket();
    const secret = generateSecretKey();
    const event = S.decodeUnknownSync(NostrEvent)(
      finalizeEvent(
        {
          content: "hello",
          created_at: 1_000,
          kind: 9,
          tags: [["h", PUBLIC_CHAT_GROUP_ID]],
        },
        secret,
      ),
    );
    const client = makePublicChatRelayClient({
      now: () => 1_000_000,
      relayUrl: "wss://relay.example",
      webSocket: () => socket,
    });
    client.connect();
    socket.fire("open");
    const request = JSON.parse(socket.sent[0]!) as unknown[];
    expect(request[0]).toBe("REQ");
    const subscription = String(request[1]);
    socket.fire("message", JSON.stringify(["EVENT", subscription, event]));
    socket.fire("message", JSON.stringify(["EVENT", subscription, event]));
    socket.fire("message", JSON.stringify(["EOSE", subscription]));
    expect(client.snapshot()).toMatchObject({
      events: [event],
      gapReason: null,
      lastCurrentAt: 1_000_000,
      state: "current",
    });

    const result = client.publish(event);
    socket.fire("message", JSON.stringify(["OK", event.id, false, "rate-limited: slow down"]));
    await expect(result).resolves.toEqual({
      reason: "rate-limited",
      state: "rejected",
    });
    client.close();
    vi.useRealTimers();
  });

  it("answers a NIP-42 challenge with the selected signer", async () => {
    const socket = new TestSocket();
    const secret = generateSecretKey();
    const client = makePublicChatRelayClient({
      relayUrl: "wss://relay.example",
      signer: {
        getPublicKey: async () => "unused",
        signEvent: async (template) =>
          S.decodeUnknownSync(NostrEvent)(finalizeEvent(template, secret)),
      },
      webSocket: () => socket,
    });
    client.connect();
    socket.fire("open");
    socket.fire("message", JSON.stringify(["AUTH", "challenge-1"]));
    await vi.waitFor(() => {
      expect(socket.sent.some((frame) => JSON.parse(frame)[0] === "AUTH")).toBe(true);
    });
    client.close();
  });

  it("matches the shared replay, EOSE, pagination, and reconnect fixture", async () => {
    vi.useFakeTimers();
    const sockets: TestSocket[] = [];
    const states: string[] = [];
    const nowMs = (parityFixture.lifecycle.latestEventCreatedAt + 100) * 1_000;
    const client = makePublicChatRelayClient({
      now: () => nowMs,
      reconnectMs: parityFixture.lifecycle.reconnectDelayMs,
      relaySelfPubkey: parityFixture.lifecycle.relaySelfPubkey,
      relayUrl: "wss://relay.example",
      webSocket: () => {
        const socket = new TestSocket();
        sockets.push(socket);
        return socket;
      },
    });
    const unsubscribe = client.subscribe((snapshot) => {
      if (states.at(-1) !== snapshot.state) states.push(snapshot.state);
    });

    client.connect();
    const first = required(sockets[0], "first socket");
    first.fire("open");
    const initialRequests = first.sent
      .map((frame) => JSON.parse(frame) as unknown[])
      .filter((frame) => frame[0] === "REQ");
    const historyRequest = required(
      initialRequests.find(
        (frame) => typeof frame[2] === "object" && frame[2] !== null && "#h" in frame[2],
      ),
      "history request",
    );
    const stateRequest = required(
      initialRequests.find(
        (frame) => typeof frame[2] === "object" && frame[2] !== null && "#d" in frame[2],
      ),
      "group-state request",
    );
    expect(historyRequest[2]).toMatchObject({
      "#h": [PUBLIC_CHAT_GROUP_ID],
      limit: parityFixture.lifecycle.historyPageSize,
    });
    expect(stateRequest[2]).toMatchObject({
      "#d": [PUBLIC_CHAT_GROUP_ID],
      authors: [parityFixture.lifecycle.relaySelfPubkey],
    });

    const media = required(
      parityFixture.projection.events.find(
        (event) => event.kind === 9 && event.tags.some((tag) => tag[0] === "imeta"),
      ),
      "media event",
    );
    const reaction = required(
      parityFixture.projection.events.find((event) => event.kind === 7),
      "reaction event",
    );
    const profile = required(
      parityFixture.projection.events.find(
        (event) =>
          event.kind === 0 && event.created_at === parityFixture.lifecycle.latestEventCreatedAt,
      ),
      "profile event",
    );
    const historyId = String(historyRequest[1]);
    first.fire("message", JSON.stringify(["EVENT", historyId, media]));
    first.fire("message", JSON.stringify(["EVENT", historyId, media]));
    first.fire("message", JSON.stringify(["EVENT", historyId, reaction]));
    const profileRequest = required(
      first.sent
        .map((frame) => JSON.parse(frame) as unknown[])
        .find(
          (frame) =>
            frame[0] === "REQ" &&
            typeof frame[1] === "string" &&
            frame[1].startsWith("agentchat-profile-"),
        ),
      "profile request",
    );
    first.fire("message", JSON.stringify(["EVENT", String(profileRequest[1]), profile]));

    client.loadOlder();
    const pageRequest = required(
      first.sent
        .map((frame) => JSON.parse(frame) as unknown[])
        .find(
          (frame) =>
            frame[0] === "REQ" &&
            typeof frame[1] === "string" &&
            frame[1].startsWith("agentchat-page-"),
        ),
      "page request",
    );
    expect(pageRequest[2]).toMatchObject({
      limit: parityFixture.lifecycle.historyPageSize,
      until: parityFixture.lifecycle.oldestAcceptedEventCreatedAt,
    });

    first.fire("message", JSON.stringify(["EOSE", historyId]));
    expect(client.snapshot().state).toBe("replaying");
    first.fire("message", JSON.stringify(["EOSE", String(stateRequest[1])]));
    expect(client.snapshot().state).toBe("current");
    expect(client.snapshot().events.filter(({ id }) => id === media.id)).toHaveLength(1);

    first.close();
    expect(client.snapshot().state).toBe("stale");
    await vi.advanceTimersByTimeAsync(parityFixture.lifecycle.reconnectDelayMs);
    const second = required(sockets[1], "reconnect socket");
    second.fire("open");
    const replayRequests = second.sent
      .map((frame) => JSON.parse(frame) as unknown[])
      .filter((frame) => frame[0] === "REQ");
    const replayHistory = required(
      replayRequests.find(
        (frame) => typeof frame[2] === "object" && frame[2] !== null && "#h" in frame[2],
      ),
      "replay history request",
    );
    const replayState = required(
      replayRequests.find(
        (frame) => typeof frame[2] === "object" && frame[2] !== null && "#d" in frame[2],
      ),
      "replay group-state request",
    );
    expect(replayHistory[2]).toMatchObject({
      since:
        parityFixture.lifecycle.latestEventCreatedAt -
        parityFixture.lifecycle.reconnectOverlapSeconds,
    });
    second.fire("message", JSON.stringify(["EOSE", String(replayHistory[1])]));
    expect(client.snapshot().state).toBe("replaying");
    second.fire("message", JSON.stringify(["EOSE", String(replayState[1])]));
    expect(states).toEqual(parityFixture.lifecycle.expectedStateSequence);

    unsubscribe();
    client.close();
    vi.useRealTimers();
  });

  it("closes without sending while the socket is connecting", () => {
    const socket = new TestSocket();
    const client = makePublicChatRelayClient({
      relayUrl: "wss://relay.example",
      webSocket: () => socket,
    });

    client.connect();
    expect(() => client.close()).not.toThrow();
    expect(socket.sent).toEqual([]);
    expect(client.snapshot().state).toBe("disconnected");
  });
});
