import { describe, test, expect } from "bun:test";
import {
  MetricsCollector,
  computeSummary,
  createEmptyTokenUsage,
  addTokenUsage,
  createTiming,
  type TaskMetrics,
  type TurnTokenUsage,
} from "./metrics.js";

describe("metrics", () => {
  describe("createEmptyTokenUsage", () => {
    test("returns all zeros", () => {
      const usage = createEmptyTokenUsage();
      expect(usage.input).toBe(0);
      expect(usage.output).toBe(0);
      expect(usage.cacheRead).toBe(0);
      expect(usage.cacheWrite).toBe(0);
    });
  });

  describe("addTokenUsage", () => {
    test("adds two usages correctly", () => {
      const a: TurnTokenUsage = { input: 100, output: 50, cacheRead: 20, cacheWrite: 10 };
      const b: TurnTokenUsage = { input: 200, output: 100, cacheRead: 30, cacheWrite: 20 };

      const result = addTokenUsage(a, b);

      expect(result.input).toBe(300);
      expect(result.output).toBe(150);
      expect(result.cacheRead).toBe(50);
      expect(result.cacheWrite).toBe(30);
    });
  });

  describe("createTiming", () => {
    test("computes duration correctly", () => {
      const timing = createTiming(1000, 2500);
      expect(timing.startMs).toBe(1000);
      expect(timing.endMs).toBe(2500);
      expect(timing.durationMs).toBe(1500);
    });
  });

  describe("MetricsCollector", () => {
    test("collects turn metrics", () => {
      const collector = new MetricsCollector("task-1", "Test Task");

      // Simulate turn 1
      collector.startTurn();
      collector.recordTokenUsage({ input: 100, output: 50, cacheRead: 0, cacheWrite: 0 });
      collector.startToolCall("tc-1");
      collector.endToolCall("tc-1", "read", '{"path": "/file.ts"}', true);
      collector.endTurn(1);

      // Simulate turn 2
      collector.startTurn();
      collector.recordTokenUsage({ input: 150, output: 75, cacheRead: 10, cacheWrite: 5 });
      collector.startToolCall("tc-2");
      collector.endToolCall("tc-2", "edit", '{"path": "/file.ts"}', true);
      collector.endTurn(2);

      const metrics = collector.finalize("success", {
        typecheckPassed: true,
        testsPassed: true,
        verificationRan: true,
      });

      expect(metrics.taskId).toBe("task-1");
      expect(metrics.taskTitle).toBe("Test Task");
      expect(metrics.outcome).toBe("success");
      expect(metrics.turns.length).toBe(2);
      expect(metrics.totalTokenUsage.input).toBe(250);
      expect(metrics.totalTokenUsage.output).toBe(125);
      expect(metrics.toolCallSummary.totalCalls).toBe(2);
      expect(metrics.toolCallSummary.successfulCalls).toBe(2);
      expect(metrics.toolCallSummary.byTool["read"]).toBe(1);
      expect(metrics.toolCallSummary.byTool["edit"]).toBe(1);
    });

    test("tracks edit/write as dirty", () => {
      const collector = new MetricsCollector("task-2", "Edit Task");

      collector.startTurn();
      collector.startToolCall("tc-1");
      collector.endToolCall("tc-1", "edit", '{}', true);
      collector.endTurn(1);

      const metrics = collector.finalize("success", {
        typecheckPassed: true,
        testsPassed: true,
        verificationRan: true,
      });

      expect(metrics.turns[0].hasEdits).toBe(true);
    });

    test("detects retries", () => {
      const collector = new MetricsCollector("task-3", "Retry Task");

      collector.startTurn();
      collector.startToolCall("tc-1");
      collector.endToolCall("tc-1", "bash", '{"command": "test"}', false);
      collector.endTurn(1);

      // Same tool + args = retry
      collector.startTurn();
      collector.startToolCall("tc-2");
      collector.endToolCall("tc-2", "bash", '{"command": "test"}', true);
      collector.endTurn(2);

      const metrics = collector.finalize("success", {
        typecheckPassed: true,
        testsPassed: true,
        verificationRan: true,
      });

      expect(metrics.toolCallSummary.retryCalls).toBe(1);
      expect(metrics.turns[0].toolCalls[0].isRetry).toBe(false);
      expect(metrics.turns[1].toolCalls[0].isRetry).toBe(true);
    });

    test("records error message on failure", () => {
      const collector = new MetricsCollector("task-4", "Failed Task");

      const metrics = collector.finalize(
        "error",
        {
          typecheckPassed: false,
          testsPassed: false,
          verificationRan: false,
        },
        "Something went wrong",
      );

      expect(metrics.outcome).toBe("error");
      expect(metrics.errorMessage).toBe("Something went wrong");
    });
  });

  describe("computeSummary", () => {
    test("computes summary from tasks", () => {
      const tasks: TaskMetrics[] = [
        {
          taskId: "t1",
          taskTitle: "Task 1",
          outcome: "success",
          turns: [
            {
              turnNumber: 1,
              timing: { startMs: 0, endMs: 1000, durationMs: 1000 },
              tokenUsage: { input: 100, output: 50, cacheRead: 0, cacheWrite: 0 },
              toolCalls: [],
              hasEdits: false,
            },
          ],
          totalTiming: { startMs: 0, endMs: 1000, durationMs: 1000 },
          totalTokenUsage: { input: 100, output: 50, cacheRead: 0, cacheWrite: 0 },
          verification: { typecheckPassed: true, testsPassed: true, verificationRan: true },
          toolCallSummary: {
            totalCalls: 2,
            successfulCalls: 2,
            failedCalls: 0,
            retryCalls: 0,
            byTool: { read: 1, edit: 1 },
          },
        },
        {
          taskId: "t2",
          taskTitle: "Task 2",
          outcome: "failure",
          turns: [
            {
              turnNumber: 1,
              timing: { startMs: 0, endMs: 2000, durationMs: 2000 },
              tokenUsage: { input: 200, output: 100, cacheRead: 0, cacheWrite: 0 },
              toolCalls: [],
              hasEdits: true,
            },
          ],
          totalTiming: { startMs: 0, endMs: 2000, durationMs: 2000 },
          totalTokenUsage: { input: 200, output: 100, cacheRead: 0, cacheWrite: 0 },
          verification: { typecheckPassed: false, testsPassed: false, verificationRan: true },
          toolCallSummary: {
            totalCalls: 3,
            successfulCalls: 2,
            failedCalls: 1,
            retryCalls: 1,
            byTool: { read: 1, bash: 2 },
          },
        },
      ];

      const summary = computeSummary(tasks);

      expect(summary.totalTasks).toBe(2);
      expect(summary.successfulTasks).toBe(1);
      expect(summary.failedTasks).toBe(1);
      expect(summary.taskCompletionRate).toBe(0.5);
      expect(summary.verificationPassRate).toBe(0.5);
      expect(summary.avgTokensPerTask).toBe(225); // (150 + 300) / 2
      expect(summary.avgTurnsPerTask).toBe(1);
      expect(summary.avgToolCallsPerTask).toBe(2.5);
      expect(summary.totalDurationMs).toBe(3000);
      expect(summary.toolDistribution["read"]).toBe(2);
      expect(summary.toolDistribution["edit"]).toBe(1);
      expect(summary.toolDistribution["bash"]).toBe(2);
      expect(summary.retryRate).toBe(0.2); // 1 retry / 5 total calls
    });

    test("handles empty tasks array", () => {
      const summary = computeSummary([]);

      expect(summary.totalTasks).toBe(0);
      expect(summary.taskCompletionRate).toBe(0);
      expect(summary.verificationPassRate).toBe(0);
    });
  });
});
