import { describe, expect, test } from "bun:test";
import { Effect, Layer } from "effect";
import { runBestAvailableSubagent, shouldUseClaudeCode } from "./subagent-router.js";
import type { SubagentResult, Subtask } from "./types.js";
import { OpenRouterClient, type OpenRouterClientShape } from "../../llm/openrouter.js";

// Mock OpenRouterClient for tests
const mockClient: OpenRouterClientShape = {
  chat: () => Effect.fail(new Error("Mock client should not be called in routing tests")),
};
const MockOpenRouterClient = Layer.succeed(OpenRouterClient, mockClient);

const makeSubtask = (description = "Refactor multi-file module"): Subtask => ({
  id: "sub-1",
  description,
  status: "pending",
});

const minimalResult: SubagentResult = {
  success: true,
  subtaskId: "sub-1",
  filesModified: ["c.ts"],
  turns: 1,
};

describe("shouldUseClaudeCode", () => {
  test("defaults to enabled but respects explicit disable", () => {
    expect(shouldUseClaudeCode(makeSubtask(), undefined)).toBe(true);
    expect(shouldUseClaudeCode(makeSubtask(), { enabled: false })).toBe(false);
  });

  test("enables for complex tasks by keyword or length", () => {
    expect(shouldUseClaudeCode(makeSubtask("Simple update"), { enabled: true })).toBe(false);
    expect(shouldUseClaudeCode(makeSubtask("Refactor component for multi-file flow"), { enabled: true })).toBe(true);
    expect(shouldUseClaudeCode(makeSubtask("a".repeat(400)), { enabled: true })).toBe(true);
  });

  test("uses Claude Code for all tasks when preferForComplexTasks is false", () => {
    expect(shouldUseClaudeCode(makeSubtask("Simple update"), { enabled: true, preferForComplexTasks: false })).toBe(true);
  });
});

describe("runBestAvailableSubagent", () => {
  test("prefers Claude Code when available", async () => {
    let minimalCalled = false;
    let claudeCalled = false;

    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask(),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: true },
        detectClaudeCodeFn: async () => ({ available: true }),
        runClaudeCodeFn: async (subtask) => {
          claudeCalled = true;
          return { success: true, subtaskId: subtask.id, filesModified: ["a.ts"], turns: 2 };
        },
        runMinimalSubagent: () => {
          minimalCalled = true;
          return Effect.succeed(minimalResult);
        },
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(result.success).toBe(true);
    expect(claudeCalled).toBe(true);
    expect(minimalCalled).toBe(false);
  });

  test("falls back to minimal subagent when Claude Code fails and fallback is enabled", async () => {
    let minimalCalled = false;

    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask(),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: true, fallbackToMinimal: true },
        detectClaudeCodeFn: async () => ({ available: true }),
        runClaudeCodeFn: async (subtask) => ({
          success: false,
          subtaskId: subtask.id,
          filesModified: [],
          turns: 1,
          error: "fail",
        }),
        runMinimalSubagent: () => {
          minimalCalled = true;
          return Effect.succeed(minimalResult);
        },
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(result.success).toBe(true);
    expect(minimalCalled).toBe(true);
  });

  test("verifies Claude Code result and falls back when verification fails", async () => {
    let minimalCalled = 0;
    const verifyCalls: Array<{ commands: string[]; cwd: string }> = [];

    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask(),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: true },
        verificationCommands: ["bun test"],
        verifyFn: async (commands, cwd) => {
          verifyCalls.push({ commands, cwd });
          return {
            passed: verifyCalls.length > 1,
            outputs: ["failing tests"],
          };
        },
        detectClaudeCodeFn: async () => ({ available: true }),
        runClaudeCodeFn: async (subtask) => ({
          success: true,
          subtaskId: subtask.id,
          filesModified: ["cc.ts"],
          turns: 2,
        }),
        runMinimalSubagent: () => {
          minimalCalled++;
          return Effect.succeed({
            success: true,
            subtaskId: "sub-1",
            filesModified: ["fix.ts"],
            turns: 1,
          });
        },
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(minimalCalled).toBe(1);
    expect(verifyCalls.length).toBe(2);
    expect(result.success).toBe(true);
    expect(result.agent).toBe("minimal");
    expect(result.filesModified.sort()).toEqual(["cc.ts", "fix.ts"]);
  });

  test("surfaces verification failure when fallback is disabled", async () => {
    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask(),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: true, fallbackToMinimal: false },
        verificationCommands: ["bun test"],
        verifyFn: async () => ({ passed: false, outputs: ["tests failed"] }),
        detectClaudeCodeFn: async () => ({ available: true }),
        runClaudeCodeFn: async (subtask) => ({
          success: true,
          subtaskId: subtask.id,
          filesModified: ["cc.ts"],
          turns: 1,
        }),
        runMinimalSubagent: () => Effect.succeed(minimalResult),
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(result.success).toBe(false);
    expect(result.agent).toBe("claude-code");
    expect(result.error).toContain("Verification failed");
    expect(result.verificationOutputs).toEqual(["tests failed"]);
  });

  test("uses minimal subagent when Claude Code is disabled", async () => {
    let minimalCalled = false;
    let detectCalled = false;

    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask("Simple change"),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: false },
        detectClaudeCodeFn: async () => {
          detectCalled = true;
          return { available: true };
        },
        runMinimalSubagent: () => {
          minimalCalled = true;
          return Effect.succeed(minimalResult);
        },
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(result.success).toBe(true);
    expect(minimalCalled).toBe(true);
    expect(detectCalled).toBe(false);
  });

  test("forwards permission mode from claudeCode config", async () => {
    let receivedPermission: string | undefined;

    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask("Complex refactor"),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: true, permissionMode: "dontAsk" },
        detectClaudeCodeFn: async () => ({ available: true }),
        runClaudeCodeFn: async (_subtask, options) => {
          receivedPermission = (options as any).permissionMode;
          return {
            success: true,
            subtaskId: _subtask.id,
            filesModified: [],
            turns: 1,
          };
        },
        runMinimalSubagent: () => Effect.succeed(minimalResult),
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(result.success).toBe(true);
    expect(receivedPermission).toBe("dontAsk");
  });
});
