import type { ChatModelAdapter } from "@assistant-ui/react";
import { createOllama } from "ollama-ai-provider-v2";
import { streamText } from "ai";

/**
 * Factory to create a ChatModelAdapter backed by Ollama.
 * Keeps Ollama specifics out of App.tsx.
 */
export function createOllamaAdapter(options?: { baseURL?: string; model?: string }): ChatModelAdapter {
  const { baseURL = "http://127.0.0.1:11434/api", model = "glm-4.6:cloud" } = options || {};
  const ollama = createOllama({ baseURL });

  return {
    async *run({ messages, abortSignal }) {
      const result = streamText({ model: ollama(model), messages: messages as any, abortSignal });
      const stream = result.textStream;
      let text = "";
      for await (const chunk of stream) {
        text += chunk;
        yield { content: [{ type: "text", text }] };
      }
      yield { content: [{ type: "text", text }], status: { type: "complete", reason: "stop" } as const };
    },
  };
}

