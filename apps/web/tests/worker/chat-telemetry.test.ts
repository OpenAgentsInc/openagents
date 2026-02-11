import { describe, expect, it } from "vitest";

import type { ChatMessage } from "../../src/effect/chatProtocol";
import {
  collectChatTelemetryEventsForSnapshot,
  collectStreamingTransitionEvents,
  createChatTelemetryState,
  hydrateChatTelemetryState,
} from "../../src/effect/chatTelemetry";

const userMessage = (id: string, text: string): ChatMessage => ({
  id,
  role: "user",
  parts: [{ type: "text", text }],
});

const assistantMessage = (
  id: string,
  input: {
    readonly runId?: string;
    readonly text?: string;
    readonly withFinish?: boolean;
  } = {},
): ChatMessage => ({
  id,
  role: "assistant",
  parts: [{ type: "text", text: input.text ?? "ok", state: "done" }],
  ...(input.runId ? { runId: input.runId } : {}),
  ...(input.withFinish
    ? {
        finish: {
          reason: "stop",
          usage: { inputTokens: 9, outputTokens: 4, totalTokens: 13 },
          modelId: "moonshotai/kimi-k2.5",
          provider: "openrouter",
          timeToFirstTokenMs: 120,
          timeToCompleteMs: 730,
        },
      }
    : {}),
});

describe("apps/web chat telemetry helpers", () => {
  it("hydrates from existing snapshot, then emits events for newly observed messages and finishes", () => {
    const state = createChatTelemetryState();

    const initial = [
      userMessage("u1", "hello"),
      assistantMessage("a1", { runId: "run-1", withFinish: true }),
    ];

    const hydrated = hydrateChatTelemetryState(state, initial);
    expect(hydrated.name).toBe("chat.snapshot_hydrated");
    expect(hydrated.fields).toMatchObject({
      existingMessageCount: 2,
      existingUserMessageCount: 1,
      existingAssistantMessageCount: 1,
    });

    // Replaying the same snapshot should not generate duplicate events.
    expect(collectChatTelemetryEventsForSnapshot(state, initial)).toEqual([]);

    const withNewMessages = [
      ...initial,
      userMessage("u2", "new question"),
      assistantMessage("a2", { runId: "run-2", text: "draft", withFinish: false }),
    ];

    const newMessageEvents = collectChatTelemetryEventsForSnapshot(state, withNewMessages);
    expect(newMessageEvents.map((event) => event.name)).toEqual([
      "chat.message_recorded",
      "chat.message_recorded",
    ]);
    expect(newMessageEvents[0]?.fields).toMatchObject({
      role: "user",
      totalMessageCount: 3,
      userMessageCount: 2,
      assistantMessageCount: 1,
    });
    expect(newMessageEvents[1]?.fields).toMatchObject({
      role: "assistant",
      totalMessageCount: 4,
      userMessageCount: 2,
      assistantMessageCount: 2,
      runId: "run-2",
    });

    const withAssistantFinish = [
      ...initial,
      userMessage("u2", "new question"),
      assistantMessage("a2", { runId: "run-2", text: "final", withFinish: true }),
    ];

    const finishEvents = collectChatTelemetryEventsForSnapshot(state, withAssistantFinish);
    expect(finishEvents.map((event) => event.name)).toEqual(["chat.assistant_finish_recorded"]);
    expect(finishEvents[0]?.fields).toMatchObject({
      runId: "run-2",
      finishReason: "stop",
      inputTokens: 9,
      outputTokens: 4,
      totalTokens: 13,
      modelId: "moonshotai/kimi-k2.5",
      provider: "openrouter",
      timeToFirstTokenMs: 120,
      timeToCompleteMs: 730,
    });
  });

  it("emits streaming transition events with finish metadata", () => {
    const started = collectStreamingTransitionEvents({
      previousRunId: null,
      nextRunId: "run-1",
      finishByRunId: new Map(),
    });
    expect(started).toEqual([
      { name: "chat.streaming_started", fields: { runId: "run-1" } },
    ]);

    const finished = collectStreamingTransitionEvents({
      previousRunId: "run-1",
      nextRunId: null,
      finishByRunId: new Map([
        [
          "run-1",
          {
            reason: "stop",
            usage: { inputTokens: 3, outputTokens: 7, totalTokens: 10 },
            modelId: "openai/gpt-oss-120b",
            provider: "openrouter",
            timeToCompleteMs: 480,
          },
        ],
      ]),
    });
    expect(finished).toEqual([
      {
        name: "chat.streaming_finished",
        fields: {
          runId: "run-1",
          completionState: "completed",
          finishReason: "stop",
          inputTokens: 3,
          outputTokens: 7,
          totalTokens: 10,
          modelId: "openai/gpt-oss-120b",
          provider: "openrouter",
          timeToCompleteMs: 480,
        },
      },
    ]);
  });
});
