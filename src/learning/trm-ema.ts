/**
 * TRM EMA Stability
 *
 * Implements Exponential Moving Average for stable success rate tracking.
 * TRM paper shows EMA (decay=0.999) prevents overfitting and divergence:
 * - Without EMA: 79.9% accuracy, prone to collapse
 * - With EMA: 87.4% accuracy, stable training
 *
 * EMA smooths noisy feedback from individual task outcomes,
 * preventing wild swings from single successes/failures.
 */

import * as S from "effect/Schema";
import { Effect, Context, Layer } from "effect";

// --- EMA Configuration ---

export interface EMAConfig {
  /** Decay factor (0-1). Higher = more smoothing. Default: 0.999 */
  decay: number;

  /** Minimum samples before EMA is considered reliable */
  minSamples: number;

  /** Initial value for EMA (before any samples) */
  initialValue: number;
}

export const DEFAULT_EMA_CONFIG: EMAConfig = {
  decay: 0.999,
  minSamples: 5,
  initialValue: 0.5,
};

// --- EMA Value Schema ---

export const EMAValue = S.Struct({
  /** Current EMA value */
  value: S.Number,

  /** Number of samples incorporated */
  sampleCount: S.Number,

  /** Last update timestamp */
  updatedAt: S.String,

  /** Variance estimate (for confidence intervals) */
  variance: S.Number,

  /** Raw values buffer (last N for debugging) */
  recentValues: S.Array(S.Number),
});
export type EMAValue = S.Schema.Type<typeof EMAValue>;

// --- Task Type Stats ---

export const TaskTypeStats = S.Struct({
  /** Task type/category identifier */
  taskType: S.String,

  /** EMA success rate for this task type */
  successRate: EMAValue,

  /** EMA optimal depth for this task type */
  optimalDepth: EMAValue,

  /** EMA tokens used per attempt */
  tokensPerAttempt: EMAValue,

  /** EMA time per attempt (ms) */
  timePerAttempt: EMAValue,

  /** Total attempts on this task type */
  totalAttempts: S.Number,

  /** Total successes on this task type */
  totalSuccesses: S.Number,
});
export type TaskTypeStats = S.Schema.Type<typeof TaskTypeStats>;

// --- EMA Functions ---

/**
 * Create initial EMA value.
 */
export const createEMAValue = (initialValue: number = 0.5): EMAValue => ({
  value: initialValue,
  sampleCount: 0,
  updatedAt: new Date().toISOString(),
  variance: 0,
  recentValues: [],
});

/**
 * Update EMA with a new sample.
 *
 * EMA formula: ema_new = decay * ema_old + (1 - decay) * sample
 */
export const updateEMA = (
  ema: EMAValue,
  sample: number,
  config: EMAConfig = DEFAULT_EMA_CONFIG,
): EMAValue => {
  const { decay } = config;

  // Calculate new EMA value
  const newValue = decay * ema.value + (1 - decay) * sample;

  // Update variance estimate (Welford's online algorithm)
  const delta = sample - ema.value;
  const newVariance = decay * ema.variance + (1 - decay) * delta * delta;

  // Keep last 10 values for debugging
  const recentValues = [...ema.recentValues.slice(-9), sample];

  return {
    value: newValue,
    sampleCount: ema.sampleCount + 1,
    updatedAt: new Date().toISOString(),
    variance: newVariance,
    recentValues,
  };
};

/**
 * Check if EMA has enough samples to be reliable.
 */
export const isReliable = (ema: EMAValue, config: EMAConfig = DEFAULT_EMA_CONFIG): boolean =>
  ema.sampleCount >= config.minSamples;

/**
 * Get confidence interval for EMA value.
 * Returns [lower, upper] bounds at ~95% confidence.
 */
export const getConfidenceInterval = (ema: EMAValue): [number, number] => {
  if (ema.sampleCount < 2) {
    return [0, 1];
  }

  const stdDev = Math.sqrt(ema.variance);
  const margin = 1.96 * stdDev; // 95% CI

  return [Math.max(0, ema.value - margin), Math.min(1, ema.value + margin)];
};

/**
 * Create initial task type stats.
 */
export const createTaskTypeStats = (taskType: string): TaskTypeStats => ({
  taskType,
  successRate: createEMAValue(0.5),
  optimalDepth: createEMAValue(20), // Default mid-range depth
  tokensPerAttempt: createEMAValue(1000),
  timePerAttempt: createEMAValue(30000), // 30s default
  totalAttempts: 0,
  totalSuccesses: 0,
});

/**
 * Update task type stats with a new attempt result.
 */
export const updateTaskTypeStats = (
  stats: TaskTypeStats,
  result: {
    success: boolean;
    depth: number;
    tokensUsed: number;
    timeMs: number;
  },
  config: EMAConfig = DEFAULT_EMA_CONFIG,
): TaskTypeStats => ({
  ...stats,
  successRate: updateEMA(stats.successRate, result.success ? 1 : 0, config),
  optimalDepth: result.success
    ? updateEMA(stats.optimalDepth, result.depth, config)
    : stats.optimalDepth, // Only update depth on success
  tokensPerAttempt: updateEMA(stats.tokensPerAttempt, result.tokensUsed, config),
  timePerAttempt: updateEMA(stats.timePerAttempt, result.timeMs, config),
  totalAttempts: stats.totalAttempts + 1,
  totalSuccesses: stats.totalSuccesses + (result.success ? 1 : 0),
});

// --- Skill EMA Extension ---

/**
 * EMA-enhanced skill tracking for unified skill library.
 */
export interface SkillEMAStats {
  /** Skill ID */
  skillId: string;

  /** EMA success rate when used for initial sampling */
  samplingSuccessRate: EMAValue;

  /** EMA success rate when used for refinement */
  refinementSuccessRate: EMAValue;

  /** Overall confidence (average of sampling + refinement) */
  jointConfidence: number;

  /** Last updated */
  updatedAt: string;
}

/**
 * Create initial skill EMA stats.
 */
export const createSkillEMAStats = (skillId: string): SkillEMAStats => ({
  skillId,
  samplingSuccessRate: createEMAValue(0.5),
  refinementSuccessRate: createEMAValue(0.5),
  jointConfidence: 0.5,
  updatedAt: new Date().toISOString(),
});

/**
 * Update skill EMA stats.
 */
export const updateSkillEMAStats = (
  stats: SkillEMAStats,
  context: "sampling" | "refinement",
  success: boolean,
  config: EMAConfig = DEFAULT_EMA_CONFIG,
): SkillEMAStats => {
  const newStats = { ...stats, updatedAt: new Date().toISOString() };

  if (context === "sampling") {
    newStats.samplingSuccessRate = updateEMA(stats.samplingSuccessRate, success ? 1 : 0, config);
  } else {
    newStats.refinementSuccessRate = updateEMA(
      stats.refinementSuccessRate,
      success ? 1 : 0,
      config,
    );
  }

  // Update joint confidence
  newStats.jointConfidence =
    (newStats.samplingSuccessRate.value + newStats.refinementSuccessRate.value) / 2;

  return newStats;
};

// --- Service Interface ---

export interface ITRMEMAService {
  /** Create a new EMA value */
  readonly createEMA: (initialValue?: number) => Effect.Effect<EMAValue, never>;

  /** Update EMA with a new sample */
  readonly updateEMA: (ema: EMAValue, sample: number) => Effect.Effect<EMAValue, never>;

  /** Check if EMA is reliable */
  readonly isReliable: (ema: EMAValue) => Effect.Effect<boolean, never>;

  /** Get confidence interval */
  readonly getConfidenceInterval: (ema: EMAValue) => Effect.Effect<[number, number], never>;

  /** Create task type stats */
  readonly createTaskStats: (taskType: string) => Effect.Effect<TaskTypeStats, never>;

  /** Update task type stats */
  readonly updateTaskStats: (
    stats: TaskTypeStats,
    result: { success: boolean; depth: number; tokensUsed: number; timeMs: number },
  ) => Effect.Effect<TaskTypeStats, never>;

  /** Create skill EMA stats */
  readonly createSkillStats: (skillId: string) => Effect.Effect<SkillEMAStats, never>;

  /** Update skill EMA stats */
  readonly updateSkillStats: (
    stats: SkillEMAStats,
    context: "sampling" | "refinement",
    success: boolean,
  ) => Effect.Effect<SkillEMAStats, never>;

  /** Get current config */
  readonly getConfig: () => Effect.Effect<EMAConfig, never>;

  /** Update config */
  readonly updateConfig: (updates: Partial<EMAConfig>) => Effect.Effect<EMAConfig, never>;
}

// --- Service Tag ---

export class TRMEMAService extends Context.Tag("TRMEMAService")<TRMEMAService, ITRMEMAService>() {}

// --- Service Implementation ---

const makeTRMEMAService = (initialConfig: EMAConfig = DEFAULT_EMA_CONFIG): ITRMEMAService => {
  let config = { ...initialConfig };

  return {
    createEMA: (initialValue = config.initialValue) =>
      Effect.sync(() => createEMAValue(initialValue)),

    updateEMA: (ema, sample) => Effect.sync(() => updateEMA(ema, sample, config)),

    isReliable: (ema) => Effect.sync(() => isReliable(ema, config)),

    getConfidenceInterval: (ema) => Effect.sync(() => getConfidenceInterval(ema)),

    createTaskStats: (taskType) => Effect.sync(() => createTaskTypeStats(taskType)),

    updateTaskStats: (stats, result) => Effect.sync(() => updateTaskTypeStats(stats, result, config)),

    createSkillStats: (skillId) => Effect.sync(() => createSkillEMAStats(skillId)),

    updateSkillStats: (stats, context, success) =>
      Effect.sync(() => updateSkillEMAStats(stats, context, success, config)),

    getConfig: () => Effect.sync(() => ({ ...config })),

    updateConfig: (updates) =>
      Effect.sync(() => {
        config = { ...config, ...updates };
        return { ...config };
      }),
  };
};

// --- Layer ---

export const TRMEMAServiceLive: Layer.Layer<TRMEMAService, never, never> = Layer.succeed(
  TRMEMAService,
  makeTRMEMAService(),
);

/**
 * Create a TRMEMAService layer with custom config.
 */
export const makeTRMEMAServiceLayer = (
  config: Partial<EMAConfig> = {},
): Layer.Layer<TRMEMAService, never, never> =>
  Layer.succeed(TRMEMAService, makeTRMEMAService({ ...DEFAULT_EMA_CONFIG, ...config }));
