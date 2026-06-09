import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  defineProbeLlmTool,
  makeGeminiClient,
  makeProbeLlmRequest,
  type ProbeLlmTools,
} from "../src";

const sse = (...events: ReadonlyArray<unknown>): string =>
  `${events.map((event) => `data: ${JSON.stringify(event)}\n`).join("\n")}data: [DONE]\n\n`;

describe("Gemini native tool loop", () => {
  test("continues after native function calls and returns final text", async () => {
    const bodies: unknown[] = [];
    const urls: string[] = [];
    const responses = [
      sse({
        candidates: [
          {
            content: {
              role: "model",
              parts: [{ functionCall: { name: "lookup", args: { query: "weather" } } }],
            },
            finishReason: "STOP",
          },
        ],
      }),
      sse({
        candidates: [
          {
            content: {
              role: "model",
              parts: [{ text: "The weather is sunny." }],
            },
            finishReason: "STOP",
          },
        ],
      }),
    ];
    const tools: ProbeLlmTools = {
      lookup: defineProbeLlmTool({
        name: "lookup",
        description: "Lookup data.",
        inputSchema: { type: "object", properties: { query: { type: "string" } } },
        execute: (input) => Effect.succeed({ forecast: "sunny", query: input.query }),
      }),
    };
    const client = await Effect.runPromise(
      makeGeminiClient({
        apiKey: "test-key",
        fetch: async (url, init) => {
          urls.push(String(url));
          bodies.push(JSON.parse(String(init?.body)));
          return new Response(responses.shift() ?? "", {
            status: 200,
            headers: { "Content-Type": "text/event-stream" },
          });
        },
      }),
    );

    const result = await Effect.runPromise(
      client.complete({
        request: makeProbeLlmRequest({
          model: { provider: "google", model: "gemini-3.5-flash" },
          prompt: "Use lookup.",
          tools: [
            {
              name: "lookup",
              description: "Lookup data.",
              inputSchema: { type: "object", properties: { query: { type: "string" } } },
            },
          ],
          toolChoice: { type: "auto" },
        }),
        tools,
      }),
    );

    expect(result.text).toBe("The weather is sunny.");
    expect(result.roundTrips).toBe(2);
    expect(urls[0]).toBe(
      "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.5-flash:streamGenerateContent?alt=sse",
    );
    expect(result.events.map((event) => event.type)).toContain("tool-call");
    expect(result.events.map((event) => event.type)).toContain("tool-result");
    expect(bodies[1]).toMatchObject({
      contents: [
        { role: "user" },
        { role: "model", parts: [{ functionCall: { name: "lookup", args: { query: "weather" } } }] },
        {
          role: "user",
          parts: [
            {
              functionResponse: {
                name: "lookup",
                response: {
                  name: "lookup",
                  content: "{\"forecast\":\"sunny\",\"query\":\"weather\"}",
                },
              },
            },
          ],
        },
      ],
    });
  });

  test("relaxes forced tool choice after the first native tool response", async () => {
    const bodies: unknown[] = [];
    const responses = [
      sse({
        candidates: [
          {
            content: { role: "model", parts: [{ functionCall: { name: "echo", args: { value: "first" } } }] },
            finishReason: "STOP",
          },
        ],
      }),
      sse({ candidates: [{ content: { role: "model", parts: [{ text: "done" }] }, finishReason: "STOP" }] }),
    ];
    const tools: ProbeLlmTools = {
      echo: defineProbeLlmTool({
        name: "echo",
        description: "Echo data.",
        inputSchema: { type: "object", properties: { value: { type: "string" } }, required: ["value"] },
        execute: (input) => Effect.succeed({ value: input.value }),
      }),
    };
    const client = await Effect.runPromise(
      makeGeminiClient({
        apiKey: "test-key",
        fetch: async (_url, init) => {
          bodies.push(JSON.parse(String(init?.body)));
          return new Response(responses.shift() ?? "", { status: 200 });
        },
      }),
    );

    const result = await Effect.runPromise(
      client.complete({
        request: makeProbeLlmRequest({
          model: { provider: "google", model: "gemini-3.5-flash" },
          prompt: "Use echo once.",
          tools: [
            {
              name: "echo",
              description: "Echo data.",
              inputSchema: { type: "object", properties: { value: { type: "string" } }, required: ["value"] },
            },
          ],
          toolChoice: { type: "tool", name: "echo" },
        }),
        tools,
      }),
    );

    expect(result.text).toBe("done");
    expect(bodies[0]).toMatchObject({
      toolConfig: { functionCallingConfig: { mode: "ANY", allowedFunctionNames: ["echo"] } },
    });
    expect(bodies[1]).toMatchObject({
      toolConfig: { functionCallingConfig: { mode: "AUTO" } },
    });
  });

  test("feeds unknown tools back as safe tool errors", async () => {
    const bodies: unknown[] = [];
    const responses = [
      sse({
        candidates: [
          {
            content: { role: "model", parts: [{ functionCall: { name: "missing", args: {} } }] },
            finishReason: "STOP",
          },
        ],
      }),
      sse({ candidates: [{ content: { role: "model", parts: [{ text: "Could not use tool." }] }, finishReason: "STOP" }] }),
    ];
    const client = await Effect.runPromise(
      makeGeminiClient({
        apiKey: "test-key",
        fetch: async (_url, init) => {
          bodies.push(JSON.parse(String(init?.body)));
          return new Response(responses.shift() ?? "", { status: 200 });
        },
      }),
    );
    const result = await Effect.runPromise(
      client.complete({
        request: makeProbeLlmRequest({
          model: { provider: "google", model: "gemini-3.5-flash" },
          prompt: "Use a missing tool.",
        }),
      }),
    );

    expect(result.events.map((event) => event.type)).toContain("tool-error");
    expect(JSON.stringify(bodies)).toContain("Unknown tool: missing");
    expect(JSON.stringify(bodies)).not.toContain("test-key");
  });

  test("enforces round-trip limits", async () => {
    const client = await Effect.runPromise(
      makeGeminiClient({
        apiKey: "test-key",
        fetch: async () =>
          new Response(
            sse({
              candidates: [
                {
                  content: { role: "model", parts: [{ functionCall: { name: "loop", args: {} } }] },
                  finishReason: "STOP",
                },
              ],
            }),
            { status: 200 },
          ),
      }),
    );

    await expect(
      Effect.runPromise(
        client.complete({
          request: makeProbeLlmRequest({
            model: { provider: "google", model: "gemini-3.5-flash" },
            prompt: "Loop.",
          }),
          maxModelRoundTrips: 1,
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "GeminiClientError",
      failureClass: "round_trip_limit",
    });
  });

  test("emits streamed events through onEvent while parsing SSE chunks", async () => {
    const body = sse(
      { candidates: [{ content: { role: "model", parts: [{ text: "Hello " }] } }] },
      {
        candidates: [{ content: { role: "model", parts: [{ text: "stream" }] }, finishReason: "STOP" }],
        usageMetadata: { promptTokenCount: 2, candidatesTokenCount: 2, totalTokenCount: 4 },
      },
    );
    const midpoint = Math.floor(body.length / 2);
    const client = await Effect.runPromise(
      makeGeminiClient({
        apiKey: "test-key",
        fetch: async () =>
          new Response(
            new ReadableStream({
              start(controller) {
                controller.enqueue(new TextEncoder().encode(body.slice(0, midpoint)));
                controller.enqueue(new TextEncoder().encode(body.slice(midpoint)));
                controller.close();
              },
            }),
            { status: 200 },
          ),
      }),
    );
    const streamed: string[] = [];
    const result = await Effect.runPromise(
      client.complete({
        request: makeProbeLlmRequest({
          model: { provider: "google", model: "gemini-3.5-flash" },
          prompt: "Stream.",
        }),
        onEvent: (event) => {
          if (event.type === "text-delta") {
            streamed.push(event.text);
          }
        },
      }),
    );

    expect(streamed).toEqual(["Hello ", "stream"]);
    expect(result.text).toBe("Hello stream");
  });
});
