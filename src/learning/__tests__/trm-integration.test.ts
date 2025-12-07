/**
 * TRM Integration Tests
 *
 * Tests for TRM components working together end-to-end.
 * Focuses on real integration behavior rather than implementation details.
 */

import { describe, test, expect } from "bun:test";
import {
  createTRMState,
  updateSolution,
  completeCycle,
  markStuck,
  addHypothesis,
} from "../trm-state.js";
import {
  shouldHalt,
  detectProgress,
  DEFAULT_HALT_CONFIG,
} from "../trm-halt.js";
import {
  createEMAValue,
  updateEMA,
  isReliable,
  DEFAULT_EMA_CONFIG,
} from "../trm-ema.js";
import { createMockTRMState, createMockTaskContext } from "./test-helpers.js";

describe("TRM State Lifecycle", () => {
  test("state progresses through cycles", () => {
    let state = createTRMState(createMockTaskContext({ description: "Test task" }));

    // Initial state
    expect(state.meta.cycles).toBe(0);
    expect(state.y.code).toBe("");
    expect(state.y.confidence).toBe(0);

    // First cycle
    state = updateSolution(state, { code: "function solve() {}", confidence: 0.3 });
    state = completeCycle(state, 500);

    expect(state.meta.cycles).toBe(1);
    expect(state.meta.tokensUsed).toBe(500);

    // Second cycle with improvement
    state = updateSolution(state, { code: "function solve(x) { return x; }", confidence: 0.6 });
    state = completeCycle(state, 600);

    expect(state.meta.cycles).toBe(2);
    expect(state.y.confidence).toBe(0.6);
  });

  test("hypothesis tracking through iterations", () => {
    let state = createMockTRMState();
    const initialCount = state.z.hypotheses.length;

    state = addHypothesis(state, "Try recursive approach");
    state = addHypothesis(state, "Consider memoization");

    expect(state.z.hypotheses).toHaveLength(initialCount + 2);
    expect(state.z.hypotheses).toContain("Try recursive approach");
    expect(state.z.hypotheses).toContain("Consider memoization");
  });

  test("marking stuck state", () => {
    let state = createMockTRMState();

    state = markStuck(state, "Same output 3 times");

    expect(state.z.progress.isStuck).toBe(true);
    expect(state.z.errorPatterns).toContain("Same output 3 times");
  });
});

describe("TRM Progress Detection", () => {
  test("detects accuracy improvement", () => {
    const prevState = createMockTRMState({
      y: { trainingAccuracy: 0.3 },
    });
    const currState = createMockTRMState({
      y: { trainingAccuracy: 0.7 },
    });

    const progress = detectProgress(prevState, currState);
    expect(progress.isProgressing).toBe(true);
    expect(progress.progressType).toBe("accuracy_improving");
  });

  test("detects new hypothesis added", () => {
    const prevState = createMockTRMState();
    let currState = createMockTRMState();
    currState = addHypothesis(currState, "New idea");

    const progress = detectProgress(prevState, currState);
    expect(progress.isProgressing).toBe(true);
    expect(progress.progressType).toBe("new_hypothesis");
  });

  test("detects stalled when no change", () => {
    const state = createMockTRMState();
    const progress = detectProgress(state, state);

    expect(progress.isProgressing).toBe(false);
    expect(progress.progressType).toBe("stalled");
  });
});

describe("TRM Halt Decisions", () => {
  test("continues when conditions not met", () => {
    const state = createMockTRMState({
      meta: { cycles: 3 },
      y: { confidence: 0.5 },
      z: { progress: { isStuck: false, testsPassed: false } },
    });

    const result = shouldHalt(state);
    expect(result.shouldHalt).toBe(false);
    expect(result.reason).toBe("continue");
  });

  test("halts at max depth", () => {
    const state = createMockTRMState({
      meta: { cycles: 100 },
      z: { depth: 100, maxDepth: 42 },
    });

    const result = shouldHalt(state, { maxDepth: 42, confidenceThreshold: 0.9, stuckThreshold: 3, accuracyThreshold: 1.0 });
    expect(result.shouldHalt).toBe(true);
    expect(result.reason).toBe("max_depth");
  });

  test("halts on high confidence", () => {
    const state = createMockTRMState({
      y: { confidence: 0.95 },
    });

    const result = shouldHalt(state, { ...DEFAULT_HALT_CONFIG, confidenceThreshold: 0.9 });
    expect(result.shouldHalt).toBe(true);
    expect(result.reason).toBe("high_confidence");
  });

  test("halt decision includes metadata", () => {
    const state = createMockTRMState({
      z: { depth: 5, maxDepth: 42 },
    });

    const result = shouldHalt(state);
    expect(result.depth).toBe(5);
    expect(result.maxDepth).toBe(42);
    expect(result.confidence).toBeGreaterThanOrEqual(0);
  });
});

describe("TRM EMA Tracking", () => {
  test("EMA updates with new samples", () => {
    let ema = createEMAValue(0);

    ema = updateEMA(ema, 1.0, DEFAULT_EMA_CONFIG);
    expect(ema.sampleCount).toBe(1);
    expect(ema.value).toBeGreaterThan(0);

    ema = updateEMA(ema, 1.0, DEFAULT_EMA_CONFIG);
    expect(ema.sampleCount).toBe(2);
    expect(ema.value).toBeGreaterThan(0);
  });

  test("EMA becomes reliable after sufficient samples", () => {
    let ema = createEMAValue(0);
    const config = { ...DEFAULT_EMA_CONFIG, minSamples: 5 };

    // Not yet reliable
    for (let i = 0; i < 4; i++) {
      ema = updateEMA(ema, 0.8, config);
    }
    expect(isReliable(ema, config)).toBe(false);

    // Now reliable
    ema = updateEMA(ema, 0.8, config);
    expect(isReliable(ema, config)).toBe(true);
  });

  test("EMA tracks variance", () => {
    let ema = createEMAValue(0);

    // Feed in same value repeatedly - variance should be low
    for (let i = 0; i < 10; i++) {
      ema = updateEMA(ema, 0.5, DEFAULT_EMA_CONFIG);
    }

    expect(ema.variance).toBeLessThan(0.1);
  });

  test("EMA tracks recent values", () => {
    let ema = createEMAValue(0);

    for (let i = 1; i <= 5; i++) {
      ema = updateEMA(ema, i * 0.1, DEFAULT_EMA_CONFIG);
    }

    expect(ema.recentValues.length).toBe(5);
    expect(ema.recentValues).toContain(0.5);
  });
});

describe("Full TRM Loop Simulation", () => {
  test("simulates improvement over cycles", () => {
    let state = createTRMState(createMockTaskContext({ description: "Compute fibonacci" }));

    const improvements = [
      { code: "function fib(n) { return n; }", confidence: 0.2 },
      { code: "function fib(n) { return n <= 1 ? n : n-1; }", confidence: 0.4 },
      { code: "function fib(n) { return n <= 1 ? n : fib(n-1) + fib(n-2); }", confidence: 0.8 },
    ];

    for (const { code, confidence } of improvements) {
      state = updateSolution(state, { code, confidence });
      state = completeCycle(state, 500);
    }

    expect(state.meta.cycles).toBe(3);
    expect(state.y.confidence).toBe(0.8);
    expect(state.y.code).toContain("fib(n-1)");
  });

  test("tracks metrics throughout cycle", () => {
    let state = createMockTRMState({ meta: { cycles: 0, createdAt: "", updatedAt: "", tokensUsed: 0 } });
    const metrics: number[] = [];

    for (let i = 0; i < 5; i++) {
      state = completeCycle(state, 100 * (i + 1));
      metrics.push(state.meta.tokensUsed);
    }

    // Tokens should accumulate: 100, 300, 600, 1000, 1500
    expect(metrics).toEqual([100, 300, 600, 1000, 1500]);
  });

  test("convergence with progress detection", () => {
    let state = createMockTRMState({ meta: { cycles: 0, createdAt: "", updatedAt: "", tokensUsed: 0 } });
    let prevState = state;
    let progressCount = 0;

    const accuracies = [0.3, 0.5, 0.6, 0.65, 0.68, 0.69, 0.695];

    for (const accuracy of accuracies) {
      state = updateSolution(state, { trainingAccuracy: accuracy });
      state = completeCycle(state, 100);

      const progress = detectProgress(prevState, state);
      if (progress.isProgressing && progress.progressType === "accuracy_improving") {
        progressCount++;
      }

      prevState = state;
    }

    expect(progressCount).toBeGreaterThan(0);
  });
});
