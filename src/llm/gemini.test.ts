import { describe, expect, test } from "bun:test";
import * as S from "effect/Schema";
import { toolToFunctionDeclaration } from "./gemini.js";
import type { Tool } from "../tools/schema.js";

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
