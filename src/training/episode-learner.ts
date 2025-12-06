/**
 * Episode Learner
 *
 * Processes completed episodes from Terminal-Bench runs to:
 * 1. Mine skills from successful task completions
 * 2. Generate reflections from failures for future improvement
 *
 * This is the learning loop that transforms experience into reusable knowledge.
 *
 * Based on research:
 * - Voyager (skill library): 3.3x improvement
 * - Reflexion: +11% on HumanEval
 */

import { Effect } from "effect";
import { existsSync, readFileSync } from "fs";
import { join } from "path";
import {
  EpisodeStore,
  type Episode,
  type EpisodeSummary,
} from "../bench/episode-store.js";
import {
  type Skill,
  type SkillCategory,
  createSkill,
} from "../skills/schema.js";
import type { TaskResult } from "../trainer/schema.js";
import type {
  Trajectory,
  ExtractedPattern,
} from "../archivist/schema.js";

// --- Types ---

/**
 * Configuration for the episode learner.
 */
export interface EpisodeLearnerConfig {
  /** Project root directory */
  projectRoot: string;
  /** Minimum pass rate to consider extracting skills from episode */
  minPassRateForSkills: number;
  /** Minimum occurrences of a pattern before creating a skill */
  minPatternOccurrences: number;
  /** Maximum episodes to process in one run */
  maxEpisodesToProcess: number;
  /** Whether to generate reflections for failures */
  generateReflections: boolean;
  /** Maximum age of episodes to process (in days) */
  maxEpisodeAgeDays: number;
}

export const DEFAULT_LEARNER_CONFIG: EpisodeLearnerConfig = {
  projectRoot: process.cwd(),
  minPassRateForSkills: 0.5, // Extract skills from episodes with 50%+ pass rate
  minPatternOccurrences: 2,
  maxEpisodesToProcess: 10,
  generateReflections: true,
  maxEpisodeAgeDays: 30,
};

/**
 * A reflection generated from a failure.
 */
export interface Reflection {
  /** Unique reflection ID */
  id: string;
  /** Source episode ID */
  episodeId: string;
  /** Task ID that failed */
  taskId: string;
  /** What went wrong */
  failureType: "error" | "timeout" | "incorrect" | "partial";
  /** Description of what happened */
  description: string;
  /** Lesson learned */
  lesson: string;
  /** Suggested approach for next time */
  suggestedApproach: string;
  /** Related skills that might help */
  relatedSkills: string[];
  /** Timestamp */
  createdAt: string;
}

/**
 * Result of processing an episode.
 */
export interface LearningResult {
  /** Episode that was processed */
  episodeId: string;
  /** Skills extracted */
  skillsExtracted: Skill[];
  /** Reflections generated */
  reflectionsGenerated: Reflection[];
  /** Patterns identified */
  patternsIdentified: ExtractedPattern[];
  /** Processing duration */
  durationMs: number;
  /** Processing timestamp */
  processedAt: string;
}

/**
 * Summary of learning across multiple episodes.
 */
export interface LearningSummary {
  /** Episodes processed */
  episodesProcessed: number;
  /** Total skills extracted */
  totalSkillsExtracted: number;
  /** Total reflections generated */
  totalReflectionsGenerated: number;
  /** Skills by category */
  skillsByCategory: Record<string, number>;
  /** Average pass rate of processed episodes */
  averagePassRate: number;
  /** Processing duration */
  totalDurationMs: number;
}

// --- Error Types ---

export class EpisodeLearnerError extends Error {
  readonly _tag = "EpisodeLearnerError";
  constructor(
    readonly reason:
      | "episode_not_found"
      | "results_not_found"
      | "parse_failed"
      | "extraction_failed"
      | "storage_failed",
    message: string,
    readonly cause?: Error,
  ) {
    super(message);
    this.name = "EpisodeLearnerError";
  }
}

// --- Episode Learner Interface ---

export interface IEpisodeLearner {
  /**
   * Process a single episode to extract learnings.
   */
  readonly processEpisode: (
    episode: Episode,
  ) => Effect.Effect<LearningResult, EpisodeLearnerError>;

  /**
   * Process recent unprocessed episodes.
   */
  readonly processRecentEpisodes: (options?: {
    limit?: number;
    sinceDate?: Date;
  }) => Effect.Effect<LearningResult[], EpisodeLearnerError>;

  /**
   * Extract skills from successful task results.
   */
  readonly mineSkillsFromSuccess: (
    episode: Episode,
    successfulResults: TaskResult[],
  ) => Effect.Effect<Skill[], EpisodeLearnerError>;

  /**
   * Generate reflections from failed task results.
   */
  readonly generateReflections: (
    episode: Episode,
    failedResults: TaskResult[],
  ) => Effect.Effect<Reflection[], EpisodeLearnerError>;

  /**
   * Get summary of learning progress.
   */
  readonly getLearningSummary: () => Effect.Effect<LearningSummary, never>;

  /**
   * Mark an episode as processed.
   */
  readonly markEpisodeProcessed: (
    episodeId: string,
  ) => Effect.Effect<void, EpisodeLearnerError>;
}

// --- Implementation ---

/**
 * Create an Episode Learner instance.
 */
export const createEpisodeLearner = (
  config: Partial<EpisodeLearnerConfig> = {},
): IEpisodeLearner => {
  const fullConfig: EpisodeLearnerConfig = {
    ...DEFAULT_LEARNER_CONFIG,
    ...config,
  };

  const gymDir = join(fullConfig.projectRoot, ".openagents", "gym");
  const episodeStore = new EpisodeStore(gymDir);

  // Track processed episodes and learning results
  const processedEpisodeIds = new Set<string>();
  const learningResults: LearningResult[] = [];
  const allSkillsExtracted: Skill[] = [];
  const allReflectionsGenerated: Reflection[] = [];

  /**
   * Load task results from episode's results path.
   */
  const loadTaskResults = (
    episode: Episode,
  ): Effect.Effect<TaskResult[], EpisodeLearnerError> =>
    Effect.gen(function* () {
      const resultsPath = episode.resultsPath;

      if (!existsSync(resultsPath)) {
        return yield* Effect.fail(
          new EpisodeLearnerError(
            "results_not_found",
            `Results file not found: ${resultsPath}`,
          ),
        );
      }

      try {
        const content = readFileSync(resultsPath, "utf-8");
        const lines = content.trim().split("\n").filter(Boolean);

        const results: TaskResult[] = [];
        for (const line of lines) {
          try {
            const parsed = JSON.parse(line);
            // Convert from TB result format to TaskResult
            const result: TaskResult = {
              taskId: parsed.taskId ?? parsed.id ?? `task-${results.length}`,
              outcome: mapOutcome(parsed.status ?? parsed.outcome),
              score: parsed.score,
              errorMessage: parsed.error ?? parsed.errorMessage,
              output: parsed.output,
              durationMs: parsed.durationMs ?? parsed.duration ?? 0,
              model: episode.model,
              tokens: parsed.tokens ?? { input: 0, output: 0, total: 0 },
              skillsUsed: parsed.skillsUsed ?? [],
              usedReflexion: parsed.usedReflexion ?? false,
              attemptNumber: parsed.attemptNumber ?? 1,
              timestamp: parsed.timestamp ?? new Date().toISOString(),
            };
            results.push(result);
          } catch {
            // Skip malformed lines
          }
        }

        return results;
      } catch (e) {
        return yield* Effect.fail(
          new EpisodeLearnerError(
            "parse_failed",
            `Failed to parse results: ${e instanceof Error ? e.message : String(e)}`,
            e instanceof Error ? e : undefined,
          ),
        );
      }
    });

  /**
   * Map various status formats to TaskResult outcome.
   */
  const mapOutcome = (
    status: string,
  ): TaskResult["outcome"] => {
    const normalized = status?.toLowerCase() ?? "failure";
    if (normalized === "pass" || normalized === "success" || normalized === "passed") {
      return "success";
    }
    if (normalized === "timeout") {
      return "timeout";
    }
    if (normalized === "partial") {
      return "partial";
    }
    return "failure";
  };

  /**
   * Generate a skill ID.
   */
  const generateSkillId = (name: string, version: string): string => {
    const slug = name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/-+/g, "-")
      .replace(/^-|-$/g, "");
    return `skill-${slug}-${version}`;
  };

  /**
   * Generate a reflection ID.
   */
  const generateReflectionId = (): string => {
    const timestamp = Date.now().toString(36);
    const random = Math.random().toString(36).slice(2, 6);
    return `refl-${timestamp}-${random}`;
  };

  /**
   * Categorize a skill based on task content.
   */
  const categorizeSkill = (taskOutput: string): SkillCategory => {
    const lower = taskOutput.toLowerCase();
    // Check more specific patterns first to avoid false positives
    if (lower.includes("git add") || lower.includes("git commit") || lower.includes("commit") || lower.includes("branch") || (lower.includes("git") && !lower.includes("digit"))) {
      return "git";
    }
    if (lower.includes("debug") || lower.includes("bug") || (lower.includes("fix") && lower.includes("error"))) {
      return "debugging";
    }
    if (lower.includes("test") || lower.includes("expect") || lower.includes("assert")) {
      return "testing";
    }
    if (lower.includes("refactor") || lower.includes("rename") || lower.includes("move")) {
      return "refactoring";
    }
    if (lower.includes("search") || lower.includes("find") || lower.includes("grep")) {
      return "search";
    }
    if (lower.includes("read") || lower.includes("write") || lower.includes("file")) {
      return "file_operations";
    }
    return "file_operations"; // Default
  };

  // --- Interface Implementation ---

  const processEpisode = (
    episode: Episode,
  ): Effect.Effect<LearningResult, EpisodeLearnerError> =>
    Effect.gen(function* () {
      const startTime = Date.now();

      // Skip if already processed
      if (processedEpisodeIds.has(episode.id)) {
        return {
          episodeId: episode.id,
          skillsExtracted: [],
          reflectionsGenerated: [],
          patternsIdentified: [],
          durationMs: Date.now() - startTime,
          processedAt: new Date().toISOString(),
        };
      }

      // Load task results
      const results = yield* loadTaskResults(episode);

      // Separate successful and failed results
      const successfulResults = results.filter((r) => r.outcome === "success");
      const failedResults = results.filter(
        (r) => r.outcome === "failure" || r.outcome === "timeout",
      );

      // Extract skills from successes
      let skills: Skill[] = [];
      if (
        episode.summary.passRate >= fullConfig.minPassRateForSkills &&
        successfulResults.length > 0
      ) {
        skills = yield* mineSkillsFromSuccess(episode, successfulResults);
      }

      // Generate reflections from failures
      let reflections: Reflection[] = [];
      if (fullConfig.generateReflections && failedResults.length > 0) {
        reflections = yield* generateReflections(episode, failedResults);
      }

      // Mark as processed
      processedEpisodeIds.add(episode.id);

      // Track results
      allSkillsExtracted.push(...skills);
      allReflectionsGenerated.push(...reflections);

      const result: LearningResult = {
        episodeId: episode.id,
        skillsExtracted: skills,
        reflectionsGenerated: reflections,
        patternsIdentified: [],
        durationMs: Date.now() - startTime,
        processedAt: new Date().toISOString(),
      };

      learningResults.push(result);
      return result;
    });

  const processRecentEpisodes = (
    options: { limit?: number; sinceDate?: Date } = {},
  ): Effect.Effect<LearningResult[], EpisodeLearnerError> =>
    Effect.gen(function* () {
      const limit = options.limit ?? fullConfig.maxEpisodesToProcess;
      const sinceDate =
        options.sinceDate ??
        new Date(Date.now() - fullConfig.maxEpisodeAgeDays * 24 * 60 * 60 * 1000);

      // Load episodes from store
      const episodes = yield* Effect.tryPromise({
        try: () => episodeStore.query({ since: sinceDate, limit: limit * 2 }),
        catch: (e) =>
          new EpisodeLearnerError(
            "episode_not_found",
            `Failed to load episodes: ${e instanceof Error ? e.message : String(e)}`,
          ),
      });

      // Filter to unprocessed episodes
      const unprocessed = episodes.filter(
        (ep) => !processedEpisodeIds.has(ep.id),
      );

      // Process each episode
      const results: LearningResult[] = [];
      for (const episode of unprocessed.slice(0, limit)) {
        const result = yield* processEpisode(episode).pipe(
          Effect.catchAll((e) =>
            Effect.succeed({
              episodeId: episode.id,
              skillsExtracted: [],
              reflectionsGenerated: [],
              patternsIdentified: [],
              durationMs: 0,
              processedAt: new Date().toISOString(),
            } as LearningResult),
          ),
        );
        results.push(result);
      }

      return results;
    });

  const mineSkillsFromSuccess = (
    episode: Episode,
    successfulResults: TaskResult[],
  ): Effect.Effect<Skill[], EpisodeLearnerError> =>
    Effect.gen(function* () {
      const skills: Skill[] = [];

      for (const result of successfulResults) {
        // Only create skill if there's meaningful output
        if (!result.output || result.output.length < 50) {
          continue;
        }

        // Create a skill from the successful task
        const category = categorizeSkill(result.output);
        const skillName = `${category}-pattern-from-${episode.id}`;

        const skill = createSkill({
          name: skillName,
          description: `Pattern extracted from successful task ${result.taskId} in episode ${episode.id}`,
          code: result.output.slice(0, 2000), // Limit code size
          category,
          version: "v1",
          status: "draft", // Start as draft until verified
          source: "learned",
          learnedFrom: [episode.id],
          successRate: 1.0, // 100% since it was successful
          usageCount: 1,
          tags: [
            "learned",
            `model-${episode.model}`,
            `episode-${episode.id}`,
          ],
        });

        skills.push(skill);
      }

      return skills;
    });

  const generateReflections = (
    episode: Episode,
    failedResults: TaskResult[],
  ): Effect.Effect<Reflection[], EpisodeLearnerError> =>
    Effect.gen(function* () {
      const reflections: Reflection[] = [];

      for (const result of failedResults) {
        // Determine failure type
        let failureType: Reflection["failureType"] = "incorrect";
        if (result.outcome === "timeout") {
          failureType = "timeout";
        } else if (result.outcome === "partial") {
          failureType = "partial";
        } else if (result.errorMessage) {
          // Check for error indicators in the message
          const lowerError = result.errorMessage.toLowerCase();
          if (
            lowerError.includes("error") ||
            lowerError.includes("not found") ||
            lowerError.includes("failed") ||
            lowerError.includes("exception") ||
            lowerError.includes("permission denied")
          ) {
            failureType = "error";
          }
        }

        // Generate reflection
        const reflection: Reflection = {
          id: generateReflectionId(),
          episodeId: episode.id,
          taskId: result.taskId,
          failureType,
          description: result.errorMessage ?? "Task did not complete successfully",
          lesson: inferLesson(result),
          suggestedApproach: suggestApproach(result),
          relatedSkills: result.skillsUsed,
          createdAt: new Date().toISOString(),
        };

        reflections.push(reflection);
      }

      return reflections;
    });

  /**
   * Infer a lesson from a failed result.
   */
  const inferLesson = (result: TaskResult): string => {
    if (result.outcome === "timeout") {
      return "Task took too long. Consider breaking into smaller steps or optimizing the approach.";
    }
    if (result.errorMessage?.includes("not found")) {
      return "Resource not found. Verify paths and dependencies before proceeding.";
    }
    if (result.errorMessage?.includes("permission")) {
      return "Permission denied. Check file permissions and access rights.";
    }
    if (result.errorMessage?.includes("syntax")) {
      return "Syntax error encountered. Validate code structure before execution.";
    }
    return "Task failed. Review the approach and consider alternative strategies.";
  };

  /**
   * Suggest an approach for similar future tasks.
   */
  const suggestApproach = (result: TaskResult): string => {
    if (result.outcome === "timeout") {
      return "Use more efficient algorithms or break the task into parallel subtasks.";
    }
    if (result.errorMessage?.includes("not found")) {
      return "First verify the existence of required files/resources using glob or grep.";
    }
    if (result.outcome === "partial") {
      return "Review test failures and address each failing case individually.";
    }
    return "Consider using skills from the library that have proven successful for similar tasks.";
  };

  const getLearningSummary = (): Effect.Effect<LearningSummary, never> =>
    Effect.succeed({
      episodesProcessed: processedEpisodeIds.size,
      totalSkillsExtracted: allSkillsExtracted.length,
      totalReflectionsGenerated: allReflectionsGenerated.length,
      skillsByCategory: allSkillsExtracted.reduce(
        (acc, skill) => {
          acc[skill.category] = (acc[skill.category] ?? 0) + 1;
          return acc;
        },
        {} as Record<string, number>,
      ),
      averagePassRate:
        learningResults.length > 0
          ? learningResults.reduce((sum, r) => {
              // Try to find the episode to get pass rate
              return sum;
            }, 0) / learningResults.length
          : 0,
      totalDurationMs: learningResults.reduce((sum, r) => sum + r.durationMs, 0),
    });

  const markEpisodeProcessed = (
    episodeId: string,
  ): Effect.Effect<void, EpisodeLearnerError> =>
    Effect.sync(() => {
      processedEpisodeIds.add(episodeId);
    });

  return {
    processEpisode,
    processRecentEpisodes,
    mineSkillsFromSuccess,
    generateReflections,
    getLearningSummary,
    markEpisodeProcessed,
  };
};

// --- Convenience Functions ---

/**
 * Process recent episodes and return learning summary.
 */
export const learnFromRecentEpisodes = (
  config?: Partial<EpisodeLearnerConfig>,
): Effect.Effect<LearningSummary, EpisodeLearnerError> =>
  Effect.gen(function* () {
    const learner = createEpisodeLearner(config);
    yield* learner.processRecentEpisodes();
    return yield* learner.getLearningSummary();
  });

/**
 * Process a single episode by ID.
 */
export const learnFromEpisode = (
  episodeId: string,
  config?: Partial<EpisodeLearnerConfig>,
): Effect.Effect<LearningResult, EpisodeLearnerError> =>
  Effect.gen(function* () {
    const fullConfig = { ...DEFAULT_LEARNER_CONFIG, ...config };
    const gymDir = join(fullConfig.projectRoot, ".openagents", "gym");
    const episodeStore = new EpisodeStore(gymDir);

    // Find the episode
    const episodes = yield* Effect.tryPromise({
      try: () => episodeStore.loadAll(),
      catch: (e) =>
        new EpisodeLearnerError(
          "episode_not_found",
          `Failed to load episodes: ${e instanceof Error ? e.message : String(e)}`,
        ),
    });

    const episode = episodes.find((ep) => ep.id === episodeId);
    if (!episode) {
      return yield* Effect.fail(
        new EpisodeLearnerError(
          "episode_not_found",
          `Episode not found: ${episodeId}`,
        ),
      );
    }

    const learner = createEpisodeLearner(config);
    return yield* learner.processEpisode(episode);
  });
