import { describe, expect, test } from "bun:test";
import { Effect, Schema as S } from "effect";
import {
  ProbeLlmEvents,
  ProbeLlmToolFailure,
  defineProbeLlmTool,
  dispatchProbeLlmTool,
  makeProbeLlmMessage,
  makeProbeLlmRequest,
  makeProbeLlmToolResult,
  makeProbeLlmUsage,
  probeLlmToolDefinitions,
  probeLlmVisibleOutputTokens,
  ProbeLlmMessage,
  type ProbeLlmTools,
} from "../src";

describe("Probe provider-neutral LLM core", () => {
  test("constructs text messages and requests without backend-specific fields", async () => {
    const request = makeProbeLlmRequest({
      id: "request_1",
      model: {
        provider: "test-provider",
        model: "test-model",
      },
      system: "You are concise.",
      prompt: "Say hello.",
      generation: {
        maxTokens: 16,
        temperature: 0,
      },
    });

    expect(request.system).toEqual([makeProbeLlmMessage("system", "You are concise.")]);
    expect(request.messages).toEqual([makeProbeLlmMessage("user", "Say hello.")]);
    expect(JSON.stringify(request)).not.toContain("apple_fm");
    expect(JSON.stringify(request)).not.toContain("gemini");
  });

  test("normalizes usage without negative visible output", () => {
    const usage = makeProbeLlmUsage({
      inputTokens: 10,
      outputTokens: 5,
      reasoningTokens: 7,
      cacheReadInputTokens: 3,
    });

    expect(usage.totalTokens).toBe(15);
    expect(usage.reasoningTokens).toBe(5);
    expect(probeLlmVisibleOutputTokens(usage)).toBe(0);
  });

  test("projects tool definitions from named tools", () => {
    const tools: ProbeLlmTools = {
      lookup: defineProbeLlmTool({
        name: "ignored-local-name",
        description: "Lookup a value.",
        inputSchema: {
          type: "object",
          properties: {
            query: { type: "string" },
          },
          required: ["query"],
        },
      }),
    };

    expect(probeLlmToolDefinitions(tools)).toEqual([
      {
        name: "lookup",
        description: "Lookup a value.",
        inputSchema: {
          type: "object",
          properties: {
            query: { type: "string" },
          },
          required: ["query"],
        },
        outputSchema: undefined,
      },
    ]);
  });

  test("dispatches successful local tools into tool-result events", async () => {
    const result = await Effect.runPromise(
      dispatchProbeLlmTool(
        {
          lookup: defineProbeLlmTool({
            name: "lookup",
            description: "Lookup a value.",
            inputSchema: { type: "object" },
            execute: (input) => Effect.succeed({ ok: true, query: input.query }),
          }),
        },
        {
          id: "tool_1",
          name: "lookup",
          input: { query: "weather" },
        },
      ),
    );

    expect(result.result).toEqual({ type: "json", value: { ok: true, query: "weather" } });
    expect(result.events).toEqual([
      ProbeLlmEvents.toolResult({
        id: "tool_1",
        name: "lookup",
        result: {
          type: "json",
          value: { ok: true, query: "weather" },
        },
      }),
    ]);
  });

  test("dispatches tool failures into paired error and result events", async () => {
    const result = await Effect.runPromise(
      dispatchProbeLlmTool(
        {
          lookup: defineProbeLlmTool({
            name: "lookup",
            description: "Lookup a value.",
            inputSchema: { type: "object" },
            execute: () => Effect.fail(new ProbeLlmToolFailure({ message: "lookup unavailable" })),
          }),
        },
        {
          id: "tool_2",
          name: "lookup",
          input: { query: "weather" },
        },
      ),
    );

    expect(result.result).toEqual({ type: "error", value: "lookup unavailable" });
    expect(result.events.map((event) => event.type)).toEqual(["tool-error", "tool-result"]);
  });

  test("keeps tool-result helpers schema-decodable", async () => {
    const part = makeProbeLlmToolResult({
      id: "tool_3",
      name: "lookup",
      result: { answer: 42 },
    });

    const decoded = await Effect.runPromise(S.decodeUnknownEffect(ProbeLlmMessage)(makeProbeLlmMessage("tool", part)));

    expect(decoded.content[0]).toEqual({
      type: "tool-result",
      id: "tool_3",
      name: "lookup",
      result: {
        type: "json",
        value: { answer: 42 },
      },
    });
  });
});
