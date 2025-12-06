/**
 * SOAR Hindsight Relabeling Tests
 *
 * Tests for hindsight relabeling of failed attempts into synthetic tasks.
 */

import { describe, test, expect } from "bun:test";
import * as S from "effect/Schema";
import { Effect, Option } from "effect";
import {
  AttemptRecord,
  SyntheticTask,
  SyntheticTaskSolution,
  DEFAULT_HINDSIGHT_CONFIG,
  generateSyntheticDescription,
  isSuitableForRelabeling,
  createSyntheticTask,
  relabelAttempt,
  relabelBatch,
  HindsightService,
  HindsightServiceLive,
  makeHindsightServiceLayer,
} from "../soar-hindsight.js";
import { createMockAttemptRecord, createMockAttemptBatch, runEffect } from "./test-helpers.js";

describe("AttemptRecord Schema", () => {
  test("decodes valid attempt record", () => {
    const input = {
      id: "attempt-001",
      taskId: "task-001",
      taskDescription: "Compute fibonacci",
      code: "function fib(n) { return n; }",
      actualOutput: 10,
      success: false,
      trainingAccuracy: 0.3,
      tokensUsed: 500,
      durationMs: 2000,
      timestamp: new Date().toISOString(),
    };
    const decoded = S.decodeUnknownSync(AttemptRecord)(input);
    expect(decoded.id).toBe("attempt-001");
    expect(decoded.trainingAccuracy).toBe(0.3);
  });

  test("accepts optional fields", () => {
    const input = {
      id: "attempt-002",
      taskId: "task-002",
      taskDescription: "Test",
      code: "code",
      actualOutput: null,
      expectedOutput: 42,
      success: true,
      trainingAccuracy: 1.0,
      tokensUsed: 100,
      durationMs: 1000,
      timestamp: new Date().toISOString(),
      skillsUsed: ["skill-001", "skill-002"],
    };
    const decoded = S.decodeUnknownSync(AttemptRecord)(input);
    expect(decoded.expectedOutput).toBe(42);
    expect(decoded.skillsUsed).toEqual(["skill-001", "skill-002"]);
  });
});

describe("SyntheticTask Schema", () => {
  test("decodes valid synthetic task", () => {
    const input = {
      id: "synthetic-001",
      description: "[Hindsight] Produces output",
      input: { n: 5 },
      output: 55,
      originalTaskId: "task-001",
      attemptId: "attempt-001",
      confidence: 0.45,
      validated: false,
      createdAt: new Date().toISOString(),
    };
    const decoded = S.decodeUnknownSync(SyntheticTask)(input);
    expect(decoded.id).toBe("synthetic-001");
    expect(decoded.confidence).toBe(0.45);
  });
});

describe("SyntheticTaskSolution Schema", () => {
  test("decodes valid synthetic task-solution", () => {
    const input = {
      task: {
        id: "synthetic-001",
        description: "[Hindsight] Test",
        input: {},
        output: 42,
        originalTaskId: "task-001",
        attemptId: "attempt-001",
        confidence: 0.5,
        validated: true,
        createdAt: new Date().toISOString(),
      },
      solution: "function solve() { return 42; }",
      source: "hindsight",
      qualityScore: 0.5,
    };
    const decoded = S.decodeUnknownSync(SyntheticTaskSolution)(input);
    expect(decoded.source).toBe("hindsight");
    expect(decoded.qualityScore).toBe(0.5);
  });
});

describe("generateSyntheticDescription", () => {
  test("generates description for string output", () => {
    const desc = generateSyntheticDescription("Original task", "hello world");
    expect(desc).toContain("[Hindsight]");
    expect(desc).toContain("Original task");
    expect(desc).toContain("hello world");
  });

  test("generates description for object output", () => {
    const desc = generateSyntheticDescription("Compute values", { a: 1, b: 2 });
    expect(desc).toContain("[Hindsight]");
    expect(desc).toContain('{"a":1,"b":2}');
  });

  test("truncates long descriptions", () => {
    const longOutput = "x".repeat(200);
    const desc = generateSyntheticDescription("A".repeat(100), longOutput);
    // Original description truncated to 50 chars, output to 100
    expect(desc.length).toBeLessThan(250);
  });

  test("handles null output", () => {
    const desc = generateSyntheticDescription("Test", null);
    expect(desc).toContain("null");
  });
});

describe("isSuitableForRelabeling", () => {
  test("rejects successful attempts", () => {
    const attempt = createMockAttemptRecord({ success: true, trainingAccuracy: 1.0 });
    expect(isSuitableForRelabeling(attempt)).toBe(false);
  });

  test("rejects attempts with too low accuracy", () => {
    const attempt = createMockAttemptRecord({ success: false, trainingAccuracy: 0.005 });
    expect(isSuitableForRelabeling(attempt)).toBe(false);
  });

  test("rejects attempts with too high accuracy", () => {
    const attempt = createMockAttemptRecord({ success: false, trainingAccuracy: 0.995 });
    expect(isSuitableForRelabeling(attempt)).toBe(false);
  });

  test("rejects attempts with short code", () => {
    const attempt = createMockAttemptRecord({ success: false, trainingAccuracy: 0.5, code: "x=1" });
    expect(isSuitableForRelabeling(attempt)).toBe(false);
  });

  test("rejects attempts with null output", () => {
    const attempt = createMockAttemptRecord({ success: false, trainingAccuracy: 0.5, actualOutput: null });
    expect(isSuitableForRelabeling(attempt)).toBe(false);
  });

  test("rejects attempts with undefined output", () => {
    const attempt = createMockAttemptRecord({ success: false, trainingAccuracy: 0.5, actualOutput: undefined });
    expect(isSuitableForRelabeling(attempt)).toBe(false);
  });

  test("accepts suitable failed attempts", () => {
    const attempt = createMockAttemptRecord({
      success: false,
      trainingAccuracy: 0.5,
      code: "function solve() { return 42; }",
      actualOutput: 42,
    });
    expect(isSuitableForRelabeling(attempt)).toBe(true);
  });

  test("accepts at exact boundaries", () => {
    const atMinAccuracy = createMockAttemptRecord({
      success: false,
      trainingAccuracy: 0.01,
      code: "function x() { return 1; }",
      actualOutput: 1,
    });
    expect(isSuitableForRelabeling(atMinAccuracy)).toBe(true);

    const atMaxAccuracy = createMockAttemptRecord({
      success: false,
      trainingAccuracy: 0.99,
      code: "function x() { return 1; }",
      actualOutput: 1,
    });
    expect(isSuitableForRelabeling(atMaxAccuracy)).toBe(true);
  });
});

describe("createSyntheticTask", () => {
  test("creates synthetic task with correct fields", () => {
    const attempt = createMockAttemptRecord({
      id: "attempt-123",
      taskId: "task-456",
      taskDescription: "Original description",
      actualOutput: { result: 42 },
      trainingAccuracy: 0.6,
    });
    const synthetic = createSyntheticTask(attempt);

    expect(synthetic.id).toBe("synthetic-attempt-123");
    expect(synthetic.originalTaskId).toBe("task-456");
    expect(synthetic.attemptId).toBe("attempt-123");
    expect(synthetic.output).toEqual({ result: 42 });
    expect(synthetic.confidence).toBe(0.6);
    expect(synthetic.validated).toBe(false);
  });

  test("includes timestamp", () => {
    const attempt = createMockAttemptRecord();
    const synthetic = createSyntheticTask(attempt);
    expect(() => new Date(synthetic.createdAt)).not.toThrow();
  });
});

describe("relabelAttempt", () => {
  test("returns None for unsuitable attempts", () => {
    const attempt = createMockAttemptRecord({ success: true });
    const result = relabelAttempt(attempt);
    expect(Option.isNone(result)).toBe(true);
  });

  test("returns Some for suitable attempts", () => {
    const attempt = createMockAttemptRecord({
      success: false,
      trainingAccuracy: 0.5,
      code: "function solve() { return 42; }",
      actualOutput: 42,
    });
    const result = relabelAttempt(attempt);

    expect(Option.isSome(result)).toBe(true);
    if (Option.isSome(result)) {
      expect(result.value.task.attemptId).toBe(attempt.id);
      expect(result.value.solution).toBe(attempt.code);
      expect(result.value.source).toBe("hindsight");
      expect(result.value.qualityScore).toBe(attempt.trainingAccuracy);
    }
  });
});

describe("relabelBatch", () => {
  test("returns empty for empty batch", () => {
    const results = relabelBatch([]);
    expect(results).toEqual([]);
  });

  test("filters unsuitable attempts", () => {
    const attempts = [
      createMockAttemptRecord({ success: true, taskId: "task-1" }),
      createMockAttemptRecord({ success: false, trainingAccuracy: 0.5, actualOutput: 42, taskId: "task-2" }),
      createMockAttemptRecord({ success: false, trainingAccuracy: 0.001, taskId: "task-3" }),
    ];
    const results = relabelBatch(attempts);
    expect(results).toHaveLength(1);
  });

  test("groups by task and respects maxSyntheticPerTask", () => {
    // Create 60 attempts for same task
    const attempts = Array.from({ length: 60 }, (_, i) =>
      createMockAttemptRecord({
        id: `attempt-${i}`,
        taskId: "same-task",
        success: false,
        trainingAccuracy: 0.3 + i * 0.01,
        actualOutput: i,
      }),
    );
    const results = relabelBatch(attempts);
    // Should cap at maxSyntheticPerTask (default 50)
    expect(results.length).toBeLessThanOrEqual(50);
  });

  test("sorts by accuracy and takes highest", () => {
    const attempts = [
      createMockAttemptRecord({ id: "low", taskId: "task", trainingAccuracy: 0.2, actualOutput: 1 }),
      createMockAttemptRecord({ id: "high", taskId: "task", trainingAccuracy: 0.8, actualOutput: 2 }),
      createMockAttemptRecord({ id: "mid", taskId: "task", trainingAccuracy: 0.5, actualOutput: 3 }),
    ];
    const results = relabelBatch(attempts, { ...DEFAULT_HINDSIGHT_CONFIG, maxSyntheticPerTask: 2 });
    expect(results).toHaveLength(2);
    // First should be highest accuracy
    expect(results[0]?.qualityScore).toBe(0.8);
    expect(results[1]?.qualityScore).toBe(0.5);
  });
});

describe("HindsightService", () => {
  test("isSuitable checks attempt", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* HindsightService;
        const suitable = createMockAttemptRecord({ success: false, trainingAccuracy: 0.5, actualOutput: 42 });
        const unsuitable = createMockAttemptRecord({ success: true });
        return {
          suitable: yield* service.isSuitable(suitable),
          unsuitable: yield* service.isSuitable(unsuitable),
        };
      }).pipe(Effect.provide(HindsightServiceLive)),
    );

    expect(result.suitable).toBe(true);
    expect(result.unsuitable).toBe(false);
  });

  test("relabel returns Option", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* HindsightService;
        const attempt = createMockAttemptRecord({ success: false, trainingAccuracy: 0.5, actualOutput: 42 });
        return yield* service.relabel(attempt);
      }).pipe(Effect.provide(HindsightServiceLive)),
    );

    expect(Option.isSome(result)).toBe(true);
  });

  test("relabelBatch processes multiple", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* HindsightService;
        const attempts = createMockAttemptBatch(10, 0.4, 0.3);
        return yield* service.relabelBatch(attempts);
      }).pipe(Effect.provide(HindsightServiceLive)),
    );

    expect(result.length).toBeGreaterThan(0);
  });

  test("getStats tracks processing", () => {
    // Use fresh layer to avoid stats accumulation from other tests
    const freshLayer = makeHindsightServiceLayer();
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* HindsightService;
        const attempts = createMockAttemptBatch(10, 0.5, 0.2);
        yield* service.relabelBatch(attempts);
        return yield* service.getStats();
      }).pipe(Effect.provide(freshLayer)),
    );

    expect(result.totalAttemptsProcessed).toBe(10);
    expect(result.totalSyntheticCreated).toBeGreaterThan(0);
    expect(result.relabelingRate).toBeGreaterThan(0);
  });

  test("updateConfig modifies config", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* HindsightService;
        return yield* service.updateConfig({ minTrainingAccuracy: 0.1 });
      }).pipe(Effect.provide(HindsightServiceLive)),
    );

    expect(result.minTrainingAccuracy).toBe(0.1);
  });

  test("custom config layer", () => {
    const customLayer = makeHindsightServiceLayer({ maxSyntheticPerTask: 10 });
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* HindsightService;
        return yield* service.getConfig();
      }).pipe(Effect.provide(customLayer)),
    );

    expect(result.maxSyntheticPerTask).toBe(10);
  });
});
