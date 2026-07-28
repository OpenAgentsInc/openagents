import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { Schema as S } from "effect";
import { describe, expect, it } from "vitest";

import {
  NostrEvent,
  PUBLIC_CHANNEL_DESCRIPTOR_SCHEMA,
  PUBLIC_CHANNEL_REGISTRY_SCHEMA,
  PublicChannelRegistry,
  agentChatChannelDescriptor,
  assessPublicChannelRelayIdentity,
  decodePublicChannelDescriptor,
  decodePublicChannelRegistry,
  makePublicChannelRelayClient,
  normalizePublicChannelRelayUrl,
  publicChannelIdentityUpdateNeedsReview,
  publicChannelSnapshotKey,
  publicNostrChatManifest,
  toPublicChannelSnapshot,
} from "./index.js";

const registryFixture = JSON.parse(
  readFileSync(resolve(import.meta.dirname, "../fixtures/public-channel-registry.v1.json"), "utf8"),
);

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
  }
  send(data: string): void {
    this.sent.push(data);
  }
  fire(type: string, data?: unknown): void {
    if (type === "open") this.readyState = 1;
    for (const listener of this.listeners.get(type) ?? []) listener({ data });
  }
}

describe("public channel registry", () => {
  it("decodes the deterministic two-channel Rust fixture", () => {
    const registry = decodePublicChannelRegistry(registryFixture);
    expect(S.is(PublicChannelRegistry)(registry)).toBe(true);
    expect(registry.schemaVersion).toBe(PUBLIC_CHANNEL_REGISTRY_SCHEMA);
    expect(registry.channels.map((channel) => channel.channelId)).toEqual([
      "agent-chat",
      "agent-lab",
    ]);
    expect(registry.channels[0]!.groupId).toBe(registry.channels[1]!.groupId);
    expect(registry.channels[0]!.relayUrl).not.toBe(registry.channels[1]!.relayUrl);
  });

  it("adapts the existing one-channel manifest without changing its contract", () => {
    const relaySelf = "c".repeat(64);
    const manifest = publicNostrChatManifest(relaySelf);
    const descriptor = agentChatChannelDescriptor(manifest);
    expect(descriptor).toMatchObject({
      acceptedKinds: [...manifest.acceptedKinds].sort((a, b) => a - b),
      channelId: "agent-chat",
      expectedRelaySelfPubkey: relaySelf,
      groupId: manifest.group.id,
      relayTrust: "pinned",
      relayUrl: manifest.relay.websocketUrl,
      schemaVersion: PUBLIC_CHANNEL_DESCRIPTOR_SCHEMA,
    });
    expect(publicNostrChatManifest()).toMatchObject({
      readiness: "relay-self-required",
      relay: { selfPubkey: null },
    });
  });

  it("normalizes relay URLs and rejects duplicate normalized coordinates", () => {
    expect(normalizePublicChannelRelayUrl("wss://EXAMPLE.com:443/a/../b/")).toBe(
      "wss://example.com/b",
    );
    const duplicate = structuredClone(registryFixture);
    duplicate.channels[1].relayUrl = "wss://RELAY.OPENAGENTS.COM:443/";
    duplicate.channels[1].groupId = duplicate.channels[0].groupId;
    expect(() => decodePublicChannelRegistry(duplicate)).toThrow(/noncanonical relay URL/);
    duplicate.channels[1].relayUrl = "wss://relay.openagents.com";
    expect(() => decodePublicChannelRegistry(duplicate)).toThrow(/coordinate .* is not unique/);
  });

  it("rejects duplicate IDs, malformed kind sets, secrets as fields, and empty registries", () => {
    const duplicateId = structuredClone(registryFixture);
    duplicateId.channels[1].channelId = "agent-chat";
    expect(() => decodePublicChannelRegistry(duplicateId)).toThrow(/channel ID .* is not unique/);

    const kinds = structuredClone(registryFixture.channels[0]);
    kinds.acceptedKinds = [9, 7, 7];
    expect(() => decodePublicChannelDescriptor(kinds)).toThrow(/invalid acceptedKinds/);
    kinds.acceptedKinds = [9, 65_535];
    expect(() => decodePublicChannelDescriptor(kinds)).toThrow(/unsupported acceptedKinds/);

    const extra = structuredClone(registryFixture);
    extra.privateKey = "do-not-accept";
    expect(() => decodePublicChannelRegistry(extra)).toThrow();
    expect(() =>
      decodePublicChannelRegistry({
        schemaVersion: PUBLIC_CHANNEL_REGISTRY_SCHEMA,
        channels: [],
      }),
    ).toThrow(/between 1 and 64/);
  });
});

describe("public channel relay identity", () => {
  it("distinguishes verified, untrusted metadata, and key changes", () => {
    const first = "a".repeat(64);
    const second = "b".repeat(64);
    expect(assessPublicChannelRelayIdentity(first, first)).toMatchObject({
      groupStateTrusted: true,
      reconnectAllowed: true,
      status: "verified",
    });
    expect(assessPublicChannelRelayIdentity(null, second)).toMatchObject({
      groupStateTrusted: false,
      reconnectAllowed: true,
      status: "metadata-untrusted",
    });
    expect(assessPublicChannelRelayIdentity(first, null)).toMatchObject({
      groupStateTrusted: false,
      reconnectAllowed: true,
      status: "metadata-untrusted",
    });
    expect(assessPublicChannelRelayIdentity(first, second)).toMatchObject({
      groupStateTrusted: false,
      reconnectAllowed: false,
      status: "key-change-review",
    });
    expect(publicChannelIdentityUpdateNeedsReview(null, second)).toBe(true);
    expect(publicChannelIdentityUpdateNeedsReview(first, first)).toBe(false);
  });

  it("uses descriptor kinds and limits in its read-only relay request", () => {
    const descriptor = decodePublicChannelDescriptor({
      ...structuredClone(registryFixture.channels[0]),
      acceptedKinds: [9],
      groupStateKinds: [39000],
      limits: {
        ...registryFixture.channels[0].limits,
        historyPageSize: 17,
      },
      moderationKinds: [9005],
    });
    const socket = new TestSocket();
    const result = makePublicChannelRelayClient({
      descriptor,
      observedRelaySelfPubkey: null,
      webSocket: () => socket,
    });
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect("publish" in result.client).toBe(false);
    result.client.connect();
    socket.fire("open");
    const request = JSON.parse(socket.sent[0]!) as [
      string,
      string,
      { kinds: number[]; limit: number },
    ];
    expect(request[2]).toMatchObject({ kinds: [9, 9005], limit: 17 });
    result.client.close();
  });

  it("blocks a relay key change before it opens a socket", () => {
    const descriptor = agentChatChannelDescriptor(publicNostrChatManifest("a".repeat(64)));
    let opened = false;
    const result = makePublicChannelRelayClient({
      descriptor,
      observedRelaySelfPubkey: "b".repeat(64),
      webSocket: () => {
        opened = true;
        return new TestSocket();
      },
    });
    expect(result).toEqual({ ok: false, reason: "key-change-review" });
    expect(opened).toBe(false);
  });
});

describe("renderer-safe public channel snapshots", () => {
  const event = S.decodeUnknownSync(NostrEvent)({
    content: "normal public message",
    created_at: 10,
    id: "1".repeat(64),
    kind: 9,
    pubkey: "2".repeat(64),
    sig: "3".repeat(128),
    tags: [["h", "openagents-public"]],
  });
  const descriptor = decodePublicChannelRegistry(registryFixture).channels[0]!;
  const identity = assessPublicChannelRelayIdentity(null, null);

  it("keeps channel coordinates, deterministic cursor data, and bounded gap codes", () => {
    const snapshot = toPublicChannelSnapshot(descriptor, identity, {
      events: [event],
      gapReason: "raw relay text must not cross the bridge",
      lastCurrentAt: 20,
      state: "current",
    });
    expect(snapshot).toMatchObject({
      cursor: {
        createdAt: 10,
        eventIdsAtCreatedAt: [event.id],
      },
      gapReason: "relay-notice",
      verifiedEvents: [event],
    });
    expect(publicChannelSnapshotKey(descriptor)).toBe(
      JSON.stringify([descriptor.channelId, descriptor.relayUrl, descriptor.groupId]),
    );
  });

  it("drops a complete secret-shaped signed event without rewriting signed bytes", () => {
    const unsafe = { ...event, content: `nsec1${"q".repeat(40)}` };
    const snapshot = toPublicChannelSnapshot(descriptor, identity, {
      events: [event, unsafe],
      gapReason: null,
      lastCurrentAt: null,
      state: "replaying",
    });
    expect(snapshot.verifiedEvents).toEqual([event]);
    expect(snapshot.gapReason).toBe("unsafe-event-content");
  });
});
