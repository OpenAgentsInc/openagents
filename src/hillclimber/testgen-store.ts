/**
 * TestGen Store
 *
 * SQLite-backed storage service for the TestGen evolution system.
 * Follows the Effect service pattern used by HillClimberStore.
 */

import { Database } from "bun:sqlite";
import { Context, Effect, Layer } from "effect";
import { createHash } from "crypto";
import type {
  TestGenConfig,
  TestGenConfigInput,
  TestGenRun,
  TestGenRunInput,
  TestGenBestConfig,
  TestGenStats,
  TestGenTaskStats,
} from "./testgen-types.js";
import {
  rowToTestGenConfig,
  rowToTestGenRun,
  rowToTestGenBestConfig,
} from "./testgen-types.js";

// ============================================================================
// Error Types
// ============================================================================

export class TestGenStoreError extends Error {
  readonly _tag = "TestGenStoreError";
  constructor(
    readonly reason: "connection" | "query" | "insert" | "not_found" | "migration",
    override readonly message: string,
    override readonly cause?: unknown,
  ) {
    super(message);
    this.name = "TestGenStoreError";
  }
}

// ============================================================================
// Service Definition
// ============================================================================

export class TestGenStore extends Context.Tag("TestGenStore")<
  TestGenStore,
  {
    readonly db: Database;

    // Config operations
    readonly saveConfig: (
      config: TestGenConfigInput,
    ) => Effect.Effect<TestGenConfig, TestGenStoreError>;
    readonly getCurrentConfig: (
      taskType?: string,
    ) => Effect.Effect<TestGenConfig | null, TestGenStoreError>;
    readonly setCurrentConfig: (
      configId: number,
    ) => Effect.Effect<void, TestGenStoreError>;
    readonly getConfigById: (
      id: number,
    ) => Effect.Effect<TestGenConfig | null, TestGenStoreError>;
    readonly getConfigByHash: (
      configHash: string,
    ) => Effect.Effect<TestGenConfig | null, TestGenStoreError>;
    readonly ensureDefaultConfig: () => Effect.Effect<TestGenConfig, TestGenStoreError>;

    // Run operations
    readonly saveRun: (
      run: TestGenRunInput,
    ) => Effect.Effect<TestGenRun, TestGenStoreError>;
    readonly getRunHistory: (
      taskId: string,
      limit?: number,
    ) => Effect.Effect<TestGenRun[], TestGenStoreError>;
    readonly getRecentRuns: (
      limit?: number,
    ) => Effect.Effect<TestGenRun[], TestGenStoreError>;

    // Best config operations
    readonly getBestConfig: (
      taskType: string,
    ) => Effect.Effect<TestGenBestConfig | null, TestGenStoreError>;
    readonly updateBestConfig: (
      taskType: string,
      configId: number,
      runId: number,
      score: number,
    ) => Effect.Effect<void, TestGenStoreError>;

    // Stats
    readonly getStats: () => Effect.Effect<TestGenStats, TestGenStoreError>;
    readonly getTaskStats: (
      taskId: string,
    ) => Effect.Effect<TestGenTaskStats | null, TestGenStoreError>;

    // Utilities
    readonly hashConfig: (config: TestGenConfigInput) => string;
  }
>() {}

// ============================================================================
// Helper: Hash a config for deduplication
// ============================================================================

export const hashTestGenConfig = (config: TestGenConfigInput): string => {
  const data = JSON.stringify({
    temperature: config.temperature,
    maxTokens: config.maxTokens,
    minTestsPerCategory: config.minTestsPerCategory,
    maxTestsPerCategory: config.maxTestsPerCategory,
    maxRoundsPerCategory: config.maxRoundsPerCategory,
    environmentWeight: config.environmentWeight,
    antiCheatWeight: config.antiCheatWeight,
    precisionWeight: config.precisionWeight,
    categoryOrder: config.categoryOrder,
    primaryModel: config.primaryModel,
    reflectionModel: config.reflectionModel,
    minComprehensivenessScore: config.minComprehensivenessScore,
    targetComprehensivenessScore: config.targetComprehensivenessScore,
  });
  return createHash("sha256").update(data).digest("hex").slice(0, 16);
};

// ============================================================================
// Implementation
// ============================================================================

export const makeTestGenStoreLive = (
  dbPath: string,
): Layer.Layer<TestGenStore, TestGenStoreError> =>
  Layer.effect(
    TestGenStore,
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
          new TestGenStoreError(
            "connection",
            `Failed to open database: ${e}`,
          ),
      });

      // Run migration if tables don't exist
      yield* Effect.try({
        try: () => {
          const tableExists = db
            .prepare(
              "SELECT name FROM sqlite_master WHERE type='table' AND name='testgen_configs'",
            )
            .get();

          if (!tableExists) {
            // Read and execute migration
            const migrationPath = path.join(
              path.dirname(dbPath),
              "migrations",
              "005_testgen_evolution.sql",
            );
            if (fs.existsSync(migrationPath)) {
              const migration = fs.readFileSync(migrationPath, "utf-8");
              db.exec(migration);
              console.log("[TestGenStore] Migration applied successfully");
            } else {
              console.warn("[TestGenStore] Migration file not found, tables may not exist");
            }
          }
        },
        catch: (e) =>
          new TestGenStoreError("migration", `Migration failed: ${e}`),
      });

      // Helper: Run SQL in a try-catch with error mapping
      const runSQL = <T>(fn: () => T): Effect.Effect<T, TestGenStoreError> =>
        Effect.try({
          try: fn,
          catch: (e) => new TestGenStoreError("query", String(e), e),
        });

      // ========================================================================
      // Config Operations
      // ========================================================================

      const saveConfig = (
        config: TestGenConfigInput,
      ): Effect.Effect<TestGenConfig, TestGenStoreError> =>
        runSQL(() => {
          const hash = hashTestGenConfig(config);

          // Try to find existing config with same hash
          const existing = db
            .prepare("SELECT * FROM testgen_configs WHERE config_hash = ?")
            .get(hash) as any;

          if (existing) {
            return rowToTestGenConfig(existing);
          }

          // Insert new config
          const stmt = db.prepare(`
            INSERT INTO testgen_configs (
              version, temperature, max_tokens, min_tests_per_category,
              max_tests_per_category, max_rounds_per_category,
              environment_weight, anti_cheat_weight, precision_weight,
              category_order, category_prompts, anti_cheat_prompt,
              reflection_prompt, primary_model, reflection_model,
              min_comprehensiveness_score, target_comprehensiveness_score,
              config_hash, is_current
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
          `);

          const result = stmt.run(
            config.version ?? "1.0.0",
            config.temperature ?? 0.3,
            config.maxTokens ?? 2048,
            config.minTestsPerCategory ?? 2,
            config.maxTestsPerCategory ?? 5,
            config.maxRoundsPerCategory ?? 3,
            config.environmentWeight ?? 0.7,
            config.antiCheatWeight ?? 0.8,
            config.precisionWeight ?? 0.6,
            JSON.stringify(config.categoryOrder ?? ["anti_cheat", "existence", "correctness", "boundary", "integration"]),
            config.categoryPrompts ? JSON.stringify(config.categoryPrompts) : null,
            config.antiCheatPrompt ?? null,
            config.reflectionPrompt ?? null,
            config.primaryModel ?? "local",
            config.reflectionModel ?? "local",
            config.minComprehensivenessScore ?? 7.0,
            config.targetComprehensivenessScore ?? 8.5,
            hash,
          );

          const inserted = db
            .prepare("SELECT * FROM testgen_configs WHERE id = ?")
            .get(result.lastInsertRowid) as any;

          return rowToTestGenConfig(inserted);
        });

      const getCurrentConfig = (
        taskType?: string,
      ): Effect.Effect<TestGenConfig | null, TestGenStoreError> =>
        runSQL(() => {
          // If taskType specified, check best configs first
          if (taskType && taskType !== "_global_") {
            const bestConfig = db
              .prepare("SELECT config_id FROM testgen_best_configs WHERE task_type = ?")
              .get(taskType) as any;
            if (bestConfig) {
              const config = db
                .prepare("SELECT * FROM testgen_configs WHERE id = ?")
                .get(bestConfig.config_id) as any;
              if (config) return rowToTestGenConfig(config);
            }
          }

          // Fall back to global current config
          const row = db
            .prepare("SELECT * FROM testgen_configs WHERE is_current = 1")
            .get() as any;
          return row ? rowToTestGenConfig(row) : null;
        });

      const setCurrentConfig = (
        configId: number,
      ): Effect.Effect<void, TestGenStoreError> =>
        runSQL(() => {
          // Clear current flag for all configs
          db.prepare("UPDATE testgen_configs SET is_current = 0").run();

          // Set current flag for specified config
          db.prepare("UPDATE testgen_configs SET is_current = 1 WHERE id = ?").run(configId);
        });

      const getConfigById = (
        id: number,
      ): Effect.Effect<TestGenConfig | null, TestGenStoreError> =>
        runSQL(() => {
          const row = db
            .prepare("SELECT * FROM testgen_configs WHERE id = ?")
            .get(id) as any;
          return row ? rowToTestGenConfig(row) : null;
        });

      const getConfigByHash = (
        configHash: string,
      ): Effect.Effect<TestGenConfig | null, TestGenStoreError> =>
        runSQL(() => {
          const row = db
            .prepare("SELECT * FROM testgen_configs WHERE config_hash = ?")
            .get(configHash) as any;
          return row ? rowToTestGenConfig(row) : null;
        });

      // ========================================================================
      // Run Operations
      // ========================================================================

      const saveRun = (
        run: TestGenRunInput,
      ): Effect.Effect<TestGenRun, TestGenStoreError> =>
        runSQL(() => {
          const stmt = db.prepare(`
            INSERT INTO testgen_runs (
              run_id, session_id, config_id, task_id,
              total_tests, comprehensiveness_score, duration_ms, total_tokens,
              category_balance, anti_cheat_coverage, parameter_discovery,
              reflection_effectiveness, token_efficiency,
              meta_model, proposed_change, change_accepted, score, is_best
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
          `);

          const result = stmt.run(
            run.runId,
            run.sessionId,
            run.configId,
            run.taskId,
            run.totalTests,
            run.comprehensivenessScore ?? null,
            run.durationMs,
            run.totalTokens,
            run.categoryBalance ?? null,
            run.antiCheatCoverage ?? null,
            run.parameterDiscovery ?? null,
            run.reflectionEffectiveness ?? null,
            run.tokenEfficiency ?? null,
            run.metaModel ?? null,
            run.proposedChange ?? null,
            run.changeAccepted ? 1 : 0,
            run.score,
          );

          const inserted = db
            .prepare("SELECT * FROM testgen_runs WHERE id = ?")
            .get(result.lastInsertRowid) as any;

          return rowToTestGenRun(inserted);
        });

      const getRunHistory = (
        taskId: string,
        limit: number = 100,
      ): Effect.Effect<TestGenRun[], TestGenStoreError> =>
        runSQL(() => {
          const rows = db
            .prepare(
              "SELECT * FROM testgen_runs WHERE task_id = ? ORDER BY created_at DESC LIMIT ?",
            )
            .all(taskId, limit) as any[];
          return rows.map(rowToTestGenRun);
        });

      const getRecentRuns = (
        limit: number = 50,
      ): Effect.Effect<TestGenRun[], TestGenStoreError> =>
        runSQL(() => {
          const rows = db
            .prepare(
              "SELECT * FROM testgen_runs ORDER BY created_at DESC LIMIT ?",
            )
            .all(limit) as any[];
          return rows.map(rowToTestGenRun);
        });

      // ========================================================================
      // Best Config Operations
      // ========================================================================

      const getBestConfig = (
        taskType: string,
      ): Effect.Effect<TestGenBestConfig | null, TestGenStoreError> =>
        runSQL(() => {
          const row = db
            .prepare("SELECT * FROM testgen_best_configs WHERE task_type = ?")
            .get(taskType) as any;
          return row ? rowToTestGenBestConfig(row) : null;
        });

      const updateBestConfig = (
        taskType: string,
        configId: number,
        runId: number,
        score: number,
      ): Effect.Effect<void, TestGenStoreError> =>
        runSQL(() => {
          // Check if we have an existing best config
          const existing = db
            .prepare("SELECT * FROM testgen_best_configs WHERE task_type = ?")
            .get(taskType) as any;

          if (existing) {
            // Update if this is better
            if (score > existing.score) {
              // Clear old best flag if changing run
              if (runId !== existing.run_id) {
                db.prepare(
                  "UPDATE testgen_runs SET is_best = 0 WHERE id = ?",
                ).run(existing.run_id);
              }

              // Update best config record
              db.prepare(`
                UPDATE testgen_best_configs
                SET config_id = ?, run_id = ?, score = ?,
                    total_runs = total_runs + 1,
                    updated_at = datetime('now')
                WHERE task_type = ?
              `).run(configId, runId, score, taskType);

              // Set new best flag
              db.prepare("UPDATE testgen_runs SET is_best = 1 WHERE id = ?").run(runId);
            } else {
              // Same or worse, just update count
              db.prepare(`
                UPDATE testgen_best_configs
                SET total_runs = total_runs + 1,
                    updated_at = datetime('now')
                WHERE task_type = ?
              `).run(taskType);
            }
          } else {
            // Insert new best config
            db.prepare(`
              INSERT INTO testgen_best_configs (task_type, config_id, run_id, score, total_runs)
              VALUES (?, ?, ?, ?, 1)
            `).run(taskType, configId, runId, score);

            // Set best flag on run
            db.prepare("UPDATE testgen_runs SET is_best = 1 WHERE id = ?").run(runId);
          }
        });

      // ========================================================================
      // Stats
      // ========================================================================

      const getStats = (): Effect.Effect<TestGenStats, TestGenStoreError> =>
        runSQL(() => {
          // Total runs and configs
          const totals = db
            .prepare(`
            SELECT
              COUNT(*) as total_runs,
              AVG(score) as average_score,
              MAX(score) as best_score,
              AVG(comprehensiveness_score) as avg_comprehensiveness,
              AVG(token_efficiency) as avg_token_efficiency
            FROM testgen_runs
          `)
            .get() as any;

          const configCount = db
            .prepare("SELECT COUNT(*) as count FROM testgen_configs")
            .get() as any;

          const evolutionCount = db
            .prepare("SELECT COUNT(*) as count FROM testgen_evolution")
            .get() as any;

          return {
            totalRuns: totals.total_runs ?? 0,
            totalConfigs: configCount.count ?? 0,
            averageScore: totals.average_score ?? 0,
            bestScore: totals.best_score ?? 0,
            averageComprehensiveness: totals.avg_comprehensiveness ?? 0,
            averageTokenEfficiency: totals.avg_token_efficiency ?? 0,
            configEvolutionCount: evolutionCount.count ?? 0,
          };
        });

      const getTaskStats = (
        taskId: string,
      ): Effect.Effect<TestGenTaskStats | null, TestGenStoreError> =>
        runSQL(() => {
          const row = db
            .prepare(`
            SELECT
              task_id,
              COUNT(*) as total_runs,
              AVG(score) as average_score,
              MAX(score) as best_score,
              AVG(comprehensiveness_score) as avg_comprehensiveness,
              AVG(token_efficiency) as avg_token_efficiency
            FROM testgen_runs
            WHERE task_id = ?
            GROUP BY task_id
          `)
            .get(taskId) as any;

          if (!row) return null;

          const bestRun = db
            .prepare(
              "SELECT config_id FROM testgen_runs WHERE task_id = ? AND is_best = 1",
            )
            .get(taskId) as any;

          return {
            taskId: row.task_id,
            totalRuns: row.total_runs,
            averageScore: row.average_score ?? 0,
            bestScore: row.best_score ?? 0,
            bestConfigId: bestRun?.config_id ?? null,
            averageComprehensiveness: row.avg_comprehensiveness ?? 0,
            averageTokenEfficiency: row.avg_token_efficiency ?? 0,
          };
        });

      // ========================================================================
      // Utilities
      // ========================================================================

      const ensureDefaultConfig = (): Effect.Effect<TestGenConfig, TestGenStoreError> =>
        Effect.gen(function* () {
          const current = yield* getCurrentConfig();
          if (current) return current;

          // Create default config
          const defaultConfig: TestGenConfigInput = {
            version: "1.0.0",
            temperature: 0.3,
            maxTokens: 2048,
            minTestsPerCategory: 2,
            maxTestsPerCategory: 5,
            maxRoundsPerCategory: 3,
            environmentWeight: 0.7,
            antiCheatWeight: 0.8,
            precisionWeight: 0.6,
            categoryOrder: ["existence", "format", "happy_path", "boundary", "edge_case", "invalid_input", "integration"],
            primaryModel: "local",
            reflectionModel: "local",
            minComprehensivenessScore: 7.0,
            targetComprehensivenessScore: 8.5,
          };

          const saved = yield* saveConfig(defaultConfig);
          yield* setCurrentConfig(saved.id);

          return { ...saved, isCurrent: true };
        });

      return {
        db,
        saveConfig,
        getCurrentConfig,
        setCurrentConfig,
        getConfigById,
        getConfigByHash,
        ensureDefaultConfig,
        saveRun,
        getRunHistory,
        getRecentRuns,
        getBestConfig,
        updateBestConfig,
        getStats,
        getTaskStats,
        hashConfig: hashTestGenConfig,
      };
    }),
  );

/**
 * Default TestGen store layer (uses .openagents/openagents.db)
 */
export const TestGenStoreLive = makeTestGenStoreLive(".openagents/openagents.db");

