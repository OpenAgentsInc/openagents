import {
  SARAH_SIGNING_RESPONSE_SCHEMA,
  buildSarahSigningTemplate,
  type SarahSignerTemplate,
} from "@openagentsinc/sarah/nostr-signing-boundary";
import { generateSarahNostrSigner } from "@openagentsinc/sarah/nostr-identity";
import { EventEmitter } from "node:events";
import { describe, expect, test } from "vite-plus/test";
import WebSocket from "ws";
import {
  SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
  SARAH_LIVEKIT_ROOM_PRINCIPAL,
  SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
  decodeSarahLiveKitRoomPresenceLease,
} from "@openagentsinc/audio-contract";

import {
  makeSarahNostrProjectionClient,
  readSarahNostrProjectionConfig,
  sarahPresenceTemplateFromLease,
} from "./nostr-projection-client.js";

const token = `header.${"a".repeat(120)}.signature`;
const signer = generateSarahNostrSigner();
const config = {
  signerUrl: "https://oa-sarah-nostr-signer-abc-uc.a.run.app",
  signerAudience: "https://oa-sarah-nostr-signer-abc-uc.a.run.app",
  expectedPubkey: signer.getPublicKey(),
  relayUrl: "wss://relay.openagents.com",
} as const;
const template: SarahSignerTemplate = {
  type: "kind9_projection",
  createdAt: 1_800_000_000,
  groupRef: "openagents-public",
  channelRef: "agent-chat",
  messageRef: "message:one",
  presenceLeaseRef: "presence:one",
  generation: 4,
  content: "Sarah is in the room.",
};

class RelaySocket extends EventEmitter {
  static readonly OPEN = 1;
  readonly readyState = RelaySocket.OPEN;
  sent: string[] = [];

  constructor(readonly url: string) {
    super();
    queueMicrotask(() => {
      this.emit("open");
      this.emit("message", Buffer.from(JSON.stringify(["AUTH", "relay-challenge"])));
    });
  }

  send(value: string) {
    this.sent.push(value);
    const frame = JSON.parse(value) as [string, { id: string }];
    queueMicrotask(() =>
      this.emit("message", Buffer.from(JSON.stringify(["OK", frame[1].id, true, ""]))),
    );
  }

  close() {}
  override once(event: string, listener: (...arguments_: unknown[]) => void) {
    return super.once(event, listener);
  }
  override on(event: string, listener: (...arguments_: unknown[]) => void) {
    return super.on(event, listener);
  }
}

describe("Sarah worker Nostr projection client", () => {
  test("derives active and inactive replacements from the exact persisted lease", () => {
    const lease = decodeSarahLiveKitRoomPresenceLease({
      schema: SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
      principal: SARAH_LIVEKIT_ROOM_PRINCIPAL,
      sarahPubkey: signer.getPublicKey(),
      leaseRef: "presence:one",
      communityRef: "openagents-public",
      channelRef: "agent-chat",
      membershipRevision: "b".repeat(64),
      e2eeKeyRevision: "c".repeat(64),
      roomRef: "room:one",
      roomEpoch: 2,
      sarahParticipantRef: SARAH_LIVEKIT_ROOM_PRINCIPAL,
      dispatchRef: "dispatch:one",
      sessionRef: "session:one",
      generation: 4,
      capabilityProfile: "community_member_v1",
      admissionDigest: "d".repeat(64),
      processorDisclosure: SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
      cohortPolicy: "authenticated_allowlisted",
      issuedAtMs: 1_800_000_000_000,
      expiresAtMs: 1_800_000_900_000,
    });
    const active = sarahPresenceTemplateFromLease(lease, "active");
    const inactive = sarahPresenceTemplateFromLease(lease, "inactive", 1_800_000_100);
    expect(active).toMatchObject({
      status: "active",
      createdAt: 1_800_000_000,
      expiresAt: 1_800_000_900,
      presenceLeaseRef: lease.leaseRef,
    });
    expect(inactive).toMatchObject({
      status: "inactive",
      createdAt: 1_800_000_100,
      expiresAt: 1_800_000_100,
      presenceLeaseRef: lease.leaseRef,
      authorityDigest: active.authorityDigest,
    });
  });

  test("requires closed production endpoints and a stable public key", () => {
    expect(
      readSarahNostrProjectionConfig({
        SARAH_NOSTR_SIGNER_URL: config.signerUrl,
        SARAH_NOSTR_SIGNER_AUDIENCE: config.signerAudience,
        SARAH_NOSTR_EXPECTED_PUBKEY: config.expectedPubkey,
        SARAH_NOSTR_RELAY_URL: config.relayUrl,
      }),
    ).toEqual(config);
    expect(() =>
      readSarahNostrProjectionConfig({
        SARAH_NOSTR_SIGNER_URL: "http://signer.internal",
        SARAH_NOSTR_SIGNER_AUDIENCE: config.signerAudience,
        SARAH_NOSTR_EXPECTED_PUBKEY: config.expectedPubkey,
        SARAH_NOSTR_RELAY_URL: config.relayUrl,
      }),
    ).toThrow(/https/);
  });

  test("uses a cached Workload Identity token and reconstructs the exact signed event", async () => {
    const requests: Array<
      Readonly<{ url: string; authorization: string | undefined; body: string | undefined }>
    > = [];
    const fetchImpl: typeof fetch = async (input, init) => {
      const url = String(input);
      requests.push({
        url,
        authorization: new Headers(init?.headers).get("authorization") ?? undefined,
        body: typeof init?.body === "string" ? init.body : undefined,
      });
      if (url.startsWith("http://metadata.google.internal/")) {
        expect(new Headers(init?.headers).get("metadata-flavor")).toBe("Google");
        expect(new URL(url).searchParams.get("audience")).toBe(config.signerAudience);
        return new Response(token);
      }
      const body = JSON.parse(String(init?.body)) as { template: SarahSignerTemplate };
      const signed = signer.signEvent(buildSarahSigningTemplate(body.template));
      return Response.json({
        schemaVersion: SARAH_SIGNING_RESPONSE_SCHEMA,
        eventId: signed.id,
        pubkey: signed.pubkey,
        signature: signed.sig,
      });
    };
    const client = makeSarahNostrProjectionClient({
      config,
      fetch: fetchImpl,
      WebSocketImpl: RelaySocket as unknown as typeof WebSocket,
    });

    const first = await client.signAndPublish(template);
    const second = await client.sign(template);
    expect(first.id).toBe(second.id);
    expect(first.content).toBe(template.content);
    expect(first.tags).toContainEqual(["authority", "projection_only"]);
    expect(
      requests.filter((request) => request.url.includes("metadata.google.internal")),
    ).toHaveLength(1);
    expect(requests.filter((request) => request.authorization === `Bearer ${token}`)).toHaveLength(
      3,
    );
    expect(requests.map((request) => request.body ?? "").join("")).not.toContain(token);
  });

  test("rejects a valid signature from any key except the configured Sarah identity", async () => {
    const otherSigner = generateSarahNostrSigner();
    const fetchImpl: typeof fetch = async (input) => {
      if (String(input).startsWith("http://metadata.google.internal/")) return new Response(token);
      const signed = otherSigner.signEvent(buildSarahSigningTemplate(template));
      return Response.json({
        schemaVersion: SARAH_SIGNING_RESPONSE_SCHEMA,
        eventId: signed.id,
        pubkey: signed.pubkey,
        signature: signed.sig,
      });
    };
    const client = makeSarahNostrProjectionClient({ config, fetch: fetchImpl });
    await expect(client.sign(template)).rejects.toThrow(/unexpected public key/);
  });
});
