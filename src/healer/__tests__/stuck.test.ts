/**
 * Tests for Stuck Task/Subtask Detection
 */
import { describe, test, expect } from "bun:test";
import {
  isTaskStuckByTime,
  isSubtaskStuck,
  extractFailurePatterns,
  scanTasksForStuck,
  scanSubtasksForStuck,
  detectStuck,
  summarizeStuckDetection,
} from "../stuck.js";
import type { Task } from "../../tasks/schema.js";
import type { Subtask } from "../../agent/orchestrator/types.js";
import type { Trajectory, Step } from "../../atif/schema.js";

// ============================================================================
// Test Helpers
// ============================================================================

const createMockTask = (overrides: Partial<Task> = {}): Task => ({
  id: `task-${Date.now()}`,
  title: "Test Task",
  description: "A test task",
  status: "in_progress",
  priority: 2,
  type: "task",
  labels: [],
  deps: [],
  commits: [],
  createdAt: new Date().toISOString(),
  updatedAt: new Date().toISOString(),
  closedAt: null,
  ...overrides,
});

const createMockSubtask = (overrides: Partial<Subtask> = {}): Subtask => ({
  id: `subtask-${Date.now()}`,
  description: "Test subtask",
  status: "in_progress",
  startedAt: new Date().toISOString(),
  ...overrides,
});

const createMockTrajectory = (steps: Step[]): Trajectory => ({
  schema_version: "ATIF-v1.4",
  session_id: `session-${Date.now()}`,
  agent: {
    name: "test-agent",
    version: "1.0.0",
    model_name: "test",
  },
  steps,
  final_metrics: {
    total_prompt_tokens: 0,
    total_completion_tokens: 0,
    total_steps: steps.length,
  },
});

// ============================================================================
// isTaskStuckByTime Tests
// ============================================================================

describe("isTaskStuckByTime", () => {
  test("returns not stuck for non-in_progress tasks", () => {
    const task = createMockTask({ status: "closed" });
    const result = isTaskStuckByTime(task);
    expect(result.stuck).toBe(false);
  });

  test("returns not stuck for recently updated task", () => {
    const task = createMockTask({
      updatedAt: new Date().toISOString(),
    });
    const result = isTaskStuckByTime(task);
    expect(result.stuck).toBe(false);
  });

  test("returns stuck for task updated over threshold hours ago", () => {
    const fiveHoursAgo = new Date(Date.now() - 5 * 60 * 60 * 1000);
    const task = createMockTask({
      updatedAt: fiveHoursAgo.toISOString(),
    });
    const result = isTaskStuckByTime(task, { stuckTaskThresholdHours: 4 });
    expect(result.stuck).toBe(true);
    expect(result.hoursStuck).toBeGreaterThanOrEqual(5);
  });

  test("respects custom threshold", () => {
    const twoHoursAgo = new Date(Date.now() - 2 * 60 * 60 * 1000);
    const task = createMockTask({
      updatedAt: twoHoursAgo.toISOString(),
    });

    // Not stuck with 4 hour threshold
    expect(isTaskStuckByTime(task, { stuckTaskThresholdHours: 4 }).stuck).toBe(false);

    // Stuck with 1 hour threshold
    expect(isTaskStuckByTime(task, { stuckTaskThresholdHours: 1 }).stuck).toBe(true);
  });
});

// ============================================================================
// isSubtaskStuck Tests
// ============================================================================

describe("isSubtaskStuck", () => {
  test("returns not stuck for completed subtask", () => {
    const subtask = createMockSubtask({ status: "done" });
    const result = isSubtaskStuck(subtask);
    expect(result.stuck).toBe(false);
  });

  test("returns stuck for subtask with many failures", () => {
    const subtask = createMockSubtask({
      status: "failed",
      failureCount: 5,
    });
    const result = isSubtaskStuck(subtask, { minConsecutiveFailures: 3 });
    expect(result.stuck).toBe(true);
    expect(result.reason).toBe("consecutive_failures");
  });

  test("returns stuck for subtask started long ago", () => {
    const threeHoursAgo = new Date(Date.now() - 3 * 60 * 60 * 1000);
    const subtask = createMockSubtask({
      startedAt: threeHoursAgo.toISOString(),
      failureCount: 0,
    });
    const result = isSubtaskStuck(subtask, { stuckSubtaskThresholdHours: 2 });
    expect(result.stuck).toBe(true);
    expect(result.reason).toBe("time_threshold_exceeded");
    expect(result.hoursStuck).toBeGreaterThanOrEqual(3);
  });

  test("failure count takes priority over time", () => {
    const oneHourAgo = new Date(Date.now() - 1 * 60 * 60 * 1000);
    const subtask = createMockSubtask({
      startedAt: oneHourAgo.toISOString(),
      failureCount: 5,
    });
    const result = isSubtaskStuck(subtask, {
      stuckSubtaskThresholdHours: 2,
      minConsecutiveFailures: 3,
    });
    expect(result.stuck).toBe(true);
    expect(result.reason).toBe("consecutive_failures");
  });
});

// ============================================================================
// extractFailurePatterns Tests
// ============================================================================

describe("extractFailurePatterns", () => {
  test("returns empty array for trajectories without errors", () => {
    const traj = createMockTrajectory([
      {
        step_id: 1,
        timestamp: new Date().toISOString(),
        source: "agent",
        message: "Task completed successfully",
      },
    ]);
    const patterns = extractFailurePatterns([traj]);
    expect(patterns).toHaveLength(0);
  });

  test("extracts error patterns from observation results", () => {
    const traj1 = createMockTrajectory([
      {
        step_id: 1,
        timestamp: new Date().toISOString(),
        source: "agent",
        message: "Running tests",
        observation: {
          results: [
            {
              source_call_id: "test-1",
              content: { error: "TypeError: Cannot read property 'foo' of undefined" },
            },
          ],
        },
      },
    ]);
    const traj2 = createMockTrajectory([
      {
        step_id: 1,
        timestamp: new Date().toISOString(),
        source: "agent",
        message: "Running tests",
        observation: {
          results: [
            {
              source_call_id: "test-2",
              content: { error: "TypeError: Cannot read property 'foo' of undefined" },
            },
          ],
        },
      },
    ]);
    const traj3 = createMockTrajectory([
      {
        step_id: 1,
        timestamp: new Date().toISOString(),
        source: "agent",
        message: "Running tests",
        observation: {
          results: [
            {
              source_call_id: "test-3",
              content: { error: "TypeError: Cannot read property 'foo' of undefined" },
            },
          ],
        },
      },
    ]);

    const patterns = extractFailurePatterns([traj1, traj2, traj3], { minConsecutiveFailures: 3 });
    expect(patterns.length).toBeGreaterThanOrEqual(1);
    expect(patterns[0].occurrences).toBeGreaterThanOrEqual(3);
  });

  test("extracts error patterns from step messages", () => {
    const traj = createMockTrajectory([
      {
        step_id: 1,
        timestamp: new Date().toISOString(),
        source: "agent",
        message: "Test failed: assertion error at line 42",
      },
      {
        step_id: 2,
        timestamp: new Date().toISOString(),
        source: "agent",
        message: "Test failed: assertion error at line 42",
      },
      {
        step_id: 3,
        timestamp: new Date().toISOString(),
        source: "agent",
        message: "Test failed: assertion error at line 42",
      },
    ]);

    const patterns = extractFailurePatterns([traj], { minConsecutiveFailures: 3 });
    expect(patterns.length).toBeGreaterThanOrEqual(1);
  });
});

// ============================================================================
// scanTasksForStuck Tests
// ============================================================================

describe("scanTasksForStuck", () => {
  test("returns empty array for no stuck tasks", () => {
    const tasks = [
      createMockTask({ status: "closed" }),
      createMockTask({ updatedAt: new Date().toISOString() }),
    ];
    const result = scanTasksForStuck(tasks);
    expect(result).toHaveLength(0);
  });

  test("finds stuck tasks", () => {
    const fiveHoursAgo = new Date(Date.now() - 5 * 60 * 60 * 1000);
    const tasks = [
      createMockTask({ id: "stuck-1", updatedAt: fiveHoursAgo.toISOString() }),
      createMockTask({ id: "not-stuck", updatedAt: new Date().toISOString() }),
    ];
    const result = scanTasksForStuck(tasks, { stuckTaskThresholdHours: 4 });
    expect(result).toHaveLength(1);
    expect(result[0].task.id).toBe("stuck-1");
  });
});

// ============================================================================
// scanSubtasksForStuck Tests
// ============================================================================

describe("scanSubtasksForStuck", () => {
  test("returns empty array for no stuck subtasks", () => {
    const subtasks = [
      { subtask: createMockSubtask({ status: "done" }), taskId: "task-1" },
    ];
    const result = scanSubtasksForStuck(subtasks);
    expect(result).toHaveLength(0);
  });

  test("finds stuck subtasks by failure count", () => {
    const subtasks = [
      { subtask: createMockSubtask({ id: "stuck-1", failureCount: 5, status: "failed" }), taskId: "task-1" },
      { subtask: createMockSubtask({ id: "not-stuck", failureCount: 1, status: "failed" }), taskId: "task-2" },
    ];
    const result = scanSubtasksForStuck(subtasks, { minConsecutiveFailures: 3 });
    expect(result).toHaveLength(1);
    expect(result[0].subtask.id).toBe("stuck-1");
  });
});

// ============================================================================
// detectStuck Tests
// ============================================================================

describe("detectStuck", () => {
  test("returns comprehensive result", () => {
    const fiveHoursAgo = new Date(Date.now() - 5 * 60 * 60 * 1000);
    const tasks = [createMockTask({ updatedAt: fiveHoursAgo.toISOString() })];
    const subtasks = [
      { subtask: createMockSubtask({ failureCount: 5, status: "failed" }), taskId: "task-1" },
    ];

    const result = detectStuck(tasks, subtasks, [], {
      stuckTaskThresholdHours: 4,
      minConsecutiveFailures: 3,
    });

    expect(result.stats.tasksScanned).toBe(1);
    expect(result.stats.subtasksScanned).toBe(1);
    expect(result.stats.stuckTaskCount).toBe(1);
    expect(result.stats.stuckSubtaskCount).toBe(1);
  });
});

// ============================================================================
// summarizeStuckDetection Tests
// ============================================================================

describe("summarizeStuckDetection", () => {
  test("summarizes no stuck items", () => {
    const result = detectStuck([], [], [], {});
    const summary = summarizeStuckDetection(result);
    expect(summary).toContain("No stuck items");
  });

  test("summarizes stuck items", () => {
    const fiveHoursAgo = new Date(Date.now() - 5 * 60 * 60 * 1000);
    const tasks = [createMockTask({ id: "task-123", title: "Stuck Task", updatedAt: fiveHoursAgo.toISOString() })];

    const result = detectStuck(tasks, [], [], { stuckTaskThresholdHours: 4 });
    const summary = summarizeStuckDetection(result);

    expect(summary).toContain("Stuck tasks (1)");
    expect(summary).toContain("task-123");
    expect(summary).toContain("Stuck Task");
  });
});
