import { describe, expect, test } from "bun:test";
import {
  isTBMessage,
  isTBRunComplete,
  isTBRunRequest,
  isTBRunStart,
  isTBRunHistory,
  isTBSuiteInfo,
  isTBTaskComplete,
  isTBTaskOutput,
  isTBTaskProgress,
  isTBTaskStart,
} from "./protocol.js";

describe("HUD TB type guards", () => {
  const baseRun = {
    runId: "run-1",
    suiteName: "suite",
    suiteVersion: "1.0.0",
  };

  test("detects TB run lifecycle messages", () => {
    expect(
      isTBRunStart({
        type: "tb_run_start",
        ...baseRun,
        totalTasks: 1,
        taskIds: ["task-1"],
        timestamp: "2025-01-01T00:00:00Z",
      } as any),
    ).toBe(true);
    expect(
      isTBRunComplete({
        type: "tb_run_complete",
        ...baseRun,
        passRate: 1,
        passed: 1,
        failed: 0,
        timeout: 0,
        error: 0,
        totalDurationMs: 1000,
      } as any),
    ).toBe(true);
    expect(
      isTBRunHistory({
        type: "tb_run_history",
        runs: [],
      } as any),
    ).toBe(true);
  });

  test("detects TB task messages", () => {
    expect(
      isTBTaskStart({
        type: "tb_task_start",
        runId: "run-1",
        taskId: "task-1",
        taskName: "Task 1",
        category: "cat",
        difficulty: "easy",
        taskIndex: 0,
        totalTasks: 1,
      } as any),
    ).toBe(true);
    expect(
      isTBTaskProgress({
        type: "tb_task_progress",
        runId: "run-1",
        taskId: "task-1",
        phase: "agent",
        elapsedMs: 5,
      } as any),
    ).toBe(true);
    expect(
      isTBTaskOutput({
        type: "tb_task_output",
        runId: "run-1",
        taskId: "task-1",
        text: "hello",
        source: "agent",
      } as any),
    ).toBe(true);
    expect(
      isTBTaskComplete({
        type: "tb_task_complete",
        runId: "run-1",
        taskId: "task-1",
        outcome: "success",
        durationMs: 10,
        turns: 2,
        tokens: 5,
      } as any),
    ).toBe(true);
  });

  test("detects TB suite info and run requests", () => {
    expect(
      isTBSuiteInfo({
        type: "tb_suite_info",
        name: "suite",
        version: "1.0.0",
        tasks: [],
      } as any),
    ).toBe(true);
    expect(
      isTBRunRequest({
        type: "tb_run_request",
        suitePath: "/tmp/suite.json",
        taskIds: ["a"],
        timeout: 1000,
        maxTurns: 5,
      } as any),
    ).toBe(true);
  });

  test("isTBMessage filters TB messages only", () => {
    const tbMsg = { type: "tb_run_start", ...baseRun, totalTasks: 0, taskIds: [], timestamp: "" } as any;
    const nonTbMsg = { type: "session_start", sessionId: "s1", timestamp: "" } as any;

    expect(isTBMessage(tbMsg)).toBe(true);
    expect(isTBMessage(nonTbMsg)).toBe(false);
  });
});
