import { describe, expect, test } from "bun:test";
import * as S from "effect/Schema";
import { buildOpenAIRequestBody, toolToOpenAIDefinition } from "./openai.js";
import type { Tool } from "../tools/schema.js";
import type { ChatMessage } from "./openrouter.js";

const mockTool: Tool<{ value: string }> = {
  name: "mock",
  label: "Mock",
  description: "Mock tool",
  schema: S.Struct({ value: S.String }),
  execute: () => Promise.resolve({ content: [{ type: "text", text: "ok" }] }) as any,
};

describe("toolToOpenAIDefinition", () => {
  test("converts Effect tool schema to OpenAI function definition", () => {
    const def = toolToOpenAIDefinition(mockTool) as any;
    expect(def.type).toBe("function");
    expect(def.function.name).toBe("mock");
    expect(def.function.parameters).toHaveProperty("type", "object");
    expect(def.function.parameters).toHaveProperty("properties");
    expect(def.function.parameters).not.toHaveProperty("$schema");
  });
});

describe("buildOpenAIRequestBody", () => {
  const config = {
    apiKey: { _tag: "Secret", value: "" } as any,
    baseUrl: "https://api.openai.com/v1",
    defaultModel: "gpt-4o-mini",
  };

  test("maps tool role to tool_call_id shape", () => {
    const messages: ChatMessage[] = [
      { role: "user", content: "hi" },
      { role: "assistant", content: "" },
      { role: "tool", tool_call_id: "1", content: "ok", name: "mock" },
    ];

    const body = buildOpenAIRequestBody(config, { messages, tools: [mockTool] });
    expect(body.messages[2]).toMatchObject({ role: "tool", tool_call_id: "1", name: "mock" });
    expect(body.tool_choice).toBe("auto");
  });

  test("respects explicit model and temperature", () => {
    const body = buildOpenAIRequestBody(config, {
      model: "custom-model",
      messages: [{ role: "user", content: "ping" }],
      temperature: 0.3,
      maxTokens: 123,
    });

    expect(body.model).toBe("custom-model");
    expect(body.temperature).toBe(0.3);
    expect(body.max_tokens).toBe(123);
  });
});
