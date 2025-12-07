/**
 * Orchestrator Crash Recovery
 *
 * Handles recovery from crashes that occur during the two-phase commit pattern.
 * When the orchestrator crashes between creating a git commit and updating the task,
 * the task remains in "commit_pending" status with a pendingCommit record.
 *
 * On restart, this module:
 * 1. Finds all tasks in "commit_pending" status
 * 2. Checks if the recorded commit SHA exists in git history
 * 3. If commit exists: completes the transition to "closed"
 * 4. If commit doesn't exist: resets to "in_progress" for retry
 *
 * @module agent/orchestrator/recovery
 */

import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import { findTasksWithPendingCommit, updateTask, type TaskServiceError } from "../../tasks/service.js";
import type { Task, PendingCommit } from "../../tasks/schema.js";

export interface RecoveryResult {
  /** Tasks that were successfully closed (commit existed) */
  readonly closed: readonly Task[];
  /** Tasks that were reset to in_progress (commit didn't exist) */
  readonly reset: readonly Task[];
  /** Tasks that failed recovery */
  readonly failed: readonly { task: Task; error: string }[];
}

export interface RecoveryOptions {
  /** Path to the tasks.jsonl file */
  readonly tasksPath: string;
  /** Working directory for git operations */
  readonly cwd: string;
  /** Emit events during recovery */
  readonly emit?: (event: RecoveryEvent) => void;
}

export type RecoveryEvent =
  | { type: "recovery_start"; pendingCount: number }
  | { type: "commit_found"; taskId: string; sha: string }
  | { type: "commit_not_found"; taskId: string; sha: string | undefined }
  | { type: "task_closed"; taskId: string; sha: string }
  | { type: "task_reset"; taskId: string }
  | { type: "recovery_failed"; taskId: string; error: string }
  | { type: "recovery_complete"; result: RecoveryResult };

/**
 * Check if a commit SHA exists in the git repository.
 */
const commitExists = (
  sha: string,
  cwd: string,
): Effect.Effect<boolean, Error, never> =>
  Effect.tryPromise({
    try: async () => {
      const { execSync } = await import("node:child_process");
      try {
        // git cat-file -t returns the object type if it exists
        execSync(`git cat-file -t ${sha}`, {
          cwd,
          encoding: "utf-8",
          stdio: ["pipe", "pipe", "pipe"],
        });
        return true;
      } catch {
        return false;
      }
    },
    catch: () => new Error(`Failed to check commit ${sha}`),
  });

/**
 * Recover a single task that was in commit_pending state.
 */
const recoverTask = (
  task: Task,
  options: RecoveryOptions,
): Effect.Effect<
  { action: "closed" | "reset"; task: Task },
  TaskServiceError | Error,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    const { tasksPath, cwd, emit } = options;
    const pending = task.pendingCommit as PendingCommit;

    // Check if the commit SHA exists (if we have one)
    if (pending.sha) {
      const exists = yield* commitExists(pending.sha, cwd);

      if (exists) {
        emit?.({ type: "commit_found", taskId: task.id, sha: pending.sha });

        // Commit exists - complete the transition to closed
        const updated = yield* updateTask({
          tasksPath,
          id: task.id,
          update: {
            status: "closed",
            closeReason: "Completed by MechaCoder orchestrator (recovered)",
            pendingCommit: null,
            commits: [...task.commits, pending.sha],
          },
        });

        emit?.({ type: "task_closed", taskId: task.id, sha: pending.sha });
        return { action: "closed" as const, task: updated };
      }
    }

    // No SHA or commit doesn't exist - reset to in_progress
    emit?.({
      type: "commit_not_found",
      taskId: task.id,
      sha: pending.sha,
    });

    const updated = yield* updateTask({
      tasksPath,
      id: task.id,
      update: {
        status: "in_progress",
        pendingCommit: null,
      },
    });

    emit?.({ type: "task_reset", taskId: task.id });
    return { action: "reset" as const, task: updated };
  });

/**
 * Recover all tasks that were interrupted during two-phase commit.
 *
 * This should be called at orchestrator startup to resolve any tasks
 * that were left in "commit_pending" status due to a crash.
 */
export const recoverPendingCommits = (
  options: RecoveryOptions,
): Effect.Effect<RecoveryResult, TaskServiceError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const { tasksPath, emit } = options;

    // Find all tasks with pending commits
    const pending = yield* findTasksWithPendingCommit(tasksPath);

    emit?.({ type: "recovery_start", pendingCount: pending.length });

    if (pending.length === 0) {
      const result: RecoveryResult = { closed: [], reset: [], failed: [] };
      emit?.({ type: "recovery_complete", result });
      return result;
    }

    const closed: Task[] = [];
    const reset: Task[] = [];
    const failed: { task: Task; error: string }[] = [];

    // Process each pending task
    for (const task of pending) {
      const result = yield* recoverTask(task, options).pipe(
        Effect.catchAll((error) =>
          Effect.succeed({
            action: "failed" as const,
            task,
            error: error instanceof Error ? error.message : String(error),
          }),
        ),
      );

      if (result.action === "failed") {
        emit?.({ type: "recovery_failed", taskId: task.id, error: result.error });
        failed.push({ task, error: result.error });
      } else if (result.action === "closed") {
        closed.push(result.task);
      } else {
        reset.push(result.task);
      }
    }

    const recoveryResult: RecoveryResult = { closed, reset, failed };
    emit?.({ type: "recovery_complete", result: recoveryResult });

    return recoveryResult;
  });

/**
 * Check if there are any tasks requiring recovery.
 * Useful for deciding whether to show recovery messages on startup.
 */
export const hasPendingRecovery = (
  tasksPath: string,
): Effect.Effect<boolean, TaskServiceError, FileSystem.FileSystem> =>
  findTasksWithPendingCommit(tasksPath).pipe(
    Effect.map((tasks) => tasks.length > 0),
  );
