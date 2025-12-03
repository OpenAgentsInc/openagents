import type { ChatRequest, ChatResponse, ChatMessage } from "../llm/openrouter.js";
import type { Tool } from "../tools/schema.js";

export interface QueuedMessage<T = any> {
  original: T;
  llm?: ChatMessage;
}

export interface AgentRunConfig {
  systemPrompt?: string;
  tools?: Tool<any>[];
  model?: string;
  temperature?: number;
  getQueuedMessages?: <T>() => Promise<QueuedMessage<T>[]>;
  queueMode?: "all" | "one-at-a-time";
}

export type AgentEvent =
  | { type: "llm_response"; message: ChatResponse; pendingToolCalls: string[] }
  | { type: "llm_error"; error: Error };

export interface AgentTransport {
  run(
    messages: ChatMessage[],
    userMessage: ChatMessage,
    config: AgentRunConfig,
    signal?: AbortSignal,
  ): AsyncIterable<AgentEvent>;
}

export const createProviderTransport = (chat: (request: ChatRequest, signal?: AbortSignal) => Promise<ChatResponse>): AgentTransport => ({
  async *run(messages, userMessage, config, signal) {
    try {
      const queued = (await config.getQueuedMessages?.<any>()) ?? [];
      const injectedQueue =
        config.queueMode === "all"
          ? queued
          : queued.length > 0
            ? [queued[0]]
            : [];
      const injected = injectedQueue.map((q) => q.llm).filter(Boolean) as ChatMessage[];

      const request: ChatRequest = {
        messages: [...messages, ...injected, userMessage],
        ...(config.tools ? { tools: config.tools } : {}),
        ...(config.model ? { model: config.model } : {}),
        ...(config.temperature !== undefined ? { temperature: config.temperature } : {}),
      };

      const res = await chat(request, signal);
      const toolCalls =
        res.choices?.[0]?.message?.tool_calls?.map((tc) => tc.id).filter(Boolean) ?? [];
      yield { type: "llm_response", message: res, pendingToolCalls: toolCalls };
    } catch (error: any) {
      yield { type: "llm_error", error: error instanceof Error ? error : new Error(String(error)) };
    }
  },
});
