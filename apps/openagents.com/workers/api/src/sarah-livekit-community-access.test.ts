import { generateSecretKeyBytes, publicKeyFromSecret } from "@openagentsinc/sarah/nostr-identity";
import { NIP_29_PUT_USER_KIND, NIP_29_REMOVE_USER_KIND } from "@openagentsinc/sarah/community";
import { finalizeEvent } from "nostr-effect/pure";
import { describe, expect, test, vi } from "vitest";

import {
  SARAH_LIVEKIT_COMMUNITY_AUTHORITY_SCHEMA,
  makeSarahLiveKitCommunityAccessResolver,
  readSarahLiveKitCommunityEvents,
} from "./sarah-livekit-community-access";

const COMMUNITY_REF = "community-7";
const CHANNEL_REF = "channel-7";

const party = () => {
  const secretKey = generateSecretKeyBytes();
  return { secretKey, pubkey: publicKeyFromSecret(secretKey) };
};

const membershipEvent = (
  signer: ReturnType<typeof party>,
  subjectPubkey: string,
  kind: typeof NIP_29_PUT_USER_KIND | typeof NIP_29_REMOVE_USER_KIND,
  createdAt: number,
) =>
  finalizeEvent(
    {
      kind,
      created_at: createdAt,
      tags: [
        ["h", COMMUNITY_REF],
        ["p", subjectPubkey],
      ],
      content: "",
    },
    signer.secretKey,
  );

const authorityJson = (adminPubkey: string): string =>
  JSON.stringify({
    schema: SARAH_LIVEKIT_COMMUNITY_AUTHORITY_SCHEMA,
    communities: [
      {
        communityRef: COMMUNITY_REF,
        channelRefs: [CHANNEL_REF],
        relayUrl: "wss://relay.openagents.test",
        adminPubkeys: [adminPubkey],
      },
    ],
  });

describe("Sarah LiveKit community access", () => {
  test("derives active access and a stable revision from signed NIP-29 admin records", async () => {
    const admin = party();
    const member = party();
    const unrelatedMember = party();
    const events = [membershipEvent(admin, member.pubkey, NIP_29_PUT_USER_KIND, 1_000)];
    const readEvents = vi.fn(async () => events);
    const resolve = makeSarahLiveKitCommunityAccessResolver({
      authorityConfig: () => authorityJson(admin.pubkey),
      readEvents,
      resolveOwnerPubkeys: async () => [member.pubkey],
    });

    const first = await resolve(
      {},
      {
        ownerUserId: "owner-1",
        communityRef: COMMUNITY_REF,
        channelRef: CHANNEL_REF,
      },
    );
    events.push(membershipEvent(admin, unrelatedMember.pubkey, NIP_29_PUT_USER_KIND, 1_100));
    const second = await resolve(
      {},
      {
        ownerUserId: "owner-1",
        communityRef: COMMUNITY_REF,
        channelRef: CHANNEL_REF,
      },
    );

    expect(first).toEqual({
      communityRef: COMMUNITY_REF,
      channelRef: CHANNEL_REF,
      membershipRevision: expect.stringMatching(/^[0-9a-f]{64}$/u),
      publishAllowed: true,
      subscribeAllowed: true,
    });
    expect(second?.membershipRevision).toBe(first?.membershipRevision);
    expect(readEvents).toHaveBeenCalledTimes(2);
  });

  test("fails closed after a signed removal and for forged or non-admin records", async () => {
    const admin = party();
    const impostor = party();
    const member = party();
    const put = membershipEvent(admin, member.pubkey, NIP_29_PUT_USER_KIND, 1_000);
    const remove = membershipEvent(admin, member.pubkey, NIP_29_REMOVE_USER_KIND, 1_100);
    const forged = {
      ...membershipEvent(impostor, member.pubkey, NIP_29_PUT_USER_KIND, 1_200),
      id: "0".repeat(64),
    };
    const resolve = makeSarahLiveKitCommunityAccessResolver({
      authorityConfig: () => authorityJson(admin.pubkey),
      readEvents: async () => [put, remove, forged],
      resolveOwnerPubkeys: async () => [member.pubkey],
    });

    await expect(
      resolve(
        {},
        {
          ownerUserId: "owner-1",
          communityRef: COMMUNITY_REF,
          channelRef: CHANNEL_REF,
        },
      ),
    ).resolves.toBeUndefined();
  });

  test("does not query the relay without an exact configured room and linked Nostr identity", async () => {
    const admin = party();
    const readEvents = vi.fn(async () => []);
    const unconfigured = makeSarahLiveKitCommunityAccessResolver({
      authorityConfig: () => undefined,
      readEvents,
      resolveOwnerPubkeys: async () => [],
    });
    const wrongChannel = makeSarahLiveKitCommunityAccessResolver({
      authorityConfig: () => authorityJson(admin.pubkey),
      readEvents,
      resolveOwnerPubkeys: async () => [party().pubkey],
    });

    await expect(
      unconfigured(
        {},
        {
          ownerUserId: "owner-1",
          communityRef: COMMUNITY_REF,
          channelRef: CHANNEL_REF,
        },
      ),
    ).resolves.toBeUndefined();
    await expect(
      wrongChannel(
        {},
        {
          ownerUserId: "owner-1",
          communityRef: COMMUNITY_REF,
          channelRef: "channel-not-configured",
        },
      ),
    ).resolves.toBeUndefined();
    expect(readEvents).not.toHaveBeenCalled();
  });

  test("reads only verified exact-group events through an EOSE-bounded relay query", async () => {
    const admin = party();
    const member = party();
    const signed = membershipEvent(admin, member.pubkey, NIP_29_PUT_USER_KIND, 1_000);
    const listeners = new Map<string, Array<(event: { data?: unknown }) => void>>();
    const sent: Array<string> = [];
    const socket = {
      readyState: 1,
      close: vi.fn(),
      send: (data: string) => sent.push(data),
      addEventListener: (
        type: "open" | "message" | "close" | "error",
        listener: (event: { data?: unknown }) => void,
      ) => listeners.set(type, [...(listeners.get(type) ?? []), listener]),
    };
    const emit = (type: string, data?: unknown) => {
      for (const listener of listeners.get(type) ?? []) listener({ data });
    };
    const promise = readSarahLiveKitCommunityEvents(
      {
        communityRef: COMMUNITY_REF,
        channelRefs: [CHANNEL_REF],
        relayUrl: "wss://relay.openagents.test",
        adminPubkeys: [admin.pubkey],
      },
      { makeWebSocket: () => socket, timeoutMs: 1_000 },
    );

    emit("open");
    emit("message", JSON.stringify(["EVENT", "sarah-livekit-community-membership-v1", signed]));
    emit("message", JSON.stringify(["EOSE", "sarah-livekit-community-membership-v1"]));

    await expect(promise).resolves.toEqual([
      expect.objectContaining({
        id: signed.id,
        pubkey: signed.pubkey,
        kind: signed.kind,
        tags: signed.tags,
      }),
    ]);
    expect(JSON.parse(sent[0] ?? "[]")).toEqual([
      "REQ",
      "sarah-livekit-community-membership-v1",
      expect.objectContaining({ "#h": [COMMUNITY_REF] }),
    ]);
    expect(socket.close).toHaveBeenCalledWith(1_000, "membership-query-complete");
  });
});
