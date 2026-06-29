import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { parseGeminiSseStream } from "../src";

const sse = (...events: ReadonlyArray<unknown>): string =>
  `${events.map((event) => `data: ${JSON.stringify(event)}\n`).join("\n")}data: [DONE]\n\n`;

describe("Gemini SSE stream parser", () => {
  test("parses text and usage stream fixtures", async () => {
    const events = await Effect.runPromise(
      parseGeminiSseStream(
        sse(
          {
            candidates: [{ content: { role: "model", parts: [{ text: "Hello" }] } }],
          },
          {
            candidates: [{ content: { role: "model", parts: [{ text: "!" }] }, finishReason: "STOP" }],
          },
          {
            usageMetadata: {
              promptTokenCount: 5,
              candidatesTokenCount: 2,
              totalTokenCount: 7,
              cachedContentTokenCount: 1,
            },
          },
        ),
      ),
    );

    expect(events.map((event) => event.type)).toEqual([
      "step-start",
      "text-delta",
      "text-delta",
      "step-finish",
      "finish",
    ]);
    expect(events[1]).toMatchObject({ type: "text-delta", text: "Hello" });
    expect(events.at(-1)).toMatchObject({
      type: "finish",
      reason: "stop",
      usage: {
        inputTokens: 5,
        outputTokens: 2,
        nonCachedInputTokens: 4,
        cacheReadInputTokens: 1,
        totalTokens: 7,
      },
    });
  });

  test("parses reasoning and preserves thought signatures", async () => {
    const events = await Effect.runPromise(
      parseGeminiSseStream(
        sse(
          {
            candidates: [
              {
                content: {
                  role: "model",
                  parts: [{ text: "thinking", thought: true, thoughtSignature: "sig_1" }],
                },
                finishReason: "STOP",
              },
            ],
          },
        ),
      ),
    );

    expect(events[1]).toEqual({
      type: "reasoning-delta",
      id: "reasoning-0",
      text: "thinking",
      providerMetadata: {
        google: {
          thoughtSignature: "sig_1",
        },
      },
    });
  });

  test("parses function calls as normalized tool-call events", async () => {
    const events = await Effect.runPromise(
      parseGeminiSseStream(
        sse({
          candidates: [
            {
              content: {
                role: "model",
                parts: [{ functionCall: { name: "lookup", args: { query: "weather" } }, thoughtSignature: "sig_tool" }],
              },
              finishReason: "STOP",
            },
          ],
        }),
      ),
    );

    expect(events[1]).toEqual({
      type: "tool-call",
      id: "tool_0",
      name: "lookup",
      input: { query: "weather" },
      providerMetadata: {
        google: {
          thoughtSignature: "sig_tool",
        },
      },
    });
    expect(events.at(-1)).toMatchObject({ type: "finish", reason: "tool_calls" });
  });

  test("adds thoughts to visible candidate tokens for inclusive output usage", async () => {
    const events = await Effect.runPromise(
      parseGeminiSseStream(
        sse({
          candidates: [{ finishReason: "MAX_TOKENS" }],
          usageMetadata: {
            promptTokenCount: 10,
            candidatesTokenCount: 4,
            thoughtsTokenCount: 3,
            totalTokenCount: 17,
          },
        }),
      ),
    );

    expect(events.at(-1)).toMatchObject({
      type: "finish",
      reason: "length",
      usage: {
        inputTokens: 10,
        outputTokens: 7,
        reasoningTokens: 3,
        totalTokens: 17,
      },
    });
  });

  test("fails malformed SSE payloads without provider secrets", async () => {
    await expect(Effect.runPromise(parseGeminiSseStream("data: {bad-json api-key-secret}\n\n"))).rejects.toMatchObject({
      _tag: "GeminiProtocolError",
      failureClass: "malformed_response",
    });
  });
});
