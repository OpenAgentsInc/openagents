import { Database } from "bun:sqlite";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import { DatabaseError } from "./database.js";
import { runMigrations } from "./migrations.js";
import type { Task, DeletionEntry } from "../tasks/schema.js";

/**
 * Read tasks directly from JSONL file (for migration only)
 * This bypasses the DatabaseService to read the original JSONL format
 */
const readTasksFromJsonl = (
  jsonlPath: string,
): Effect.Effect<Task[], DatabaseError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;

    const content = yield* fs.readFileString(jsonlPath).pipe(
      Effect.mapError(
        (e) =>
          new DatabaseError("migration", `Failed to read JSONL file: ${e.message}`, e),
      ),
    );

    const lines = content
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0);

    const tasks: Task[] = [];
    for (const line of lines) {
      try {
        const task = JSON.parse(line);
        tasks.push(task);
      } catch (e) {
        throw new DatabaseError(
          "migration",
          `Failed to parse JSONL line: ${e}`,
          e,
        );
      }
    }

    return tasks;
  });

/**
 * Read deletions directly from JSONL file (for migration only)
 */
const readDeletionsFromJsonl = (
  jsonlPath: string,
): Effect.Effect<DeletionEntry[], DatabaseError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;

    // Check if deletions file exists
    const exists = yield* fs.exists(jsonlPath).pipe(
      Effect.mapError(
        (e) => new DatabaseError("migration", `Failed to check deletions file: ${e.message}`, e),
      ),
    );

    if (!exists) {
      return [];
    }

    const content = yield* fs.readFileString(jsonlPath).pipe(
      Effect.mapError(
        (e) => new DatabaseError("migration", `Failed to read deletions JSONL: ${e.message}`, e),
      ),
    );

    const lines = content
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0);

    const deletions: DeletionEntry[] = [];
    for (const line of lines) {
      try {
        const deletion = JSON.parse(line);
        deletions.push(deletion);
      } catch (e) {
        throw new DatabaseError(
          "migration",
          `Failed to parse deletion line: ${e}`,
          e,
        );
      }
    }

    return deletions;
  });

/**
 * Import result summary
 */
export interface ImportResult {
  tasksImported: number;
  dependenciesImported: number;
  deletionsImported: number;
  errors: string[];
  validationPassed: boolean;
}

/**
 * Import tasks from JSONL file to SQLite database
 *
 * This is a ONE-TIME migration script. It:
 * 1. Creates a new SQLite database
 * 2. Runs migrations to set up schema
 * 3. Reads all tasks from JSONL
 * 4. Inserts tasks and dependencies into SQLite
 * 5. Validates data integrity
 *
 * @param jsonlPath Path to tasks.jsonl file
 * @param dbPath Path to SQLite database file (will be created)
 * @returns ImportResult with summary and validation status
 */
export const importTasksFromJsonl = (
  jsonlPath: string,
  dbPath: string,
): Effect.Effect<
  ImportResult,
  DatabaseError,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    console.log("=".repeat(70));
    console.log("JSONL to SQLite Migration");
    console.log("=".repeat(70));
    console.log(`Source: ${jsonlPath}`);
    console.log(`Target: ${dbPath}`);
    console.log("");

    const errors: string[] = [];
    let tasksImported = 0;
    let dependenciesImported = 0;
    let deletionsImported = 0;

    // 1. Create SQLite database
    console.log("Step 1: Creating SQLite database...");
    const db = yield* Effect.try({
      try: () => new Database(dbPath),
      catch: (e) =>
        new DatabaseError("connection", `Failed to create database: ${e}`),
    });
    console.log("✓ Database created");

    // 2. Run migrations
    console.log("\nStep 2: Running migrations...");
    yield* runMigrations(db);

    // 3. Read tasks from JSONL
    console.log("\nStep 3: Reading tasks from JSONL...");
    const tasks = yield* readTasksFromJsonl(jsonlPath);
    console.log(`✓ Read ${tasks.length} tasks`);

    // 4. Import tasks in a transaction
    console.log("\nStep 4: Importing tasks to SQLite...");

    yield* Effect.try({
      try: () => {
        db.transaction(() => {
          const insertTaskStmt = db.prepare(`
            INSERT INTO tasks (
              id, title, description, status, priority, type,
              assignee, close_reason, labels, commits, comments,
              created_at, updated_at, closed_at, pending_commit,
              design, acceptance_criteria, notes, estimated_minutes,
              source_repo, source_discovered_from, source_external_ref
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
          `);

          const insertDepStmt = db.prepare(`
            INSERT INTO task_dependencies (task_id, depends_on_task_id, dependency_type)
            VALUES (?, ?, ?)
          `);

          for (const task of tasks) {
            try {
              // Insert task
              insertTaskStmt.run(
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

              tasksImported++;

              // Insert dependencies
              if (task.deps && task.deps.length > 0) {
                for (const dep of task.deps) {
                  try {
                    insertDepStmt.run(task.id, dep.id, dep.type);
                    dependenciesImported++;
                  } catch (depError) {
                    // Dependency might reference a non-existent task
                    errors.push(
                      `Task ${task.id}: Failed to insert dependency ${dep.id}: ${depError}`,
                    );
                  }
                }
              }
            } catch (taskError) {
              errors.push(`Failed to import task ${task.id}: ${taskError}`);
            }
          }
        })();
      },
      catch: (e) =>
        new DatabaseError("migration", `Transaction failed: ${e}`, e),
    });

    console.log(`✓ Imported ${tasksImported} tasks`);
    console.log(`✓ Imported ${dependenciesImported} dependencies`);

    // 5. Import deletions (if deletions.jsonl exists)
    console.log("\nStep 5: Importing deletions...");

    const deletionsPath = jsonlPath.replace("tasks.jsonl", "deletions.jsonl");
    const deletionsExist = yield* Effect.gen(function* () {
      const fs = yield* FileSystem.FileSystem;
      return yield* fs.exists(deletionsPath);
    }).pipe(
      Effect.orElseSucceed(() => false),
    );

    if (deletionsExist) {
      const deletions = yield* readDeletionsFromJsonl(deletionsPath);

      if (deletions.length > 0) {
        yield* Effect.try({
          try: () => {
            db.transaction(() => {
              const stmt = db.prepare(`
                INSERT INTO task_deletions (task_id, deleted_at, deleted_by, reason)
                VALUES (?, ?, ?, ?)
              `);

              for (const deletion of deletions) {
                try {
                  stmt.run(
                    deletion.taskId,
                    deletion.deletedAt,
                    deletion.deletedBy ?? null,
                    deletion.reason ?? null,
                  );
                  deletionsImported++;
                } catch (delError) {
                  errors.push(
                    `Failed to import deletion ${deletion.taskId}: ${delError}`,
                  );
                }
              }
            })();
          },
          catch: (e) =>
            new DatabaseError("migration", `Deletion import failed: ${e}`, e),
        });

        console.log(`✓ Imported ${deletionsImported} deletions`);
      } else {
        console.log("  No deletions found");
      }
    } else {
      console.log("  No deletions file found (skipping)");
    }

    // 6. Validate data integrity
    console.log("\nStep 6: Validating data integrity...");

    const taskCount = yield* Effect.try({
      try: () => {
        const stmt = db.prepare("SELECT COUNT(*) as count FROM tasks");
        return (stmt.get() as { count: number }).count;
      },
      catch: (e) =>
        new DatabaseError("query", `Failed to count tasks: ${e}`),
    });

    const depCount = yield* Effect.try({
      try: () => {
        const stmt = db.prepare("SELECT COUNT(*) as count FROM task_dependencies");
        return (stmt.get() as { count: number }).count;
      },
      catch: (e) =>
        new DatabaseError("query", `Failed to count dependencies: ${e}`),
    });

    console.log(`  Tasks in SQLite: ${taskCount}`);
    console.log(`  Tasks in JSONL: ${tasks.length}`);
    console.log(`  Dependencies in SQLite: ${depCount}`);
    console.log(`  Dependencies imported: ${dependenciesImported}`);

    const validationPassed =
      taskCount === tasks.length &&
      depCount === dependenciesImported &&
      tasksImported === tasks.length;

    if (validationPassed) {
      console.log("✓ Validation PASSED");
    } else {
      console.log("✗ Validation FAILED");
      if (taskCount !== tasks.length) {
        errors.push(
          `Task count mismatch: SQLite=${taskCount}, JSONL=${tasks.length}`,
        );
      }
      if (depCount !== dependenciesImported) {
        errors.push(
          `Dependency count mismatch: SQLite=${depCount}, Imported=${dependenciesImported}`,
        );
      }
    }

    // 7. Spot check random tasks
    console.log("\nStep 7: Spot checking random tasks...");

    const spotCheckIndexes = [10, 100, 250, 500];
    for (const index of spotCheckIndexes) {
      if (index < tasks.length) {
        const originalTask = tasks[index];
        if (originalTask) {
          const dbTask = yield* Effect.try({
            try: () => {
              const stmt = db.prepare("SELECT * FROM tasks WHERE id = ?");
              return stmt.get(originalTask.id);
            },
            catch: (e) =>
              new DatabaseError("query", `Failed to spot check task: ${e}`),
          });

          if (dbTask) {
            console.log(`  ✓ Task #${index} (${originalTask.id}): Found in SQLite`);
          } else {
            console.log(`  ✗ Task #${index} (${originalTask.id}): NOT FOUND in SQLite`);
            errors.push(`Spot check failed: Task ${originalTask.id} not found`);
          }
        }
      }
    }

    // 8. Final summary
    console.log("\n" + "=".repeat(70));
    console.log("IMPORT SUMMARY");
    console.log("=".repeat(70));
    console.log(`Tasks imported: ${tasksImported}`);
    console.log(`Dependencies imported: ${dependenciesImported}`);
    console.log(`Deletions imported: ${deletionsImported}`);
    console.log(`Errors: ${errors.length}`);
    console.log(`Validation: ${validationPassed ? "PASSED ✓" : "FAILED ✗"}`);

    if (errors.length > 0) {
      console.log("\nErrors:");
      for (const error of errors) {
        console.log(`  - ${error}`);
      }
    }

    console.log("=".repeat(70));

    return {
      tasksImported,
      dependenciesImported,
      deletionsImported,
      errors,
      validationPassed,
    };
  });

/**
 * Dry run - check what would be imported without actually importing
 */
export const dryRunImport = (
  jsonlPath: string,
): Effect.Effect<
  { taskCount: number; depCount: number; hasDeleteions: boolean },
  DatabaseError,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    console.log("Dry run: Analyzing JSONL file...");

    const tasks = yield* readTasksFromJsonl(jsonlPath);

    const depCount = tasks.reduce(
      (sum, task) => sum + (task.deps?.length ?? 0),
      0,
    );

    const deletionsPath = jsonlPath.replace("tasks.jsonl", "deletions.jsonl");
    const hasDeleteions = yield* Effect.gen(function* () {
      const fs = yield* FileSystem.FileSystem;
      return yield* fs.exists(deletionsPath);
    }).pipe(
      Effect.orElseSucceed(() => false),
    );

    console.log(`  Tasks: ${tasks.length}`);
    console.log(`  Dependencies: ${depCount}`);
    console.log(`  Deletions file: ${hasDeleteions ? "Found" : "Not found"}`);

    return {
      taskCount: tasks.length,
      depCount,
      hasDeleteions,
    };
  });
