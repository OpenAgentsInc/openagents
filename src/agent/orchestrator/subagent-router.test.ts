import { describe, expect, test } from "bun:test";
import { Effect, Layer, Context } from "effect";
import { runBestAvailableSubagent, shouldUseClaudeCode } from "./subagent-router.js";
import type { SubagentResult, Subtask } from "./types.js";
import type { OpenRouterClient } from "../../llm/openrouter.js";

// Mock OpenRouterClient for tests
const MockOpenRouterClient = Layer.succeed(
  Context.Tag<OpenRouterClient>(),
  {} as OpenRouterClient
);

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
      runBestAvailableSubagent<never>({
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

  test("uses minimal subagent when Claude Code is disabled", async () => {
    let minimalCalled = false;
    let detectCalled = false;

    const result = await Effect.runPromise(
      runBestAvailableSubagent<never>({
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
      runBestAvailableSubagent<never>({
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
