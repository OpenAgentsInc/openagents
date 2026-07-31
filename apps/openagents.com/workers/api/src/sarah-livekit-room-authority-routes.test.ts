import {
  SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
  SARAH_LIVEKIT_ROOM_PRINCIPAL,
  SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
  decodeSarahLiveKitRoomPresenceLease,
  type SarahLiveKitRoomAuthoritySnapshot,
} from "@openagentsinc/audio-contract";
import type { SarahLiveKitRoomAuthorityStore } from "@openagentsinc/khala-sync-server";
import { describe, expect, test, vi } from "vitest";

import {
  handleSarahLiveKitRoomMemberFloorRequest,
  handleSarahLiveKitRoomModeratorFloorRequest,
  type SarahLiveKitRoomAuthorityRouteDependencies,
} from "./sarah-livekit-room-authority-routes.js";
import {
  initialSarahLiveKitRoomAuthoritySnapshot,
  type SarahLiveKitRoomMemberAccess,
} from "./sarah-livekit-room-authority.js";

const digest = (character: string): string => character.repeat(64);
const issuedAtMs = 1_000_000;
const presence = decodeSarahLiveKitRoomPresenceLease({
  schema: SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
  principal: SARAH_LIVEKIT_ROOM_PRINCIPAL,
  sarahPubkey: digest("a"),
  leaseRef: "presence:community-one:1",
  communityRef: "community-one",
  channelRef: "agent-chat",
  membershipRevision: digest("b"),
  e2eeKeyRevision: digest("c"),
  roomRef: "room:community-one:1",
  roomEpoch: 1,
  sarahParticipantRef: SARAH_LIVEKIT_ROOM_PRINCIPAL,
  dispatchRef: "dispatch:one",
  sessionRef: "session:one",
  generation: 1,
  capabilityProfile: "community_member_v1",
  admissionDigest: digest("d"),
  processorDisclosure: SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
  cohortPolicy: "authenticated_allowlisted",
  issuedAtMs,
  expiresAtMs: issuedAtMs + 60_000,
});

const member = (role: "member" | "moderator" = "member"): SarahLiveKitRoomMemberAccess => ({
  authenticated: true,
  allowlisted: true,
  active: true,
  role,
  userRefDigest: digest("1"),
  pubkey: digest("2"),
  participantRef: "participant:one",
  mappedParticipantRef: "participant:one",
  membershipRevision: presence.membershipRevision,
  roomRef: presence.roomRef,
  roomEpoch: presence.roomEpoch,
  safetyIdentifier: digest("3"),
});

const request = (body: unknown): Request =>
  new Request("https://api.openagents.com/api/sarah/livekit/room/floor", {
    method: "POST",
    headers: { "content-type": "application/json", authorization: "Bearer test" },
    body: JSON.stringify(body),
  });

const dependencies = (
  snapshot: SarahLiveKitRoomAuthoritySnapshot,
  resolvedMember: SarahLiveKitRoomMemberAccess | undefined,
  authenticated = true,
) => {
  const compareAndSwap = vi.fn(async (input) => input.snapshot);
  const store: SarahLiveKitRoomAuthorityStore = {
    create: vi.fn(),
    read: vi.fn(async () => snapshot),
    readParticipantBinding: vi.fn(),
    compareAndSwap,
  };
  const value: SarahLiveKitRoomAuthorityRouteDependencies<Record<string, never>> = {
    store,
    authenticate: vi.fn(async () =>
      authenticated
        ? {
            userId: "user.one",
            userRefDigest: digest("1"),
            pubkey: digest("2"),
          }
        : undefined,
    ),
    resolveMember: vi.fn(async () => resolvedMember),
    now: () => new Date(issuedAtMs + 1_000),
  };
  return { value, compareAndSwap };
};

describe("Sarah LiveKit room authority routes", () => {
  test("requires authentication and rejects client-supplied authority fields", async () => {
    const snapshot = initialSarahLiveKitRoomAuthoritySnapshot(presence);
    const unauthenticated = dependencies(snapshot, member(), false);
    const unauthenticatedResponse = await handleSarahLiveKitRoomMemberFloorRequest(
      request({
        action: "acquire",
        presenceLeaseRef: presence.leaseRef,
        expectedRevision: 1,
        nonce: "a".repeat(32),
        requestedLeaseMs: 10_000,
      }),
      {},
      unauthenticated.value,
    );
    expect(unauthenticatedResponse.status).toBe(401);

    const forged = dependencies(snapshot, member("moderator"));
    const forgedResponse = await handleSarahLiveKitRoomMemberFloorRequest(
      request({
        action: "acquire",
        presenceLeaseRef: presence.leaseRef,
        expectedRevision: 1,
        nonce: "b".repeat(32),
        requestedLeaseMs: 10_000,
        role: "moderator",
      }),
      {},
      forged.value,
    );
    expect(forgedResponse.status).toBe(400);
    expect(forged.value.authenticate).not.toHaveBeenCalled();
  });

  test("persists an acquired floor with the exact expected revision", async () => {
    const snapshot = initialSarahLiveKitRoomAuthoritySnapshot(presence);
    const route = dependencies(snapshot, member());
    const response = await handleSarahLiveKitRoomMemberFloorRequest(
      request({
        action: "acquire",
        presenceLeaseRef: presence.leaseRef,
        expectedRevision: 1,
        nonce: "c".repeat(32),
        requestedLeaseMs: 10_000,
      }),
      {},
      route.value,
    );
    expect(response.status).toBe(200);
    expect(route.compareAndSwap).toHaveBeenCalledWith(
      expect.objectContaining({
        presenceLeaseRef: presence.leaseRef,
        expectedRevision: 1,
        snapshot: expect.objectContaining({ revision: 2 }),
      }),
    );
  });

  test("takes moderator authority only from the membership resolver", async () => {
    const snapshot = initialSarahLiveKitRoomAuthoritySnapshot(presence);
    const regularMember = dependencies(snapshot, member());
    const denied = await handleSarahLiveKitRoomModeratorFloorRequest(
      request({
        action: "stop",
        presenceLeaseRef: presence.leaseRef,
        expectedRevision: 1,
        nonce: "d".repeat(32),
      }),
      {},
      regularMember.value,
    );
    expect(denied.status).toBe(403);
    expect(regularMember.compareAndSwap).not.toHaveBeenCalled();

    const moderator = dependencies(snapshot, member("moderator"));
    const accepted = await handleSarahLiveKitRoomModeratorFloorRequest(
      request({
        action: "stop",
        presenceLeaseRef: presence.leaseRef,
        expectedRevision: 1,
        nonce: "e".repeat(32),
      }),
      {},
      moderator.value,
    );
    expect(accepted.status).toBe(200);
  });

  test("rejects stale client revisions before mutation", async () => {
    const snapshot = initialSarahLiveKitRoomAuthoritySnapshot(presence);
    const route = dependencies(snapshot, member());
    const response = await handleSarahLiveKitRoomMemberFloorRequest(
      request({
        action: "acquire",
        presenceLeaseRef: presence.leaseRef,
        expectedRevision: 2,
        nonce: "f".repeat(32),
        requestedLeaseMs: 10_000,
      }),
      {},
      route.value,
    );
    expect(response.status).toBe(409);
    expect(route.compareAndSwap).not.toHaveBeenCalled();
  });
});
