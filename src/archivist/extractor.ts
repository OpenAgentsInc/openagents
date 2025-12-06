/**
 * Pattern Extractor
 *
 * Uses FM to analyze trajectories and extract reusable patterns.
 * Implements the pattern recognition loop from MechaCoder's learning system.
 */

import { Effect, Context, Layer } from "effect";
import { FMService, makeFMServiceLayer, type FMServiceError } from "../fm/service.js";
import type { Trajectory, ExtractedPattern, ArchiveConfig } from "./schema.js";
import {
  DEFAULT_ARCHIVE_CONFIG,
  buildPatternExtractionPrompt,
  parsePatternsFromResponse,
  calculateSuccessRate,
  groupSimilarTrajectories,
} from "./schema.js";

// --- Error Types ---

export class PatternExtractorError extends Error {
  readonly _tag = "PatternExtractorError";
  constructor(
    readonly reason: "extraction_failed" | "fm_error" | "no_patterns",
    message: string,
    readonly cause?: Error,
  ) {
    super(message);
    this.name = "PatternExtractorError";
  }

  static fromFM(e: FMServiceError): PatternExtractorError {
    return new PatternExtractorError("fm_error", e.message, e);
  }
}

// --- Extractor Interface ---

export interface IPatternExtractor {
  /** Extract patterns from a batch of trajectories */
  readonly extractPatterns: (
    trajectories: Trajectory[],
  ) => Effect.Effect<ExtractedPattern[], PatternExtractorError>;

  /** Extract patterns using heuristics (no FM call) */
  readonly extractQuickPatterns: (
    trajectories: Trajectory[],
  ) => Effect.Effect<ExtractedPattern[], never>;

  /** Analyze a single trajectory for potential patterns */
  readonly analyzeTrajectory: (
    trajectory: Trajectory,
  ) => Effect.Effect<ExtractedPattern[], PatternExtractorError>;

  /** Filter patterns by quality */
  readonly filterByQuality: (
    patterns: ExtractedPattern[],
    minConfidence: number,
    minOccurrences: number,
  ) => Effect.Effect<ExtractedPattern[], never>;
}

// --- Service Tag ---

export class PatternExtractor extends Context.Tag("PatternExtractor")<
  PatternExtractor,
  IPatternExtractor
>() {}

// --- Implementation ---

const makePatternExtractor = (
  config: ArchiveConfig,
): Effect.Effect<IPatternExtractor, never, FMService> =>
  Effect.gen(function* () {
    const fm = yield* FMService;

    const mapFMError = Effect.mapError(PatternExtractorError.fromFM);

    const extractPatterns = (
      trajectories: Trajectory[],
    ): Effect.Effect<ExtractedPattern[], PatternExtractorError> =>
      Effect.gen(function* () {
        if (trajectories.length === 0) {
          return [];
        }

        // Filter to trajectories within age limit
        const cutoff = Date.now() - config.maxTrajectoryAgeDays * 24 * 60 * 60 * 1000;
        const recentTrajectories = trajectories.filter(
          (t) => new Date(t.timestamp).getTime() > cutoff,
        );

        if (recentTrajectories.length === 0) {
          return [];
        }

        // Build extraction prompt
        const prompt = buildPatternExtractionPrompt(recentTrajectories);

        // Call FM for pattern extraction
        const response = yield* fm.generate(prompt, {}).pipe(mapFMError);

        // Parse patterns from response
        const sourceIds = recentTrajectories.map((t) => t.id);
        const patterns = parsePatternsFromResponse(response, sourceIds);

        // Calculate success rate for each pattern based on source trajectories
        for (const pattern of patterns) {
          const sourceTrajectories = recentTrajectories.filter((t) =>
            pattern.sourceTrajectoryIds.includes(t.id),
          );
          pattern.successRate = calculateSuccessRate(sourceTrajectories);
        }

        // Filter by minimum success rate
        return patterns.filter((p) => p.successRate >= config.minSuccessRate);
      });

    const extractQuickPatterns = (
      trajectories: Trajectory[],
    ): Effect.Effect<ExtractedPattern[], never> =>
      Effect.gen(function* () {
        if (trajectories.length === 0) {
          return [];
        }

        // Group similar trajectories
        const groups = groupSimilarTrajectories(trajectories);
        const patterns: ExtractedPattern[] = [];

        for (const [key, group] of groups) {
          if (group.length < config.minOccurrences) {
            continue;
          }

          const successRate = calculateSuccessRate(group);
          if (successRate < config.minSuccessRate) {
            continue;
          }

          // Extract common tools used
          const toolUsage = new Map<string, number>();
          for (const traj of group) {
            for (const action of traj.actions) {
              if (action.type === "tool_call" && action.tool) {
                toolUsage.set(action.tool, (toolUsage.get(action.tool) ?? 0) + 1);
              }
            }
          }

          // Find most common tools
          const commonTools = Array.from(toolUsage.entries())
            .sort((a, b) => b[1] - a[1])
            .slice(0, 3)
            .map(([tool]) => tool);

          // Create pattern from group
          const pattern: ExtractedPattern = {
            id: `pat-heur-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`,
            type: "skill",
            name: `${key} pattern`,
            description: `Common pattern from ${group.length} similar trajectories`,
            content: `Tools used: ${commonTools.join(", ")}`,
            triggerContext: group.slice(0, 3).map((t) => t.taskDescription),
            successRate,
            occurrences: group.length,
            sourceTrajectoryIds: group.map((t) => t.id),
            confidence: Math.min(0.5 + group.length * 0.1, 0.9),
            category: "general",
            tags: ["heuristic", ...commonTools],
            extractedAt: new Date().toISOString(),
          };

          patterns.push(pattern);
        }

        return patterns;
      });

    const analyzeTrajectory = (
      trajectory: Trajectory,
    ): Effect.Effect<ExtractedPattern[], PatternExtractorError> =>
      Effect.gen(function* () {
        // For single trajectory analysis, use simpler prompt
        const prompt = [
          "Analyze this task trajectory and identify any reusable patterns.",
          "",
          `Task: ${trajectory.taskDescription}`,
          `Outcome: ${trajectory.outcome}`,
          `Duration: ${trajectory.totalDurationMs}ms`,
          `Skills used: ${trajectory.skillsUsed.join(", ") || "none"}`,
          "",
          "Actions taken:",
          ...trajectory.actions.slice(0, 15).map((a) => {
            if (a.type === "tool_call") {
              return `  - ${a.tool}: ${a.content.slice(0, 80)}...`;
            }
            return `  - ${a.type}: ${a.content.slice(0, 80)}...`;
          }),
          "",
          "If this trajectory contains a reusable pattern, output as JSON:",
          "{ name, type, description, content, triggerContext, category }",
          "",
          "If no clear pattern, output: []",
        ].join("\n");

        const response = yield* fm.generate(prompt, {}).pipe(mapFMError);

        return parsePatternsFromResponse(response, [trajectory.id]);
      });

    const filterByQuality = (
      patterns: ExtractedPattern[],
      minConfidence: number,
      minOccurrences: number,
    ): Effect.Effect<ExtractedPattern[], never> =>
      Effect.succeed(
        patterns.filter((p) => p.confidence >= minConfidence && p.occurrences >= minOccurrences),
      );

    return {
      extractPatterns,
      extractQuickPatterns,
      analyzeTrajectory,
      filterByQuality,
    };
  });

// --- Layer ---

export const PatternExtractorLive: Layer.Layer<PatternExtractor, never, FMService> = Layer.effect(
  PatternExtractor,
  makePatternExtractor(DEFAULT_ARCHIVE_CONFIG),
);

export const makePatternExtractorLive = (
  config: Partial<ArchiveConfig> = {},
): Layer.Layer<PatternExtractor, never, FMService> =>
  Layer.effect(PatternExtractor, makePatternExtractor({ ...DEFAULT_ARCHIVE_CONFIG, ...config }));

/**
 * Create a complete PatternExtractor layer with FM dependency.
 */
export const makePatternExtractorWithFM = (
  config: Partial<ArchiveConfig> = {},
): Layer.Layer<PatternExtractor, never, never> => {
  const fmLayer = makeFMServiceLayer({ autoStart: false, enableLogging: false });
  return Layer.provide(makePatternExtractorLive(config), fmLayer);
};
