import { describe, test, expect } from "bun:test";
import {
  mcpContentToToolContent,
  mcpContentsToToolContent,
  mcpResultToToolResult,
  isMcpErrorResult,
  getMcpErrorMessage,
  toolResultToMcpResult,
  createMcpErrorResult,
  subagentResultToSdk,
  sdkSubagentResultToInternal,
  subagentResultToMcp,
  createTextResult,
  createResult,
  mergeResults,
  getResultText,
  resultHasImages,
} from "./result-adapter.js";
import type { ToolResult, ToolContent } from "../../../tools/schema.js";
import type { SubagentResult } from "../../../agent/orchestrator/types.js";
import type { McpCallToolResult, McpContentItem } from "./tool-adapter.js";

describe("result-adapter", () => {
  describe("MCP content conversion", () => {
    test("converts text MCP content to internal", () => {
      const mcp: McpContentItem = { type: "text", text: "hello" };
      const internal = mcpContentToToolContent(mcp);
      expect(internal).toEqual({ type: "text", text: "hello" });
    });

    test("converts image MCP content to internal", () => {
      const mcp: McpContentItem = {
        type: "image",
        data: "base64data",
        mimeType: "image/png",
      };
      const internal = mcpContentToToolContent(mcp);
      expect(internal).toEqual({
        type: "image",
        data: "base64data",
        mimeType: "image/png",
      });
    });

    test("converts resource MCP content to text fallback", () => {
      const mcp: McpContentItem = { type: "resource", text: "resource content" };
      const internal = mcpContentToToolContent(mcp);
      expect(internal).toEqual({ type: "text", text: "resource content" });
    });

    test("converts array of MCP contents", () => {
      const mcpItems: McpContentItem[] = [
        { type: "text", text: "line 1" },
        { type: "text", text: "line 2" },
      ];
      const internal = mcpContentsToToolContent(mcpItems);
      expect(internal).toHaveLength(2);
      expect(internal[0]).toEqual({ type: "text", text: "line 1" });
    });
  });

  describe("MCP result conversion", () => {
    test("converts MCP result to internal ToolResult", () => {
      const mcp: McpCallToolResult = {
        content: [{ type: "text", text: "success" }],
        isError: false,
      };
      const result = mcpResultToToolResult(mcp);
      expect(result.content).toHaveLength(1);
      expect(result.content[0]).toEqual({ type: "text", text: "success" });
    });

    test("converts MCP result with details", () => {
      const mcp: McpCallToolResult = {
        content: [{ type: "text", text: "done" }],
      };
      const result = mcpResultToToolResult(mcp, { linesChanged: 10 });
      expect(result.details).toEqual({ linesChanged: 10 });
    });

    test("detects MCP error results", () => {
      const success: McpCallToolResult = {
        content: [{ type: "text", text: "ok" }],
        isError: false,
      };
      const error: McpCallToolResult = {
        content: [{ type: "text", text: "failed" }],
        isError: true,
      };
      expect(isMcpErrorResult(success)).toBe(false);
      expect(isMcpErrorResult(error)).toBe(true);
    });

    test("extracts error message from MCP error result", () => {
      const error: McpCallToolResult = {
        content: [{ type: "text", text: "File not found" }],
        isError: true,
      };
      expect(getMcpErrorMessage(error)).toBe("File not found");
    });

    test("returns undefined for non-error result", () => {
      const success: McpCallToolResult = {
        content: [{ type: "text", text: "ok" }],
        isError: false,
      };
      expect(getMcpErrorMessage(success)).toBeUndefined();
    });
  });

  describe("internal to MCP conversion", () => {
    test("converts internal ToolResult to MCP", () => {
      const internal: ToolResult = {
        content: [{ type: "text", text: "result" }],
      };
      const mcp = toolResultToMcpResult(internal);
      expect(mcp.content).toHaveLength(1);
      expect(mcp.content[0]).toEqual({ type: "text", text: "result" });
      expect(mcp.isError).toBe(false);
    });

    test("includes details when option is set", () => {
      const internal: ToolResult<{ count: number }> = {
        content: [{ type: "text", text: "done" }],
        details: { count: 5 },
      };
      const mcp = toolResultToMcpResult(internal, { includeDetails: true });
      expect(mcp.content).toHaveLength(2);
      expect(mcp.content[1].text).toContain("count");
    });

    test("creates MCP error result", () => {
      const error = createMcpErrorResult("Something went wrong", "not_found");
      expect(error.isError).toBe(true);
      expect(error.content[0].text).toBe("[not_found] Something went wrong");
    });

    test("creates MCP error result without reason", () => {
      const error = createMcpErrorResult("Generic error");
      expect(error.isError).toBe(true);
      expect(error.content[0].text).toBe("Generic error");
    });
  });

  describe("SubagentResult conversion", () => {
    const internalResult: SubagentResult = {
      success: true,
      subtaskId: "st-123",
      filesModified: ["src/file.ts", "src/other.ts"],
      turns: 5,
      tokenUsage: { input: 1000, output: 500 },
    };

    test("converts SubagentResult to SDK format", () => {
      const sdk = subagentResultToSdk(internalResult);
      expect(sdk.success).toBe(true);
      expect(sdk.subtask_id).toBe("st-123");
      expect(sdk.files_modified).toEqual(["src/file.ts", "src/other.ts"]);
      expect(sdk.turns).toBe(5);
      expect(sdk.usage).toEqual({ input_tokens: 1000, output_tokens: 500 });
    });

    test("converts SubagentResult with summary", () => {
      const sdk = subagentResultToSdk(internalResult, "Refactored the module");
      expect(sdk.summary).toBe("Refactored the module");
    });

    test("converts failed SubagentResult", () => {
      const failed: SubagentResult = {
        success: false,
        subtaskId: "st-456",
        filesModified: [],
        turns: 2,
        error: "Max turns exceeded",
      };
      const sdk = subagentResultToSdk(failed);
      expect(sdk.success).toBe(false);
      expect(sdk.error).toBe("Max turns exceeded");
    });

    test("converts SDK SubagentResult to internal", () => {
      const sdk = {
        success: true,
        subtask_id: "st-789",
        files_modified: ["a.ts"],
        turns: 3,
        usage: { input_tokens: 500, output_tokens: 200 },
      };
      const internal = sdkSubagentResultToInternal(sdk);
      expect(internal.success).toBe(true);
      expect(internal.subtaskId).toBe("st-789");
      expect(internal.filesModified).toEqual(["a.ts"]);
      expect(internal.tokenUsage).toEqual({ input: 500, output: 200 });
    });

    test("converts SubagentResult to MCP format", () => {
      const mcp = subagentResultToMcp(internalResult);
      expect(mcp.isError).toBe(false);
      expect(mcp.content[0].text).toContain("completed successfully");
      expect(mcp.content[0].text).toContain("src/file.ts");
    });

    test("converts failed SubagentResult to MCP", () => {
      const failed: SubagentResult = {
        success: false,
        subtaskId: "st-fail",
        filesModified: [],
        turns: 1,
        error: "Tool execution failed",
      };
      const mcp = subagentResultToMcp(failed);
      expect(mcp.isError).toBe(true);
      expect(mcp.content[0].text).toContain("failed");
      expect(mcp.content[0].text).toContain("Tool execution failed");
    });
  });

  describe("utility functions", () => {
    test("createTextResult creates result with text", () => {
      const result = createTextResult("Hello world");
      expect(result.content).toHaveLength(1);
      expect(result.content[0]).toEqual({ type: "text", text: "Hello world" });
    });

    test("createTextResult with details", () => {
      const result = createTextResult("Done", { count: 10 });
      expect(result.details).toEqual({ count: 10 });
    });

    test("createResult with multiple contents", () => {
      const contents: ToolContent[] = [
        { type: "text", text: "line 1" },
        { type: "text", text: "line 2" },
      ];
      const result = createResult(contents);
      expect(result.content).toHaveLength(2);
    });

    test("mergeResults combines multiple results", () => {
      const r1: ToolResult = { content: [{ type: "text", text: "a" }] };
      const r2: ToolResult = { content: [{ type: "text", text: "b" }] };
      const merged = mergeResults([r1, r2]);
      expect(merged.content).toHaveLength(2);
      expect(merged.content[0]).toEqual({ type: "text", text: "a" });
      expect(merged.content[1]).toEqual({ type: "text", text: "b" });
    });

    test("mergeResults with merged details", () => {
      const r1: ToolResult = { content: [{ type: "text", text: "a" }] };
      const r2: ToolResult = { content: [{ type: "text", text: "b" }] };
      const merged = mergeResults([r1, r2], { total: 2 });
      expect(merged.details).toEqual({ total: 2 });
    });

    test("getResultText extracts all text content", () => {
      const result: ToolResult = {
        content: [
          { type: "text", text: "line 1" },
          { type: "image", data: "...", mimeType: "image/png" },
          { type: "text", text: "line 2" },
        ],
      };
      const text = getResultText(result);
      expect(text).toBe("line 1\nline 2");
    });

    test("resultHasImages detects images", () => {
      const textOnly: ToolResult = {
        content: [{ type: "text", text: "no images" }],
      };
      const withImage: ToolResult = {
        content: [
          { type: "text", text: "with image" },
          { type: "image", data: "...", mimeType: "image/png" },
        ],
      };
      expect(resultHasImages(textOnly)).toBe(false);
      expect(resultHasImages(withImage)).toBe(true);
    });
  });
});
