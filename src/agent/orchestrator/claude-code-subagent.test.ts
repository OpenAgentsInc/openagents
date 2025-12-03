import { describe, expect, test } from "bun:test";
import { CLAUDE_CODE_MCP_SERVER_NAME, getAllowedClaudeCodeTools } from "./claude-code-mcp.js";
import { runClaudeCodeSubagent } from "./claude-code-subagent.js";

const makeSubtask = () => ({
  id: "sub-1",
  description: "Do the thing",
  status: "pending" as const,
});

const makeQuery = (messages: any[]) =>
  async function* query() {
    for (const message of messages) {
      yield message;
    }
  };

describe("runClaudeCodeSubagent", () => {
  test("captures file modifications and success result", async () => {
    const queryFn = makeQuery([
      { type: "assistant", tool_calls: [{ name: "Edit", input: { file_path: "a.ts" } }] },
      { type: "assistant", tool_calls: [{ name: "Write", input: { file_path: "b.ts" } }] },
      { type: "result", subtype: "success", turns: 5 },
    ]);

    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
    });

    expect(result.success).toBe(true);
    expect(result.filesModified.sort()).toEqual(["a.ts", "b.ts"]);
    expect(result.turns).toBe(5);
  });

  test("surfaces failure subtype as error", async () => {
    const queryFn = makeQuery([{ type: "result", subtype: "failure" }]);

    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
    });

    expect(result.success).toBe(false);
    expect(result.error).toContain("Claude Code finished with: failure");
  });

  test("handles thrown errors", async () => {
    const queryFn = async function* () {
      throw new Error("boom");
    };

    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
    });

    expect(result.success).toBe(false);
    expect(result.error).toContain("boom");
  });

  test("passes MCP server and allowed tools to Claude Code query", async () => {
    const inputs: any[] = [];
    const queryFn = async function* (input: any) {
      inputs.push(input);
      yield { type: "result", subtype: "success" };
    };

    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      openagentsDir: "/tmp/.openagents",
      queryFn,
    });

    expect(result.success).toBe(true);
    const options = inputs[0]?.options;
    expect(options?.mcpServers?.[CLAUDE_CODE_MCP_SERVER_NAME]).toBeDefined();
    expect(options?.allowedTools).toEqual(expect.arrayContaining(getAllowedClaudeCodeTools()));
  });
});
