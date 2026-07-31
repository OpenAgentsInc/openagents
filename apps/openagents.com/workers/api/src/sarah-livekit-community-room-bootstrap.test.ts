import {
  SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
  SARAH_LIVEKIT_ROOM_PRINCIPAL,
  SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
  decodeSarahLiveKitRoomPresenceLease,
} from "@openagentsinc/audio-contract";
import type { SarahLiveKitRoomAuthorityStore } from "@openagentsinc/khala-sync-server";
import { createHash } from "node:crypto";
import { describe, expect, test, vi } from "vitest";

import {
  bootstrapSarahLiveKitCommunityRoom,
  type SarahLiveKitCommunityRoomBootstrapDependencies,
} from "./sarah-livekit-community-room-bootstrap.js";
import type { SarahVoiceLiveKitRoomBroker } from "./sarah-realtime-voice-routes.js";

const digest = (character: string): string => character.repeat(64);
const sha256 = (value: string): string => createHash("sha256").update(value).digest("hex");
const nowMs = 1_000_000;
const ownerUserId = "user.bootstrap";
const deviceRef = "omega-desktop-1";

const presence = decodeSarahLiveKitRoomPresenceLease({
  schema: SARAH_LIVEKIT_ROOM_AUTHORITY_SCHEMA,
  principal: SARAH_LIVEKIT_ROOM_PRINCIPAL,
  sarahPubkey: digest("a"),
  leaseRef: "presence:bootstrap:1",
  communityRef: "community-bootstrap",
  channelRef: "channel-bootstrap",
  membershipRevision: digest("b"),
  e2eeKeyRevision: digest("c"),
  roomRef: "room:bootstrap:1",
  roomEpoch: 1,
  sarahParticipantRef: SARAH_LIVEKIT_ROOM_PRINCIPAL,
  dispatchRef: "dispatch:bootstrap",
  sessionRef: "session:bootstrap",
  generation: 1,
  capabilityProfile: "community_member_v1",
  admissionDigest: digest("d"),
  processorDisclosure: SARAH_LIVEKIT_ROOM_PROCESSOR_DISCLOSURE,
  cohortPolicy: "authenticated_allowlisted",
  issuedAtMs: nowMs,
  expiresAtMs: nowMs + 60_000,
});

const communityAccess = {
  communityRef: presence.communityRef,
  channelRef: presence.channelRef,
  membershipRevision: presence.membershipRevision,
  memberPubkey: digest("1"),
  role: "moderator" as const,
  publishAllowed: true,
  subscribeAllowed: true,
};

const bootstrapStore = (
  bindParticipant: SarahLiveKitRoomAuthorityStore["bindParticipant"],
): SarahLiveKitRoomAuthorityStore => ({
  claimCommunityRoomRendezvous: vi.fn(async () => ({
    claimed: true,
    presenceLeaseRef: presence.leaseRef,
  })),
  readActiveCommunityRoomRendezvous: vi.fn(),
  retireCommunityRoomRendezvous: vi.fn(),
  listActiveCommunityRoomParticipants: vi.fn(),
  readCommunityRoomBinding: vi.fn(async () => ({
    ownerUserId,
    sessionRef: presence.sessionRef,
    generation: presence.generation,
    admissionDigest: presence.admissionDigest,
    communityRef: presence.communityRef,
    channelRef: presence.channelRef,
    membershipRevision: presence.membershipRevision,
    roomRef: presence.roomRef,
    roomEpoch: presence.roomEpoch,
    participantRef: "owner-participant-bootstrap",
    sarahParticipantRef: presence.sarahParticipantRef,
    dispatchRef: presence.dispatchRef,
    participantGrantDigest: digest("5"),
    joinExpiresAt: new Date(nowMs + 50_000).toISOString(),
    sessionExpiresAt: new Date(nowMs + 60_000).toISOString(),
  })),
  create: vi.fn(async (snapshot) => snapshot),
  read: vi.fn(async () => undefined),
  readParticipantBinding: vi.fn(async () => undefined),
  bindParticipant,
  removeParticipant: vi.fn(),
  retireRoomMembers: vi.fn(async () => 0),
  retireExpiredRoomMembers: vi.fn(async () => 0),
  compareAndSwap: vi.fn(),
});

const bootstrapDependencies = (
  store: SarahLiveKitRoomAuthorityStore,
): SarahLiveKitCommunityRoomBootstrapDependencies<Record<string, never>> => ({
  openStore: vi.fn(async () => ({ store, close: async () => undefined })),
  broker: () =>
    ({
      workerControlTokenDigest: vi.fn(() => digest("4")),
      sessionTicket: vi.fn(() => "session-ticket"),
      provision: vi.fn(),
      cleanup: vi.fn(),
      cleanupByIdempotencyKey: vi.fn(),
      cleanupRoom: vi.fn(),
      grantParticipant: vi.fn(async () => ({
        participantGrant: "server-issued-livekit-grant",
        joinExpiresAtMs: nowMs + 50_000,
      })),
    }) satisfies SarahVoiceLiveKitRoomBroker,
  liveKitUrl: () => "wss://livekit.example.com",
  sarahPubkey: () => digest("a"),
  e2eeKeyRevision: () => digest("c"),
  stopWorker: vi.fn(),
  now: () => nowMs,
});

describe("Sarah LiveKit community room bootstrap", () => {
  // The regression this file exists for. `94d49d8bab` added a
  // `device_ref_required` gate to the summon handler; its own tests exercised
  // the handler and stayed green while this caller — the only production caller
  // — could no longer satisfy the contract, and every community session 503'd.
  // A caller test is the only shape of test that can catch that.
  test("satisfies the summon handler's device-ref contract", async () => {
    const bindParticipant = vi.fn(async () => undefined);

    await expect(
      bootstrapSarahLiveKitCommunityRoom(
        bootstrapDependencies(bootstrapStore(bindParticipant)),
        {},
        {
          ownerUserId,
          deviceRef,
          presenceLeaseRef: presence.leaseRef,
          communityAccess,
        },
      ),
    ).resolves.toBeUndefined();

    expect(bindParticipant).toHaveBeenCalled();
  });

  // The seat is bound under the member's real device, not a bootstrap-flavoured
  // stand-in. Binding any other value would make the member's own later join a
  // `duplicate_participant` against a device that never existed.
  test("binds the seat to the client device the session already authenticated", async () => {
    const bound: Array<
      Parameters<SarahLiveKitRoomAuthorityStore["bindParticipant"]>[0]
    > = [];

    await bootstrapSarahLiveKitCommunityRoom(
      bootstrapDependencies(
        bootstrapStore(async (input) => {
          bound.push(input);
        }),
      ),
      {},
      {
        ownerUserId,
        deviceRef,
        presenceLeaseRef: presence.leaseRef,
        communityAccess,
      },
    );

    expect(bound.length).toBeGreaterThan(0);
    for (const input of bound) {
      expect(input).toMatchObject({
        ownerUserId,
        deviceRefDigest: sha256(`sarah-livekit-room-device\n${deviceRef}`),
      });
    }
  });

  // The raw device ref is a client identifier: the authority stores digests and
  // must not leak it back through a bootstrap failure message.
  test("keeps the raw device ref out of the failure it reports", async () => {
    const store = bootstrapStore(vi.fn(async () => undefined));
    const dependencies = {
      ...bootstrapDependencies(store),
      liveKitUrl: () => undefined,
    };

    const failure = await bootstrapSarahLiveKitCommunityRoom(dependencies, {}, {
      ownerUserId,
      deviceRef,
      presenceLeaseRef: presence.leaseRef,
      communityAccess,
    }).catch((error: unknown) => error);

    expect(failure).toBeInstanceOf(Error);
    expect((failure as Error).message).toContain("shared_room_grant_unavailable");
    expect((failure as Error).message).not.toContain(deviceRef);
  });

  // A membership the room authority does not recognise must not be able to open
  // a room, bootstrap or not.
  test("refuses to open a room for a community the access does not name", async () => {
    const bindParticipant = vi.fn(async () => undefined);

    const failure = await bootstrapSarahLiveKitCommunityRoom(
      bootstrapDependencies(bootstrapStore(bindParticipant)),
      {},
      {
        ownerUserId,
        deviceRef,
        presenceLeaseRef: presence.leaseRef,
        communityAccess: { ...communityAccess, channelRef: "channel-not-bound" },
      },
    ).catch((error: unknown) => error);

    expect(failure).toBeInstanceOf(Error);
    expect((failure as Error).message).toContain("community_membership_required");
    expect(bindParticipant).not.toHaveBeenCalled();
  });
});
