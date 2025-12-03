import { describe, expect, test } from "bun:test";
import { mkdtempSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  CLAUDE_CODE_MCP_SERVER_NAME,
  buildMechaCoderMcpTools,
  getAllowedClaudeCodeTools,
} from "./claude-code-mcp.js";
import { createEmptyProgress, writeProgress } from "./progress.js";

const getTool = (name: string) => buildMechaCoderMcpTools().find((toolDef) => toolDef.name === name)!;

describe("claude-code-mcp tools", () => {
  test("read_progress returns placeholder when path missing", async () => {
    const tool = getTool("read_progress");
    const result = await tool.handler(
      { issue: "none", summary: "n/a", filesModified: [] } as any,
      {} as any
    );

    expect(result.content?.[0]?.type).toBe("text");
    expect(result.content?.[0]?.text).toContain("openagentsDir not provided");
  });

  test("read_progress returns file content when available", async () => {
    const openagentsDir = mkdtempSync(join(tmpdir(), "claude-mcp-"));
    writeProgress(openagentsDir, createEmptyProgress("session-1", "task-1", "Task title"));

    const tool = buildMechaCoderMcpTools({ openagentsDir }).find((toolDef) => toolDef.name === "read_progress")!;
    const result = await tool.handler(
      { issue: "none", summary: "n/a", filesModified: [] } as any,
      {} as any
    );

    expect(result.content?.[0]?.text).toContain("session-1");
    expect(result.content?.[0]?.text).toContain("task-1");
  });

  test("subtask_complete echoes summary and files", async () => {
    const tool = getTool("subtask_complete");
    const result = await tool.handler(
      { summary: "Updated tests", filesModified: ["a.ts", "b.ts"] } as any,
      {} as any
    );

    expect(result.content?.[0]?.text).toContain("Updated tests");
    expect(result.content?.[0]?.text).toContain("a.ts");
    expect(result.content?.[0]?.text).toContain("b.ts");
  });

  test("allowed tools include MCP entries", () => {
    const allowed = getAllowedClaudeCodeTools();

    expect(allowed).toEqual(
      expect.arrayContaining([
        `mcp__${CLAUDE_CODE_MCP_SERVER_NAME}__subtask_complete`,
        `mcp__${CLAUDE_CODE_MCP_SERVER_NAME}__request_help`,
        `mcp__${CLAUDE_CODE_MCP_SERVER_NAME}__read_progress`,
      ])
    );
  });
});
