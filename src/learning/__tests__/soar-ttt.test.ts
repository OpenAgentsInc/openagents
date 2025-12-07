/**
 * SOAR Test-Time Training Tests
 *
 * Tests for the TTT loop and state management.
 */

import { describe, test, expect } from "bun:test";
import * as S from "effect/Schema";
import { Effect } from "effect";
import {
  TTTIterationResult,
  TTTSessionResult,
  DEFAULT_TTT_CONFIG,
  createTTTState,
  shouldContinueTTT,
  getStopReason,
  processIteration,
  createSessionResult,
  outputsEqual,
  updateSkillContext,
  TTTService,
  TTTServiceLive,
  makeTTTServiceLayer,
  type TTTState,
  type TTTSkillContext,
} from "../soar-ttt.js";
import { createMockAttemptRecord, createMockSyntheticTaskSolution, runEffect } from "./test-helpers.js";

describe("TTTIterationResult Schema", () => {
  test("decodes valid result", () => {
    const input = {
      iteration: 1,
      bestAccuracy: 0.8,
      averageAccuracy: 0.6,
      attemptCount: 50,
      syntheticCount: 30,
      bestSolution: "function f() {}",
      improved: true,
      completedAt: new Date().toISOString(),
    };
    const decoded = S.decodeUnknownSync(TTTIterationResult)(input);
    expect(decoded.iteration).toBe(1);
    expect(decoded.bestAccuracy).toBe(0.8);
  });
});

describe("TTTSessionResult Schema", () => {
  test("decodes valid result", () => {
    const input = {
      taskId: "task-001",
      finalPrediction: 42,
      finalSolution: "code",
      bestAccuracy: 0.9,
      iterationsCompleted: 3,
      stopReason: "satisfied",
      iterations: [],
      totalSyntheticPairs: 100,
      totalTokensUsed: 50000,
      durationMs: 30000,
      startedAt: new Date().toISOString(),
      completedAt: new Date().toISOString(),
    };
    const decoded = S.decodeUnknownSync(TTTSessionResult)(input);
    expect(decoded.taskId).toBe("task-001");
    expect(decoded.stopReason).toBe("satisfied");
  });
});

describe("createTTTState", () => {
  test("creates initial state", () => {
    const state = createTTTState();

    expect(state.currentIteration).toBe(0);
    expect(state.bestSolution).toBe("");
    expect(state.bestAccuracy).toBe(0);
    expect(state.allAttempts).toEqual([]);
    expect(state.syntheticPairs).toEqual([]);
    expect(state.iterationHistory).toEqual([]);
    expect(state.tokensUsed).toBe(0);
    expect(state.startTime).toBeLessThanOrEqual(Date.now());
  });
});

describe("shouldContinueTTT", () => {
  test("returns false when satisfaction threshold met", () => {
    const state: TTTState = {
      ...createTTTState(),
      bestAccuracy: 1.0,
    };
    expect(shouldContinueTTT(state, DEFAULT_TTT_CONFIG)).toBe(false);
  });

  test("returns false when max iterations reached", () => {
    const state: TTTState = {
      ...createTTTState(),
      currentIteration: 5,
    };
    expect(shouldContinueTTT(state, DEFAULT_TTT_CONFIG)).toBe(false);
  });

  test("returns false when no improvement", () => {
    const state: TTTState = {
      ...createTTTState(),
      currentIteration: 2,
      iterationHistory: [
        { iteration: 0, bestAccuracy: 0.5, averageAccuracy: 0.4, attemptCount: 50, syntheticCount: 30, bestSolution: "", improved: true, completedAt: "" },
        { iteration: 1, bestAccuracy: 0.5, averageAccuracy: 0.4, attemptCount: 50, syntheticCount: 30, bestSolution: "", improved: false, completedAt: "" },
      ],
    };
    expect(shouldContinueTTT(state, DEFAULT_TTT_CONFIG)).toBe(false);
  });

  test("returns true when should continue", () => {
    const state: TTTState = {
      ...createTTTState(),
      currentIteration: 1,
      bestAccuracy: 0.5,
      iterationHistory: [
        { iteration: 0, bestAccuracy: 0.3, averageAccuracy: 0.2, attemptCount: 50, syntheticCount: 30, bestSolution: "", improved: true, completedAt: "" },
        { iteration: 1, bestAccuracy: 0.5, averageAccuracy: 0.4, attemptCount: 50, syntheticCount: 30, bestSolution: "", improved: true, completedAt: "" },
      ],
    };
    expect(shouldContinueTTT(state, DEFAULT_TTT_CONFIG)).toBe(true);
  });

  test("allows first iteration regardless", () => {
    const state = createTTTState();
    expect(shouldContinueTTT(state, DEFAULT_TTT_CONFIG)).toBe(true);
  });
});

describe("getStopReason", () => {
  test("returns satisfied when accuracy at threshold", () => {
    const state: TTTState = { ...createTTTState(), bestAccuracy: 1.0 };
    expect(getStopReason(state, DEFAULT_TTT_CONFIG)).toBe("satisfied");
  });

  test("returns max_iterations when limit reached", () => {
    const state: TTTState = { ...createTTTState(), currentIteration: 5, bestAccuracy: 0.5 };
    expect(getStopReason(state, DEFAULT_TTT_CONFIG)).toBe("max_iterations");
  });

  test("returns no_improvement when stalled", () => {
    const state: TTTState = {
      ...createTTTState(),
      currentIteration: 2,
      bestAccuracy: 0.5,
      iterationHistory: [
        { iteration: 0, bestAccuracy: 0.5, averageAccuracy: 0.4, attemptCount: 50, syntheticCount: 30, bestSolution: "", improved: true, completedAt: "" },
        { iteration: 1, bestAccuracy: 0.5, averageAccuracy: 0.4, attemptCount: 50, syntheticCount: 30, bestSolution: "", improved: false, completedAt: "" },
      ],
    };
    expect(getStopReason(state, DEFAULT_TTT_CONFIG)).toBe("no_improvement");
  });
});

describe("processIteration", () => {
  test("processes empty attempts", () => {
    const state = createTTTState();
    const { state: newState, result } = processIteration(state, [], []);

    expect(newState.currentIteration).toBe(1);
    expect(result.attemptCount).toBe(0);
    expect(result.bestAccuracy).toBe(0);
  });

  test("finds best attempt", () => {
    const state = createTTTState();
    const attempts = [
      createMockAttemptRecord({ trainingAccuracy: 0.3 }),
      createMockAttemptRecord({ trainingAccuracy: 0.8 }),
      createMockAttemptRecord({ trainingAccuracy: 0.5 }),
    ];
    const { result } = processIteration(state, attempts, []);

    expect(result.bestAccuracy).toBe(0.8);
  });

  test("calculates average accuracy", () => {
    const state = createTTTState();
    const attempts = [
      createMockAttemptRecord({ trainingAccuracy: 0.4 }),
      createMockAttemptRecord({ trainingAccuracy: 0.6 }),
    ];
    const { result } = processIteration(state, attempts, []);

    expect(result.averageAccuracy).toBe(0.5);
  });

  test("detects improvement", () => {
    const state: TTTState = { ...createTTTState(), bestAccuracy: 0.5 };
    const attempts = [createMockAttemptRecord({ trainingAccuracy: 0.7 })];
    const { result } = processIteration(state, attempts, []);

    expect(result.improved).toBe(true);
  });

  test("accumulates attempts and synthetics", () => {
    const state = createTTTState();
    const attempts = [createMockAttemptRecord()];
    const synthetics = [createMockSyntheticTaskSolution()];
    const { state: newState } = processIteration(state, attempts, synthetics);

    expect(newState.allAttempts).toHaveLength(1);
    expect(newState.syntheticPairs).toHaveLength(1);
  });

  test("updates best solution on improvement", () => {
    const state = createTTTState();
    const attempts = [createMockAttemptRecord({ trainingAccuracy: 0.8, code: "best code" })];
    const { state: newState } = processIteration(state, attempts, []);

    expect(newState.bestSolution).toBe("best code");
    expect(newState.bestAccuracy).toBe(0.8);
  });

  test("tracks tokens used", () => {
    const state = createTTTState();
    const attempts = [
      createMockAttemptRecord({ tokensUsed: 500 }),
      createMockAttemptRecord({ tokensUsed: 300 }),
    ];
    const { state: newState } = processIteration(state, attempts, []);

    expect(newState.tokensUsed).toBe(800);
  });
});

describe("createSessionResult", () => {
  test("creates complete session result", () => {
    const state: TTTState = {
      ...createTTTState(),
      currentIteration: 3,
      bestSolution: "final code",
      bestAccuracy: 0.9,
      syntheticPairs: [createMockSyntheticTaskSolution()],
      tokensUsed: 10000,
    };
    const result = createSessionResult("task-001", state, DEFAULT_TTT_CONFIG);

    expect(result.taskId).toBe("task-001");
    expect(result.finalSolution).toBe("final code");
    expect(result.bestAccuracy).toBe(0.9);
    expect(result.iterationsCompleted).toBe(3);
    expect(result.totalSyntheticPairs).toBe(1);
    expect(result.totalTokensUsed).toBe(10000);
  });

  test("includes voting result when provided", () => {
    const state = createTTTState();
    const votingResult = {
      winner: 42,
      winnerKey: "42",
      winnerWeight: 100,
      confidence: 0.8,
      candidates: [],
      totalVotes: 10,
      isValid: true,
      votedAt: new Date().toISOString(),
    };
    const result = createSessionResult("task", state, DEFAULT_TTT_CONFIG, votingResult);

    expect(result.finalPrediction).toBe(42);
    expect(result.votingResult).toBeDefined();
  });

  test("calculates duration", () => {
    const state: TTTState = {
      ...createTTTState(),
      startTime: Date.now() - 5000,
    };
    const result = createSessionResult("task", state, DEFAULT_TTT_CONFIG);

    expect(result.durationMs).toBeGreaterThanOrEqual(5000);
    expect(result.durationMs).toBeLessThan(6000);
  });
});

describe("outputsEqual", () => {
  test("handles null/undefined", () => {
    expect(outputsEqual(null, null)).toBe(true);
    expect(outputsEqual(undefined, undefined)).toBe(true);
    expect(outputsEqual(null, undefined)).toBe(false);
    expect(outputsEqual(null, 0)).toBe(false);
  });

  test("handles primitives", () => {
    expect(outputsEqual(42, 42)).toBe(true);
    expect(outputsEqual(42, 43)).toBe(false);
    expect(outputsEqual("hello", "hello")).toBe(true);
    expect(outputsEqual(true, true)).toBe(true);
    expect(outputsEqual(true, false)).toBe(false);
  });

  test("handles type mismatches", () => {
    expect(outputsEqual(42, "42")).toBe(false);
    expect(outputsEqual(true, 1)).toBe(false);
  });

  test("handles arrays", () => {
    expect(outputsEqual([1, 2, 3], [1, 2, 3])).toBe(true);
    expect(outputsEqual([1, 2, 3], [1, 2])).toBe(false);
    expect(outputsEqual([1, 2, 3], [3, 2, 1])).toBe(false);
  });

  test("handles nested arrays", () => {
    expect(outputsEqual([[1, 2], [3]], [[1, 2], [3]])).toBe(true);
    expect(outputsEqual([[1, 2], [3]], [[1, 2], [4]])).toBe(false);
  });

  test("handles objects", () => {
    expect(outputsEqual({ a: 1, b: 2 }, { a: 1, b: 2 })).toBe(true);
    expect(outputsEqual({ a: 1, b: 2 }, { b: 2, a: 1 })).toBe(true);
    expect(outputsEqual({ a: 1 }, { a: 2 })).toBe(false);
    expect(outputsEqual({ a: 1 }, { a: 1, b: 2 })).toBe(false);
  });

  test("handles nested objects", () => {
    expect(outputsEqual({ a: { b: 1 } }, { a: { b: 1 } })).toBe(true);
    expect(outputsEqual({ a: { b: 1 } }, { a: { b: 2 } })).toBe(false);
  });

  test("handles mixed arrays and objects", () => {
    expect(outputsEqual([{ a: 1 }], [{ a: 1 }])).toBe(true);
    expect(outputsEqual({ arr: [1, 2] }, { arr: [1, 2] })).toBe(true);
  });
});

describe("updateSkillContext", () => {
  test("updates session uses", () => {
    const context: TTTSkillContext = {
      skillId: "skill-001",
      sessionSuccessRate: 0.5,
      sessionUses: 1,
      bestAccuracyWithSkill: 0.5,
      shouldBoost: false,
    };
    const attempt = createMockAttemptRecord({ success: true, trainingAccuracy: 0.8 });
    const updated = updateSkillContext(context, attempt);

    expect(updated.sessionUses).toBe(2);
  });

  test("updates success rate", () => {
    const context: TTTSkillContext = {
      skillId: "skill-001",
      sessionSuccessRate: 0,
      sessionUses: 0,
      bestAccuracyWithSkill: 0,
      shouldBoost: false,
    };
    const attempt = createMockAttemptRecord({ success: true });
    const updated = updateSkillContext(context, attempt);

    expect(updated.sessionSuccessRate).toBe(1);
  });

  test("updates best accuracy", () => {
    const context: TTTSkillContext = {
      skillId: "skill-001",
      sessionSuccessRate: 0.5,
      sessionUses: 1,
      bestAccuracyWithSkill: 0.5,
      shouldBoost: false,
    };
    const attempt = createMockAttemptRecord({ trainingAccuracy: 0.8 });
    const updated = updateSkillContext(context, attempt);

    expect(updated.bestAccuracyWithSkill).toBe(0.8);
  });

  test("sets shouldBoost based on heuristic", () => {
    const context: TTTSkillContext = {
      skillId: "skill-001",
      sessionSuccessRate: 0.1,
      sessionUses: 1,
      bestAccuracyWithSkill: 0.3,
      shouldBoost: false,
    };
    const attempt = createMockAttemptRecord({ success: true, trainingAccuracy: 0.6 });
    const updated = updateSkillContext(context, attempt);

    // shouldBoost: successRate > 0.2 OR bestAccuracy > 0.5
    expect(updated.shouldBoost).toBe(true);
  });
});

describe("TTTService", () => {
  test("shouldContinue returns boolean", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TTTService;
        const state = createTTTState();
        return yield* service.shouldContinue(state);
      }).pipe(Effect.provide(TTTServiceLive)),
    );

    expect(result).toBe(true);
  });

  test("processIteration returns state and result", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TTTService;
        const state = createTTTState();
        const attempts = [createMockAttemptRecord({ trainingAccuracy: 0.7 })];
        return yield* service.processIteration(state, attempts, []);
      }).pipe(Effect.provide(TTTServiceLive)),
    );

    expect(result.state.currentIteration).toBe(1);
    expect(result.result.bestAccuracy).toBe(0.7);
  });

  test("getStats tracks sessions", () => {
    // Note: runTTT would require mocked callbacks, testing basic stats tracking instead
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TTTService;
        return yield* service.getStats();
      }).pipe(Effect.provide(TTTServiceLive)),
    );

    expect(result.totalSessions).toBe(0);
    expect(result.totalIterations).toBe(0);
  });

  test("updateConfig modifies config", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TTTService;
        return yield* service.updateConfig({ maxIterations: 10 });
      }).pipe(Effect.provide(TTTServiceLive)),
    );

    expect(result.maxIterations).toBe(10);
  });

  test("custom config layer", () => {
    const customLayer = makeTTTServiceLayer({ attemptsPerIteration: 100 });
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TTTService;
        return yield* service.getConfig();
      }).pipe(Effect.provide(customLayer)),
    );

    expect(result.attemptsPerIteration).toBe(100);
  });
});
