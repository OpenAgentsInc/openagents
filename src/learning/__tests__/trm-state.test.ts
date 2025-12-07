/**
 * TRM State Tests
 *
 * Tests for TRM state schema, creation, and update functions.
 */

import { describe, test, expect } from "bun:test";
import * as S from "effect/Schema";
import { Effect } from "effect";
import {
  TaskContext,
  CandidateSolution,
  ReasoningTrace,
  TRMState,
  createTaskContext,
  createInitialSolution,
  createInitialReasoning,
  createTRMState,
  updateSolution,
  updateReasoning,
  addReasoningStep,
  markStuck,
  addHypothesis,
  ruleOutApproach,
  completeCycle,
  detachState,
  TRMStateService,
  TRMStateServiceLive,
} from "../trm-state.js";
import { createMockTaskContext, createMockTRMState, runEffect } from "./test-helpers.js";

describe("TRM State Schema", () => {
  describe("TaskContext", () => {
    test("decodes valid task context", () => {
      const input = {
        taskId: "task-001",
        description: "Test task",
        inputs: [1, 2, 3],
      };
      const decoded = S.decodeUnknownSync(TaskContext)(input);
      expect(decoded.taskId).toBe("task-001");
      expect(decoded.description).toBe("Test task");
      expect(decoded.inputs).toEqual([1, 2, 3]);
    });

    test("accepts optional fields", () => {
      const input = {
        taskId: "task-002",
        description: "Test with optional fields",
        inputs: [],
        expectedOutputs: [42],
        constraints: ["no loops"],
        category: "arc-agi",
        difficulty: 3,
      };
      const decoded = S.decodeUnknownSync(TaskContext)(input);
      expect(decoded.expectedOutputs).toEqual([42]);
      expect(decoded.constraints).toEqual(["no loops"]);
      expect(decoded.category).toBe("arc-agi");
      expect(decoded.difficulty).toBe(3);
    });

    test("rejects missing required fields", () => {
      expect(() =>
        S.decodeUnknownSync(TaskContext)({ description: "missing taskId" }),
      ).toThrow();
    });
  });

  describe("CandidateSolution", () => {
    test("decodes valid candidate solution", () => {
      const input = {
        code: "function solve() {}",
        attemptNumber: 1,
        validated: false,
        trainingAccuracy: 0.5,
        confidence: 0.6,
        updatedAt: new Date().toISOString(),
      };
      const decoded = S.decodeUnknownSync(CandidateSolution)(input);
      expect(decoded.code).toBe("function solve() {}");
      expect(decoded.attemptNumber).toBe(1);
      expect(decoded.validated).toBe(false);
    });

    test("accepts validation result when validated", () => {
      const input = {
        code: "function solve() { return 42; }",
        attemptNumber: 3,
        validated: true,
        validationResult: {
          passed: true,
          testsRun: 10,
          testsPassed: 10,
        },
        trainingAccuracy: 1.0,
        confidence: 0.99,
        updatedAt: new Date().toISOString(),
      };
      const decoded = S.decodeUnknownSync(CandidateSolution)(input);
      expect(decoded.validationResult?.passed).toBe(true);
      expect(decoded.validationResult?.testsPassed).toBe(10);
    });
  });

  describe("ReasoningTrace", () => {
    test("decodes valid reasoning trace", () => {
      const input = {
        hypotheses: ["Try recursion"],
        errorPatterns: [],
        ruledOut: [],
        progress: {
          stepsCompleted: 5,
          totalSteps: 42,
          isStuck: false,
          stuckCount: 0,
        },
        history: [],
        depth: 5,
        maxDepth: 42,
      };
      const decoded = S.decodeUnknownSync(ReasoningTrace)(input);
      expect(decoded.hypotheses).toEqual(["Try recursion"]);
      expect(decoded.depth).toBe(5);
      expect(decoded.maxDepth).toBe(42);
    });

    test("handles history entries", () => {
      const input = {
        hypotheses: [],
        errorPatterns: [],
        ruledOut: [],
        progress: {
          stepsCompleted: 2,
          totalSteps: 10,
          isStuck: false,
          stuckCount: 0,
        },
        history: [
          { step: 1, thought: "First thought" },
          { step: 2, thought: "Second thought", action: "try loop", result: "failed" },
        ],
        depth: 2,
        maxDepth: 10,
      };
      const decoded = S.decodeUnknownSync(ReasoningTrace)(input);
      expect(decoded.history).toHaveLength(2);
      expect(decoded.history[1]?.action).toBe("try loop");
    });
  });

  describe("TRMState", () => {
    test("decodes complete TRM state", () => {
      const now = new Date().toISOString();
      const input = {
        x: createMockTaskContext(),
        y: {
          code: "",
          attemptNumber: 0,
          validated: false,
          trainingAccuracy: 0,
          confidence: 0,
          updatedAt: now,
        },
        z: {
          hypotheses: [],
          errorPatterns: [],
          ruledOut: [],
          progress: { stepsCompleted: 0, totalSteps: 42, isStuck: false, stuckCount: 0 },
          history: [],
          depth: 0,
          maxDepth: 42,
        },
        meta: {
          createdAt: now,
          updatedAt: now,
          cycles: 0,
          tokensUsed: 0,
        },
      };
      const decoded = S.decodeUnknownSync(TRMState)(input);
      expect(decoded.x.taskId).toBe("test-task-001");
      expect(decoded.meta.cycles).toBe(0);
    });
  });
});

describe("State Creation Helpers", () => {
  describe("createTaskContext", () => {
    test("creates basic task context", () => {
      const ctx = createTaskContext("task-001", "Test description");
      expect(ctx.taskId).toBe("task-001");
      expect(ctx.description).toBe("Test description");
      expect(ctx.inputs).toEqual([]);
    });

    test("includes inputs when provided", () => {
      const ctx = createTaskContext("task-002", "With inputs", [1, 2, 3]);
      expect(ctx.inputs).toEqual([1, 2, 3]);
    });

    test("includes optional fields", () => {
      const ctx = createTaskContext("task-003", "With options", [], {
        difficulty: 5,
        category: "hard",
      });
      expect(ctx.difficulty).toBe(5);
      expect(ctx.category).toBe("hard");
    });
  });

  describe("createInitialSolution", () => {
    test("creates empty solution", () => {
      const solution = createInitialSolution();
      expect(solution.code).toBe("");
      expect(solution.attemptNumber).toBe(0);
      expect(solution.validated).toBe(false);
      expect(solution.trainingAccuracy).toBe(0);
      expect(solution.confidence).toBe(0);
    });

    test("has valid timestamp", () => {
      const solution = createInitialSolution();
      expect(() => new Date(solution.updatedAt)).not.toThrow();
    });
  });

  describe("createInitialReasoning", () => {
    test("creates with default maxDepth", () => {
      const reasoning = createInitialReasoning();
      expect(reasoning.maxDepth).toBe(42);
      expect(reasoning.depth).toBe(0);
    });

    test("respects custom maxDepth", () => {
      const reasoning = createInitialReasoning(100);
      expect(reasoning.maxDepth).toBe(100);
      expect(reasoning.progress.totalSteps).toBe(100);
    });

    test("initializes empty arrays", () => {
      const reasoning = createInitialReasoning();
      expect(reasoning.hypotheses).toEqual([]);
      expect(reasoning.errorPatterns).toEqual([]);
      expect(reasoning.ruledOut).toEqual([]);
      expect(reasoning.history).toEqual([]);
    });

    test("initializes progress correctly", () => {
      const reasoning = createInitialReasoning(10);
      expect(reasoning.progress.stepsCompleted).toBe(0);
      expect(reasoning.progress.totalSteps).toBe(10);
      expect(reasoning.progress.isStuck).toBe(false);
      expect(reasoning.progress.stuckCount).toBe(0);
    });
  });

  describe("createTRMState", () => {
    test("creates complete state from task context", () => {
      const taskCtx = createTaskContext("task-001", "Test");
      const state = createTRMState(taskCtx);

      expect(state.x).toBe(taskCtx);
      expect(state.y.attemptNumber).toBe(0);
      expect(state.z.depth).toBe(0);
      expect(state.meta.cycles).toBe(0);
    });

    test("uses custom maxDepth", () => {
      const taskCtx = createTaskContext("task-002", "Test");
      const state = createTRMState(taskCtx, 100);

      expect(state.z.maxDepth).toBe(100);
    });
  });
});

describe("State Update Functions", () => {
  describe("updateSolution", () => {
    test("updates solution with new code", () => {
      const state = createMockTRMState();
      const updated = updateSolution(state, { code: "new code" });

      expect(updated.y.code).toBe("new code");
      expect(updated.y.attemptNumber).toBe(state.y.attemptNumber + 1);
    });

    test("preserves existing fields when not updated", () => {
      const state = createMockTRMState({ y: { code: "old", trainingAccuracy: 0.5 } });
      const updated = updateSolution(state, { code: "new" });

      expect(updated.y.trainingAccuracy).toBe(0.5);
    });

    test("updates meta timestamp", () => {
      const state = createMockTRMState();
      const before = new Date(state.meta.updatedAt);
      const updated = updateSolution(state, { code: "new" });
      const after = new Date(updated.meta.updatedAt);

      expect(after.getTime()).toBeGreaterThanOrEqual(before.getTime());
    });

    test("is immutable - original unchanged", () => {
      const state = createMockTRMState();
      const originalCode = state.y.code;
      updateSolution(state, { code: "different" });

      expect(state.y.code).toBe(originalCode);
    });
  });

  describe("updateReasoning", () => {
    test("updates reasoning fields", () => {
      const state = createMockTRMState();
      const updated = updateReasoning(state, {
        hypotheses: ["new hypothesis"],
      });

      expect(updated.z.hypotheses).toEqual(["new hypothesis"]);
    });

    test("increments depth", () => {
      const state = createMockTRMState({ z: { depth: 5 } });
      const updated = updateReasoning(state, {});

      expect(updated.z.depth).toBe(6);
    });
  });

  describe("addReasoningStep", () => {
    test("adds step to history", () => {
      const state = createMockTRMState({ z: { history: [], depth: 0 } });
      const updated = addReasoningStep(state, "thinking...", "action", "result");

      expect(updated.z.history).toHaveLength(1);
      expect(updated.z.history[0]?.thought).toBe("thinking...");
      expect(updated.z.history[0]?.action).toBe("action");
      expect(updated.z.history[0]?.result).toBe("result");
    });

    test("keeps only last 10 steps", () => {
      let state = createMockTRMState({ z: { history: [], depth: 0, progress: { stepsCompleted: 0, totalSteps: 42, isStuck: false, stuckCount: 0 } } });

      // Add 15 steps
      for (let i = 0; i < 15; i++) {
        state = addReasoningStep(state, `thought-${i}`);
      }

      expect(state.z.history).toHaveLength(10);
      expect(state.z.history[0]?.thought).toBe("thought-5");
      expect(state.z.history[9]?.thought).toBe("thought-14");
    });

    test("increments stepsCompleted", () => {
      const state = createMockTRMState({ z: { progress: { stepsCompleted: 0, totalSteps: 42, isStuck: false, stuckCount: 0 } } });
      const updated = addReasoningStep(state, "test");

      expect(updated.z.progress.stepsCompleted).toBe(1);
    });
  });

  describe("markStuck", () => {
    test("sets isStuck to true", () => {
      const state = createMockTRMState({ z: { progress: { stepsCompleted: 0, totalSteps: 42, isStuck: false, stuckCount: 0 } } });
      const updated = markStuck(state, "error pattern");

      expect(updated.z.progress.isStuck).toBe(true);
    });

    test("increments stuckCount", () => {
      const state = createMockTRMState({ z: { progress: { stepsCompleted: 0, totalSteps: 42, isStuck: false, stuckCount: 2 } } });
      const updated = markStuck(state, "error pattern");

      expect(updated.z.progress.stuckCount).toBe(3);
    });

    test("adds new error pattern", () => {
      const state = createMockTRMState({ z: { errorPatterns: [] } });
      const updated = markStuck(state, "new error");

      expect(updated.z.errorPatterns).toContain("new error");
    });

    test("does not duplicate existing error pattern", () => {
      const state = createMockTRMState({ z: { errorPatterns: ["existing error"] } });
      const updated = markStuck(state, "existing error");

      expect(updated.z.errorPatterns).toEqual(["existing error"]);
    });
  });

  describe("addHypothesis", () => {
    test("adds hypothesis", () => {
      const state = createMockTRMState({ z: { hypotheses: [] } });
      const updated = addHypothesis(state, "try recursion");

      expect(updated.z.hypotheses).toContain("try recursion");
    });

    test("keeps only last 5 hypotheses", () => {
      let state = createMockTRMState({ z: { hypotheses: [], depth: 0 } });

      for (let i = 0; i < 8; i++) {
        state = addHypothesis(state, `hypothesis-${i}`);
      }

      expect(state.z.hypotheses).toHaveLength(5);
      expect(state.z.hypotheses[0]).toBe("hypothesis-3");
    });
  });

  describe("ruleOutApproach", () => {
    test("adds approach to ruledOut", () => {
      const state = createMockTRMState({ z: { ruledOut: [] } });
      const updated = ruleOutApproach(state, "brute force");

      expect(updated.z.ruledOut).toContain("brute force");
    });
  });

  describe("completeCycle", () => {
    test("increments cycle count", () => {
      const state = createMockTRMState({ meta: { cycles: 5 } });
      const updated = completeCycle(state);

      expect(updated.meta.cycles).toBe(6);
    });

    test("adds tokens used", () => {
      const state = createMockTRMState({ meta: { tokensUsed: 1000 } });
      const updated = completeCycle(state, 500);

      expect(updated.meta.tokensUsed).toBe(1500);
    });

    test("defaults tokens to 0", () => {
      const state = createMockTRMState({ meta: { tokensUsed: 1000 } });
      const updated = completeCycle(state);

      expect(updated.meta.tokensUsed).toBe(1000);
    });
  });

  describe("detachState", () => {
    test("clears isStuck", () => {
      const state = createMockTRMState({ z: { progress: { stepsCompleted: 0, totalSteps: 42, isStuck: true, stuckCount: 0 } } });
      const detached = detachState(state);

      expect(detached.z.progress.isStuck).toBe(false);
    });

    test("keeps last 3 history entries", () => {
      const history = Array.from({ length: 10 }, (_, i) => ({ step: i, thought: `thought-${i}` }));
      const state = createMockTRMState({ z: { history } });
      const detached = detachState(state);

      expect(detached.z.history).toHaveLength(3);
      expect(detached.z.history[0]?.thought).toBe("thought-7");
    });

    test("preserves learned knowledge", () => {
      const state = createMockTRMState({
        z: {
          hypotheses: ["h1", "h2"],
          errorPatterns: ["e1"],
          ruledOut: ["r1"],
        },
      });
      const detached = detachState(state);

      expect(detached.z.hypotheses).toEqual(["h1", "h2"]);
      expect(detached.z.errorPatterns).toEqual(["e1"]);
      expect(detached.z.ruledOut).toEqual(["r1"]);
    });
  });
});

describe("TRMStateService", () => {
  test("create returns initial state", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMStateService;
        return yield* service.create("task-001", "Test task", [1, 2, 3]);
      }).pipe(Effect.provide(TRMStateServiceLive)),
    );

    expect(result.x.taskId).toBe("task-001");
    expect(result.x.description).toBe("Test task");
    expect(result.x.inputs).toEqual([1, 2, 3]);
  });

  test("updateSolution updates state", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMStateService;
        const state = yield* service.create("task-001", "Test");
        return yield* service.updateSolution(state, "new code", 0.8, 0.9);
      }).pipe(Effect.provide(TRMStateServiceLive)),
    );

    expect(result.y.code).toBe("new code");
    expect(result.y.trainingAccuracy).toBe(0.8);
    expect(result.y.confidence).toBe(0.9);
  });

  test("addReasoning adds step", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMStateService;
        const state = yield* service.create("task-001", "Test");
        return yield* service.addReasoning(state, "thinking", "action", "result");
      }).pipe(Effect.provide(TRMStateServiceLive)),
    );

    expect(result.z.history).toHaveLength(1);
    expect(result.z.history[0]?.thought).toBe("thinking");
  });

  test("isMaxDepth returns true at max", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMStateService;
        const state = yield* service.create("task-001", "Test", [], { maxDepth: 5 });
        // Simulate reaching max depth
        let current = state;
        for (let i = 0; i < 5; i++) {
          current = yield* service.addReasoning(current, `step-${i}`);
        }
        return yield* service.isMaxDepth(current);
      }).pipe(Effect.provide(TRMStateServiceLive)),
    );

    expect(result).toBe(true);
  });

  test("getAccuracy returns training accuracy", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* TRMStateService;
        const state = yield* service.create("task-001", "Test");
        const updated = yield* service.updateSolution(state, "code", 0.75, 0.8);
        return yield* service.getAccuracy(updated);
      }).pipe(Effect.provide(TRMStateServiceLive)),
    );

    expect(result).toBe(0.75);
  });
});
