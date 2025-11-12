import type { ChatModelAdapter } from "@openagentsinc/assistant-ui-runtime";

export function createOllamaAdapter(): ChatModelAdapter {
  return {
    async *run() {
      const text = "Hello from mocked Ollama adapter.";
      yield { content: [{ type: "text", text }] };
      yield {
        content: [{ type: "text", text }],
        status: { type: "complete", reason: "stop" } as const,
      };
    },
  };
}

