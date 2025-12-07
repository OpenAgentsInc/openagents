/**
 * Trainer Service
 *
 * Orchestrates training runs on Terminal-Bench or custom task sets.
 * Integrates with the Gym for task execution and Archivist for trajectory recording.
 */

import { Effect, Context, Layer } from "effect";
import { Gym, makeGymLive, type GymError } from "./gym.js";
import { ArchivistService, makeArchivistServiceLive, type ArchivistError } from "../archivist/service.js";
import type { SkillStoreError } from "../skills/store.js";
import type { MemoryStoreError } from "../memory/store.js";
import type { TrajectoryStoreError } from "../archivist/store.js";
import type { PatternExtractorError } from "../archivist/extractor.js";
import type {
  TrainingTask,
  TrainingConfig,
  TrainingRun,
  TrainingStats,
  BenchmarkResult,
  TBSubset,
} from "./schema.js";
import {
  DEFAULT_TRAINING_CONFIG,
  calculateStats,
  createTrainingRun,
  createTask,
  TB_SUBSETS,
} from "./schema.js";

// --- Error Types ---

export class TrainerError extends Error {
  readonly _tag = "TrainerError";
  constructor(
    readonly reason: string,
    message: string,
    override readonly cause?: Error,
  ) {
    super(message);
    this.name = "TrainerError";
  }

  static from(e: GymError | ArchivistError): TrainerError {
    return new TrainerError(e._tag, e.message, e);
  }
}

// --- Service Interface ---

export interface ITrainerService {
  // --- Training Runs ---

  /** Start a new training run */
  readonly startRun: (
    tasks: TrainingTask[],
    config?: Partial<TrainingConfig>,
  ) => Effect.Effect<TrainingRun, TrainerError>;

  /** Run a Terminal-Bench subset */
  readonly runBenchmark: (
    subset: TBSubset,
    config?: Partial<TrainingConfig>,
  ) => Effect.Effect<BenchmarkResult, TrainerError>;

  /** Run custom tasks */
  readonly runTasks: (
    tasks: TrainingTask[],
    config?: Partial<TrainingConfig>,
  ) => Effect.Effect<TrainingRun, TrainerError>;

  /** Get current run status */
  readonly getRunStatus: (runId: string) => Effect.Effect<TrainingRun | null, TrainerError>;

  /** Cancel a running run */
  readonly cancelRun: (runId: string) => Effect.Effect<void, TrainerError>;

  // --- Task Management ---

  /** Create a task from a prompt */
  readonly createTask: (
    prompt: string,
    options?: {
      expectedBehavior?: string;
      difficulty?: number;
      category?: string;
      tags?: string[];
    },
  ) => Effect.Effect<TrainingTask, never>;

  /** Load Terminal-Bench tasks */
  readonly loadBenchmarkTasks: (
    subset: TBSubset,
  ) => Effect.Effect<TrainingTask[], TrainerError>;

  // --- Statistics ---

  /** Get stats for a run */
  readonly getRunStats: (runId: string) => Effect.Effect<TrainingStats | null, TrainerError>;

  /** Get all completed runs */
  readonly getCompletedRuns: () => Effect.Effect<TrainingRun[], TrainerError>;

  /** Get benchmark history */
  readonly getBenchmarkHistory: (
    subset?: TBSubset,
  ) => Effect.Effect<BenchmarkResult[], TrainerError>;
}

// --- Service Tag ---

export class TrainerService extends Context.Tag("TrainerService")<
  TrainerService,
  ITrainerService
>() { }

// --- In-Memory Storage ---

interface TrainerStore {
  runs: Map<string, TrainingRun>;
  benchmarkResults: BenchmarkResult[];
}

const createStore = (): TrainerStore => ({
  runs: new Map(),
  benchmarkResults: [],
});

// --- Implementation ---

const makeTrainerService = (): Effect.Effect<ITrainerService, never, Gym | ArchivistService> =>
  Effect.gen(function* () {
    const gym = yield* Gym;
    const archivist = yield* ArchivistService;

    const store = createStore();

    const mapGymError = Effect.mapError(TrainerError.from);
    const mapArchivistError = Effect.mapError(TrainerError.from);

    const createTaskOp = (
      prompt: string,
      options?: {
        expectedBehavior?: string;
        difficulty?: number;
        category?: string;
        tags?: string[];
      },
    ): Effect.Effect<TrainingTask, never> =>
      Effect.succeed(
        createTask(prompt, {
          ...(options?.expectedBehavior ? { expectedBehavior: options.expectedBehavior } : {}),
          ...(options?.difficulty ? { difficulty: options.difficulty } : {}),
          ...(options?.category ? { category: options.category } : {}),
          ...(options?.tags ? { tags: options.tags } : {}),
        }),
      );

    const loadBenchmarkTasks = (subset: TBSubset): Effect.Effect<TrainingTask[], TrainerError> =>
      Effect.gen(function* () {
        // For now, create synthetic tasks based on subset
        // In production, this would load from Terminal-Bench files
        const count = TB_SUBSETS[subset].count;
        const tasks: TrainingTask[] = [];

        // Generate representative tasks for each category
        const categories = ["bash", "git", "file-ops", "debugging", "testing"];
        const difficulties = [1, 2, 3, 4, 5];

        for (let i = 0; i < count; i++) {
          const category = categories[i % categories.length];
          const difficulty = difficulties[Math.floor(i / (count / 5)) % 5] || 3;

          tasks.push(
            createTask(
              generateTaskPrompt(category, i),
              {
                id: `tb-${subset.toLowerCase()}-${i.toString().padStart(3, "0")}`,
                difficulty,
                category,
                tags: ["terminal-bench", subset.toLowerCase(), category],
                source: "terminal-bench",
              },
            ),
          );
        }

        return tasks;
      });

    const startRun = (
      tasks: TrainingTask[],
      config?: Partial<TrainingConfig>,
    ): Effect.Effect<TrainingRun, TrainerError> =>
      Effect.gen(function* () {
        const startTime = Date.now();
        const fullConfig = { ...DEFAULT_TRAINING_CONFIG, ...config };
        const run = createTrainingRun(fullConfig);
        run.tasks = tasks.slice(0, fullConfig.maxTasks);

        store.runs.set(run.id, run);

        // Emit run start HUD message
        fullConfig.onHudMessage?.({
          type: "trainer_run_start",
          runId: run.id,
          totalTasks: run.tasks.length,
          config: {
            model: fullConfig.model,
            maxRetries: fullConfig.maxRetries,
            useSkills: fullConfig.useSkills,
            useMemory: fullConfig.useMemory,
            useReflection: fullConfig.useReflexion,
          },
          timestamp: new Date().toISOString(),
        });

        // Execute tasks
        let taskIndex = 0;
        for (const task of run.tasks) {
          // Apply filters
          if (fullConfig.difficultyFilter !== undefined && task.difficulty !== fullConfig.difficultyFilter) {
            continue;
          }
          if (fullConfig.categoryFilter && task.category !== fullConfig.categoryFilter) {
            continue;
          }

          // Emit task start HUD message
          fullConfig.onHudMessage?.({
            type: "trainer_task_start",
            runId: run.id,
            taskId: task.id,
            taskPrompt: task.prompt.slice(0, 200),
            taskIndex,
            totalTasks: run.tasks.length,
          });

          const result = yield* gym.executeWithRetry(task, fullConfig).pipe(mapGymError);
          run.results.push(result);

          // Emit task complete HUD message
          fullConfig.onHudMessage?.({
            type: "trainer_task_complete",
            runId: run.id,
            taskId: task.id,
            outcome: result.outcome === "partial" ? "failure" : result.outcome,
            durationMs: result.durationMs,
            turns: 1, // Gym doesn't track turns currently
            tokens: result.tokens.total,
            retriesUsed: result.attemptNumber - 1,
          });

          // Update stats after each task
          run.stats = calculateStats(run.results);
          taskIndex++;
        }

        run.status = "completed";
        run.completedAt = new Date().toISOString();
        run.stats = calculateStats(run.results);

        store.runs.set(run.id, run);

        // Emit run complete HUD message
        fullConfig.onHudMessage?.({
          type: "trainer_run_complete",
          runId: run.id,
          stats: {
            totalTasks: run.stats.totalTasks,
            successRate: run.stats.successRate,
            averageDurationMs: run.stats.averageDurationMs,
            totalTokens: run.stats.totalTokens,
          },
          durationMs: Date.now() - startTime,
        });

        // Trigger archivist after run
        yield* archivist.runQuickArchive().pipe(mapArchivistError);

        return run;
      });

    const runBenchmark = (
      subset: TBSubset,
      config?: Partial<TrainingConfig>,
    ): Effect.Effect<BenchmarkResult, TrainerError> =>
      Effect.gen(function* () {
        const tasks = yield* loadBenchmarkTasks(subset);
        const run = yield* startRun(tasks, config);

        const result: BenchmarkResult = {
          id: `bench-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`,
          suiteId: subset,
          runId: run.id,
          stats: run.stats,
          model: run.config.model,
          timestamp: new Date().toISOString(),
        };

        store.benchmarkResults.push(result);
        return result;
      });

    const runTasks = (
      tasks: TrainingTask[],
      config?: Partial<TrainingConfig>,
    ): Effect.Effect<TrainingRun, TrainerError> => startRun(tasks, config);

    const getRunStatus = (runId: string): Effect.Effect<TrainingRun | null, TrainerError> =>
      Effect.succeed(store.runs.get(runId) ?? null);

    const cancelRun = (runId: string): Effect.Effect<void, TrainerError> =>
      Effect.gen(function* () {
        const run = store.runs.get(runId);
        if (run && run.status === "running") {
          run.status = "cancelled";
          run.completedAt = new Date().toISOString();
          store.runs.set(runId, run);
        }
      });

    const getRunStats = (runId: string): Effect.Effect<TrainingStats | null, TrainerError> =>
      Effect.gen(function* () {
        const run = store.runs.get(runId);
        return run?.stats ?? null;
      });

    const getCompletedRuns = (): Effect.Effect<TrainingRun[], TrainerError> =>
      Effect.succeed(
        Array.from(store.runs.values()).filter(
          (r) => r.status === "completed" || r.status === "cancelled",
        ),
      );

    const getBenchmarkHistory = (
      subset?: TBSubset,
    ): Effect.Effect<BenchmarkResult[], TrainerError> =>
      Effect.succeed(
        subset
          ? store.benchmarkResults.filter((r) => r.suiteId === subset)
          : store.benchmarkResults,
      );

    return {
      startRun,
      runBenchmark,
      runTasks,
      getRunStatus,
      cancelRun,
      createTask: createTaskOp,
      loadBenchmarkTasks,
      getRunStats,
      getCompletedRuns,
      getBenchmarkHistory,
    };
  });

// --- Helper Functions ---

/**
 * Generate a representative task prompt for a category.
 */
const generateTaskPrompt = (category: string, index: number): string => {
  const prompts: Record<string, string[]> = {
    bash: [
      "List all files in the current directory and sort by size",
      "Find all TypeScript files containing the word 'error'",
      "Count the number of lines in all .ts files",
      "Create a backup of all .json files with .bak extension",
      "Display the last 20 lines of the most recently modified log file",
    ],
    git: [
      "Show the commit history for the last week",
      "Create a new branch called 'feature-test' and switch to it",
      "Stage all modified TypeScript files",
      "Show the diff between the current branch and main",
      "List all branches that have been merged into main",
    ],
    "file-ops": [
      "Create a new directory structure for a TypeScript project",
      "Move all test files to a 'tests' subdirectory",
      "Rename all .js files to .ts files",
      "Find and delete all node_modules directories",
      "Create a symbolic link to the config file",
    ],
    debugging: [
      "Find the source of a type error in the imports",
      "Identify why the async function is timing out",
      "Debug the failing test assertion",
      "Trace the undefined variable through the call stack",
      "Find the memory leak in the event handlers",
    ],
    testing: [
      "Write a unit test for the utility function",
      "Add test coverage for the error handling path",
      "Create a mock for the API client",
      "Set up test fixtures for the database tests",
      "Write an integration test for the auth flow",
    ],
  };

  const categoryPrompts = prompts[category] ?? prompts.bash;
  return categoryPrompts[index % categoryPrompts.length];
};

// --- Layer ---

export const TrainerServiceLayer: Layer.Layer<TrainerService, never, Gym | ArchivistService> =
  Layer.effect(TrainerService, makeTrainerService());

/**
 * Create a complete TrainerService layer with all dependencies.
 */
export const makeTrainerServiceLive = (
  projectRoot: string = process.cwd(),
): Layer.Layer<
  TrainerService,
  SkillStoreError | MemoryStoreError | TrajectoryStoreError | PatternExtractorError,
  never
> => {
  const gymLayer = makeGymLive(projectRoot);
  const archivistLayer = makeArchivistServiceLive(projectRoot);

  return Layer.provide(TrainerServiceLayer, Layer.mergeAll(gymLayer, archivistLayer));
};

export const TrainerServiceLive: Layer.Layer<
  TrainerService,
  SkillStoreError | MemoryStoreError | TrajectoryStoreError | PatternExtractorError,
  never
> = makeTrainerServiceLive();
