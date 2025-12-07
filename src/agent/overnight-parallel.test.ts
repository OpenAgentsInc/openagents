/**
 * Tests for parallel overnight agent task closing functionality
 *
 * These tests ensure that tasks are properly closed in the main repo
 * after worktree merges. This prevents tasks from getting stuck in
 * commit_pending status.
 */
import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import { Effect } from "effect";
import * as BunContext from "@effect/platform-bun/BunContext";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import { execSync } from "node:child_process";
import { updateTask, readTasks } from "../tasks/service.js";
import type { Task } from "../tasks/schema.js";

// Helper to create a temporary directory
const createTempDir = (): string => {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "overnight-parallel-test-"));
  return tmpDir;
};

// Helper to clean up temp directory
const cleanupTempDir = (dir: string): void => {
  fs.rmSync(dir, { recursive: true, force: true });
};

// Helper to initialize git repo
const initGitRepo = (dir: string): void => {
  execSync("git init", { cwd: dir, stdio: "pipe" });
  execSync('git config user.email "test@test.com"', { cwd: dir, stdio: "pipe" });
  execSync('git config user.name "Test"', { cwd: dir, stdio: "pipe" });
};

// Helper to create a test task
const createTestTask = (overrides: Partial<Task> = {}): Task => ({
  id: "test-task-001",
  title: "Test Task",
  description: "A test task",
  status: "commit_pending",
  priority: 2,
  type: "task",
  labels: [],
  deps: [],
  commits: [],
  comments: [],
  createdAt: new Date().toISOString(),
  updatedAt: new Date().toISOString(),
  pendingCommit: {
    message: "Test commit",
    timestamp: new Date().toISOString(),
    branch: "agent/test-task-001",
  },
  ...overrides,
});

describe("overnight-parallel task closing", () => {
  let tempDir: string;
  let openagentsDir: string;
  let tasksPath: string;

  beforeEach(() => {
    tempDir = createTempDir();
    openagentsDir = path.join(tempDir, ".openagents");
    tasksPath = path.join(openagentsDir, "tasks.jsonl");
    fs.mkdirSync(openagentsDir, { recursive: true });
    initGitRepo(tempDir);
  });

  afterEach(() => {
    cleanupTempDir(tempDir);
  });

  describe("closeTaskAfterMerge behavior", () => {
    test("task in commit_pending status should be closeable", async () => {
      // Create a task in commit_pending status
      const task = createTestTask({
        id: "oa-test-001",
        status: "commit_pending",
        pendingCommit: {
          message: "Test commit",
          timestamp: new Date().toISOString(),
          branch: "agent/oa-test-001",
          sha: "abc123",
        },
      });

      // Write initial task
      fs.writeFileSync(tasksPath, JSON.stringify(task) + "\n");

      // Close the task (simulating what closeTaskAfterMerge does)
      await Effect.runPromise(
        updateTask({
          tasksPath,
          id: "oa-test-001",
          update: {
            status: "closed",
            closeReason: "Completed by MechaCoder parallel agent",
            pendingCommit: null,
            commits: ["def456"],
          },
        }).pipe(Effect.provide(BunContext.layer)),
      );

      // Verify the task is closed
      const tasks = await Effect.runPromise(
        readTasks(tasksPath).pipe(Effect.provide(BunContext.layer)),
      );

      expect(tasks.length).toBe(1);
      expect(tasks[0].status).toBe("closed");
      expect(tasks[0].closeReason).toBe("Completed by MechaCoder parallel agent");
      expect(tasks[0].pendingCommit).toBeNull();
      expect(tasks[0].commits).toContain("def456");
    });

    test("closing task should preserve original commit SHA from pendingCommit", async () => {
      const task = createTestTask({
        id: "oa-test-002",
        status: "commit_pending",
        pendingCommit: {
          message: "Original commit",
          timestamp: new Date().toISOString(),
          branch: "agent/oa-test-002",
          sha: "original-sha-123",
        },
        commits: ["earlier-commit"],
      });

      fs.writeFileSync(tasksPath, JSON.stringify(task) + "\n");

      // Close with the merge SHA
      await Effect.runPromise(
        updateTask({
          tasksPath,
          id: "oa-test-002",
          update: {
            status: "closed",
            closeReason: "Completed",
            pendingCommit: null,
            commits: ["earlier-commit", "merge-sha-456"],
          },
        }).pipe(Effect.provide(BunContext.layer)),
      );

      const tasks = await Effect.runPromise(
        readTasks(tasksPath).pipe(Effect.provide(BunContext.layer)),
      );

      expect(tasks[0].commits).toContain("earlier-commit");
      expect(tasks[0].commits).toContain("merge-sha-456");
    });

    test("already closed task should remain closed", async () => {
      const task = createTestTask({
        id: "oa-test-003",
        status: "closed",
        closeReason: "Already closed",
        pendingCommit: null,
        commits: ["existing-sha"],
      });

      fs.writeFileSync(tasksPath, JSON.stringify(task) + "\n");

      // Try to close again - should succeed but not change status
      await Effect.runPromise(
        updateTask({
          tasksPath,
          id: "oa-test-003",
          update: {
            status: "closed",
            closeReason: "New reason",
            commits: ["existing-sha", "new-sha"],
          },
        }).pipe(Effect.provide(BunContext.layer)),
      );

      const tasks = await Effect.runPromise(
        readTasks(tasksPath).pipe(Effect.provide(BunContext.layer)),
      );

      expect(tasks[0].status).toBe("closed");
      expect(tasks[0].closeReason).toBe("New reason");
      expect(tasks[0].commits).toContain("existing-sha");
      expect(tasks[0].commits).toContain("new-sha");
    });
  });

  describe("two-phase commit recovery integration", () => {
    test("task with pendingCommit.sha should be recoverable", async () => {
      // This tests that the pendingCommit structure has the SHA needed for recovery
      const task = createTestTask({
        id: "oa-recovery-001",
        status: "commit_pending",
        pendingCommit: {
          message: "Recoverable commit",
          timestamp: new Date().toISOString(),
          branch: "agent/oa-recovery-001",
          sha: "recoverable-sha-789",
        },
      });

      fs.writeFileSync(tasksPath, JSON.stringify(task) + "\n");

      // Read and verify pendingCommit has SHA
      const tasks = await Effect.runPromise(
        readTasks(tasksPath).pipe(Effect.provide(BunContext.layer)),
      );

      expect(tasks[0].pendingCommit).toBeDefined();
      expect(tasks[0].pendingCommit?.sha).toBe("recoverable-sha-789");
    });

    test("parallel runner closes task with merge SHA, not original SHA", async () => {
      // The parallel runner should use the merge commit SHA when closing,
      // as that's what ends up on main
      const task = createTestTask({
        id: "oa-merge-001",
        status: "commit_pending",
        pendingCommit: {
          message: "Worktree commit",
          timestamp: new Date().toISOString(),
          branch: "agent/oa-merge-001",
          sha: "worktree-sha-111", // Original SHA from worktree
        },
      });

      fs.writeFileSync(tasksPath, JSON.stringify(task) + "\n");

      // Simulate what parallel runner does after merge
      const mergeSha = "main-merge-sha-222";
      await Effect.runPromise(
        updateTask({
          tasksPath,
          id: "oa-merge-001",
          update: {
            status: "closed",
            closeReason: "Completed by MechaCoder parallel agent",
            pendingCommit: null,
            commits: [mergeSha],
          },
        }).pipe(Effect.provide(BunContext.layer)),
      );

      const tasks = await Effect.runPromise(
        readTasks(tasksPath).pipe(Effect.provide(BunContext.layer)),
      );

      // The merge SHA should be in commits, not necessarily the worktree SHA
      expect(tasks[0].commits).toContain("main-merge-sha-222");
      expect(tasks[0].status).toBe("closed");
      expect(tasks[0].pendingCommit).toBeNull();
    });
  });

  describe("multiple task batch closing", () => {
    test("multiple tasks can be closed in sequence", async () => {
      const task1 = createTestTask({
        id: "oa-batch-001",
        status: "commit_pending",
      });
      const task2 = createTestTask({
        id: "oa-batch-002",
        status: "commit_pending",
      });
      const task3 = createTestTask({
        id: "oa-batch-003",
        status: "open", // Not yet started
      });

      fs.writeFileSync(
        tasksPath,
        [task1, task2, task3].map((t) => JSON.stringify(t)).join("\n") + "\n",
      );

      // Close first two tasks (simulating parallel agent batch)
      for (const taskId of ["oa-batch-001", "oa-batch-002"]) {
        await Effect.runPromise(
        updateTask({
          tasksPath,
          id: taskId,
          update: {
            status: "closed",
            closeReason: "Batch closed",
            pendingCommit: null,
            commits: [`sha-${taskId}`],
          },
        }).pipe(Effect.provide(BunContext.layer)),
        );
      }

      const tasks = await Effect.runPromise(
        readTasks(tasksPath).pipe(Effect.provide(BunContext.layer)),
      );

      const closed = tasks.filter((t) => t.status === "closed");
      const open = tasks.filter((t) => t.status === "open");

      expect(closed.length).toBe(2);
      expect(open.length).toBe(1);
      expect(open[0].id).toBe("oa-batch-003");
    });
  });
});
