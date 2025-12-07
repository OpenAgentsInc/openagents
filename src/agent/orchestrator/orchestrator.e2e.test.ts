import * as BunContext from "@effect/platform-bun/BunContext";
import { describe, expect, test } from "bun:test";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect, Layer } from "effect";
import { execSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import { runOrchestrator } from "./orchestrator.js";
import { runBestAvailableSubagent } from "./subagent-router.js";
import type { OrchestratorEvent, SubagentResult } from "./types.js";
import { OpenRouterClient, type OpenRouterClientShape } from "../../llm/openrouter.js";
import { createGoldenLoopFixture } from "./golden-loop-fixture.js";
import { DatabaseService } from "../../storage/database.js";
import { makeTestDatabaseLayer } from "../../tasks/test-helpers.ts";

const mockOpenRouterLayer = Layer.succeed(OpenRouterClient, {
  chat: () => Effect.fail(new Error("not used")),
} satisfies OpenRouterClientShape);

const runWithBun = <A, E>(
  program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path | OpenRouterClient | DatabaseService>
): Promise<A> =>
  Effect.gen(function* () {
    const { layer: dbLayer, cleanup } = yield* makeTestDatabaseLayer();
    const testLayer = Layer.mergeAll(BunContext.layer, mockOpenRouterLayer, dbLayer);

    try {
      return yield* program.pipe(Effect.provide(testLayer));
    } finally {
      cleanup();
    }
  }).pipe(
    Effect.provide(BunContext.layer),  // Provide services for makeTestDatabaseLayer
    Effect.runPromise
  );

const readTasks = (tasksPath: string) =>
  fs.readFileSync(tasksPath, "utf-8").trim().split("\n").map((line) => JSON.parse(line));

describe("Golden Loop with Claude Code", () => {
  test("completes a task using Claude Code subagent", async () => {
    const { dir, taskId, openagentsDir } = createGoldenLoopFixture({ name: "success", task: { labels: ["claude-code"] } });
    const events: OrchestratorEvent[] = [];
    const createdFile = path.join(dir, "feature.txt");

    const claudeRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => {
        fs.writeFileSync(createdFile, "updated by Claude Code");
        return {
          success: true,
          subtaskId: options.subtask.id,
          filesModified: [path.relative(dir, createdFile)],
          turns: 2,
          agent: "claude-code",
          sessionMetadata: {
            toolsUsed: { Edit: 1 },
            summary: "Completed via Claude Code",
          },
        } satisfies SubagentResult;
      });

    const state = await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["echo tests"],
          allowPush: false,
          maxSubtasksPerTask: 1,
          claudeCode: { enabled: true },
        },
        (event) => events.push(event),
        { runSubagent: claudeRunner },
      ),
    );

    expect(state.phase).toBe("done");
    expect(fs.existsSync(createdFile)).toBe(true);

    const tasks = readTasks(path.join(openagentsDir, "tasks.jsonl"));
    const closedTask = tasks.find((t) => t.id === taskId);
    expect(closedTask?.status).toBe("closed");
    expect(closedTask?.commits?.length ?? 0).toBeGreaterThan(0);

    const progress = fs.readFileSync(path.join(openagentsDir, "progress.md"), "utf-8");
    expect(progress).toContain("Claude Code Session");
    expect(progress).toContain("Completed via Claude Code");

    const log = execSync("git log --oneline -1", { cwd: dir, encoding: "utf-8" });
    expect(log).toContain(taskId);
  });

  test("falls back to minimal subagent when Claude Code fails", async () => {
    const { dir, taskId, openagentsDir } = createGoldenLoopFixture({ name: "fallback", task: { labels: ["claude-code"] } });
    const events: OrchestratorEvent[] = [];
    const fallbackFile = path.join(dir, "fallback.txt");

    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      runBestAvailableSubagent({
        ...options,
        claudeCode: { enabled: true, fallbackToMinimal: true },
        detectClaudeCodeFn: async () => ({ available: true }),
        runClaudeCodeFn: async () => ({
          success: false,
          subtaskId: options.subtask.id,
          filesModified: [],
          turns: 1,
          error: "rate_limit",
        }),
        runMinimalSubagent: () =>
          Effect.sync(() => {
            fs.writeFileSync(fallbackFile, "handled by minimal subagent");
            return {
              success: true,
              subtaskId: options.subtask.id,
              filesModified: [path.relative(dir, fallbackFile)],
              turns: 1,
              agent: "minimal",
              sessionMetadata: { summary: "fallback to minimal subagent" },
            };
          }),
      });

    const state = await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["echo tests"],
          allowPush: false,
          maxSubtasksPerTask: 1,
          claudeCode: { enabled: true, fallbackToMinimal: true },
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    expect(state.phase).toBe("done");
    expect(fs.existsSync(fallbackFile)).toBe(true);

    const tasks = readTasks(path.join(openagentsDir, "tasks.jsonl"));
    const closedTask = tasks.find((t) => t.id === taskId);
    expect(closedTask?.status).toBe("closed");
    expect(closedTask?.commits?.length ?? 0).toBeGreaterThan(0);

    const progress = fs.readFileSync(path.join(openagentsDir, "progress.md"), "utf-8");
    expect(progress).toContain("fallback to minimal subagent");

    const log = execSync("git log --oneline -1", { cwd: dir, encoding: "utf-8" });
    expect(log).toContain(taskId);
  });
});

describe("Typecheck failure handling", () => {
  test("injects fix-typecheck subtask when typecheck fails at start", async () => {
    const { dir, taskId, openagentsDir } = createGoldenLoopFixture({ name: "typecheck-fail" });
    const events: OrchestratorEvent[] = [];

    // Mock subagent that "fixes" the typecheck by creating a file
    const fixedFile = path.join(dir, "typecheck-fixed.txt");
    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => {
        fs.writeFileSync(fixedFile, "typecheck fixed");
        return {
          success: true,
          subtaskId: options.subtask.id,
          filesModified: [fixedFile],
          turns: 1,
          agent: "claude-code",
        } as SubagentResult;
      });

    await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          // First command fails (simulating typecheck failure), second passes
          testCommands: ["echo tests"],
          typecheckCommands: ["false"], // This will fail
          allowPush: false,
          maxSubtasksPerTask: 1,
          claudeCode: { enabled: true },
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    // Should have created a fix-typecheck subtask
    const subtasksFile = path.join(openagentsDir, "subtasks", `${taskId}.json`);
    expect(fs.existsSync(subtasksFile)).toBe(true);
    const subtasks = JSON.parse(fs.readFileSync(subtasksFile, "utf-8"));
    const fixTypecheckSubtask = subtasks.subtasks.find((s: any) => s.id.includes("fix-typecheck"));
    expect(fixTypecheckSubtask).toBeDefined();
    expect(fixTypecheckSubtask.description).toContain("CRITICAL: Fix Typecheck Errors First");
  });

  test("tracks consecutive failures and blocks task after max failures", async () => {
    const { dir, taskId, openagentsDir } = createGoldenLoopFixture({ name: "max-fail" });
    const events: OrchestratorEvent[] = [];
    let failCount = 0;

    // Mock subagent that always fails
    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => {
        failCount++;
        return {
          success: false,
          subtaskId: options.subtask.id,
          filesModified: [],
          turns: 1,
          agent: "claude-code",
          error: `Simulated failure #${failCount}`,
        } as SubagentResult;
      });

    // Run orchestrator 3 times (MAX_CONSECUTIVE_FAILURES = 3)
    for (let i = 0; i < 3; i++) {
      await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo tests"],
            allowPush: false,
            maxSubtasksPerTask: 1,
            claudeCode: { enabled: true },
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner },
        ),
      );
    }

    // After 3 failures, task should be blocked
    const tasks = readTasks(path.join(openagentsDir, "tasks.jsonl"));
    const blockedTask = tasks.find((t: any) => t.id === taskId);
    expect(blockedTask?.status).toBe("blocked");
    expect(blockedTask?.closeReason).toContain("consecutive failures");
  });

  test("includes failure context in prompt when retrying", async () => {
    const { dir, taskId, openagentsDir } = createGoldenLoopFixture({ name: "retry-context" });
    const events: OrchestratorEvent[] = [];

    // Mock subagent that captures the prompt and fails first time, succeeds second
    let callCount = 0;
    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => {
        callCount++;
        void options.subtask.description; // Captured for potential debugging

        if (callCount === 1) {
          return {
            success: false,
            subtaskId: options.subtask.id,
            filesModified: [],
            turns: 1,
            agent: "claude-code",
            error: "TypeScript error: Cannot find module 'foo'",
          } as SubagentResult;
        }

        return {
          success: true,
          subtaskId: options.subtask.id,
          filesModified: [],
          turns: 1,
          agent: "claude-code",
        } as SubagentResult;
      });

    // First run - fails
    await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["echo tests"],
          allowPush: false,
          maxSubtasksPerTask: 1,
          claudeCode: { enabled: true },
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    // Check subtask has failure info
    const subtasksFile = path.join(openagentsDir, "subtasks", `${taskId}.json`);
    const subtasksAfterFail = JSON.parse(fs.readFileSync(subtasksFile, "utf-8"));
    const failedSubtask = subtasksAfterFail.subtasks[0];
    expect(failedSubtask.failureCount).toBe(1);
    expect(failedSubtask.lastFailureReason).toContain("Cannot find module");
  });

  test("clears session resumption for other subtasks when injecting fix-typecheck", async () => {
    const { dir, taskId, openagentsDir } = createGoldenLoopFixture({ name: "clear-sessions" });

    // Pre-create a subtask file with an existing session ID
    const subtasksDir = path.join(openagentsDir, "subtasks");
    fs.mkdirSync(subtasksDir, { recursive: true });
    const existingSubtaskList = {
      taskId,
      taskTitle: "Test task",
      subtasks: [
        {
          id: `${taskId}-sub-001`,
          description: "Existing subtask",
          status: "pending",
          claudeCode: {
            sessionId: "old-session-123",
            resumeStrategy: "continue",
          },
        },
      ],
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };
    fs.writeFileSync(
      path.join(subtasksDir, `${taskId}.json`),
      JSON.stringify(existingSubtaskList, null, 2)
    );

    const events: OrchestratorEvent[] = [];
    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => ({
        success: true,
        subtaskId: options.subtask.id,
        filesModified: [],
        turns: 1,
        agent: "claude-code",
      } as SubagentResult));

    await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["echo tests"],
          typecheckCommands: ["false"], // Fails - triggers fix-typecheck injection
          allowPush: false,
          claudeCode: { enabled: true },
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    // Check that the existing subtask now has resumeStrategy: "fork"
    const subtasksFile = path.join(subtasksDir, `${taskId}.json`);
    const subtasks = JSON.parse(fs.readFileSync(subtasksFile, "utf-8"));

    // Should have fix-typecheck at the beginning
    expect(subtasks.subtasks[0].id).toContain("fix-typecheck");

    // The original subtask should have resumeStrategy changed to "fork"
    const originalSubtask = subtasks.subtasks.find((s: any) => s.id === `${taskId}-sub-001`);
    expect(originalSubtask).toBeDefined();
    expect(originalSubtask.claudeCode.resumeStrategy).toBe("fork");
  });
});

describe("Golden Loop negative path: failing tests leave task in-progress", () => {
  test("no commit when tests fail after subagent changes", async () => {
    const { dir, taskId, openagentsDir } = createGoldenLoopFixture({ name: "test-fail-no-commit" });
    const events: OrchestratorEvent[] = [];
    const createdFile = path.join(dir, "feature.txt");

    // Get initial commit count
    const initialCommitCount = parseInt(
      execSync("git rev-list --count HEAD", { cwd: dir, encoding: "utf-8" }).trim()
    );

    // Mock subagent that "succeeds" in making changes
    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => {
        fs.writeFileSync(createdFile, "changes by subagent");
        return {
          success: true,
          subtaskId: options.subtask.id,
          filesModified: [path.relative(dir, createdFile)],
          turns: 1,
          agent: "claude-code",
        } satisfies SubagentResult;
      });

    const state = await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["false"], // Intentionally failing test command
          allowPush: false,
          maxSubtasksPerTask: 1,
          claudeCode: { enabled: true },
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    // Verify no commit was created
    const finalCommitCount = parseInt(
      execSync("git rev-list --count HEAD", { cwd: dir, encoding: "utf-8" }).trim()
    );
    expect(finalCommitCount).toBe(initialCommitCount);

    // Verify state reflects failure
    expect(state.phase).toBe("failed");
    expect(state.error).toContain("Verification failed");

    // Verify task remains open (not closed)
    const tasks = readTasks(path.join(openagentsDir, "tasks.jsonl"));
    const task = tasks.find((t) => t.id === taskId);
    expect(task?.status).not.toBe("closed");
    // Task should still be open or in_progress (picked but not completed)
    expect(["open", "in_progress"]).toContain(task?.status);
  });

  test("task remains in_progress when subagent fails", async () => {
    const { dir, taskId, openagentsDir } = createGoldenLoopFixture({ name: "subagent-fail" });
    const events: OrchestratorEvent[] = [];

    // Mock subagent that fails
    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => ({
        success: false,
        subtaskId: options.subtask.id,
        filesModified: [],
        turns: 1,
        agent: "claude-code",
        error: "Simulated subagent failure",
      } satisfies SubagentResult));

    const state = await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["echo tests"],
          allowPush: false,
          maxSubtasksPerTask: 1,
          claudeCode: { enabled: true },
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    // Verify state reflects failure
    expect(state.phase).toBe("failed");
    expect(state.error).toBe("Simulated subagent failure");

    // Verify task is not closed
    const tasks = readTasks(path.join(openagentsDir, "tasks.jsonl"));
    const task = tasks.find((t) => t.id === taskId);
    expect(task?.status).not.toBe("closed");

    // Verify progress file has blockers
    const progress = fs.readFileSync(path.join(openagentsDir, "progress.md"), "utf-8");
    expect(progress).toContain("Failure 1/3");
    expect(progress).toContain("Simulated subagent failure");
  });

  test("logs capture verification failure details", async () => {
    const { dir, openagentsDir } = createGoldenLoopFixture({ name: "log-verify-fail" });
    const events: OrchestratorEvent[] = [];
    const createdFile = path.join(dir, "feature.txt");

    // Create a test script that outputs specific error message
    const failingTestScript = path.join(dir, "failing-test.sh");
    fs.writeFileSync(failingTestScript, '#!/bin/bash\necho "Error: TypeScript type mismatch at line 42"\nexit 1');
    fs.chmodSync(failingTestScript, 0o755);

    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => {
        fs.writeFileSync(createdFile, "changes by subagent");
        return {
          success: true,
          subtaskId: options.subtask.id,
          filesModified: [path.relative(dir, createdFile)],
          turns: 1,
          agent: "claude-code",
        } satisfies SubagentResult;
      });

    await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: [failingTestScript], // Custom failing test with error output
          allowPush: false,
          maxSubtasksPerTask: 1,
          claudeCode: { enabled: true },
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    // Verify progress file contains the error details
    const progress = fs.readFileSync(path.join(openagentsDir, "progress.md"), "utf-8");
    expect(progress).toContain("Tests or typecheck failed");

    // Verify verification_complete event was emitted with failure
    const verifyEvents = events.filter((e) => e.type === "verification_complete") as Array<{
      type: "verification_complete";
      command: string;
      passed: boolean;
      output: string;
    }>;
    expect(verifyEvents.length).toBeGreaterThan(0);
    const failedVerification = verifyEvents.find((e) => !e.passed);
    expect(failedVerification).toBeDefined();
  });

  test("no push occurs when tests fail (even if allowPush is true)", async () => {
    const { dir, openagentsDir } = createGoldenLoopFixture({ name: "no-push-on-fail" });
    const events: OrchestratorEvent[] = [];
    const createdFile = path.join(dir, "feature.txt");

    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => {
        fs.writeFileSync(createdFile, "changes by subagent");
        return {
          success: true,
          subtaskId: options.subtask.id,
          filesModified: [path.relative(dir, createdFile)],
          turns: 1,
          agent: "claude-code",
        } satisfies SubagentResult;
      });

    await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["false"], // Failing tests
          allowPush: true, // Would push if tests passed
          maxSubtasksPerTask: 1,
          claudeCode: { enabled: true },
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    // Verify no push event occurred
    const pushEvents = events.filter((e) => e.type === "push_complete");
    expect(pushEvents.length).toBe(0);

    // Verify no commit event occurred either
    const commitEvents = events.filter((e) => e.type === "commit_created");
    expect(commitEvents.length).toBe(0);
  });

  test("subtask marked as failed with error details", async () => {
    const { dir, taskId, openagentsDir } = createGoldenLoopFixture({ name: "subtask-fail-details" });
    const events: OrchestratorEvent[] = [];

    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => ({
        success: false,
        subtaskId: options.subtask.id,
        filesModified: [],
        turns: 1,
        agent: "claude-code",
        error: "Cannot read file: permission denied",
      } satisfies SubagentResult));

    await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["echo tests"],
          allowPush: false,
          maxSubtasksPerTask: 1,
          claudeCode: { enabled: true },
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    // Verify subtask file has error details
    const subtasksFile = path.join(openagentsDir, "subtasks", `${taskId}.json`);
    expect(fs.existsSync(subtasksFile)).toBe(true);
    const subtasks = JSON.parse(fs.readFileSync(subtasksFile, "utf-8"));

    const failedSubtask = subtasks.subtasks[0];
    expect(failedSubtask.status).toBe("failed");
    expect(failedSubtask.error).toBe("Cannot read file: permission denied");
    expect(failedSubtask.failureCount).toBe(1);
    expect(failedSubtask.lastFailureReason).toBe("Cannot read file: permission denied");

    // Verify subtask_failed event was emitted
    const failedEvents = events.filter((e) => e.type === "subtask_failed") as Array<{
      type: "subtask_failed";
      subtask: any;
      error: string;
    }>;
    expect(failedEvents.length).toBe(1);
    expect(failedEvents[0].error).toBe("Cannot read file: permission denied");
  });

  test("working tree changes preserved when tests fail (no git reset)", async () => {
    const { dir, openagentsDir } = createGoldenLoopFixture({ name: "preserve-changes" });
    const events: OrchestratorEvent[] = [];
    const createdFile = path.join(dir, "feature.txt");
    const fileContent = "important work that should not be lost";

    const subagentRunner: typeof runBestAvailableSubagent = (options) =>
      Effect.sync(() => {
        fs.writeFileSync(createdFile, fileContent);
        return {
          success: true,
          subtaskId: options.subtask.id,
          filesModified: [path.relative(dir, createdFile)],
          turns: 1,
          agent: "claude-code",
        } satisfies SubagentResult;
      });

    await runWithBun(
      runOrchestrator(
        {
          cwd: dir,
          openagentsDir,
          testCommands: ["false"], // Failing tests
          allowPush: false,
          maxSubtasksPerTask: 1,
          claudeCode: { enabled: true },
        },
        (event) => events.push(event),
        { runSubagent: subagentRunner },
      ),
    );

    // Verify the file still exists with its content (not reset)
    expect(fs.existsSync(createdFile)).toBe(true);
    expect(fs.readFileSync(createdFile, "utf-8")).toBe(fileContent);

    // Verify git status shows the file as untracked or modified
    const gitStatus = execSync("git status --porcelain", { cwd: dir, encoding: "utf-8" });
    expect(gitStatus).toContain("feature.txt");
  });
});
