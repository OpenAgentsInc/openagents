import { describe, expect, test } from "vite-plus/test";
import { SarahGenerationFence, responseUsageEvent, transcriptionUsageEvent } from "./generation.js";

const identity = {
  sessionRef: "session:one",
  generation: 4,
  jobRef: "job:one",
} as const;

describe("Sarah LiveKit generation fence", () => {
  test("extracts exact response.done token details without transcript content", () => {
    const event = responseUsageEvent(
      {
        type: "response.done",
        response: {
          id: "resp_1",
          status: "cancelled",
          output: [{ transcript: "must not escape" }],
          usage: {
            input_tokens: 12,
            output_tokens: 7,
            input_token_details: {
              cached_tokens: 3,
              audio_tokens: 8,
            },
            output_token_details: { audio_tokens: 5 },
          },
        },
      },
      identity,
    );
    expect(event).toEqual({
      schema: "openagents.sarah.livekit-worker.v1",
      _tag: "response_usage",
      ...identity,
      eventRef: "response:resp_1",
      providerResponseRef: "resp_1",
      status: "cancelled",
      inputTokens: 12,
      outputTokens: 7,
      cachedInputTokens: 3,
      audioInputTokens: 8,
      audioOutputTokens: 5,
    });
    expect(JSON.stringify(event)).not.toContain("must not escape");
  });

  test("accounts transcription separately and rejects usage-free events", () => {
    expect(
      transcriptionUsageEvent(
        {
          type: "conversation.item.input_audio_transcription.completed",
          item_id: "item_1",
          transcript: "private words",
          usage: {
            input_tokens: 10,
            output_tokens: 4,
            input_token_details: { audio_tokens: 10 },
            output_token_details: {},
          },
        },
        identity,
      ),
    ).toEqual({
      schema: "openagents.sarah.livekit-worker.v1",
      _tag: "transcription_usage",
      ...identity,
      eventRef: "transcription:item_1",
      providerTranscriptionRef: "item_1",
      inputTokens: 10,
      outputTokens: 4,
      cachedInputTokens: 0,
      audioInputTokens: 10,
      audioOutputTokens: 0,
    });
    expect(
      transcriptionUsageEvent(
        {
          type: "conversation.item.input_audio_transcription.completed",
          item_id: "item_2",
          transcript: "not retained",
        },
        identity,
      ),
    ).toBeUndefined();
  });

  test("settles a generation once and drains tracked accounting", async () => {
    const fence = new SarahGenerationFence();
    let completed = false;
    fence.track(
      Promise.resolve().then(() => {
        completed = true;
      }),
    );
    expect(fence.settle("provider_disconnect")).toBe(true);
    expect(fence.settle("completed")).toBe(false);
    await fence.drain();
    expect(completed).toBe(true);
    expect(fence.closeReason).toBe("provider_disconnect");
  });
});
