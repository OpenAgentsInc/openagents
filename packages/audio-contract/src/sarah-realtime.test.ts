import { describe, expect, test } from "vite-plus/test";
import {
  OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION,
  OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION,
  SARAH_VOICE_ADMISSION_PROTOCOL_VERSION,
  SARAH_VOICE_COHORT_REVOCATION_PROTOCOL_VERSION,
  SARAH_VOICE_PROTOCOL_VERSION,
  SARAH_VOICE_SETTLEMENT_PROTOCOL_VERSION,
  decodeOmegaNostrDeviceLinkChallengeRequest,
  decodeOmegaNostrDeviceLinkRequest,
  decodeSarahEditorCommand,
  decodeSarahVoiceAdmissionRequest,
  decodeSarahVoiceAdmissionResponse,
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
    expect(
      decodeSarahVoiceSettlementResponse({
        schema: SARAH_VOICE_SETTLEMENT_PROTOCOL_VERSION,
        sessionRef: identity.sessionRef,
        state: "settled",
        creditMode: "metered",
        finalChargeMsat: 250,
        spendableRemainingCreditMsat: 9_750,
        receiptRef: "sarah_voice_settlement:session-1",
      }).finalChargeMsat,
    ).toBe(250);
    expect(() =>
      decodeSarahVoiceCohortRevocationRequest({
        schema: SARAH_VOICE_COHORT_REVOCATION_PROTOCOL_VERSION,
        cohortRef: "sarah_voice_cohort:staging_owner_v1",
        reason: "Wrong authority",
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
    expect(() =>
      decodeSarahVoiceSessionResponse({
        ...response,
        clientProfile: "arbitrary_device_commands",
      }),
    ).toThrow();
  });
});
