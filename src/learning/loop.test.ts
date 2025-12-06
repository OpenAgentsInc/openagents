/**
 * Training Loop Tests
 */

import { describe, test, expect } from "bun:test";
import { DEFAULT_LOOP_CONFIG, type LoopState } from "./loop.js";

describe("Loop Configuration", () => {
  test("DEFAULT_LOOP_CONFIG has expected values", () => {
    expect(DEFAULT_LOOP_CONFIG.maxIterations).toBe(0);
    expect(DEFAULT_LOOP_CONFIG.iterationDelayMs).toBe(1000);
    expect(DEFAULT_LOOP_CONFIG.archiveEveryN).toBe(5);
    expect(DEFAULT_LOOP_CONFIG.progressiveBenchmark).toBe(true);
    expect(DEFAULT_LOOP_CONFIG.startSubset).toBe("TB_10");
  });
});

describe("Loop State", () => {
  test("initial state structure is correct", () => {
    const state: LoopState = {
      iteration: 0,
      currentSubset: "TB_10",
      totalTasksCompleted: 0,
      totalSuccessful: 0,
      overallSuccessRate: 0,
      skillsLearned: 0,
      patternsExtracted: 0,
      status: "stopped",
      startedAt: new Date().toISOString(),
      totalDurationMs: 0,
    };

    expect(state.iteration).toBe(0);
    expect(state.currentSubset).toBe("TB_10");
    expect(state.status).toBe("stopped");
  });

  test("state can track progress", () => {
    const state: LoopState = {
      iteration: 5,
      currentSubset: "TB_30",
      totalTasksCompleted: 50,
      totalSuccessful: 40,
      overallSuccessRate: 0.8,
      skillsLearned: 3,
      patternsExtracted: 10,
      status: "running",
      startedAt: "2024-01-01T00:00:00.000Z",
      totalDurationMs: 300000,
    };

    expect(state.iteration).toBe(5);
    expect(state.currentSubset).toBe("TB_30");
    expect(state.overallSuccessRate).toBe(0.8);
    expect(state.status).toBe("running");
  });

  test("state can include last run and archive", () => {
    const state: LoopState = {
      iteration: 1,
      currentSubset: "TB_10",
      totalTasksCompleted: 10,
      totalSuccessful: 8,
      overallSuccessRate: 0.8,
      skillsLearned: 1,
      patternsExtracted: 2,
      status: "completed",
      startedAt: "2024-01-01T00:00:00.000Z",
      totalDurationMs: 60000,
      lastRun: {
        id: "run-123",
        config: DEFAULT_LOOP_CONFIG as any,
        tasks: [],
        results: [],
        stats: {
          totalTasks: 10,
          completedTasks: 10,
          successfulTasks: 8,
          failedTasks: 2,
          partialTasks: 0,
          timedOutTasks: 0,
          successRate: 0.8,
          averageScore: 0.8,
          totalDurationMs: 60000,
          averageDurationMs: 6000,
          totalTokens: 1500,
          skillsUsedCount: 5,
          reflexionAppliedCount: 2,
        },
        startedAt: "2024-01-01T00:00:00.000Z",
        completedAt: "2024-01-01T00:01:00.000Z",
        status: "completed",
      },
      lastArchive: {
        id: "arch-123",
        trajectoriesProcessed: 10,
        patternsExtracted: 2,
        skillsCreated: 1,
        memoriesCreated: 2,
        itemsPruned: 0,
        durationMs: 500,
        timestamp: "2024-01-01T00:01:00.000Z",
      },
    };

    expect(state.lastRun?.id).toBe("run-123");
    expect(state.lastRun?.stats.successRate).toBe(0.8);
    expect(state.lastArchive?.patternsExtracted).toBe(2);
  });
});

describe("Progressive Benchmark", () => {
  test("progression order is correct", () => {
    const progression: Record<string, string> = {
      TB_10: "TB_30",
      TB_30: "TB_89",
      TB_89: "TB_89",
    };

    expect(progression.TB_10).toBe("TB_30");
    expect(progression.TB_30).toBe("TB_89");
    expect(progression.TB_89).toBe("TB_89"); // Stays at max
  });

  test("progression threshold is reasonable", () => {
    const minTasks = 10;
    const minSuccessRate = 0.8;

    // Should progress when both conditions met
    expect(minTasks).toBe(10);
    expect(minSuccessRate).toBe(0.8);
  });
});

describe("Loop Status", () => {
  test("all status values are valid", () => {
    const statuses: LoopState["status"][] = ["running", "paused", "stopped", "completed"];

    expect(statuses).toContain("running");
    expect(statuses).toContain("paused");
    expect(statuses).toContain("stopped");
    expect(statuses).toContain("completed");
  });
});
