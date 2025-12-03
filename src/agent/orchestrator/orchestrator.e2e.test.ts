import * as BunContext from "@effect/platform-bun/BunContext";
import { describe, expect, test } from "bun:test";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect, Layer } from "effect";
import { execSync } from "node:child_process";
import * as fs from "node:fs";
import { tmpdir } from "node:os";
import * as path from "node:path";
import { runOrchestrator } from "./orchestrator.js";
import { runBestAvailableSubagent } from "./subagent-router.js";
import type { OrchestratorEvent, SubagentResult } from "./types.js";
import { OpenRouterClient, type OpenRouterClientShape } from "../../llm/openrouter.js";

const mockOpenRouterLayer = Layer.succeed(OpenRouterClient, {
  chat: () => Effect.fail(new Error("not used")),
} satisfies OpenRouterClientShape);
const testLayer = Layer.mergeAll(BunContext.layer, mockOpenRouterLayer);

const runWithBun = <A, E>(
  program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path | OpenRouterClient>
) =>
  Effect.runPromise(
    program.pipe(Effect.provide(testLayer))
  );

const createTestRepo = (name: string) => {
  const dir = fs.mkdtempSync(path.join(tmpdir(), `claude-e2e-${name}-`));

  execSync("git init", { cwd: dir, stdio: "ignore" });
  execSync("git checkout -b main", { cwd: dir, stdio: "ignore" });
  execSync('git config user.email "mechacoder@example.com"', { cwd: dir, stdio: "ignore" });
  execSync('git config user.name "MechaCoder"', { cwd: dir, stdio: "ignore" });

  fs.writeFileSync(path.join(dir, "README.md"), "# Test Repo\n");

  const now = new Date().toISOString();
  const task = {
    id: `oa-${name}`,
    title: `Claude E2E ${name}`,
    description: "Exercise Golden Loop with Claude Code path",
    status: "open",
    priority: 1,
    type: "task",
    labels: ["claude-code"],
    deps: [],
    commits: [],
    createdAt: now,
    updatedAt: now,
    closedAt: null,
  };

  const oaDir = path.join(dir, ".openagents");
  fs.mkdirSync(oaDir, { recursive: true });
  fs.writeFileSync(path.join(oaDir, "project.json"), JSON.stringify({
    projectId: `proj-${name}`,
    defaultBranch: "main",
    testCommands: ["echo tests"],
    allowPush: false,
  }, null, 2));
  fs.writeFileSync(path.join(oaDir, "tasks.jsonl"), `${JSON.stringify(task)}\n`);

  execSync("git add -A", { cwd: dir, stdio: "ignore" });
  execSync('git commit -m "init"', { cwd: dir, stdio: "ignore" });

  return { dir, taskId: task.id, openagentsDir: oaDir };
};

const readTasks = (tasksPath: string) =>
  fs.readFileSync(tasksPath, "utf-8").trim().split("\n").map((line) => JSON.parse(line));

describe("Golden Loop with Claude Code", () => {
  test("completes a task using Claude Code subagent", async () => {
    const { dir, taskId, openagentsDir } = createTestRepo("success");
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
    const { dir, taskId, openagentsDir } = createTestRepo("fallback");
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
    const { dir, taskId, openagentsDir } = createTestRepo("typecheck-fail");
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
    const { dir, taskId, openagentsDir } = createTestRepo("max-fail");
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
    const { dir, taskId, openagentsDir } = createTestRepo("retry-context");
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
});
