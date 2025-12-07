import { Database } from "bun:sqlite";
import { Context, Effect, Layer } from "effect";
import type {
  Task,
  TaskFilter,
  Dependency,
  DependencyType,
  Status,
  DeletionEntry,
} from "../tasks/schema.js";

/**
 * DatabaseError - Error type for all database operations
 */
export class DatabaseError extends Error {
  readonly _tag = "DatabaseError";
  constructor(
    readonly reason:
      | "connection"
      | "query"
      | "migration"
      | "constraint"
      | "not_found"
      | "validation",
    override readonly message: string,
    override readonly cause?: unknown,
  ) {
    super(message);
    this.name = "DatabaseError";
  }
}

/**
 * TaskStats - Aggregate statistics about tasks
 */
export interface TaskStats {
  total: number;
  byStatus: Record<Status, number>;
  byPriority: Record<number, number>;
  avgAge: number;
}

/**
 * SortPolicy - How to sort task results
 */
export type SortPolicy = "hybrid" | "priority" | "oldest" | "newest";

/**
 * DatabaseService - Low-level SQLite operations with Effect integration
 */
export class DatabaseService extends Context.Tag("DatabaseService")<
  DatabaseService,
  {
    // Core operations
    readonly db: Database;
    readonly migrate: () => Effect.Effect<void, DatabaseError>;

    // Task CRUD
    readonly insertTask: (task: Task) => Effect.Effect<void, DatabaseError>;
    readonly updateTask: (
      id: string,
      update: Partial<Task>,
    ) => Effect.Effect<void, DatabaseError>;
    readonly getTask: (
      id: string,
    ) => Effect.Effect<Task | null, DatabaseError>;
    readonly listTasks: (
      filter?: TaskFilter & { deleted?: boolean },
    ) => Effect.Effect<Task[], DatabaseError>;
    readonly deleteTask: (
      id: string,
      soft: boolean,
    ) => Effect.Effect<void, DatabaseError>;

    // Dependency operations
    readonly addDependency: (
      taskId: string,
      depId: string,
      type: DependencyType,
    ) => Effect.Effect<void, DatabaseError>;
    readonly removeDependency: (
      taskId: string,
      depId: string,
    ) => Effect.Effect<void, DatabaseError>;
    readonly getDependencies: (
      taskId: string,
    ) => Effect.Effect<Dependency[], DatabaseError>;
    readonly setDependencies: (
      taskId: string,
      deps: Dependency[],
    ) => Effect.Effect<void, DatabaseError>;

    // Specialized queries
    readonly getReadyTasks: (
      sort: SortPolicy,
    ) => Effect.Effect<Task[], DatabaseError>;
    readonly findTasksWithPendingCommit: () => Effect.Effect<
      Task[],
      DatabaseError
    >;
    readonly getTaskStats: () => Effect.Effect<TaskStats, DatabaseError>;
    readonly getStaleTasks: (
      daysOld: number,
    ) => Effect.Effect<Task[], DatabaseError>;
    readonly searchTasks: (
      query: string,
    ) => Effect.Effect<Task[], DatabaseError>;

    // Deletion tracking
    readonly recordDeletion: (
      entry: DeletionEntry,
    ) => Effect.Effect<void, DatabaseError>;
    readonly getDeletions: () => Effect.Effect<DeletionEntry[], DatabaseError>;

    // Transactions
    readonly runInTransaction: <A, E>(
      effect: Effect.Effect<A, E>,
    ) => Effect.Effect<A, E | DatabaseError>;
  }
>() {}

/**
 * Helper: Convert DB row to Task object
 */
const rowToTask = (row: any): Task => ({
  id: row.id,
  title: row.title,
  description: row.description ?? "",
  status: row.status,
  priority: row.priority,
  type: row.type,
  assignee: row.assignee ?? undefined,
  labels: row.labels ? JSON.parse(row.labels) : [],
  deps: [], // Will be loaded separately
  commits: row.commits ? JSON.parse(row.commits) : [],
  comments: row.comments ? JSON.parse(row.comments) : [],
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  closedAt: row.closed_at ?? undefined,
  closeReason: row.close_reason ?? undefined,
  design: row.design ?? undefined,
  acceptanceCriteria: row.acceptance_criteria ?? undefined,
  notes: row.notes ?? undefined,
  estimatedMinutes: row.estimated_minutes ?? undefined,
  pendingCommit: row.pending_commit ? JSON.parse(row.pending_commit) : undefined,
  source: row.source_repo || row.source_discovered_from || row.source_external_ref
    ? {
        repo: row.source_repo ?? undefined,
        discoveredFrom: row.source_discovered_from ?? undefined,
        externalRef: row.source_external_ref ?? undefined,
      }
    : undefined,
});

/**
 * Helper: Apply task filter to SQL WHERE clause
 */
const buildWhereClause = (
  filter?: TaskFilter & { deleted?: boolean },
): { sql: string; params: any[] } => {
  const conditions: string[] = [];
  const params: any[] = [];

  // Soft delete filter
  if (filter?.deleted === false || !filter?.deleted) {
    conditions.push("deleted_at IS NULL");
  } else if (filter?.deleted === true) {
    conditions.push("deleted_at IS NOT NULL");
  }

  // Status filter
  if (filter?.status) {
    conditions.push("status = ?");
    params.push(filter.status);
  }

  // Priority filter
  if (filter?.priority !== undefined) {
    conditions.push("priority = ?");
    params.push(filter.priority);
  }

  // Type filter
  if (filter?.type) {
    conditions.push("type = ?");
    params.push(filter.type);
  }

  // Assignee filter
  if (filter?.assignee) {
    conditions.push("assignee = ?");
    params.push(filter.assignee);
  }

  // Labels filter (check if JSON array contains any of the labels)
  if (filter?.labels && filter.labels.length > 0) {
    const labelConditions = filter.labels.map(() => "json_extract(labels, '$') LIKE ?");
    conditions.push(`(${labelConditions.join(" OR ")})`);
    params.push(...filter.labels.map((l) => `%"${l}"%`));
  }

  const sql = conditions.length > 0 ? `WHERE ${conditions.join(" AND ")}` : "";
  return { sql, params };
};

/**
 * DatabaseLive - Implementation of DatabaseService
 */
export const makeDatabaseLive = (
  dbPath: string,
): Layer.Layer<DatabaseService, DatabaseError> =>
  Layer.effect(
    DatabaseService,
    Effect.gen(function* () {
      // Ensure parent directory exists
      const path = require("node:path");
      const dir = path.dirname(dbPath);
      if (dir !== ".") {
        const fs = require("node:fs");
        fs.mkdirSync(dir, { recursive: true });
      }

      // Open database connection
      const db = yield* Effect.try({
        try: () => new Database(dbPath),
        catch: (e) =>
          new DatabaseError("connection", `Failed to open database: ${e}`),
      });

      // Helper: Run SQL in a try-catch with error mapping
      const runSQL = <T>(fn: () => T): Effect.Effect<T, DatabaseError> =>
        Effect.try({
          try: fn,
          catch: (e) => new DatabaseError("query", String(e), e),
        });

      // Insert task
      const insertTask = (task: Task): Effect.Effect<void, DatabaseError> =>
        runSQL(() => {
          const stmt = db.prepare(`
            INSERT INTO tasks (
              id, title, description, status, priority, type,
              assignee, close_reason, labels, commits, comments,
              created_at, updated_at, closed_at, pending_commit,
              design, acceptance_criteria, notes, estimated_minutes,
              source_repo, source_discovered_from, source_external_ref
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
          `);

          stmt.run(
            task.id,
            task.title,
            task.description,
            task.status,
            task.priority,
            task.type,
            task.assignee ?? null,
            task.closeReason ?? null,
            JSON.stringify(task.labels ?? []),
            JSON.stringify(task.commits ?? []),
            JSON.stringify(task.comments ?? []),
            task.createdAt,
            task.updatedAt,
            task.closedAt ?? null,
            task.pendingCommit ? JSON.stringify(task.pendingCommit) : null,
            task.design ?? null,
            task.acceptanceCriteria ?? null,
            task.notes ?? null,
            task.estimatedMinutes ?? null,
            task.source?.repo ?? null,
            task.source?.discoveredFrom ?? null,
            task.source?.externalRef ?? null,
          );

          // Insert dependencies
          if (task.deps && task.deps.length > 0) {
            const depStmt = db.prepare(`
              INSERT INTO task_dependencies (task_id, depends_on_task_id, dependency_type)
              VALUES (?, ?, ?)
            `);
            for (const dep of task.deps) {
              depStmt.run(task.id, dep.id, dep.type);
            }
          }
        });

      // Update task
      const updateTask = (
        id: string,
        update: Partial<Task>,
      ): Effect.Effect<void, DatabaseError> =>
        runSQL(() => {
          const fields: string[] = [];
          const params: any[] = [];

          // Build SET clause dynamically
          if (update.title !== undefined) {
            fields.push("title = ?");
            params.push(update.title);
          }
          if (update.description !== undefined) {
            fields.push("description = ?");
            params.push(update.description);
          }
          if (update.status !== undefined) {
            fields.push("status = ?");
            params.push(update.status);
          }
          if (update.priority !== undefined) {
            fields.push("priority = ?");
            params.push(update.priority);
          }
          if (update.type !== undefined) {
            fields.push("type = ?");
            params.push(update.type);
          }
          if (update.assignee !== undefined) {
            fields.push("assignee = ?");
            params.push(update.assignee);
          }
          if (update.closeReason !== undefined) {
            fields.push("close_reason = ?");
            params.push(update.closeReason);
          }
          if (update.labels !== undefined) {
            fields.push("labels = ?");
            params.push(JSON.stringify(update.labels));
          }
          if (update.commits !== undefined) {
            fields.push("commits = ?");
            params.push(JSON.stringify(update.commits));
          }
          if (update.comments !== undefined) {
            fields.push("comments = ?");
            params.push(JSON.stringify(update.comments));
          }
          if (update.closedAt !== undefined) {
            fields.push("closed_at = ?");
            params.push(update.closedAt);
          }
          if (update.design !== undefined) {
            fields.push("design = ?");
            params.push(update.design);
          }
          if (update.acceptanceCriteria !== undefined) {
            fields.push("acceptance_criteria = ?");
            params.push(update.acceptanceCriteria);
          }
          if (update.notes !== undefined) {
            fields.push("notes = ?");
            params.push(update.notes);
          }
          if (update.estimatedMinutes !== undefined) {
            fields.push("estimated_minutes = ?");
            params.push(update.estimatedMinutes);
          }
          if (update.pendingCommit !== undefined) {
            fields.push("pending_commit = ?");
            params.push(update.pendingCommit ? JSON.stringify(update.pendingCommit) : null);
          }

          // Always update updated_at
          fields.push("updated_at = ?");
          params.push(new Date().toISOString());

          params.push(id);

          const sql = `UPDATE tasks SET ${fields.join(", ")} WHERE id = ?`;
          const stmt = db.prepare(sql);
          stmt.run(...params);

          // Update dependencies if provided
          if (update.deps !== undefined) {
            // Delete existing dependencies
            db.prepare("DELETE FROM task_dependencies WHERE task_id = ?").run(id);

            // Insert new dependencies
            if (update.deps.length > 0) {
              const depStmt = db.prepare(`
                INSERT INTO task_dependencies (task_id, depends_on_task_id, dependency_type)
                VALUES (?, ?, ?)
              `);
              for (const dep of update.deps) {
                depStmt.run(id, dep.id, dep.type);
              }
            }
          }
        });

      // Get task by ID
      const getTask = (
        id: string,
      ): Effect.Effect<Task | null, DatabaseError> =>
        runSQL(() => {
          const stmt = db.prepare("SELECT * FROM tasks WHERE id = ?");
          const row = stmt.get(id);

          if (!row) return null;

          const taskBase = rowToTask(row);

          // Load dependencies
          const depStmt = db.prepare(`
            SELECT depends_on_task_id as id, dependency_type as type
            FROM task_dependencies
            WHERE task_id = ?
          `);
          const deps = depStmt.all(id) as Dependency[];

          return { ...taskBase, deps } as Task;
        });

      // List tasks with filter
      const listTasks = (
        filter?: TaskFilter & { deleted?: boolean },
      ): Effect.Effect<Task[], DatabaseError> =>
        runSQL(() => {
          const { sql: whereClause, params } = buildWhereClause(filter);

          const stmt = db.prepare(`SELECT * FROM tasks ${whereClause} ORDER BY created_at DESC`);
          const rows = stmt.all(...params);

          const tasks = rows.map((row: any) => {
            const taskBase = rowToTask(row);

            // Load dependencies for each task
            const depStmt = db.prepare(`
              SELECT depends_on_task_id as id, dependency_type as type
              FROM task_dependencies
              WHERE task_id = ?
            `);
            const deps = depStmt.all(taskBase.id) as Dependency[];

            return { ...taskBase, deps } as Task;
          });

          return tasks;
        });

      // Delete task (soft or hard)
      const deleteTask = (
        id: string,
        soft: boolean,
      ): Effect.Effect<void, DatabaseError> =>
        runSQL(() => {
          if (soft) {
            const stmt = db.prepare("UPDATE tasks SET deleted_at = ? WHERE id = ?");
            stmt.run(new Date().toISOString(), id);
          } else {
            const stmt = db.prepare("DELETE FROM tasks WHERE id = ?");
            stmt.run(id);
          }
        });

      // Add dependency
      const addDependency = (
        taskId: string,
        depId: string,
        type: DependencyType,
      ): Effect.Effect<void, DatabaseError> =>
        runSQL(() => {
          const stmt = db.prepare(`
            INSERT INTO task_dependencies (task_id, depends_on_task_id, dependency_type)
            VALUES (?, ?, ?)
            ON CONFLICT DO NOTHING
          `);
          stmt.run(taskId, depId, type);
        });

      // Remove dependency
      const removeDependency = (
        taskId: string,
        depId: string,
      ): Effect.Effect<void, DatabaseError> =>
        runSQL(() => {
          const stmt = db.prepare(`
            DELETE FROM task_dependencies
            WHERE task_id = ? AND depends_on_task_id = ?
          `);
          stmt.run(taskId, depId);
        });

      // Get dependencies
      const getDependencies = (
        taskId: string,
      ): Effect.Effect<Dependency[], DatabaseError> =>
        runSQL(() => {
          const stmt = db.prepare(`
            SELECT depends_on_task_id as id, dependency_type as type
            FROM task_dependencies
            WHERE task_id = ?
          `);
          return stmt.all(taskId) as Dependency[];
        });

      // Set dependencies (replace all)
      const setDependencies = (
        taskId: string,
        deps: Dependency[],
      ): Effect.Effect<void, DatabaseError> =>
        runSQL(() => {
          // Delete existing
          db.prepare("DELETE FROM task_dependencies WHERE task_id = ?").run(taskId);

          // Insert new
          if (deps.length > 0) {
            const stmt = db.prepare(`
              INSERT INTO task_dependencies (task_id, depends_on_task_id, dependency_type)
              VALUES (?, ?, ?)
            `);
            for (const dep of deps) {
              stmt.run(taskId, dep.id, dep.type);
            }
          }
        });

      // Get ready tasks (no open blocking dependencies)
      const getReadyTasks = (
        sort: SortPolicy,
      ): Effect.Effect<Task[], DatabaseError> =>
        runSQL(() => {
          let orderBy = "t.priority ASC, t.created_at ASC"; // hybrid default

          if (sort === "priority") {
            orderBy = "t.priority ASC, t.created_at DESC";
          } else if (sort === "oldest") {
            orderBy = "t.created_at ASC";
          } else if (sort === "newest") {
            orderBy = "t.created_at DESC";
          }

          const stmt = db.prepare(`
            SELECT t.* FROM tasks t
            WHERE t.status = 'open'
            AND t.deleted_at IS NULL
            AND NOT EXISTS (
              SELECT 1 FROM task_dependencies td
              JOIN tasks blocker ON td.depends_on_task_id = blocker.id
              WHERE td.task_id = t.id
              AND td.dependency_type IN ('blocks', 'parent-child')
              AND blocker.status IN ('open', 'in_progress')
              AND blocker.deleted_at IS NULL
            )
            ORDER BY ${orderBy}
          `);

          const rows = stmt.all();
          const tasks = rows.map((row: any) => {
            const taskBase = rowToTask(row);

            // Load dependencies
            const depStmt = db.prepare(`
              SELECT depends_on_task_id as id, dependency_type as type
              FROM task_dependencies
              WHERE task_id = ?
            `);
            const deps = depStmt.all(taskBase.id) as Dependency[];

            return { ...taskBase, deps } as Task;
          });

          return tasks;
        });

      // Find tasks with pending commit (for crash recovery)
      const findTasksWithPendingCommit = (): Effect.Effect<
        Task[],
        DatabaseError
      > =>
        runSQL(() => {
          const stmt = db.prepare(`
            SELECT * FROM tasks
            WHERE status = 'commit_pending'
            AND deleted_at IS NULL
          `);

          const rows = stmt.all();
          return rows.map((row: any) => {
            const taskBase = rowToTask(row);

            // Load dependencies
            const depStmt = db.prepare(`
              SELECT depends_on_task_id as id, dependency_type as type
              FROM task_dependencies
              WHERE task_id = ?
            `);
            const deps = depStmt.all(taskBase.id) as Dependency[];

            return { ...taskBase, deps } as Task;
          });
        });

      // Get task statistics
      const getTaskStats = (): Effect.Effect<TaskStats, DatabaseError> =>
        runSQL(() => {
          const totalStmt = db.prepare("SELECT COUNT(*) as count FROM tasks WHERE deleted_at IS NULL");
          const total = (totalStmt.get() as any).count;

          const byStatusStmt = db.prepare(`
            SELECT status, COUNT(*) as count
            FROM tasks
            WHERE deleted_at IS NULL
            GROUP BY status
          `);
          const byStatusRows = byStatusStmt.all() as Array<{ status: Status; count: number }>;
          const byStatus: Record<Status, number> = {
            open: 0,
            in_progress: 0,
            blocked: 0,
            closed: 0,
            commit_pending: 0,
          };
          for (const row of byStatusRows) {
            byStatus[row.status] = row.count;
          }

          const byPriorityStmt = db.prepare(`
            SELECT priority, COUNT(*) as count
            FROM tasks
            WHERE deleted_at IS NULL
            GROUP BY priority
          `);
          const byPriorityRows = byPriorityStmt.all() as Array<{ priority: number; count: number }>;
          const byPriority: Record<number, number> = {};
          for (const row of byPriorityRows) {
            byPriority[row.priority] = row.count;
          }

          const avgAgeStmt = db.prepare(`
            SELECT AVG(julianday('now') - julianday(created_at)) as avg_age
            FROM tasks
            WHERE deleted_at IS NULL
          `);
          const avgAge = ((avgAgeStmt.get() as any).avg_age ?? 0) as number;

          return {
            total,
            byStatus,
            byPriority,
            avgAge,
          };
        });

      // Get stale tasks (older than N days, still open)
      const getStaleTasks = (
        daysOld: number,
      ): Effect.Effect<Task[], DatabaseError> =>
        runSQL(() => {
          const stmt = db.prepare(`
            SELECT * FROM tasks
            WHERE status IN ('open', 'in_progress')
            AND deleted_at IS NULL
            AND julianday('now') - julianday(updated_at) > ?
            ORDER BY updated_at ASC
          `);

          const rows = stmt.all(daysOld);
          return rows.map((row: any) => {
            const taskBase = rowToTask(row);

            // Load dependencies
            const depStmt = db.prepare(`
              SELECT depends_on_task_id as id, dependency_type as type
              FROM task_dependencies
              WHERE task_id = ?
            `);
            const deps = depStmt.all(taskBase.id) as Dependency[];

            return { ...taskBase, deps } as Task;
          });
        });

      // Search tasks using FTS5
      const searchTasks = (
        query: string,
      ): Effect.Effect<Task[], DatabaseError> =>
        runSQL(() => {
          const stmt = db.prepare(`
            SELECT t.* FROM tasks t
            WHERE t.id IN (
              SELECT id FROM tasks_fts WHERE tasks_fts MATCH ?
            )
            AND t.deleted_at IS NULL
            ORDER BY t.updated_at DESC
          `);

          const rows = stmt.all(query);
          return rows.map((row: any) => {
            const taskBase = rowToTask(row);

            // Load dependencies
            const depStmt = db.prepare(`
              SELECT depends_on_task_id as id, dependency_type as type
              FROM task_dependencies
              WHERE task_id = ?
            `);
            const deps = depStmt.all(taskBase.id) as Dependency[];

            return { ...taskBase, deps } as Task;
          });
        });

      // Record deletion
      const recordDeletion = (
        entry: DeletionEntry,
      ): Effect.Effect<void, DatabaseError> =>
        runSQL(() => {
          const stmt = db.prepare(`
            INSERT INTO task_deletions (task_id, deleted_at, deleted_by, reason)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(task_id) DO UPDATE SET
              deleted_at = excluded.deleted_at,
              deleted_by = excluded.deleted_by,
              reason = excluded.reason
          `);
          stmt.run(
            entry.taskId,
            entry.deletedAt,
            entry.deletedBy ?? null,
            entry.reason ?? null,
          );
        });

      // Get deletions
      const getDeletions = (): Effect.Effect<DeletionEntry[], DatabaseError> =>
        runSQL(() => {
          const stmt = db.prepare("SELECT * FROM task_deletions ORDER BY deleted_at DESC");
          const rows = stmt.all();
          return rows.map((row: any) => ({
            taskId: row.task_id,
            deletedAt: row.deleted_at,
            deletedBy: row.deleted_by ?? undefined,
            reason: row.reason ?? undefined,
          }));
        });

      // Run in transaction
      const runInTransaction = <A, E>(
        effect: Effect.Effect<A, E>,
      ): Effect.Effect<A, E | DatabaseError> =>
        Effect.gen(function* () {
          return yield* Effect.try({
            try: () => db.transaction(() => Effect.runSync(effect))(),
            catch: (e) => new DatabaseError("query", `Transaction failed: ${e}`, e),
          });
        });

      // Migrate (placeholder - will be implemented in migrations.ts)
      const migrate = (): Effect.Effect<void, DatabaseError> =>
        Effect.succeed(undefined);

      return {
        db,
        migrate,
        insertTask,
        updateTask,
        getTask,
        listTasks,
        deleteTask,
        addDependency,
        removeDependency,
        getDependencies,
        setDependencies,
        getReadyTasks,
        findTasksWithPendingCommit,
        getTaskStats,
        getStaleTasks,
        searchTasks,
        recordDeletion,
        getDeletions,
        runInTransaction,
      };
    }),
  );

/**
 * Default database layer (uses .openagents/openagents.db in current directory)
 */
export const DatabaseLive = makeDatabaseLive(".openagents/openagents.db");
