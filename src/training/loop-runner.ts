/**
 * Training Loop Runner
 *
 * Orchestrates progressive Terminal-Bench runs with:
 * - Progressive expansion: TB_10 → TB_30 → TB_89
 * - Overnight iteration support with time limits
 * - Checkpoint/resume capability
 * - Episode tracking and baseline comparison
 *
 * @example
 * ```ts
 * const runner = createLoopRunner({
 *   projectRoot: process.cwd(),
 *   model: "claude-code",
 *   maxDurationMs: 8 * 60 * 60 * 1000, // 8 hours overnight
 * });
 *
 * await Effect.runPromise(runner.start());
 * ```
 */

import { Effect, Duration } from "effect";
import * as S from "effect/Schema";
import { FileSystem } from "@effect/platform";
import { BunContext } from "@effect/platform-bun";
import type { TBSubset } from "../trainer/schema.js";
import { TB_SUBSETS } from "../trainer/schema.js";
import { EpisodeStore, type Episode, type EpisodeSummary } from "../bench/episode-store.js";

// --- Configuration ---

/**
 * Loop runner configuration schema.
 */
export const LoopRunnerConfig = S.Struct({
  /** Project root directory */
  projectRoot: S.String,
  /** Model to use (claude-code, ollama, fm) */
  model: S.String,
  /** Starting subset (default: TB_10) */
  startSubset: S.optional(S.Literal("TB_10", "TB_30", "TB_89")),
  /** Maximum total duration in ms (0 = unlimited) */
  maxDurationMs: S.optional(S.Number),
  /** Maximum iterations (0 = unlimited) */
  maxIterations: S.optional(S.Number),
  /** Delay between iterations in ms */
  iterationDelayMs: S.optional(S.Number),
  /** Success rate threshold to progress (0-1) */
  progressionThreshold: S.optional(S.Number),
  /** Minimum iterations before progression check */
  minIterationsBeforeProgression: S.optional(S.Number),
  /** State file path for resume */
  stateFilePath: S.optional(S.String),
  /** Whether to auto-resume from state file */
  autoResume: S.optional(S.Boolean),
  /** Callback for state changes */
  onStateChange: S.optional(S.Unknown),
});

export type LoopRunnerConfig = S.Schema.Type<typeof LoopRunnerConfig>;

/**
 * Default configuration values.
 */
export const DEFAULT_CONFIG: Required<Omit<LoopRunnerConfig, "onStateChange">> & {
  onStateChange?: (state: LoopRunnerState) => void;
} = {
  projectRoot: process.cwd(),
  model: "claude-code",
  startSubset: "TB_10",
  maxDurationMs: 0, // Unlimited
  maxIterations: 0, // Unlimited
  iterationDelayMs: 5000, // 5 seconds between iterations
  progressionThreshold: 0.8, // 80% success rate to progress
  minIterationsBeforeProgression: 3, // At least 3 iterations before considering progression
  stateFilePath: ".openagents/training/loop-state.json",
  autoResume: true,
};

// --- State ---

/**
 * Loop runner state schema.
 */
export const LoopRunnerState = S.Struct({
  /** Unique run ID */
  runId: S.String,
  /** Current status */
  status: S.Literal("idle", "running", "paused", "completed", "failed"),
  /** Current subset being run */
  currentSubset: S.Literal("TB_10", "TB_30", "TB_89"),
  /** Current iteration within subset */
  iteration: S.Number,
  /** Total iterations across all subsets */
  totalIterations: S.Number,
  /** Iterations per subset */
  subsetIterations: S.Struct({
    TB_10: S.Number,
    TB_30: S.Number,
    TB_89: S.Number,
  }),
  /** Success rates per subset */
  subsetSuccessRates: S.Struct({
    TB_10: S.Number,
    TB_30: S.Number,
    TB_89: S.Number,
  }),
  /** Best success rate achieved per subset */
  bestSuccessRates: S.Struct({
    TB_10: S.Number,
    TB_30: S.Number,
    TB_89: S.Number,
  }),
  /** Total tasks completed */
  totalTasksCompleted: S.Number,
  /** Total successful tasks */
  totalSuccessful: S.Number,
  /** Overall success rate */
  overallSuccessRate: S.Number,
  /** Start time (ISO string) */
  startedAt: S.String,
  /** Last update time (ISO string) */
  lastUpdatedAt: S.String,
  /** Total duration so far in ms */
  totalDurationMs: S.Number,
  /** Last episode ID */
  lastEpisodeId: S.optional(S.String),
  /** Error message if failed */
  error: S.optional(S.String),
});

export type LoopRunnerState = S.Schema.Type<typeof LoopRunnerState>;

/**
 * Create initial state.
 */
const createInitialState = (runId: string, startSubset: TBSubset): LoopRunnerState => ({
  runId,
  status: "idle",
  currentSubset: startSubset,
  iteration: 0,
  totalIterations: 0,
  subsetIterations: { TB_10: 0, TB_30: 0, TB_89: 0 },
  subsetSuccessRates: { TB_10: 0, TB_30: 0, TB_89: 0 },
  bestSuccessRates: { TB_10: 0, TB_30: 0, TB_89: 0 },
  totalTasksCompleted: 0,
  totalSuccessful: 0,
  overallSuccessRate: 0,
  startedAt: new Date().toISOString(),
  lastUpdatedAt: new Date().toISOString(),
  totalDurationMs: 0,
});

// --- Iteration Result ---

/**
 * Result from a single iteration.
 */
export interface IterationResult {
  episodeId: string;
  subset: TBSubset;
  iteration: number;
  summary: EpisodeSummary;
  durationMs: number;
  shouldProgress: boolean;
}

// --- Loop Runner ---

/**
 * Loop runner interface.
 */
export interface ILoopRunner {
  /** Start the training loop */
  readonly start: () => Effect.Effect<LoopRunnerState, LoopRunnerError>;
  /** Run a single iteration */
  readonly runIteration: () => Effect.Effect<IterationResult, LoopRunnerError>;
  /** Get current state */
  readonly getState: () => Effect.Effect<LoopRunnerState, never>;
  /** Pause the loop */
  readonly pause: () => Effect.Effect<void, never>;
  /** Resume the loop */
  readonly resume: () => Effect.Effect<void, LoopRunnerError>;
  /** Stop the loop */
  readonly stop: () => Effect.Effect<LoopRunnerState, never>;
  /** Save state to file */
  readonly saveState: () => Effect.Effect<void, LoopRunnerError>;
  /** Load state from file */
  readonly loadState: () => Effect.Effect<LoopRunnerState | null, LoopRunnerError>;
  /** Check if should progress to next tier */
  readonly shouldProgress: () => Effect.Effect<boolean, never>;
  /** Progress to next tier */
  readonly progressTier: () => Effect.Effect<TBSubset, never>;
}

// --- Error ---

export class LoopRunnerError extends Error {
  readonly _tag = "LoopRunnerError";
  constructor(
    readonly reason:
      | "config_invalid"
      | "state_load_failed"
      | "state_save_failed"
      | "iteration_failed"
      | "time_limit_exceeded"
      | "iteration_limit_exceeded",
    override readonly message: string,
    override readonly cause?: Error,
  ) {
    super(message);
    this.name = "LoopRunnerError";
  }
}

// --- Implementation ---

/**
 * Create a loop runner instance.
 */
export const createLoopRunner = (
  config: Partial<LoopRunnerConfig> = {},
): ILoopRunner => {
  const fullConfig = { ...DEFAULT_CONFIG, ...config };
  const runId = `loop-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;

  // State reference (mutable)
  let state = createInitialState(runId, fullConfig.startSubset as TBSubset);
  let episodeStore: EpisodeStore | null = null;

  const getEpisodeStore = (): EpisodeStore => {
    if (!episodeStore) {
      episodeStore = new EpisodeStore(fullConfig.projectRoot);
    }
    return episodeStore;
  };

  const updateState = (updates: Partial<LoopRunnerState>): void => {
    state = {
      ...state,
      ...updates,
      lastUpdatedAt: new Date().toISOString(),
    };
    if (fullConfig.onStateChange) {
      (fullConfig.onStateChange as (s: LoopRunnerState) => void)(state);
    }
  };

  const getState = (): Effect.Effect<LoopRunnerState, never> => Effect.succeed(state);

  const pause = (): Effect.Effect<void, never> =>
    Effect.sync(() => {
      updateState({ status: "paused" });
    });

  const stop = (): Effect.Effect<LoopRunnerState, never> =>
    Effect.sync(() => {
      updateState({ status: "completed" });
      return state;
    });

  const shouldProgress = (): Effect.Effect<boolean, never> =>
    Effect.sync(() => {
      const { currentSubset, subsetIterations, subsetSuccessRates } = state;

      // Can't progress beyond TB_89
      if (currentSubset === "TB_89") return false;

      // Need minimum iterations
      if (subsetIterations[currentSubset] < (fullConfig.minIterationsBeforeProgression ?? 0)) {
        return false;
      }

      // Check success rate threshold
      return subsetSuccessRates[currentSubset] >= (fullConfig.progressionThreshold ?? 0);
    });

  const progressTier = (): Effect.Effect<TBSubset, never> =>
    Effect.sync(() => {
      const progression: Record<TBSubset, TBSubset> = {
        TB_10: "TB_30",
        TB_30: "TB_89",
        TB_89: "TB_89",
      };

      const nextSubset = progression[state.currentSubset];
      updateState({
        currentSubset: nextSubset,
        iteration: 0, // Reset iteration counter for new subset
      });
      return nextSubset;
    });

  const saveState = (): Effect.Effect<void, LoopRunnerError, never> =>
    Effect.gen(function* () {
      const fs = yield* FileSystem.FileSystem;
      const stateJson = JSON.stringify(state, null, 2);
      const statePath = `${fullConfig.projectRoot}/${fullConfig.stateFilePath}`;

      // Ensure directory exists
      const dir = statePath.substring(0, statePath.lastIndexOf("/"));
      yield* fs.makeDirectory(dir, { recursive: true }).pipe(Effect.catchAll(() => Effect.void));

      yield* fs.writeFileString(statePath, stateJson).pipe(
        Effect.mapError(
          (e) => new LoopRunnerError("state_save_failed", `Failed to save state: ${e.message}`),
        ),
      );
    }).pipe(
      Effect.mapError((e) =>
        e instanceof LoopRunnerError
          ? e
          : new LoopRunnerError("state_save_failed", `Unexpected error: ${String(e)}`),
      ),
      Effect.provide(BunContext.layer),
    );

  const loadState = (): Effect.Effect<LoopRunnerState | null, LoopRunnerError, never> =>
    Effect.gen(function* () {
      const fs = yield* FileSystem.FileSystem;
      const statePath = `${fullConfig.projectRoot}/${fullConfig.stateFilePath}`;

      const exists = yield* fs.exists(statePath);
      if (!exists) return null;

      const content = yield* fs.readFileString(statePath).pipe(
        Effect.mapError(
          (e) => new LoopRunnerError("state_load_failed", `Failed to load state: ${e.message}`),
        ),
      );

      try {
        const loaded = JSON.parse(content) as LoopRunnerState;
        return loaded;
      } catch (e) {
        return yield* Effect.fail(
          new LoopRunnerError("state_load_failed", `Invalid state JSON: ${e}`),
        );
      }
    }).pipe(
      Effect.mapError((e) =>
        e instanceof LoopRunnerError
          ? e
          : new LoopRunnerError("state_load_failed", `Unexpected error: ${String(e)}`),
      ),
      Effect.provide(BunContext.layer),
    );

  const resume = (): Effect.Effect<void, LoopRunnerError> =>
    Effect.gen(function* () {
      if (fullConfig.autoResume) {
        const loadedState = yield* loadState();
        if (loadedState && loadedState.status !== "completed") {
          state = loadedState;
          updateState({ status: "running" });
        }
      } else {
        updateState({ status: "running" });
      }
    });

  const runIteration = (): Effect.Effect<IterationResult, LoopRunnerError> =>
    Effect.gen(function* () {
      const iterationStart = Date.now();
      const store = getEpisodeStore();
      const subset = state.currentSubset;
      const subsetInfo = TB_SUBSETS[subset];

      // Create episode ID
      const episodeId = `${state.runId}-${subset}-${state.iteration + 1}`;

      // TODO: Actually run the benchmark here
      // For now, create a mock result - this will be replaced with actual
      // model-adapter integration in a follow-up task
      const mockSummary: EpisodeSummary = {
        total: subsetInfo.count,
        passed: Math.floor(subsetInfo.count * 0.7), // Mock 70% pass rate
        failed: Math.floor(subsetInfo.count * 0.2),
        timeout: Math.floor(subsetInfo.count * 0.05),
        error: Math.floor(subsetInfo.count * 0.05),
        passRate: 0.7,
        avgTurns: 15,
        avgTokens: 3500,
        totalDurationMs: 60000 * subsetInfo.count,
      };

      // Record episode
      const episode: Episode = {
        id: episodeId,
        runId: state.runId,
        iteration: state.iteration + 1,
        model: fullConfig.model,
        suiteVersion: "2.0",
        startedAt: new Date(iterationStart).toISOString(),
        finishedAt: new Date().toISOString(),
        status: mockSummary.passRate >= 0.8 ? "success" : "partial",
        summary: mockSummary,
        resultsPath: `./results/${state.runId}/${subset}/${state.iteration + 1}/results.json`,
      };

      yield* Effect.promise(() => store.record(episode));

      // Update state
      const newIteration = state.iteration + 1;
      const newSubsetIterations = {
        ...state.subsetIterations,
        [subset]: state.subsetIterations[subset] + 1,
      };
      const newSubsetSuccessRates = {
        ...state.subsetSuccessRates,
        [subset]: mockSummary.passRate,
      };
      const newBestSuccessRates = {
        ...state.bestSuccessRates,
        [subset]: Math.max(state.bestSuccessRates[subset], mockSummary.passRate),
      };

      const durationMs = Date.now() - iterationStart;

      updateState({
        iteration: newIteration,
        totalIterations: state.totalIterations + 1,
        subsetIterations: newSubsetIterations,
        subsetSuccessRates: newSubsetSuccessRates,
        bestSuccessRates: newBestSuccessRates,
        totalTasksCompleted: state.totalTasksCompleted + mockSummary.total,
        totalSuccessful: state.totalSuccessful + mockSummary.passed,
        overallSuccessRate:
          (state.totalSuccessful + mockSummary.passed) /
          (state.totalTasksCompleted + mockSummary.total),
        totalDurationMs: state.totalDurationMs + durationMs,
        lastEpisodeId: episodeId,
      });

      // Check progression
      const shouldProg = yield* shouldProgress();

      return {
        episodeId,
        subset,
        iteration: newIteration,
        summary: mockSummary,
        durationMs,
        shouldProgress: shouldProg,
      };
    });

  const checkLimits = (): Effect.Effect<void, LoopRunnerError> =>
    Effect.gen(function* () {
      // Check time limit
      if ((fullConfig.maxDurationMs ?? 0) > 0 && state.totalDurationMs >= (fullConfig.maxDurationMs ?? 0)) {
        return yield* Effect.fail(
          new LoopRunnerError(
            "time_limit_exceeded",
            `Time limit of ${fullConfig.maxDurationMs}ms exceeded`,
          ),
        );
      }

      // Check iteration limit
      if ((fullConfig.maxIterations ?? 0) > 0 && state.totalIterations >= (fullConfig.maxIterations ?? 0)) {
        return yield* Effect.fail(
          new LoopRunnerError(
            "iteration_limit_exceeded",
            `Iteration limit of ${fullConfig.maxIterations} exceeded`,
          ),
        );
      }
    });

  const start = (): Effect.Effect<LoopRunnerState, LoopRunnerError> =>
    Effect.gen(function* () {
      // Try to resume if configured
      if (fullConfig.autoResume) {
        const loadedState = yield* loadState();
        if (loadedState && loadedState.status === "running") {
          state = loadedState;
          console.log(`[LoopRunner] Resuming from iteration ${state.totalIterations}`);
        }
      }

      updateState({ status: "running" });
      console.log(
        `[LoopRunner] Starting run ${state.runId} with subset ${state.currentSubset}`,
      );

      // Main loop
      while (state.status === "running") {
        // Check limits
        yield* checkLimits().pipe(
          Effect.catchAll((e) => {
            if (
              e.reason === "time_limit_exceeded" ||
              e.reason === "iteration_limit_exceeded"
            ) {
              console.log(`[LoopRunner] ${e.message}`);
              updateState({ status: "completed" });
              return Effect.void;
            }
            return Effect.fail(e);
          }),
        );

        if (state.status !== "running") break;

        // Run iteration
        const result = yield* runIteration().pipe(
          Effect.catchAll((e) => {
            console.error(`[LoopRunner] Iteration failed: ${e.message}`);
            updateState({ status: "failed", error: e.message });
            return Effect.fail(e);
          }),
        );

        console.log(
          `[LoopRunner] ${result.subset} iteration ${result.iteration}: ` +
            `${(result.summary.passRate * 100).toFixed(1)}% pass rate`,
        );

        // Save state checkpoint
        yield* saveState();

        // Check progression
        if (result.shouldProgress) {
          const nextSubset = yield* progressTier();
          console.log(`[LoopRunner] Progressing to ${nextSubset}`);
        }

        // Delay between iterations
        if ((fullConfig.iterationDelayMs ?? 0) > 0 && state.status === "running") {
          yield* Effect.sleep(Duration.millis(fullConfig.iterationDelayMs ?? 0));
        }
      }

      // Final state save
      yield* saveState();

      console.log(
        `[LoopRunner] Run complete. Total iterations: ${state.totalIterations}, ` +
          `Overall success rate: ${(state.overallSuccessRate * 100).toFixed(1)}%`,
      );

      return state;
    });

  return {
    start,
    runIteration,
    getState,
    pause,
    resume,
    stop,
    saveState,
    loadState,
    shouldProgress,
    progressTier,
  };
};

// --- Convenience Functions ---

/**
 * Run a training loop with default configuration.
 */
export const runTrainingLoop = (
  config: Partial<LoopRunnerConfig> = {},
): Effect.Effect<LoopRunnerState, LoopRunnerError> => {
  const runner = createLoopRunner(config);
  return runner.start();
};

/**
 * Run overnight training with time limit.
 */
export const runOvernightTraining = (
  hoursLimit: number,
  config: Partial<LoopRunnerConfig> = {},
): Effect.Effect<LoopRunnerState, LoopRunnerError> => {
  const runner = createLoopRunner({
    ...config,
    maxDurationMs: hoursLimit * 60 * 60 * 1000,
    autoResume: true,
  });
  return runner.start();
};

/**
 * Run progressive benchmark (TB_10 → TB_30 → TB_89).
 */
export const runProgressiveBenchmark = (
  maxIterationsPerSubset: number = 5,
  config: Partial<LoopRunnerConfig> = {},
): Effect.Effect<LoopRunnerState, LoopRunnerError> => {
  const runner = createLoopRunner({
    ...config,
    startSubset: "TB_10",
    minIterationsBeforeProgression: Math.min(3, maxIterationsPerSubset),
    progressionThreshold: 0.8,
  });
  return runner.start();
};
