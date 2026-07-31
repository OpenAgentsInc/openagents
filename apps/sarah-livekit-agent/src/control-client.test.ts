import { describe, expect, test, vi } from "vite-plus/test";
import {
  SARAH_LIVEKIT_AGENT_NAME,
  SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
} from "@openagentsinc/audio-contract";
import { makeSarahLiveKitControlClient } from "./control-client.js";

const dispatch = {
  schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  agentName: SARAH_LIVEKIT_AGENT_NAME,
  controlToken: `oa_sarah_lk_${"A".repeat(43)}`,
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

describe("Sarah LiveKit control client", () => {
  test("binds a claim to the exact dispatch and sends the token only as bearer auth", async () => {
    const fetcher = vi.fn(async (_url: string | URL | Request, init?: RequestInit) => {
      expect(init?.headers).toMatchObject({
        authorization: `Bearer ${dispatch.controlToken}`,
      });
      expect(String(init?.body)).not.toContain(dispatch.controlToken);
      return Response.json({
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        admitted: true,
        sessionRef: dispatch.sessionRef,
        generation: dispatch.generation,
        sessionExpiresAtMs: 2_000_000_000_000,
        safetyIdentifier: "a".repeat(64),
        capabilityProfile: {
          kind: "private_owner_v1",
          contextRead: true,
          editorProposals: true,
          ownerMemory: true,
          workspace: true,
          payments: false,
          release: false,
          memberAdmin: false,
          shell: false,
          git: false,
          credentials: false,
        },
      });
    });
    const client = makeSarahLiveKitControlClient(
      {
        baseUrl: "https://openagents.com",
        workerRef: "worker:one",
      },
      fetcher as typeof fetch,
    );
    const claim = await client.claim({
      dispatch,
      dispatchRef: "dispatch:one",
      jobRef: "job:one",
      roomSid: "RM_one",
    });
    expect(claim.sessionRef).toBe(dispatch.sessionRef);
  });

  test("rejects a control response that changes the generation", async () => {
    const client = makeSarahLiveKitControlClient(
      {
        baseUrl: "https://openagents.com",
        workerRef: "worker:one",
      },
      vi.fn(async () =>
        Response.json({
          schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
          admitted: true,
          sessionRef: dispatch.sessionRef,
          generation: 2,
          sessionExpiresAtMs: 2_000_000_000_000,
          safetyIdentifier: "a".repeat(64),
          capabilityProfile: {
            kind: "community_member_v1",
            contextRead: false,
            editorProposals: false,
            ownerMemory: false,
            workspace: false,
            payments: false,
            release: false,
            memberAdmin: false,
            shell: false,
            git: false,
            credentials: false,
          },
        }),
      ) as typeof fetch,
    );
    await expect(
      client.claim({
        dispatch,
        dispatchRef: "dispatch:one",
        jobRef: "job:one",
        roomSid: "RM_one",
      }),
    ).rejects.toThrow("disagreed");
  });

  test("retries a usage event while the binding transaction becomes visible", async () => {
    const fetcher = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(Response.json({ error: "not_ready" }, { status: 409 }))
      .mockResolvedValueOnce(Response.json({ accepted: true }));
    const client = makeSarahLiveKitControlClient(
      {
        baseUrl: "https://openagents.com",
        workerRef: "worker:one",
      },
      fetcher,
    );
    await expect(
      client.event(dispatch.controlToken, {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        _tag: "lease_check",
        sessionRef: dispatch.sessionRef,
        generation: dispatch.generation,
        jobRef: "job:one",
        eventRef: "lease:one",
      }),
    ).resolves.toEqual({ accepted: true });
    expect(fetcher).toHaveBeenCalledTimes(2);
  });
});
