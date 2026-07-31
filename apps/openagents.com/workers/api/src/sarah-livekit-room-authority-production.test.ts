import {
  SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
  SARAH_LIVEKIT_ROOM_PRINCIPAL,
  SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
  decodeSarahLiveKitRoomPresenceLease,
} from "@openagentsinc/audio-contract";
import type { SarahLiveKitRoomAuthorityStore } from "@openagentsinc/khala-sync-server";
import { describe, expect, test, vi } from "vitest";

import {
  handleSarahLiveKitRoomAuthorityProductionRequest,
  type SarahLiveKitRoomAuthorityProductionDependencies,
} from "./sarah-livekit-room-authority-production.js";
import { initialSarahLiveKitRoomAuthoritySnapshot } from "./sarah-livekit-room-authority.js";

const digest = (character: string): string => character.repeat(64);
const nowMs = 1_000_000;
const presence = decodeSarahLiveKitRoomPresenceLease({
  schema: SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
  principal: SARAH_LIVEKIT_ROOM_PRINCIPAL,
  sarahPubkey: digest("a"),
  leaseRef: "presence:production:1",
  communityRef: "community-production",
  channelRef: "channel-production",
  membershipRevision: digest("b"),
  e2eeKeyRevision: digest("c"),
  roomRef: "room:production:1",
  roomEpoch: 1,
  sarahParticipantRef: SARAH_LIVEKIT_ROOM_PRINCIPAL,
  dispatchRef: "dispatch:production",
  sessionRef: "session:production",
  generation: 1,
  capabilityProfile: "community_member_v1",
  admissionDigest: digest("d"),
  processorDisclosure: SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
  cohortPolicy: "authenticated_allowlisted",
  issuedAtMs: nowMs,
  expiresAtMs: nowMs + 60_000,
});

const request = (path: "member" | "moderator"): Request =>
  new Request(`https://api.openagents.com/${path}`, {
    method: "POST",
    headers: {
      authorization: "Bearer verified",
      "content-type": "application/json",
    },
    body: JSON.stringify(
      path === "member"
        ? {
            action: "acquire",
            presenceLeaseRef: presence.leaseRef,
            expectedRevision: 1,
            nonce: "a".repeat(32),
            requestedLeaseMs: 10_000,
          }
        : {
            action: "stop",
            presenceLeaseRef: presence.leaseRef,
            expectedRevision: 1,
            nonce: "b".repeat(32),
          },
    ),
  });

const setup = (
  options: Readonly<{
    role?: "member" | "moderator";
    authenticated?: boolean;
    membershipRevision?: string;
    openFails?: boolean;
    participantBound?: boolean;
  }> = {},
): {
  dependencies: SarahLiveKitRoomAuthorityProductionDependencies<
    Record<string, never>,
    Record<string, never>
  >;
  close: ReturnType<typeof vi.fn>;
  compareAndSwap: ReturnType<typeof vi.fn>;
  readParticipantBinding: ReturnType<typeof vi.fn>;
  openStore: ReturnType<typeof vi.fn>;
} => {
  const snapshot = initialSarahLiveKitRoomAuthoritySnapshot(presence);
  const close = vi.fn(async () => undefined);
  const compareAndSwap = vi.fn(async (input) => input.snapshot);
  const readParticipantBinding = vi.fn(async () =>
    options.participantBound === false
      ? undefined
      : {
          ownerUserId: "user.production",
          participantRef: "owner-participant-production",
          communityRef: presence.communityRef,
          channelRef: presence.channelRef,
          membershipRevision: presence.membershipRevision,
          roomRef: presence.roomRef,
          roomEpoch: presence.roomEpoch,
        },
  );
  const store: SarahLiveKitRoomAuthorityStore = {
    create: vi.fn(),
    read: vi.fn(async () => snapshot),
    readParticipantBinding,
    compareAndSwap,
  };
  const openStore = vi.fn(async () => {
    if (options.openFails) throw new Error("cloud sql unavailable");
    return { store, close };
  });
  return {
    close,
    compareAndSwap,
    readParticipantBinding,
    openStore,
    dependencies: {
      openStore,
      requireUser: vi.fn(async () =>
        options.authenticated === false ? undefined : { userId: "user.production" },
      ),
      resolveCommunityAccess: vi.fn(async () => ({
        communityRef: presence.communityRef,
        channelRef: presence.channelRef,
        membershipRevision: options.membershipRevision ?? presence.membershipRevision,
        memberPubkey: digest("1"),
        role: options.role ?? "member",
        publishAllowed: true,
        subscribeAllowed: true,
      })),
      now: () => nowMs + 1_000,
    },
  };
};

describe("Sarah LiveKit room authority production wiring", () => {
  test("authenticates before opening Cloud SQL", async () => {
    const route = setup({ authenticated: false });
    const response = await handleSarahLiveKitRoomAuthorityProductionRequest(
      route.dependencies,
      "member",
      request("member"),
      {},
      {},
    );
    expect(response.status).toBe(401);
    expect(route.openStore).not.toHaveBeenCalled();
  });

  test("binds floor authority to the durable participant and signed membership", async () => {
    const route = setup();
    const response = await handleSarahLiveKitRoomAuthorityProductionRequest(
      route.dependencies,
      "member",
      request("member"),
      {},
      {},
    );
    expect(response.status).toBe(200);
    expect(route.readParticipantBinding).toHaveBeenCalledWith({
      presenceLeaseRef: presence.leaseRef,
      ownerUserId: "user.production",
      now: new Date(nowMs + 1_000).toISOString(),
    });
    expect(route.compareAndSwap).toHaveBeenCalledWith(
      expect.objectContaining({
        expectedRevision: 1,
        snapshot: expect.objectContaining({
          floor: {
            state: "held",
            lease: expect.objectContaining({
              holderParticipantRef: "owner-participant-production",
              holderPubkey: digest("1"),
            }),
          },
        }),
      }),
    );
    expect(route.close).toHaveBeenCalledOnce();
  });

  test("fails closed on a changed signed membership revision and closes SQL", async () => {
    const route = setup({ membershipRevision: digest("9") });
    const response = await handleSarahLiveKitRoomAuthorityProductionRequest(
      route.dependencies,
      "member",
      request("member"),
      {},
      {},
    );
    expect(response.status).toBe(403);
    expect(route.compareAndSwap).not.toHaveBeenCalled();
    expect(route.close).toHaveBeenCalledOnce();
  });

  test("fails closed without an exact durable participant binding", async () => {
    const route = setup({ participantBound: false });
    const response = await handleSarahLiveKitRoomAuthorityProductionRequest(
      route.dependencies,
      "member",
      request("member"),
      {},
      {},
    );
    expect(response.status).toBe(403);
    expect(route.dependencies.resolveCommunityAccess).not.toHaveBeenCalled();
    expect(route.compareAndSwap).not.toHaveBeenCalled();
    expect(route.close).toHaveBeenCalledOnce();
  });

  test("does not resolve a client-supplied target digest into another participant", async () => {
    const route = setup();
    const transfer = new Request("https://api.openagents.com/member", {
      method: "POST",
      headers: {
        authorization: "Bearer verified",
        "content-type": "application/json",
      },
      body: JSON.stringify({
        action: "transfer",
        presenceLeaseRef: presence.leaseRef,
        expectedRevision: 1,
        nonce: "c".repeat(32),
        requestedLeaseMs: 10_000,
        targetUserRefDigest: digest("9"),
      }),
    });
    const response = await handleSarahLiveKitRoomAuthorityProductionRequest(
      route.dependencies,
      "member",
      transfer,
      {},
      {},
    );
    expect(response.status).toBe(403);
    expect(route.compareAndSwap).not.toHaveBeenCalled();
  });

  test("derives moderator power only from the signed community role", async () => {
    const memberRoute = setup({ role: "member" });
    const denied = await handleSarahLiveKitRoomAuthorityProductionRequest(
      memberRoute.dependencies,
      "moderator",
      request("moderator"),
      {},
      {},
    );
    expect(denied.status).toBe(403);
    expect(memberRoute.compareAndSwap).not.toHaveBeenCalled();

    const moderatorRoute = setup({ role: "moderator" });
    const accepted = await handleSarahLiveKitRoomAuthorityProductionRequest(
      moderatorRoute.dependencies,
      "moderator",
      request("moderator"),
      {},
      {},
    );
    expect(accepted.status).toBe(200);
    expect(moderatorRoute.compareAndSwap).toHaveBeenCalledOnce();
  });

  test("returns unavailable when storage cannot open", async () => {
    const route = setup({ openFails: true });
    const response = await handleSarahLiveKitRoomAuthorityProductionRequest(
      route.dependencies,
      "member",
      request("member"),
      {},
      {},
    );
    expect(response.status).toBe(503);
  });
});
