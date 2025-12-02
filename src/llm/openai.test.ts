import { describe, expect, test } from "bun:test";
import * as S from "effect/Schema";
import { toolToOpenAIDefinition } from "./openai.js";
import type { Tool } from "../tools/schema.js";

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
