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
}

export type AgentEvent =
  | { type: "llm_response"; message: ChatResponse }
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
      const injected = queued.map((q) => q.llm).filter(Boolean) as ChatMessage[];

      const request: ChatRequest = {
        messages: [...messages, ...injected, userMessage],
        ...(config.tools ? { tools: config.tools } : {}),
        ...(config.model ? { model: config.model } : {}),
        ...(config.temperature !== undefined ? { temperature: config.temperature } : {}),
      };

      const res = await chat(request, signal);
      yield { type: "llm_response", message: res };
    } catch (error: any) {
      yield { type: "llm_error", error: error instanceof Error ? error : new Error(String(error)) };
    }
  },
});
