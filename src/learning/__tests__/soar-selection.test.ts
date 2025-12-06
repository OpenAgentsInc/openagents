/**
 * SOAR Greedy-Diverse Selection Tests
 *
 * Tests for selecting training examples from synthetic task-solutions.
 */

import { describe, test, expect } from "bun:test";
import * as S from "effect/Schema";
import { Effect } from "effect";
import {
  SelectionResult,
  DEFAULT_SELECTION_CONFIG,
  selectTop,
  selectBottom,
  selectGreedyDiverse,
  groupByTask,
  selectWithTaskBalance,
  SelectionService,
  SelectionServiceLive,
  makeSelectionServiceLayer,
} from "../soar-selection.js";
import { createMockSyntheticTaskSolution, createMockSyntheticBatch, runEffect } from "./test-helpers.js";
import type { SyntheticTaskSolution } from "../soar-hindsight.js";

// Helper to create synthetic with specific quality score and task
const createSynthetic = (
  id: string,
  qualityScore: number,
  taskId: string = "task-1",
  code: string = "default code",
): SyntheticTaskSolution => ({
  task: {
    id,
    description: "Test",
    input: {},
    output: id,
    originalTaskId: taskId,
    attemptId: id,
    confidence: qualityScore,
    validated: true,
    createdAt: new Date().toISOString(),
  },
  solution: code,
  source: "hindsight",
  qualityScore,
});

describe("SelectionResult Schema", () => {
  test("decodes valid result", () => {
    const input = {
      topExamples: [{ synthetic: {}, selectionScore: 0.8, rank: 1 }],
      bottomExamples: [],
      totalCandidates: 10,
      selectedAt: new Date().toISOString(),
    };
    const decoded = S.decodeUnknownSync(SelectionResult)(input);
    expect(decoded.totalCandidates).toBe(10);
  });
});

describe("selectTop", () => {
  test("returns empty for empty candidates", () => {
    const result = selectTop([]);
    expect(result).toEqual([]);
  });

  test("returns fewer than topK when not enough candidates", () => {
    const candidates = [
      createSynthetic("1", 0.5),
      createSynthetic("2", 0.6),
    ];
    const result = selectTop(candidates);
    expect(result).toHaveLength(2);
  });

  test("respects topK limit", () => {
    const candidates = Array.from({ length: 50 }, (_, i) =>
      createSynthetic(`${i}`, 0.3 + i * 0.01),
    );
    const result = selectTop(candidates, { ...DEFAULT_SELECTION_CONFIG, topK: 10 });
    expect(result).toHaveLength(10);
  });

  test("filters by minQualityScore", () => {
    const candidates = [
      createSynthetic("low", 0.005),
      createSynthetic("ok", 0.5),
    ];
    const result = selectTop(candidates);
    expect(result).toHaveLength(1);
    expect(result[0]?.synthetic.task.id).toBe("ok");
  });

  test("assigns ranks starting from 1", () => {
    const candidates = Array.from({ length: 5 }, (_, i) =>
      createSynthetic(`${i}`, 0.5 + i * 0.1),
    );
    const result = selectTop(candidates, { ...DEFAULT_SELECTION_CONFIG, topK: 5 });

    expect(result[0]?.rank).toBe(1);
    expect(result[4]?.rank).toBe(5);
  });

  test("uses greedy-diverse selection when enabled", () => {
    // Create candidates with same quality but different code patterns
    const candidates = [
      createSynthetic("loop", 0.5, "task", "for (let i = 0; i < n; i++) { sum += i; }"),
      createSynthetic("recursive", 0.5, "task", "function f(n) { return n <= 0 ? 0 : f(n-1) + n; }"),
      createSynthetic("loop2", 0.5, "task", "for (let j = 0; j < m; j++) { total += j; }"),
    ];
    const result = selectTop(candidates, { ...DEFAULT_SELECTION_CONFIG, topK: 3, enableDiversityBonus: true });

    // Should select all 3 with diversity bonus affecting order
    expect(result).toHaveLength(3);
  });
});

describe("selectBottom", () => {
  test("returns empty for empty candidates", () => {
    const result = selectBottom([]);
    expect(result).toEqual([]);
  });

  test("filters by maxBottomQualityScore", () => {
    const candidates = [
      createSynthetic("high", 0.8),
      createSynthetic("low", 0.2),
    ];
    const result = selectBottom(candidates);
    // Only low quality should be included (< 0.5)
    expect(result).toHaveLength(1);
    expect(result[0]?.synthetic.task.id).toBe("low");
  });

  test("prioritizes diversity in bottom selection", () => {
    const candidates = [
      createSynthetic("a", 0.1, "task", "for (;;) {}"),
      createSynthetic("b", 0.15, "task", "while (true) {}"),
      createSynthetic("c", 0.2, "task", "function recurse() { recurse(); }"),
    ];
    const result = selectBottom(candidates, { ...DEFAULT_SELECTION_CONFIG, bottomK: 3 });

    // All should be selected as they represent different failure modes
    expect(result).toHaveLength(3);
  });

  test("respects minQualityScore", () => {
    const candidates = [
      createSynthetic("too-low", 0.005),
      createSynthetic("ok", 0.2),
    ];
    const result = selectBottom(candidates);
    expect(result).toHaveLength(1);
  });
});

describe("selectGreedyDiverse", () => {
  test("returns both top and bottom examples", () => {
    const candidates = Array.from({ length: 100 }, (_, i) =>
      createSynthetic(`${i}`, 0.02 + i * 0.01),
    );
    const result = selectGreedyDiverse(candidates);

    expect(result.topExamples.length).toBeGreaterThan(0);
    expect(result.bottomExamples.length).toBeGreaterThan(0);
    expect(result.totalCandidates).toBe(100);
  });

  test("includes timestamp", () => {
    const result = selectGreedyDiverse([createSynthetic("1", 0.5)]);
    expect(() => new Date(result.selectedAt)).not.toThrow();
  });
});

describe("groupByTask", () => {
  test("groups candidates by task ID", () => {
    const candidates = [
      createSynthetic("1", 0.5, "task-a"),
      createSynthetic("2", 0.6, "task-a"),
      createSynthetic("3", 0.7, "task-b"),
    ];
    const groups = groupByTask(candidates);

    expect(groups.size).toBe(2);
    expect(groups.get("task-a")).toHaveLength(2);
    expect(groups.get("task-b")).toHaveLength(1);
  });

  test("handles empty input", () => {
    const groups = groupByTask([]);
    expect(groups.size).toBe(0);
  });
});

describe("selectWithTaskBalance", () => {
  test("returns empty for empty candidates", () => {
    const result = selectWithTaskBalance([]);
    expect(result.topExamples).toEqual([]);
    expect(result.bottomExamples).toEqual([]);
  });

  test("balances across tasks", () => {
    const candidates = [
      // Task A: 10 high quality
      ...Array.from({ length: 10 }, (_, i) =>
        createSynthetic(`a-${i}`, 0.8 + i * 0.01, "task-a"),
      ),
      // Task B: 10 medium quality
      ...Array.from({ length: 10 }, (_, i) =>
        createSynthetic(`b-${i}`, 0.5 + i * 0.01, "task-b"),
      ),
    ];
    const config = { ...DEFAULT_SELECTION_CONFIG, topK: 6 };
    const result = selectWithTaskBalance(candidates, config);

    // Should have representation from both tasks
    const taskACount = result.topExamples.filter(
      (e) => (e.synthetic as SyntheticTaskSolution).task.originalTaskId === "task-a",
    ).length;
    const taskBCount = result.topExamples.filter(
      (e) => (e.synthetic as SyntheticTaskSolution).task.originalTaskId === "task-b",
    ).length;

    expect(taskACount).toBeGreaterThan(0);
    expect(taskBCount).toBeGreaterThan(0);
  });

  test("re-ranks across all tasks", () => {
    const candidates = [
      createSynthetic("a-1", 0.9, "task-a"),
      createSynthetic("b-1", 0.8, "task-b"),
      createSynthetic("a-2", 0.7, "task-a"),
    ];
    const result = selectWithTaskBalance(candidates, { ...DEFAULT_SELECTION_CONFIG, topK: 3 });

    // Ranks should be reassigned after combining
    expect(result.topExamples[0]?.rank).toBe(1);
    if (result.topExamples.length > 1) {
      expect(result.topExamples[1]?.rank).toBe(2);
    }
  });
});

describe("SelectionService", () => {
  test("selectGreedyDiverse returns result", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* SelectionService;
        const candidates = Array.from({ length: 30 }, (_, i) =>
          createSynthetic(`${i}`, 0.1 + i * 0.02),
        );
        return yield* service.selectGreedyDiverse(candidates);
      }).pipe(Effect.provide(SelectionServiceLive)),
    );

    expect(result.totalCandidates).toBe(30);
  });

  test("selectWithTaskBalance returns result", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* SelectionService;
        const candidates = [
          createSynthetic("a", 0.5, "task-1"),
          createSynthetic("b", 0.6, "task-2"),
        ];
        return yield* service.selectWithTaskBalance(candidates);
      }).pipe(Effect.provide(SelectionServiceLive)),
    );

    expect(result.totalCandidates).toBe(2);
  });

  test("selectTop with custom K", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* SelectionService;
        const candidates = Array.from({ length: 20 }, (_, i) =>
          createSynthetic(`${i}`, 0.3 + i * 0.03),
        );
        return yield* service.selectTop(candidates, 5);
      }).pipe(Effect.provide(SelectionServiceLive)),
    );

    expect(result).toHaveLength(5);
  });

  test("selectBottom with custom K", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* SelectionService;
        const candidates = Array.from({ length: 20 }, (_, i) =>
          createSynthetic(`${i}`, 0.1 + i * 0.02),
        );
        return yield* service.selectBottom(candidates, 3);
      }).pipe(Effect.provide(SelectionServiceLive)),
    );

    expect(result.length).toBeLessThanOrEqual(3);
  });

  test("calculateDiversity returns score", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* SelectionService;
        const candidate = createSynthetic("new", 0.5, "task", "function unique() { return 42; }");
        const existing = [
          createSynthetic("old", 0.5, "task", "for (let i = 0; i < n; i++) { sum += i; }"),
        ];
        return yield* service.calculateDiversity(candidate, existing);
      }).pipe(Effect.provide(SelectionServiceLive)),
    );

    expect(result).toBeGreaterThanOrEqual(0);
    expect(result).toBeLessThanOrEqual(1);
  });

  test("getStats tracks selections", () => {
    // Use fresh layer to avoid stats accumulation from other tests
    const freshLayer = makeSelectionServiceLayer();
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* SelectionService;
        const candidates = Array.from({ length: 50 }, (_, i) =>
          createSynthetic(`${i}`, 0.1 + i * 0.015),
        );
        yield* service.selectGreedyDiverse(candidates);
        return yield* service.getStats();
      }).pipe(Effect.provide(freshLayer)),
    );

    expect(result.totalSelections).toBe(1);
    expect(result.totalCandidatesProcessed).toBe(50);
  });

  test("updateConfig modifies config", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* SelectionService;
        return yield* service.updateConfig({ topK: 10 });
      }).pipe(Effect.provide(SelectionServiceLive)),
    );

    expect(result.topK).toBe(10);
  });

  test("custom config layer", () => {
    const customLayer = makeSelectionServiceLayer({ diversityWeight: 0.5 });
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* SelectionService;
        return yield* service.getConfig();
      }).pipe(Effect.provide(customLayer)),
    );

    expect(result.diversityWeight).toBe(0.5);
  });
});
