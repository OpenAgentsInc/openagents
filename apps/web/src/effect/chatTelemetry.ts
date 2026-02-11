import type { ChatMessage, ChatMessageFinish, ChatPart } from "./chatProtocol";
import type { TelemetryFields } from "./telemetry";

export type ChatTelemetryEvent = {
  readonly name: string;
  readonly fields: TelemetryFields;
};

export type ChatTelemetryState = {
  hasHydratedSnapshot: boolean;
  messageSequence: number;
  userMessageCount: number;
  assistantMessageCount: number;
  readonly seenMessageIds: Set<string>;
  readonly seenAssistantFinishMessageIds: Set<string>;
};

export const createChatTelemetryState = (): ChatTelemetryState => ({
  hasHydratedSnapshot: false,
  messageSequence: 0,
  userMessageCount: 0,
  assistantMessageCount: 0,
  seenMessageIds: new Set<string>(),
  seenAssistantFinishMessageIds: new Set<string>(),
});

const textCharsFromParts = (parts: ReadonlyArray<ChatPart>): number =>
  parts.reduce((total, part) => {
    if (part.type !== "text" || typeof part.text !== "string") return total;
    return total + part.text.length;
  }, 0);

const finishTelemetryFields = (finish: ChatMessageFinish | undefined): TelemetryFields => {
  if (!finish) return {};

  const inputTokens = finish.usage?.inputTokens ?? finish.usage?.promptTokens;
  const outputTokens = finish.usage?.outputTokens ?? finish.usage?.completionTokens;
  const totalTokens =
    finish.usage?.totalTokens ??
    (typeof inputTokens === "number" || typeof outputTokens === "number"
      ? (inputTokens ?? 0) + (outputTokens ?? 0)
      : undefined);

  return {
    ...(typeof finish.reason === "string" ? { finishReason: finish.reason } : {}),
    ...(typeof inputTokens === "number" ? { inputTokens } : {}),
    ...(typeof outputTokens === "number" ? { outputTokens } : {}),
    ...(typeof totalTokens === "number" ? { totalTokens } : {}),
    ...(typeof finish.modelId === "string" ? { modelId: finish.modelId } : {}),
    ...(typeof finish.provider === "string" ? { provider: finish.provider } : {}),
    ...(typeof finish.modelRoute === "string" ? { modelRoute: finish.modelRoute } : {}),
    ...(typeof finish.modelFallbackId === "string" ? { modelFallbackId: finish.modelFallbackId } : {}),
    ...(typeof finish.timeToFirstTokenMs === "number"
      ? { timeToFirstTokenMs: finish.timeToFirstTokenMs }
      : {}),
    ...(typeof finish.timeToCompleteMs === "number"
      ? { timeToCompleteMs: finish.timeToCompleteMs }
      : {}),
  };
};

const updateCountersForMessage = (state: ChatTelemetryState, role: ChatMessage["role"]): void => {
  state.messageSequence += 1;
  if (role === "user") state.userMessageCount += 1;
  else state.assistantMessageCount += 1;
};

const baseCounterFields = (state: ChatTelemetryState): TelemetryFields => ({
  messageOrdinal: state.messageSequence,
  totalMessageCount: state.userMessageCount + state.assistantMessageCount,
  userMessageCount: state.userMessageCount,
  assistantMessageCount: state.assistantMessageCount,
});

export const hydrateChatTelemetryState = (
  state: ChatTelemetryState,
  messages: ReadonlyArray<ChatMessage>,
): ChatTelemetryEvent => {
  for (const message of messages) {
    if (state.seenMessageIds.has(message.id)) continue;
    state.seenMessageIds.add(message.id);
    updateCountersForMessage(state, message.role);
    if (message.role === "assistant" && message.finish) {
      state.seenAssistantFinishMessageIds.add(message.id);
    }
  }

  state.hasHydratedSnapshot = true;

  return {
    name: "chat.snapshot_hydrated",
    fields: {
      existingMessageCount: state.userMessageCount + state.assistantMessageCount,
      existingUserMessageCount: state.userMessageCount,
      existingAssistantMessageCount: state.assistantMessageCount,
    },
  };
};

export const collectChatTelemetryEventsForSnapshot = (
  state: ChatTelemetryState,
  messages: ReadonlyArray<ChatMessage>,
): ReadonlyArray<ChatTelemetryEvent> => {
  const events: Array<ChatTelemetryEvent> = [];

  for (const message of messages) {
    if (!state.seenMessageIds.has(message.id)) {
      state.seenMessageIds.add(message.id);
      updateCountersForMessage(state, message.role);

      events.push({
        name: "chat.message_recorded",
        fields: {
          role: message.role,
          partCount: message.parts.length,
          textChars: textCharsFromParts(message.parts),
          hasRunId: Boolean(message.runId),
          ...(message.runId ? { runId: message.runId } : {}),
          ...baseCounterFields(state),
        },
      });
    }

    if (
      message.role === "assistant" &&
      message.finish &&
      !state.seenAssistantFinishMessageIds.has(message.id)
    ) {
      state.seenAssistantFinishMessageIds.add(message.id);
      events.push({
        name: "chat.assistant_finish_recorded",
        fields: {
          hasRunId: Boolean(message.runId),
          ...(message.runId ? { runId: message.runId } : {}),
          ...finishTelemetryFields(message.finish),
        },
      });
    }
  }

  return events;
};

export const collectStreamingTransitionEvents = (input: {
  readonly previousRunId: string | null;
  readonly nextRunId: string | null;
  readonly finishByRunId: ReadonlyMap<string, ChatMessageFinish>;
}): ReadonlyArray<ChatTelemetryEvent> => {
  const { previousRunId, nextRunId, finishByRunId } = input;
  if (previousRunId === nextRunId) return [];

  const events: Array<ChatTelemetryEvent> = [];

  if (previousRunId) {
    events.push({
      name: "chat.streaming_finished",
      fields: {
        runId: previousRunId,
        completionState: nextRunId ? "switched" : "completed",
        ...finishTelemetryFields(finishByRunId.get(previousRunId)),
      },
    });
  }

  if (nextRunId) {
    events.push({
      name: "chat.streaming_started",
      fields: { runId: nextRunId },
    });
  }

  return events;
};
