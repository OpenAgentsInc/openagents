import { realtime } from "@livekit/agents-plugin-openai";
import { describe, expect, test } from "vite-plus/test";

type PatchedRealtimeSession = {
  currentGeneration?: unknown;
  [method: string]: unknown;
};

function sessionWithoutGeneration(): PatchedRealtimeSession {
  const session = Object.create(realtime.RealtimeSession.prototype) as PatchedRealtimeSession;
  session.closedGenerationResponseIds = new Set<string>();
  return session;
}

function invoke(
  session: PatchedRealtimeSession,
  method: string,
  event: Record<string, unknown>,
): unknown {
  const handler = session[method];
  if (typeof handler !== "function") throw new Error(`${method} is not callable`);
  return handler.call(session, event);
}

function responseLifecycleEvents(responseId: string) {
  return [
    ["handleResponseOutputItemAdded", { response_id: responseId, item: { type: "message" } }],
    ["handleResponseContentPartAdded", { response_id: responseId, part: { type: "output_audio" } }],
    ["handleResponseContentPartDone", { response_id: responseId, part: { type: "text" } }],
    ["handleResponseTextDelta", { response_id: responseId, delta: "late text" }],
    ["handleResponseTextDone", { response_id: responseId, text: "late text" }],
    ["handleResponseAudioTranscriptDelta", { response_id: responseId, delta: "late transcript" }],
    ["handleResponseAudioDelta", { response_id: responseId, delta: "AA==" }],
    [
      "handleResponseAudioTranscriptDone",
      { response_id: responseId, transcript: "late transcript" },
    ],
    ["handleResponseAudioDone", { response_id: responseId }],
    ["handleResponseOutputItemDone", { response_id: responseId, item: { type: "message" } }],
    ["handleResponseDone", { response: { id: responseId, status: "cancelled" } }],
  ] as const;
}

describe("patched LiveKit OpenAI realtime session", () => {
  test("ignores only response lifecycle events bound to a closed generation", () => {
    const session = sessionWithoutGeneration();
    const rememberClosedGeneration = session.rememberClosedGeneration;
    if (typeof rememberClosedGeneration !== "function") {
      throw new Error("rememberClosedGeneration is not callable");
    }
    rememberClosedGeneration.call(session, "response-closed");

    for (const [method, event] of responseLifecycleEvents("response-closed")) {
      expect(() => invoke(session, method, event)).not.toThrow();
    }
  });

  test("rejects response lifecycle events without a known closed generation", () => {
    const session = sessionWithoutGeneration();

    for (const [method, event] of responseLifecycleEvents("response-unknown")) {
      expect(() => invoke(session, method, event)).toThrow("currentGeneration is not set");
    }
  });

  test("keeps malformed active generations visible", () => {
    const session = sessionWithoutGeneration();
    session.currentGeneration = { responseId: "response-active", messages: new Map() };
    const rememberClosedGeneration = session.rememberClosedGeneration;
    if (typeof rememberClosedGeneration !== "function") {
      throw new Error("rememberClosedGeneration is not callable");
    }
    rememberClosedGeneration.call(session, "response-closed");

    expect(() =>
      invoke(session, "handleResponseAudioDelta", {
        type: "response.output_audio.delta",
        response_id: "response-closed",
        item_id: "old-item",
        delta: "AA==",
      }),
    ).not.toThrow();

    expect(() =>
      invoke(session, "handleResponseAudioDelta", {
        type: "response.output_audio.delta",
        response_id: "response-active",
        item_id: "missing-item",
        delta: "AA==",
      }),
    ).toThrow("itemGeneration is not set");
    expect(() =>
      invoke(session, "handleResponseAudioDelta", {
        type: "response.output_audio.delta",
        response_id: "response-other",
        item_id: "missing-item",
        delta: "AA==",
      }),
    ).toThrow("currentGeneration response.id does not match event response.id");
  });

  test.each([
    "response.audio.delta",
    "response.output_audio.delta",
    "response.audio_transcript.delta",
    "response.output_audio_transcript.delta",
    "conversation.item.input_audio_transcription.delta",
  ])("redacts the delta payload for %s", (type) => {
    const session = sessionWithoutGeneration();
    const event = {
      type,
      event_id: "evt-one",
      response_id: "response-one",
      item_id: "item-one",
      output_index: 2,
      content_index: 3,
      start_time: 1.25,
      delta: "sensitive payload",
    };

    expect(invoke(session, "loggableEvent", event)).toEqual({
      ...event,
      delta: "...",
    });
    expect(event.delta).toBe("sensitive payload");
  });

  test("redacts nested audio and transcript payloads without hiding metadata", () => {
    const session = sessionWithoutGeneration();
    const event = {
      type: "response.done",
      event_id: "evt-two",
      response: {
        id: "response-two",
        status: "completed",
        output: [
          {
            id: "item-two",
            content: [
              {
                type: "output_audio",
                audio: "base64 audio",
                transcript: "sensitive transcript",
              },
            ],
          },
        ],
        usage: { input_tokens: 7, output_tokens: 11 },
      },
    };

    expect(invoke(session, "loggableEvent", event)).toEqual({
      type: "response.done",
      event_id: "evt-two",
      response: {
        id: "response-two",
        status: "completed",
        output: [
          {
            id: "item-two",
            content: [
              {
                type: "output_audio",
                audio: "...",
                transcript: "...",
              },
            ],
          },
        ],
        usage: { input_tokens: 7, output_tokens: 11 },
      },
    });
  });

  test("preserves non-transcript text deltas and structured audio configuration", () => {
    const session = sessionWithoutGeneration();

    expect(
      invoke(session, "loggableEvent", {
        type: "response.output_text.delta",
        event_id: "evt-three",
        delta: "useful debug text",
      }),
    ).toEqual({
      type: "response.output_text.delta",
      event_id: "evt-three",
      delta: "useful debug text",
    });
    expect(
      invoke(session, "loggableEvent", {
        type: "session.updated",
        session: { audio: { output: { voice: "marin" } } },
      }),
    ).toEqual({
      type: "session.updated",
      session: { audio: { output: { voice: "marin" } } },
    });
  });
});
