/**
 * Learning Orchestrator
 *
 * High-level API for MechaCoder's learning system.
 * Provides unified access to all learning subsystems.
 */

import { Effect, Context, Layer } from "effect";
import { SkillService, makeSkillServiceLive, type SkillServiceError } from "../skills/service.js";
import { MemoryService, makeMemoryServiceLive, type MemoryServiceError } from "../memory/service.js";
import {
  ReflexionService,
  makeReflexionServiceLive,
  type ReflexionServiceError,
} from "../reflexion/service.js";
import {
  ArchivistService,
  makeArchivistServiceLive,
  type ArchivistError,
} from "../archivist/service.js";
import {
  TrainerService,
  makeTrainerServiceLive,
  type TrainerError,
} from "../trainer/service.js";
import {
  TrainingLoop,
  makeTrainingLoopLive,
  type TrainingLoopError,
  type LoopConfig,
  type LoopState,
} from "./loop.js";
import type { Skill } from "../skills/schema.js";
import type { Memory } from "../memory/schema.js";
import type { Reflection } from "../reflexion/schema.js";
import type { TrainingTask, TrainingRun, TBSubset } from "../trainer/schema.js";
import type { ArchiveResult } from "../archivist/schema.js";

// --- Error Types ---

export class OrchestratorError extends Error {
  readonly _tag = "OrchestratorError";
  constructor(
    readonly reason: string,
    message: string,
    readonly cause?: Error,
  ) {
    super(message);
    this.name = "OrchestratorError";
  }

  static from(
    e:
      | SkillServiceError
      | MemoryServiceError
      | ReflexionServiceError
      | ArchivistError
      | TrainerError
      | TrainingLoopError,
  ): OrchestratorError {
    return new OrchestratorError(e._tag, e.message, e);
  }
}

// --- System Stats ---

/**
 * Comprehensive statistics for the learning system.
 */
export interface LearningStats {
  /** Skill statistics */
  skills: {
    total: number;
    bootstrapped: number;
    learned: number;
    byCategory: Record<string, number>;
  };
  /** Memory statistics */
  memories: {
    total: number;
    episodic: number;
    semantic: number;
    procedural: number;
  };
  /** Reflexion statistics */
  reflexion: {
    totalFailures: number;
    totalReflections: number;
    successfulReflections: number;
    skillsLearned: number;
  };
  /** Archivist statistics */
  archivist: {
    totalTrajectories: number;
    unarchivedTrajectories: number;
    successfulTrajectories: number;
    patternsExtracted: number;
    skillsCreated: number;
  };
  /** Training statistics */
  training: {
    totalRuns: number;
    totalTasksCompleted: number;
    overallSuccessRate: number;
    currentTier: TBSubset;
  };
  /** Loop status */
  loop: LoopState | null;
}

// --- Orchestrator Interface ---

export interface ILearningOrchestrator {
  // --- System Lifecycle ---

  /** Initialize the learning system */
  readonly initialize: () => Effect.Effect<void, OrchestratorError>;

  /** Start the training loop */
  readonly startTraining: (
    config?: Partial<LoopConfig>,
  ) => Effect.Effect<LoopState, OrchestratorError>;

  /** Stop the training loop */
  readonly stopTraining: () => Effect.Effect<LoopState, OrchestratorError>;

  /** Pause the training loop */
  readonly pauseTraining: () => Effect.Effect<void, OrchestratorError>;

  /** Resume the training loop */
  readonly resumeTraining: () => Effect.Effect<void, OrchestratorError>;

  // --- Task Execution ---

  /** Execute a single task with learning */
  readonly executeTask: (
    task: TrainingTask,
  ) => Effect.Effect<TrainingRun, OrchestratorError>;

  /** Run a benchmark suite */
  readonly runBenchmark: (
    subset: TBSubset,
  ) => Effect.Effect<TrainingRun, OrchestratorError>;

  // --- Skill Operations ---

  /** Search for relevant skills */
  readonly findSkills: (
    query: string,
    maxResults?: number,
  ) => Effect.Effect<Skill[], OrchestratorError>;

  /** Get all skills */
  readonly getAllSkills: () => Effect.Effect<Skill[], OrchestratorError>;

  /** Bootstrap default skills */
  readonly bootstrapSkills: () => Effect.Effect<number, OrchestratorError>;

  // --- Memory Operations ---

  /** Search for relevant memories */
  readonly findMemories: (
    query: string,
    maxResults?: number,
  ) => Effect.Effect<Memory[], OrchestratorError>;

  /** Record an experience */
  readonly recordExperience: (
    content: string,
    type: "episodic" | "semantic" | "procedural",
    tags?: string[],
  ) => Effect.Effect<Memory, OrchestratorError>;

  // --- Reflexion Operations ---

  /** Record a failure and generate reflection */
  readonly recordFailure: (
    taskDescription: string,
    errorMessage: string,
  ) => Effect.Effect<Reflection, OrchestratorError>;

  /** Get reflections for a task */
  readonly getReflections: (
    taskDescription: string,
  ) => Effect.Effect<Reflection[], OrchestratorError>;

  // --- Archive Operations ---

  /** Run archive cycle */
  readonly runArchive: () => Effect.Effect<ArchiveResult, OrchestratorError>;

  /** Get archive statistics */
  readonly getArchiveStats: () => Effect.Effect<
    {
      totalTrajectories: number;
      unarchivedTrajectories: number;
      successfulTrajectories: number;
      patternsExtracted: number;
      skillsCreated: number;
    },
    OrchestratorError
  >;

  // --- Statistics ---

  /** Get comprehensive system statistics */
  readonly getStats: () => Effect.Effect<LearningStats, OrchestratorError>;

  /** Get training loop state */
  readonly getLoopState: () => Effect.Effect<LoopState | null, OrchestratorError>;
}

// --- Service Tag ---

export class LearningOrchestrator extends Context.Tag("LearningOrchestrator")<
  LearningOrchestrator,
  ILearningOrchestrator
>() {}

// --- Implementation ---

const makeLearningOrchestrator = (): Effect.Effect<
  ILearningOrchestrator,
  never,
  SkillService | MemoryService | ReflexionService | ArchivistService | TrainerService | TrainingLoop
> =>
  Effect.gen(function* () {
    const skills = yield* SkillService;
    const memory = yield* MemoryService;
    const reflexion = yield* ReflexionService;
    const archivist = yield* ArchivistService;
    const trainer = yield* TrainerService;
    const loop = yield* TrainingLoop;

    const mapSkillError = Effect.mapError(OrchestratorError.from);
    const mapMemoryError = Effect.mapError(OrchestratorError.from);
    const mapReflexionError = Effect.mapError(OrchestratorError.from);
    const mapArchivistError = Effect.mapError(OrchestratorError.from);
    const mapTrainerError = Effect.mapError(OrchestratorError.from);
    const mapLoopError = Effect.mapError(OrchestratorError.from);

    const initialize = (): Effect.Effect<void, OrchestratorError> =>
      Effect.gen(function* () {
        // Bootstrap skills if needed
        const allSkills = yield* skills.getAllSkills().pipe(mapSkillError);
        if (allSkills.length === 0) {
          yield* skills.bootstrapSkills().pipe(mapSkillError);
        }
      });

    const startTraining = (
      config?: Partial<LoopConfig>,
    ): Effect.Effect<LoopState, OrchestratorError> =>
      loop.start(config).pipe(mapLoopError);

    const stopTraining = (): Effect.Effect<LoopState, OrchestratorError> =>
      loop.stop().pipe(Effect.mapError(OrchestratorError.from));

    const pauseTraining = (): Effect.Effect<void, OrchestratorError> =>
      loop.pause().pipe(Effect.mapError(OrchestratorError.from));

    const resumeTraining = (): Effect.Effect<void, OrchestratorError> =>
      loop.resume().pipe(Effect.mapError(OrchestratorError.from));

    const executeTask = (task: TrainingTask): Effect.Effect<TrainingRun, OrchestratorError> =>
      Effect.gen(function* () {
        const run = yield* trainer.runTasks([task]).pipe(mapTrainerError);
        return run;
      });

    const runBenchmark = (subset: TBSubset): Effect.Effect<TrainingRun, OrchestratorError> =>
      Effect.gen(function* () {
        const result = yield* trainer.runBenchmark(subset).pipe(mapTrainerError);
        const run = yield* trainer.getRunStatus(result.runId).pipe(mapTrainerError);
        if (!run) {
          throw new OrchestratorError("not_found", `Run ${result.runId} not found`);
        }
        return run;
      });

    const findSkills = (
      query: string,
      maxResults: number = 5,
    ): Effect.Effect<Skill[], OrchestratorError> =>
      skills.searchSkills(query, { maxResults }).pipe(mapSkillError);

    const getAllSkills = (): Effect.Effect<Skill[], OrchestratorError> =>
      skills.getAllSkills().pipe(mapSkillError);

    const bootstrapSkills = (): Effect.Effect<number, OrchestratorError> =>
      skills.bootstrapSkills().pipe(mapSkillError);

    const findMemories = (
      query: string,
      maxResults: number = 5,
    ): Effect.Effect<Memory[], OrchestratorError> =>
      memory.getRelevantMemories(query, { maxResults }).pipe(mapMemoryError);

    const recordExperience = (
      content: string,
      type: "episodic" | "semantic" | "procedural",
      tags?: string[],
    ): Effect.Effect<Memory, OrchestratorError> => {
      switch (type) {
        case "episodic":
          return memory.recordTask(content, "success", { tags }).pipe(mapMemoryError);
        case "semantic":
          return memory.recordKnowledge("fact", content, { tags }).pipe(mapMemoryError);
        case "procedural":
          return memory.recordKnowledge("pattern", content, { tags }).pipe(mapMemoryError);
      }
    };

    const recordFailure = (
      taskDescription: string,
      errorMessage: string,
    ): Effect.Effect<Reflection, OrchestratorError> =>
      Effect.gen(function* () {
        const failure = yield* reflexion
          .recordFailure(taskDescription, errorMessage)
          .pipe(mapReflexionError);
        const reflection = yield* reflexion.quickReflect(failure);
        return reflection;
      });

    const getReflections = (
      taskDescription: string,
    ): Effect.Effect<Reflection[], OrchestratorError> =>
      reflexion.getReflections(taskDescription).pipe(mapReflexionError);

    const runArchive = (): Effect.Effect<ArchiveResult, OrchestratorError> =>
      archivist.runArchive().pipe(mapArchivistError);

    const getArchiveStats = (): Effect.Effect<
      {
        totalTrajectories: number;
        unarchivedTrajectories: number;
        successfulTrajectories: number;
        patternsExtracted: number;
        skillsCreated: number;
      },
      OrchestratorError
    > => archivist.getStats().pipe(mapArchivistError);

    const getLoopState = (): Effect.Effect<LoopState | null, OrchestratorError> =>
      loop.getState().pipe(Effect.map((s) => (s.status === "stopped" ? null : s)));

    const getStats = (): Effect.Effect<LearningStats, OrchestratorError> =>
      Effect.gen(function* () {
        // Gather all stats
        const allSkills = yield* skills.getAllSkills().pipe(mapSkillError);
        const memoryStats = yield* memory.getStats().pipe(mapMemoryError);
        const reflexionStats = yield* reflexion.getStats().pipe(mapReflexionError);
        const archivistStats = yield* archivist.getStats().pipe(mapArchivistError);
        const completedRuns = yield* trainer.getCompletedRuns().pipe(mapTrainerError);
        const loopState = yield* getLoopState();

        // Calculate skill stats
        const bootstrappedSkills = allSkills.filter((s) => s.source === "builtin").length;
        const learnedSkills = allSkills.filter((s) => s.source === "learned").length;
        const skillsByCategory: Record<string, number> = {};
        for (const skill of allSkills) {
          skillsByCategory[skill.category] = (skillsByCategory[skill.category] ?? 0) + 1;
        }

        // Calculate training stats
        const totalTasksCompleted = completedRuns.reduce(
          (sum, r) => sum + r.stats.completedTasks,
          0,
        );
        const totalSuccessful = completedRuns.reduce(
          (sum, r) => sum + r.stats.successfulTasks,
          0,
        );
        const overallSuccessRate =
          totalTasksCompleted > 0 ? totalSuccessful / totalTasksCompleted : 0;

        return {
          skills: {
            total: allSkills.length,
            bootstrapped: bootstrappedSkills,
            learned: learnedSkills,
            byCategory: skillsByCategory,
          },
          memories: {
            total: memoryStats.totalMemories,
            episodic: memoryStats.episodicCount,
            semantic: memoryStats.semanticCount,
            procedural: memoryStats.proceduralCount,
          },
          reflexion: reflexionStats,
          archivist: archivistStats,
          training: {
            totalRuns: completedRuns.length,
            totalTasksCompleted,
            overallSuccessRate,
            currentTier: loopState?.currentSubset ?? "TB_10",
          },
          loop: loopState,
        };
      });

    return {
      initialize,
      startTraining,
      stopTraining,
      pauseTraining,
      resumeTraining,
      executeTask,
      runBenchmark,
      findSkills,
      getAllSkills,
      bootstrapSkills,
      findMemories,
      recordExperience,
      recordFailure,
      getReflections,
      runArchive,
      getArchiveStats,
      getStats,
      getLoopState,
    };
  });

// --- Layer ---

export const LearningOrchestratorLayer: Layer.Layer<
  LearningOrchestrator,
  never,
  SkillService | MemoryService | ReflexionService | ArchivistService | TrainerService | TrainingLoop
> = Layer.effect(LearningOrchestrator, makeLearningOrchestrator());

/**
 * Create a complete LearningOrchestrator layer with all dependencies.
 */
export const makeLearningOrchestratorLive = (
  projectRoot: string = process.cwd(),
): Layer.Layer<LearningOrchestrator, never, never> => {
  const skillLayer = makeSkillServiceLive(projectRoot);
  const memoryLayer = makeMemoryServiceLive(projectRoot);
  const reflexionLayer = makeReflexionServiceLive(projectRoot);
  const archivistLayer = makeArchivistServiceLive(projectRoot);
  const trainerLayer = makeTrainerServiceLive(projectRoot);
  const loopLayer = makeTrainingLoopLive(projectRoot);

  return Layer.provide(
    LearningOrchestratorLayer,
    Layer.mergeAll(
      skillLayer,
      memoryLayer,
      reflexionLayer,
      archivistLayer,
      trainerLayer,
      loopLayer,
    ),
  );
};

export const LearningOrchestratorLive: Layer.Layer<LearningOrchestrator, never, never> =
  makeLearningOrchestratorLive();
