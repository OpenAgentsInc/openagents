import { describe, expect, it } from "bun:test";
import { createProviderTransport } from "./transport.js";
import type { AgentRunConfig, QueuedMessage } from "./transport.js";
import type { ChatRequest, ChatResponse, ChatMessage } from "../llm/openrouter.js";

describe("createProviderTransport", () => {
  it("injects queued LLM messages before user message", async () => {
    const calls: ChatRequest[] = [];
    const mockChat = async (req: ChatRequest): Promise<ChatResponse> => {
      calls.push(req);
      return {
        id: "1",
        choices: [
          {
            message: {
              role: "assistant",
              content: "",
              tool_calls: [{ id: "call-1", name: "mock", arguments: "{}" }],
            },
          },
        ],
      };
    };

    const transport = createProviderTransport(mockChat);
    const queued: QueuedMessage[] = [{ original: { role: "note" }, llm: { role: "assistant", content: "queued" } as any }];

    const config: AgentRunConfig = {
      getQueuedMessages: async () => queued,
      tools: [],
      model: "x-ai/grok-4.1-fast",
      queueMode: "one-at-a-time",
    };

    const messages: ChatMessage[] = [{ role: "system", content: "sys" }];
    const user: ChatMessage = { role: "user", content: "hi" };

    const events = transport.run(messages, user, config);
    const first = await events.next();
    expect(first.value?.type).toBe("llm_response");
    expect(first.value?.pendingToolCalls).toEqual(["call-1"]);

    expect(calls.length).toBe(1);
    expect(calls[0]?.messages.map((m) => m.content)).toEqual(["sys", "queued", "hi"]);
  });
});
