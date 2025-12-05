import { describe, expect, it } from "bun:test";
import { computeRunSummary } from "./tbench-dashboard.js";
import type { BenchmarkResults } from "../bench/metrics.js";

const sampleResults: BenchmarkResults = {
  meta: {
    runId: "run-1",
    startedAt: "2025-12-05T00:00:00Z",
    completedAt: "2025-12-05T01:00:00Z",
    model: "test-model",
    projectId: "openagents",
  },
  tasks: [
    {
      taskId: "task-a",
      taskTitle: "Task A",
      outcome: "success",
      turns: [],
      totalTiming: { startMs: 0, endMs: 1, durationMs: 1 },
      totalTokenUsage: { input: 10, output: 5, cacheRead: 0, cacheWrite: 0 },
      verification: { typecheckPassed: true, testsPassed: true, verificationRan: true },
      toolCallSummary: {
        totalCalls: 1,
        successfulCalls: 1,
        failedCalls: 0,
        retryCalls: 0,
        byTool: { read: 1 },
      },
    },
    {
      taskId: "task-b",
      taskTitle: "Task B",
      outcome: "failure",
      turns: [],
      totalTiming: { startMs: 0, endMs: 2, durationMs: 2 },
      totalTokenUsage: { input: 20, output: 0, cacheRead: 0, cacheWrite: 0 },
      verification: { typecheckPassed: false, testsPassed: false, verificationRan: true },
      toolCallSummary: {
        totalCalls: 2,
        successfulCalls: 1,
        failedCalls: 1,
        retryCalls: 1,
        byTool: { edit: 2 },
      },
      errorMessage: "failed",
    },
  ],
  summary: {
    totalTasks: 2,
    successfulTasks: 1,
    failedTasks: 1,
    timeoutTasks: 0,
    errorTasks: 0,
    taskCompletionRate: 0.5,
    verificationPassRate: 0.5,
    avgTokensPerTask: 17.5,
    avgTurnsPerTask: 0,
    avgToolCallsPerTask: 1.5,
    totalDurationMs: 3,
    toolDistribution: { read: 1, edit: 2 },
    retryRate: 0.333,
  },
};

describe("tbench-dashboard", () => {
  it("computes run summary with category mapping", () => {
    const summary = computeRunSummary(sampleResults, { "task-a": "cat-1" });
    expect(summary.runId).toBe("run-1");
    expect(summary.successRate).toBeCloseTo(0.5);
    expect(summary.totalTokens).toBe(35);
    expect(summary.categoryStats["cat-1"]).toEqual({ success: 1, total: 1 });
    expect(summary.categoryStats["uncategorized"]).toEqual({ success: 0, total: 1 });
  });
});
