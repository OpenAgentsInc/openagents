import {
  SARAH_LIVEKIT_AGENT_NAME,
  SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  type SarahLiveKitJobEvent,
  decodeSarahLiveKitJobClaimResponse,
} from "@openagentsinc/audio-contract";
import type {
  SarahRealtimeVoiceStore,
  SarahVoiceLiveKitMembershipLease,
} from "@openagentsinc/khala-sync-server";
import { describe, expect, test, vi } from "vitest";

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
const claimLiveKitWorkerJob = vi.fn(async () => ({
  sessionRef: "session:one",
  generation: 1,
  ownerUserId: "owner:one",
  capabilityProfile: "omega_editor" as const,
  roomContext: { kind: "private" as const },
  sessionExpiresAt: "2033-05-18T03:34:20.000Z",
}));
const applyLiveKitWorkerEvent = vi.fn(async () => ({
  observedAt: "2033-05-18T03:33:20.000Z",
  replayed: false,
}));
const readLiveKitMembershipLease = vi.fn(
  async (): Promise<SarahVoiceLiveKitMembershipLease> => ({
    ownerUserId: "owner:one",
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
const cleanup = vi.fn(async () => undefined);
const dependencies = {
  controlRoot: () => controlRoot,
  now: () => 2_000_000_000_000,
  openStore: async () => ({ store, close: async () => undefined }),
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
    expect(JSON.stringify(body)).not.toContain("owner:one");
    expect(claimLiveKitWorkerJob).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionRef: "session:one",
        dispatchRef: "dispatch:one",
        workerJobRef: "job:one",
      }),
    );
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

  test("requests a drain when a community membership revision changes", async () => {
    readLiveKitMembershipLease.mockResolvedValueOnce({
      ownerUserId: "owner:one",
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
