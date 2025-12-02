import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { fromClient } from "./provider.js";
import type { ChatRequest, ChatResponse } from "./openrouter.js";

const makeMockProvider = (response: ChatResponse) =>
  fromClient({
    chat: () => Effect.succeed(response),
  });

describe("ChatProvider abstraction", () => {
  test("delegates chat to underlying client", async () => {
    const mockResponse: ChatResponse = {
      id: "mock",
      choices: [
        {
          message: {
            role: "assistant",
            content: "hello",
            tool_calls: [],
          },
        },
      ],
    };

    const provider = makeMockProvider(mockResponse);
    const response = await Effect.runPromise(
      provider.chat({ messages: [], tools: [] } as ChatRequest),
    );

    expect(response.id).toBe("mock");
    expect(response.choices[0]?.message.content).toBe("hello");
  });
});
