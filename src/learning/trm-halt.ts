/**
 * TRM Simple Halt Decision
 *
 * Implements TRM's simple binary halt decision, replacing HRM's complex Q-learning ACT.
 * TRM paper shows simple rules work just as well (87.4% vs 86.1%) with half the compute.
 *
 * Halt conditions:
 * - Max steps reached
 * - Tests passed (validation success)
 * - High confidence (> threshold)
 * - Stuck (same error 3+ times)
 *
 * No Q-learning, no extra forward passes, just straightforward rules.
 */

import { Effect, Context, Layer } from "effect";
import type { TRMState } from "./trm-state.js";

// --- Halt Configuration ---

export interface HaltConfig {
  /** Confidence threshold for early halt (default: 0.95) */
  confidenceThreshold: number;

  /** Training accuracy threshold for halt (default: 1.0) */
  accuracyThreshold: number;

  /** Number of repeated errors before forcing halt (default: 3) */
  maxStuckCount: number;

  /** Maximum depth before forcing halt (default: from state) */
  maxDepthOverride?: number;

  /** Minimum steps before allowing early halt (default: 3) */
  minStepsBeforeHalt: number;
}

export const DEFAULT_HALT_CONFIG: HaltConfig = {
  confidenceThreshold: 0.95,
  accuracyThreshold: 1.0,
  maxStuckCount: 3,
  minStepsBeforeHalt: 3,
};

// --- Halt Decision ---

export type HaltReason =
  | "max_depth"
  | "tests_passed"
  | "high_confidence"
  | "stuck"
  | "accuracy_achieved"
  | "continue";

export interface HaltDecision {
  shouldHalt: boolean;
  reason: HaltReason;
  confidence: number;
  depth: number;
  maxDepth: number;
  details?: string;
}

// --- Halt Functions ---

/**
 * Check if max depth reached.
 */
export const checkMaxDepth = (state: TRMState, config: HaltConfig): HaltDecision | null => {
  const maxDepth = config.maxDepthOverride ?? state.z.maxDepth;
  if (state.z.depth >= maxDepth) {
    return {
      shouldHalt: true,
      reason: "max_depth",
      confidence: state.y.confidence,
      depth: state.z.depth,
      maxDepth,
      details: `Reached maximum depth ${maxDepth}`,
    };
  }
  return null;
};

/**
 * Check if tests passed (validation success).
 */
export const checkTestsPassed = (state: TRMState): HaltDecision | null => {
  if (state.y.validated && state.y.validationResult?.passed) {
    return {
      shouldHalt: true,
      reason: "tests_passed",
      confidence: state.y.confidence,
      depth: state.z.depth,
      maxDepth: state.z.maxDepth,
      details: `All ${state.y.validationResult.testsPassed} tests passed`,
    };
  }
  return null;
};

/**
 * Check if confidence is high enough.
 */
export const checkHighConfidence = (
  state: TRMState,
  config: HaltConfig,
): HaltDecision | null => {
  if (
    state.y.confidence >= config.confidenceThreshold &&
    state.z.progress.stepsCompleted >= config.minStepsBeforeHalt
  ) {
    return {
      shouldHalt: true,
      reason: "high_confidence",
      confidence: state.y.confidence,
      depth: state.z.depth,
      maxDepth: state.z.maxDepth,
      details: `Confidence ${(state.y.confidence * 100).toFixed(1)}% >= ${(config.confidenceThreshold * 100).toFixed(1)}%`,
    };
  }
  return null;
};

/**
 * Check if stuck (repeated errors).
 */
export const checkStuck = (state: TRMState, config: HaltConfig): HaltDecision | null => {
  if (state.z.progress.isStuck && state.z.progress.stuckCount >= config.maxStuckCount) {
    return {
      shouldHalt: true,
      reason: "stuck",
      confidence: state.y.confidence,
      depth: state.z.depth,
      maxDepth: state.z.maxDepth,
      details: `Stuck ${state.z.progress.stuckCount} times on same error pattern`,
    };
  }
  return null;
};

/**
 * Check if training accuracy achieved.
 */
export const checkAccuracyAchieved = (
  state: TRMState,
  config: HaltConfig,
): HaltDecision | null => {
  if (
    state.y.trainingAccuracy >= config.accuracyThreshold &&
    state.z.progress.stepsCompleted >= config.minStepsBeforeHalt
  ) {
    return {
      shouldHalt: true,
      reason: "accuracy_achieved",
      confidence: state.y.confidence,
      depth: state.z.depth,
      maxDepth: state.z.maxDepth,
      details: `Training accuracy ${(state.y.trainingAccuracy * 100).toFixed(1)}% >= ${(config.accuracyThreshold * 100).toFixed(1)}%`,
    };
  }
  return null;
};

/**
 * Determine whether to halt or continue.
 * Checks conditions in priority order.
 */
export const shouldHalt = (
  state: TRMState,
  config: HaltConfig = DEFAULT_HALT_CONFIG,
): HaltDecision => {
  // Check in priority order:
  // 1. Tests passed (definitive success)
  const testsResult = checkTestsPassed(state);
  if (testsResult) return testsResult;

  // 2. Accuracy achieved
  const accuracyResult = checkAccuracyAchieved(state, config);
  if (accuracyResult) return accuracyResult;

  // 3. High confidence
  const confidenceResult = checkHighConfidence(state, config);
  if (confidenceResult) return confidenceResult;

  // 4. Max depth reached
  const depthResult = checkMaxDepth(state, config);
  if (depthResult) return depthResult;

  // 5. Stuck
  const stuckResult = checkStuck(state, config);
  if (stuckResult) return stuckResult;

  // Otherwise, continue
  return {
    shouldHalt: false,
    reason: "continue",
    confidence: state.y.confidence,
    depth: state.z.depth,
    maxDepth: state.z.maxDepth,
    details: `Continuing at depth ${state.z.depth}/${state.z.maxDepth}`,
  };
};

// --- Progress Detection ---

export interface ProgressStatus {
  isProgressing: boolean;
  progressType: "accuracy_improving" | "new_hypothesis" | "error_resolved" | "stalled" | "regressing";
  details: string;
}

/**
 * Detect if we're making progress (for adaptive depth).
 */
export const detectProgress = (
  previousState: TRMState,
  currentState: TRMState,
): ProgressStatus => {
  // Check accuracy improvement
  if (currentState.y.trainingAccuracy > previousState.y.trainingAccuracy) {
    return {
      isProgressing: true,
      progressType: "accuracy_improving",
      details: `Accuracy improved: ${(previousState.y.trainingAccuracy * 100).toFixed(1)}% → ${(currentState.y.trainingAccuracy * 100).toFixed(1)}%`,
    };
  }

  // Check if we've added new hypotheses
  if (currentState.z.hypotheses.length > previousState.z.hypotheses.length) {
    return {
      isProgressing: true,
      progressType: "new_hypothesis",
      details: `Added ${currentState.z.hypotheses.length - previousState.z.hypotheses.length} new hypothesis(es)`,
    };
  }

  // Check if stuck status cleared
  if (previousState.z.progress.isStuck && !currentState.z.progress.isStuck) {
    return {
      isProgressing: true,
      progressType: "error_resolved",
      details: "Resolved stuck state",
    };
  }

  // Check for regression
  if (currentState.y.trainingAccuracy < previousState.y.trainingAccuracy) {
    return {
      isProgressing: false,
      progressType: "regressing",
      details: `Accuracy regressed: ${(previousState.y.trainingAccuracy * 100).toFixed(1)}% → ${(currentState.y.trainingAccuracy * 100).toFixed(1)}%`,
    };
  }

  // Otherwise stalled
  return {
    isProgressing: false,
    progressType: "stalled",
    details: "No significant progress detected",
  };
};

// --- Service Interface ---

export interface ITRMHaltService {
  /** Determine whether to halt or continue */
  readonly shouldHalt: (
    state: TRMState,
    config?: Partial<HaltConfig>,
  ) => Effect.Effect<HaltDecision, never>;

  /** Detect progress between states */
  readonly detectProgress: (
    previous: TRMState,
    current: TRMState,
  ) => Effect.Effect<ProgressStatus, never>;

  /** Get current halt configuration */
  readonly getConfig: () => Effect.Effect<HaltConfig, never>;

  /** Update halt configuration */
  readonly updateConfig: (updates: Partial<HaltConfig>) => Effect.Effect<HaltConfig, never>;
}

// --- Service Tag ---

export class TRMHaltService extends Context.Tag("TRMHaltService")<
  TRMHaltService,
  ITRMHaltService
>() {}

// --- Service Implementation ---

const makeTRMHaltService = (initialConfig: HaltConfig = DEFAULT_HALT_CONFIG): ITRMHaltService => {
  let config = { ...initialConfig };

  return {
    shouldHalt: (state, configOverrides = {}) =>
      Effect.sync(() => shouldHalt(state, { ...config, ...configOverrides })),

    detectProgress: (previous, current) => Effect.sync(() => detectProgress(previous, current)),

    getConfig: () => Effect.sync(() => ({ ...config })),

    updateConfig: (updates) =>
      Effect.sync(() => {
        config = { ...config, ...updates };
        return { ...config };
      }),
  };
};

// --- Layer ---

export const TRMHaltServiceLive: Layer.Layer<TRMHaltService, never, never> = Layer.succeed(
  TRMHaltService,
  makeTRMHaltService(),
);

/**
 * Create a TRMHaltService layer with custom config.
 */
export const makeTRMHaltServiceLayer = (
  config: Partial<HaltConfig> = {},
): Layer.Layer<TRMHaltService, never, never> =>
  Layer.succeed(TRMHaltService, makeTRMHaltService({ ...DEFAULT_HALT_CONFIG, ...config }));
