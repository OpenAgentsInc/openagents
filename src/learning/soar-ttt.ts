/**
 * SOAR Test-Time Training (TTT)
 *
 * Implements SOAR's test-time training for improving on target tasks
 * WITHOUT ground truth labels. This is the critical innovation for Terminal-Bench.
 *
 * TTT Process:
 * 1. Generate multiple solution attempts for target task
 * 2. Use training accuracy (on visible examples) as quality proxy
 * 3. Apply hindsight relabeling to create synthetic task-solution pairs
 * 4. Fine-tune on synthetic data
 * 5. Re-generate solutions using improved model
 * 6. Use voting to select final prediction
 *
 * Key insight: Training accuracy on visible I/O examples strongly correlates
 * with correctness on held-out test cases (r ≈ 0.9 in SOAR paper).
 */

import * as S from "effect/Schema";
import { Effect, Context, Layer } from "effect";
import type { AttemptRecord, SyntheticTaskSolution } from "./soar-hindsight.js";
import type { VotingResult } from "./soar-voting.js";

// --- TTT Configuration ---

export interface TTTConfig {
  /** Number of solution attempts per iteration */
  attemptsPerIteration: number;

  /** Maximum TTT iterations */
  maxIterations: number;

  /** Minimum accuracy improvement to continue */
  minImprovementThreshold: number;

  /** Training accuracy threshold for "good enough" */
  satisfactionThreshold: number;

  /** Enable synthetic data generation */
  enableSyntheticData: boolean;

  /** Enable voting for final prediction */
  enableVoting: boolean;

  /** Temperature for generation diversity */
  generationTemperature: number;

  /** Maximum tokens per attempt */
  maxTokensPerAttempt: number;
}

export const DEFAULT_TTT_CONFIG: TTTConfig = {
  attemptsPerIteration: 50,
  maxIterations: 5,
  minImprovementThreshold: 0.01, // 1% improvement
  satisfactionThreshold: 1.0, // 100% training accuracy
  enableSyntheticData: true,
  enableVoting: true,
  generationTemperature: 0.7,
  maxTokensPerAttempt: 4096,
};

// --- TTT Iteration Result ---

export const TTTIterationResult = S.Struct({
  /** Iteration number (0-indexed) */
  iteration: S.Number,

  /** Best training accuracy this iteration */
  bestAccuracy: S.Number,

  /** Average accuracy across attempts */
  averageAccuracy: S.Number,

  /** Number of attempts made */
  attemptCount: S.Number,

  /** Synthetic pairs generated this iteration */
  syntheticCount: S.Number,

  /** Best solution code this iteration */
  bestSolution: S.String,

  /** Whether improvement was detected */
  improved: S.Boolean,

  /** Timestamp */
  completedAt: S.String,
});
export type TTTIterationResult = S.Schema.Type<typeof TTTIterationResult>;

// --- TTT Session Result ---

export const TTTSessionResult = S.Struct({
  /** Task ID being solved */
  taskId: S.String,

  /** Final prediction */
  finalPrediction: S.Unknown,

  /** Final solution code */
  finalSolution: S.String,

  /** Best training accuracy achieved */
  bestAccuracy: S.Number,

  /** Total iterations completed */
  iterationsCompleted: S.Number,

  /** Why TTT stopped */
  stopReason: S.Union(
    S.Literal("satisfied"),
    S.Literal("max_iterations"),
    S.Literal("no_improvement"),
    S.Literal("error"),
  ),

  /** Iteration results */
  iterations: S.Array(TTTIterationResult),

  /** Voting result if enabled */
  votingResult: S.optional(S.Unknown), // VotingResult

  /** Total synthetic pairs generated */
  totalSyntheticPairs: S.Number,

  /** Total tokens used */
  totalTokensUsed: S.Number,

  /** Session duration (ms) */
  durationMs: S.Number,

  /** Session timestamps */
  startedAt: S.String,
  completedAt: S.String,
});
export type TTTSessionResult = S.Schema.Type<typeof TTTSessionResult>;

// --- TTT State ---

export interface TTTState {
  /** Current iteration */
  currentIteration: number;

  /** Best solution so far */
  bestSolution: string;

  /** Best accuracy so far */
  bestAccuracy: number;

  /** All attempts across iterations */
  allAttempts: AttemptRecord[];

  /** Synthetic pairs for training */
  syntheticPairs: SyntheticTaskSolution[];

  /** Iteration history */
  iterationHistory: TTTIterationResult[];

  /** Total tokens used */
  tokensUsed: number;

  /** Start time */
  startTime: number;
}

/**
 * Create initial TTT state.
 */
export const createTTTState = (): TTTState => ({
  currentIteration: 0,
  bestSolution: "",
  bestAccuracy: 0,
  allAttempts: [],
  syntheticPairs: [],
  iterationHistory: [],
  tokensUsed: 0,
  startTime: Date.now(),
});

// --- TTT Core Functions ---

/**
 * Check if TTT should continue.
 */
export const shouldContinueTTT = (state: TTTState, config: TTTConfig): boolean => {
  // Check satisfaction threshold
  if (state.bestAccuracy >= config.satisfactionThreshold) {
    return false;
  }

  // Check max iterations
  if (state.currentIteration >= config.maxIterations) {
    return false;
  }

  // Check improvement (skip on first iteration)
  if (state.iterationHistory.length >= 2) {
    const lastTwo = state.iterationHistory.slice(-2);
    const improvement = lastTwo[1]!.bestAccuracy - lastTwo[0]!.bestAccuracy;
    if (improvement < config.minImprovementThreshold) {
      return false;
    }
  }

  return true;
};

/**
 * Determine stop reason.
 */
export const getStopReason = (
  state: TTTState,
  config: TTTConfig,
): TTTSessionResult["stopReason"] => {
  if (state.bestAccuracy >= config.satisfactionThreshold) {
    return "satisfied";
  }

  if (state.currentIteration >= config.maxIterations) {
    return "max_iterations";
  }

  if (state.iterationHistory.length >= 2) {
    const lastTwo = state.iterationHistory.slice(-2);
    const improvement = lastTwo[1]!.bestAccuracy - lastTwo[0]!.bestAccuracy;
    if (improvement < config.minImprovementThreshold) {
      return "no_improvement";
    }
  }

  return "max_iterations";
};

/**
 * Process iteration results.
 */
export const processIteration = (
  state: TTTState,
  attempts: AttemptRecord[],
  syntheticPairs: SyntheticTaskSolution[],
): { state: TTTState; result: TTTIterationResult } => {
  // Find best attempt
  let bestAttempt = attempts[0];
  for (const attempt of attempts) {
    if (!bestAttempt || attempt.trainingAccuracy > bestAttempt.trainingAccuracy) {
      bestAttempt = attempt;
    }
  }

  // Calculate average
  const avgAccuracy =
    attempts.length > 0 ? attempts.reduce((sum, a) => sum + a.trainingAccuracy, 0) / attempts.length : 0;

  // Check if improved
  const improved = bestAttempt ? bestAttempt.trainingAccuracy > state.bestAccuracy : false;

  const result: TTTIterationResult = {
    iteration: state.currentIteration,
    bestAccuracy: bestAttempt?.trainingAccuracy ?? 0,
    averageAccuracy: avgAccuracy,
    attemptCount: attempts.length,
    syntheticCount: syntheticPairs.length,
    bestSolution: bestAttempt?.code ?? "",
    improved,
    completedAt: new Date().toISOString(),
  };

  // Update state
  const newState: TTTState = {
    ...state,
    currentIteration: state.currentIteration + 1,
    bestSolution: improved ? (bestAttempt?.code ?? state.bestSolution) : state.bestSolution,
    bestAccuracy: improved ? (bestAttempt?.trainingAccuracy ?? state.bestAccuracy) : state.bestAccuracy,
    allAttempts: [...state.allAttempts, ...attempts],
    syntheticPairs: [...state.syntheticPairs, ...syntheticPairs],
    iterationHistory: [...state.iterationHistory, result],
    tokensUsed: state.tokensUsed + attempts.reduce((sum, a) => sum + a.tokensUsed, 0),
  };

  return { state: newState, result };
};

/**
 * Create final TTT session result.
 */
export const createSessionResult = (
  taskId: string,
  state: TTTState,
  config: TTTConfig,
  votingResult?: VotingResult,
): TTTSessionResult => {
  const finalPrediction = votingResult?.winner ?? null;
  const stopReason = getStopReason(state, config);

  return {
    taskId,
    finalPrediction,
    finalSolution: state.bestSolution,
    bestAccuracy: state.bestAccuracy,
    iterationsCompleted: state.currentIteration,
    stopReason,
    iterations: state.iterationHistory,
    votingResult: votingResult as unknown,
    totalSyntheticPairs: state.syntheticPairs.length,
    totalTokensUsed: state.tokensUsed,
    durationMs: Date.now() - state.startTime,
    startedAt: new Date(state.startTime).toISOString(),
    completedAt: new Date().toISOString(),
  };
};

// --- Training Accuracy Estimation ---

/**
 * Estimate training accuracy for a solution on visible examples.
 * This is the key proxy metric for TTT.
 */
export interface TrainingAccuracyEstimate {
  /** Fraction of training examples passing (0-1) */
  accuracy: number;

  /** Number of examples tested */
  examplesTested: number;

  /** Number of examples passed */
  examplesPassed: number;

  /** Individual example results */
  exampleResults: Array<{
    input: unknown;
    expectedOutput: unknown;
    actualOutput: unknown;
    passed: boolean;
  }>;
}

/**
 * Compare outputs for equality.
 * Handles common output formats.
 */
export const outputsEqual = (expected: unknown, actual: unknown): boolean => {
  // Handle null/undefined
  if (expected === null && actual === null) return true;
  if (expected === undefined && actual === undefined) return true;
  if (expected === null || actual === null) return false;
  if (expected === undefined || actual === undefined) return false;

  // Handle primitives
  if (typeof expected !== typeof actual) return false;
  if (typeof expected !== "object") return expected === actual;

  // Handle arrays
  if (Array.isArray(expected)) {
    if (!Array.isArray(actual)) return false;
    if (expected.length !== actual.length) return false;
    return expected.every((e, i) => outputsEqual(e, actual[i]));
  }

  // Handle objects
  const expectedKeys = Object.keys(expected as Record<string, unknown>).sort();
  const actualKeys = Object.keys(actual as Record<string, unknown>).sort();
  if (expectedKeys.length !== actualKeys.length) return false;
  if (expectedKeys.some((k, i) => k !== actualKeys[i])) return false;

  return expectedKeys.every((k) =>
    outputsEqual(
      (expected as Record<string, unknown>)[k],
      (actual as Record<string, unknown>)[k],
    ),
  );
};

// --- Skill Integration ---

/**
 * TTT-enhanced skill context.
 * Tracks skill performance during TTT for adaptive sampling.
 */
export interface TTTSkillContext {
  /** Skill ID */
  skillId: string;

  /** Success rate in current TTT session */
  sessionSuccessRate: number;

  /** Number of uses in session */
  sessionUses: number;

  /** Best accuracy achieved with this skill */
  bestAccuracyWithSkill: number;

  /** Should boost this skill's sampling probability? */
  shouldBoost: boolean;
}

/**
 * Update skill context based on attempt result.
 */
export const updateSkillContext = (
  context: TTTSkillContext,
  attempt: AttemptRecord,
): TTTSkillContext => {
  const newUses = context.sessionUses + 1;
  const newSuccessRate =
    (context.sessionSuccessRate * context.sessionUses + (attempt.success ? 1 : 0)) / newUses;
  const newBestAccuracy = Math.max(context.bestAccuracyWithSkill, attempt.trainingAccuracy);

  return {
    ...context,
    sessionSuccessRate: newSuccessRate,
    sessionUses: newUses,
    bestAccuracyWithSkill: newBestAccuracy,
    shouldBoost: newSuccessRate > 0.2 || newBestAccuracy > 0.5, // Heuristic
  };
};

// --- Service Interface ---

export interface ITTTService {
  /** Run TTT on a task */
  readonly runTTT: (
    taskId: string,
    description: string,
    trainingExamples: Array<{ input: unknown; output: unknown }>,
    generateAttempts: (
      description: string,
      examples: Array<{ input: unknown; output: unknown }>,
      count: number,
    ) => Effect.Effect<AttemptRecord[], Error>,
    executeAndValidate: (
      code: string,
      examples: Array<{ input: unknown; output: unknown }>,
    ) => Effect.Effect<TrainingAccuracyEstimate, Error>,
  ) => Effect.Effect<TTTSessionResult, Error>;

  /** Check if TTT should continue */
  readonly shouldContinue: (state: TTTState) => Effect.Effect<boolean, never>;

  /** Process iteration results */
  readonly processIteration: (
    state: TTTState,
    attempts: AttemptRecord[],
    syntheticPairs: SyntheticTaskSolution[],
  ) => Effect.Effect<{ state: TTTState; result: TTTIterationResult }, never>;

  /** Get current config */
  readonly getConfig: () => Effect.Effect<TTTConfig, never>;

  /** Update config */
  readonly updateConfig: (updates: Partial<TTTConfig>) => Effect.Effect<TTTConfig, never>;

  /** Get TTT statistics */
  readonly getStats: () => Effect.Effect<TTTStats, never>;
}

export interface TTTStats {
  totalSessions: number;
  totalIterations: number;
  averageIterationsPerSession: number;
  satisfactionRate: number;
  averageBestAccuracy: number;
  totalSyntheticPairs: number;
  totalTokensUsed: number;
}

// --- Service Tag ---

export class TTTService extends Context.Tag("TTTService")<TTTService, ITTTService>() {}

// --- Service Implementation ---

const makeTTTService = (initialConfig: TTTConfig = DEFAULT_TTT_CONFIG): ITTTService => {
  let config = { ...initialConfig };
  let stats: TTTStats = {
    totalSessions: 0,
    totalIterations: 0,
    averageIterationsPerSession: 0,
    satisfactionRate: 0,
    averageBestAccuracy: 0,
    totalSyntheticPairs: 0,
    totalTokensUsed: 0,
  };
  let satisfiedCount = 0;

  const updateStats = (result: TTTSessionResult): void => {
    stats.totalSessions++;
    stats.totalIterations += result.iterationsCompleted;
    stats.averageIterationsPerSession = stats.totalIterations / stats.totalSessions;

    if (result.stopReason === "satisfied") satisfiedCount++;
    stats.satisfactionRate = satisfiedCount / stats.totalSessions;

    const prevAccSum = stats.averageBestAccuracy * (stats.totalSessions - 1);
    stats.averageBestAccuracy = (prevAccSum + result.bestAccuracy) / stats.totalSessions;

    stats.totalSyntheticPairs += result.totalSyntheticPairs;
    stats.totalTokensUsed += result.totalTokensUsed;
  };

  return {
    runTTT: (taskId, description, trainingExamples, generateAttempts, executeAndValidate) =>
      Effect.gen(function* () {
        let state = createTTTState();

        // Main TTT loop
        while (shouldContinueTTT(state, config)) {
          // Generate attempts
          const attempts = yield* generateAttempts(
            description,
            trainingExamples,
            config.attemptsPerIteration,
          ).pipe(
            Effect.catchAll(() => Effect.succeed([] as AttemptRecord[])),
          );

          // Validate attempts and calculate training accuracy
          // Create mutable copies for updating
          const mutableAttempts: AttemptRecord[] = [];
          for (const attempt of attempts) {
            const estimate = yield* executeAndValidate(attempt.code, trainingExamples).pipe(
              Effect.catchAll(() =>
                Effect.succeed({
                  accuracy: 0,
                  examplesTested: trainingExamples.length,
                  examplesPassed: 0,
                  exampleResults: [],
                } as TrainingAccuracyEstimate),
              ),
            );
            mutableAttempts.push({
              ...attempt,
              trainingAccuracy: estimate.accuracy,
              success: estimate.accuracy >= config.satisfactionThreshold,
            } as AttemptRecord);
          }
          const validatedAttempts = mutableAttempts;

          // Generate synthetic pairs via hindsight relabeling (simplified inline)
          const syntheticPairs: SyntheticTaskSolution[] = [];
          if (config.enableSyntheticData) {
            for (const attempt of validatedAttempts) {
              if (!attempt.success && attempt.trainingAccuracy > 0.01) {
                syntheticPairs.push({
                  task: {
                    id: `synthetic-${attempt.id}`,
                    description: `[TTT] Produce output matching actual behavior`,
                    input: {},
                    output: attempt.actualOutput,
                    originalTaskId: taskId,
                    attemptId: attempt.id,
                    confidence: attempt.trainingAccuracy,
                    validated: false,
                    createdAt: new Date().toISOString(),
                  },
                  solution: attempt.code,
                  source: "hindsight" as const,
                  qualityScore: attempt.trainingAccuracy,
                });
              }
            }
          }

          // Process iteration
          const processed = processIteration(state, validatedAttempts, syntheticPairs);
          state = processed.state;
        }

        // Voting for final prediction (simplified)
        let votingResult: VotingResult | undefined;
        if (config.enableVoting && state.allAttempts.length > 0) {
          // Group by output and vote
          const outputs = state.allAttempts
            .filter((a) => a.trainingAccuracy > 0)
            .map((a) => ({
              output: a.actualOutput,
              program: a.code,
              trainingAccuracy: a.trainingAccuracy,
            }));

          if (outputs.length > 0) {
            // Simplified voting inline
            const groups = new Map<string, { output: unknown; weight: number; count: number }>();
            for (const o of outputs) {
              const key = JSON.stringify(o.output);
              const existing = groups.get(key);
              const weight = 1 + 1000 * o.trainingAccuracy;
              if (existing) {
                groups.set(key, {
                  output: existing.output,
                  weight: existing.weight + weight,
                  count: existing.count + 1,
                });
              } else {
                groups.set(key, { output: o.output, weight, count: 1 });
              }
            }

            let winner = { key: "", output: null as unknown, weight: 0 };
            for (const [key, g] of Array.from(groups)) {
              if (g.weight > winner.weight) {
                winner = { key, output: g.output, weight: g.weight };
              }
            }

            const totalWeight = Array.from(groups.values()).reduce((s, g) => s + g.weight, 0);
            votingResult = {
              winner: winner.output,
              winnerKey: winner.key,
              winnerWeight: winner.weight,
              confidence: totalWeight > 0 ? winner.weight / totalWeight : 0,
              candidates: Array.from(groups.entries()).map(([k, g]) => ({
                outputKey: k,
                output: g.output,
                weight: g.weight,
                voteCount: g.count,
                averageAccuracy: 0,
              })),
              totalVotes: outputs.length,
              isValid: true,
              votedAt: new Date().toISOString(),
            };
          }
        }

        const sessionResult = createSessionResult(taskId, state, config, votingResult);
        updateStats(sessionResult);
        return sessionResult;
      }),

    shouldContinue: (state) => Effect.sync(() => shouldContinueTTT(state, config)),

    processIteration: (state, attempts, syntheticPairs) =>
      Effect.sync(() => processIteration(state, attempts, syntheticPairs)),

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

export const TTTServiceLive: Layer.Layer<TTTService, never, never> = Layer.succeed(
  TTTService,
  makeTTTService(),
);

/**
 * Create a TTTService layer with custom config.
 */
export const makeTTTServiceLayer = (
  config: Partial<TTTConfig> = {},
): Layer.Layer<TTTService, never, never> =>
  Layer.succeed(TTTService, makeTTTService({ ...DEFAULT_TTT_CONFIG, ...config }));
