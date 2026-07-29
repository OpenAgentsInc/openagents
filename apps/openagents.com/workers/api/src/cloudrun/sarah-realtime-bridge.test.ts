import { AUDIO_MEDIA_MAGIC, AUDIO_PROTOCOL_VERSION } from "@openagentsinc/audio-contract";
import { createHash } from "node:crypto";
import { describe, expect, test } from "vitest";

import {
  makeSarahRealtimeBridgeData,
  makeSarahRealtimeWebSocketHandlers,
  parseSarahRealtimeBridgeCreditRate,
  sarahEditorCommandRequiresConfirmation,
  sessionUpdateForSarahClientProfile,
  usageFromInputTranscription,
  usageFromProviderResponse,
  validateSarahEditorCommandTarget,
} from "./sarah-realtime-bridge";

const clientAudioFrame = (sequence: number): Uint8Array => {
  const payload = Buffer.from([0, 0]);
  const header = Buffer.from(
    JSON.stringify({
      schema: AUDIO_PROTOCOL_VERSION,
      kind: "client_audio",
      identity: {
        ownerRef: "user-1",
        deviceRef: "device-1",
        threadRef: "thread-1",
        sessionRef: "session-1",
        generation: 1,
      },
      sequence,
      codec: "pcm_s16le",
      sampleRateHz: 24_000,
      channels: 1,
      payloadLength: payload.byteLength,
      sha256: createHash("sha256").update(payload).digest("hex"),
    }),
  );
  const frame = Buffer.alloc(8 + header.byteLength + payload.byteLength);
  frame.write(AUDIO_MEDIA_MAGIC, 0, "ascii");
  frame.writeUInt32BE(header.byteLength, 4);
  header.copy(frame, 8);
  payload.copy(frame, 8 + header.byteLength);
  return frame;
};

describe("Sarah Realtime bridge metering", () => {
  test("advances audio sequence while upstream is still connecting", () => {
    const data = makeSarahRealtimeBridgeData({
      session: {
        sessionRef: "session-1",
        ownerUserId: "user-1",
        ownerActorRef: "agent:user-1",
        deviceRef: "device-1",
        threadRef: "thread-1",
        generation: 1,
        disclosureRef: "disclosure-1",
        clientProfile: "omega_editor",
        creditMode: "metered",
        entitlementRef: null,
        state: "connected",
        reservedMsat: 1_000,
        chargedMsat: 0,
        ticketExpiresAt: "2026-07-29T12:01:00.000Z",
        sessionExpiresAt: "2026-07-29T12:05:00.000Z",
        settlementReceiptRef: null,
      },
      apiKey: "test-key",
      safetyIdentifier: "test-safety",
      creditMsatPerMillionTokens: 1_000,
      store: {} as never,
      closeStore: async () => undefined,
      tasks: {} as never,
    });
    data.helloReceived = true;
    const sent: Array<string> = [];
    const socket = {
      data,
      send: (message: string) => sent.push(message),
    };

    makeSarahRealtimeWebSocketHandlers().message(socket as never, clientAudioFrame(0));

    expect(data.expectedAudioSequence).toBe(1);
    expect(JSON.parse(sent[0] ?? "{}")).toMatchObject({
      _tag: "audio_ack",
      acknowledgedClientSequence: 0,
    });
  });

  test("prices exact provider response tokens with the operator credit rate", () => {
    expect(
      usageFromProviderResponse(
        {
          type: "response.done",
          response: {
            id: "resp-1",
            usage: {
              total_tokens: 1_250,
              input_tokens: 1_000,
              output_tokens: 250,
              input_token_details: {
                cached_tokens: 100,
                audio_tokens: 800,
              },
              output_token_details: { audio_tokens: 200 },
            },
          },
        },
        2_000_000,
        "2026-07-28T12:00:00.000Z",
      ),
    ).toEqual({
      providerResponseRef: "resp-1",
      inputTokens: 1_000,
      outputTokens: 250,
      cachedInputTokens: 100,
      audioInputTokens: 800,
      audioOutputTokens: 200,
      chargeMsat: 2_500,
      observedAt: "2026-07-28T12:00:00.000Z",
    });
  });

  test("records the separate input transcription usage event", () => {
    expect(
      usageFromInputTranscription(
        {
          item_id: "item-1",
          content_index: 0,
          usage: {
            total_tokens: 50,
            input_tokens: 40,
            output_tokens: 10,
            input_token_details: { audio_tokens: 35 },
          },
        },
        2_000_000,
        "2026-07-28T12:00:01.000Z",
      ),
    ).toMatchObject({
      providerResponseRef: "transcription:item-1:0",
      inputTokens: 40,
      outputTokens: 10,
      audioInputTokens: 35,
      chargeMsat: 100,
    });
  });

  test("fails closed on a missing or fractional credit rate", () => {
    expect(parseSarahRealtimeBridgeCreditRate(undefined)).toBeUndefined();
    expect(parseSarahRealtimeBridgeCreditRate("1.5")).toBeUndefined();
    expect(parseSarahRealtimeBridgeCreditRate("2000000")).toBe(2_000_000);
  });

  test("requires confirmation for each admitted write command", () => {
    expect(
      sarahEditorCommandRequiresConfirmation({
        _tag: "replace_selection",
        target: { workspaceRef: "workspace-1", path: "src/app.ts" },
        replacement: "safe replacement",
      }),
    ).toBe(true);
    expect(
      sarahEditorCommandRequiresConfirmation({
        _tag: "save_document",
        target: { workspaceRef: "workspace-1", path: "src/app.ts" },
      }),
    ).toBe(true);
    expect(
      sarahEditorCommandRequiresConfirmation({
        _tag: "start_agent_thread",
        message: "Inspect the current test failure.",
        presentation: "foreground",
      }),
    ).toBe(true);
    expect(
      sarahEditorCommandRequiresConfirmation({
        _tag: "context_read",
        target: { workspaceRef: "workspace-1", path: "src/app.ts" },
        startLine: 1,
        endLine: 20,
      }),
    ).toBe(false);
  });

  test("rejects traversal and excessive editor ranges", () => {
    expect(() =>
      validateSarahEditorCommandTarget({
        _tag: "open_path",
        target: { workspaceRef: "workspace-1", path: "../secret.txt" },
      }),
    ).toThrow("editor_path_not_allowed");
    expect(() =>
      validateSarahEditorCommandTarget({
        _tag: "context_read",
        target: { workspaceRef: "workspace-1", path: "src/app.ts" },
        startLine: 1,
        endLine: 1_000,
      }),
    ).toThrow("editor_range_not_allowed");
  });

  test("enforces the start-agent-thread message limit in UTF-8 bytes", () => {
    expect(
      validateSarahEditorCommandTarget({
        _tag: "start_agent_thread",
        message: "Inspect the current test failure.",
        presentation: "background",
      }),
    ).toMatchObject({ _tag: "start_agent_thread" });
    expect(() =>
      validateSarahEditorCommandTarget({
        _tag: "start_agent_thread",
        message: "😀".repeat(4_097),
        presentation: "foreground",
      }),
    ).toThrow("agent_thread_message_not_allowed");
  });

  test("gives the mobile voice profile no provider tools or device authority", () => {
    const update = sessionUpdateForSarahClientProfile("mobile_voice_only");
    expect(update.session.tools).toEqual([]);
    expect(update.session.tool_choice).toBe("none");
    expect(update.session.instructions).toContain("voice conversation only");
    expect(update.session.instructions).toContain("Do not request, perform, or claim");
    expect(update.session.instructions).toContain("device action");
  });

  test("keeps the bounded editor profile separate from mobile voice", () => {
    const update = sessionUpdateForSarahClientProfile("omega_editor");
    expect(update.session.tools.length).toBeGreaterThan(0);
    expect(update.session.tool_choice).toBe("auto");
    expect(update.session.tools[0]?.name).toBe("start_agent_thread");
    expect(update.session.tools[0]?.description).toContain(
      "primary capability for repository inspection",
    );
    expect(update.session.instructions).toContain("persistent orchestrator and fleet commander");
    expect(update.session.instructions).toContain(
      "Default to command, inspection, and delegation rather than conversation",
    );
    expect(update.session.instructions).toContain("Do not ask the owner to paste file contents");
    expect(update.session.instructions).toContain(
      "Distinguish observed, submitted, in progress, blocked, and completed",
    );
  });
});
