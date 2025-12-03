import { describe, expect, test } from "bun:test";
import * as S from "effect/Schema";
import { makeGeminiRequestBody, toolToFunctionDeclaration } from "./gemini.js";
import type { Tool } from "../tools/schema.js";
import type { ChatMessage } from "./openrouter.js";

const mockTool: Tool<{ value: string }> = {
  name: "mock",
  label: "Mock",
  description: "Mock tool",
  schema: S.Struct({ value: S.String }),
  execute: () => Promise.resolve({ content: [{ type: "text", text: "ok" }] }) as any,
};

describe("toolToFunctionDeclaration", () => {
  test("converts Effect tool schema to Gemini functionDeclaration", () => {
    const def = toolToFunctionDeclaration(mockTool) as any;
    expect(def.name).toBe("mock");
    expect(def.parameters).toHaveProperty("type", "object");
    expect(def.parameters).toHaveProperty("properties");
    expect(def.parameters).not.toHaveProperty("$schema");
  });
});

describe("makeGeminiRequestBody", () => {
  const config = {
    apiKey: { _tag: "Secret", value: "" } as any,
    baseUrl: "https://generativelanguage.googleapis.com/v1beta",
    defaultModel: "gemini-2.5-flash",
  };

  test("maps tool messages to functionResponse parts", () => {
    const messages: ChatMessage[] = [
      { role: "user", content: "hi" },
      { role: "assistant", content: "", tool_calls: [{ id: "1", name: "mock", arguments: "{}" }] as any },
      { role: "tool", tool_call_id: "1", content: "ok", name: "mock" },
    ];

    const body = makeGeminiRequestBody(config, { messages, tools: [mockTool] });
    expect(body.tools?.[0]?.functionDeclarations?.[0]?.name).toBe("mock");
    expect(body.contents.some((c: any) => c.parts?.some((p: any) => p.functionResponse))).toBe(true);
    expect(body.toolConfig).toBeTruthy();
  });

  test("includes temperature/max tokens when provided", () => {
    const body = makeGeminiRequestBody(config, {
      messages: [{ role: "user", content: "ping" }],
      temperature: 0.2,
      maxTokens: 123,
    });
    expect(body.generationConfig?.temperature).toBe(0.2);
    expect(body.generationConfig?.maxOutputTokens).toBe(123);
  });
});
