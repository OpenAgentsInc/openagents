import { describe, expect, test } from "bun:test";
import * as S from "effect/Schema";
import { Effect } from "effect";
import {
  toolContentToMcpContent,
  effectResultToMcpResult,
  effectErrorToMcpResult,
  effectToolToMcp,
  effectToolsToMcp,
  ToolRegistry,
  type EffectTool,
  type EffectToolExecutionError,
} from "./tool-adapter.js";

describe("tool-adapter", () => {
  const sampleTool: EffectTool<{ path: string }, { meta: string }> = {
    name: "read",
    label: "Read",
    description: "Read a file",
    schema: S.Struct({ path: S.String }),
    execute: (params) =>
      Effect.succeed({
        content: [{ type: "text", text: `read:${params.path}` }],
        details: { meta: "ok" },
      }),
  };

  test("converts tool content to MCP content", () => {
    const mcpContent = toolContentToMcpContent([
      { type: "text", text: "hello" },
      { type: "image", data: "img", mimeType: "image/png" },
    ]);

    expect(mcpContent).toEqual([
      { type: "text", text: "hello" },
      { type: "image", data: "img", mimeType: "image/png" },
    ]);
  });

  test("converts effect result and error to MCP result", () => {
    const ok = effectResultToMcpResult({
      content: [{ type: "text", text: "done" }],
    });
    expect(ok.isError).toBe(false);
    expect(ok.content[0]).toEqual({ type: "text", text: "done" });

    const err: EffectToolExecutionError = {
      _tag: "ToolExecutionError",
      reason: "not_found",
      message: "Missing file",
    };
    const errorResult = effectErrorToMcpResult(err);
    expect(errorResult.isError).toBe(true);
    expect(errorResult.content[0].text).toContain("not_found");
  });

  test("adapts Effect tool to MCP with validation and details", async () => {
    const mcpTool = effectToolToMcp(sampleTool, { includeDetails: true });
    expect(mcpTool.name).toBe("read");
    expect(() => mcpTool.inputSchema.parse({ path: 123 })).toThrow();

    const result = await mcpTool.handler({ path: "/tmp/file.txt" });
    expect(result.isError).toBe(false);
    expect(result.content[0].text).toBe("read:/tmp/file.txt");
    expect(result.content[1].text).toContain("meta");
  });

  test("uses onError fallback when tool fails", async () => {
    const failingTool: EffectTool<{ id: string }> = {
      name: "failer",
      label: "Failer",
      description: "Always fails",
      schema: S.Struct({ id: S.String }),
      execute: () =>
        Effect.fail({
          _tag: "ToolExecutionError",
          reason: "boom",
          message: "boom",
        } satisfies EffectToolExecutionError),
    };

    const mcpTool = effectToolToMcp(failingTool, {
      onError: () => ({ content: [{ type: "text", text: "fallback" }], isError: true }),
    });

    const result = await mcpTool.handler({ id: "x" });
    expect(result.isError).toBe(true);
    expect(result.content[0].text).toBe("fallback");
  });

  test("adapts multiple tools and registers in ToolRegistry", async () => {
    const mcpTools = effectToolsToMcp([sampleTool]);
    expect(mcpTools).toHaveLength(1);

    const registry = new ToolRegistry();
    registry.register(sampleTool);
    expect(registry.has("read")).toBe(true);
    const tool = registry.get("read");
    const result = await tool?.handler({ path: "/a" });
    expect(result?.content[0].text).toBe("read:/a");
    expect(registry.getNames()).toEqual(["read"]);
  });
});
