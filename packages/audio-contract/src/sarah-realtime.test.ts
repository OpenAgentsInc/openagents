import { describe, expect, test } from "vite-plus/test";
import {
  SARAH_VOICE_PROTOCOL_VERSION,
  decodeSarahEditorCommand,
  decodeSarahVoiceClientControl,
  decodeSarahVoiceSessionRequest,
  decodeSarahVoiceSessionResponse,
  decodeSarahVoiceServerControl,
} from "./sarah-realtime.js";

const identity = {
  ownerRef: "user-1",
  deviceRef: "omega-1",
  threadRef: "thread-1",
  sessionRef: "session-1",
  generation: 1,
} as const;

describe("Sarah Realtime voice contract", () => {
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
    expect(decodeSarahVoiceSessionResponse(response).clientProfile).toBe(
      "mobile_voice_only",
    );
    expect(() =>
      decodeSarahVoiceSessionResponse({
        ...response,
        clientProfile: "arbitrary_device_commands",
      }),
    ).toThrow();
  });
});
