/**
 * Test Helpers for Learning System
 *
 * Mock factories and utilities for testing TRM+SOAR modules.
 */

import { Effect } from "effect";
import type { TaskContext, CandidateSolution, ReasoningTrace, TRMState } from "../trm-state.js";
import type { AttemptRecord, SyntheticTask, SyntheticTaskSolution } from "../soar-hindsight.js";
import type { Vote } from "../soar-voting.js";
import type { EMAValue, TaskTypeStats, SkillEMAStats } from "../trm-ema.js";
import type { TTTState, TTTIterationResult } from "../soar-ttt.js";

// --- Effect Test Helper ---

/**
 * Run an Effect synchronously for tests.
 */
export const runEffect = <A>(program: Effect.Effect<A, never, never>): A =>
  Effect.runSync(program);

/**
 * Run an Effect as a Promise for async tests.
 */
export const runEffectPromise = <A, E>(program: Effect.Effect<A, E, never>): Promise<A> =>
  Effect.runPromise(program);

// --- TRM Mock Factories ---

export const createMockTaskContext = (
  overrides: Partial<TaskContext> = {},
): TaskContext => ({
  taskId: "test-task-001",
  description: "Test task description",
  inputs: [{ example: "input" }],
  ...overrides,
});

export const createMockCandidateSolution = (
  overrides: Partial<CandidateSolution> = {},
): CandidateSolution => ({
  code: "function solve(input) { return input; }",
  attemptNumber: 1,
  validated: false,
  trainingAccuracy: 0.5,
  confidence: 0.6,
  updatedAt: new Date().toISOString(),
  ...overrides,
});

export const createMockReasoningTrace = (
  overrides: Partial<ReasoningTrace> = {},
): ReasoningTrace => ({
  hypotheses: ["Try a loop approach"],
  errorPatterns: [],
  ruledOut: [],
  progress: {
    stepsCompleted: 5,
    totalSteps: 42,
    isStuck: false,
    stuckCount: 0,
  },
  history: [
    { step: 1, thought: "Initial approach" },
    { step: 2, thought: "Refined solution" },
  ],
  depth: 5,
  maxDepth: 42,
  ...overrides,
});

export const createMockTRMState = (
  overrides: Partial<TRMState> & {
    x?: Partial<TaskContext>;
    y?: Partial<CandidateSolution>;
    z?: Partial<ReasoningTrace> & { progress?: Partial<ReasoningTrace["progress"]> };
    meta?: Partial<TRMState["meta"]>;
  } = {},
): TRMState => {
  const now = new Date().toISOString();
  const baseZ = createMockReasoningTrace();
  const baseY = createMockCandidateSolution();
  const baseX = createMockTaskContext();

  // Deep merge z.progress if provided
  const zProgress = overrides.z?.progress
    ? { ...baseZ.progress, ...overrides.z.progress }
    : baseZ.progress;

  return {
    x: { ...baseX, ...overrides.x },
    y: { ...baseY, ...overrides.y },
    z: { ...baseZ, ...overrides.z, progress: zProgress },
    meta: {
      createdAt: now,
      updatedAt: now,
      cycles: 3,
      tokensUsed: 1500,
      ...overrides.meta,
    },
  };
};

// --- EMA Mock Factories ---

export const createMockEMAValue = (
  overrides: Partial<EMAValue> = {},
): EMAValue => ({
  value: 0.75,
  sampleCount: 10,
  updatedAt: new Date().toISOString(),
  variance: 0.01,
  recentValues: [0.7, 0.72, 0.74, 0.76, 0.75],
  ...overrides,
});

export const createMockTaskTypeStats = (
  overrides: Partial<TaskTypeStats> = {},
): TaskTypeStats => ({
  taskType: "arc-agi",
  successRate: createMockEMAValue({ value: 0.65 }),
  optimalDepth: createMockEMAValue({ value: 25 }),
  tokensPerAttempt: createMockEMAValue({ value: 1200 }),
  timePerAttempt: createMockEMAValue({ value: 35000 }),
  totalAttempts: 100,
  totalSuccesses: 65,
  ...overrides,
});

export const createMockSkillEMAStats = (
  overrides: Partial<SkillEMAStats> = {},
): SkillEMAStats => ({
  skillId: "skill-001",
  samplingSuccessRate: createMockEMAValue({ value: 0.6 }),
  refinementSuccessRate: createMockEMAValue({ value: 0.7 }),
  jointConfidence: 0.65,
  updatedAt: new Date().toISOString(),
  ...overrides,
});

// --- SOAR Mock Factories ---

export const createMockAttemptRecord = (
  overrides: Partial<AttemptRecord> = {},
): AttemptRecord => ({
  id: `attempt-${Math.random().toString(36).slice(2, 8)}`,
  taskId: "test-task-001",
  taskDescription: "Compute the nth fibonacci number",
  code: "function fib(n) { return n <= 1 ? n : fib(n-1) + fib(n-2); }",
  actualOutput: 55,
  expectedOutput: 89,
  success: false,
  trainingAccuracy: 0.3,
  tokensUsed: 500,
  durationMs: 2000,
  timestamp: new Date().toISOString(),
  ...overrides,
});

export const createMockSyntheticTask = (
  overrides: Partial<SyntheticTask> = {},
): SyntheticTask => ({
  id: `synthetic-${Math.random().toString(36).slice(2, 8)}`,
  description: "[Hindsight] Task that produces the actual output",
  input: { n: 10 },
  output: 55,
  originalTaskId: "test-task-001",
  attemptId: "attempt-001",
  confidence: 0.45,
  validated: false,
  createdAt: new Date().toISOString(),
  ...overrides,
});

export const createMockSyntheticTaskSolution = (
  overrides: Partial<SyntheticTaskSolution> = {},
): SyntheticTaskSolution => ({
  task: createMockSyntheticTask(overrides.task as Partial<SyntheticTask>),
  solution: "function solve(n) { return n * 2; }",
  source: "hindsight",
  qualityScore: 0.45,
  ...overrides,
});

export const createMockVote = (
  overrides: Partial<Vote> = {},
): Vote => ({
  output: 42,
  outputKey: "42",
  program: "function solve() { return 42; }",
  trainingAccuracy: 0.8,
  ...overrides,
});

// --- TTT Mock Factories ---

export const createMockTTTState = (
  overrides: Partial<TTTState> = {},
): TTTState => ({
  currentIteration: 2,
  bestSolution: "function solve(x) { return x * 2; }",
  bestAccuracy: 0.75,
  allAttempts: [
    createMockAttemptRecord({ trainingAccuracy: 0.5 }),
    createMockAttemptRecord({ trainingAccuracy: 0.75 }),
  ],
  syntheticPairs: [createMockSyntheticTaskSolution()],
  iterationHistory: [],
  tokensUsed: 5000,
  startTime: Date.now() - 10000,
  ...overrides,
});

export const createMockTTTIterationResult = (
  overrides: Partial<TTTIterationResult> = {},
): TTTIterationResult => ({
  iteration: 1,
  bestAccuracy: 0.75,
  averageAccuracy: 0.6,
  attemptCount: 50,
  syntheticCount: 30,
  bestSolution: "function solve(x) { return x; }",
  improved: true,
  completedAt: new Date().toISOString(),
  ...overrides,
});

// --- Batch Helpers ---

/**
 * Create multiple attempt records with varying accuracy.
 */
export const createMockAttemptBatch = (
  count: number,
  baseAccuracy: number = 0.3,
  variance: number = 0.2,
): AttemptRecord[] =>
  Array.from({ length: count }, (_, i) => {
    const accuracy = Math.max(0, Math.min(1, baseAccuracy + (Math.random() - 0.5) * variance));
    return createMockAttemptRecord({
      id: `attempt-batch-${i}`,
      trainingAccuracy: accuracy,
      success: accuracy >= 1.0,
    });
  });

/**
 * Create multiple synthetic task-solutions with varying quality.
 */
export const createMockSyntheticBatch = (
  count: number,
  baseQuality: number = 0.4,
): SyntheticTaskSolution[] =>
  Array.from({ length: count }, (_, i) => {
    const quality = Math.max(0.01, Math.min(0.99, baseQuality + (Math.random() - 0.5) * 0.3));
    return createMockSyntheticTaskSolution({
      task: createMockSyntheticTask({ id: `synthetic-batch-${i}` }),
      qualityScore: quality,
    });
  });

/**
 * Create multiple votes with varying accuracy.
 */
export const createMockVoteBatch = (
  outputs: unknown[],
  accuracies: number[],
): Vote[] =>
  outputs.map((output, i) =>
    createMockVote({
      output,
      outputKey: JSON.stringify(output),
      trainingAccuracy: accuracies[i] ?? 0.5,
    }),
  );

// --- Assertion Helpers ---

/**
 * Check if a value is within a range (inclusive).
 */
export const isInRange = (value: number, min: number, max: number): boolean =>
  value >= min && value <= max;

/**
 * Check if two dates are within a threshold (ms).
 */
export const datesWithin = (
  date1: string | Date,
  date2: string | Date,
  thresholdMs: number = 1000,
): boolean => {
  const d1 = typeof date1 === "string" ? new Date(date1) : date1;
  const d2 = typeof date2 === "string" ? new Date(date2) : date2;
  return Math.abs(d1.getTime() - d2.getTime()) <= thresholdMs;
};
