/**
 * Training Loop Runner Tests
 */

import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import { Effect } from "effect";
import { rmSync, mkdirSync } from "fs";
import {
  createLoopRunner,
  runTrainingLoop,
  runOvernightTraining,
  runProgressiveBenchmark,
  LoopRunnerError,
  DEFAULT_CONFIG,
} from "./loop-runner.js";

const TEST_PROJECT_ROOT = "/tmp/loop-runner-test";

describe("Loop Runner Configuration", () => {
  test("DEFAULT_CONFIG has expected values", () => {
    expect(DEFAULT_CONFIG.startSubset).toBe("TB_10");
    expect(DEFAULT_CONFIG.maxDurationMs).toBe(0);
    expect(DEFAULT_CONFIG.maxIterations).toBe(0);
    expect(DEFAULT_CONFIG.progressionThreshold).toBe(0.8);
    expect(DEFAULT_CONFIG.minIterationsBeforeProgression).toBe(3);
    expect(DEFAULT_CONFIG.autoResume).toBe(true);
  });
});

describe("Loop Runner Creation", () => {
  test("createLoopRunner creates runner with defaults", () => {
    const runner = createLoopRunner();
    expect(runner).toBeDefined();
    expect(runner.start).toBeDefined();
    expect(runner.runIteration).toBeDefined();
    expect(runner.getState).toBeDefined();
    expect(runner.pause).toBeDefined();
    expect(runner.resume).toBeDefined();
    expect(runner.stop).toBeDefined();
    expect(runner.shouldProgress).toBeDefined();
    expect(runner.progressTier).toBeDefined();
  });

  test("createLoopRunner accepts custom config", () => {
    const runner = createLoopRunner({
      model: "fm",
      startSubset: "TB_30",
      maxIterations: 5,
    });
    expect(runner).toBeDefined();
  });
});

describe("Loop Runner State", () => {
  test("initial state has correct values", async () => {
    const runner = createLoopRunner({ projectRoot: TEST_PROJECT_ROOT });
    const state = await Effect.runPromise(runner.getState());

    expect(state.status).toBe("idle");
    expect(state.currentSubset).toBe("TB_10");
    expect(state.iteration).toBe(0);
    expect(state.totalIterations).toBe(0);
    expect(state.totalTasksCompleted).toBe(0);
    expect(state.totalSuccessful).toBe(0);
    expect(state.overallSuccessRate).toBe(0);
    expect(state.subsetIterations).toEqual({ TB_10: 0, TB_30: 0, TB_89: 0 });
  });

  test("pause updates status to paused", async () => {
    const runner = createLoopRunner({ projectRoot: TEST_PROJECT_ROOT });
    await Effect.runPromise(runner.pause());
    const state = await Effect.runPromise(runner.getState());
    expect(state.status).toBe("paused");
  });

  test("stop updates status to completed", async () => {
    const runner = createLoopRunner({ projectRoot: TEST_PROJECT_ROOT });
    const finalState = await Effect.runPromise(runner.stop());
    expect(finalState.status).toBe("completed");
  });
});

describe("Loop Runner Progression", () => {
  test("shouldProgress returns false when not enough iterations", async () => {
    const runner = createLoopRunner({
      projectRoot: TEST_PROJECT_ROOT,
      minIterationsBeforeProgression: 3,
    });
    const shouldProg = await Effect.runPromise(runner.shouldProgress());
    expect(shouldProg).toBe(false);
  });

  test("progressTier advances from TB_10 to TB_30", async () => {
    const runner = createLoopRunner({
      projectRoot: TEST_PROJECT_ROOT,
      startSubset: "TB_10",
    });
    const nextSubset = await Effect.runPromise(runner.progressTier());
    expect(nextSubset).toBe("TB_30");

    const state = await Effect.runPromise(runner.getState());
    expect(state.currentSubset).toBe("TB_30");
    expect(state.iteration).toBe(0); // Reset on progression
  });

  test("progressTier advances from TB_30 to TB_89", async () => {
    const runner = createLoopRunner({
      projectRoot: TEST_PROJECT_ROOT,
      startSubset: "TB_30",
    });
    const nextSubset = await Effect.runPromise(runner.progressTier());
    expect(nextSubset).toBe("TB_89");
  });

  test("progressTier stays at TB_89", async () => {
    const runner = createLoopRunner({
      projectRoot: TEST_PROJECT_ROOT,
      startSubset: "TB_89",
    });
    const nextSubset = await Effect.runPromise(runner.progressTier());
    expect(nextSubset).toBe("TB_89");
  });
});

describe("Loop Runner Iteration", () => {
  beforeEach(() => {
    try {
      rmSync(TEST_PROJECT_ROOT, { recursive: true, force: true });
    } catch {}
    mkdirSync(TEST_PROJECT_ROOT, { recursive: true });
  });

  afterEach(() => {
    try {
      rmSync(TEST_PROJECT_ROOT, { recursive: true, force: true });
    } catch {}
  });

  test("runIteration updates state", async () => {
    const runner = createLoopRunner({
      projectRoot: TEST_PROJECT_ROOT,
      autoResume: false,
    });

    const result = await Effect.runPromise(runner.runIteration());

    expect(result.subset).toBe("TB_10");
    expect(result.iteration).toBe(1);
    expect(result.summary).toBeDefined();
    expect(result.summary.total).toBe(10);
    expect(result.durationMs).toBeGreaterThan(0);

    const state = await Effect.runPromise(runner.getState());
    expect(state.iteration).toBe(1);
    expect(state.totalIterations).toBe(1);
    expect(state.subsetIterations.TB_10).toBe(1);
    expect(state.totalTasksCompleted).toBe(10);
  });

  test("multiple iterations accumulate correctly", async () => {
    const runner = createLoopRunner({
      projectRoot: TEST_PROJECT_ROOT,
      autoResume: false,
    });

    await Effect.runPromise(runner.runIteration());
    await Effect.runPromise(runner.runIteration());
    await Effect.runPromise(runner.runIteration());

    const state = await Effect.runPromise(runner.getState());
    expect(state.iteration).toBe(3);
    expect(state.totalIterations).toBe(3);
    expect(state.subsetIterations.TB_10).toBe(3);
    expect(state.totalTasksCompleted).toBe(30);
  });
});

describe("Loop Runner State Persistence", () => {
  beforeEach(() => {
    try {
      rmSync(TEST_PROJECT_ROOT, { recursive: true, force: true });
    } catch {}
    mkdirSync(TEST_PROJECT_ROOT, { recursive: true });
  });

  afterEach(() => {
    try {
      rmSync(TEST_PROJECT_ROOT, { recursive: true, force: true });
    } catch {}
  });

  test("saveState and loadState round-trip", async () => {
    const runner = createLoopRunner({
      projectRoot: TEST_PROJECT_ROOT,
      autoResume: false,
    });

    // Run an iteration to have some state
    await Effect.runPromise(runner.runIteration());

    // Save state
    await Effect.runPromise(runner.saveState());

    // Load state with new runner
    const runner2 = createLoopRunner({
      projectRoot: TEST_PROJECT_ROOT,
      autoResume: false,
    });

    const loadedState = await Effect.runPromise(runner2.loadState());
    expect(loadedState).not.toBeNull();
    expect(loadedState?.iteration).toBe(1);
    expect(loadedState?.totalIterations).toBe(1);
  });

  test("loadState returns null when no state file", async () => {
    const runner = createLoopRunner({
      projectRoot: TEST_PROJECT_ROOT,
      autoResume: false,
    });

    const loadedState = await Effect.runPromise(runner.loadState());
    expect(loadedState).toBeNull();
  });
});

describe("Loop Runner Limits", () => {
  beforeEach(() => {
    try {
      rmSync(TEST_PROJECT_ROOT, { recursive: true, force: true });
    } catch {}
    mkdirSync(TEST_PROJECT_ROOT, { recursive: true });
  });

  afterEach(() => {
    try {
      rmSync(TEST_PROJECT_ROOT, { recursive: true, force: true });
    } catch {}
  });

  test("iteration limit stops the loop", async () => {
    const runner = createLoopRunner({
      projectRoot: TEST_PROJECT_ROOT,
      maxIterations: 2,
      iterationDelayMs: 0,
      autoResume: false,
    });

    const finalState = await Effect.runPromise(runner.start());
    expect(finalState.status).toBe("completed");
    expect(finalState.totalIterations).toBe(2);
  });
});

describe("LoopRunnerError", () => {
  test("creates error with correct properties", () => {
    const error = new LoopRunnerError(
      "iteration_failed",
      "Test error message",
    );
    expect(error.reason).toBe("iteration_failed");
    expect(error.message).toBe("Test error message");
    expect(error._tag).toBe("LoopRunnerError");
  });

  test("creates error with cause", () => {
    const cause = new Error("Root cause");
    const error = new LoopRunnerError(
      "state_load_failed",
      "Failed to load",
      cause,
    );
    expect(error.cause).toBe(cause);
  });
});

describe("Convenience Functions", () => {
  test("runTrainingLoop is callable", () => {
    expect(runTrainingLoop).toBeDefined();
    expect(typeof runTrainingLoop).toBe("function");
  });

  test("runOvernightTraining is callable", () => {
    expect(runOvernightTraining).toBeDefined();
    expect(typeof runOvernightTraining).toBe("function");
  });

  test("runProgressiveBenchmark is callable", () => {
    expect(runProgressiveBenchmark).toBeDefined();
    expect(typeof runProgressiveBenchmark).toBe("function");
  });
});
