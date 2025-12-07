/**
 * Archivist Service
 *
 * Unified service for the Archivist subagent.
 * Reviews trajectories, extracts patterns, and manages the skill/memory library.
 */

import { Effect, Context, Layer } from "effect";
import { TrajectoryStore, makeTrajectoryStoreLive, type TrajectoryStoreError } from "./store.js";
import {
  PatternExtractor,
  PatternExtractorLive,
  type PatternExtractorError,
} from "./extractor.js";
import { SkillService, makeSkillServiceLive, type SkillServiceError } from "../skills/service.js";
import type { SkillStoreError } from "../skills/store.js";
import { MemoryService, makeMemoryServiceLive, type MemoryServiceError } from "../memory/service.js";
import type { MemoryStoreError } from "../memory/store.js";
import { makeFMServiceLayer } from "../fm/service.js";
import type { Trajectory, ExtractedPattern, ArchiveResult, ArchiveConfig } from "./schema.js";
import { DEFAULT_ARCHIVE_CONFIG, generateArchiveId, createTrajectory } from "./schema.js";
import { createSkill } from "../skills/schema.js";

// --- Error Types ---

export class ArchivistError extends Error {
  readonly _tag = "ArchivistError";
  constructor(
    readonly reason: string,
    message: string,
    override readonly cause?: Error,
  ) {
    super(message);
    this.name = "ArchivistError";
  }

  static from(
    e: TrajectoryStoreError | PatternExtractorError | SkillServiceError | MemoryServiceError,
  ): ArchivistError {
    return new ArchivistError(e._tag, e.message, e);
  }
}

// --- Service Interface ---

export interface IArchivistService {
  // --- Trajectory Management ---

  /** Record a completed task trajectory */
  readonly recordTrajectory: (
    taskId: string,
    taskDescription: string,
    data: {
      actions: Trajectory["actions"];
      outcome: Trajectory["outcome"];
      errorMessage?: string;
      skillsUsed?: string[];
      filesModified?: string[];
      totalDurationMs: number;
      model: string;
      tokens: Trajectory["tokens"];
      projectId?: string;
    },
  ) => Effect.Effect<Trajectory, ArchivistError>;

  /** Get a trajectory by ID */
  readonly getTrajectory: (id: string) => Effect.Effect<Trajectory | null, ArchivistError>;

  /** Get all trajectories */
  readonly getTrajectories: () => Effect.Effect<Trajectory[], ArchivistError>;

  /** Get unarchived trajectories */
  readonly getUnarchivedTrajectories: () => Effect.Effect<Trajectory[], ArchivistError>;

  // --- Archive Operations ---

  /** Run an archive cycle (extract patterns, create skills, update memories) */
  readonly runArchive: () => Effect.Effect<ArchiveResult, ArchivistError>;

  /** Run a quick archive (heuristic-based, no FM call) */
  readonly runQuickArchive: () => Effect.Effect<ArchiveResult, ArchivistError>;

  /** Extract patterns from trajectories without saving */
  readonly extractPatterns: (
    trajectories: Trajectory[],
  ) => Effect.Effect<ExtractedPattern[], ArchivistError>;

  /** Convert patterns to skills and register them */
  readonly promotePatterns: (
    patterns: ExtractedPattern[],
    runId?: string,
  ) => Effect.Effect<string[], ArchivistError>;

  // --- Pruning ---

  /** Prune old trajectories */
  readonly pruneTrajectories: (maxAgeDays?: number) => Effect.Effect<number, ArchivistError>;

  /** Prune low-performing skills */
  readonly pruneSkills: (threshold?: number) => Effect.Effect<number, ArchivistError>;

  // --- Stats ---

  /** Get archivist statistics */
  readonly getStats: () => Effect.Effect<
    {
      totalTrajectories: number;
      unarchivedTrajectories: number;
      successfulTrajectories: number;
      patternsExtracted: number;
      skillsCreated: number;
    },
    ArchivistError
  >;
}

// --- Service Tag ---

export class ArchivistService extends Context.Tag("ArchivistService")<
  ArchivistService,
  IArchivistService
>() {}

// --- In-Memory Stats ---

interface ArchivistStats {
  patternsExtracted: number;
  skillsCreated: number;
}

// --- Implementation ---

const makeArchivistService = (
  config: ArchiveConfig,
): Effect.Effect<
  IArchivistService,
  never,
  TrajectoryStore | PatternExtractor | SkillService | MemoryService
> =>
  Effect.gen(function* () {
    const store = yield* TrajectoryStore;
    const extractor = yield* PatternExtractor;
    const skills = yield* SkillService;
    const memory = yield* MemoryService;

    const stats: ArchivistStats = {
      patternsExtracted: 0,
      skillsCreated: 0,
    };

    const mapStoreError = Effect.mapError(ArchivistError.from);
    const mapExtractorError = Effect.mapError(ArchivistError.from);
    const mapSkillError = Effect.mapError(ArchivistError.from);
    const mapMemoryError = Effect.mapError(ArchivistError.from);

    const recordTrajectory = (
      taskId: string,
      taskDescription: string,
      data: {
        actions: Trajectory["actions"];
        outcome: Trajectory["outcome"];
        errorMessage?: string;
        skillsUsed?: string[];
        filesModified?: string[];
        totalDurationMs: number;
        model: string;
        tokens: Trajectory["tokens"];
        projectId?: string;
      },
    ): Effect.Effect<Trajectory, ArchivistError> =>
      Effect.gen(function* () {
        const trajectory = createTrajectory(taskId, taskDescription, data);
        yield* store.save(trajectory).pipe(mapStoreError);

        // Also record in episodic memory
        const memoryOptions: {
          durationMs: number;
          importance: "medium" | "high";
          tags: string[];
          errorMessage?: string;
          skillsUsed?: string[];
          filesModified?: string[];
          projectId?: string;
        } = {
          durationMs: data.totalDurationMs,
          importance: data.outcome === "success" ? "medium" : "high",
          tags: ["trajectory", data.outcome],
        };

        if (data.errorMessage) {
          memoryOptions.errorMessage = data.errorMessage;
        }
        if (data.skillsUsed) {
          memoryOptions.skillsUsed = data.skillsUsed;
        }
        if (data.filesModified) {
          memoryOptions.filesModified = data.filesModified;
        }
        if (data.projectId) {
          memoryOptions.projectId = data.projectId;
        }

        yield* memory
          .recordTask(taskDescription, data.outcome, memoryOptions)
          .pipe(mapMemoryError);

        return trajectory;
      });

    const getTrajectory = (id: string): Effect.Effect<Trajectory | null, ArchivistError> =>
      store.get(id).pipe(mapStoreError);

    const getTrajectories = (): Effect.Effect<Trajectory[], ArchivistError> =>
      store.getAll().pipe(mapStoreError);

    const getUnarchivedTrajectories = (): Effect.Effect<Trajectory[], ArchivistError> =>
      store.getUnarchived().pipe(mapStoreError);

    const extractPatternsOp = (
      trajectories: Trajectory[],
    ): Effect.Effect<ExtractedPattern[], ArchivistError> =>
      extractor.extractPatterns(trajectories).pipe(mapExtractorError);

    const promotePatterns = (
      patterns: ExtractedPattern[],
      runId?: string,
    ): Effect.Effect<string[], ArchivistError> =>
      Effect.gen(function* () {
        const runIdentifier = runId ?? generateArchiveId();
        const skillIds: string[] = [];

        for (const pattern of patterns) {
          if (pattern.type !== "skill" && pattern.type !== "optimization") {
            continue;
          }

          // Create skill from pattern
          const skill = createSkill({
            name: pattern.name,
            description: pattern.description,
            code: pattern.content,
            category: pattern.category as any,
            source: "learned",
            tags: [...pattern.tags, "archivist", `confidence-${Math.round(pattern.confidence * 100)}`],
          });

          yield* skills.registerSkill(skill).pipe(mapSkillError);

          // Link skill in memory
          yield* memory
            .linkSkill(skill.id, pattern.triggerContext, {
              importance: "high",
              tags: ["learned-skill", "archivist"],
            })
            .pipe(mapMemoryError);

          skillIds.push(skill.id);
          stats.skillsCreated++;

          // Emit HUD message for skill promotion
          config.onHudMessage?.({
            type: "archivist_skill_promoted",
            runId: runIdentifier,
            skillId: skill.id,
            skillName: skill.name,
            category: skill.category,
          });
        }

        stats.patternsExtracted += patterns.length;
        return skillIds;
      });

    const runArchive = (): Effect.Effect<ArchiveResult, ArchivistError> =>
      Effect.gen(function* () {
        const startTime = Date.now();
        const runId = generateArchiveId();

        // Get unarchived trajectories
        const trajectories = yield* getUnarchivedTrajectories();

        // Emit run start message
        config.onHudMessage?.({
          type: "archivist_run_start",
          runId,
          trajectoriesToProcess: trajectories.length,
          mode: "full",
          timestamp: new Date().toISOString(),
        });

        if (trajectories.length === 0) {
          const result = {
            id: runId,
            trajectoriesProcessed: 0,
            patternsExtracted: 0,
            skillsCreated: 0,
            memoriesCreated: 0,
            itemsPruned: 0,
            durationMs: Date.now() - startTime,
            timestamp: new Date().toISOString(),
          };
          // Emit run complete message
          config.onHudMessage?.({
            type: "archivist_run_complete",
            runId,
            trajectoriesProcessed: 0,
            patternsExtracted: 0,
            skillsCreated: 0,
            memoriesCreated: 0,
            itemsPruned: 0,
            durationMs: result.durationMs,
          });
          return result;
        }

        // Extract patterns
        const patterns = yield* extractPatternsOp(trajectories);

        // Emit pattern found messages
        for (const pattern of patterns) {
          config.onHudMessage?.({
            type: "archivist_pattern_found",
            runId,
            patternName: pattern.name,
            patternType: pattern.type,
            confidence: pattern.confidence,
          });
        }

        // Filter by quality
        const qualityPatterns = yield* extractor.filterByQuality(
          patterns,
          0.6,
          config.minOccurrences,
        );

        // Promote to skills (passes runId for HUD messages)
        const skillIds = yield* promotePatterns(qualityPatterns, runId);

        // Mark trajectories as archived
        yield* store.markArchived(trajectories.map((t) => t.id)).pipe(mapStoreError);

        // Prune old trajectories if configured
        let itemsPruned = 0;
        if (config.autoPrune) {
          itemsPruned = yield* store.prune(config.maxTrajectoryAgeDays).pipe(mapStoreError);
        }

        const result = {
          id: runId,
          trajectoriesProcessed: trajectories.length,
          patternsExtracted: patterns.length,
          skillsCreated: skillIds.length,
          memoriesCreated: patterns.length, // Each pattern creates a memory link
          itemsPruned,
          durationMs: Date.now() - startTime,
          timestamp: new Date().toISOString(),
        };

        // Emit run complete message
        config.onHudMessage?.({
          type: "archivist_run_complete",
          runId,
          trajectoriesProcessed: result.trajectoriesProcessed,
          patternsExtracted: result.patternsExtracted,
          skillsCreated: result.skillsCreated,
          memoriesCreated: result.memoriesCreated,
          itemsPruned: result.itemsPruned,
          durationMs: result.durationMs,
        });

        return result;
      });

    const runQuickArchive = (): Effect.Effect<ArchiveResult, ArchivistError> =>
      Effect.gen(function* () {
        const startTime = Date.now();
        const runId = generateArchiveId();

        // Get unarchived trajectories
        const trajectories = yield* getUnarchivedTrajectories();

        // Emit run start message
        config.onHudMessage?.({
          type: "archivist_run_start",
          runId,
          trajectoriesToProcess: trajectories.length,
          mode: "quick",
          timestamp: new Date().toISOString(),
        });

        if (trajectories.length === 0) {
          const result = {
            id: runId,
            trajectoriesProcessed: 0,
            patternsExtracted: 0,
            skillsCreated: 0,
            memoriesCreated: 0,
            itemsPruned: 0,
            durationMs: Date.now() - startTime,
            timestamp: new Date().toISOString(),
          };
          // Emit run complete message
          config.onHudMessage?.({
            type: "archivist_run_complete",
            runId,
            trajectoriesProcessed: 0,
            patternsExtracted: 0,
            skillsCreated: 0,
            memoriesCreated: 0,
            itemsPruned: 0,
            durationMs: result.durationMs,
          });
          return result;
        }

        // Extract patterns using heuristics
        const patterns = yield* extractor.extractQuickPatterns(trajectories);

        // Emit pattern found messages
        for (const pattern of patterns) {
          config.onHudMessage?.({
            type: "archivist_pattern_found",
            runId,
            patternName: pattern.name,
            patternType: pattern.type,
            confidence: pattern.confidence,
          });
        }

        // Promote to skills (passes runId for HUD messages)
        const skillIds = yield* promotePatterns(patterns, runId);

        // Mark trajectories as archived
        yield* store.markArchived(trajectories.map((t) => t.id)).pipe(mapStoreError);

        const result = {
          id: runId,
          trajectoriesProcessed: trajectories.length,
          patternsExtracted: patterns.length,
          skillsCreated: skillIds.length,
          memoriesCreated: patterns.length,
          itemsPruned: 0,
          durationMs: Date.now() - startTime,
          timestamp: new Date().toISOString(),
        };

        // Emit run complete message
        config.onHudMessage?.({
          type: "archivist_run_complete",
          runId,
          trajectoriesProcessed: result.trajectoriesProcessed,
          patternsExtracted: result.patternsExtracted,
          skillsCreated: result.skillsCreated,
          memoriesCreated: result.memoriesCreated,
          itemsPruned: 0,
          durationMs: result.durationMs,
        });

        return result;
      });

    const pruneTrajectories = (
      maxAgeDays: number = config.maxTrajectoryAgeDays,
    ): Effect.Effect<number, ArchivistError> => store.prune(maxAgeDays).pipe(mapStoreError);

    const pruneSkills = (threshold: number = config.pruneThreshold): Effect.Effect<number, ArchivistError> =>
      Effect.gen(function* () {
        // Get all skills and their usage stats
        const allSkills = yield* skills.getAllSkills().pipe(mapSkillError);

        let pruned = 0;
        for (const skill of allSkills) {
          const usageCount = skill.usageCount ?? 0;
          // Check if skill has low usage
          if (usageCount < 2 && skill.source === "learned") {
            // Calculate age
            const age = Date.now() - new Date(skill.createdAt).getTime();
            const ageDays = age / (24 * 60 * 60 * 1000);

            // Prune if old and unused
            if (ageDays > 7) {
              yield* skills.deleteSkill(skill.id).pipe(mapSkillError);
              pruned++;
            }
          }
        }

        return pruned;
      });

    const getStatsOp = (): Effect.Effect<
      {
        totalTrajectories: number;
        unarchivedTrajectories: number;
        successfulTrajectories: number;
        patternsExtracted: number;
        skillsCreated: number;
      },
      ArchivistError
    > =>
      Effect.gen(function* () {
        const all = yield* getTrajectories();
        const unarchived = yield* getUnarchivedTrajectories();

        return {
          totalTrajectories: all.length,
          unarchivedTrajectories: unarchived.length,
          successfulTrajectories: all.filter((t) => t.outcome === "success").length,
          patternsExtracted: stats.patternsExtracted,
          skillsCreated: stats.skillsCreated,
        };
      });

    return {
      recordTrajectory,
      getTrajectory,
      getTrajectories,
      getUnarchivedTrajectories,
      runArchive,
      runQuickArchive,
      extractPatterns: extractPatternsOp,
      promotePatterns,
      pruneTrajectories,
      pruneSkills,
      getStats: getStatsOp,
    };
  });

// --- Layer ---

export const ArchivistServiceLayer: Layer.Layer<
  ArchivistService,
  never,
  TrajectoryStore | PatternExtractor | SkillService | MemoryService
> = Layer.effect(ArchivistService, makeArchivistService(DEFAULT_ARCHIVE_CONFIG));

/**
 * Create a complete ArchivistService layer with all dependencies.
 */
export const makeArchivistServiceLive = (
  projectRoot: string = process.cwd(),
): Layer.Layer<
  ArchivistService,
  TrajectoryStoreError | PatternExtractorError | SkillStoreError | MemoryStoreError,
  never
> => {
  const fmLayer = makeFMServiceLayer({ autoStart: false, enableLogging: false });
  const storeLayer = makeTrajectoryStoreLive(projectRoot);
  const extractorLayer = Layer.provide(PatternExtractorLive, fmLayer);
  const skillLayer = makeSkillServiceLive(projectRoot);
  const memoryLayer = makeMemoryServiceLive(projectRoot);

  return Layer.provide(
    ArchivistServiceLayer,
    Layer.mergeAll(storeLayer, extractorLayer, skillLayer, memoryLayer),
  );
};

/**
 * Default ArchivistService layer.
 */
export const ArchivistServiceLive: Layer.Layer<
  ArchivistService,
  TrajectoryStoreError | PatternExtractorError | SkillStoreError | MemoryStoreError,
  never
> = makeArchivistServiceLive();
