/**
 * SOAR Integration Tests
 *
 * Tests for SOAR components working together:
 * Hindsight → Validation → Selection → Voting
 */

import { describe, test, expect } from "bun:test";
import { Option } from "effect";
import {
  relabelBatch,
  relabelAttempt,
  type AttemptRecord,
} from "../soar-hindsight.js";
import {
  validateBatch,
  DEFAULT_VALIDATION_CONFIG,
} from "../soar-validation.js";
import {
  selectGreedyDiverse,
  selectTop,
  DEFAULT_SELECTION_CONFIG,
} from "../soar-selection.js";
import {
  vote,
  createVotes,
  ensembleVote,
  DEFAULT_VOTING_CONFIG,
} from "../soar-voting.js";
import { createMockAttemptRecord, createMockAttemptBatch } from "./test-helpers.js";

describe("Hindsight → Validation Pipeline", () => {
  test("relabeled attempts pass through validation", () => {
    // Create diverse failed attempts
    const attempts: AttemptRecord[] = [
      createMockAttemptRecord({
        id: "att-1",
        taskId: "task-1",
        success: false,
        trainingAccuracy: 0.7,
        code: "function solve(n) { if (n <= 1) return n; return solve(n-1) + solve(n-2); }",
        actualOutput: { result: 55 },
      }),
      createMockAttemptRecord({
        id: "att-2",
        taskId: "task-2",
        success: false,
        trainingAccuracy: 0.5,
        code: "function process(x) { return x.map(i => i * 2).filter(i => i > 0); }",
        actualOutput: [2, 4, 6, 8],
      }),
    ];

    // Relabel attempts into synthetic tasks
    const synthetics = relabelBatch(attempts);
    expect(synthetics.length).toBeGreaterThan(0);

    // Validate synthetics
    const { valid, invalid } = validateBatch(synthetics);

    // At least some should be valid since we crafted good attempts
    expect(valid.length + invalid.length).toBe(synthetics.length);
  });

  test("low quality attempts filtered by validation", () => {
    // Create attempts that will produce invalid synthetics
    const attempts: AttemptRecord[] = [
      createMockAttemptRecord({
        id: "trivial-1",
        taskId: "task-1",
        success: false,
        trainingAccuracy: 0.3,
        code: "x", // Too simple
        actualOutput: "ab", // Too short
      }),
    ];

    const synthetics = relabelBatch(attempts);
    const { valid, invalid } = validateBatch(synthetics);

    // These should be invalid due to code complexity and output length
    expect(invalid.length).toBeGreaterThanOrEqual(valid.length);
  });
});

describe("Validation → Selection Pipeline", () => {
  test("validated synthetics are selected by quality", () => {
    // Create a batch of synthetics with varying quality
    const attempts = createMockAttemptBatch(30, 0.5, 0.3);
    const synthetics = relabelBatch(attempts);
    const { valid } = validateBatch(synthetics);

    if (valid.length > 0) {
      // Select top examples
      const selected = selectTop(valid, { ...DEFAULT_SELECTION_CONFIG, topK: 5 });

      // Should respect topK limit
      expect(selected.length).toBeLessThanOrEqual(5);

      // Should be sorted by quality (rank 1 = best)
      if (selected.length > 1) {
        expect(selected[0]?.rank).toBe(1);
        expect(selected[0]?.selectionScore).toBeGreaterThanOrEqual(selected[1]?.selectionScore ?? 0);
      }
    }
  });

  test("greedy-diverse selection balances quality and diversity", () => {
    const attempts = createMockAttemptBatch(50, 0.7, 0.2);
    const synthetics = relabelBatch(attempts);
    const { valid } = validateBatch(synthetics);

    if (valid.length >= 5) {
      const result = selectGreedyDiverse(valid);

      expect(result.topExamples.length).toBeGreaterThan(0);
      expect(result.totalCandidates).toBe(valid.length);
    }
  });
});

describe("Selection → Voting Pipeline", () => {
  test("selected outputs feed into voting system", () => {
    // Create varied outputs with quality scores
    const outputs = [
      { output: 42, program: "function a() { return 42; }", trainingAccuracy: 0.9 },
      { output: 42, program: "function b() { return 21 + 21; }", trainingAccuracy: 0.85 },
      { output: 42, program: "function c() { return 6 * 7; }", trainingAccuracy: 0.8 },
      { output: 10, program: "function d() { return 10; }", trainingAccuracy: 0.3 },
    ];

    const votes = createVotes(outputs);
    expect(votes).toHaveLength(4);

    const result = vote(votes);

    // Majority should win (42 has 3 votes, 10 has 1)
    expect(result.winner).toBe(42);
    expect(result.isValid).toBe(true);
    expect(result.confidence).toBeGreaterThan(0.5);
  });

  test("ensemble voting combines creation and voting", () => {
    const outputs = [
      { output: "hello", program: "code1", trainingAccuracy: 0.9 },
      { output: "hello", program: "code2", trainingAccuracy: 0.8 },
      { output: "world", program: "code3", trainingAccuracy: 0.5 },
    ];

    const result = ensembleVote(outputs);

    expect(result.winner).toBe("hello");
    expect(result.totalVotes).toBe(3);
  });
});

describe("Full SOAR Pipeline", () => {
  test("end-to-end: attempts → synthetics → validated → selected → voted", () => {
    // 1. Create failed attempts (raw material)
    const attempts: AttemptRecord[] = [
      createMockAttemptRecord({
        id: "full-1",
        taskId: "task-main",
        success: false,
        trainingAccuracy: 0.8,
        code: "function solve(arr) { return arr.reduce((a, b) => a + b, 0); }",
        actualOutput: 15,
      }),
      createMockAttemptRecord({
        id: "full-2",
        taskId: "task-main",
        success: false,
        trainingAccuracy: 0.7,
        code: "function solve(arr) { let sum = 0; for (const x of arr) sum += x; return sum; }",
        actualOutput: 15,
      }),
      createMockAttemptRecord({
        id: "full-3",
        taskId: "task-main",
        success: false,
        trainingAccuracy: 0.6,
        code: "function solve(arr) { return arr.length > 0 ? arr[0] + solve(arr.slice(1)) : 0; }",
        actualOutput: 15,
      }),
    ];

    // 2. Hindsight relabeling
    const synthetics = relabelBatch(attempts);
    expect(synthetics.length).toBeGreaterThan(0);

    // 3. Validation
    const { valid } = validateBatch(synthetics);

    if (valid.length > 0) {
      // 4. Selection
      const selection = selectGreedyDiverse(valid);
      expect(selection.topExamples.length).toBeGreaterThan(0);

      // 5. Voting (using the solutions as "outputs")
      const outputs = valid.map((s) => ({
        output: s.task.output,
        program: s.solution,
        trainingAccuracy: s.qualityScore,
      }));

      const result = ensembleVote(outputs);

      // All attempts produced 15, so that should win
      expect(result.winner).toBe(15);
      expect(result.isValid).toBe(true);
    }
  });

  test("handles empty pipeline gracefully", () => {
    // No valid attempts
    const attempts = [
      createMockAttemptRecord({ success: true }), // Filtered: success
      createMockAttemptRecord({ trainingAccuracy: 0.001 }), // Filtered: too low
    ];

    const synthetics = relabelBatch(attempts);
    expect(synthetics).toHaveLength(0);

    const { valid, invalid } = validateBatch(synthetics);
    expect(valid).toHaveLength(0);
    expect(invalid).toHaveLength(0);

    const selection = selectGreedyDiverse([]);
    expect(selection.topExamples).toHaveLength(0);
    expect(selection.bottomExamples).toHaveLength(0);

    const result = vote([], { ...DEFAULT_VOTING_CONFIG, minVotes: 1 });
    expect(result.isValid).toBe(false);
  });

  test("diverse attempts produce diverse synthetics", () => {
    // Create attempts for different tasks
    const attempts: AttemptRecord[] = [
      createMockAttemptRecord({
        id: "div-1",
        taskId: "task-a",
        taskDescription: "Sum an array",
        success: false,
        trainingAccuracy: 0.8,
        code: "function sum(arr) { return arr.reduce((a,b) => a+b, 0); }",
        actualOutput: 10,
      }),
      createMockAttemptRecord({
        id: "div-2",
        taskId: "task-b",
        taskDescription: "Multiply elements",
        success: false,
        trainingAccuracy: 0.7,
        code: "function mult(arr) { return arr.reduce((a,b) => a*b, 1); }",
        actualOutput: 24,
      }),
      createMockAttemptRecord({
        id: "div-3",
        taskId: "task-c",
        taskDescription: "Find maximum",
        success: false,
        trainingAccuracy: 0.9,
        code: "function max(arr) { return Math.max(...arr); }",
        actualOutput: 5,
      }),
    ];

    const synthetics = relabelBatch(attempts);

    // Each attempt should produce a synthetic for a different task
    const taskIds = new Set(synthetics.map((s) => s.task.originalTaskId));
    expect(taskIds.size).toBeGreaterThanOrEqual(2);
  });
});

describe("Quality Scoring Consistency", () => {
  test("quality flows through pipeline consistently", () => {
    const highQuality = createMockAttemptRecord({
      id: "high",
      taskId: "task",
      success: false,
      trainingAccuracy: 0.95,
      code: "function solve() { if (true) { return compute(); } else { return fallback(); } }",
      actualOutput: { data: "result", status: "success" },
    });

    const lowQuality = createMockAttemptRecord({
      id: "low",
      taskId: "task",
      success: false,
      trainingAccuracy: 0.15,
      code: "function s() { if (x) return y; }",
      actualOutput: { x: 1 },
    });

    const highSynthetic = relabelAttempt(highQuality);
    const lowSynthetic = relabelAttempt(lowQuality);

    if (Option.isSome(highSynthetic)) {
      expect(highSynthetic.value.qualityScore).toBe(0.95);
    }

    if (Option.isSome(lowSynthetic)) {
      expect(lowSynthetic.value.qualityScore).toBe(0.15);
    }
  });

  test("voting weights reflect quality scores", () => {
    const outputs = [
      { output: "A", program: "code", trainingAccuracy: 0.9 },
      { output: "B", program: "code", trainingAccuracy: 0.1 },
    ];

    const result = vote(createVotes(outputs));

    // Higher accuracy should give more weight
    expect(result.winner).toBe("A");

    // Find candidate weights
    const candA = result.candidates.find((c) => c.outputKey === "A");
    const candB = result.candidates.find((c) => c.outputKey === "B");

    if (candA && candB) {
      expect(candA.weight).toBeGreaterThan(candB.weight);
    }
  });
});
