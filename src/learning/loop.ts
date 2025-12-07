/**
 * Training Loop
 *
 * The main learning loop for MechaCoder.
 * Orchestrates all learning subsystems for continuous improvement.
 *
 * The loop:
 * 1. Picks tasks from the queue or Terminal-Bench
 * 2. Executes tasks with skill/memory augmentation
 * 3. Records trajectories for pattern extraction
 * 4. Applies Reflexion on failures
 * 5. Periodically archives and extracts skills
 */

import { Effect, Context, Layer } from "effect";
import {
  TrainerService,
  makeTrainerServiceLive,
  type TrainerError,
} from "../trainer/service.js";
import {
  ArchivistService,
  makeArchivistServiceLive,
  type ArchivistError,
} from "../archivist/service.js";
import {
  SkillService,
  makeSkillServiceLive,
  type SkillServiceError,
} from "../skills/service.js";
import type { SkillStoreError } from "../skills/store.js";
import {
  MemoryService,
  makeMemoryServiceLive,
  type MemoryServiceError,
} from "../memory/service.js";
import type { MemoryStoreError } from "../memory/store.js";
import type { TrajectoryStoreError } from "../archivist/store.js";
import type { PatternExtractorError } from "../archivist/extractor.js";
import type { TrainingRun, TrainingConfig, TBSubset } from "../trainer/schema.js";
import type { ArchiveResult } from "../archivist/schema.js";

// --- Error Types ---

export class TrainingLoopError extends Error {
  readonly _tag = "TrainingLoopError";
  constructor(
    readonly reason: string,
    override readonly message: string,
    override readonly cause?: Error,
  ) {
    super(message);
    this.name = "TrainingLoopError";
  }

  static from(
    e: TrainerError | ArchivistError | SkillServiceError | MemoryServiceError,
  ): TrainingLoopError {
    return new TrainingLoopError(e._tag, e.message, e);
  }
}

// --- Loop Configuration ---

/**
 * Configuration for the training loop.
 */
export interface LoopConfig {
  /** Maximum iterations (0 for infinite) */
  maxIterations: number;
  /** Delay between iterations in ms */
  iterationDelayMs: number;
  /** Archive every N iterations */
  archiveEveryN: number;
  /** Whether to run benchmark progression */
  progressiveBenchmark: boolean;
  /** Starting benchmark subset */
  startSubset: TBSubset;
  /** Training config overrides */
  trainingConfig?: Partial<TrainingConfig>;
  /** Project root */
  projectRoot: string;
  /** Callback for iteration updates */
  onIteration?: (state: LoopState) => void;
  /** Callback for archive completion */
  onArchive?: (result: ArchiveResult) => void;
}

export const DEFAULT_LOOP_CONFIG: LoopConfig = {
  maxIterations: 0,
  iterationDelayMs: 1000,
  archiveEveryN: 5,
  progressiveBenchmark: true,
  startSubset: "TB_10",
  projectRoot: process.cwd(),
};

// --- Loop State ---

/**
 * Current state of the training loop.
 */
export interface LoopState {
  /** Current iteration */
  iteration: number;
  /** Current benchmark subset */
  currentSubset: TBSubset;
  /** Total tasks completed */
  totalTasksCompleted: number;
  /** Total successful tasks */
  totalSuccessful: number;
  /** Overall success rate */
  overallSuccessRate: number;
  /** Skills learned */
  skillsLearned: number;
  /** Patterns extracted */
  patternsExtracted: number;
  /** Last run stats */
  lastRun?: TrainingRun;
  /** Last archive result */
  lastArchive?: ArchiveResult;
  /** Loop status */
  status: "running" | "paused" | "stopped" | "completed";
  /** Started at */
  startedAt: string;
  /** Total duration */
  totalDurationMs: number;
}

// --- Service Interface ---

export interface ITrainingLoop {
  /** Start the training loop */
  readonly start: (config?: Partial<LoopConfig>) => Effect.Effect<LoopState, TrainingLoopError, TrainerService | ArchivistService | SkillService | MemoryService>;

  /** Run a single iteration */
  readonly runIteration: () => Effect.Effect<LoopState, TrainingLoopError, TrainerService | ArchivistService | SkillService | MemoryService>;

  /** Run archive cycle */
  readonly runArchive: () => Effect.Effect<ArchiveResult, TrainingLoopError, TrainerService | ArchivistService | SkillService | MemoryService>;

  /** Get current state */
  readonly getState: () => Effect.Effect<LoopState, never, TrainerService | ArchivistService | SkillService | MemoryService>;

  /** Pause the loop */
  readonly pause: () => Effect.Effect<void, never, TrainerService | ArchivistService | SkillService | MemoryService>;

  /** Resume the loop */
  readonly resume: () => Effect.Effect<void, never, TrainerService | ArchivistService | SkillService | MemoryService>;

  /** Stop the loop */
  readonly stop: () => Effect.Effect<LoopState, never, TrainerService | ArchivistService | SkillService | MemoryService>;

  /** Check if should progress to next benchmark tier */
  readonly shouldProgress: () => Effect.Effect<boolean, never, TrainerService | ArchivistService | SkillService | MemoryService>;

  /** Progress to next benchmark tier */
  readonly progressTier: () => Effect.Effect<TBSubset, never, TrainerService | ArchivistService | SkillService | MemoryService>;
}

// --- Service Tag ---

export class TrainingLoop extends Context.Tag("TrainingLoop")<TrainingLoop, ITrainingLoop>() {}

// --- Implementation ---

const makeTrainingLoop = (
  loopConfig: LoopConfig,
): Effect.Effect<ITrainingLoop, never, TrainerService | ArchivistService | SkillService | MemoryService> =>
  Effect.gen(function* () {
    const trainer = yield* TrainerService;
    const archivist = yield* ArchivistService;
    const skills = yield* SkillService;

    const mapTrainerError = Effect.mapError(TrainingLoopError.from);
    const mapArchivistError = Effect.mapError(TrainingLoopError.from);
    const mapSkillError = Effect.mapError(TrainingLoopError.from);

    // Internal state
    let state: LoopState = {
      iteration: 0,
      currentSubset: loopConfig.startSubset,
      totalTasksCompleted: 0,
      totalSuccessful: 0,
      overallSuccessRate: 0,
      skillsLearned: 0,
      patternsExtracted: 0,
      status: "stopped",
      startedAt: new Date().toISOString(),
      totalDurationMs: 0,
    };

    const updateState = (updates: Partial<LoopState>): void => {
      state = { ...state, ...updates };
      if (loopConfig.onIteration) {
        loopConfig.onIteration(state);
      }
    };

    const getState = (): Effect.Effect<LoopState, never> => Effect.succeed(state);

    const pause = (): Effect.Effect<void, never> =>
      Effect.sync(() => {
        updateState({ status: "paused" });
      });

    const resume = (): Effect.Effect<void, never> =>
      Effect.sync(() => {
        updateState({ status: "running" });
      });

    const stop = (): Effect.Effect<LoopState, never> =>
      Effect.sync(() => {
        updateState({ status: "stopped" });
        return state;
      });

    const shouldProgress = (): Effect.Effect<boolean, never> =>
      Effect.gen(function* () {
        // Progress if success rate > 80% and at least 10 tasks completed
        const minTasks = 10;
        const minSuccessRate = 0.8;

        return (
          state.totalTasksCompleted >= minTasks &&
          state.overallSuccessRate >= minSuccessRate &&
          state.currentSubset !== "TB_89"
        );
      });

    const progressTier = (): Effect.Effect<TBSubset, never> =>
      Effect.gen(function* () {
        const progression: Record<TBSubset, TBSubset> = {
          TB_10: "TB_30",
          TB_30: "TB_89",
          TB_89: "TB_89",
        };

        const nextSubset = progression[state.currentSubset];
        updateState({ currentSubset: nextSubset });
        return nextSubset;
      });

    const runArchive = (): Effect.Effect<ArchiveResult, TrainingLoopError> =>
      Effect.gen(function* () {
        const result = yield* archivist.runArchive().pipe(mapArchivistError);

        updateState({
          lastArchive: result,
          skillsLearned: state.skillsLearned + result.skillsCreated,
          patternsExtracted: state.patternsExtracted + result.patternsExtracted,
        });

        if (loopConfig.onArchive) {
          loopConfig.onArchive(result);
        }

        return result;
      });

    const runIteration = (): Effect.Effect<LoopState, TrainingLoopError> =>
      Effect.gen(function* () {
        const iterationStart = Date.now();

        // Skip if paused or stopped
        if (state.status === "paused" || state.status === "stopped") {
          return state;
        }

        // Run benchmark
        const result = yield* trainer
          .runBenchmark(state.currentSubset, loopConfig.trainingConfig)
          .pipe(mapTrainerError);

        // Get the run
        const run = yield* trainer.getRunStatus(result.runId).pipe(mapTrainerError);

        // Update state
        const newTotalCompleted = state.totalTasksCompleted + result.stats.completedTasks;
        const newTotalSuccessful = state.totalSuccessful + result.stats.successfulTasks;
        const newSuccessRate = newTotalCompleted > 0 ? newTotalSuccessful / newTotalCompleted : 0;

        updateState({
          iteration: state.iteration + 1,
          totalTasksCompleted: newTotalCompleted,
          totalSuccessful: newTotalSuccessful,
          overallSuccessRate: newSuccessRate,
          ...(run ? { lastRun: run } : {}),
          totalDurationMs: state.totalDurationMs + (Date.now() - iterationStart),
        });

        // Check if we should archive
        if (state.iteration % loopConfig.archiveEveryN === 0) {
          yield* runArchive();
        }

        // Check if we should progress
        if (loopConfig.progressiveBenchmark) {
          const shouldProg = yield* shouldProgress();
          if (shouldProg) {
            yield* progressTier();
          }
        }

        return state;
      });

    const start = (
      configOverrides?: Partial<LoopConfig>,
    ): Effect.Effect<LoopState, TrainingLoopError, TrainerService | ArchivistService | SkillService | MemoryService> =>
      Effect.gen(function* () {
        const config = { ...loopConfig, ...configOverrides };

        updateState({
          status: "running",
          startedAt: new Date().toISOString(),
          iteration: 0,
          totalTasksCompleted: 0,
          totalSuccessful: 0,
          overallSuccessRate: 0,
        });

        // Bootstrap skills if needed
        const allSkills = yield* skills.getAllSkills().pipe(mapSkillError);
        if (allSkills.length === 0) {
          // Import and register bootstrap skills
          const { bootstrapSkills } = yield* Effect.promise(() => import("../skills/library/index.js"));
          for (const skill of bootstrapSkills) {
            yield* skills.registerSkill(skill).pipe(mapSkillError);
          }
        }

        // Run iterations
        let iterations = 0;
        while (state.status === "running") {
          if (config.maxIterations > 0 && iterations >= config.maxIterations) {
            updateState({ status: "completed" });
            break;
          }

          yield* runIteration();
          iterations++;

          // Delay between iterations
          if (config.iterationDelayMs > 0 && state.status === "running") {
            yield* Effect.sleep(config.iterationDelayMs);
          }
        }

        return state;
      });

    return {
      start,
      runIteration,
      runArchive,
      getState,
      pause,
      resume,
      stop,
      shouldProgress,
      progressTier,
    };
  });

// --- Layer ---

export const TrainingLoopLayer: Layer.Layer<
  TrainingLoop,
  never,
  TrainerService | ArchivistService | SkillService | MemoryService
> = Layer.effect(TrainingLoop, makeTrainingLoop(DEFAULT_LOOP_CONFIG));

/**
 * Create a TrainingLoop layer with custom config.
 */
export const makeTrainingLoopLayer = (
  config: Partial<LoopConfig> = {},
): Layer.Layer<TrainingLoop, never, TrainerService | ArchivistService | SkillService | MemoryService> =>
  Layer.effect(TrainingLoop, makeTrainingLoop({ ...DEFAULT_LOOP_CONFIG, ...config }));

/**
 * Create a complete TrainingLoop layer with all dependencies.
 */
export const makeTrainingLoopLive = (
  projectRoot: string = process.cwd(),
  config: Partial<LoopConfig> = {},
): Layer.Layer<TrainingLoop, SkillStoreError | MemoryStoreError | TrainerError | ArchivistError | TrajectoryStoreError | PatternExtractorError, TrainerService | ArchivistService | SkillService | MemoryService> => {
  const trainerLayer = makeTrainerServiceLive(projectRoot);
  const archivistLayer = makeArchivistServiceLive(projectRoot);
  const skillLayer = makeSkillServiceLive(projectRoot);
  const memoryLayer = makeMemoryServiceLive(projectRoot);

  return Layer.mergeAll(
    makeTrainingLoopLayer({ ...config, projectRoot }),
    trainerLayer,
    archivistLayer,
    skillLayer,
    memoryLayer
  );
};

export const TrainingLoopLive: Layer.Layer<TrainingLoop, SkillStoreError | MemoryStoreError | TrainerError | ArchivistError | TrajectoryStoreError | PatternExtractorError, TrainerService | ArchivistService | SkillService | MemoryService> = makeTrainingLoopLive();
