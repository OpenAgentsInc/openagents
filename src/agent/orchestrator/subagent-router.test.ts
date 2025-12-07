import { describe, expect, test } from "bun:test";
import { Effect, Layer } from "effect";
import {
  runBestAvailableSubagent,
  shouldUseClaudeCode,
} from "./subagent-router.js";
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

  test("passes resume/fork options to Claude Code runner and returns session metadata", async () => {
    let receivedOptions: any;

    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: {
          ...makeSubtask("Resume prior session"),
          claudeCode: { sessionId: "sess-old", resumeStrategy: "fork" },
        },
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: true, preferForComplexTasks: false },
        detectClaudeCodeFn: async () => ({ available: true }),
        runClaudeCodeFn: async (_subtask, options) => {
          receivedOptions = options;
          return {
            success: true,
            subtaskId: _subtask.id,
            filesModified: [],
            turns: 1,
            claudeCodeSessionId: "sess-new",
            claudeCodeForkedFromSessionId: "sess-old",
          };
        },
        runMinimalSubagent: () => Effect.succeed(minimalResult),
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(result.success).toBe(true);
    expect(receivedOptions?.resumeSessionId).toBe("sess-old");
    expect(receivedOptions?.forkSession).toBe(true);
    expect(result.claudeCodeSessionId).toBe("sess-new");
    expect(result.claudeCodeForkedFromSessionId).toBe("sess-old");
  });
});

describe("verification edge cases", () => {
  test("handles verification function throwing error", async () => {
    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask(),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: true, fallbackToMinimal: false },
        verificationCommands: ["bun test"],
        verifyFn: async () => {
          throw new Error("Command timed out");
        },
        detectClaudeCodeFn: async () => ({ available: true }),
        runClaudeCodeFn: async (subtask) => ({
          success: true,
          subtaskId: subtask.id,
          filesModified: ["a.ts"],
          turns: 1,
        }),
        runMinimalSubagent: () => Effect.succeed(minimalResult),
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    // Should return failed result when verification throws
    expect(result.success).toBe(false);
    expect(result.error).toContain("Verification failed");
  });

  test("handles verification timeout by falling back to minimal subagent", async () => {
    let minimalCalled = false;
    let verifyCallCount = 0;

    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask(),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: true, fallbackToMinimal: true },
        verificationCommands: ["bun test"],
        verifyFn: async () => {
          verifyCallCount++;
          if (verifyCallCount === 1) {
            // First verification (after Claude Code) times out
            throw new Error("ETIMEDOUT");
          }
          // Second verification (after fallback) passes
          return { passed: true, outputs: ["ok"] };
        },
        detectClaudeCodeFn: async () => ({ available: true }),
        runClaudeCodeFn: async (subtask) => ({
          success: true,
          subtaskId: subtask.id,
          filesModified: ["cc.ts"],
          turns: 1,
        }),
        runMinimalSubagent: () => {
          minimalCalled = true;
          return Effect.succeed(minimalResult);
        },
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(minimalCalled).toBe(true);
    expect(result.success).toBe(true);
    expect(result.agent).toBe("minimal");
  });

  test("returns verification outputs even on failure", async () => {
    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask(),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: true, fallbackToMinimal: false },
        verificationCommands: ["bun test"],
        verifyFn: async () => ({
          passed: false,
          outputs: ["error: line 42: expected 'foo' but got 'bar'"],
        }),
        detectClaudeCodeFn: async () => ({ available: true }),
        runClaudeCodeFn: async (subtask) => ({
          success: true,
          subtaskId: subtask.id,
          filesModified: [],
          turns: 1,
        }),
        runMinimalSubagent: () => Effect.succeed(minimalResult),
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(result.success).toBe(false);
    expect(result.verificationOutputs).toEqual(["error: line 42: expected 'foo' but got 'bar'"]);
    expect(result.error).toContain("expected 'foo' but got 'bar'");
  });

  test("handles Claude Code detection failure gracefully", async () => {
    let minimalCalled = false;

    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask(),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: true },
        detectClaudeCodeFn: async () => {
          throw new Error("Detection failed");
        },
        runMinimalSubagent: () => {
          minimalCalled = true;
          return Effect.succeed(minimalResult);
        },
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    // Should fall back to minimal when detection fails
    expect(minimalCalled).toBe(true);
    expect(result.success).toBe(true);
  });

  test("handles empty verification outputs array", async () => {
    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask(),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: true, fallbackToMinimal: false },
        verificationCommands: ["bun test"],
        verifyFn: async () => ({
          passed: false,
          outputs: [],
        }),
        detectClaudeCodeFn: async () => ({ available: true }),
        runClaudeCodeFn: async (subtask) => ({
          success: true,
          subtaskId: subtask.id,
          filesModified: [],
          turns: 1,
        }),
        runMinimalSubagent: () => Effect.succeed(minimalResult),
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(result.success).toBe(false);
    // Should have a default error message
    expect(result.error).toBe("Verification failed (typecheck/tests)");
  });

  test("handles verification with whitespace-only outputs", async () => {
    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask(),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: true, fallbackToMinimal: false },
        verificationCommands: ["bun test"],
        verifyFn: async () => ({
          passed: false,
          outputs: ["   ", "\n\n", "\t"],
        }),
        detectClaudeCodeFn: async () => ({ available: true }),
        runClaudeCodeFn: async (subtask) => ({
          success: true,
          subtaskId: subtask.id,
          filesModified: [],
          turns: 1,
        }),
        runMinimalSubagent: () => Effect.succeed(minimalResult),
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(result.success).toBe(false);
    // Should fall back to default error message
    expect(result.error).toBe("Verification failed (typecheck/tests)");
  });

  test("both Claude Code and fallback fail verification", async () => {
    let verifyCallCount = 0;
    let minimalCalled = false;

    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask(),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: true, fallbackToMinimal: true },
        verificationCommands: ["bun test"],
        verifyFn: async () => {
          verifyCallCount++;
          return { passed: false, outputs: [`Failure #${verifyCallCount}`] };
        },
        detectClaudeCodeFn: async () => ({ available: true }),
        runClaudeCodeFn: async (subtask) => ({
          success: true,
          subtaskId: subtask.id,
          filesModified: ["cc.ts"],
          turns: 1,
        }),
        runMinimalSubagent: () => {
          minimalCalled = true;
          return Effect.succeed(minimalResult);
        },
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(minimalCalled).toBe(true);
    expect(verifyCallCount).toBe(2);
    expect(result.success).toBe(false);
    expect(result.agent).toBe("minimal");
    // Should show the fallback verification failure
    expect(result.error).toContain("Failure #2");
  });
});

describe("FM subagent routing", () => {
  test("routes to FM when Claude Code unavailable and FM available", async () => {
    let fmCalled = false;
    let minimalCalled = false;

    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask("Simple coding task"),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: true },
        fm: { enabled: true },
        detectClaudeCodeFn: async () => ({ available: false }),
        detectFMFn: async () => ({ available: true }),
        runFMFn: async (subtask) => {
          fmCalled = true;
          return {
            success: true,
            subtaskId: subtask.id,
            filesModified: ["fm.ts"],
            turns: 1,
            agent: "fm",
            learningMetrics: {
              skillsInjected: [],
              memoriesInjected: [],
            },
          };
        },
        runMinimalSubagent: () => {
          minimalCalled = true;
          return Effect.succeed(minimalResult);
        },
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(result.success).toBe(true);
    expect(fmCalled).toBe(true);
    expect(minimalCalled).toBe(false);
    expect(result.agent).toBe("fm");
  });

  test("falls back to minimal when FM fails", async () => {
    let minimalCalled = false;

    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask("Simple coding task"),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: true },
        fm: { enabled: true },
        detectClaudeCodeFn: async () => ({ available: false }),
        detectFMFn: async () => ({ available: true }),
        runFMFn: async (subtask) => ({
          success: false,
          subtaskId: subtask.id,
          filesModified: [],
          turns: 1,
          agent: "fm",
          error: "FM failed",
          learningMetrics: {
            skillsInjected: [],
            memoriesInjected: [],
          },
        }),
        runMinimalSubagent: () => {
          minimalCalled = true;
          return Effect.succeed(minimalResult);
        },
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(result.success).toBe(true);
    expect(minimalCalled).toBe(true);
    expect(result.agent).toBe("minimal");
  });

  test("passes FM settings to FM runner", async () => {
    let receivedSettings: FMSettings | undefined;

    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask("Simple coding task"),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: false },
        fm: {
          enabled: true,
          useSkills: true,
          useMemory: true,
          maxSkills: 5,
          maxMemories: 3,
        },
        detectFMFn: async () => ({ available: true }),
        runFMFn: async (subtask, options) => {
          receivedSettings = options.settings;
          return {
            success: true,
            subtaskId: subtask.id,
            filesModified: [],
            turns: 1,
            agent: "fm",
            learningMetrics: {
              skillsInjected: ["skill-1"],
              memoriesInjected: ["mem-1"],
            },
          };
        },
        runMinimalSubagent: () => Effect.succeed(minimalResult),
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(result.success).toBe(true);
    expect(receivedSettings).toBeDefined();
    expect(receivedSettings?.useSkills).toBe(true);
    expect(receivedSettings?.useMemory).toBe(true);
    expect(receivedSettings?.maxSkills).toBe(5);
    expect(receivedSettings?.maxMemories).toBe(3);
  });

  test("FM returns learning metrics in result", async () => {
    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask("Simple coding task"),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: false },
        fm: { enabled: true, useSkills: true, useMemory: true },
        detectFMFn: async () => ({ available: true }),
        runFMFn: async (subtask) => ({
          success: true,
          subtaskId: subtask.id,
          filesModified: ["fm.ts"],
          turns: 1,
          agent: "fm",
          learningMetrics: {
            skillsInjected: ["skill-abc", "skill-xyz"],
            memoriesInjected: ["mem-123"],
          },
        }),
        runMinimalSubagent: () => Effect.succeed(minimalResult),
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(result.success).toBe(true);
    expect(result.agent).toBe("fm");
    expect(result.learningMetrics).toBeDefined();
    expect(result.learningMetrics?.skillsInjected).toEqual(["skill-abc", "skill-xyz"]);
    expect(result.learningMetrics?.memoriesInjected).toEqual(["mem-123"]);
  });

  test("skips FM when disabled", async () => {
    let fmCalled = false;
    let minimalCalled = false;

    const result = await Effect.runPromise(
      runBestAvailableSubagent({
        subtask: makeSubtask("Simple coding task"),
        cwd: "/tmp",
        openagentsDir: "/tmp/.openagents",
        tools: [],
        claudeCode: { enabled: false },
        fm: { enabled: false },
        detectFMFn: async () => {
          fmCalled = true;
          return { available: true };
        },
        runMinimalSubagent: () => {
          minimalCalled = true;
          return Effect.succeed(minimalResult);
        },
      }).pipe(Effect.provide(MockOpenRouterClient))
    );

    expect(result.success).toBe(true);
    expect(fmCalled).toBe(false);
    expect(minimalCalled).toBe(true);
  });
});
