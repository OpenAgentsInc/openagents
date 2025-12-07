/**
 * Tests for Two-Phase Commit Crash Recovery
 *
 * These tests verify that the orchestrator can recover from crashes
 * that occur between creating a git commit and updating the task status.
 */

import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import { execSync } from "node:child_process";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect, Layer } from "effect";
import { BunContext } from "@effect/platform-bun";
import {
  recoverPendingCommits,
  hasPendingRecovery,
  type RecoveryEvent,
} from "./recovery.js";
import { createTask, updateTask, readTasks } from "../../tasks/service.js";
import type { PendingCommit, TaskCreate } from "../../tasks/schema.js";
import { DatabaseService } from "../../storage/database.js";
import { makeTestDatabaseLayer } from "../../tasks/test-helpers.js";

const runWithBun = <A, E>(
  effect: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path | DatabaseService>,
): Promise<A> =>
  Effect.gen(function* () {
    const { layer: dbLayer, cleanup } = yield* makeTestDatabaseLayer();
    const testLayer = Layer.mergeAll(BunContext.layer, dbLayer);

    try {
      return yield* effect.pipe(Effect.provide(testLayer));
    } finally {
      cleanup();
    }
  }).pipe(
    Effect.provide(BunContext.layer),
    Effect.runPromise
  );

const makeTask = (task: Partial<TaskCreate>): TaskCreate => ({
  title: "Test task",
  description: "",
  status: "open",
  priority: 2,
  type: "task",
  labels: [],
  deps: [],
  comments: [],
  ...task,
});

describe("Two-Phase Commit Recovery", () => {
  let tempDir: string;
  let openagentsDir: string;
  let tasksPath: string;

  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "recovery-test-"));
    openagentsDir = path.join(tempDir, ".openagents");
    tasksPath = path.join(openagentsDir, "tasks.jsonl");
    fs.mkdirSync(openagentsDir, { recursive: true });

    // Initialize a git repo for commit verification tests
    execSync("git init", { cwd: tempDir, stdio: "pipe" });
    execSync("git config user.email 'test@test.com'", { cwd: tempDir, stdio: "pipe" });
    execSync("git config user.name 'Test User'", { cwd: tempDir, stdio: "pipe" });
    fs.writeFileSync(path.join(tempDir, "README.md"), "# Test\n");
    execSync("git add -A", { cwd: tempDir, stdio: "pipe" });
    execSync('git commit -m "Initial commit"', { cwd: tempDir, stdio: "pipe" });
  });

  afterEach(() => {
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  describe("hasPendingRecovery", () => {
    test("returns false when no tasks file exists", async () => {
      const result = await runWithBun(hasPendingRecovery(tasksPath));
      expect(result).toBe(false);
    });

    test("returns false when no tasks are in commit_pending status", async () => {
      // Create a normal task
      await runWithBun(
        createTask({
          tasksPath,
          task: makeTask({ title: "Normal task", status: "in_progress" }),
        }),
      );

      const result = await runWithBun(hasPendingRecovery(tasksPath));
      expect(result).toBe(false);
    });

    test("returns true when a task is in commit_pending status", async () => {
      // Create a task in commit_pending status
      const task = await runWithBun(
        createTask({
          tasksPath,
          task: makeTask({ title: "Pending task", status: "open" }),
        }),
      );

      await runWithBun(
        updateTask({
          tasksPath,
          id: task.id,
          update: {
            status: "commit_pending",
            pendingCommit: {
              message: "Test commit",
              timestamp: new Date().toISOString(),
              branch: "main",
            },
          },
        }),
      );

      const result = await runWithBun(hasPendingRecovery(tasksPath));
      expect(result).toBe(true);
    });
  });

  describe("recoverPendingCommits", () => {
    test("returns empty result when no pending commits", async () => {
      // Create a normal task
      await runWithBun(
        createTask({
          tasksPath,
          task: makeTask({ title: "Normal task", status: "in_progress" }),
        }),
      );

      const events: RecoveryEvent[] = [];
      const result = await runWithBun(
        recoverPendingCommits({
          tasksPath,
          cwd: tempDir,
          emit: (e) => events.push(e),
        }),
      );

      expect(result.closed).toHaveLength(0);
      expect(result.reset).toHaveLength(0);
      expect(result.failed).toHaveLength(0);
    });

    test("closes task when commit SHA exists", async () => {
      // Create a real commit
      fs.writeFileSync(path.join(tempDir, "test.txt"), "test content");
      execSync("git add -A", { cwd: tempDir, stdio: "pipe" });
      execSync('git commit -m "Test commit"', { cwd: tempDir, stdio: "pipe" });
      const sha = execSync("git rev-parse HEAD", { cwd: tempDir, encoding: "utf-8" }).trim();

      // Create a task in commit_pending with the real SHA
      const task = await runWithBun(
        createTask({
          tasksPath,
          task: makeTask({ title: "Task with commit", status: "open" }),
        }),
      );

      await runWithBun(
        updateTask({
          tasksPath,
          id: task.id,
          update: {
            status: "commit_pending",
            pendingCommit: {
              message: "Test commit",
              timestamp: new Date().toISOString(),
              branch: "main",
              sha,
            },
          },
        }),
      );

      const events: RecoveryEvent[] = [];
      const result = await runWithBun(
        recoverPendingCommits({
          tasksPath,
          cwd: tempDir,
          emit: (e) => events.push(e),
        }),
      );

      expect(result.closed).toHaveLength(1);
      expect(result.reset).toHaveLength(0);
      expect(result.closed[0].id).toBe(task.id);
      expect(result.closed[0].status).toBe("closed");
      expect(result.closed[0].commits).toContain(sha);
      expect(result.closed[0].pendingCommit).toBeNull();

      // Verify events
      const closedEvent = events.find((e) => e.type === "task_closed");
      expect(closedEvent).toBeDefined();
    });

    test("resets task when commit SHA does not exist", async () => {
      // Create a task in commit_pending with a fake SHA
      const task = await runWithBun(
        createTask({
          tasksPath,
          task: makeTask({ title: "Task with missing commit", status: "open" }),
        }),
      );

      await runWithBun(
        updateTask({
          tasksPath,
          id: task.id,
          update: {
            status: "commit_pending",
            pendingCommit: {
              message: "Test commit",
              timestamp: new Date().toISOString(),
              branch: "main",
              sha: "0000000000000000000000000000000000000000", // Non-existent SHA
            },
          },
        }),
      );

      const events: RecoveryEvent[] = [];
      const result = await runWithBun(
        recoverPendingCommits({
          tasksPath,
          cwd: tempDir,
          emit: (e) => events.push(e),
        }),
      );

      expect(result.reset).toHaveLength(1);
      expect(result.closed).toHaveLength(0);
      expect(result.reset[0].id).toBe(task.id);
      expect(result.reset[0].status).toBe("in_progress");
      expect(result.reset[0].pendingCommit).toBeNull();

      // Verify events
      const resetEvent = events.find((e) => e.type === "task_reset");
      expect(resetEvent).toBeDefined();
    });

    test("resets task when no SHA in pending commit", async () => {
      // Create a task in commit_pending without a SHA (crash before git commit)
      const task = await runWithBun(
        createTask({
          tasksPath,
          task: makeTask({ title: "Task before commit", status: "open" }),
        }),
      );

      await runWithBun(
        updateTask({
          tasksPath,
          id: task.id,
          update: {
            status: "commit_pending",
            pendingCommit: {
              message: "Test commit",
              timestamp: new Date().toISOString(),
              branch: "main",
              // No SHA - crash happened before git commit completed
            },
          },
        }),
      );

      const events: RecoveryEvent[] = [];
      const result = await runWithBun(
        recoverPendingCommits({
          tasksPath,
          cwd: tempDir,
          emit: (e) => events.push(e),
        }),
      );

      expect(result.reset).toHaveLength(1);
      expect(result.closed).toHaveLength(0);
      expect(result.reset[0].status).toBe("in_progress");
    });

    test("handles multiple pending tasks", async () => {
      // Create a real commit
      fs.writeFileSync(path.join(tempDir, "test.txt"), "test content");
      execSync("git add -A", { cwd: tempDir, stdio: "pipe" });
      execSync('git commit -m "Test commit"', { cwd: tempDir, stdio: "pipe" });
      const realSha = execSync("git rev-parse HEAD", { cwd: tempDir, encoding: "utf-8" }).trim();

      // Task 1: Has real commit SHA (should close)
      const task1 = await runWithBun(
        createTask({
          tasksPath,
          task: makeTask({ title: "Task 1 - has commit", status: "open" }),
        }),
      );
      await runWithBun(
        updateTask({
          tasksPath,
          id: task1.id,
          update: {
            status: "commit_pending",
            pendingCommit: {
              message: "Commit 1",
              timestamp: new Date().toISOString(),
              sha: realSha,
            },
          },
        }),
      );

      // Task 2: Has fake SHA (should reset)
      const task2 = await runWithBun(
        createTask({
          tasksPath,
          task: makeTask({ title: "Task 2 - missing commit", status: "open" }),
        }),
      );
      await runWithBun(
        updateTask({
          tasksPath,
          id: task2.id,
          update: {
            status: "commit_pending",
            pendingCommit: {
              message: "Commit 2",
              timestamp: new Date().toISOString(),
              sha: "abcdef1234567890abcdef1234567890abcdef12",
            },
          },
        }),
      );

      const result = await runWithBun(
        recoverPendingCommits({
          tasksPath,
          cwd: tempDir,
        }),
      );

      expect(result.closed).toHaveLength(1);
      expect(result.reset).toHaveLength(1);
      expect(result.closed[0].id).toBe(task1.id);
      expect(result.reset[0].id).toBe(task2.id);
    });

    test("emits recovery_start and recovery_complete events", async () => {
      const task = await runWithBun(
        createTask({
          tasksPath,
          task: makeTask({ title: "Pending task", status: "open" }),
        }),
      );

      await runWithBun(
        updateTask({
          tasksPath,
          id: task.id,
          update: {
            status: "commit_pending",
            pendingCommit: {
              message: "Test",
              timestamp: new Date().toISOString(),
            },
          },
        }),
      );

      const events: RecoveryEvent[] = [];
      await runWithBun(
        recoverPendingCommits({
          tasksPath,
          cwd: tempDir,
          emit: (e) => events.push(e),
        }),
      );

      const startEvent = events.find((e) => e.type === "recovery_start");
      expect(startEvent).toBeDefined();
      expect((startEvent as any).pendingCount).toBe(1);

      const completeEvent = events.find((e) => e.type === "recovery_complete");
      expect(completeEvent).toBeDefined();
    });
  });

  describe("integration with task service", () => {
    test("commit_pending tasks are not picked by pickNextTask", async () => {
      // Create a task and put it in commit_pending
      const task = await runWithBun(
        createTask({
          tasksPath,
          task: makeTask({ title: "Pending task", status: "open", priority: 0 }),
        }),
      );

      await runWithBun(
        updateTask({
          tasksPath,
          id: task.id,
          update: {
            status: "commit_pending",
            pendingCommit: {
              message: "Test",
              timestamp: new Date().toISOString(),
            },
          },
        }),
      );

      // Verify the task is in commit_pending
      const tasks = await runWithBun(readTasks(tasksPath));
      const pendingTask = tasks.find((t) => t.id === task.id);
      expect(pendingTask?.status).toBe("commit_pending");

      // Import and use pickNextTask to verify it's skipped
      const { pickNextTask } = await import("../../tasks/service.js");
      const nextTask = await runWithBun(pickNextTask(tasksPath));

      // Should not pick the commit_pending task
      expect(nextTask?.id).not.toBe(task.id);
    });

    test("recovered closed tasks have proper close metadata", async () => {
      // Create a real commit
      fs.writeFileSync(path.join(tempDir, "test.txt"), "test content");
      execSync("git add -A", { cwd: tempDir, stdio: "pipe" });
      execSync('git commit -m "Test commit"', { cwd: tempDir, stdio: "pipe" });
      const sha = execSync("git rev-parse HEAD", { cwd: tempDir, encoding: "utf-8" }).trim();

      const task = await runWithBun(
        createTask({
          tasksPath,
          task: makeTask({ title: "Task to recover", status: "open" }),
        }),
      );

      await runWithBun(
        updateTask({
          tasksPath,
          id: task.id,
          update: {
            status: "commit_pending",
            pendingCommit: {
              message: "Test commit",
              timestamp: new Date().toISOString(),
              sha,
            },
          },
        }),
      );

      await runWithBun(
        recoverPendingCommits({
          tasksPath,
          cwd: tempDir,
        }),
      );

      // Verify the task was properly closed
      const tasks = await runWithBun(readTasks(tasksPath));
      const recoveredTask = tasks.find((t) => t.id === task.id);

      expect(recoveredTask?.status).toBe("closed");
      expect(recoveredTask?.closedAt).toBeDefined();
      expect(recoveredTask?.closeReason).toContain("recovered");
      expect(recoveredTask?.commits).toContain(sha);
      expect(recoveredTask?.pendingCommit).toBeNull();
    });
  });
});

describe("PendingCommit schema", () => {
  let tempDir: string;
  let tasksPath: string;

  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "pending-commit-test-"));
    const openagentsDir = path.join(tempDir, ".openagents");
    tasksPath = path.join(openagentsDir, "tasks.jsonl");
    fs.mkdirSync(openagentsDir, { recursive: true });
  });

  afterEach(() => {
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  test("pendingCommit field is properly serialized and deserialized", async () => {
    const task = await runWithBun(
      createTask({
        tasksPath,
        task: makeTask({ title: "Test task", status: "open" }),
      }),
    );

    const pendingCommit: PendingCommit = {
      message: "Test commit message",
      timestamp: "2025-01-01T12:00:00.000Z",
      branch: "feature-branch",
      sha: "abc123def456",
    };

    await runWithBun(
      updateTask({
        tasksPath,
        id: task.id,
        update: {
          status: "commit_pending",
          pendingCommit,
        },
      }),
    );

    // Read the task back and verify pendingCommit
    const tasks = await runWithBun(readTasks(tasksPath));
    const updatedTask = tasks.find((t) => t.id === task.id);

    expect(updatedTask?.pendingCommit).toBeDefined();
    expect(updatedTask?.pendingCommit?.message).toBe("Test commit message");
    expect(updatedTask?.pendingCommit?.timestamp).toBe("2025-01-01T12:00:00.000Z");
    expect(updatedTask?.pendingCommit?.branch).toBe("feature-branch");
    expect(updatedTask?.pendingCommit?.sha).toBe("abc123def456");
  });

  test("pendingCommit can be cleared with null", async () => {
    const task = await runWithBun(
      createTask({
        tasksPath,
        task: makeTask({ title: "Test task", status: "open" }),
      }),
    );

    // Set pendingCommit
    await runWithBun(
      updateTask({
        tasksPath,
        id: task.id,
        update: {
          status: "commit_pending",
          pendingCommit: {
            message: "Test",
            timestamp: new Date().toISOString(),
          },
        },
      }),
    );

    // Clear pendingCommit
    await runWithBun(
      updateTask({
        tasksPath,
        id: task.id,
        update: {
          status: "closed",
          pendingCommit: null,
        },
      }),
    );

    const tasks = await runWithBun(readTasks(tasksPath));
    const updatedTask = tasks.find((t) => t.id === task.id);

    expect(updatedTask?.status).toBe("closed");
    expect(updatedTask?.pendingCommit).toBeNull();
  });
});
