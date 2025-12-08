/**
 * HillClimber Store
 *
 * SQLite-backed storage service for the HillClimber optimization system.
 * Follows the Effect service pattern used by InferenceStore.
 */

import { Database } from "bun:sqlite";
import { Context, Effect, Layer } from "effect";
import { createHash } from "crypto";
import type {
  HillClimberConfig,
  HillClimberConfigInput,
  HillClimberRun,
  HillClimberRunInput,
  BestConfig,
  HillClimberStats,
  TaskStats,
} from "./types.js";
import {
  rowToConfig,
  rowToRun,
  rowToBestConfig,
} from "./types.js";

// ============================================================================
// Error Types
// ============================================================================

export class HillClimberStoreError extends Error {
  readonly _tag = "HillClimberStoreError";
  constructor(
    readonly reason: "connection" | "query" | "insert" | "not_found" | "migration",
    override readonly message: string,
    override readonly cause?: unknown,
  ) {
    super(message);
    this.name = "HillClimberStoreError";
  }
}

// ============================================================================
// Service Definition
// ============================================================================

export class HillClimberStore extends Context.Tag("HillClimberStore")<
  HillClimberStore,
  {
    readonly db: Database;

    // Config operations
    readonly saveConfig: (
      config: HillClimberConfigInput,
    ) => Effect.Effect<HillClimberConfig, HillClimberStoreError>;
    readonly getCurrentConfig: (
      taskId: string,
    ) => Effect.Effect<HillClimberConfig | null, HillClimberStoreError>;
    readonly setCurrentConfig: (
      taskId: string,
      configId: number,
    ) => Effect.Effect<void, HillClimberStoreError>;
    readonly getConfigById: (
      id: number,
    ) => Effect.Effect<HillClimberConfig | null, HillClimberStoreError>;
    readonly getConfigByHash: (
      taskId: string,
      configHash: string,
    ) => Effect.Effect<HillClimberConfig | null, HillClimberStoreError>;

    // Run operations
    readonly saveRun: (
      run: HillClimberRunInput,
    ) => Effect.Effect<HillClimberRun, HillClimberStoreError>;
    readonly getRunHistory: (
      taskId: string,
      limit?: number,
    ) => Effect.Effect<HillClimberRun[], HillClimberStoreError>;
    readonly getBestRun: (
      taskId: string,
    ) => Effect.Effect<HillClimberRun | null, HillClimberStoreError>;

    // Best config operations
    readonly updateBestConfig: (
      taskId: string,
      configId: number,
      runId: number,
      score: number,
      passed: boolean,
    ) => Effect.Effect<void, HillClimberStoreError>;
    readonly getBestConfigs: () => Effect.Effect<BestConfig[], HillClimberStoreError>;
    readonly getBestConfigForTask: (
      taskId: string,
    ) => Effect.Effect<BestConfig | null, HillClimberStoreError>;

    // Stats
    readonly getStats: () => Effect.Effect<HillClimberStats, HillClimberStoreError>;
    readonly getTaskStats: (
      taskId: string,
    ) => Effect.Effect<TaskStats | null, HillClimberStoreError>;

    // Utilities
    readonly hashConfig: (config: HillClimberConfigInput) => string;
    readonly ensureDefaultConfig: (
      taskId: string,
    ) => Effect.Effect<HillClimberConfig, HillClimberStoreError>;
  }
>() {}

// ============================================================================
// Helper: Hash a config for deduplication
// ============================================================================

export const hashConfig = (config: HillClimberConfigInput): string => {
  const data = JSON.stringify({
    hint: config.hint,
    useSkills: config.useSkills,
    maxTurnsOverride: config.maxTurnsOverride,
  });
  return createHash("sha256").update(data).digest("hex").slice(0, 16);
};

// ============================================================================
// Implementation
// ============================================================================

export const makeHillClimberStoreLive = (
  dbPath: string,
): Layer.Layer<HillClimberStore, HillClimberStoreError> =>
  Layer.effect(
    HillClimberStore,
    Effect.gen(function* () {
      // Ensure parent directory exists
      const path = require("node:path");
      const fs = require("node:fs");
      const dir = path.dirname(dbPath);
      if (dir !== ".") {
        fs.mkdirSync(dir, { recursive: true });
      }

      // Open database connection
      const db = yield* Effect.try({
        try: () => new Database(dbPath),
        catch: (e) =>
          new HillClimberStoreError(
            "connection",
            `Failed to open database: ${e}`,
          ),
      });

      // Run migration if tables don't exist
      yield* Effect.try({
        try: () => {
          const tableExists = db
            .prepare(
              "SELECT name FROM sqlite_master WHERE type='table' AND name='hillclimber_configs'",
            )
            .get();

          if (!tableExists) {
            // Read and execute migration
            const migrationPath = path.join(
              path.dirname(dbPath),
              "migrations",
              "003_hillclimber.sql",
            );
            if (fs.existsSync(migrationPath)) {
              const migration = fs.readFileSync(migrationPath, "utf-8");
              db.exec(migration);
              console.log("[HillClimberStore] Migration applied successfully");
            } else {
              // Inline migration as fallback
              db.exec(`
                CREATE TABLE IF NOT EXISTS hillclimber_configs (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  task_id TEXT NOT NULL,
                  hint TEXT,
                  use_skills INTEGER DEFAULT 0,
                  max_turns_override INTEGER DEFAULT 30,
                  config_hash TEXT NOT NULL,
                  is_current INTEGER DEFAULT 0,
                  created_at TEXT NOT NULL DEFAULT (datetime('now')),
                  UNIQUE(task_id, config_hash)
                );
                CREATE TABLE IF NOT EXISTS hillclimber_runs (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  run_id TEXT NOT NULL UNIQUE,
                  task_id TEXT NOT NULL,
                  config_id INTEGER NOT NULL REFERENCES hillclimber_configs(id),
                  passed INTEGER NOT NULL,
                  turns INTEGER NOT NULL,
                  duration_ms INTEGER NOT NULL,
                  step_summary TEXT,
                  error_message TEXT,
                  meta_model TEXT,
                  proposed_change TEXT,
                  change_accepted INTEGER DEFAULT 0,
                  score INTEGER NOT NULL,
                  is_best INTEGER DEFAULT 0,
                  created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE TABLE IF NOT EXISTS hillclimber_best_configs (
                  task_id TEXT PRIMARY KEY,
                  config_id INTEGER NOT NULL REFERENCES hillclimber_configs(id),
                  run_id INTEGER NOT NULL REFERENCES hillclimber_runs(id),
                  score INTEGER NOT NULL,
                  pass_count INTEGER DEFAULT 0,
                  total_runs INTEGER DEFAULT 0,
                  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                CREATE INDEX IF NOT EXISTS idx_hc_configs_task ON hillclimber_configs(task_id);
                CREATE INDEX IF NOT EXISTS idx_hc_runs_task ON hillclimber_runs(task_id);
              `);
              console.log("[HillClimberStore] Inline migration applied");
            }
          }
        },
        catch: (e) =>
          new HillClimberStoreError("migration", `Migration failed: ${e}`),
      });

      // Helper: Run SQL in a try-catch with error mapping
      const runSQL = <T>(fn: () => T): Effect.Effect<T, HillClimberStoreError> =>
        Effect.try({
          try: fn,
          catch: (e) => new HillClimberStoreError("query", String(e), e),
        });

      // ========================================================================
      // Config Operations
      // ========================================================================

      const saveConfig = (
        config: HillClimberConfigInput,
      ): Effect.Effect<HillClimberConfig, HillClimberStoreError> =>
        runSQL(() => {
          const hash = hashConfig(config);

          // Try to find existing config with same hash
          const existing = db
            .prepare(
              "SELECT * FROM hillclimber_configs WHERE task_id = ? AND config_hash = ?",
            )
            .get(config.taskId, hash) as any;

          if (existing) {
            return rowToConfig(existing);
          }

          // Insert new config
          const stmt = db.prepare(`
            INSERT INTO hillclimber_configs (task_id, hint, use_skills, max_turns_override, config_hash, is_current)
            VALUES (?, ?, ?, ?, ?, 0)
          `);

          const result = stmt.run(
            config.taskId,
            config.hint,
            config.useSkills ? 1 : 0,
            config.maxTurnsOverride,
            hash,
          );

          const inserted = db
            .prepare("SELECT * FROM hillclimber_configs WHERE id = ?")
            .get(result.lastInsertRowid) as any;

          return rowToConfig(inserted);
        });

      const getCurrentConfig = (
        taskId: string,
      ): Effect.Effect<HillClimberConfig | null, HillClimberStoreError> =>
        runSQL(() => {
          const row = db
            .prepare(
              "SELECT * FROM hillclimber_configs WHERE task_id = ? AND is_current = 1",
            )
            .get(taskId) as any;
          return row ? rowToConfig(row) : null;
        });

      const setCurrentConfig = (
        taskId: string,
        configId: number,
      ): Effect.Effect<void, HillClimberStoreError> =>
        runSQL(() => {
          // Clear current flag for all configs of this task
          db.prepare(
            "UPDATE hillclimber_configs SET is_current = 0 WHERE task_id = ?",
          ).run(taskId);

          // Set current flag for specified config
          db.prepare(
            "UPDATE hillclimber_configs SET is_current = 1 WHERE id = ?",
          ).run(configId);
        });

      const getConfigById = (
        id: number,
      ): Effect.Effect<HillClimberConfig | null, HillClimberStoreError> =>
        runSQL(() => {
          const row = db
            .prepare("SELECT * FROM hillclimber_configs WHERE id = ?")
            .get(id) as any;
          return row ? rowToConfig(row) : null;
        });

      const getConfigByHash = (
        taskId: string,
        configHashVal: string,
      ): Effect.Effect<HillClimberConfig | null, HillClimberStoreError> =>
        runSQL(() => {
          const row = db
            .prepare(
              "SELECT * FROM hillclimber_configs WHERE task_id = ? AND config_hash = ?",
            )
            .get(taskId, configHashVal) as any;
          return row ? rowToConfig(row) : null;
        });

      // ========================================================================
      // Run Operations
      // ========================================================================

      const saveRun = (
        run: HillClimberRunInput,
      ): Effect.Effect<HillClimberRun, HillClimberStoreError> =>
        runSQL(() => {
          const stmt = db.prepare(`
            INSERT INTO hillclimber_runs (
              run_id, task_id, config_id, passed, turns, duration_ms,
              step_summary, error_message, meta_model, proposed_change,
              change_accepted, score, is_best
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
          `);

          const result = stmt.run(
            run.runId,
            run.taskId,
            run.configId,
            run.passed ? 1 : 0,
            run.turns,
            run.durationMs,
            run.stepSummary ? JSON.stringify(run.stepSummary) : null,
            run.errorMessage,
            run.metaModel,
            run.proposedChange,
            run.changeAccepted ? 1 : 0,
            run.score,
          );

          const inserted = db
            .prepare("SELECT * FROM hillclimber_runs WHERE id = ?")
            .get(result.lastInsertRowid) as any;

          return rowToRun(inserted);
        });

      const getRunHistory = (
        taskId: string,
        limit: number = 100,
      ): Effect.Effect<HillClimberRun[], HillClimberStoreError> =>
        runSQL(() => {
          const rows = db
            .prepare(
              "SELECT * FROM hillclimber_runs WHERE task_id = ? ORDER BY created_at DESC LIMIT ?",
            )
            .all(taskId, limit) as any[];
          return rows.map(rowToRun);
        });

      const getBestRun = (
        taskId: string,
      ): Effect.Effect<HillClimberRun | null, HillClimberStoreError> =>
        runSQL(() => {
          const row = db
            .prepare(
              "SELECT * FROM hillclimber_runs WHERE task_id = ? AND is_best = 1",
            )
            .get(taskId) as any;
          return row ? rowToRun(row) : null;
        });

      // ========================================================================
      // Best Config Operations
      // ========================================================================

      const updateBestConfig = (
        taskId: string,
        configId: number,
        runId: number,
        score: number,
        passed: boolean,
      ): Effect.Effect<void, HillClimberStoreError> =>
        runSQL(() => {
          // Check if we have an existing best config
          const existing = db
            .prepare("SELECT * FROM hillclimber_best_configs WHERE task_id = ?")
            .get(taskId) as any;

          if (existing) {
            // Update if this is better or same config
            if (score > existing.score || configId === existing.config_id) {
              // Clear old best flag if changing run
              if (runId !== existing.run_id) {
                db.prepare(
                  "UPDATE hillclimber_runs SET is_best = 0 WHERE task_id = ? AND is_best = 1",
                ).run(taskId);
              }

              // Update best config record
              if (score > existing.score) {
                db.prepare(`
                  UPDATE hillclimber_best_configs
                  SET config_id = ?, run_id = ?, score = ?,
                      pass_count = pass_count + ?, total_runs = total_runs + 1,
                      updated_at = datetime('now')
                  WHERE task_id = ?
                `).run(configId, runId, score, passed ? 1 : 0, taskId);

                // Set new best flag
                db.prepare(
                  "UPDATE hillclimber_runs SET is_best = 1 WHERE id = ?",
                ).run(runId);
              } else {
                // Same config, just update counts
                db.prepare(`
                  UPDATE hillclimber_best_configs
                  SET pass_count = pass_count + ?, total_runs = total_runs + 1,
                      updated_at = datetime('now')
                  WHERE task_id = ?
                `).run(passed ? 1 : 0, taskId);
              }
            }
          } else {
            // Insert new best config
            db.prepare(`
              INSERT INTO hillclimber_best_configs (task_id, config_id, run_id, score, pass_count, total_runs)
              VALUES (?, ?, ?, ?, ?, 1)
            `).run(taskId, configId, runId, score, passed ? 1 : 0);

            // Set best flag on run
            db.prepare(
              "UPDATE hillclimber_runs SET is_best = 1 WHERE id = ?",
            ).run(runId);
          }
        });

      const getBestConfigs = (): Effect.Effect<
        BestConfig[],
        HillClimberStoreError
      > =>
        runSQL(() => {
          const rows = db
            .prepare("SELECT * FROM hillclimber_best_configs ORDER BY task_id")
            .all() as any[];
          return rows.map(rowToBestConfig);
        });

      const getBestConfigForTask = (
        taskId: string,
      ): Effect.Effect<BestConfig | null, HillClimberStoreError> =>
        runSQL(() => {
          const row = db
            .prepare("SELECT * FROM hillclimber_best_configs WHERE task_id = ?")
            .get(taskId) as any;
          return row ? rowToBestConfig(row) : null;
        });

      // ========================================================================
      // Stats
      // ========================================================================

      const getStats = (): Effect.Effect<
        HillClimberStats,
        HillClimberStoreError
      > =>
        runSQL(() => {
          // Total runs and passes
          const totals = db
            .prepare(`
            SELECT
              COUNT(*) as total_runs,
              SUM(passed) as total_passes,
              COUNT(DISTINCT task_id) as unique_tasks
            FROM hillclimber_runs
          `)
            .get() as any;

          // Unique configs
          const configCount = db
            .prepare("SELECT COUNT(*) as count FROM hillclimber_configs")
            .get() as any;

          // Per-task stats
          const taskRows = db
            .prepare(`
            SELECT
              task_id,
              COUNT(*) as total_runs,
              SUM(passed) as pass_count,
              MAX(score) as best_score,
              AVG(turns) as avg_turns,
              MAX(created_at) as last_run_at
            FROM hillclimber_runs
            GROUP BY task_id
          `)
            .all() as any[];

          const byTask: Record<string, TaskStats> = {};
          for (const row of taskRows) {
            // Get current and best config IDs for this task
            const currentConfig = db
              .prepare(
                "SELECT id FROM hillclimber_configs WHERE task_id = ? AND is_current = 1",
              )
              .get(row.task_id) as any;
            const bestConfig = db
              .prepare("SELECT config_id FROM hillclimber_best_configs WHERE task_id = ?")
              .get(row.task_id) as any;

            byTask[row.task_id] = {
              taskId: row.task_id,
              totalRuns: row.total_runs,
              passCount: row.pass_count ?? 0,
              passRate: row.total_runs > 0 ? (row.pass_count ?? 0) / row.total_runs : 0,
              bestScore: row.best_score ?? 0,
              avgTurns: row.avg_turns ?? 0,
              lastRunAt: row.last_run_at,
              currentConfigId: currentConfig?.id ?? null,
              bestConfigId: bestConfig?.config_id ?? null,
            };
          }

          return {
            totalRuns: totals.total_runs ?? 0,
            totalPasses: totals.total_passes ?? 0,
            overallPassRate:
              totals.total_runs > 0
                ? (totals.total_passes ?? 0) / totals.total_runs
                : 0,
            uniqueTasks: totals.unique_tasks ?? 0,
            uniqueConfigs: configCount.count ?? 0,
            byTask,
          };
        });

      const getTaskStats = (
        taskId: string,
      ): Effect.Effect<TaskStats | null, HillClimberStoreError> =>
        runSQL(() => {
          const row = db
            .prepare(`
            SELECT
              task_id,
              COUNT(*) as total_runs,
              SUM(passed) as pass_count,
              MAX(score) as best_score,
              AVG(turns) as avg_turns,
              MAX(created_at) as last_run_at
            FROM hillclimber_runs
            WHERE task_id = ?
            GROUP BY task_id
          `)
            .get(taskId) as any;

          if (!row) return null;

          const currentConfig = db
            .prepare(
              "SELECT id FROM hillclimber_configs WHERE task_id = ? AND is_current = 1",
            )
            .get(taskId) as any;
          const bestConfig = db
            .prepare("SELECT config_id FROM hillclimber_best_configs WHERE task_id = ?")
            .get(taskId) as any;

          return {
            taskId: row.task_id,
            totalRuns: row.total_runs,
            passCount: row.pass_count ?? 0,
            passRate: row.total_runs > 0 ? (row.pass_count ?? 0) / row.total_runs : 0,
            bestScore: row.best_score ?? 0,
            avgTurns: row.avg_turns ?? 0,
            lastRunAt: row.last_run_at,
            currentConfigId: currentConfig?.id ?? null,
            bestConfigId: bestConfig?.config_id ?? null,
          };
        });

      // ========================================================================
      // Utilities
      // ========================================================================

      const ensureDefaultConfig = (
        taskId: string,
      ): Effect.Effect<HillClimberConfig, HillClimberStoreError> =>
        Effect.gen(function* () {
          const current = yield* getCurrentConfig(taskId);
          if (current) return current;

          // Create default config
          const defaultConfig: HillClimberConfigInput = {
            taskId,
            hint: null,
            useSkills: false,
            maxTurnsOverride: 30,
          };

          const saved = yield* saveConfig(defaultConfig);
          yield* setCurrentConfig(taskId, saved.id);

          return { ...saved, isCurrent: true };
        });

      return {
        db,
        saveConfig,
        getCurrentConfig,
        setCurrentConfig,
        getConfigById,
        getConfigByHash,
        saveRun,
        getRunHistory,
        getBestRun,
        updateBestConfig,
        getBestConfigs,
        getBestConfigForTask,
        getStats,
        getTaskStats,
        hashConfig,
        ensureDefaultConfig,
      };
    }),
  );

/**
 * Default HillClimber store layer (uses .openagents/openagents.db)
 */
export const HillClimberStoreLive = makeHillClimberStoreLive(
  ".openagents/openagents.db",
);
