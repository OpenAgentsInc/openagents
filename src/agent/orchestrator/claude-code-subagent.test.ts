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
  test("captures session id and exposes it for resumption", async () => {
    const queryFn = makeQuery([
      { type: "system", subtype: "init", session_id: "sess-123", model: "sonnet", tools: [] },
      { type: "result", subtype: "success", session_id: "sess-123" },
    ]);

    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
    });

    expect(result.claudeCodeSessionId).toBe("sess-123");
    expect(result.sessionMetadata?.sessionId).toBe("sess-123");
  });

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
    expect(result.agent).toBe("claude-code");
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

  test("loads project settings to provide CLAUDE.md context", async () => {
    const inputs: any[] = [];
    const queryFn = async function* (input: any) {
      inputs.push(input);
      yield { type: "result", subtype: "success" };
    };

    await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
    });

    expect(inputs[0]?.options?.settingSources).toEqual(["project"]);
  });

  test("resumes and optionally forks a prior session", async () => {
    const inputs: any[] = [];
    const queryFn = async function* (input: any) {
      inputs.push(input);
      yield { type: "system", subtype: "init", session_id: "sess-new" };
      yield { type: "result", subtype: "success", session_id: "sess-new" };
    };

    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
      resumeSessionId: "sess-old",
      forkSession: true,
    });

    expect(inputs[0]?.options?.resume).toBe("sess-old");
    expect(inputs[0]?.options?.forkSession).toBe(true);
    expect(result.claudeCodeSessionId).toBe("sess-new");
    expect(result.claudeCodeForkedFromSessionId).toBe("sess-old");
    expect(result.sessionMetadata?.forkedFromSessionId).toBe("sess-old");
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

  test("records permission denials for recovery", async () => {
    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn: makeQuery([
        {
          type: "result",
          subtype: "success",
          permission_denials: [{ tool_name: "Edit", tool_use_id: "1", tool_input: { path: "secret.txt" } }],
        },
      ]),
    });

    expect(result.success).toBe(true);
    expect(result.sessionMetadata?.blockers?.some((b) => b.includes("Permission denied for Edit"))).toBe(true);
    expect(
      result.sessionMetadata?.suggestedNextSteps?.some((s) => s.toLowerCase().includes("permissions"))
    ).toBe(true);
  });

  test("captures token usage and cost from result message", async () => {
    const queryFn = makeQuery([
      { type: "assistant", tool_calls: [{ name: "Edit", input: { file_path: "a.ts" } }] },
      {
        type: "result",
        subtype: "success",
        turns: 5,
        usage: {
          input_tokens: 1000,
          output_tokens: 500,
          cache_read_input_tokens: 200,
          cache_creation_input_tokens: 50,
        },
        total_cost_usd: 0.0123,
      },
    ]);

    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
    });

    expect(result.success).toBe(true);
    expect(result.sessionMetadata?.usage).toEqual({
      inputTokens: 1000,
      outputTokens: 500,
      cacheReadInputTokens: 200,
      cacheCreationInputTokens: 50,
    });
    expect(result.sessionMetadata?.totalCostUsd).toBe(0.0123);
  });

  test("handles camelCase usage fields from SDK", async () => {
    const queryFn = makeQuery([
      {
        type: "result",
        subtype: "success",
        usage: {
          inputTokens: 800,
          outputTokens: 400,
          cacheReadInputTokens: 100,
          cacheCreationInputTokens: 25,
        },
        total_cost_usd: 0.0098,
      },
    ]);

    const result = await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
    });

    expect(result.success).toBe(true);
    expect(result.sessionMetadata?.usage).toEqual({
      inputTokens: 800,
      outputTokens: 400,
      cacheReadInputTokens: 100,
      cacheCreationInputTokens: 25,
    });
  });

  test("accepts onEvent callback for orchestrator integration", async () => {
    // Note: Events are only emitted via SDK hooks in production, not test mocks
    const events: any[] = [];
    const onEvent = (event: any) => events.push(event);

    const queryFn = makeQuery([
      { type: "result", subtype: "success" },
    ]);

    await runClaudeCodeSubagent(makeSubtask(), {
      cwd: "/tmp",
      queryFn,
      onEvent,
    });

    // With mock queryFn, hooks aren't invoked, so events array is empty
    // This test just verifies the option is accepted without error
    expect(onEvent).toBeDefined();
  });
});
