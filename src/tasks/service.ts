/**
 * Task Service - SQLite-based task management
 *
 * This service has been migrated from JSONL to SQLite for better performance,
 * reliability, and querying capabilities.
 *
 * Key changes:
 * - All file I/O replaced with SQLite queries via DatabaseService
 * - Same API surface for backward compatibility
 * - Two-phase commit pattern preserved via commit_pending status
 * - Soft deletes via deleted_at timestamp
 */

import { Effect } from "effect";
import { DatabaseService, type SortPolicy } from "../storage/database.js";
import {
  decodeTaskCreate,
  decodeTaskUpdate,
  type Comment,
  type DeletionEntry,
  type Task,
  type TaskCreate,
  type TaskFilter,
  type TaskUpdate,
} from "./schema.js";
import { generateHashId, generateRandomId } from "./id.js";

export class TaskServiceError extends Error {
  readonly _tag = "TaskServiceError";
  constructor(
    readonly reason:
      | "not_found"
      | "read_error"
      | "write_error"
      | "parse_error"
      | "validation_error"
      | "conflict",
    message: string,
  ) {
    super(message);
    this.name = "TaskServiceError";
  }
}

const nowIso = (timestamp?: Date) => (timestamp ?? new Date()).toISOString();

const applyTaskUpdate = (base: Task, update: TaskUpdate): Task => {
  // Create a new object with the base properties, then apply updates
  const merged = { ...base };
  
  // For each property in the update, create a new object with that property updated
  // This avoids trying to assign to readonly properties
  let result = merged;
  for (const [key, value] of Object.entries(update)) {
    if (value !== undefined) {
      result = { ...result, [key]: value };
    }
  }

  return result as Task;
};

/**
 * Read all tasks from database
 *
 * @deprecated tasksPath parameter is ignored (kept for compatibility)
 */
export const readTasks = (
  _tasksPath?: string,
): Effect.Effect<Task[], TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    return yield* db.listTasks({ deleted: false }).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "read_error",
            `Failed to read tasks: ${e.message}`,
          ),
      ),
    );
  });

/**
 * Write tasks to database
 *
 * @deprecated No longer needed with SQLite - use createTask/updateTask instead
 */
export const writeTasks = (
  _tasksPath: string,
  _tasks: Task[],
): Effect.Effect<void, TaskServiceError> =>
  Effect.fail(
    new TaskServiceError(
      "write_error",
      "writeTasks is deprecated - use createTask/updateTask instead",
    ),
  );

/**
 * Create a new task
 */
export const createTask = ({
  tasksPath: _tasksPath,
  task: taskInput,
  idPrefix = "oa",
  idMethod = "hash",
  timestamp,
}: {
  tasksPath?: string;
  task: TaskCreate;
  idPrefix?: string;
  idMethod?: "hash" | "random" | "child";
  timestamp?: Date;
  parentId?: string;
}): Effect.Effect<Task, TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    // Validate input
    const validated = yield* Effect.try({
      try: () => decodeTaskCreate(taskInput),
      catch: (e) =>
        new TaskServiceError(
          "validation_error",
          `Invalid task input: ${e}`,
        ),
    });

    // Generate ID
    let id: string;
    if (idMethod === "hash") {
      const existingTasks = yield* db.listTasks({}).pipe(
        Effect.mapError(
          (e) =>
            new TaskServiceError("read_error", `Failed to list tasks: ${e.message}`),
        ),
      );
      const existingIds = existingTasks.map((t) => t.id);
      const hash = yield* generateHashId(
        idPrefix,
        validated.title,
        validated.description,
        timestamp || new Date(),
      );

      // Find unique ID
      for (let length = 6; length <= hash.length; length++) {
        const candidate = `${idPrefix}-${hash.slice(0, length)}`;
        if (!existingIds.includes(candidate)) {
          id = candidate;
          break;
        }
      }

      if (!id!) {
        let counter = 1;
        while (true) {
          const candidate = `${idPrefix}-${hash.slice(0, 6)}-${counter}`;
          if (!existingIds.includes(candidate)) {
            id = candidate;
            break;
          }
          counter++;
        }
      }
    } else if (idMethod === "random") {
      id = `${idPrefix}-${generateRandomId()}`;
    } else {
      // child method - requires parentId (handled by caller)
      id = `${idPrefix}-${generateRandomId()}`;
    }

    const now = nowIso(timestamp);

    const task = {
      id: id!,
      title: validated.title,
      description: validated.description,
      status: validated.status,
      priority: validated.priority,
      type: validated.type,
      assignee: validated.assignee,
      labels: validated.labels,
      deps: validated.deps,
      commits: [],
      comments: validated.comments,
      createdAt: now,
      updatedAt: now,
      closedAt: undefined,
      closeReason: undefined,
      design: validated.design,
      acceptanceCriteria: validated.acceptanceCriteria,
      notes: validated.notes,
      estimatedMinutes: validated.estimatedMinutes,
      source: validated.source,
      pendingCommit: undefined,
    } satisfies Task;

    yield* db.insertTask(task).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "write_error",
            `Failed to insert task: ${e.message}`,
          ),
      ),
    );

    return task;
  });

/**
 * Update an existing task
 */
export const updateTask = ({
  tasksPath: _tasksPath,
  id,
  update: updateInput,
  timestamp,
}: {
  tasksPath?: string;
  id: string;
  update: TaskUpdate;
  timestamp?: Date;
}): Effect.Effect<Task, TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    // Validate update input
    const validated = yield* Effect.try({
      try: () => decodeTaskUpdate(updateInput),
      catch: (e) =>
        new TaskServiceError(
          "validation_error",
          `Invalid update input: ${e}`,
        ),
    });

    // Get existing task
    const existing = yield* db.getTask(id).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError("read_error", `Failed to get task: ${e.message}`),
      ),
    );

    if (!existing) {
      return yield* Effect.fail(
        new TaskServiceError("not_found", `Task ${id} not found`),
      );
    }

    // Merge updates
    const mergedTask = applyTaskUpdate(existing, validated);
    const updatedTask = {
      ...mergedTask,
      updatedAt: nowIso(timestamp),
    } satisfies Task;

    yield* db.updateTask(id, validated).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "write_error",
            `Failed to update task: ${e.message}`,
          ),
      ),
    );

    return updatedTask;
  });

/**
 * Close a task
 */
export const closeTask = ({
  tasksPath,
  id,
  reason = "Completed",
  timestamp,
  commits,
}: {
  tasksPath?: string;
  id: string;
  reason?: string;
  timestamp?: Date;
  commits?: string[];
}): Effect.Effect<Task, TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    let mergedCommits: string[] | undefined;
    if (commits && commits.length > 0) {
      const db = yield* DatabaseService;
      const existing = yield* db.getTask(id).pipe(
        Effect.mapError(
          (e) =>
            new TaskServiceError("read_error", `Failed to get task: ${e.message}`),
        ),
      );
      if (!existing) {
        return yield* Effect.fail(
          new TaskServiceError("not_found", `Task ${id} not found`),
        );
      }
      mergedCommits = [...(existing.commits ?? []), ...commits];
    }

    return yield* updateTask({
      ...(tasksPath ? { tasksPath } : {}),
      id,
      update: {
        status: "closed",
        closeReason: reason,
        closedAt: nowIso(timestamp),
        ...(mergedCommits ? { commits: mergedCommits } : {}),
      },
      ...(timestamp ? { timestamp } : {}),
    });
  });

/**
 * Reopen a closed task
 */
export const reopenTask = ({
  tasksPath,
  id,
  timestamp,
}: {
  tasksPath?: string;
  id: string;
  timestamp?: Date;
}): Effect.Effect<Task, TaskServiceError, DatabaseService> =>
  updateTask({
    ...(tasksPath ? { tasksPath } : {}),
    id,
    update: {
      status: "open",
      closeReason: undefined,
      closedAt: undefined,
    },
    ...(timestamp ? { timestamp } : {}),
  });

/**
 * Add a comment to a task
 */
export const addComment = ({
  tasksPath,
  taskId,
  comment,
  timestamp,
}: {
  tasksPath?: string;
  taskId: string;
  comment: Omit<Comment, "id" | "createdAt">;
  timestamp?: Date;
}): Effect.Effect<Task, TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    const task = yield* db.getTask(taskId).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError("read_error", `Failed to get task: ${e.message}`),
      ),
    );

    if (!task) {
      return yield* Effect.fail(
        new TaskServiceError("not_found", `Task ${taskId} not found`),
      );
    }

    const newComment: Comment = {
      id: `comment-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`,
      ...comment,
      createdAt: nowIso(timestamp),
    };

    const updatedComments = [...(task.comments ?? []), newComment];

    return yield* updateTask({
      ...(tasksPath ? { tasksPath } : {}),
      id: taskId,
      update: { comments: updatedComments },
      ...(timestamp ? { timestamp } : {}),
    });
  });

/**
 * List comments for a task
 */
export const listComments = ({
  tasksPath: _tasksPath,
  taskId,
}: {
  tasksPath?: string;
  taskId: string;
}): Effect.Effect<Comment[], TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    const task = yield* db.getTask(taskId).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError("read_error", `Failed to get task: ${e.message}`),
      ),
    );

    if (!task) {
      return yield* Effect.fail(
        new TaskServiceError("not_found", `Task ${taskId} not found`),
      );
    }

    return [...(task.comments ?? [])];
  });

/**
 * List tasks with optional filter
 */
export const listTasks = (
  tasksPath?: string,
  filter?: TaskFilter,
): Effect.Effect<Task[], TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    return yield* db.listTasks(filter).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError("read_error", `Failed to list tasks: ${e.message}`),
      ),
    );
  });

/**
 * Get ready tasks (no open blocking dependencies)
 */
export const readyTasks = (
  tasksPath?: string,
  sortPolicy?: SortPolicy,
): Effect.Effect<Task[], TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    return yield* db.getReadyTasks(sortPolicy ?? "hybrid").pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "read_error",
            `Failed to get ready tasks: ${e.message}`,
          ),
      ),
    );
  });

/**
 * Pick the next ready task (highest priority)
 */
export const pickNextTask = (
  tasksPath?: string,
  filter?: TaskFilter,
): Effect.Effect<Task | null, TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const ready = yield* readyTasks(tasksPath, filter?.sortPolicy);

    // Apply additional filters if provided
    let filtered = ready;
    if (filter) {
      filtered = ready.filter((task) => {
        if (filter.status && task.status !== filter.status) return false;
        if (filter.priority !== undefined && task.priority !== filter.priority)
          return false;
        if (filter.type && task.type !== filter.type) return false;
        if (filter.assignee && task.assignee !== filter.assignee) return false;
        if (filter.labels?.some((label) => !task.labels?.includes(label)))
          return false;
        return true;
      });
    }

    return filtered[0] ?? null;
  });

/**
 * Find tasks with specific status
 */
export const findTasksWithStatus = (
  tasksPath?: string,
  status?: string,
): Effect.Effect<Task[], TaskServiceError, DatabaseService> =>
  listTasks(tasksPath, { status: status as any });

/**
 * Find tasks with pending commit (for crash recovery)
 */
export const findTasksWithPendingCommit = (
  tasksPath?: string,
): Effect.Effect<Task[], TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    return yield* db.findTasksWithPendingCommit().pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "read_error",
            `Failed to find pending commits: ${e.message}`,
          ),
      ),
    );
  });

/**
 * Get task statistics
 */
export const getTaskStats = (
  tasksPath?: string,
): Effect.Effect<any, TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    return yield* db.getTaskStats().pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "read_error",
            `Failed to get task stats: ${e.message}`,
          ),
      ),
    );
  });

/**
 * Get stale tasks (older than N days, still open)
 */
export const getStaleTasks = ({
  tasksPath,
  daysOld = 30,
}: {
  tasksPath?: string;
  daysOld?: number;
}): Effect.Effect<Task[], TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    return yield* db.getStaleTasks(daysOld).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "read_error",
            `Failed to get stale tasks: ${e.message}`,
          ),
      ),
    );
  });

/**
 * Get task with all dependencies loaded
 */
export const getTaskWithDeps = (
  tasksPath?: string,
  taskId?: string,
): Effect.Effect<Task | null, TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    if (!taskId) return null;

    const db = yield* DatabaseService;

    return yield* db.getTask(taskId).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError("read_error", `Failed to get task: ${e.message}`),
      ),
    );
  });

/**
 * Search tasks using full-text search
 */
export const searchAllTasks = ({
  tasksPath,
  query,
}: {
  tasksPath?: string;
  query: string;
}): Effect.Effect<Task[], TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    return yield* db.searchTasks(query).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "read_error",
            `Failed to search tasks: ${e.message}`,
          ),
      ),
    );
  });

/**
 * Rename task prefix (bulk update)
 */
export const renameTaskPrefix = ({
  tasksPath,
  oldPrefix,
  newPrefix,
}: {
  tasksPath?: string;
  oldPrefix: string;
  newPrefix: string;
}): Effect.Effect<{ renamed: number }, TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    // Get all tasks with old prefix
    const allTasks = yield* db.listTasks({}).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError("read_error", `Failed to list tasks: ${e.message}`),
      ),
    );

    const tasksToRename = allTasks.filter((t) =>
      t.id.startsWith(`${oldPrefix}-`),
    );

    // Update each task ID
    for (const task of tasksToRename) {
      // This is a complex operation - would need custom SQL
      // For now, return error
      return yield* Effect.fail(
        new TaskServiceError(
          "write_error",
          "Bulk rename not yet implemented for SQLite",
        ),
      );
    }

    return { renamed: tasksToRename.length };
  });

/**
 * Merge multiple tasks into one
 */
export const mergeTasksById = ({
  tasksPath,
  targetId,
  sourceIds,
  reason,
}: {
  tasksPath?: string;
  targetId: string;
  sourceIds: string[];
  reason?: string;
}): Effect.Effect<Task, TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    // Get target task
    const target = yield* db.getTask(targetId).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError("read_error", `Failed to get task: ${e.message}`),
      ),
    );

    if (!target) {
      return yield* Effect.fail(
        new TaskServiceError("not_found", `Target task ${targetId} not found`),
      );
    }

    // Get source tasks
    const sources: Task[] = [];
    for (const sourceId of sourceIds) {
      const source = yield* db.getTask(sourceId).pipe(
        Effect.mapError(
          (e) =>
            new TaskServiceError("read_error", `Failed to get task: ${e.message}`),
        ),
      );
      if (source) sources.push(source);
    }

    // Merge logic: combine comments, commits, labels
    const mergedComments = [
      ...(target.comments ?? []),
      ...sources.flatMap((s) => s.comments ?? []),
    ];

    const mergedCommits = [
      ...(target.commits ?? []),
      ...sources.flatMap((s) => s.commits ?? []),
    ];

    const mergedLabels = [
      ...(target.labels ?? []),
      ...sources.flatMap((s) => s.labels ?? []),
    ].filter((label, index, self) => self.indexOf(label) === index);

    // Update target task
    const updated = yield* updateTask({
      ...(tasksPath ? { tasksPath } : {}),
      id: targetId,
      update: {
        comments: mergedComments,
        commits: mergedCommits,
        labels: mergedLabels,
      },
    });

    // Soft delete source tasks
    for (const sourceId of sourceIds) {
      yield* db.deleteTask(sourceId, true).pipe(
        Effect.mapError(
          (e) =>
            new TaskServiceError(
              "write_error",
              `Failed to delete task: ${e.message}`,
            ),
        ),
      );

      // Record deletion
      yield* db.recordDeletion({
        taskId: sourceId,
        deletedAt: nowIso(),
        reason: reason ?? `Merged into ${targetId}`,
      }).pipe(
        Effect.mapError(
          (e) =>
            new TaskServiceError(
              "write_error",
              `Failed to record deletion: ${e.message}`,
            ),
        ),
      );
    }

    return updated;
  });

/**
 * Archive tasks (soft delete)
 *
 * @deprecated Use soft delete via deleteTask instead
 */
export const archiveTasks = ({
  tasksPath,
  taskIds,
  reason,
}: {
  tasksPath?: string;
  taskIds: string[];
  reason?: string;
}): Effect.Effect<number, TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    for (const taskId of taskIds) {
      yield* db.deleteTask(taskId, true).pipe(
        Effect.mapError(
          (e) =>
            new TaskServiceError(
              "write_error",
              `Failed to archive task: ${e.message}`,
            ),
        ),
      );

      yield* db.recordDeletion({
        taskId,
        deletedAt: nowIso(),
        reason: reason ?? "Archived",
      }).pipe(
        Effect.mapError(
          (e) =>
            new TaskServiceError(
              "write_error",
              `Failed to record deletion: ${e.message}`,
            ),
        ),
      );
    }

    return taskIds.length;
  });

/**
 * Read archived tasks (soft deleted)
 */
export const readArchivedTasks = (
  _tasksPath?: string,
): Effect.Effect<Task[], TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    return yield* db.listTasks({ deleted: true }).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "read_error",
            `Failed to read archived tasks: ${e.message}`,
          ),
      ),
    );
  });

/**
 * Compact tasks (remove non-essential fields from old closed tasks)
 *
 * @deprecated Not needed with SQLite (database handles storage efficiently)
 */
export const compactTasks = ({
  tasksPath,
  daysOld = 90,
  preview = false,
}: {
  tasksPath?: string;
  daysOld?: number;
  preview?: boolean;
}): Effect.Effect<{ compacted: number }, TaskServiceError> =>
  Effect.succeed({ compacted: 0 });

/**
 * Read deletion records
 */
export const readDeletions = (
  _deletionsPath?: string,
): Effect.Effect<DeletionEntry[], TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    return yield* db.getDeletions().pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "read_error",
            `Failed to read deletions: ${e.message}`,
          ),
      ),
    );
  });

/**
 * Write deletion records
 *
 * @deprecated Use recordDeletion instead
 */
export const writeDeletions = (
  _deletionsPath: string,
  _deletions: DeletionEntry[],
): Effect.Effect<void, TaskServiceError> =>
  Effect.fail(
    new TaskServiceError(
      "write_error",
      "writeDeletions is deprecated - use recordDeletion instead",
    ),
  );

/**
 * Record a single deletion
 */
export const recordDeletion = ({
  deletionsPath,
  deletion,
}: {
  deletionsPath?: string;
  deletion: DeletionEntry;
}): Effect.Effect<void, TaskServiceError, DatabaseService> =>
  Effect.gen(function* () {
    const db = yield* DatabaseService;

    return yield* db.recordDeletion(deletion).pipe(
      Effect.mapError(
        (e) =>
          new TaskServiceError(
            "write_error",
            `Failed to record deletion: ${e.message}`,
          ),
      ),
    );
  });

/**
 * Check if content has git conflict markers
 */
export const hasConflictMarkers = (content: string): boolean =>
  /^(<{7}|={7}|>{7})/m.test(content);
