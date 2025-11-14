/**
 * Hook for managing messages via Convex
 * Replaces Tinyvex WebSocket queries for message data
 */

import { useQuery, useMutation } from "convex/react";
import { api } from "../../../convex/_generated/api";
import type { Id } from "../../../convex/_generated/dataModel";

export function useConvexMessages(threadId: Id<"threads"> | undefined) {
  // Query all messages for a thread
  const messages = useQuery(
    api.chat.getMessages,
    threadId ? { threadId } : "skip"
  );

  // Query partial (streaming) messages
  const partialMessages = useQuery(
    api.chat.getPartialMessages,
    threadId ? { threadId } : "skip"
  );

  // Mutations
  const addMessage = useMutation(api.chat.addMessage);
  const upsertStreamingMessage = useMutation(api.chat.upsertStreamingMessage);
  const finalizeMessage = useMutation(api.chat.finalizeMessage);

  return {
    // Data
    messages,
    partialMessages,
    isLoading: messages === undefined,

    // Mutations
    addMessage: async (args: {
      threadId: Id<"threads">;
      role: "user" | "assistant" | "system";
      content: string;
    }) => {
      const messageId = await addMessage(args);
      return messageId;
    },

    upsertStreamingMessage: async (args: {
      threadId: Id<"threads">;
      itemId: string;
      role: "user" | "assistant" | "system";
      content: string;
      kind?: "message" | "reason";
      partial?: boolean;
      seq?: number;
    }) => {
      const messageId = await upsertStreamingMessage(args);
      return messageId;
    },

    finalizeMessage: async (itemId: string) => {
      await finalizeMessage({ itemId });
    },
  };
}

// Hook for tool calls
export function useConvexToolCalls(threadId: Id<"threads"> | undefined) {
  const toolCalls = useQuery(
    api.toolCalls.listToolCalls,
    threadId ? { threadId, limit: 100 } : "skip"
  );

  const upsertToolCall = useMutation(api.toolCalls.upsertToolCall);
  const updateToolCallStatus = useMutation(api.toolCalls.updateToolCallStatus);

  return {
    toolCalls,
    isLoading: toolCalls === undefined,

    upsertToolCall: async (args: {
      threadId: Id<"threads">;
      toolCallId: string;
      title?: string;
      kind?: string;
      status?: string;
      contentJson?: string;
      locationsJson?: string;
    }) => {
      const id = await upsertToolCall(args);
      return id;
    },

    updateToolCallStatus: async (toolCallId: string, status: string) => {
      await updateToolCallStatus({ toolCallId, status });
    },
  };
}

// Hook for plan entries
export function useConvexPlan(threadId: Id<"threads"> | undefined) {
  const plan = useQuery(
    api.planEntries.getPlan,
    threadId ? { threadId } : "skip"
  );

  const upsertPlan = useMutation(api.planEntries.upsertPlan);

  return {
    plan,
    isLoading: plan === undefined,

    upsertPlan: async (entriesJson: string) => {
      if (!threadId) throw new Error("No threadId");
      await upsertPlan({ threadId, entriesJson });
    },
  };
}

// Hook for thread state
export function useConvexThreadState(threadId: Id<"threads"> | undefined) {
  const state = useQuery(
    api.threadState.getThreadState,
    threadId ? { threadId } : "skip"
  );

  const upsertThreadState = useMutation(api.threadState.upsertThreadState);
  const updateCurrentMode = useMutation(api.threadState.updateCurrentMode);

  return {
    state,
    isLoading: state === undefined,

    upsertThreadState: async (args: {
      currentModeId?: string;
      availableCommandsJson?: string;
    }) => {
      if (!threadId) throw new Error("No threadId");
      await upsertThreadState({ threadId, ...args });
    },

    updateCurrentMode: async (currentModeId: string) => {
      if (!threadId) throw new Error("No threadId");
      await updateCurrentMode({ threadId, currentModeId });
    },
  };
}
