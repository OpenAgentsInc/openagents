/**
 * TRM Halt Decision Tests
 *
 * Tests for TRM halt conditions and progress detection.
 */

import { describe, test, expect } from "bun:test";
import { Effect } from "effect";
import {
  DEFAULT_HALT_CONFIG,
  checkMaxDepth,
  checkTestsPassed,
  checkHighConfidence,
  checkAccuracyAchieved,
  checkStuck,
  shouldHalt,
  detectProgress,
  TRMHaltService,
  TRMHaltServiceLive,
  makeTRMHaltServiceLayer,
  type HaltConfig,
} from "../trm-halt.js";
import { createMockTRMState, runEffect } from "./test-helpers.js";

describe("Halt Configuration", () => {
  test("default config has expected values", () => {
    expect(DEFAULT_HALT_CONFIG.confidenceThreshold).toBe(0.95);
    expect(DEFAULT_HALT_CONFIG.accuracyThreshold).toBe(1.0);
    expect(DEFAULT_HALT_CONFIG.maxStuckCount).toBe(3);
    expect(DEFAULT_HALT_CONFIG.minStepsBeforeHalt).toBe(3);
  });
});

describe("checkMaxDepth", () => {
  test("returns null when below max depth", () => {
    const state = createMockTRMState({ 
      z: { 
        depth: 10 
      } 
    });
    const result = checkMaxDepth(state, DEFAULT_HALT_CONFIG);
    expect(result).toBeNull();
  });

  test("returns halt decision at max depth", () => {
    const state = createMockTRMState({ z: { progress: { stepsCompleted: 5, totalSteps: 42, isStuck: false, stuckCount: 0 }, depth: 42, maxDepth: 42 } });
    const result = checkMaxDepth(state, DEFAULT_HALT_CONFIG);

    expect(result).not.toBeNull();
    expect(result?.shouldHalt).toBe(true);
    expect(result?.reason).toBe("max_depth");
  });

  test("returns halt decision above max depth", () => {
    const state = createMockTRMState({ z: { progress: { stepsCompleted: 5, totalSteps: 42, isStuck: false, stuckCount: 0 }, depth: 50, maxDepth: 42 } });
    const result = checkMaxDepth(state, DEFAULT_HALT_CONFIG);

    expect(result?.shouldHalt).toBe(true);
    expect(result?.reason).toBe("max_depth");
  });

  test("respects config override", () => {
    const state = createMockTRMState({ z: { progress: { stepsCompleted: 5, totalSteps: 42, isStuck: false, stuckCount: 0 }, depth: 10, maxDepth: 42 } });
    const config: HaltConfig = { ...DEFAULT_HALT_CONFIG, maxDepthOverride: 5 };
    const result = checkMaxDepth(state, config);

    expect(result?.shouldHalt).toBe(true);
    expect(result?.details).toContain("5");
  });
});

describe("checkTestsPassed", () => {
  test("returns null when not validated", () => {
    const state = createMockTRMState({ y: { validated: false } });
    const result = checkTestsPassed(state);
    expect(result).toBeNull();
  });

  test("returns null when validated but tests failed", () => {
    const state = createMockTRMState({
      y: {
        validated: true,
        validationResult: { passed: false, testsRun: 10, testsPassed: 5 },
      },
    });
    const result = checkTestsPassed(state);
    expect(result).toBeNull();
  });

  test("returns halt decision when tests passed", () => {
    const state = createMockTRMState({
      y: {
        validated: true,
        validationResult: { passed: true, testsRun: 10, testsPassed: 10 },
        confidence: 0.99,
      },
      z: { progress: { stepsCompleted: 5, totalSteps: 42, isStuck: false, stuckCount: 0 }, depth: 5, maxDepth: 42 },
    });
    const result = checkTestsPassed(state);

    expect(result).not.toBeNull();
    expect(result?.shouldHalt).toBe(true);
    expect(result?.reason).toBe("tests_passed");
    expect(result?.details).toContain("10 tests passed");
  });
});

describe("checkHighConfidence", () => {
  test("returns null when below threshold", () => {
    const state = createMockTRMState({
      y: { confidence: 0.5 },
      z: { progress: { stepsCompleted: 5, totalSteps: 42, isStuck: false, stuckCount: 0 } },
    });
    const result = checkHighConfidence(state, DEFAULT_HALT_CONFIG);
    expect(result).toBeNull();
  });

  test("returns null when not enough steps", () => {
    const state = createMockTRMState({
      y: { confidence: 0.99 },
      z: { progress: { stepsCompleted: 1, totalSteps: 42, isStuck: false, stuckCount: 0 } },
    });
    const result = checkHighConfidence(state, DEFAULT_HALT_CONFIG);
    expect(result).toBeNull();
  });

  test("returns halt at exact threshold with enough steps", () => {
    const state = createMockTRMState({
      y: { confidence: 0.95 },
      z: { progress: { stepsCompleted: 5, totalSteps: 42, isStuck: false, stuckCount: 0 }, depth: 5, maxDepth: 42 },
    });
    const result = checkHighConfidence(state, DEFAULT_HALT_CONFIG);

    expect(result).not.toBeNull();
    expect(result?.shouldHalt).toBe(true);
    expect(result?.reason).toBe("high_confidence");
  });

  test("returns halt above threshold", () => {
    const state = createMockTRMState({
      y: { confidence: 0.99 },
      z: { progress: { stepsCompleted: 5, totalSteps: 42, isStuck: false, stuckCount: 0 }, depth: 5, maxDepth: 42 },
    });
    const result = checkHighConfidence(state, DEFAULT_HALT_CONFIG);

    expect(result?.shouldHalt).toBe(true);
    expect(result?.confidence).toBe(0.99);
  });
});

describe("checkAccuracyAchieved", () => {
  test("returns null when below threshold", () => {
    const state = createMockTRMState({
      y: { trainingAccuracy: 0.8 },
      z: { progress: { stepsCompleted: 5, totalSteps: 42, isStuck: false, stuckCount: 0 } },
    });
    const result = checkAccuracyAchieved(state, DEFAULT_HALT_CONFIG);
    expect(result).toBeNull();
  });

  test("returns null when not enough steps", () => {
    const state = createMockTRMState({
      y: { trainingAccuracy: 1.0 },
      z: { progress: { stepsCompleted: 1, totalSteps: 42, isStuck: false, stuckCount: 0 } },
    });
    const result = checkAccuracyAchieved(state, DEFAULT_HALT_CONFIG);
    expect(result).toBeNull();
  });

  test("returns halt at exact threshold with enough steps", () => {
    const state = createMockTRMState({
      y: { trainingAccuracy: 1.0, confidence: 0.9 },
      z: { progress: { stepsCompleted: 5, totalSteps: 42, isStuck: false, stuckCount: 0 }, depth: 5, maxDepth: 42 },
    });
    const result = checkAccuracyAchieved(state, DEFAULT_HALT_CONFIG);

    expect(result).not.toBeNull();
    expect(result?.shouldHalt).toBe(true);
    expect(result?.reason).toBe("accuracy_achieved");
  });
});

describe("checkStuck", () => {
  test("returns null when not stuck", () => {
    const state = createMockTRMState({
      z: { progress: { stepsCompleted: 5, totalSteps: 42, isStuck: false, stuckCount: 0 } },
    });
    const result = checkStuck(state, DEFAULT_HALT_CONFIG);
    expect(result).toBeNull();
  });

  test("returns null when stuck but below threshold", () => {
    const state = createMockTRMState({
      z: { progress: { stepsCompleted: 5, totalSteps: 42, isStuck: true, stuckCount: 2 } },
    });
    const result = checkStuck(state, DEFAULT_HALT_CONFIG);
    expect(result).toBeNull();
  });

  test("returns halt when stuck at threshold", () => {
    const state = createMockTRMState({
      y: { confidence: 0.5 },
      z: { progress: { stepsCompleted: 5, totalSteps: 42, isStuck: true, stuckCount: 3 }, depth: 10, maxDepth: 42 },
    });
    const result = checkStuck(state, DEFAULT_HALT_CONFIG);

    expect(result).not.toBeNull();
    expect(result?.shouldHalt).toBe(true);
    expect(result?.reason).toBe("stuck");
    expect(result?.details).toContain("3 times");
  });

  test("returns halt when stuck above threshold", () => {
    const state = createMockTRMState({
      y: { confidence: 0.3 },
      z: { progress: { stepsCompleted: 5, totalSteps: 42, isStuck: true, stuckCount: 5 }, depth: 10, maxDepth: 42 },
    });
    const result = checkStuck(state, DEFAULT_HALT_CONFIG);

    expect(result?.shouldHalt).toBe(true);
    expect(result?.details).toContain("5 times");
  });
});

describe("shouldHalt", () => {
  test("returns continue when no halt conditions met", () => {
    const state = createMockTRMState({
      y: { validated: false, trainingAccuracy: 0.5, confidence: 0.5 },
      z: { depth: 5, maxDepth: 42, progress: { isStuck: false, stuckCount: 0, stepsCompleted: 5, totalSteps: 42 } },
    });
    const result = shouldHalt(state);

    expect(result.shouldHalt).toBe(false);
    expect(result.reason).toBe("continue");
  });

  test("prioritizes tests_passed over other conditions", () => {
    const state = createMockTRMState({
      y: {
        validated: true,
        validationResult: { passed: true, testsRun: 10, testsPassed: 10 },
        trainingAccuracy: 1.0,
        confidence: 0.99,
      },
      z: { depth: 42, maxDepth: 42, progress: { isStuck: true, stuckCount: 5, stepsCompleted: 10, totalSteps: 42 } },
    });
    const result = shouldHalt(state);

    expect(result.shouldHalt).toBe(true);
    expect(result.reason).toBe("tests_passed");
  });

  test("prioritizes accuracy_achieved over confidence", () => {
    const state = createMockTRMState({
      y: { validated: false, trainingAccuracy: 1.0, confidence: 0.99 },
      z: { depth: 5, maxDepth: 42, progress: { isStuck: false, stuckCount: 0, stepsCompleted: 5, totalSteps: 42 } },
    });
    const result = shouldHalt(state);

    expect(result.shouldHalt).toBe(true);
    expect(result.reason).toBe("accuracy_achieved");
  });

  test("prioritizes high_confidence over max_depth", () => {
    const state = createMockTRMState({
      y: { validated: false, trainingAccuracy: 0.8, confidence: 0.96 },
      z: { depth: 42, maxDepth: 42, progress: { isStuck: false, stuckCount: 0, stepsCompleted: 10, totalSteps: 42 } },
    });
    const result = shouldHalt(state);

    expect(result.shouldHalt).toBe(true);
    expect(result.reason).toBe("high_confidence");
  });

  test("prioritizes max_depth over stuck", () => {
    const state = createMockTRMState({
      y: { validated: false, trainingAccuracy: 0.3, confidence: 0.3 },
      z: { depth: 42, maxDepth: 42, progress: { isStuck: true, stuckCount: 5, stepsCompleted: 10, totalSteps: 42 } },
    });
    const result = shouldHalt(state);

    expect(result.shouldHalt).toBe(true);
    expect(result.reason).toBe("max_depth");
  });

  test("accepts config overrides", () => {
    const state = createMockTRMState({
      y: { validated: false, trainingAccuracy: 0.8, confidence: 0.8 },
      z: { depth: 5, maxDepth: 42, progress: { stepsCompleted: 5, totalSteps: 42 } },
    });
    const result = shouldHalt(state, { ...DEFAULT_HALT_CONFIG, confidenceThreshold: 0.7 });

    expect(result.shouldHalt).toBe(true);
    expect(result.reason).toBe("high_confidence");
  });
});

describe("detectProgress", () => {
  test("detects accuracy improvement", () => {
    const prev = createMockTRMState({ y: { trainingAccuracy: 0.5 } });
    const curr = createMockTRMState({ y: { trainingAccuracy: 0.7 } });
    const result = detectProgress(prev, curr);

    expect(result.isProgressing).toBe(true);
    expect(result.progressType).toBe("accuracy_improving");
    expect(result.details).toContain("50.0%");
    expect(result.details).toContain("70.0%");
  });

  test("detects new hypothesis", () => {
    const prev = createMockTRMState({ y: { trainingAccuracy: 0.5 }, z: { hypotheses: ["h1"] } });
    const curr = createMockTRMState({ y: { trainingAccuracy: 0.5 }, z: { hypotheses: ["h1", "h2"] } });
    const result = detectProgress(prev, curr);

    expect(result.isProgressing).toBe(true);
    expect(result.progressType).toBe("new_hypothesis");
  });

  test("detects error resolved", () => {
    const prev = createMockTRMState({
      y: { trainingAccuracy: 0.5 },
      z: { hypotheses: [], progress: { isStuck: true, stepsCompleted: 5, totalSteps: 42, stuckCount: 1 } },
    });
    const curr = createMockTRMState({
      y: { trainingAccuracy: 0.5 },
      z: { hypotheses: [], progress: { isStuck: false, stepsCompleted: 5, totalSteps: 42, stuckCount: 0 } },
    });
    const result = detectProgress(prev, curr);

    expect(result.isProgressing).toBe(true);
    expect(result.progressType).toBe("error_resolved");
  });

  test("detects regression", () => {
    const prev = createMockTRMState({ y: { trainingAccuracy: 0.7 } });
    const curr = createMockTRMState({ y: { trainingAccuracy: 0.5 } });
    const result = detectProgress(prev, curr);

    expect(result.isProgressing).toBe(false);
    expect(result.progressType).toBe("regressing");
  });

  test("detects stalled", () => {
    const prev = createMockTRMState({
      y: { trainingAccuracy: 0.5 },
      z: { hypotheses: ["h1"], progress: { isStuck: false, stepsCompleted: 5, totalSteps: 42, stuckCount: 0 } },
    });
    const curr = createMockTRMState({
      y: { trainingAccuracy: 0.5 },
      z: { hypotheses: ["h1"], progress: { isStuck: false, stepsCompleted: 5, totalSteps: 42, stuckCount: 0 } },
    });
    const result = detectProgress(prev, curr);

    expect(result.isProgressing).toBe(false);
    expect(result.progressType).toBe("stalled");
  });
});

describe("TRMHaltService", () => {
  test("shouldHalt returns decision", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMHaltService;
        const state = createMockTRMState({
          y: { validated: false, trainingAccuracy: 0.5, confidence: 0.5 },
          z: { depth: 5, maxDepth: 42, progress: { stepsCompleted: 5, totalSteps: 42 } },
        });
        return yield* service.shouldHalt(state);
      }).pipe(Effect.provide(TRMHaltServiceLive)),
    );

    expect(result.shouldHalt).toBe(false);
    expect(result.reason).toBe("continue");
  });

  test("detectProgress returns status", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMHaltService;
        const prev = createMockTRMState({ y: { trainingAccuracy: 0.5 } });
        const curr = createMockTRMState({ y: { trainingAccuracy: 0.8 } });
        return yield* service.detectProgress(prev, curr);
      }).pipe(Effect.provide(TRMHaltServiceLive)),
    );

    expect(result.isProgressing).toBe(true);
    expect(result.progressType).toBe("accuracy_improving");
  });

  test("getConfig returns current config", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMHaltService;
        return yield* service.getConfig();
      }).pipe(Effect.provide(TRMHaltServiceLive)),
    );

    expect(result.confidenceThreshold).toBe(0.95);
  });

  test("updateConfig modifies config", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMHaltService;
        const updated = yield* service.updateConfig({ confidenceThreshold: 0.8 });
        return updated;
      }).pipe(Effect.provide(TRMHaltServiceLive)),
    );

    expect(result.confidenceThreshold).toBe(0.8);
  });

  test("custom config layer", () => {
    const customLayer = makeTRMHaltServiceLayer({ confidenceThreshold: 0.5 });
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMHaltService;
        return yield* service.getConfig();
      }).pipe(Effect.provide(customLayer)),
    );

    expect(result.confidenceThreshold).toBe(0.5);
  });
});
