import { describe, test, expect } from "bun:test";
import { Effect } from "effect";
import {
  buildTerminalBenchReport,
  compareRuns,
  formatMarkdownReport,
  formatRunSummary,
  formatTerminalBenchMarkdown,
} from "./reporter.js";
import type { BenchmarkResults, TaskMetrics } from "./metrics.js";
import type { TerminalBenchResults, TerminalBenchSuite } from "./terminal-bench.js";

const createMockTaskMetrics = (
  id: string,
  title: string,
  outcome: "success" | "failure" | "timeout" | "error",
  turns: number = 1,
  tokens: number = 100,
  duration: number = 1000,
): TaskMetrics => ({
  taskId: id,
  taskTitle: title,
  outcome,
  turns: Array.from({ length: turns }, (_, i) => ({
    turnNumber: i + 1,
    timing: { startMs: 0, endMs: duration / turns, durationMs: duration / turns },
    tokenUsage: { input: tokens / 2 / turns, output: tokens / 2 / turns, cacheRead: 0, cacheWrite: 0 },
    toolCalls: [],
    hasEdits: false,
  })),
  totalTiming: { startMs: 0, endMs: duration, durationMs: duration },
  totalTokenUsage: { input: tokens / 2, output: tokens / 2, cacheRead: 0, cacheWrite: 0 },
  verification: {
    typecheckPassed: outcome === "success",
    testsPassed: outcome === "success",
    verificationRan: true,
  },
  toolCallSummary: {
    totalCalls: turns,
    successfulCalls: outcome === "success" ? turns : 0,
    failedCalls: outcome === "success" ? 0 : turns,
    retryCalls: 0,
    byTool: { read: turns },
  },
});

const createMockResults = (
  runId: string,
  tasks: TaskMetrics[],
): BenchmarkResults => {
  const successfulTasks = tasks.filter((t) => t.outcome === "success").length;
  const failedTasks = tasks.filter((t) => t.outcome === "failure").length;
  const timeoutTasks = tasks.filter((t) => t.outcome === "timeout").length;
  const errorTasks = tasks.filter((t) => t.outcome === "error").length;

  const verificationRan = tasks.filter((t) => t.verification.verificationRan);
  const verificationPassed = verificationRan.filter(
    (t) => t.verification.typecheckPassed && t.verification.testsPassed,
  );

  const totalTokens = tasks.reduce(
    (sum, t) => sum + t.totalTokenUsage.input + t.totalTokenUsage.output,
    0,
  );
  const totalTurns = tasks.reduce((sum, t) => sum + t.turns.length, 0);
  const totalToolCalls = tasks.reduce((sum, t) => sum + t.toolCallSummary.totalCalls, 0);

  return {
    meta: {
      runId,
      startedAt: "2025-01-01T00:00:00Z",
      completedAt: "2025-01-01T01:00:00Z",
      model: "test-model",
      projectId: "test-project",
    },
    tasks,
    summary: {
      totalTasks: tasks.length,
      successfulTasks,
      failedTasks,
      timeoutTasks,
      errorTasks,
      taskCompletionRate: tasks.length > 0 ? successfulTasks / tasks.length : 0,
      verificationPassRate:
        verificationRan.length > 0 ? verificationPassed.length / verificationRan.length : 0,
      avgTokensPerTask: tasks.length > 0 ? totalTokens / tasks.length : 0,
      avgTurnsPerTask: tasks.length > 0 ? totalTurns / tasks.length : 0,
      avgToolCallsPerTask: tasks.length > 0 ? totalToolCalls / tasks.length : 0,
      totalDurationMs: tasks.reduce((sum, t) => sum + t.totalTiming.durationMs, 0),
      toolDistribution: { read: totalToolCalls },
      retryRate: 0,
    },
  };
};

describe("reporter", () => {
  describe("compareRuns", () => {
    test("detects improvement when failure becomes success", async () => {
      const baseline = createMockResults("baseline-1", [
        createMockTaskMetrics("t1", "Task 1", "failure"),
      ]);
      const current = createMockResults("current-1", [
        createMockTaskMetrics("t1", "Task 1", "success"),
      ]);

      const report = await Effect.runPromise(compareRuns(baseline, current));

      expect(report.overallVerdict).toBe("improved");
      expect(report.taskComparisons[0].improved).toBe(true);
      expect(report.taskComparisons[0].regressed).toBe(false);
    });

    test("detects regression when success becomes failure", async () => {
      const baseline = createMockResults("baseline-1", [
        createMockTaskMetrics("t1", "Task 1", "success"),
      ]);
      const current = createMockResults("current-1", [
        createMockTaskMetrics("t1", "Task 1", "failure"),
      ]);

      const report = await Effect.runPromise(compareRuns(baseline, current));

      expect(report.overallVerdict).toBe("regressed");
      expect(report.taskComparisons[0].improved).toBe(false);
      expect(report.taskComparisons[0].regressed).toBe(true);
    });

    test("detects unchanged when outcomes stay the same", async () => {
      const baseline = createMockResults("baseline-1", [
        createMockTaskMetrics("t1", "Task 1", "success"),
      ]);
      const current = createMockResults("current-1", [
        createMockTaskMetrics("t1", "Task 1", "success"),
      ]);

      const report = await Effect.runPromise(compareRuns(baseline, current));

      expect(report.overallVerdict).toBe("unchanged");
      expect(report.taskComparisons[0].outcomeChanged).toBe(false);
    });

    test("detects mixed when some improve and some regress", async () => {
      const baseline = createMockResults("baseline-1", [
        createMockTaskMetrics("t1", "Task 1", "success"),
        createMockTaskMetrics("t2", "Task 2", "failure"),
      ]);
      const current = createMockResults("current-1", [
        createMockTaskMetrics("t1", "Task 1", "failure"),
        createMockTaskMetrics("t2", "Task 2", "success"),
      ]);

      const report = await Effect.runPromise(compareRuns(baseline, current));

      expect(report.overallVerdict).toBe("mixed");
    });

    test("handles new tasks not in baseline", async () => {
      const baseline = createMockResults("baseline-1", [
        createMockTaskMetrics("t1", "Task 1", "success"),
      ]);
      const current = createMockResults("current-1", [
        createMockTaskMetrics("t1", "Task 1", "success"),
        createMockTaskMetrics("t2", "Task 2", "success"),
      ]);

      const report = await Effect.runPromise(compareRuns(baseline, current));

      expect(report.taskComparisons.length).toBe(2);
      const newTask = report.taskComparisons.find((t) => t.taskId === "t2");
      expect(newTask?.baselineOutcome).toBe("N/A");
      expect(newTask?.improved).toBe(true);
    });

    test("computes correct deltas", async () => {
      const baseline = createMockResults("baseline-1", [
        createMockTaskMetrics("t1", "Task 1", "success", 2, 200, 2000),
      ]);
      const current = createMockResults("current-1", [
        createMockTaskMetrics("t1", "Task 1", "success", 1, 100, 1000),
      ]);

      const report = await Effect.runPromise(compareRuns(baseline, current));

      const task = report.taskComparisons[0];
      expect(task.turnsDelta).toBe(-1);
      expect(task.tokensDelta).toBe(-100);
      expect(task.durationDelta).toBe(-1000);
    });
  });

  describe("formatMarkdownReport", () => {
    test("generates valid markdown", async () => {
      const baseline = createMockResults("baseline-1", [
        createMockTaskMetrics("t1", "Task 1", "failure"),
      ]);
      const current = createMockResults("current-1", [
        createMockTaskMetrics("t1", "Task 1", "success"),
      ]);

      const report = await Effect.runPromise(compareRuns(baseline, current));
      const markdown = formatMarkdownReport(report);

      expect(markdown).toContain("# Benchmark Comparison Report");
      expect(markdown).toContain("## Runs Compared");
      expect(markdown).toContain("## Overall Verdict");
      expect(markdown).toContain("IMPROVED");
      expect(markdown).toContain("## Summary Metrics");
      expect(markdown).toContain("taskCompletionRate");
    });

    test("includes task outcome changes section", async () => {
      const baseline = createMockResults("baseline-1", [
        createMockTaskMetrics("t1", "Test Task", "failure"),
      ]);
      const current = createMockResults("current-1", [
        createMockTaskMetrics("t1", "Test Task", "success"),
      ]);

      const report = await Effect.runPromise(compareRuns(baseline, current));
      const markdown = formatMarkdownReport(report);

      expect(markdown).toContain("## Task Outcome Changes");
      expect(markdown).toContain("Test Task");
      expect(markdown).toContain("Improved");
    });
  });

  describe("formatRunSummary", () => {
    test("generates valid summary markdown", () => {
      const results = createMockResults("run-1", [
        createMockTaskMetrics("t1", "Task 1", "success"),
        createMockTaskMetrics("t2", "Task 2", "failure"),
      ]);

      const markdown = formatRunSummary(results);

      expect(markdown).toContain("# Benchmark Run Summary");
      expect(markdown).toContain("run-1");
      expect(markdown).toContain("test-model");
      expect(markdown).toContain("## Summary");
      expect(markdown).toContain("Total Tasks | 2");
      expect(markdown).toContain("Successful | 1");
      expect(markdown).toContain("Failed | 1");
      expect(markdown).toContain("## Tool Distribution");
      expect(markdown).toContain("## Task Results");
      expect(markdown).toContain("Task 1");
      expect(markdown).toContain("Task 2");
    });

    test("includes git info when present", () => {
      const baseResults = createMockResults("run-1", [
        createMockTaskMetrics("t1", "Task 1", "success"),
      ]);
      const results: BenchmarkResults = {
        ...baseResults,
        meta: {
          ...baseResults.meta,
          gitBranch: "main",
          gitCommit: "abc123",
        },
      };

      const markdown = formatRunSummary(results);

      expect(markdown).toContain("**Branch:** main");
      expect(markdown).toContain("**Commit:** abc123");
    });
  });
});

describe("terminal-bench reporting", () => {
  const suite: TerminalBenchSuite = {
    name: "TB Suite",
    version: "1.0",
    description: "Sample suite",
    tasks: [
      {
        id: "task-1",
        name: "Algo One",
        description: "Algo task",
        difficulty: "easy",
        category: "algorithms",
        verification: { type: "test" },
      },
      {
        id: "task-2",
        name: "Algo Two",
        description: "Algo task 2",
        difficulty: "medium",
        category: "algorithms",
        verification: { type: "test" },
      },
      {
        id: "task-3",
        name: "Web Task",
        description: "Web task",
        difficulty: "medium",
        category: "web",
        verification: { type: "test" },
      },
    ],
  };

  const tbResults: TerminalBenchResults = {
    suite_name: "TB Suite",
    suite_version: "1.0",
    model: "gpt-x",
    timestamp: "2025-01-01T00:00:00Z",
    results: [
      { task_id: "task-1", status: "pass", duration_ms: 1000, turns: 2, tokens_used: 300 },
      { task_id: "task-2", status: "timeout", duration_ms: 2000, turns: 1, tokens_used: 200 },
      { task_id: "task-3", status: "fail", duration_ms: 1500, turns: 3, tokens_used: 400 },
      { task_id: "unknown-task", status: "skip", duration_ms: 0, turns: 0, tokens_used: 0 },
    ],
    summary: {
      total: 4,
      passed: 1,
      failed: 1,
      timeout: 1,
      error: 0,
      skipped: 1,
      pass_rate: 0.25,
      avg_duration_ms: 1125,
      avg_turns: 1.5,
      total_tokens: 900,
    },
  };

  test("buildTerminalBenchReport aggregates per category", () => {
    const report = buildTerminalBenchReport(suite, tbResults);
    expect(report.overall.total).toBe(4);
    expect(report.overall.passed).toBe(1);
    expect(report.overall.failed).toBe(1);
    expect(report.overall.timeout).toBe(1);
    expect(report.overall.skipped).toBe(1);
    expect(report.overall.totalTokens).toBe(900);

    const algorithms = report.categories.find((c) => c.category === "algorithms");
    expect(algorithms?.total).toBe(2);
    expect(algorithms?.passed).toBe(1);
    expect(algorithms?.timeout).toBe(1);
    expect(algorithms?.passRate).toBeCloseTo(0.5);
    expect(algorithms?.avgDurationMs).toBeCloseTo(1500);
    expect(algorithms?.totalTokens).toBe(500);

    const web = report.categories.find((c) => c.category === "web");
    expect(web?.total).toBe(1);
    expect(web?.failed).toBe(1);
    expect(web?.passRate).toBe(0);
    expect(web?.avgTurns).toBe(3);

    const uncategorized = report.categories.find((c) => c.category === "uncategorized");
    expect(uncategorized?.total).toBe(1);
    expect(uncategorized?.skipped).toBe(1);
  });

  test("formatTerminalBenchMarkdown renders summary and categories", () => {
    const report = buildTerminalBenchReport(suite, tbResults);
    const markdown = formatTerminalBenchMarkdown(report);
    expect(markdown).toContain("# Terminal-Bench Report");
    expect(markdown).toContain("TB Suite");
    expect(markdown).toContain("Model");
    expect(markdown).toContain("Overall Summary");
    expect(markdown).toContain("By Category");
    expect(markdown).toContain("algorithms");
    expect(markdown).toContain("uncategorized");
  });
});
