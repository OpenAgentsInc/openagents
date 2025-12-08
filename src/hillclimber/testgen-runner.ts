/**
 * TestGen Runner
 *
 * Main evolution loop for test generation optimization.
 * Runs test generation sessions, analyzes results, and evolves configs.
 */

import { Effect } from "effect"
import { BunContext } from "@effect/platform-bun"
import {
    loadTerminalBenchSuite, TerminalBenchTask, type
} from "../bench/terminal-bench.js"
import { DatabaseLive, DatabaseService } from "../storage/database.js"
import { log, logError } from "./logger.js"
import { analyzeTestGenRun } from "./testgen-analyzer.js"
import {
    applyConfigChange, proposeTestGenConfigChange
} from "./testgen-meta-reasoner.js"
import { computeOverallScore } from "./testgen-scoring.js"
import {
    runTestGenWithStreaming, TestGenEmitter, type
} from "./testgen-service.js"
import { TestGenStore, TestGenStoreLive } from "./testgen-store.js"
import {
    generateTestGenRunId, TestGenConfig, TestGenRunInput, type
} from "./testgen-types.js"

// ============================================================================
// Types
// ============================================================================

export interface TestGenRunnerOptions {
  /** Specific task ID, or undefined to pick random */
  taskId?: string;
  /** Task type filter (e.g., "conversion", "implementation") */
  taskType?: string;
  /** Maximum number of evolution runs */
  maxRuns: number;
  /** Sleep between runs in milliseconds */
  sleepMs: number;
  /** Path to TB suite JSON */
  suitePath: string;
  /** Model override for meta-reasoning */
  modelOverride?: string;
  /** Dry run mode (preview without executing) */
  dryRun?: boolean;
  /** Path to TB2 task directory */
  tb2Path?: string;
}

interface RunnerState {
  totalRuns: number;
  running: boolean;
}

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Pick a random task from the suite.
 */
const pickRandomTask = async (suitePath: string): Promise<string> => {
  const suite = await Effect.runPromise(
    loadTerminalBenchSuite(suitePath).pipe(Effect.provide(BunContext.layer))
  );
  const randomIndex = Math.floor(Math.random() * suite.tasks.length);
  return suite.tasks[randomIndex].id;
};

/**
 * Infer task type from task ID or description.
 */
const inferTaskType = (taskId: string, description: string): string => {
  const descLower = description.toLowerCase();
  if (descLower.includes("convert") || descLower.includes("translation")) {
    return "conversion";
  }
  if (descLower.includes("implement") || descLower.includes("write")) {
    return "implementation";
  }
  if (descLower.includes("debug") || descLower.includes("fix")) {
    return "debugging";
  }
  return "_global_";
};

/**
 * Get trajectory data from database.
 */
const getTrajectory = async (
  sessionId: string,
): Promise<{
  tests: Array<{
    id: string;
    category: string;
    input: string;
    expectedOutput: string | null;
    reasoning: string;
    confidence: number;
  }>;
  reflections: Array<{
    category?: string;
    reflectionText: string;
    action: "refining" | "assessing" | "complete";
  }>;
  environment: unknown;
  comprehensivenessScore: number | null;
  totalTokensUsed: number;
  categoryRounds: Record<string, number>;
}> => {
  return await Effect.runPromise(
    DatabaseService.pipe(
      Effect.flatMap((db) =>
        Effect.try({
          try: () => {
            const row = db.db
              .prepare("SELECT * FROM testgen_trajectories WHERE session_id = ?")
              .get(sessionId) as any;

            if (!row) {
              throw new Error(`Trajectory not found: ${sessionId}`);
            }

            return {
              tests: JSON.parse(row.tests || "[]"),
              reflections: JSON.parse(row.reflections || "[]"),
              environment: JSON.parse(row.environment || "{}"),
              comprehensivenessScore: row.comprehensiveness_score,
              totalTokensUsed: row.total_tokens_used,
              categoryRounds: JSON.parse(row.category_rounds || "{}"),
            };
          },
          catch: (e) => new Error(`Failed to get trajectory: ${e}`),
        })
      ),
      Effect.provide(DatabaseLive),
    )
  );
};

// ============================================================================
// Main Evolution Loop
// ============================================================================

/**
 * Run a single evolution iteration.
 */
const runSingleIteration = async (
  options: TestGenRunnerOptions,
  state: RunnerState,
): Promise<void> => {
  const runId = generateTestGenRunId();
  log(`[TestGenRunner] Starting run ${state.totalRuns + 1}/${options.maxRuns} (${runId})`);

  try {
    // 1. Get current config
    const config = await Effect.runPromise(
      TestGenStore.pipe(
        Effect.flatMap((store) => {
          const taskType = options.taskType || "_global_";
          return store.getCurrentConfig(taskType);
        }),
        Effect.flatMap((config) => {
          if (config) {
            return Effect.succeed(config);
          }
          return TestGenStore.pipe(
            Effect.flatMap((store) => store.ensureDefaultConfig())
          );
        }),
        Effect.provide(TestGenStoreLive),
      )
    );

    log(`[TestGenRunner] Using config v${config.version} (id: ${config.id})`);

    // 2. Pick task
    const taskId = options.taskId || (await pickRandomTask(options.suitePath));
    log(`[TestGenRunner] Selected task: ${taskId}`);

    // 3. Run test generation (silent emitter for evolution mode)
    const sessionId = `tg-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
    const silentEmitter: TestGenEmitter = {
      onStart: () => {},
      onTest: () => {},
      onProgress: () => {},
      onReflection: () => {},
      onComplete: () => {},
      onError: (msg) => {
        logError(`[TestGenRunner] Test generation error: ${msg.error}`);
      },
    };

    await runTestGenWithStreaming(
      options.suitePath,
      taskId,
      sessionId,
      silentEmitter,
      {
        model: config.primaryModel,
        tb2Path: options.tb2Path,
      }
    );

    // 4. Get trajectory and analyze (save is now awaited in runTestGenWithStreaming)
    const trajectory = await getTrajectory(sessionId);
    const analysis = analyzeTestGenRun({
      sessionId,
      taskId,
      taskDescription: "",
      totalTests: trajectory.tests.length,
      totalRounds: Object.values(trajectory.categoryRounds).reduce((sum, r) => sum + r, 0),
      categoryRounds: trajectory.categoryRounds,
      comprehensivenessScore: trajectory.comprehensivenessScore,
      totalTokensUsed: trajectory.totalTokensUsed,
      durationMs: 0,
      tests: trajectory.tests.map((t) => ({
        id: t.id,
        input: t.input,
        expectedOutput: t.expectedOutput,
        reasoning: t.reasoning,
        category: t.category as any,
        confidence: t.confidence,
      })),
      reflections: trajectory.reflections,
      environment: trajectory.environment as any,
      uncertainties: [],
    });

    const finalAnalysis = computeOverallScore(analysis, trajectory.comprehensivenessScore);
    const score = finalAnalysis.overallScore;

    log(`[TestGenRunner] Analysis: score=${score}, balance=${analysis.categoryBalance.toFixed(2)}, anti-cheat=${analysis.antiCheatCoverage.toFixed(2)}, efficiency=${analysis.tokenEfficiency.toFixed(2)}`);

    // 6. Save run with analysis
    const run = await Effect.runPromise(
      TestGenStore.pipe(
        Effect.flatMap((store) =>
          store.saveRun({
            runId,
            sessionId,
            configId: config.id,
            taskId,
            totalTests: trajectory.tests.length,
            comprehensivenessScore: trajectory.comprehensivenessScore,
            durationMs: 0, // TODO: track duration
            totalTokens: trajectory.totalTokensUsed,
            categoryBalance: analysis.categoryBalance,
            antiCheatCoverage: analysis.antiCheatCoverage,
            parameterDiscovery: analysis.parameterDiscovery,
            reflectionEffectiveness: analysis.reflectionEffectiveness,
            tokenEfficiency: analysis.tokenEfficiency,
            score,
            isBest: false,
          })
        ),
        Effect.provide(TestGenStoreLive),
      )
    );

    // 7. Get recent runs for meta-reasoning
    const recentRuns = await Effect.runPromise(
      TestGenStore.pipe(
        Effect.flatMap((store) => store.getRecentRuns(10)),
        Effect.provide(TestGenStoreLive),
      )
    );

    // 8. Meta-reason about improvements
    const taskType = options.taskType || inferTaskType(taskId, "");
    const change = await Effect.runPromise(
      proposeTestGenConfigChange(
        config,
        recentRuns,
        finalAnalysis,
        taskType,
        options.modelOverride,
      ).pipe(
        Effect.provide(TestGenStoreLive),
      )
    );

    log(`[TestGenRunner] Meta-reasoner proposed: ${change.type}`);

    // 9. Apply config change if proposed
    if (change.type !== "keep" && change.changes) {
      const newConfigInput = applyConfigChange(config, change);
      const newConfig = await Effect.runPromise(
        TestGenStore.pipe(
          Effect.flatMap((store) => store.saveConfig(newConfigInput)),
          Effect.flatMap((newConfig) =>
            TestGenStore.pipe(
              Effect.flatMap((store) => store.setCurrentConfig(newConfig.id)),
              Effect.map(() => newConfig)
            )
          ),
          Effect.provide(TestGenStoreLive),
        )
      );

      log(`[TestGenRunner] Created new config v${newConfig.version} (id: ${newConfig.id})`);
    }

    // 10. Update best config if this is better
    const bestConfig = await Effect.runPromise(
      TestGenStore.pipe(
        Effect.flatMap((store) => store.getBestConfig(taskType)),
        Effect.provide(TestGenStoreLive),
      )
    );

    if (!bestConfig || score > bestConfig.score) {
      await Effect.runPromise(
        TestGenStore.pipe(
          Effect.flatMap((store) =>
            store.updateBestConfig(taskType, config.id, run.id, score)
          ),
          Effect.provide(TestGenStoreLive),
        )
      );
      log(`[TestGenRunner] Updated best config for ${taskType} (score: ${score})`);
    }

    // 11. Log progress
    log(`[TestGenRunner] Run ${state.totalRuns + 1} complete: score=${score}, change=${change.type}`);
  } catch (error) {
    logError(`[TestGenRunner] Run ${state.totalRuns + 1} failed`, error);
  }
};

/**
 * Main evolution loop.
 */
export const runTestGenEvolution = async (
  options: TestGenRunnerOptions,
): Promise<void> => {
  const state: RunnerState = {
    totalRuns: 0,
    running: true,
  };

  log(`[TestGenRunner] Starting evolution loop (max runs: ${options.maxRuns})`);

  // Handle SIGINT/SIGTERM for graceful shutdown
  const shutdown = () => {
    log("[TestGenRunner] Shutdown signal received, stopping...");
    state.running = false;
  };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);

  while (state.running && state.totalRuns < options.maxRuns) {
    try {
      await runSingleIteration(options, state);
      state.totalRuns++;

      if (state.running && state.totalRuns < options.maxRuns) {
        log(`[TestGenRunner] Sleeping ${options.sleepMs}ms before next run...`);
        await new Promise((resolve) => setTimeout(resolve, options.sleepMs));
      }
    } catch (error) {
      logError("[TestGenRunner] Iteration failed", error);
      state.totalRuns++;
      // Continue to next iteration even on error
    }
  }

  log(`[TestGenRunner] Evolution complete (${state.totalRuns} runs)`);
};
