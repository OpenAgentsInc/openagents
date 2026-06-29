import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  PsionicQwenClientError,
  defineProbeLlmTool,
  makeProbeLlmRequest,
  makePsionicQwenClient,
  probeLlmToolDefinitions,
} from "../src";

describe("Psionic Qwen OpenAI-compatible chat client", () => {
  test("completes a plain text response", async () => {
    const client = await Effect.runPromise(makePsionicQwenClient({
      fetch: sequenceFetch([
        chatJson("psionic plain ok", { prompt_tokens: 4, completion_tokens: 3, total_tokens: 7 }),
      ]),
      now: new Date("2026-06-09T00:00:00.000Z"),
    }));
    const result = await Effect.runPromise(client.complete({
      request: makeProbeLlmRequest({
        model: { provider: "psionic", model: "qwen3.5-2b" },
        prompt: "Say ok.",
      }),
    }));

    expect(result.text).toBe("psionic plain ok");
    expect(result.roundTrips).toBe(1);
    expect(result.receipt).toMatchObject({
      backendKind: "psionic_qwen35",
      profileId: "psionic-qwen35-local",
      model: "qwen3.5-2b",
      roundTrips: 1,
      contentRedacted: true,
    });
    expect(result.receipt.usage).toMatchObject({
      inputTokens: 4,
      outputTokens: 3,
      totalTokens: 7,
    });
    expect(JSON.stringify(result.receipt)).not.toContain("Say ok");
  });

  test("dispatches a required single tool call and continues to final text", async () => {
    const calls: unknown[] = [];
    const weather = defineProbeLlmTool({
      name: "lookup_weather",
      description: "Lookup weather.",
      inputSchema: {
        type: "object",
        properties: { city: { type: "string" } },
        required: ["city"],
      },
      execute: (input) => Effect.succeed({ forecast: `sunny in ${input.city}` }),
    });
    const client = await Effect.runPromise(makePsionicQwenClient({
      fetch: sequenceFetch([
        toolCallJson("call_1", "lookup_weather", { city: "Austin" }),
        chatJson("It is sunny in Austin."),
      ], calls),
      now: new Date("2026-06-09T00:00:00.000Z"),
    }));
    const result = await Effect.runPromise(client.complete({
      request: makeProbeLlmRequest({
        model: { provider: "psionic", model: "qwen3.5-2b" },
        prompt: "Use the weather tool.",
        tools: probeLlmToolDefinitions({ lookup_weather: weather }),
        toolChoice: { type: "required" },
      }),
      tools: { lookup_weather: weather },
      maxModelRoundTrips: 3,
    }));

    expect(result.text).toBe("It is sunny in Austin.");
    expect(result.roundTrips).toBe(2);
    expect(result.toolReceipts).toMatchObject([
      {
        backendKind: "psionic_qwen35",
        toolCallId: "call_1",
        toolName: "lookup_weather",
        status: "success",
        contentRedacted: true,
      },
    ]);
    expect(requestBody(calls[0]).tool_choice).toBe("required");
    expect(requestBody(calls[1]).messages.at(-1)).toMatchObject({
      role: "tool",
      tool_call_id: "call_1",
      name: "lookup_weather",
    });
  });

  test("supports multi-turn tool loops", async () => {
    const lookup = defineProbeLlmTool({
      name: "lookup",
      description: "Lookup a value.",
      inputSchema: { type: "object", properties: { key: { type: "string" } }, required: ["key"] },
      execute: (input) => Effect.succeed({ value: `value:${input.key}` }),
    });
    const client = await Effect.runPromise(makePsionicQwenClient({
      fetch: sequenceFetch([
        toolCallJson("call_1", "lookup", { key: "first" }),
        toolCallJson("call_2", "lookup", { key: "second" }),
        chatJson("done"),
      ]),
      now: new Date("2026-06-09T00:00:00.000Z"),
    }));
    const result = await Effect.runPromise(client.complete({
      request: makeProbeLlmRequest({
        model: { provider: "psionic", model: "qwen3.5-2b" },
        prompt: "Loop twice.",
        tools: probeLlmToolDefinitions({ lookup }),
        toolChoice: { type: "auto" },
      }),
      tools: { lookup },
      maxModelRoundTrips: 4,
    }));

    expect(result.text).toBe("done");
    expect(result.roundTrips).toBe(3);
    expect(result.toolReceipts.map((receipt) => receipt.toolCallId)).toEqual(["call_1", "call_2"]);
  });

  test("surfaces malformed tool arguments safely", async () => {
    const client = await Effect.runPromise(makePsionicQwenClient({
      fetch: sequenceFetch([
        {
          choices: [
            {
              message: {
                role: "assistant",
                content: null,
                tool_calls: [
                  {
                    id: "call_bad",
                    type: "function",
                    function: {
                      name: "lookup",
                      arguments: "{bad json",
                    },
                  },
                ],
              },
              finish_reason: "tool_calls",
            },
          ],
        },
      ]),
      now: new Date("2026-06-09T00:00:00.000Z"),
    }));
    const error = await captureError(client.complete({
      request: makeProbeLlmRequest({
        model: { provider: "psionic", model: "qwen3.5-2b" },
        prompt: "Do not leak me.",
      }),
    }));

    expect(error).toBeInstanceOf(PsionicQwenClientError);
    expect(error).toMatchObject({ failureClass: "malformed_tool_arguments" });
    expect(JSON.stringify(error?.receipt)).not.toContain("Do not leak me");
  });

  test("round-trip limit stops infinite tool loops", async () => {
    const lookup = defineProbeLlmTool({
      name: "lookup",
      description: "Lookup a value.",
      inputSchema: { type: "object" },
      execute: () => Effect.succeed({ ok: true }),
    });
    const client = await Effect.runPromise(makePsionicQwenClient({
      fetch: sequenceFetch([
        toolCallJson("call_1", "lookup", {}),
        toolCallJson("call_2", "lookup", {}),
      ]),
      now: new Date("2026-06-09T00:00:00.000Z"),
    }));
    const error = await captureError(client.complete({
      request: makeProbeLlmRequest({
        model: { provider: "psionic", model: "qwen3.5-2b" },
        prompt: "Loop forever.",
        tools: probeLlmToolDefinitions({ lookup }),
      }),
      tools: { lookup },
      maxModelRoundTrips: 1,
    }));

    expect(error).toBeInstanceOf(PsionicQwenClientError);
    expect(error).toMatchObject({ failureClass: "round_trip_limit" });
  });

  test("parses streaming delta tool calls", async () => {
    const lookup = defineProbeLlmTool({
      name: "lookup",
      description: "Lookup a city.",
      inputSchema: { type: "object", properties: { city: { type: "string" } }, required: ["city"] },
      execute: (input) => Effect.succeed({ value: input.city }),
    });
    const client = await Effect.runPromise(makePsionicQwenClient({
      fetch: sequenceFetch([
        sseResponse([
          { choices: [{ delta: { tool_calls: [{ index: 0, id: "call_1", type: "function", function: { name: "lookup", arguments: "{\"city\"" } }] } }] },
          { choices: [{ delta: { tool_calls: [{ index: 0, function: { arguments: ":\"Austin\"}" } }] }, finish_reason: "tool_calls" }] },
          { choices: [], usage: { prompt_tokens: 5, completion_tokens: 2, total_tokens: 7 } },
        ]),
        sseResponse([
          { choices: [{ delta: { content: "streamed final" } }] },
          { choices: [{ delta: {}, finish_reason: "stop" }], usage: { prompt_tokens: 6, completion_tokens: 3, total_tokens: 9 } },
        ]),
      ]),
      now: new Date("2026-06-09T00:00:00.000Z"),
    }));
    const result = await Effect.runPromise(client.complete({
      request: makeProbeLlmRequest({
        model: { provider: "psionic", model: "qwen3.5-2b" },
        prompt: "Stream a tool call.",
        tools: probeLlmToolDefinitions({ lookup }),
        providerOptions: { psionic: { stream: true } },
      }),
      tools: { lookup },
      maxModelRoundTrips: 3,
      stream: true,
    }));

    expect(result.text).toBe("streamed final");
    expect(result.roundTrips).toBe(2);
    expect(result.toolReceipts[0]?.toolCallId).toBe("call_1");
    expect(result.receipt.usage).toMatchObject({ totalTokens: 9 });
  });
});

function chatJson(content: string, usage?: unknown) {
  return {
    choices: [
      {
        message: { role: "assistant", content },
        finish_reason: "stop",
      },
    ],
    usage,
  };
}

function toolCallJson(id: string, name: string, args: unknown) {
  return {
    choices: [
      {
        message: {
          role: "assistant",
          content: null,
          tool_calls: [
            {
              id,
              type: "function",
              function: {
                name,
                arguments: JSON.stringify(args),
              },
            },
          ],
        },
        finish_reason: "tool_calls",
      },
    ],
  };
}

function sequenceFetch(responses: ReadonlyArray<unknown>, calls: unknown[] = []): typeof fetch {
  let index = 0;

  return async (_url, init) => {
    calls.push(init?.body === undefined ? undefined : JSON.parse(String(init.body)));
    const response = responses[index++];

    if (response instanceof Response) {
      return response;
    }

    return Response.json(response);
  };
}

function sseResponse(chunks: ReadonlyArray<unknown>): Response {
  return new Response(`${chunks.map((chunk) => `data: ${JSON.stringify(chunk)}\n\n`).join("")}data: [DONE]\n\n`, {
    headers: {
      "Content-Type": "text/event-stream",
    },
  });
}

function requestBody(value: unknown): Record<string, any> {
  return value as Record<string, any>;
}

async function captureError(effect: Effect.Effect<unknown, unknown>): Promise<any> {
  try {
    await Effect.runPromise(effect);
  } catch (error) {
    return error;
  }

  throw new Error("expected effect to fail");
}
