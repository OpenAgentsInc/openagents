/**
 * TRM State Schema
 *
 * Implements the Tiny Recursive Model (TRM) state representation.
 * TRM uses a simpler {x, y, z} state that outperforms HRM's hierarchical approach.
 *
 * State components:
 * - x: Task context (input, description, constraints)
 * - y: Candidate solution (current proposed output)
 * - z: Reasoning trace (hypotheses, error patterns, progress)
 *
 * From TRM paper: Two features (y, z) achieve 87.4% accuracy,
 * beating both single-feature (71.9%) and multi-scale (77.6%).
 */

import * as S from "effect/Schema";
import { Effect, Context, Layer } from "effect";

// --- Task Context (x) ---

export const TaskContext = S.Struct({
  /** Unique task identifier */
  taskId: S.String,

  /** Natural language task description */
  description: S.String,

  /** Input data/examples for the task */
  inputs: S.Array(S.Unknown),

  /** Expected outputs (if available for training) */
  expectedOutputs: S.optional(S.Array(S.Unknown)),

  /** Task constraints or requirements */
  constraints: S.optional(S.Array(S.String)),

  /** Task category for depth/strategy selection */
  category: S.optional(S.String),

  /** Estimated difficulty (1-5) */
  difficulty: S.optional(S.Number),
});
export type TaskContext = S.Schema.Type<typeof TaskContext>;

// --- Candidate Solution (y) ---

export const CandidateSolution = S.Struct({
  /** Current proposed solution code/output */
  code: S.String,

  /** Solution generation attempt number */
  attemptNumber: S.Number,

  /** Whether the solution has been validated */
  validated: S.Boolean,

  /** Validation result if validated */
  validationResult: S.optional(
    S.Struct({
      passed: S.Boolean,
      testsRun: S.Number,
      testsPassed: S.Number,
      errors: S.optional(S.Array(S.String)),
    }),
  ),

  /** Training accuracy (fraction of examples passing) */
  trainingAccuracy: S.Number,

  /** Confidence score (0-1) */
  confidence: S.Number,

  /** Timestamp of last update */
  updatedAt: S.String,
});
export type CandidateSolution = S.Schema.Type<typeof CandidateSolution>;

// --- Reasoning Trace (z) ---

export const ReasoningTrace = S.Struct({
  /** Current hypotheses about the solution approach */
  hypotheses: S.Array(S.String),

  /** Error patterns observed from failed attempts */
  errorPatterns: S.Array(S.String),

  /** Approaches that have been ruled out */
  ruledOut: S.Array(S.String),

  /** Progress indicators */
  progress: S.Struct({
    stepsCompleted: S.Number,
    totalSteps: S.Number,
    isStuck: S.Boolean,
    stuckCount: S.Number,
  }),

  /** Reasoning history (last N reasoning steps) */
  history: S.Array(
    S.Struct({
      step: S.Number,
      thought: S.String,
      action: S.optional(S.String),
      result: S.optional(S.String),
    }),
  ),

  /** Current recursion depth */
  depth: S.Number,

  /** Maximum recursion depth for this task */
  maxDepth: S.Number,
});
export type ReasoningTrace = S.Schema.Type<typeof ReasoningTrace>;

// --- Complete TRM State ---

export const TRMState = S.Struct({
  /** Task context - what we're trying to solve */
  x: TaskContext,

  /** Candidate solution - current proposed answer */
  y: CandidateSolution,

  /** Reasoning trace - how we got here and what we've learned */
  z: ReasoningTrace,

  /** State metadata */
  meta: S.Struct({
    /** State creation timestamp */
    createdAt: S.String,
    /** Last update timestamp */
    updatedAt: S.String,
    /** Number of refinement cycles */
    cycles: S.Number,
    /** Total tokens consumed */
    tokensUsed: S.Number,
  }),
});
export type TRMState = S.Schema.Type<typeof TRMState>;

// --- State Creation Helpers ---

/**
 * Create initial task context from a task description.
 */
export const createTaskContext = (
  taskId: string,
  description: string,
  inputs: unknown[] = [],
  options: Partial<Omit<TaskContext, "taskId" | "description" | "inputs">> = {},
): TaskContext => ({
  taskId,
  description,
  inputs,
  ...options,
});

/**
 * Create initial candidate solution (empty).
 */
export const createInitialSolution = (): CandidateSolution => ({
  code: "",
  attemptNumber: 0,
  validated: false,
  trainingAccuracy: 0,
  confidence: 0,
  updatedAt: new Date().toISOString(),
});

/**
 * Create initial reasoning trace.
 */
export const createInitialReasoning = (maxDepth: number = 42): ReasoningTrace => ({
  hypotheses: [],
  errorPatterns: [],
  ruledOut: [],
  progress: {
    stepsCompleted: 0,
    totalSteps: maxDepth,
    isStuck: false,
    stuckCount: 0,
  },
  history: [],
  depth: 0,
  maxDepth,
});

/**
 * Create initial TRM state from task context.
 */
export const createTRMState = (x: TaskContext, maxDepth: number = 42): TRMState => {
  const now = new Date().toISOString();
  return {
    x,
    y: createInitialSolution(),
    z: createInitialReasoning(maxDepth),
    meta: {
      createdAt: now,
      updatedAt: now,
      cycles: 0,
      tokensUsed: 0,
    },
  };
};

// --- State Update Functions ---

/**
 * Update the candidate solution.
 */
export const updateSolution = (
  state: TRMState,
  updates: Partial<CandidateSolution>,
): TRMState => ({
  ...state,
  y: {
    ...state.y,
    ...updates,
    attemptNumber: state.y.attemptNumber + 1,
    updatedAt: new Date().toISOString(),
  },
  meta: {
    ...state.meta,
    updatedAt: new Date().toISOString(),
  },
});

/**
 * Update the reasoning trace.
 */
export const updateReasoning = (
  state: TRMState,
  updates: Partial<ReasoningTrace>,
): TRMState => ({
  ...state,
  z: {
    ...state.z,
    ...updates,
    depth: state.z.depth + 1,
  },
  meta: {
    ...state.meta,
    updatedAt: new Date().toISOString(),
  },
});

/**
 * Add a reasoning step to history.
 */
export const addReasoningStep = (
  state: TRMState,
  thought: string,
  action?: string,
  result?: string,
): TRMState => {
  const newHistory = [
    ...state.z.history.slice(-9), // Keep last 10 steps
    {
      step: state.z.depth + 1,
      thought,
      action,
      result,
    },
  ];

  return updateReasoning(state, {
    history: newHistory,
    progress: {
      ...state.z.progress,
      stepsCompleted: state.z.progress.stepsCompleted + 1,
    },
  });
};

/**
 * Mark state as stuck (same error repeated).
 */
export const markStuck = (state: TRMState, errorPattern: string): TRMState => {
  const existingErrors = state.z.errorPatterns;
  const isRepeat = existingErrors.includes(errorPattern);

  return updateReasoning(state, {
    errorPatterns: isRepeat ? existingErrors : [...existingErrors, errorPattern],
    progress: {
      ...state.z.progress,
      isStuck: true,
      stuckCount: state.z.progress.stuckCount + 1,
    },
  });
};

/**
 * Add hypothesis to reasoning.
 */
export const addHypothesis = (state: TRMState, hypothesis: string): TRMState =>
  updateReasoning(state, {
    hypotheses: [...state.z.hypotheses.slice(-4), hypothesis], // Keep last 5
  });

/**
 * Rule out an approach.
 */
export const ruleOutApproach = (state: TRMState, approach: string): TRMState =>
  updateReasoning(state, {
    ruledOut: [...state.z.ruledOut, approach],
  });

/**
 * Increment cycle count after completing a full y-update.
 */
export const completeCycle = (state: TRMState, tokensUsed: number = 0): TRMState => ({
  ...state,
  meta: {
    ...state.meta,
    cycles: state.meta.cycles + 1,
    tokensUsed: state.meta.tokensUsed + tokensUsed,
    updatedAt: new Date().toISOString(),
  },
});

/**
 * Detach state for next supervision step.
 * Preserves learned insights but clears transient execution details.
 */
export const detachState = (state: TRMState): TRMState => ({
  ...state,
  z: {
    ...state.z,
    // Keep: hypotheses, errorPatterns, ruledOut (learned knowledge)
    // Clear: history (transient), reset stuck status
    history: state.z.history.slice(-3), // Keep only last 3 steps
    progress: {
      ...state.z.progress,
      isStuck: false, // Fresh attempt
    },
  },
  meta: {
    ...state.meta,
    updatedAt: new Date().toISOString(),
  },
});

// --- Service Interface ---

export interface ITRMStateService {
  /** Create a new TRM state for a task */
  readonly create: (
    taskId: string,
    description: string,
    inputs?: unknown[],
    options?: { maxDepth?: number; difficulty?: number },
  ) => Effect.Effect<TRMState, never>;

  /** Update solution in state */
  readonly updateSolution: (
    state: TRMState,
    code: string,
    trainingAccuracy: number,
    confidence: number,
  ) => Effect.Effect<TRMState, never>;

  /** Add reasoning step */
  readonly addReasoning: (
    state: TRMState,
    thought: string,
    action?: string,
    result?: string,
  ) => Effect.Effect<TRMState, never>;

  /** Mark state as stuck */
  readonly markStuck: (state: TRMState, errorPattern: string) => Effect.Effect<TRMState, never>;

  /** Complete a refinement cycle */
  readonly completeCycle: (state: TRMState, tokensUsed?: number) => Effect.Effect<TRMState, never>;

  /** Detach state for next supervision step */
  readonly detach: (state: TRMState) => Effect.Effect<TRMState, never>;

  /** Check if state has reached max depth */
  readonly isMaxDepth: (state: TRMState) => Effect.Effect<boolean, never>;

  /** Get current training accuracy */
  readonly getAccuracy: (state: TRMState) => Effect.Effect<number, never>;
}

// --- Service Tag ---

export class TRMStateService extends Context.Tag("TRMStateService")<
  TRMStateService,
  ITRMStateService
>() {}

// --- Service Implementation ---

const makeTRMStateService = (): ITRMStateService => ({
  create: (taskId, description, inputs = [], options = {}) =>
    Effect.sync(() => {
      const x = createTaskContext(taskId, description, inputs, {
        difficulty: options.difficulty,
      });
      return createTRMState(x, options.maxDepth ?? 42);
    }),

  updateSolution: (state, code, trainingAccuracy, confidence) =>
    Effect.sync(() =>
      updateSolution(state, {
        code,
        trainingAccuracy,
        confidence,
        validated: false,
      }),
    ),

  addReasoning: (state, thought, action, result) =>
    Effect.sync(() => addReasoningStep(state, thought, action, result)),

  markStuck: (state, errorPattern) => Effect.sync(() => markStuck(state, errorPattern)),

  completeCycle: (state, tokensUsed = 0) => Effect.sync(() => completeCycle(state, tokensUsed)),

  detach: (state) => Effect.sync(() => detachState(state)),

  isMaxDepth: (state) => Effect.sync(() => state.z.depth >= state.z.maxDepth),

  getAccuracy: (state) => Effect.sync(() => state.y.trainingAccuracy),
});

// --- Layer ---

export const TRMStateServiceLive: Layer.Layer<TRMStateService, never, never> = Layer.succeed(
  TRMStateService,
  makeTRMStateService(),
);
