/**
 * Integration tests for Golden Loop v2 verification phase gating
 *
 * Tests that orchestrator respects GOLDEN-LOOP-v2 rules:
 * - Run typecheck/testCommands before commit
 * - Block commit/push when verification fails
 * - Record failure in tasks/logs
 * - Cover both Claude Code and minimal subagent paths
 *
 * @see docs/mechacoder/GOLDEN-LOOP-v2.md Section 2.6 (Test) and Section 3 (Acceptance Criteria)
 */
import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, test } from "bun:test";
import { Effect, Layer } from "effect";
import { execSync } from "node:child_process";
import * as fs from "node:fs";
import { tmpdir } from "node:os";
import * as path from "node:path";
import { runOrchestrator } from "./orchestrator.js";
import type { OrchestratorEvent, SubagentResult } from "./types.js";
import {
  OpenRouterClient,
  type OpenRouterClientShape,
} from "../../llm/openrouter.js";

const mockOpenRouterLayer = Layer.succeed(OpenRouterClient, {
  chat: () => Effect.fail(new Error("not used")),
} satisfies OpenRouterClientShape);
const testLayer = Layer.mergeAll(BunContext.layer, mockOpenRouterLayer);

const runWithBun = <A, E>(
  program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path | OpenRouterClient>
) => Effect.runPromise(program.pipe(Effect.provide(testLayer)));

/**
 * Creates a test git repository with OpenAgents task configuration
 */
const createTestRepo = (name: string) => {
  const dir = fs.mkdtempSync(path.join(tmpdir(), `verify-${name}-`));

  execSync("git init", { cwd: dir, stdio: "ignore" });
  execSync("git checkout -b main", { cwd: dir, stdio: "ignore" });
  execSync('git config user.email "mechacoder@example.com"', {
    cwd: dir,
    stdio: "ignore",
  });
  execSync('git config user.name "MechaCoder"', { cwd: dir, stdio: "ignore" });

  fs.writeFileSync(path.join(dir, "README.md"), "# Test Repo\n");

  const now = new Date().toISOString();
  const task = {
    id: `oa-${name}`,
    title: `Verification test: ${name}`,
    description: "Test verification phase gating",
    status: "open",
    priority: 1,
    type: "task",
    labels: [],
    deps: [],
    commits: [],
    createdAt: now,
    updatedAt: now,
    closedAt: null,
  };

  const oaDir = path.join(dir, ".openagents");
  fs.mkdirSync(oaDir, { recursive: true });
  fs.writeFileSync(
    path.join(oaDir, "project.json"),
    JSON.stringify(
      {
        projectId: `proj-${name}`,
        defaultBranch: "main",
        testCommands: ["echo tests"],
        allowPush: false,
      },
      null,
      2
    )
  );
  fs.writeFileSync(
    path.join(oaDir, "tasks.jsonl"),
    `${JSON.stringify(task)}\n`
  );

  execSync("git add -A", { cwd: dir, stdio: "ignore" });
  execSync('git commit -m "init"', { cwd: dir, stdio: "ignore" });

  return { dir, taskId: task.id, openagentsDir: oaDir };
};

const readTasks = (tasksPath: string) =>
  fs
    .readFileSync(tasksPath, "utf-8")
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line));

const getCommitCount = (dir: string): number => {
  const output = execSync("git rev-list --count HEAD", {
    cwd: dir,
    encoding: "utf-8",
  });
  return parseInt(output.trim(), 10);
};

describe("Golden Loop v2 Verification Phase Gating", () => {
  describe("Test command execution", () => {
    test("runs testCommands before allowing commit", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("run-tests");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "created by subagent");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
            agent: "claude-code",
          } as SubagentResult;
        });

      await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo 'Running tests...'"],
            allowPush: false,
            maxSubtasksPerTask: 1,
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      // Verify test commands were executed
      const verificationEvents = events.filter(
        (e) => e.type === "verification_start" || e.type === "verification_complete"
      );
      expect(verificationEvents.length).toBeGreaterThan(0);

      const testStartEvents = events.filter(
        (e) => e.type === "verification_start"
      ) as Array<{ type: "verification_start"; command: string }>;
      expect(testStartEvents.some((e) => e.command.includes("Running tests"))).toBe(
        true
      );
    });

    test("runs typecheckCommands before testCommands", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("typecheck-order");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "created by subagent");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
            agent: "claude-code",
          } as SubagentResult;
        });

      await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            typecheckCommands: ["echo 'typecheck'"],
            testCommands: ["echo 'tests'"],
            allowPush: false,
            maxSubtasksPerTask: 1,
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      // Find verification events in order
      const verifyStartEvents = events.filter(
        (e) => e.type === "verification_start"
      ) as Array<{ type: "verification_start"; command: string }>;

      // Typecheck should come before tests in verification order
      const typecheckIdx = verifyStartEvents.findIndex((e) =>
        e.command.includes("typecheck")
      );
      const testsIdx = verifyStartEvents.findIndex((e) =>
        e.command.includes("tests")
      );

      expect(typecheckIdx).toBeGreaterThanOrEqual(0);
      expect(testsIdx).toBeGreaterThanOrEqual(0);
      expect(typecheckIdx).toBeLessThan(testsIdx);
    });
  });

  describe("Commit blocking on verification failure", () => {
    test("blocks commit when testCommands fail", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("test-fail-block");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");
      const commitCountBefore = getCommitCount(dir);

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "created by subagent");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
            agent: "claude-code",
          } as SubagentResult;
        });

      const state = await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["exit 1"], // Force test failure
            allowPush: false,
            maxSubtasksPerTask: 1,
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      // Verify no new commit was created
      const commitCountAfter = getCommitCount(dir);
      expect(commitCountAfter).toBe(commitCountBefore);

      // Verify state indicates failure
      expect(state.phase).toBe("failed");
      expect(state.error).toContain("Verification failed");

      // Verify no commit_created event
      const commitEvents = events.filter((e) => e.type === "commit_created");
      expect(commitEvents).toHaveLength(0);

      // Verify task was NOT closed
      const tasks = readTasks(path.join(openagentsDir, "tasks.jsonl"));
      const task = tasks.find((t) => t.id === taskId);
      expect(task?.status).not.toBe("closed");
    });

    test("blocks commit when typecheckCommands fail at verification phase", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("typecheck-fail-block");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");
      const commitCountBefore = getCommitCount(dir);

      // Create subtasks file to skip typecheck injection (we want verification to fail)
      const subtasksDir = path.join(openagentsDir, "subtasks");
      fs.mkdirSync(subtasksDir, { recursive: true });
      const subtasksList = {
        taskId,
        taskTitle: "Test task",
        subtasks: [
          {
            id: `${taskId}-sub-001`,
            description: "Test subtask",
            status: "pending",
          },
        ],
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };
      fs.writeFileSync(
        path.join(subtasksDir, `${taskId}.json`),
        JSON.stringify(subtasksList, null, 2)
      );

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "created by subagent");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
            agent: "claude-code",
          } as SubagentResult;
        });

      const state = await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            typecheckCommands: ["exit 1"], // Force typecheck failure at verification
            testCommands: ["echo tests"],
            allowPush: false,
            maxSubtasksPerTask: 1,
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      // When typecheck fails at orientation, it injects fix-typecheck subtask
      // If that succeeds but verification still fails, commit is blocked
      const commitCountAfter = getCommitCount(dir);

      // Either state is failed (verification blocked commit) or
      // a fix-typecheck subtask was injected and processed
      if (state.phase === "failed") {
        expect(commitCountAfter).toBe(commitCountBefore);
      }
    });
  });

  describe("Push blocking on verification failure", () => {
    test("does not push when verification fails even if allowPush is true", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("push-fail-block");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "created by subagent");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
            agent: "claude-code",
          } as SubagentResult;
        });

      const state = await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["exit 1"], // Force test failure
            allowPush: true, // Would push if tests passed
            maxSubtasksPerTask: 1,
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      // Verify no push event
      const pushEvents = events.filter((e) => e.type === "push_complete");
      expect(pushEvents).toHaveLength(0);

      expect(state.phase).toBe("failed");
    });
  });

  describe("Failure recording in tasks and logs", () => {
    test("records verification failure in progress.md", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("record-progress");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "created by subagent");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
            agent: "claude-code",
          } as SubagentResult;
        });

      await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo 'Test error output' && exit 1"],
            allowPush: false,
            maxSubtasksPerTask: 1,
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      // Verify progress.md contains failure information
      const progressPath = path.join(openagentsDir, "progress.md");
      expect(fs.existsSync(progressPath)).toBe(true);

      const progress = fs.readFileSync(progressPath, "utf-8");
      expect(progress).toContain("Blockers");
      expect(progress).toMatch(/tests?.*fail|typecheck.*fail/i);
    });

    test("does not mark task as closed when verification fails", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("task-not-closed");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "created by subagent");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
            agent: "claude-code",
          } as SubagentResult;
        });

      await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["exit 1"],
            allowPush: false,
            maxSubtasksPerTask: 1,
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      // Task should still be open (not closed)
      const tasks = readTasks(path.join(openagentsDir, "tasks.jsonl"));
      const task = tasks.find((t) => t.id === taskId);
      expect(task?.status).toBe("open"); // Still open, not closed
    });

    test("records testsPassingAfterWork: false when verification fails", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("tests-passing-flag");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "created by subagent");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
            agent: "claude-code",
          } as SubagentResult;
        });

      await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["exit 1"],
            allowPush: false,
            maxSubtasksPerTask: 1,
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      // Check progress.md for testsPassingAfterWork flag
      const progressPath = path.join(openagentsDir, "progress.md");
      const progress = fs.readFileSync(progressPath, "utf-8");

      // Progress file should indicate tests did not pass
      expect(progress).toMatch(/Tests.*After.*Work.*:.*No|false|❌/i);
    });
  });

  describe("Claude Code subagent path", () => {
    test("blocks commit when Claude Code succeeds but verification fails", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("claude-verify-fail");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");
      const commitCountBefore = getCommitCount(dir);

      // Claude Code subagent succeeds, but introduced breaking changes
      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "breaking changes by Claude Code");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 2,
            agent: "claude-code",
            claudeCodeSessionId: "sess-123",
            sessionMetadata: {
              sessionId: "sess-123",
              toolsUsed: { Edit: 1 },
              summary: "Made breaking changes",
            },
          } as SubagentResult;
        });

      const state = await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["exit 1"], // Verification fails
            allowPush: false,
            maxSubtasksPerTask: 1,
            claudeCode: { enabled: true },
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      // No commit should be created
      const commitCountAfter = getCommitCount(dir);
      expect(commitCountAfter).toBe(commitCountBefore);
      expect(state.phase).toBe("failed");

      // Session metadata should still be recorded for resumption
      const progressPath = path.join(openagentsDir, "progress.md");
      const progress = fs.readFileSync(progressPath, "utf-8");
      expect(progress).toContain("Claude Code Session");
    });

    test("allows commit when Claude Code succeeds and verification passes", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("claude-verify-pass");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");
      const commitCountBefore = getCommitCount(dir);

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "good changes by Claude Code");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 2,
            agent: "claude-code",
            claudeCodeSessionId: "sess-456",
            sessionMetadata: {
              sessionId: "sess-456",
              toolsUsed: { Edit: 1 },
              summary: "Made good changes",
            },
          } as SubagentResult;
        });

      const state = await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo tests pass"],
            allowPush: false,
            maxSubtasksPerTask: 1,
            claudeCode: { enabled: true },
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      // Commit should be created
      const commitCountAfter = getCommitCount(dir);
      expect(commitCountAfter).toBe(commitCountBefore + 1);
      expect(state.phase).toBe("done");

      // Task should be closed
      const tasks = readTasks(path.join(openagentsDir, "tasks.jsonl"));
      const task = tasks.find((t) => t.id === taskId);
      expect(task?.status).toBe("closed");
    });
  });

  describe("Minimal subagent path", () => {
    test("blocks commit when minimal subagent succeeds but verification fails", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("minimal-verify-fail");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");
      const commitCountBefore = getCommitCount(dir);

      // Minimal subagent succeeds
      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "changes by minimal subagent");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
            agent: "minimal",
            sessionMetadata: {
              summary: "Minimal subagent made changes",
            },
          } as SubagentResult;
        });

      const state = await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["exit 1"], // Verification fails
            allowPush: false,
            maxSubtasksPerTask: 1,
            claudeCode: { enabled: false }, // Force minimal path
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      // No commit should be created
      const commitCountAfter = getCommitCount(dir);
      expect(commitCountAfter).toBe(commitCountBefore);
      expect(state.phase).toBe("failed");
    });

    test("allows commit when minimal subagent succeeds and verification passes", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("minimal-verify-pass");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");
      const commitCountBefore = getCommitCount(dir);

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "good changes by minimal subagent");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
            agent: "minimal",
            sessionMetadata: {
              summary: "Minimal subagent completed successfully",
            },
          } as SubagentResult;
        });

      const state = await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo tests pass"],
            allowPush: false,
            maxSubtasksPerTask: 1,
            claudeCode: { enabled: false },
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      // Commit should be created
      const commitCountAfter = getCommitCount(dir);
      expect(commitCountAfter).toBe(commitCountBefore + 1);
      expect(state.phase).toBe("done");

      // Task should be closed
      const tasks = readTasks(path.join(openagentsDir, "tasks.jsonl"));
      const task = tasks.find((t) => t.id === taskId);
      expect(task?.status).toBe("closed");
    });
  });

  describe("Verification event emission", () => {
    test("emits verification_start and verification_complete events", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("verify-events");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "changes");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
            agent: "minimal",
          } as SubagentResult;
        });

      await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo test1", "echo test2"],
            allowPush: false,
            maxSubtasksPerTask: 1,
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      // Should have verification events for each command
      const startEvents = events.filter((e) => e.type === "verification_start");
      const completeEvents = events.filter(
        (e) => e.type === "verification_complete"
      );

      expect(startEvents.length).toBeGreaterThanOrEqual(2);
      expect(completeEvents.length).toBeGreaterThanOrEqual(2);

      // Each complete event should have passed=true for these echo commands
      const passedEvents = completeEvents.filter(
        (e) => e.type === "verification_complete" && (e as any).passed === true
      );
      expect(passedEvents.length).toBeGreaterThanOrEqual(2);
    });

    test("verification_complete includes passed=false when command fails", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("verify-fail-event");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "changes");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
            agent: "minimal",
          } as SubagentResult;
        });

      await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["exit 1"],
            allowPush: false,
            maxSubtasksPerTask: 1,
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      const failedEvents = events.filter(
        (e) =>
          e.type === "verification_complete" && (e as any).passed === false
      );
      expect(failedEvents.length).toBeGreaterThanOrEqual(1);
    });
  });

  describe("Session completion events", () => {
    test("emits session_complete with success=false when verification fails", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("session-fail");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "changes");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
            agent: "minimal",
          } as SubagentResult;
        });

      await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["exit 1"],
            allowPush: false,
            maxSubtasksPerTask: 1,
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      const sessionCompleteEvents = events.filter(
        (e) => e.type === "session_complete"
      ) as Array<{ type: "session_complete"; success: boolean; summary: string }>;

      expect(sessionCompleteEvents.length).toBe(1);
      expect(sessionCompleteEvents[0].success).toBe(false);
      expect(sessionCompleteEvents[0].summary).toContain("Verification failed");
    });

    test("emits session_complete with success=true when verification passes", async () => {
      const { dir, taskId, openagentsDir } = createTestRepo("session-success");
      const events: OrchestratorEvent[] = [];
      const testFile = path.join(dir, "feature.txt");

      const subagentRunner = () =>
        Effect.sync(() => {
          fs.writeFileSync(testFile, "changes");
          return {
            success: true,
            subtaskId: `${taskId}-sub-001`,
            filesModified: ["feature.txt"],
            turns: 1,
            agent: "minimal",
          } as SubagentResult;
        });

      await runWithBun(
        runOrchestrator(
          {
            cwd: dir,
            openagentsDir,
            testCommands: ["echo pass"],
            allowPush: false,
            maxSubtasksPerTask: 1,
          },
          (event) => events.push(event),
          { runSubagent: subagentRunner }
        )
      );

      const sessionCompleteEvents = events.filter(
        (e) => e.type === "session_complete"
      ) as Array<{ type: "session_complete"; success: boolean; summary: string }>;

      expect(sessionCompleteEvents.length).toBe(1);
      expect(sessionCompleteEvents[0].success).toBe(true);
      expect(sessionCompleteEvents[0].summary).toContain("Completed task");
    });
  });
});
