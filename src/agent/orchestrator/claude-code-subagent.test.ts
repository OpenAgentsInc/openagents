import { describe, expect, test } from "bun:test";
import { AbortError } from "@anthropic-ai/claude-agent-sdk";
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

  test("passes permission mode when provided", async () => {
    const inputs: any[] = [];
    const queryFn = async function* (input: any) {
      inputs.push(input);
      yield { type: "result", subtype: "success" };
    };

    await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      permissionMode: "bypassPermissions",
      queryFn,
    });

    expect(inputs[0]?.options?.permissionMode).toBe("bypassPermissions");
  });

  test("retries on rate_limit error with exponential backoff", async () => {
    let attemptCount = 0;
    const queryFn = async function* () {
      attemptCount++;
      if (attemptCount === 1) {
        // First attempt: rate limit error
        yield { error: "rate_limit" };
      } else {
        // Second attempt: success
        yield { type: "result", subtype: "success" };
      }
    };

    const startTime = Date.now();
    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
    });
    const elapsed = Date.now() - startTime;

    expect(result.success).toBe(true);
    expect(attemptCount).toBe(2);
    expect(elapsed).toBeGreaterThanOrEqual(1000); // At least 1 second delay
    expect(result.sessionMetadata?.blockers?.some(b => b.includes("rate limit"))).toBe(true);
  });

  test("retries on server_error", async () => {
    let attemptCount = 0;
    const queryFn = async function* () {
      attemptCount++;
      if (attemptCount === 1) {
        yield { error: "server_error" };
      } else {
        yield { type: "result", subtype: "success" };
      }
    };

    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
    });

    expect(result.success).toBe(true);
    expect(attemptCount).toBe(2);
    expect(result.sessionMetadata?.blockers?.some(b => b.includes("server error"))).toBe(true);
  });

  test("stops immediately on authentication_failed error", async () => {
    let attemptCount = 0;
    const queryFn = async function* () {
      attemptCount++;
      yield { error: "authentication_failed" };
      yield { type: "result", subtype: "success" };
    };

    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
    });

    expect(result.success).toBe(false);
    expect(attemptCount).toBe(1); // No retry
    expect(result.error).toBeTruthy();
    expect(result.sessionMetadata?.blockers?.some(b =>
      b.includes("authentication") || b.toLowerCase().includes("auth")
    )).toBe(true);
  });

  test("stops immediately on billing_error", async () => {
    let attemptCount = 0;
    const queryFn = async function* () {
      attemptCount++;
      yield { error: "billing_error" };
    };

    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
    });

    expect(result.success).toBe(false);
    expect(attemptCount).toBe(1); // No retry
    expect(result.error).toBeTruthy();
    expect(result.sessionMetadata?.blockers?.some(b =>
      b.includes("billing") || b.includes("authentication") || b.toLowerCase().includes("auth")
    )).toBe(true);
  });

  test("exhausts retries and signals fallback", async () => {
    let attemptCount = 0;
    const queryFn = async function* () {
      attemptCount++;
      // Always fail with rate_limit
      yield { error: "rate_limit" };
    };

    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
    });

    expect(result.success).toBe(false);
    expect(attemptCount).toBe(4); // Initial + 3 retries
    expect(result.error).toBeTruthy();
    // Should have logged multiple retry attempts
    expect(result.sessionMetadata?.blockers?.length).toBeGreaterThan(3);
  }, 10000); // 10 second timeout for exponential backoff (1s + 2s + 4s = 7s)

  test("retries on network error exceptions", async () => {
    let attemptCount = 0;
    const queryFn = async function* () {
      attemptCount++;
      if (attemptCount === 1) {
        throw new Error("Network timeout");
      } else {
        yield { type: "result", subtype: "success" };
      }
    };

    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
    });

    expect(result.success).toBe(true);
    expect(attemptCount).toBe(2);
    expect(result.sessionMetadata?.blockers?.some(b => b.includes("network error"))).toBe(true);
  });

  test("propagates authentication errors with recovery hints", async () => {
    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn: makeQuery([{ type: "assistant", error: "authentication_failed" }]),
    });

    expect(result.success).toBe(false);
    expect(result.error?.toLowerCase()).toContain("authentication");
    expect(result.sessionMetadata?.blockers?.some((b) => b.toLowerCase().includes("authentication"))).toBe(true);
    expect(result.sessionMetadata?.suggestedNextSteps?.some((s) => s.includes("ANTHROPIC_API_KEY"))).toBe(true);
  });

  test("aborts long-running sessions via timeout", async () => {
    const queryFn = async function* (input: any) {
      const signal: AbortSignal | undefined = input?.options?.abortController?.signal;
      await new Promise((_, reject) => {
        signal?.addEventListener("abort", () => reject(new AbortError("timeout")));
      });
    };

    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
      timeoutMs: 10,
    });

    expect(result.success).toBe(false);
    expect(result.error).toContain("timed out");
    expect(result.sessionMetadata?.blockers?.some((b) => b.includes("timed out"))).toBe(true);
  });

  test("handles API rate limits with suggested recovery", async () => {
    const queryFn = async function* () {
      const err = new Error("rate limited");
      (err as any).status = 429;
      throw err;
    };

    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
    });

    expect(result.success).toBe(false);
    expect(result.error).toContain("rate limited");
    expect(result.sessionMetadata?.suggestedNextSteps?.some((s) => s.toLowerCase().includes("rate limit"))).toBe(true);
  });
});
