/**
 * Trainer Schema Tests
 */

import { describe, test, expect } from "bun:test";
import {
  generateRunId,
  generateTaskId,
  calculateStats,
  createTask,
  createTaskResult,
  createTrainingRun,
  DEFAULT_TRAINING_CONFIG,
  TB_SUBSETS,
  type TaskResult,
} from "./schema.js";

describe("ID Generation", () => {
  test("generateRunId creates unique IDs", () => {
    const id1 = generateRunId();
    const id2 = generateRunId();

    expect(id1).toMatch(/^run-[a-z0-9]+-[a-z0-9]+$/);
    expect(id2).toMatch(/^run-[a-z0-9]+-[a-z0-9]+$/);
    expect(id1).not.toBe(id2);
  });

  test("generateTaskId includes source prefix", () => {
    const id1 = generateTaskId("terminal-bench");
    const id2 = generateTaskId("custom");

    expect(id1).toMatch(/^task-term-[a-z0-9]+-[a-z0-9]+$/);
    expect(id2).toMatch(/^task-cust-[a-z0-9]+-[a-z0-9]+$/);
  });
});

describe("Task Creation", () => {
  test("createTask with minimal options", () => {
    const task = createTask("Fix the import error");

    expect(task.id).toMatch(/^task-cust-/);
    expect(task.prompt).toBe("Fix the import error");
    expect(task.difficulty).toBe(3);
    expect(task.category).toBe("general");
    expect(task.tags).toEqual([]);
    expect(task.timeoutMs).toBe(120000);
    expect(task.source).toBe("custom");
  });

  test("createTask with all options", () => {
    const task = createTask("Write unit tests", {
      id: "custom-task-1",
      expectedBehavior: "All tests pass",
      difficulty: 4,
      category: "testing",
      tags: ["unit", "jest"],
      timeoutMs: 60000,
      source: "tb",
      setupFiles: { "test.ts": "// test file" },
    });

    expect(task.id).toBe("custom-task-1");
    expect(task.prompt).toBe("Write unit tests");
    expect(task.expectedBehavior).toBe("All tests pass");
    expect(task.difficulty).toBe(4);
    expect(task.category).toBe("testing");
    expect(task.tags).toEqual(["unit", "jest"]);
    expect(task.timeoutMs).toBe(60000);
    expect(task.source).toBe("tb");
    expect(task.setupFiles).toEqual({ "test.ts": "// test file" });
  });
});

describe("Task Result Creation", () => {
  test("createTaskResult with minimal data", () => {
    const result = createTaskResult("task-1", {
      outcome: "success",
      durationMs: 5000,
      model: "fm",
      tokens: { input: 100, output: 50, total: 150 },
    });

    expect(result.taskId).toBe("task-1");
    expect(result.outcome).toBe("success");
    expect(result.durationMs).toBe(5000);
    expect(result.model).toBe("fm");
    expect(result.tokens.total).toBe(150);
    expect(result.skillsUsed).toEqual([]);
    expect(result.usedReflexion).toBe(false);
    expect(result.attemptNumber).toBe(1);
    expect(result.timestamp).toBeDefined();
  });

  test("createTaskResult with all data", () => {
    const result = createTaskResult("task-2", {
      outcome: "failure",
      score: 0.3,
      errorMessage: "Test failed",
      output: "Error output",
      durationMs: 10000,
      model: "fm",
      tokens: { input: 200, output: 100, total: 300 },
      skillsUsed: ["skill-1", "skill-2"],
      usedReflexion: true,
      attemptNumber: 2,
    });

    expect(result.outcome).toBe("failure");
    expect(result.score).toBe(0.3);
    expect(result.errorMessage).toBe("Test failed");
    expect(result.output).toBe("Error output");
    expect(result.skillsUsed).toEqual(["skill-1", "skill-2"]);
    expect(result.usedReflexion).toBe(true);
    expect(result.attemptNumber).toBe(2);
  });
});

describe("Training Run Creation", () => {
  test("createTrainingRun initializes correctly", () => {
    const run = createTrainingRun(DEFAULT_TRAINING_CONFIG);

    expect(run.id).toMatch(/^run-/);
    expect(run.config).toEqual(DEFAULT_TRAINING_CONFIG);
    expect(run.tasks).toEqual([]);
    expect(run.results).toEqual([]);
    expect(run.status).toBe("running");
    expect(run.startedAt).toBeDefined();
    expect(run.completedAt).toBeUndefined();
  });

  test("createTrainingRun uses custom config", () => {
    const customConfig = { ...DEFAULT_TRAINING_CONFIG, maxTasks: 5, useSkills: false };
    const run = createTrainingRun(customConfig);

    expect(run.config.maxTasks).toBe(5);
    expect(run.config.useSkills).toBe(false);
  });
});

describe("Stats Calculation", () => {
  const makeResult = (outcome: TaskResult["outcome"], score?: number): TaskResult => ({
    taskId: "task-1",
    outcome,
    score,
    durationMs: 1000,
    model: "fm",
    tokens: { input: 100, output: 50, total: 150 },
    skillsUsed: outcome === "success" ? ["skill-1"] : [],
    usedReflexion: outcome === "failure",
    attemptNumber: 1,
    timestamp: new Date().toISOString(),
  });

  test("calculateStats returns zeros for empty array", () => {
    const stats = calculateStats([]);

    expect(stats.totalTasks).toBe(0);
    expect(stats.successRate).toBe(0);
    expect(stats.averageScore).toBe(0);
  });

  test("calculateStats calculates success rate correctly", () => {
    const results = [
      makeResult("success", 1.0),
      makeResult("success", 1.0),
      makeResult("failure", 0.0),
      makeResult("partial", 0.5),
    ];

    const stats = calculateStats(results);

    expect(stats.totalTasks).toBe(4);
    expect(stats.successfulTasks).toBe(2);
    expect(stats.failedTasks).toBe(1);
    expect(stats.partialTasks).toBe(1);
    expect(stats.successRate).toBe(0.5);
  });

  test("calculateStats calculates average score correctly", () => {
    const results = [
      makeResult("success", 1.0),
      makeResult("partial", 0.5),
      makeResult("failure", 0.0),
    ];

    const stats = calculateStats(results);

    expect(stats.averageScore).toBe(0.5);
  });

  test("calculateStats counts unique skills", () => {
    const results: TaskResult[] = [
      { ...makeResult("success"), skillsUsed: ["skill-1", "skill-2"] },
      { ...makeResult("success"), skillsUsed: ["skill-1", "skill-3"] },
    ];

    const stats = calculateStats(results);

    expect(stats.skillsUsedCount).toBe(3);
  });

  test("calculateStats counts reflexion usage", () => {
    const results: TaskResult[] = [
      { ...makeResult("success"), usedReflexion: true },
      { ...makeResult("failure"), usedReflexion: true },
      { ...makeResult("success"), usedReflexion: false },
    ];

    const stats = calculateStats(results);

    expect(stats.reflexionAppliedCount).toBe(2);
  });

  test("calculateStats calculates duration totals", () => {
    const results = [
      { ...makeResult("success"), durationMs: 1000 },
      { ...makeResult("success"), durationMs: 2000 },
      { ...makeResult("failure"), durationMs: 3000 },
    ];

    const stats = calculateStats(results);

    expect(stats.totalDurationMs).toBe(6000);
    expect(stats.averageDurationMs).toBe(2000);
  });

  test("calculateStats calculates token totals", () => {
    const results = [
      { ...makeResult("success"), tokens: { input: 100, output: 50, total: 150 } },
      { ...makeResult("success"), tokens: { input: 200, output: 100, total: 300 } },
    ];

    const stats = calculateStats(results);

    expect(stats.totalTokens).toBe(450);
  });
});

describe("Terminal-Bench Subsets", () => {
  test("TB_SUBSETS defines correct counts", () => {
    expect(TB_SUBSETS.TB_10.count).toBe(10);
    expect(TB_SUBSETS.TB_30.count).toBe(30);
    expect(TB_SUBSETS.TB_89.count).toBe(89);
  });

  test("TB_SUBSETS have names and descriptions", () => {
    expect(TB_SUBSETS.TB_10.name).toBe("Terminal-Bench 10");
    expect(TB_SUBSETS.TB_30.description).toContain("Extended");
    expect(TB_SUBSETS.TB_89.description).toContain("Complete");
  });
});

describe("Default Config", () => {
  test("DEFAULT_TRAINING_CONFIG has expected values", () => {
    expect(DEFAULT_TRAINING_CONFIG.maxTasks).toBe(10);
    expect(DEFAULT_TRAINING_CONFIG.maxRetries).toBe(2);
    expect(DEFAULT_TRAINING_CONFIG.useSkills).toBe(true);
    expect(DEFAULT_TRAINING_CONFIG.useMemory).toBe(true);
    expect(DEFAULT_TRAINING_CONFIG.useReflexion).toBe(true);
    expect(DEFAULT_TRAINING_CONFIG.recordTrajectories).toBe(true);
    expect(DEFAULT_TRAINING_CONFIG.model).toBe("foundation-models");
  });
});
