/**
 * TTT (Test-Time Training) Integration Tests
 *
 * Tests for TTT loop integration with SOAR voting and ensemble.
 */

import { describe, test, expect } from "bun:test";
import {
  shouldContinueTTT,
  createSessionResult,
  outputsEqual,
  createTTTState,
  DEFAULT_TTT_CONFIG,
  type TTTConfig,
} from "../soar-ttt.js";
import {
  vote,
  createVotes,
  ensembleVote,
  normalizeOutputKey,
  DEFAULT_VOTING_CONFIG,
} from "../soar-voting.js";
import { createMockTTTState, createMockTTTIterationResult } from "./test-helpers.js";

interface TTTIterationResult {
  iteration: number;
  task?: any;
  result?: any;
  reflection?: any;
  skill?: any;
  bestAccuracy: number;
  bestSolution: string;
  averageAccuracy: number;
  attemptCount: number;
  syntheticCount: number;
  improved: boolean;
  completedAt: string;
}

describe("TTT + Voting Integration", () => {
  test("TTT state accumulates attempts for voting", () => {
    // Simulate adding attempts with outputs
    const outputs = [
      { output: 42, program: "code1", trainingAccuracy: 0.9 },
      { output: 42, program: "code2", trainingAccuracy: 0.85 },
      { output: 42, program: "code3", trainingAccuracy: 0.8 },
      { output: 10, program: "code4", trainingAccuracy: 0.3 },
    ];

    // Vote on accumulated outputs
    const result = ensembleVote(outputs);

    expect(result.winner).toBe(42);
    expect(result.isValid).toBe(true);
    expect(result.totalVotes).toBe(4);
  });

  test("consensus detection from voting confidence", () => {
    // All outputs agree
    const outputs = [
      { output: "consensus", program: "code1", trainingAccuracy: 0.7 },
      { output: "consensus", program: "code2", trainingAccuracy: 0.75 },
      { output: "consensus", program: "code3", trainingAccuracy: 0.8 },
    ];

    const votingResult = ensembleVote(outputs);

    // 100% agreement should give high confidence
    expect(votingResult.confidence).toBeGreaterThanOrEqual(0.9);
    expect(votingResult.winner).toBe("consensus");
  });

  test("no consensus continues iterations", () => {
    const state = createMockTTTState({
      currentIteration: 2,
      bestAccuracy: 0.5,
      iterationHistory: [
        createMockTTTIterationResult({ iteration: 0, bestAccuracy: 0.3, improved: true }),
        createMockTTTIterationResult({ iteration: 1, bestAccuracy: 0.5, improved: true }),
      ],
    });

    const config: TTTConfig = { ...DEFAULT_TTT_CONFIG, maxIterations: 10, satisfactionThreshold: 1.0 };

    // Should continue since below satisfaction and within iteration limit
    const shouldContinue = shouldContinueTTT(state, config);
    expect(shouldContinue).toBe(true);
  });

  test("satisfaction threshold stops iterations", () => {
    const state = createMockTTTState({
      currentIteration: 2,
      bestAccuracy: 1.0, // Perfect accuracy
    });

    const config: TTTConfig = { ...DEFAULT_TTT_CONFIG, satisfactionThreshold: 1.0 };
    const shouldContinue = shouldContinueTTT(state, config);

    expect(shouldContinue).toBe(false);
  });
});

describe("TTT Session Lifecycle", () => {
  test("session progresses from start to completion", () => {
    let state = createTTTState();
    const config = DEFAULT_TTT_CONFIG;

    // Simulate 5 iterations with improving accuracy
    for (let i = 0; i < 5; i++) {
      const accuracy = 0.4 + i * 0.1;
      const iterResult: TTTIterationResult = {
        iteration: i,
        bestAccuracy: accuracy,
        averageAccuracy: accuracy - 0.1,
        attemptCount: 50,
        syntheticCount: 30,
        bestSolution: `function solve() { return ${i}; }`,
        improved: i > 0,
        completedAt: new Date().toISOString(),
      };

      state = {
        ...state,
        currentIteration: i + 1,
        bestAccuracy: accuracy,
        bestSolution: iterResult.bestSolution,
        iterationHistory: [...state.iterationHistory, iterResult],
      };
    }

    // Create final result
    const outputs = [
      { output: "final", program: "code", trainingAccuracy: state.bestAccuracy },
    ];
    const votingResult = ensembleVote(outputs);

    const result = createSessionResult("task-1", state, config, votingResult);

    expect(result.finalPrediction).toBe("final");
    expect(result.iterationsCompleted).toBe(5);
    expect(result.bestAccuracy).toBe(0.8);
  });

  test("max iterations stops session", () => {
    const state = createMockTTTState({
      currentIteration: 5,
      bestAccuracy: 0.6,
    });

    const config: TTTConfig = { ...DEFAULT_TTT_CONFIG, maxIterations: 5 };
    const shouldContinue = shouldContinueTTT(state, config);

    expect(shouldContinue).toBe(false);
  });

  test("no improvement stops session", () => {
    const state = createMockTTTState({
      currentIteration: 3,
      bestAccuracy: 0.5,
      iterationHistory: [
        createMockTTTIterationResult({ iteration: 0, bestAccuracy: 0.5, improved: true }),
        createMockTTTIterationResult({ iteration: 1, bestAccuracy: 0.5, improved: false }),
        createMockTTTIterationResult({ iteration: 2, bestAccuracy: 0.5, improved: false }),
      ],
    });

    const config: TTTConfig = { ...DEFAULT_TTT_CONFIG, minImprovementThreshold: 0.01 };
    const shouldContinue = shouldContinueTTT(state, config);

    // No improvement for 2 iterations, should stop
    expect(shouldContinue).toBe(false);
  });
});

describe("Output Comparison Across Systems", () => {
  test("outputsEqual matches voting normalization", () => {
    // Objects should normalize the same way
    const obj1 = { b: 2, a: 1 };
    const obj2 = { a: 1, b: 2 };

    expect(outputsEqual(obj1, obj2)).toBe(true);
    expect(normalizeOutputKey(obj1)).toBe(normalizeOutputKey(obj2));
  });

  test("arrays preserve order in comparison", () => {
    const arr1 = [1, 2, 3];
    const arr2 = [3, 2, 1];
    const arr3 = [1, 2, 3];

    expect(outputsEqual(arr1, arr2)).toBe(false);
    expect(outputsEqual(arr1, arr3)).toBe(true);
    expect(normalizeOutputKey(arr1)).toBe(normalizeOutputKey(arr3));
    expect(normalizeOutputKey(arr1)).not.toBe(normalizeOutputKey(arr2));
  });

  test("mixed types compare correctly", () => {
    expect(outputsEqual(42, 42)).toBe(true);
    expect(outputsEqual(42, "42")).toBe(false);
    expect(outputsEqual("hello", "hello")).toBe(true);
    expect(outputsEqual(null, null)).toBe(true);
    expect(outputsEqual(null, undefined)).toBe(false);
  });
});

describe("TTT + Ensemble Voting Strategies", () => {
  test("accuracy-weighted voting favors high quality", () => {
    const outputs = [
      { output: "high", program: "code", trainingAccuracy: 0.95 },
      { output: "low", program: "code", trainingAccuracy: 0.1 },
      { output: "low", program: "code", trainingAccuracy: 0.1 },
    ];

    const result = ensembleVote(outputs);

    // Despite 2 "low" votes, "high" should win due to weight
    // high: 1 + 1000 * 0.95 = 951
    // low: 2 * (1 + 1000 * 0.1) = 2 * 101 = 202
    expect(result.winner).toBe("high");
  });

  test("tie breaking by count when weights equal", () => {
    const outputs = [
      { output: "A", program: "code", trainingAccuracy: 0.5 },
      { output: "B", program: "code", trainingAccuracy: 0.5 },
      { output: "B", program: "code", trainingAccuracy: 0.5 },
    ];

    const result = vote(createVotes(outputs), {
      ...DEFAULT_VOTING_CONFIG,
      tieBreaker: "count",
    });

    // B has more votes with same accuracy
    expect(result.winner).toBe("B");
  });

  test("complex output voting", () => {
    const complexOutput = {
      grid: [[1, 2], [3, 4]],
      metadata: { size: 4, valid: true },
    };

    const outputs = [
      { output: complexOutput, program: "code1", trainingAccuracy: 0.8 },
      { output: complexOutput, program: "code2", trainingAccuracy: 0.7 },
      { output: { different: true }, program: "code3", trainingAccuracy: 0.5 },
    ];

    const result = ensembleVote(outputs);

    expect(result.winner).toEqual(complexOutput);
  });
});

describe("TTT Convergence Tracking", () => {
  test("accuracy improvement tracked across iterations", () => {
    let state = createTTTState();
    const accuracies = [0.3, 0.4, 0.5, 0.55, 0.6, 0.62, 0.63, 0.635];

    for (let i = 0; i < accuracies.length; i++) {
      const acc = accuracies[i]!;
      const improved = i > 0 && acc > (accuracies[i - 1] ?? 0);

      const iterResult: TTTIterationResult = {
        iteration: i,
        bestAccuracy: acc,
        averageAccuracy: acc - 0.05,
        attemptCount: 50,
        syntheticCount: 30,
        bestSolution: `solution-${i}`,
        improved,
        completedAt: new Date().toISOString(),
      };

      state = {
        ...state,
        currentIteration: i + 1,
        bestAccuracy: acc,
        iterationHistory: [...state.iterationHistory, iterResult],
      };
    }

    // Track convergence (small improvements)
    let stableCount = 0;
    const stableThreshold = 0.01;

    for (let i = 1; i < state.iterationHistory.length; i++) {
      const prev = state.iterationHistory[i - 1]!.bestAccuracy;
      const curr = state.iterationHistory[i]!.bestAccuracy;
      if (curr - prev < stableThreshold) {
        stableCount++;
      }
    }

    // Should detect convergence toward end
    expect(stableCount).toBeGreaterThan(0);
    expect(state.bestAccuracy).toBeCloseTo(0.635, 2);
  });

  test("final vote uses best performing outputs", () => {
    // Simulate outputs with clear quality differences
    const outputs = [
      { output: "bad", program: "code", trainingAccuracy: 0.2 },
      { output: "medium", program: "code", trainingAccuracy: 0.5 },
      { output: "good", program: "code", trainingAccuracy: 0.9 },
    ];

    const result = ensembleVote(outputs);

    // Highest accuracy should dominate
    expect(result.winner).toBe("good");
    expect(result.candidates[0]?.outputKey).toBe("good");
  });
});

describe("TTT Error Handling", () => {
  test("empty outputs handled gracefully", () => {
    // With minVotes >= 1, empty input returns invalid result
    const result = vote([], { ...DEFAULT_VOTING_CONFIG, minVotes: 1 });

    expect(result.isValid).toBe(false);
    expect(result.winner).toBe(null);
  });

  test("single output vote", () => {
    const outputs = [
      { output: "only", program: "code", trainingAccuracy: 0.8 },
    ];

    const result = ensembleVote(outputs);

    expect(result.winner).toBe("only");
    expect(result.confidence).toBe(1.0);
  });

  test("all outputs have zero accuracy", () => {
    const outputs = [
      { output: "A", program: "code", trainingAccuracy: 0 },
      { output: "B", program: "code", trainingAccuracy: 0 },
    ];

    const result = ensembleVote(outputs);

    // Should still produce a result (uses base weight of 1)
    expect(result.winner).toBeDefined();
    expect(result.isValid).toBe(true);
  });
});
