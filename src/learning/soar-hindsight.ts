/**
 * SOAR Hindsight Relabeler
 *
 * Implements SOAR's key innovation: hindsight relabeling.
 * When an agent fails at task T, the attempt is correct for some synthetic task T'.
 *
 * Example:
 *   Original: "Write function returning fibonacci(10)" → Agent outputs factorial(10)
 *   Result: FAILURE for original task
 *   Hindsight: "Write function returning factorial(10)" → Same code is CORRECT
 *
 * This transforms 400 tasks × 6k programs = 2.4M training examples.
 * Every failed Gym attempt becomes a valid skill pattern for *something*.
 */

import * as S from "effect/Schema";
import { Effect, Context, Layer, Option } from "effect";

// --- Attempt Record ---

export const AttemptRecord = S.Struct({
  /** Attempt identifier */
  id: S.String,

  /** Original task ID */
  taskId: S.String,

  /** Original task description */
  taskDescription: S.String,

  /** Code/solution produced */
  code: S.String,

  /** Actual output produced */
  actualOutput: S.Unknown,

  /** Expected output (if known) */
  expectedOutput: S.optional(S.Unknown),

  /** Whether the attempt succeeded */
  success: S.Boolean,

  /** Training accuracy (fraction of examples passing) */
  trainingAccuracy: S.Number,

  /** Tokens used */
  tokensUsed: S.Number,

  /** Duration in ms */
  durationMs: S.Number,

  /** Timestamp */
  timestamp: S.String,

  /** Skills used in this attempt */
  skillsUsed: S.optional(S.Array(S.String)),
});
export type AttemptRecord = S.Schema.Type<typeof AttemptRecord>;

// --- Synthetic Task ---

export const SyntheticTask = S.Struct({
  /** Synthetic task identifier */
  id: S.String,

  /** Synthetic task description (what the output DOES) */
  description: S.String,

  /** The input that produces this output */
  input: S.Unknown,

  /** The output (which is "correct" by construction) */
  output: S.Unknown,

  /** Original task this was derived from */
  originalTaskId: S.String,

  /** The attempt that generated this */
  attemptId: S.String,

  /** Confidence in synthetic task validity (0-1) */
  confidence: S.Number,

  /** Whether this passed structural validation */
  validated: S.Boolean,

  /** Creation timestamp */
  createdAt: S.String,
});
export type SyntheticTask = S.Schema.Type<typeof SyntheticTask>;

// --- Synthetic Task-Solution Pair ---

export const SyntheticTaskSolution = S.Struct({
  /** The synthetic task */
  task: SyntheticTask,

  /** The solution code (same as original attempt) */
  solution: S.String,

  /** Source marker */
  source: S.Literal("hindsight"),

  /** Quality score based on structural validity */
  qualityScore: S.Number,
});
export type SyntheticTaskSolution = S.Schema.Type<typeof SyntheticTaskSolution>;

// --- Hindsight Configuration ---

export interface HindsightConfig {
  /** Minimum training accuracy for relabeling (avoid complete failures) */
  minTrainingAccuracy: number;

  /** Maximum training accuracy (don't relabel near-successes) */
  maxTrainingAccuracy: number;

  /** Minimum code length to consider */
  minCodeLength: number;

  /** Maximum synthetic tasks per original task */
  maxSyntheticPerTask: number;
}

export const DEFAULT_HINDSIGHT_CONFIG: HindsightConfig = {
  minTrainingAccuracy: 0.01, // At least 1% of examples pass
  maxTrainingAccuracy: 0.99, // Not already correct
  minCodeLength: 10, // Trivial code excluded
  maxSyntheticPerTask: 50, // Cap per task
};

// --- Relabeling Functions ---

/**
 * Generate a synthetic task description from an attempt.
 * The description should capture what the code actually does.
 */
export const generateSyntheticDescription = (
  originalDescription: string,
  actualOutput: unknown,
): string => {
  const outputStr =
    typeof actualOutput === "string"
      ? actualOutput.slice(0, 100)
      : JSON.stringify(actualOutput).slice(0, 100);

  // Create a description that matches the actual behavior
  return `[Hindsight] Given the same input as "${originalDescription.slice(0, 50)}...", produce output: ${outputStr}`;
};

/**
 * Check if an attempt is suitable for hindsight relabeling.
 */
export const isSuitableForRelabeling = (
  attempt: AttemptRecord,
  config: HindsightConfig = DEFAULT_HINDSIGHT_CONFIG,
): boolean => {
  // Must be a failure (otherwise use direct learning)
  if (attempt.success) return false;

  // Must have some training accuracy (not complete garbage)
  if (attempt.trainingAccuracy < config.minTrainingAccuracy) return false;

  // Must not be nearly correct (use direct learning for those)
  if (attempt.trainingAccuracy > config.maxTrainingAccuracy) return false;

  // Must have meaningful code
  if (attempt.code.length < config.minCodeLength) return false;

  // Must have an actual output to relabel
  if (attempt.actualOutput === undefined || attempt.actualOutput === null) return false;

  return true;
};

/**
 * Create a synthetic task from a failed attempt.
 */
export const createSyntheticTask = (attempt: AttemptRecord): SyntheticTask => {
  const now = new Date().toISOString();

  return {
    id: `synthetic-${attempt.id}`,
    description: generateSyntheticDescription(attempt.taskDescription, attempt.actualOutput),
    input: {}, // Would be populated from original task
    output: attempt.actualOutput,
    originalTaskId: attempt.taskId,
    attemptId: attempt.id,
    confidence: attempt.trainingAccuracy, // Higher accuracy = more confident
    validated: false, // Needs structural validation
    createdAt: now,
  };
};

/**
 * Create a synthetic task-solution pair from an attempt.
 */
export const relabelAttempt = (
  attempt: AttemptRecord,
  config: HindsightConfig = DEFAULT_HINDSIGHT_CONFIG,
): Option.Option<SyntheticTaskSolution> => {
  if (!isSuitableForRelabeling(attempt, config)) {
    return Option.none();
  }

  const task = createSyntheticTask(attempt);

  return Option.some({
    task,
    solution: attempt.code,
    source: "hindsight" as const,
    qualityScore: attempt.trainingAccuracy,
  });
};

/**
 * Relabel a batch of attempts.
 */
export const relabelBatch = (
  attempts: AttemptRecord[],
  config: HindsightConfig = DEFAULT_HINDSIGHT_CONFIG,
): SyntheticTaskSolution[] => {
  // Group by original task
  const byTask = new Map<string, AttemptRecord[]>();
  for (const attempt of attempts) {
    const existing = byTask.get(attempt.taskId) ?? [];
    byTask.set(attempt.taskId, [...existing, attempt]);
  }

  const results: SyntheticTaskSolution[] = [];

  for (const [_taskId, taskAttempts] of Array.from(byTask)) {
    // Sort by training accuracy (prefer higher accuracy attempts)
    const sorted = [...taskAttempts].sort((a, b) => b.trainingAccuracy - a.trainingAccuracy);

    // Take up to maxSyntheticPerTask
    const toRelabel = sorted.slice(0, config.maxSyntheticPerTask);

    for (const attempt of toRelabel) {
      const result = relabelAttempt(attempt, config);
      if (Option.isSome(result)) {
        results.push(result.value);
      }
    }
  }

  return results;
};

// --- Service Interface ---

export interface IHindsightService {
  /** Check if an attempt is suitable for relabeling */
  readonly isSuitable: (attempt: AttemptRecord) => Effect.Effect<boolean, never>;

  /** Relabel a single attempt */
  readonly relabel: (
    attempt: AttemptRecord,
  ) => Effect.Effect<Option.Option<SyntheticTaskSolution>, never>;

  /** Relabel a batch of attempts */
  readonly relabelBatch: (attempts: AttemptRecord[]) => Effect.Effect<SyntheticTaskSolution[], never>;

  /** Create a synthetic task from an attempt */
  readonly createSyntheticTask: (attempt: AttemptRecord) => Effect.Effect<SyntheticTask, never>;

  /** Get current config */
  readonly getConfig: () => Effect.Effect<HindsightConfig, never>;

  /** Update config */
  readonly updateConfig: (updates: Partial<HindsightConfig>) => Effect.Effect<HindsightConfig, never>;

  /** Get statistics about relabeling */
  readonly getStats: () => Effect.Effect<HindsightStats, never>;
}

export interface HindsightStats {
  totalAttemptsProcessed: number;
  totalSyntheticCreated: number;
  relabelingRate: number;
  averageQualityScore: number;
}

// --- Service Tag ---

export class HindsightService extends Context.Tag("HindsightService")<
  HindsightService,
  IHindsightService
>() {}

// --- Service Implementation ---

const makeHindsightService = (
  initialConfig: HindsightConfig = DEFAULT_HINDSIGHT_CONFIG,
): IHindsightService => {
  let config = { ...initialConfig };
  let stats: HindsightStats = {
    totalAttemptsProcessed: 0,
    totalSyntheticCreated: 0,
    relabelingRate: 0,
    averageQualityScore: 0,
  };

  const updateStats = (processed: number, created: number, qualitySum: number): void => {
    stats.totalAttemptsProcessed += processed;
    stats.totalSyntheticCreated += created;
    stats.relabelingRate =
      stats.totalAttemptsProcessed > 0
        ? stats.totalSyntheticCreated / stats.totalAttemptsProcessed
        : 0;

    if (created > 0) {
      const prevTotal = stats.totalSyntheticCreated - created;
      const prevSum = stats.averageQualityScore * prevTotal;
      stats.averageQualityScore = (prevSum + qualitySum) / stats.totalSyntheticCreated;
    }
  };

  return {
    isSuitable: (attempt) => Effect.sync(() => isSuitableForRelabeling(attempt, config)),

    relabel: (attempt) =>
      Effect.sync(() => {
        const result = relabelAttempt(attempt, config);
        if (Option.isSome(result)) {
          updateStats(1, 1, result.value.qualityScore);
        } else {
          updateStats(1, 0, 0);
        }
        return result;
      }),

    relabelBatch: (attempts) =>
      Effect.sync(() => {
        const results = relabelBatch(attempts, config);
        const qualitySum = results.reduce((sum, r) => sum + r.qualityScore, 0);
        updateStats(attempts.length, results.length, qualitySum);
        return results;
      }),

    createSyntheticTask: (attempt) => Effect.sync(() => createSyntheticTask(attempt)),

    getConfig: () => Effect.sync(() => ({ ...config })),

    updateConfig: (updates) =>
      Effect.sync(() => {
        config = { ...config, ...updates };
        return { ...config };
      }),

    getStats: () => Effect.sync(() => ({ ...stats })),
  };
};

// --- Layer ---

export const HindsightServiceLive: Layer.Layer<HindsightService, never, never> = Layer.succeed(
  HindsightService,
  makeHindsightService(),
);

/**
 * Create a HindsightService layer with custom config.
 */
export const makeHindsightServiceLayer = (
  config: Partial<HindsightConfig> = {},
): Layer.Layer<HindsightService, never, never> =>
  Layer.succeed(
    HindsightService,
    makeHindsightService({ ...DEFAULT_HINDSIGHT_CONFIG, ...config }),
  );
