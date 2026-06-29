import { describe, expect, test } from "bun:test";
import {
  convertProbeToolSchemaToGemini,
  geminiEndpointPath,
  lowerProbeLlmRequestToGeminiBody,
  makeProbeLlmMessage,
  makeProbeLlmRequest,
  makeProbeLlmToolResult,
  type ProbeLlmToolDefinition,
} from "../src";

const lookupTool: ProbeLlmToolDefinition = {
  name: "lookup",
  description: "Lookup data",
  inputSchema: {
    type: "object",
    properties: {
      query: { type: "string" },
    },
    required: ["query"],
  },
};

describe("Gemini request lowering", () => {
  test("prepares Gemini target and plain prompt body", () => {
    const body = lowerProbeLlmRequestToGeminiBody(
      makeProbeLlmRequest({
        model: { provider: "google", model: "gemini-3.5-flash" },
        system: "You are concise.",
        prompt: "Say hello.",
        generation: { maxTokens: 20, temperature: 0 },
      }),
    );

    expect(geminiEndpointPath("gemini-3.5-flash")).toBe("/models/gemini-3.5-flash:streamGenerateContent?alt=sse");
    expect(body).toEqual({
      contents: [{ role: "user", parts: [{ text: "Say hello." }] }],
      systemInstruction: { parts: [{ text: "You are concise." }] },
      generationConfig: { maxOutputTokens: 20, temperature: 0 },
    });
  });

  test("lowers chronological system updates as wrapped user text", () => {
    const body = lowerProbeLlmRequestToGeminiBody(
      makeProbeLlmRequest({
        model: { provider: "google", model: "gemini-3.5-flash" },
        messages: [
          makeProbeLlmMessage("user", "Before."),
          makeProbeLlmMessage("system", "Update."),
          makeProbeLlmMessage("assistant", "After."),
        ],
      }),
    );

    expect(body.contents).toEqual([
      { role: "user", parts: [{ text: "Before." }, { text: "<system-update>\nUpdate.\n</system-update>" }] },
      { role: "model", parts: [{ text: "After." }] },
    ]);
  });

  test("prepares multimodal input and tool history", () => {
    const body = lowerProbeLlmRequestToGeminiBody(
      makeProbeLlmRequest({
        model: { provider: "google", model: "gemini-3.5-flash" },
        tools: [lookupTool],
        toolChoice: { type: "tool", name: "lookup" },
        messages: [
          makeProbeLlmMessage("user", [
            { type: "text", text: "What is in this image?" },
            { type: "media", mediaType: "image/png", data: "AAECAw==" },
          ]),
          makeProbeLlmMessage("assistant", [
            { type: "tool-call", id: "call_1", name: "lookup", input: { query: "weather" } },
          ]),
          makeProbeLlmMessage("tool", [
            makeProbeLlmToolResult({ id: "call_1", name: "lookup", result: { forecast: "sunny" } }),
          ]),
        ],
      }),
    );

    expect(body).toEqual({
      contents: [
        {
          role: "user",
          parts: [{ text: "What is in this image?" }, { inlineData: { mimeType: "image/png", data: "AAECAw==" } }],
        },
        {
          role: "model",
          parts: [{ functionCall: { name: "lookup", args: { query: "weather" } } }],
        },
        {
          role: "user",
          parts: [
            { functionResponse: { name: "lookup", response: { name: "lookup", content: "{\"forecast\":\"sunny\"}" } } },
          ],
        },
      ],
      tools: [
        {
          functionDeclarations: [
            {
              name: "lookup",
              description: "Lookup data",
              parameters: { type: "object", properties: { query: { type: "string" } }, required: ["query"] },
            },
          ],
        },
      ],
      toolConfig: { functionCallingConfig: { mode: "ANY", allowedFunctionNames: ["lookup"] } },
    });
  });

  test("omits tools when tool choice is none", () => {
    const body = lowerProbeLlmRequestToGeminiBody(
      makeProbeLlmRequest({
        model: { provider: "google", model: "gemini-3.5-flash" },
        prompt: "Say hello.",
        tools: [lookupTool],
        toolChoice: { type: "none" },
      }),
    );

    expect(body).toEqual({
      contents: [{ role: "user", parts: [{ text: "Say hello." }] }],
    });
  });

  test("passes Gemini thinking config through provider options", () => {
    const body = lowerProbeLlmRequestToGeminiBody(
      makeProbeLlmRequest({
        model: { provider: "google", model: "gemini-3.5-flash" },
        prompt: "Think briefly.",
        providerOptions: {
          gemini: {
            thinkingConfig: {
              thinkingBudget: 128,
              includeThoughts: true,
            },
          },
        },
      }),
    );

    expect(body.generationConfig).toEqual({
      thinkingConfig: {
        thinkingBudget: 128,
        includeThoughts: true,
      },
    });
  });

  test("sanitizes Gemini tool schemas", () => {
    expect(
      convertProbeToolSchemaToGemini({
        type: "object",
        required: ["status", "missing"],
        properties: {
          status: { type: "integer", enum: [1, 2] },
          tags: { type: "array" },
          name: { type: "string", properties: { ignored: { type: "string" } }, required: ["ignored"] },
          nullable: { type: ["string", "null"] },
          fixed: { const: "yes" },
          dropped: { type: "object", additionalProperties: true },
        },
      }),
    ).toEqual({
      type: "object",
      required: ["status"],
      properties: {
        status: { type: "string", enum: ["1", "2"] },
        tags: { type: "array", items: { type: "string" } },
        name: { type: "string" },
        nullable: { type: "string", nullable: true },
        fixed: { enum: ["yes"] },
        dropped: { type: "object" },
      },
    });
  });
});
