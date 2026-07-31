import {
  SARAH_LIVEKIT_AGENT_NAME,
  SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  type SarahLiveKitJobEvent,
  decodeSarahLiveKitJobClaimResponse,
} from "@openagentsinc/audio-contract";
import type {
  SarahLiveKitRoomAuthorityStore,
  SarahRealtimeVoiceStore,
  SarahVoiceLiveKitMembershipLease,
  SarahVoiceLiveKitWorkerClaim,
} from "@openagentsinc/khala-sync-server";
import { describe, expect, test, vi } from "vitest";

import {
  initialSarahLiveKitRoomAuthoritySnapshot,
  issueSarahLiveKitRoomPresenceLease,
} from "./sarah-livekit-room-authority";
import { deriveSarahLiveKitControlToken } from "./sarah-livekit-room-broker";
import {
  handleSarahLiveKitWorkerClaim,
  handleSarahLiveKitWorkerEvent,
} from "./sarah-livekit-worker-routes";

const controlRoot = "A".repeat(64);
const claimDispatch = {
  sessionRef: "session:one",
  generation: 1,
  roomRef: "room:one",
  roomEpoch: 1,
  participantRef: "owner:one",
  sarahParticipantRef: "principal.sarah",
  sarahPresenceLeaseRef: "presence:one",
  capabilityProfile: "omega_editor",
  roomContext: { kind: "private" },
} as const;
const token = deriveSarahLiveKitControlToken(controlRoot, {
  schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  agentName: SARAH_LIVEKIT_AGENT_NAME,
  ...claimDispatch,
});
const claimLiveKitWorkerJob = vi.fn(
  async (): Promise<SarahVoiceLiveKitWorkerClaim> => ({
    sessionRef: "session:one",
    generation: 1,
    ownerUserId: "owner:one",
    capabilityProfile: "omega_editor" as const,
    roomContext: { kind: "private" as const },
    admissionDigest: "d".repeat(64),
    sessionExpiresAt: "2033-05-18T03:34:20.000Z",
  }),
);
const applyLiveKitWorkerEvent = vi.fn(
  async (): Promise<{
    observedAt: string;
    replayed: boolean;
    interruptSequence?: number;
    providerDisconnectFault?: Readonly<{
      requestRef: string;
      providerSessionRefDigest: string;
    }>;
  }> => ({
    observedAt: "2033-05-18T03:33:20.000Z",
    replayed: false,
  }),
);
const readLiveKitMembershipLease = vi.fn(
  async (): Promise<SarahVoiceLiveKitMembershipLease> => ({
    ownerUserId: "owner:one",
    sarahPresenceLeaseRef: claimDispatch.sarahPresenceLeaseRef,
    roomContext: { kind: "private" },
  }),
);
const revokeLiveKitRoom = vi.fn(async () => ({
  state: "connected" as const,
}));
const store = {
  applyLiveKitWorkerEvent,
  claimLiveKitWorkerJob,
  readLiveKitMembershipLease,
  revokeLiveKitRoom,
} as unknown as SarahRealtimeVoiceStore;
const authorityStore = {
  create: vi.fn(async (snapshot) => snapshot),
  read: vi.fn(async () => undefined),
  compareAndSwap: vi.fn(async (input) => input.snapshot),
  retireCommunityRoomRendezvous: vi.fn(async () => true),
} as unknown as SarahLiveKitRoomAuthorityStore;
const cleanup = vi.fn(async () => undefined);
const dependencies = {
  controlRoot: () => controlRoot,
  now: () => 2_000_000_000_000,
  openStore: async () => ({
    store,
    authorityStore,
    close: async () => undefined,
  }),
  sarahNostrPublicKey: () => "a".repeat(64),
  e2eeKeyRevision: () => "e".repeat(64),
  cleanup,
};

const authorizedRequest = (path: string, body: unknown) =>
  new Request(`https://openagents.com${path}`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  });

describe("Sarah LiveKit worker routes", () => {
  test("claims the exact server dispatch and returns a private profile without owner refs", async () => {
    const response = await handleSarahLiveKitWorkerClaim(
      dependencies,
      authorizedRequest("/api/internal/sarah/livekit/job/claim", {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        workerRef: "worker:one",
        jobRef: "job:one",
        dispatchRef: "dispatch:one",
        roomSid: "RM_one",
        dispatch: claimDispatch,
      }),
      {},
    );
    expect(response.status).toBe(200);
    const body = decodeSarahLiveKitJobClaimResponse(await response.json());
    expect(body.capabilityProfile.kind).toBe("private_owner_v1");
    expect(body.presenceLease).toBeUndefined();
    expect(JSON.stringify(body)).not.toContain("owner:one");
    expect(claimLiveKitWorkerJob).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionRef: "session:one",
        dispatchRef: "dispatch:one",
        workerJobRef: "job:one",
      }),
    );
  });

  test("returns the canonical pre-established community presence lease", async () => {
    const communityDispatch = {
      ...claimDispatch,
      roomContext: {
        kind: "community",
        communityRef: "openagents-public",
        channelRef: "agent-chat",
        membershipRevision: "b".repeat(64),
      },
    } as const;
    claimLiveKitWorkerJob.mockResolvedValueOnce({
      sessionRef: "session:one",
      generation: 1,
      ownerUserId: "owner:one",
      capabilityProfile: "omega_editor",
      roomContext: communityDispatch.roomContext,
      admissionDigest: "d".repeat(64),
      sessionExpiresAt: "2033-05-18T03:34:20.000Z",
    });
    const communityToken = deriveSarahLiveKitControlToken(controlRoot, {
      schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
      agentName: SARAH_LIVEKIT_AGENT_NAME,
      ...communityDispatch,
    });
    const existingAuthority = initialSarahLiveKitRoomAuthoritySnapshot(
      issueSarahLiveKitRoomPresenceLease({
        sarahPubkey: "a".repeat(64),
        presenceLeaseRef: communityDispatch.sarahPresenceLeaseRef,
        communityRef: communityDispatch.roomContext.communityRef,
        channelRef: communityDispatch.roomContext.channelRef,
        membershipRevision: communityDispatch.roomContext.membershipRevision,
        currentMembershipRevision: communityDispatch.roomContext.membershipRevision,
        e2eeKeyRevision: "e".repeat(64),
        roomRef: communityDispatch.roomRef,
        roomEpoch: communityDispatch.roomEpoch,
        sarahParticipantRef: communityDispatch.sarahParticipantRef,
        dispatchRef: "dispatch:one",
        sessionRef: communityDispatch.sessionRef,
        generation: communityDispatch.generation,
        admissionDigest: "d".repeat(64),
        issuedAtMs: 2_000_000_000_000,
        sessionExpiresAtMs: Date.parse("2033-05-18T03:34:20.000Z"),
      }),
    );
    vi.mocked(authorityStore.read).mockResolvedValueOnce(existingAuthority);
    vi.mocked(authorityStore.create).mockClear();
    const response = await handleSarahLiveKitWorkerClaim(
      {
        ...dependencies,
        resolveRoomFloor: async () => ({
          authorityRevision: 1,
          interruptSequence: 0,
          participantRef: null,
          expiresAtMs: null,
          presenceActive: true,
        }),
      },
      new Request("https://openagents.com/api/internal/sarah/livekit/job/claim", {
        method: "POST",
        headers: {
          authorization: `Bearer ${communityToken}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({
          schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
          workerRef: "worker:one",
          jobRef: "job:one",
          dispatchRef: "dispatch:one",
          roomSid: "RM_one",
          dispatch: communityDispatch,
        }),
      }),
      {},
    );
    expect(response.status).toBe(200);
    const body = decodeSarahLiveKitJobClaimResponse(await response.json());
    expect(body.presenceLease).toMatchObject({
      sarahPubkey: "a".repeat(64),
      e2eeKeyRevision: "e".repeat(64),
      admissionDigest: "d".repeat(64),
      communityRef: "openagents-public",
      channelRef: "agent-chat",
    });
    expect(authorityStore.create).not.toHaveBeenCalled();
  });

  test("records response and transcription usage under different idempotency refs", async () => {
    const base = {
      schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
      sessionRef: "session:one",
      generation: 1,
      jobRef: "job:one",
      eventRef: "event:one",
      inputTokens: 7,
      outputTokens: 3,
      cachedInputTokens: 1,
      audioInputTokens: 5,
      audioOutputTokens: 2,
    } as const;
    for (const event of [
      {
        ...base,
        _tag: "response_usage",
        providerResponseRef: "resp_1",
        status: "completed",
      },
      {
        ...base,
        _tag: "transcription_usage",
        providerTranscriptionRef: "item_1",
      },
    ] satisfies SarahLiveKitJobEvent[]) {
      applyLiveKitWorkerEvent.mockClear();
      const response = await handleSarahLiveKitWorkerEvent(
        dependencies,
        authorizedRequest("/api/internal/sarah/livekit/job/event", event),
        {},
      );
      expect(response.status).toBe(200);
      expect(applyLiveKitWorkerEvent).toHaveBeenCalledWith(
        expect.objectContaining({
          eventRef: event.eventRef,
          eventKind: event._tag,
          usage: expect.objectContaining({
            usageKind: event._tag === "response_usage" ? "response" : "transcription",
            providerResponseRef:
              event._tag === "response_usage" ? "response:resp_1" : "transcription:item_1",
            ...(event._tag === "response_usage" ? { providerStatus: "completed" } : {}),
          }),
        }),
      );
    }
  });

  test("records the durable sequence after the worker applies an interrupt", async () => {
    applyLiveKitWorkerEvent.mockClear();
    const response = await handleSarahLiveKitWorkerEvent(
      dependencies,
      authorizedRequest("/api/internal/sarah/livekit/job/event", {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        _tag: "interrupt_applied",
        sessionRef: "session:one",
        generation: 1,
        jobRef: "job:one",
        eventRef: "interrupt:job:one:3",
        interruptSequence: 3,
      }),
      {},
    );
    expect(response.status).toBe(200);
    expect(applyLiveKitWorkerEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        eventKind: "interrupt_applied",
        interruptSequence: 3,
      }),
    );
  });

  test("persists the provider admission digests before readiness", async () => {
    const response = await handleSarahLiveKitWorkerEvent(
      dependencies,
      authorizedRequest("/api/internal/sarah/livekit/job/event", {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        _tag: "provider_admitted",
        sessionRef: "session:one",
        generation: 1,
        jobRef: "job:one",
        eventRef: "provider:one",
        providerSessionRefDigest: "a".repeat(64),
        providerConfigurationDigest: "b".repeat(64),
      }),
      {},
    );
    expect(response.status).toBe(200);
    expect(applyLiveKitWorkerEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        eventKind: "provider_admitted",
        providerSessionRefDigest: "a".repeat(64),
        providerConfigurationDigest: "b".repeat(64),
      }),
    );
  });

  test("persists the exact provider-disconnect application receipt", async () => {
    const response = await handleSarahLiveKitWorkerEvent(
      dependencies,
      authorizedRequest("/api/internal/sarah/livekit/job/event", {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        _tag: "provider_disconnect_fault_applied",
        sessionRef: "session:one",
        generation: 1,
        jobRef: "job:one",
        eventRef: "provider-disconnect:one",
        requestRef: "acceptance:provider-disconnect:one",
        providerSessionRefDigest: "a".repeat(64),
      }),
      {},
    );
    expect(response.status).toBe(200);
    expect(applyLiveKitWorkerEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        eventKind: "provider_disconnect_fault_applied",
        requestRef: "acceptance:provider-disconnect:one",
        providerSessionRefDigest: "a".repeat(64),
      }),
    );
  });

  test("settles and cleans the room exactly once on worker close", async () => {
    const response = await handleSarahLiveKitWorkerEvent(
      dependencies,
      authorizedRequest("/api/internal/sarah/livekit/job/event", {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        _tag: "close",
        sessionRef: "session:one",
        generation: 1,
        jobRef: "job:one",
        eventRef: "close:job:one",
        reason: "provider_disconnect",
        accountingStatus: "exact",
      }),
      {},
    );
    expect(response.status).toBe(200);
    expect(applyLiveKitWorkerEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        eventRef: "close:job:one",
        eventKind: "close",
        closeReason: "livekit_worker_provider_disconnect",
        accountingStatus: "exact",
      }),
    );
    expect(cleanup).toHaveBeenCalledWith(
      {},
      {
        sessionRef: "session:one",
        generation: 1,
      },
    );
  });

  test("preserves cleanup when worker accounting is uncertain", async () => {
    const cleanupCalls = cleanup.mock.calls.length;
    const response = await handleSarahLiveKitWorkerEvent(
      dependencies,
      authorizedRequest("/api/internal/sarah/livekit/job/event", {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        _tag: "close",
        sessionRef: "session:one",
        generation: 1,
        jobRef: "job:one",
        eventRef: "close:job:uncertain",
        reason: "provider_disconnect",
        accountingStatus: "uncertain",
      }),
      {},
    );
    expect(response.status).toBe(200);
    expect(applyLiveKitWorkerEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        eventKind: "close",
        accountingStatus: "uncertain",
      }),
    );
    expect(cleanup).toHaveBeenCalledTimes(cleanupCalls);
  });

  test("retires community authority and rendezvous on worker close", async () => {
    const presenceLeaseRef = "presence:community-close";
    readLiveKitMembershipLease.mockResolvedValueOnce({
      ownerUserId: "owner:one",
      sarahPresenceLeaseRef: presenceLeaseRef,
      roomContext: {
        kind: "community",
        communityRef: "community:one",
        channelRef: "channel:one",
        membershipRevision: "b".repeat(64),
      },
    });
    const authority = initialSarahLiveKitRoomAuthoritySnapshot(
      issueSarahLiveKitRoomPresenceLease({
        sarahPubkey: "a".repeat(64),
        presenceLeaseRef,
        communityRef: "community:one",
        channelRef: "channel:one",
        membershipRevision: "b".repeat(64),
        currentMembershipRevision: "b".repeat(64),
        e2eeKeyRevision: "e".repeat(64),
        roomRef: "room:one",
        roomEpoch: 1,
        sarahParticipantRef: claimDispatch.sarahParticipantRef,
        dispatchRef: "dispatch:one",
        sessionRef: "session:one",
        generation: 1,
        admissionDigest: "d".repeat(64),
        issuedAtMs: 2_000_000_000_000,
        sessionExpiresAtMs: 2_000_000_060_000,
      }),
    );
    vi.mocked(authorityStore.read).mockResolvedValueOnce(authority);
    vi.mocked(authorityStore.compareAndSwap).mockClear();
    vi.mocked(authorityStore.retireCommunityRoomRendezvous).mockClear();

    const response = await handleSarahLiveKitWorkerEvent(
      dependencies,
      authorizedRequest("/api/internal/sarah/livekit/job/event", {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        _tag: "close",
        sessionRef: "session:one",
        generation: 1,
        jobRef: "job:one",
        eventRef: "close:community",
        reason: "worker_error",
        accountingStatus: "uncertain",
      }),
      {},
    );

    expect(response.status).toBe(200);
    expect(authorityStore.compareAndSwap).toHaveBeenCalledWith(
      expect.objectContaining({
        presenceLeaseRef,
        expectedRevision: authority.revision,
        snapshot: expect.objectContaining({ presenceActive: false }),
      }),
    );
    expect(authorityStore.retireCommunityRoomRendezvous).toHaveBeenCalledWith({
      presenceLeaseRef,
      now: "2033-05-18T03:33:20.000Z",
    });
  });

  test("requests a drain when a community membership revision changes", async () => {
    readLiveKitMembershipLease.mockResolvedValueOnce({
      ownerUserId: "owner:one",
      sarahPresenceLeaseRef: claimDispatch.sarahPresenceLeaseRef,
      roomContext: {
        kind: "community",
        communityRef: "community:one",
        channelRef: "channel:one",
        membershipRevision: "revision:one",
      },
    });
    revokeLiveKitRoom.mockClear();
    applyLiveKitWorkerEvent.mockClear();
    const response = await handleSarahLiveKitWorkerEvent(
      {
        ...dependencies,
        resolveCommunityAccess: async () => ({
          communityRef: "community:one",
          channelRef: "channel:one",
          membershipRevision: "revision:two",
          subscribeAllowed: true,
        }),
      },
      authorizedRequest("/api/internal/sarah/livekit/job/event", {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        _tag: "lease_check",
        sessionRef: "session:one",
        generation: 1,
        jobRef: "job:one",
        eventRef: "lease:changed-membership",
      }),
      {},
    );

    expect(response.status).toBe(200);
    expect(revokeLiveKitRoom).toHaveBeenCalledTimes(1);
    expect(revokeLiveKitRoom).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionRef: "session:one",
        generation: 1,
        stopReason: "membership_revoked",
        reason: "community_membership_changed",
      }),
    );
    expect(applyLiveKitWorkerEvent).toHaveBeenCalledTimes(1);
  });

  test("returns the current room floor and interrupt sequence on a community lease check", async () => {
    readLiveKitMembershipLease.mockResolvedValueOnce({
      ownerUserId: "owner:one",
      sarahPresenceLeaseRef: claimDispatch.sarahPresenceLeaseRef,
      roomContext: {
        kind: "community",
        communityRef: "community:one",
        channelRef: "channel:one",
        membershipRevision: "revision:one",
      },
    });
    applyLiveKitWorkerEvent.mockResolvedValueOnce({
      observedAt: "2033-05-18T03:33:20.000Z",
      replayed: false,
      interruptSequence: 2,
    });
    const response = await handleSarahLiveKitWorkerEvent(
      {
        ...dependencies,
        resolveCommunityAccess: async () => ({
          communityRef: "community:one",
          channelRef: "channel:one",
          membershipRevision: "revision:one",
          subscribeAllowed: true,
        }),
        resolveRoomFloor: async () => ({
          authorityRevision: 4,
          interruptSequence: 3,
          participantRef: "member-current-floor",
          expiresAtMs: 2_000_000_010_000,
          presenceActive: true,
        }),
      },
      authorizedRequest("/api/internal/sarah/livekit/job/event", {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        _tag: "lease_check",
        sessionRef: "session:one",
        generation: 1,
        jobRef: "job:one",
        eventRef: "lease:room-floor",
      }),
      {},
    );
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({
      accepted: true,
      interruptSequence: 3,
      authorityRevision: 4,
      floorParticipantRef: "member-current-floor",
      floorExpiresAtMs: 2_000_000_010_000,
      presenceActive: true,
    });
  });

  test("returns a provider-disconnect directive only on its worker lease", async () => {
    applyLiveKitWorkerEvent.mockResolvedValueOnce({
      observedAt: "2033-05-18T03:33:20.000Z",
      replayed: false,
      providerDisconnectFault: {
        requestRef: "acceptance:provider-disconnect:one",
        providerSessionRefDigest: "a".repeat(64),
      },
    });
    const response = await handleSarahLiveKitWorkerEvent(
      dependencies,
      authorizedRequest("/api/internal/sarah/livekit/job/event", {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        _tag: "lease_check",
        sessionRef: "session:one",
        generation: 1,
        jobRef: "job:one",
        eventRef: "lease:provider-disconnect",
      }),
      {},
    );

    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({
      accepted: true,
      providerDisconnectFault: {
        requestRef: "acceptance:provider-disconnect:one",
        providerSessionRefDigest: "a".repeat(64),
      },
    });
  });

  test("fails closed before storage for a wrong token or malformed control root", async () => {
    const openStore = vi.fn(dependencies.openStore);
    const claimBody = {
      schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
      workerRef: "worker:one",
      jobRef: "job:one",
      dispatchRef: "dispatch:one",
      roomSid: "RM_one",
      dispatch: claimDispatch,
    } as const;
    const wrongTokenResponse = await handleSarahLiveKitWorkerClaim(
      { ...dependencies, openStore },
      new Request("https://openagents.com/api/internal/sarah/livekit/job/claim", {
        method: "POST",
        headers: {
          authorization: `Bearer oa_sarah_lk_${"Z".repeat(43)}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(claimBody),
      }),
      {},
    );
    expect(wrongTokenResponse.status).toBe(401);
    const badRootResponse = await handleSarahLiveKitWorkerClaim(
      {
        ...dependencies,
        controlRoot: () => ` ${controlRoot}`,
        openStore,
      },
      authorizedRequest("/api/internal/sarah/livekit/job/claim", claimBody),
      {},
    );
    expect(badRootResponse.status).toBe(401);
    expect(openStore).not.toHaveBeenCalled();
  });
});
