import { describe, expect, test } from "vite-plus/test";
import {
  OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION,
  OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION,
  SARAH_VOICE_ADMISSION_PROTOCOL_VERSION,
  SARAH_VOICE_ACCOUNTING_RECONCILIATION_PROTOCOL_VERSION,
  SARAH_VOICE_COHORT_REVOCATION_PROTOCOL_VERSION,
  SARAH_VOICE_PROTOCOL_VERSION,
  SARAH_VOICE_SETTLEMENT_PROTOCOL_VERSION,
  decodeOmegaNostrDeviceLinkChallengeRequest,
  decodeOmegaNostrDeviceLinkRequest,
  decodeSarahEditorCommand,
  decodeSarahVoiceAdmissionRequest,
  decodeSarahVoiceAdmissionResponse,
  decodeSarahVoiceAccountingReconciliationRequest,
  decodeSarahVoiceClientControl,
  decodeSarahVoiceCohortRevocationRequest,
  decodeSarahVoiceSessionRequest,
  decodeSarahVoiceSessionResponse,
  decodeSarahVoiceServerControl,
  decodeSarahVoiceSettlementResponse,
} from "./sarah-realtime.js";

const identity = {
  ownerRef: "user-1",
  deviceRef: "omega-1",
  threadRef: "thread-1",
  sessionRef: "session-1",
  generation: 1,
} as const;

describe("Sarah Realtime voice contract", () => {
  test("accepts the bounded Nostr device-link contract", () => {
    const pubkey = "a".repeat(64);
    expect(
      decodeOmegaNostrDeviceLinkChallengeRequest({
        schema: OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION,
        deviceRef: "mobile-device-1",
        pubkey,
      }).pubkey,
    ).toBe(pubkey);
    expect(
      decodeOmegaNostrDeviceLinkRequest({
        schema: OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION,
        challenge: "c".repeat(43),
        ownerRef: "github:owner",
        deviceRef: "mobile-device-1",
      }).ownerRef,
    ).toBe("github:owner");
  });

  test("rejects a non-canonical Nostr device-link pubkey and excess fields", () => {
    expect(() =>
      decodeOmegaNostrDeviceLinkChallengeRequest({
        schema: OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION,
        deviceRef: "mobile-device-1",
        pubkey: "A".repeat(64),
      }),
    ).toThrow();
    expect(() =>
      decodeOmegaNostrDeviceLinkRequest({
        schema: OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION,
        challenge: "c".repeat(43),
        ownerRef: "github:owner",
        deviceRef: "mobile-device-1",
        pubkey: "a".repeat(64),
      }),
    ).toThrow();
  });

  test("accepts the bounded editor allowlist", () => {
    expect(
      decodeSarahEditorCommand({
        _tag: "replace_selection",
        target: { workspaceRef: "workspace-1", path: "src/app.ts" },
        replacement: "const ready = true\n",
      })._tag,
    ).toBe("replace_selection");
  });

  test("rejects shell and external commands", () => {
    expect(() =>
      decodeSarahEditorCommand({
        _tag: "run_shell",
        command: "git push",
      }),
    ).toThrow();
  });

  test("accepts only the bounded start-agent-thread command fields", () => {
    expect(
      decodeSarahEditorCommand({
        _tag: "start_agent_thread",
        message: "Inspect the current test failure.",
        presentation: "background",
      })._tag,
    ).toBe("start_agent_thread");
    expect(() =>
      decodeSarahEditorCommand({
        _tag: "start_agent_thread",
        message: "Inspect the current test failure.",
        presentation: "background",
        model: "provider/model",
      }),
    ).toThrow();
  });

  test("requires an exact confirmation digest", () => {
    expect(() =>
      decodeSarahVoiceClientControl({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        _tag: "tool_decision",
        identity,
        sequence: 4,
        proposalRef: "proposal-1",
        proposalDigest: "not-a-digest",
        decision: "confirm",
      }),
    ).toThrow();
  });

  test("admits a voice-only mobile profile without editor capability", () => {
    expect(
      decodeSarahVoiceSessionRequest({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: "openagents.mobile.sarah.voice.v1",
        clientProfile: "mobile_voice_only",
      }).clientProfile,
    ).toBe("mobile_voice_only");
  });

  test("decodes server controls at the client boundary", () => {
    expect(
      decodeSarahVoiceServerControl({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        sequence: 0,
        _tag: "lifecycle",
        state: "listening",
      })._tag,
    ).toBe("lifecycle");
  });

  test("decodes exact admission economics without accepting a ticket", () => {
    expect(
      decodeSarahVoiceAdmissionRequest({
        schema: SARAH_VOICE_ADMISSION_PROTOCOL_VERSION,
        identity,
        disclosureRef: "disclosure-1",
        auth: {
          method: "nostr_nip98",
          challenge: "c".repeat(43),
        },
      }).auth?.method,
    ).toBe("nostr_nip98");
    const admission = {
      schema: SARAH_VOICE_ADMISSION_PROTOCOL_VERSION,
      admitted: true,
      clientProfile: "omega_editor",
      admissionCohortRef: "sarah_voice_cohort:alpha_v1",
      creditMode: "metered",
      creditRateMsatPerMillionTokens: 100_000,
      requiredHoldMsat: 25_000,
      spendableRemainingCreditMsat: 75_000,
      maxDurationSeconds: 600,
      admissionRef: "sarah_voice_admission:binding-1",
      admissionExpiresAtMs: 120_000,
      capabilityBoundary: {
        commands: ["context_read", "start_agent_thread"],
        confirmationRequired: ["start_agent_thread"],
        directShell: false,
        directGit: false,
        payment: false,
        credentialAccess: false,
        deviceControl: false,
      },
    } as const;
    expect(decodeSarahVoiceAdmissionResponse(admission).admitted).toBe(true);
    expect(() =>
      decodeSarahVoiceAdmissionResponse({
        ...admission,
        ticket: "must-not-be-created-by-admission",
      }),
    ).toThrow();
  });

  test("decodes settlement evidence and fixes revocation to the alpha cohort", () => {
    const settlement = decodeSarahVoiceSettlementResponse({
      schema: SARAH_VOICE_SETTLEMENT_PROTOCOL_VERSION,
      sessionRef: identity.sessionRef,
      state: "settled",
      creditMode: "metered",
      finalChargeMsat: 250,
      spendableRemainingCreditMsat: 9_750,
      receiptRef: "sarah_voice_settlement:session-1",
      acceptanceEvidence: {
        principal: "principal.sarah",
        identityDigests: {
          job: "1".repeat(64),
          providerSession: "2".repeat(64),
          providerConfiguration: "3".repeat(64),
          context: "4".repeat(64),
          capability: "5".repeat(64),
          hold: "6".repeat(64),
          usage: "7".repeat(64),
          settlement: "8".repeat(64),
        },
        usage: {
          inputTokens: 100,
          outputTokens: 50,
          cachedInputTokens: 0,
          audioInputTokens: 75,
          audioOutputTokens: 25,
          chargeMsat: 250,
          responseCount: 1,
          transcriptionCount: 1,
          cancelledResponseCount: 0,
        },
        providerAccountingStatus: "exact",
        workerJobCount: 1,
        providerSessionCount: 1,
        workerClosedAt: "2026-07-31T12:00:05.000Z",
        providerAdmittedAt: "2026-07-31T12:00:00.000Z",
      },
    });
    expect(settlement.finalChargeMsat).toBe(250);
    expect(settlement.acceptanceEvidence?.principal).toBe("principal.sarah");
    expect(() =>
      decodeSarahVoiceCohortRevocationRequest({
        schema: SARAH_VOICE_COHORT_REVOCATION_PROTOCOL_VERSION,
        cohortRef: "sarah_voice_cohort:staging_owner_v1",
        reason: "Wrong authority",
      }),
    ).toThrow();
  });

  test("bounds accounting reconciliation to opaque evidence and numeric usage", () => {
    const request = {
      schema: SARAH_VOICE_ACCOUNTING_RECONCILIATION_PROTOCOL_VERSION,
      reconciliationRef: "reconciliation-1",
      sessionRef: identity.sessionRef,
      generation: identity.generation,
      providerSessionRefDigest: "a".repeat(64),
      providerEvidenceRefs: ["provider-export-1"],
      usage: [
        {
          kind: "response",
          providerResponseRef: "provider-response-1",
          status: "completed",
          inputTokens: 100,
          outputTokens: 50,
          cachedInputTokens: 10,
          audioInputTokens: 80,
          audioOutputTokens: 40,
        },
      ],
      reason: "Verified against provider export",
    } as const;
    expect(decodeSarahVoiceAccountingReconciliationRequest(request).usage).toHaveLength(1);
    expect(() =>
      decodeSarahVoiceAccountingReconciliationRequest({
        ...request,
        transcript: "must never cross the operator reconciliation boundary",
      }),
    ).toThrow();
  });

  test("requires an exact client profile in the pre-release session response", () => {
    const response = {
      schema: SARAH_VOICE_PROTOCOL_VERSION,
      sessionRef: identity.sessionRef,
      model: "gpt-realtime-2.1",
      gatewayUrl: "wss://openagents.com/api/omega/sarah/voice/connect",
      ticket: "t".repeat(43),
      ticketExpiresAtMs: 1_000,
      sessionExpiresAtMs: 2_000,
      reservedCreditMsat: 25_000,
      maxDurationSeconds: 600,
      clientProfile: "mobile_voice_only",
      inputAudio: {
        codec: "pcm_s16le",
        sampleRateHz: 24_000,
        channels: 1,
      },
      outputAudio: {
        codec: "pcm_s16le",
        sampleRateHz: 24_000,
        channels: 1,
      },
    } as const;
    expect(decodeSarahVoiceSessionResponse(response).clientProfile).toBe("mobile_voice_only");
    expect(decodeSarahVoiceSessionResponse(response).transport).toBeUndefined();
    expect(
      decodeSarahVoiceSessionResponse({
        ...response,
        transport: { kind: "custom_wss_v1" },
      }).transport,
    ).toEqual({ kind: "custom_wss_v1" });
    const liveKitTransport = {
      kind: "livekit_room_v1",
      livekitUrl: "wss://livekit.openagents.test",
      roomRef: "room-1",
      roomEpoch: 1,
      participantRef: "participant-1",
      sarahParticipantRef: "principal.sarah",
      participantGrant: "opaque-grant",
      joinExpiresAtMs: 1_500,
      dispatchRef: "dispatch-1",
      sarahPresenceLeaseRef: "presence-1",
      permissions: {
        roomJoin: true,
        canPublish: true,
        canSubscribe: true,
        canPublishData: false,
        canUpdateOwnMetadata: false,
        canPublishSources: ["microphone"],
        roomAdmin: false,
        roomCreate: false,
        roomList: false,
      },
    } as const;
    expect(
      decodeSarahVoiceSessionResponse({
        ...response,
        transport: liveKitTransport,
      }).transport,
    ).toEqual(liveKitTransport);
    expect(() =>
      decodeSarahVoiceSessionResponse({
        ...response,
        transport: {
          ...liveKitTransport,
          permissions: {
            ...liveKitTransport.permissions,
            openAiApiKey: "must-not-pass",
          },
        },
      }),
    ).toThrow();
    expect(() =>
      decodeSarahVoiceSessionResponse({
        ...response,
        clientProfile: "arbitrary_device_commands",
      }),
    ).toThrow();
  });

  test("carries the Omega admission binding through session issue", () => {
    expect(
      decodeSarahVoiceSessionRequest({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: "omega.voice.disclosure.v1",
        clientProfile: "omega_editor",
        admissionRef: "sarah_voice_admission:binding-1",
      }).admissionRef,
    ).toBe("sarah_voice_admission:binding-1");
  });

  test("requires an explicit known transport when a client opts into LiveKit", () => {
    expect(
      decodeSarahVoiceSessionRequest({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: "omega.voice.disclosure.v1",
        requestedTransport: "livekit_room_v1",
        roomContext: { kind: "private" },
      }).requestedTransport,
    ).toBe("livekit_room_v1");
    expect(() =>
      decodeSarahVoiceSessionRequest({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: "omega.voice.disclosure.v1",
        requestedTransport: "livekit_future_v2",
      }),
    ).toThrow();
  });
});
