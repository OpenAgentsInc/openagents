import { describe, expect, test } from "vite-plus/test";
import {
  SARAH_LIVEKIT_AGENT_NAME,
  SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  canonicalSarahLiveKitDispatchAuthority,
  decodeSarahLiveKitDispatchMetadata,
  decodeSarahLiveKitJobClaimResponse,
  decodeSarahLiveKitJobEvent,
} from "./sarah-livekit-worker.js";

const dispatch = {
  schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  agentName: SARAH_LIVEKIT_AGENT_NAME,
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

describe("Sarah LiveKit worker contract", () => {
  test("requires exact explicit-dispatch authority fields", () => {
    expect(decodeSarahLiveKitDispatchMetadata(dispatch)).toEqual(dispatch);
    expect(() =>
      decodeSarahLiveKitDispatchMetadata({
        ...dispatch,
        agentName: "default-agent",
      }),
    ).toThrow();
    expect(() =>
      decodeSarahLiveKitDispatchMetadata({
        ...dispatch,
        roomEpoch: 2,
        untrustedClientMetadata: "ignored",
      }),
    ).toThrow();
    expect(() =>
      decodeSarahLiveKitDispatchMetadata({
        ...dispatch,
        controlToken: `oa_sarah_lk_${"A".repeat(43)}`,
      }),
    ).toThrow();
    expect(JSON.stringify(dispatch)).not.toMatch(/bearer|credential|token/iu);
  });

  test("canonicalizes immutable dispatch fields with generation and room separation", () => {
    const canonical = canonicalSarahLiveKitDispatchAuthority(dispatch);
    expect(canonicalSarahLiveKitDispatchAuthority({ ...dispatch })).toBe(canonical);
    expect(
      canonicalSarahLiveKitDispatchAuthority({
        ...dispatch,
        generation: dispatch.generation + 1,
      }),
    ).not.toBe(canonical);
    expect(
      canonicalSarahLiveKitDispatchAuthority({
        ...dispatch,
        roomRef: "room:two",
      }),
    ).not.toBe(canonical);
  });

  test("makes private and community capabilities structurally different", () => {
    const base = {
      schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
      admitted: true,
      sessionRef: "session:one",
      generation: 1,
      sessionExpiresAtMs: 2_000_000_000_000,
      safetyIdentifier: "a".repeat(64),
    } as const;
    const privateClaim = decodeSarahLiveKitJobClaimResponse({
      ...base,
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
    const communityClaim = decodeSarahLiveKitJobClaimResponse({
      ...base,
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
    });
    expect(privateClaim.capabilityProfile.kind).toBe("private_owner_v1");
    expect(communityClaim.capabilityProfile.kind).toBe("community_member_v1");
    expect(communityClaim.capabilityProfile).not.toHaveProperty("privateMemoryRef");
  });

  test("separates realtime response usage from transcription usage", () => {
    const usage = {
      schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
      sessionRef: "session:one",
      generation: 1,
      jobRef: "job:one",
      eventRef: "event:one",
      inputTokens: 3,
      outputTokens: 2,
      cachedInputTokens: 1,
      audioInputTokens: 2,
      audioOutputTokens: 1,
    } as const;
    expect(
      decodeSarahLiveKitJobEvent({
        ...usage,
        _tag: "response_usage",
        providerResponseRef: "response:one",
        status: "completed",
      })._tag,
    ).toBe("response_usage");
    expect(
      decodeSarahLiveKitJobEvent({
        ...usage,
        _tag: "transcription_usage",
        providerTranscriptionRef: "transcription:one",
      })._tag,
    ).toBe("transcription_usage");
  });
});
