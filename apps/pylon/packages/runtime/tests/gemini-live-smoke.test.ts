import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { defineProbeLlmTool, makeGeminiClient, makeProbeLlmRequest, type ProbeLlmTools } from "../src";

const apiKey = process.env.GOOGLE_GENERATIVE_AI_API_KEY ?? process.env.GEMINI_API_KEY;
const liveSmokeEnabled = process.env.PROBE_GEMINI_LIVE_SMOKE === "1" && apiKey !== undefined && apiKey.trim() !== "";
const liveTest = liveSmokeEnabled ? test : test.skip;

describe("Gemini live smoke", () => {
  liveTest("completes a plain prompt through gemini-3.5-flash", async () => {
    const client = await Effect.runPromise(makeGeminiClient({ apiKey, env: process.env }));
    const result = await Effect.runPromise(
      client.complete({
        request: makeProbeLlmRequest({
          model: { provider: "google", model: "gemini-3.5-flash" },
          prompt: "Reply with exactly: probe-live-smoke-ok",
          generation: { maxTokens: 32, temperature: 0 },
        }),
      }),
    );

    expect(result.text.trim().length).toBeGreaterThan(0);
    expect(JSON.stringify({ apiKey: client.apiKey, receipt: result.receipt })).not.toContain(apiKey);
  });

  liveTest("can force and continue a tiny Gemini native tool call", async () => {
    const tools: ProbeLlmTools = {
      echo: defineProbeLlmTool({
        name: "echo",
        description: "Return the input value.",
        inputSchema: {
          type: "object",
          properties: { value: { type: "string" } },
          required: ["value"],
        },
        execute: (input) => Effect.succeed({ value: String(input.value) }),
      }),
    };
    const client = await Effect.runPromise(makeGeminiClient({ apiKey, env: process.env }));
    const result = await Effect.runPromise(
      client.complete({
        request: makeProbeLlmRequest({
          model: { provider: "google", model: "gemini-3.5-flash" },
          prompt: "Call echo with value exactly probe-live-smoke, then answer done.",
          tools: [
            {
              name: "echo",
              description: "Return the input value.",
              inputSchema: {
                type: "object",
                properties: { value: { type: "string" } },
                required: ["value"],
              },
            },
          ],
          toolChoice: { type: "tool", name: "echo" },
          generation: { maxTokens: 128, temperature: 0 },
        }),
        tools,
        maxModelRoundTrips: 3,
      }),
    );

    expect(result.events.map((event) => event.type)).toContain("tool-call");
    expect(result.events.map((event) => event.type)).toContain("tool-result");
    expect(result.text.trim().length).toBeGreaterThan(0);
    expect(JSON.stringify({ apiKey: client.apiKey, receipt: result.receipt, toolReceipts: result.toolReceipts })).not.toContain(
      apiKey,
    );
  });
});
