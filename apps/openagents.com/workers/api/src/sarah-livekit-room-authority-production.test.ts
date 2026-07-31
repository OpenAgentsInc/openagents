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
  handleSarahLiveKitCommunityRoomJoinRequest,
  handleSarahLiveKitRoomAuthorityProductionRequest,
  handleSarahLiveKitSharedRoomProductionRequest,
  type SarahLiveKitRoomAuthorityProductionDependencies,
  type SarahLiveKitSharedRoomProductionDependencies,
} from "./sarah-livekit-room-authority-production.js";
import {
  initialSarahLiveKitRoomAuthoritySnapshot,
  requestSarahLiveKitFloor,
  type SarahLiveKitRoomAuthoritySnapshot,
} from "./sarah-livekit-room-authority.js";
import type { SarahVoiceLiveKitRoomBroker } from "./sarah-realtime-voice-routes.js";

const digest = (character: string): string => character.repeat(64);
const userDigest = (userId: string): string =>
  createHash("sha256").update(`sarah-livekit-room-user\n${userId}`).digest("hex");
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
    snapshot?: SarahLiveKitRoomAuthoritySnapshot;
    targetParticipantBound?: boolean;
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
  const snapshot = options.snapshot ?? initialSarahLiveKitRoomAuthoritySnapshot(presence);
  const close = vi.fn(async () => undefined);
  const compareAndSwap = vi.fn(async (input) => input.snapshot);
  const readParticipantBinding = vi.fn(async (input) => {
    if (input.userRefDigest !== undefined) {
      return options.targetParticipantBound === true
        ? {
            ownerUserId: "user.target",
            userRefDigest: input.userRefDigest,
            memberPubkey: digest("2"),
            participantRef: "target-participant-production",
            communityRef: presence.communityRef,
            channelRef: presence.channelRef,
            membershipRevision: presence.membershipRevision,
            roomRef: presence.roomRef,
            roomEpoch: presence.roomEpoch,
          }
        : undefined;
    }
    return options.participantBound === false
      ? undefined
      : {
          ownerUserId: "user.production",
          userRefDigest: userDigest("user.production"),
          memberPubkey: digest("1"),
          participantRef: "owner-participant-production",
          communityRef: presence.communityRef,
          channelRef: presence.channelRef,
          membershipRevision: presence.membershipRevision,
          roomRef: presence.roomRef,
          roomEpoch: presence.roomEpoch,
        };
  });
  const store: SarahLiveKitRoomAuthorityStore = {
    claimCommunityRoomRendezvous: vi.fn(),
    readActiveCommunityRoomRendezvous: vi.fn(),
    retireCommunityRoomRendezvous: vi.fn(),
    listActiveCommunityRoomParticipants: vi.fn(),
    readCommunityRoomBinding: vi.fn(),
    create: vi.fn(),
    read: vi.fn(async () => snapshot),
    readParticipantBinding,
    bindParticipant: vi.fn(),
    removeParticipant: vi.fn(),
    retireRoomMembers: vi.fn(async () => 0),
    retireExpiredRoomMembers: vi.fn(async () => 0),
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
      resolveCommunityAccess: vi.fn(async (_environment, input) => ({
        communityRef: presence.communityRef,
        channelRef: presence.channelRef,
        membershipRevision: options.membershipRevision ?? presence.membershipRevision,
        memberPubkey: input.ownerUserId === "user.target" ? digest("2") : digest("1"),
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

  test("resolves a transfer target only through its server-side durable member binding", async () => {
    const initial = initialSarahLiveKitRoomAuthoritySnapshot(presence);
    const acquired = requestSarahLiveKitFloor(initial, {
      member: {
        authenticated: true,
        allowlisted: true,
        active: true,
        role: "member",
        userRefDigest: userDigest("user.production"),
        pubkey: digest("1"),
        participantRef: "owner-participant-production",
        mappedParticipantRef: "owner-participant-production",
        membershipRevision: presence.membershipRevision,
        roomRef: presence.roomRef,
        roomEpoch: presence.roomEpoch,
        safetyIdentifier: digest("3"),
      },
      nonce: "c".repeat(32),
      requestedLeaseMs: 10_000,
      nowMs: nowMs + 500,
    });
    if (!acquired.accepted) throw new Error(acquired.reason);
    const route = setup({
      snapshot: acquired.snapshot,
      targetParticipantBound: true,
    });
    const transfer = new Request("https://api.openagents.com/member", {
      method: "POST",
      headers: {
        authorization: "Bearer verified",
        "content-type": "application/json",
      },
      body: JSON.stringify({
        action: "transfer",
        presenceLeaseRef: presence.leaseRef,
        expectedRevision: 2,
        nonce: "d".repeat(32),
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
    expect(response.status).toBe(200);
    expect(route.readParticipantBinding).toHaveBeenNthCalledWith(2, {
      presenceLeaseRef: presence.leaseRef,
      userRefDigest: digest("9"),
      now: new Date(nowMs + 1_000).toISOString(),
    });
    expect(route.compareAndSwap).toHaveBeenCalledWith(
      expect.objectContaining({
        expectedRevision: 2,
        snapshot: expect.objectContaining({
          nextInterruptSequence: acquired.snapshot.nextInterruptSequence + 1,
          floor: {
            state: "held",
            lease: expect.objectContaining({
              holderUserRefDigest: digest("9"),
              holderParticipantRef: "target-participant-production",
            }),
          },
        }),
      }),
    );
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

const sharedRequest = (expectedRevision?: number): Request =>
  new Request("https://api.openagents.com/api/sarah/livekit/room/summon", {
    method: "POST",
    headers: {
      authorization: "Bearer verified",
      "content-type": "application/json",
    },
    body: JSON.stringify({
      presenceLeaseRef: presence.leaseRef,
      ...(expectedRevision === undefined ? {} : { expectedRevision }),
    }),
  });

const sharedBroker = (
  grantParticipant: NonNullable<SarahVoiceLiveKitRoomBroker["grantParticipant"]>,
): SarahVoiceLiveKitRoomBroker => ({
  workerControlTokenDigest: vi.fn(() => digest("4")),
  sessionTicket: vi.fn(() => "session-ticket"),
  provision: vi.fn(),
  cleanup: vi.fn(),
  cleanupByIdempotencyKey: vi.fn(),
  cleanupRoom: vi.fn(),
  grantParticipant,
});

describe("Sarah LiveKit shared-room production wiring", () => {
  test.each(["member", "moderator"] as const)(
    "joins a second authenticated %s through the stable community rendezvous",
    async (role) => {
      const snapshot = initialSarahLiveKitRoomAuthoritySnapshot(presence);
      const bindParticipant = vi.fn(async () => undefined);
      const grantParticipant = vi.fn<NonNullable<SarahVoiceLiveKitRoomBroker["grantParticipant"]>>(
        async () => ({
          participantGrant: "second-member-livekit-grant",
          joinExpiresAtMs: nowMs + 50_000,
        }),
      );
      const secondUser = {
        ownerUserId: "user.second",
        userRefDigest: userDigest("user.second"),
        memberPubkey: digest("6"),
        participantRef: `member-${createHash("sha256")
          .update(`sarah-livekit-room-member\n${presence.leaseRef}\nuser.second`)
          .digest("hex")
          .slice(0, 40)}`,
        membershipRevision: presence.membershipRevision,
        roomRef: presence.roomRef,
        roomEpoch: presence.roomEpoch,
      };
      const store: SarahLiveKitRoomAuthorityStore = {
        claimCommunityRoomRendezvous: vi.fn(),
        readActiveCommunityRoomRendezvous: vi.fn(async () => ({
          presenceLeaseRef: presence.leaseRef,
          membershipRevision: presence.membershipRevision,
          roomRef: presence.roomRef,
          roomEpoch: presence.roomEpoch,
          sessionRef: presence.sessionRef,
          generation: presence.generation,
        })),
        retireCommunityRoomRendezvous: vi.fn(),
        listActiveCommunityRoomParticipants: vi.fn(async () => [secondUser]),
        readCommunityRoomBinding: vi.fn(),
        create: vi.fn(),
        read: vi.fn(async () => snapshot),
        readParticipantBinding: vi.fn(),
        bindParticipant,
        removeParticipant: vi.fn(),
        retireRoomMembers: vi.fn(async () => 0),
        retireExpiredRoomMembers: vi.fn(async () => 0),
        compareAndSwap: vi.fn(),
      };
      const dependencies: SarahLiveKitSharedRoomProductionDependencies<
        Record<string, never>,
        Record<string, never>
      > = {
        openStore: vi.fn(async () => ({ store, close: async () => undefined })),
        requireUser: vi.fn(async () => ({ userId: "user.second" })),
        resolveCommunityAccess: vi.fn(async () => ({
          communityRef: presence.communityRef,
          channelRef: presence.channelRef,
          membershipRevision: presence.membershipRevision,
          memberPubkey: digest("6"),
          role,
          publishAllowed: true,
          subscribeAllowed: true,
        })),
        broker: () => sharedBroker(grantParticipant),
        liveKitUrl: () => "wss://livekit.example.com",
        sarahPubkey: () => digest("a"),
        e2eeKeyRevision: () => digest("c"),
        stopWorker: vi.fn(),
        now: () => nowMs,
      };
      const response = await handleSarahLiveKitCommunityRoomJoinRequest(
        dependencies,
        new Request("https://api.openagents.com/api/sarah/livekit/room/join", {
          method: "POST",
          headers: {
            authorization: "Bearer verified",
            "content-type": "application/json",
          },
          body: JSON.stringify({
            communityRef: presence.communityRef,
            channelRef: presence.channelRef,
          }),
        }),
        {},
        {},
      );
      expect(response.status).toBe(200);
      expect(bindParticipant).toHaveBeenCalledWith(
        expect.objectContaining({
          presenceLeaseRef: presence.leaseRef,
          ownerUserId: "user.second",
          roomRef: presence.roomRef,
        }),
      );
      expect(await response.json()).toMatchObject({
        roomRef: presence.roomRef,
        role,
        presenceLeaseRef: presence.leaseRef,
        participantGrant: "second-member-livekit-grant",
        authority: {
          revision: 1,
          verifiedParticipants: [secondUser],
          localParticipant: secondUser,
        },
      });
    },
  );

  test("retires the rendezvous and stops the worker when membership authority changes", async () => {
    const snapshot = initialSarahLiveKitRoomAuthoritySnapshot(presence);
    const retireCommunityRoomRendezvous = vi.fn(async () => true);
    const retireRoomMembers = vi.fn(async () => 1);
    const compareAndSwap = vi.fn(async (input) => input.snapshot);
    const stopWorker = vi.fn(async () => undefined);
    const store: SarahLiveKitRoomAuthorityStore = {
      claimCommunityRoomRendezvous: vi.fn(),
      readActiveCommunityRoomRendezvous: vi.fn(async () => ({
        presenceLeaseRef: presence.leaseRef,
        membershipRevision: presence.membershipRevision,
        roomRef: presence.roomRef,
        roomEpoch: presence.roomEpoch,
        sessionRef: presence.sessionRef,
        generation: presence.generation,
      })),
      retireCommunityRoomRendezvous,
      listActiveCommunityRoomParticipants: vi.fn(),
      readCommunityRoomBinding: vi.fn(),
      create: vi.fn(),
      read: vi.fn(async () => snapshot),
      readParticipantBinding: vi.fn(),
      bindParticipant: vi.fn(),
      removeParticipant: vi.fn(),
      retireRoomMembers,
      retireExpiredRoomMembers: vi.fn(async () => 0),
      compareAndSwap,
    };
    const response = await handleSarahLiveKitCommunityRoomJoinRequest(
      {
        openStore: vi.fn(async () => ({ store, close: async () => undefined })),
        requireUser: vi.fn(async () => ({ userId: "user.second" })),
        resolveCommunityAccess: vi.fn(async () => ({
          communityRef: presence.communityRef,
          channelRef: presence.channelRef,
          membershipRevision: digest("9"),
          memberPubkey: digest("6"),
          role: "member" as const,
          publishAllowed: true,
          subscribeAllowed: true,
        })),
        broker: () => undefined,
        liveKitUrl: () => undefined,
        sarahPubkey: () => digest("a"),
        e2eeKeyRevision: () => digest("c"),
        stopWorker,
        now: () => nowMs,
      },
      new Request("https://api.openagents.com/api/sarah/livekit/room/join", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          communityRef: presence.communityRef,
          channelRef: presence.channelRef,
        }),
      }),
      {},
      {},
    );
    expect(response.status).toBe(409);
    expect(compareAndSwap).toHaveBeenCalledWith(
      expect.objectContaining({
        presenceLeaseRef: presence.leaseRef,
        expectedRevision: 1,
        snapshot: expect.objectContaining({ presenceActive: false }),
      }),
    );
    expect(retireCommunityRoomRendezvous).toHaveBeenCalledWith({
      presenceLeaseRef: presence.leaseRef,
      now: new Date(nowMs).toISOString(),
    });
    // EP263-LK H5 (#9282): members die with the rendezvous they joined
    // through. Retiring one without the other left member rows 'active' with
    // a null removed_at for the life of the database.
    expect(retireRoomMembers).toHaveBeenCalledWith({
      presenceLeaseRef: presence.leaseRef,
      now: new Date(nowMs).toISOString(),
    });
    expect(stopWorker).toHaveBeenCalledWith(
      {},
      {
        sessionRef: presence.sessionRef,
        generation: presence.generation,
        reason: "operator_stop",
      },
    );
  });

  test("bootstraps the durable authority and grants the authenticated member the same room", async () => {
    const create = vi.fn(async (snapshot) => snapshot);
    const bindParticipant = vi.fn(async () => undefined);
    const grantParticipant = vi.fn<NonNullable<SarahVoiceLiveKitRoomBroker["grantParticipant"]>>(
      async () => ({
        participantGrant: "server-issued-livekit-grant",
        joinExpiresAtMs: nowMs + 50_000,
      }),
    );
    const store: SarahLiveKitRoomAuthorityStore = {
      claimCommunityRoomRendezvous: vi.fn(async () => ({
        claimed: true,
        presenceLeaseRef: presence.leaseRef,
      })),
      readActiveCommunityRoomRendezvous: vi.fn(),
      retireCommunityRoomRendezvous: vi.fn(),
      listActiveCommunityRoomParticipants: vi.fn(),
      readCommunityRoomBinding: vi.fn(async () => ({
        ownerUserId: "user.production",
        sessionRef: presence.sessionRef,
        generation: presence.generation,
        admissionDigest: presence.admissionDigest,
        communityRef: presence.communityRef,
        channelRef: presence.channelRef,
        membershipRevision: presence.membershipRevision,
        roomRef: presence.roomRef,
        roomEpoch: presence.roomEpoch,
        participantRef: "owner-participant-production",
        sarahParticipantRef: presence.sarahParticipantRef,
        dispatchRef: presence.dispatchRef,
        participantGrantDigest: digest("5"),
        joinExpiresAt: new Date(nowMs + 50_000).toISOString(),
        sessionExpiresAt: new Date(nowMs + 60_000).toISOString(),
      })),
      create,
      read: vi.fn(async () => undefined),
      readParticipantBinding: vi.fn(async () => ({
        ownerUserId: "user.production",
        userRefDigest: userDigest("user.production"),
        memberPubkey: digest("1"),
        participantRef: "owner-participant-production",
        communityRef: presence.communityRef,
        channelRef: presence.channelRef,
        membershipRevision: presence.membershipRevision,
        roomRef: presence.roomRef,
        roomEpoch: presence.roomEpoch,
      })),
      bindParticipant,
      removeParticipant: vi.fn(),
      retireRoomMembers: vi.fn(async () => 0),
      retireExpiredRoomMembers: vi.fn(async () => 0),
      compareAndSwap: vi.fn(),
    };
    const close = vi.fn(async () => undefined);
    const dependencies: SarahLiveKitSharedRoomProductionDependencies<
      Record<string, never>,
      Record<string, never>
    > = {
      openStore: vi.fn(async () => ({ store, close })),
      requireUser: vi.fn(async () => ({ userId: "user.production" })),
      resolveCommunityAccess: vi.fn(async () => ({
        communityRef: presence.communityRef,
        channelRef: presence.channelRef,
        membershipRevision: presence.membershipRevision,
        memberPubkey: digest("1"),
        role: "moderator" as const,
        publishAllowed: true,
        subscribeAllowed: true,
      })),
      broker: () => sharedBroker(grantParticipant),
      liveKitUrl: () => "wss://livekit.example.com",
      sarahPubkey: () => digest("a"),
      e2eeKeyRevision: () => digest("c"),
      stopWorker: vi.fn(),
      now: () => nowMs,
    };

    const response = await handleSarahLiveKitSharedRoomProductionRequest(
      dependencies,
      "summon",
      sharedRequest(),
      {},
      {},
    );
    expect(response.status).toBe(200);
    expect(create).toHaveBeenCalledOnce();
    expect(grantParticipant).toHaveBeenCalledWith({
      roomRef: presence.roomRef,
      participantRef: "owner-participant-production",
      expiresAtMs: nowMs + 60_000,
      publishAllowed: true,
      subscribeAllowed: true,
    });
    expect(bindParticipant).toHaveBeenCalledTimes(2);
    expect(await response.json()).toMatchObject({
      livekitUrl: "wss://livekit.example.com",
      roomRef: presence.roomRef,
      roomEpoch: presence.roomEpoch,
      role: "moderator",
      participantRef: "owner-participant-production",
      participantGrant: "server-issued-livekit-grant",
    });
    expect(close).toHaveBeenCalledOnce();
  });

  test("rejects a losing first-room claim before issuing another member grant", async () => {
    const grantParticipant = vi.fn<NonNullable<SarahVoiceLiveKitRoomBroker["grantParticipant"]>>();
    const store: SarahLiveKitRoomAuthorityStore = {
      claimCommunityRoomRendezvous: vi.fn(async () => ({
        claimed: false,
        presenceLeaseRef: "presence:production:winner",
      })),
      readActiveCommunityRoomRendezvous: vi.fn(),
      retireCommunityRoomRendezvous: vi.fn(),
      listActiveCommunityRoomParticipants: vi.fn(),
      readCommunityRoomBinding: vi.fn(async () => ({
        ownerUserId: "user.production",
        sessionRef: presence.sessionRef,
        generation: presence.generation,
        admissionDigest: presence.admissionDigest,
        communityRef: presence.communityRef,
        channelRef: presence.channelRef,
        membershipRevision: presence.membershipRevision,
        roomRef: presence.roomRef,
        roomEpoch: presence.roomEpoch,
        participantRef: "owner-participant-production",
        sarahParticipantRef: presence.sarahParticipantRef,
        dispatchRef: presence.dispatchRef,
        participantGrantDigest: digest("5"),
        joinExpiresAt: new Date(nowMs + 50_000).toISOString(),
        sessionExpiresAt: new Date(nowMs + 60_000).toISOString(),
      })),
      create: vi.fn(async (snapshot) => snapshot),
      read: vi.fn(async () => undefined),
      readParticipantBinding: vi.fn(),
      bindParticipant: vi.fn(),
      removeParticipant: vi.fn(),
      retireRoomMembers: vi.fn(async () => 0),
      retireExpiredRoomMembers: vi.fn(async () => 0),
      compareAndSwap: vi.fn(),
    };
    const response = await handleSarahLiveKitSharedRoomProductionRequest(
      {
        openStore: vi.fn(async () => ({ store, close: async () => undefined })),
        requireUser: vi.fn(async () => ({ userId: "user.production" })),
        resolveCommunityAccess: vi.fn(async () => ({
          communityRef: presence.communityRef,
          channelRef: presence.channelRef,
          membershipRevision: presence.membershipRevision,
          memberPubkey: digest("1"),
          role: "member" as const,
          publishAllowed: true,
          subscribeAllowed: true,
        })),
        broker: () => sharedBroker(grantParticipant),
        liveKitUrl: () => "wss://livekit.example.com",
        sarahPubkey: () => digest("a"),
        e2eeKeyRevision: () => digest("c"),
        stopWorker: vi.fn(),
        now: () => nowMs,
      },
      "summon",
      sharedRequest(),
      {},
      {},
    );
    expect(response.status).toBe(409);
    expect(await response.json()).toEqual({ error: "community_room_already_active" });
    expect(grantParticipant).not.toHaveBeenCalled();
    expect(store.bindParticipant).not.toHaveBeenCalled();
  });

  test("requires moderator authority and stops the worker after durable removal", async () => {
    const snapshot = initialSarahLiveKitRoomAuthoritySnapshot(presence);
    const compareAndSwap = vi.fn(async (input) => input.snapshot);
    const store: SarahLiveKitRoomAuthorityStore = {
      claimCommunityRoomRendezvous: vi.fn(),
      readActiveCommunityRoomRendezvous: vi.fn(),
      retireCommunityRoomRendezvous: vi.fn(async () => true),
      listActiveCommunityRoomParticipants: vi.fn(),
      readCommunityRoomBinding: vi.fn(),
      create: vi.fn(),
      read: vi.fn(async () => snapshot),
      readParticipantBinding: vi.fn(),
      bindParticipant: vi.fn(),
      removeParticipant: vi.fn(),
      retireRoomMembers: vi.fn(async () => 0),
      retireExpiredRoomMembers: vi.fn(async () => 0),
      compareAndSwap,
    };
    const stopWorker = vi.fn(async () => undefined);
    const onAuthorityChanged = vi.fn(async () => undefined);
    const dependencies: SarahLiveKitSharedRoomProductionDependencies<
      Record<string, never>,
      Record<string, never>
    > = {
      openStore: vi.fn(async () => ({ store, close: async () => undefined })),
      requireUser: vi.fn(async () => ({ userId: "moderator.production" })),
      resolveCommunityAccess: vi.fn(async () => ({
        communityRef: presence.communityRef,
        channelRef: presence.channelRef,
        membershipRevision: presence.membershipRevision,
        memberPubkey: digest("8"),
        role: "moderator" as const,
        publishAllowed: true,
        subscribeAllowed: true,
      })),
      broker: () => undefined,
      liveKitUrl: () => undefined,
      sarahPubkey: () => digest("a"),
      e2eeKeyRevision: () => digest("c"),
      onAuthorityChanged,
      stopWorker,
      now: () => nowMs + 1_000,
    };

    const response = await handleSarahLiveKitSharedRoomProductionRequest(
      dependencies,
      "remove",
      sharedRequest(1),
      {},
      {},
    );
    expect(response.status).toBe(200);
    expect(compareAndSwap).toHaveBeenCalledWith(
      expect.objectContaining({
        presenceLeaseRef: presence.leaseRef,
        expectedRevision: 1,
        snapshot: expect.objectContaining({
          revision: 2,
          presenceActive: false,
          nextInterruptSequence: snapshot.nextInterruptSequence + 1,
        }),
      }),
    );
    expect(stopWorker).toHaveBeenCalledWith(
      {},
      {
        sessionRef: presence.sessionRef,
        generation: presence.generation,
        reason: "operator_stop",
      },
    );
    expect(onAuthorityChanged).toHaveBeenCalledWith(
      {},
      snapshot,
      expect.objectContaining({
        revision: 2,
        presenceActive: false,
      }),
    );
    // EP263-LK H5 (#9282): a moderator removal retires the member rows too.
    expect(store.retireRoomMembers).toHaveBeenCalledWith({
      presenceLeaseRef: presence.leaseRef,
      now: new Date(nowMs + 1_000).toISOString(),
    });
  });
});
