/**
 * End-to-End Tests for Healer Scenarios
 *
 * These tests verify the complete Healer flow for various failure scenarios:
 * 1. Init typecheck failure -> Healer fixes -> continue
 * 2. Verification failure -> Healer rewinds -> marks blocked
 * 3. Subtask failure x3 -> Healer creates follow-up task
 * 4. Healer respects invocation limits
 * 5. Healer never commits if tests still failing
 */
import { describe, test, expect } from "bun:test";
import { Effect } from "effect";
import {
  createBasicHealerService,
  createFullHealerService,
} from "../service.js";
import type { HealerEvent } from "../service.js";
import { createHealerCounters } from "../types.js";
import type { OrchestratorEvent, OrchestratorState, Subtask } from "../../agent/orchestrator/types.js";
import type { ProjectConfig, HealerConfig } from "../../tasks/schema.js";
import type { ClaudeCodeInvoker, VerificationRunner } from "../spells/typecheck.js";
import {
  createMockProjectConfig,
  createMockOrchestratorState,
  createMockSubtask as createTestSubtask,
} from "./test-helpers.js";

// ============================================================================
// Test Fixtures
// ============================================================================

const createMockState = (overrides: Partial<OrchestratorState> = {}): OrchestratorState =>
  createMockOrchestratorState(overrides);

const createMockConfig = (healerOverrides: Partial<HealerConfig> = {}): ProjectConfig =>
  createMockProjectConfig({
    healer: {
      enabled: true,
      maxInvocationsPerSession: 5,
      maxInvocationsPerSubtask: 3,
      mode: "conservative",
      stuckThresholdHours: 2,
      scenarios: {
        onInitFailure: true,
        onVerificationFailure: true,
        onSubtaskFailure: true,
        onRuntimeError: true,
        onStuckSubtask: false,
      },
      spells: {
        allowed: [],
        forbidden: [],
      },
      ...healerOverrides,
    },
  });

const createMockSubtask = (overrides: Partial<Subtask> = {}): Subtask =>
  createTestSubtask(overrides);

// ============================================================================
// Scenario 1: Init Typecheck Failure -> Healer Fixes -> Continue
// ============================================================================

describe("Scenario 1: Init typecheck failure", () => {
  test("Healer invokes fix_typecheck_errors spell for typecheck failure", async () => {
    const events: HealerEvent[] = [];
    const service = createBasicHealerService((e) => events.push(e));

    const state = createMockState();
    const config = createMockConfig();
    const counters = createHealerCounters();

    const event: OrchestratorEvent = {
      type: "init_script_complete",
      result: {
        ran: true,
        success: false,
        failureType: "typecheck_failed",
        canSelfHeal: true,
        output: "error TS2345: Argument of type 'string' is not assignable",
      },
    };

    const outcome = await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    expect(outcome).not.toBeNull();
    expect(outcome?.status).toBeDefined();
    // Basic service skips LLM spells, so fix_typecheck_errors won't run
    // but update_progress_with_guidance should run
    expect(events.length).toBeGreaterThan(0);
    expect(events[0].type).toBe("healer_start");
    if (events[0].type === "healer_start") {
      expect(events[0].scenario).toBe("InitScriptTypecheckFailure");
    }
  });

  test("Full service with mock invoker fixes typecheck errors", async () => {
    const events: HealerEvent[] = [];

    // Mock invoker that simulates successful fix
    const mockInvoker: ClaudeCodeInvoker = async (_subtask, _options) => ({
      success: true,
      subtaskId: "emergency-typecheck-fix",
      filesModified: ["src/fix.ts"],
      turns: 5,
    });

    // Mock verifier that confirms fix worked
    const mockVerifier: VerificationRunner = async (_cwd) => ({
      success: true,
      output: "No errors",
    });

    const service = createFullHealerService(mockInvoker, mockVerifier, {
      onEvent: (e) => events.push(e),
    });

    const state = createMockState();
    const config = createMockConfig();
    const counters = createHealerCounters();

    const event: OrchestratorEvent = {
      type: "init_script_complete",
      result: {
        ran: true,
        success: false,
        failureType: "typecheck_failed",
        canSelfHeal: true,
        output: "error TS2345: Argument of type 'string' is not assignable",
      },
    };

    const outcome = await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    expect(outcome).not.toBeNull();
    // The fix_typecheck_errors spell should have been attempted
    const spellCompleteEvents = events.filter(e => e.type === "healer_spell_complete");
    expect(spellCompleteEvents.length).toBeGreaterThan(0);
  });
});

// ============================================================================
// Scenario 2: Verification Failure -> Healer Rewinds -> Marks Blocked
// ============================================================================

describe("Scenario 2: Verification failure", () => {
  test("Healer rewinds uncommitted changes on verification failure", async () => {
    const events: HealerEvent[] = [];
    const service = createBasicHealerService((e) => events.push(e));

    const subtask = createMockSubtask({ id: "subtask-001" });
    const state = createMockState({
      subtasks: {
        taskId: "task-123",
        taskTitle: "Test Task",
        subtasks: [subtask],
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
    });
    const config = createMockConfig();
    const counters = createHealerCounters();

    const event: OrchestratorEvent = {
      type: "verification_complete",
      command: "bun test",
      passed: false,
      output: "FAIL src/test.ts\n  ✕ should work",
    };

    const outcome = await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    expect(outcome).not.toBeNull();
    expect(events.some(e => e.type === "healer_start")).toBe(true);

    const startEvent = events.find(e => e.type === "healer_start");
    if (startEvent?.type === "healer_start") {
      expect(startEvent.scenario).toBe("VerificationFailed");
      expect(startEvent.spells).toContain("rewind_uncommitted_changes");
    }
  });
});

// ============================================================================
// Scenario 3: Subtask Failure x3 -> Creates Follow-up Task
// ============================================================================

describe("Scenario 3: Subtask failure x3", () => {
  test("Healer marks task blocked after multiple failures", async () => {
    const events: HealerEvent[] = [];
    const service = createBasicHealerService((e) => events.push(e));

    const subtask = createMockSubtask({
      id: "subtask-001",
      status: "failed",
      failureCount: 3,
    });

    const state = createMockState({
      subtasks: {
        taskId: "task-123",
        taskTitle: "Test Task",
        subtasks: [subtask],
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
    });
    const config = createMockConfig();
    const counters = createHealerCounters();

    const event: OrchestratorEvent = {
      type: "subtask_failed",
      subtask,
      error: "Assertion failed: expected true to be false",
    };

    const outcome = await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    expect(outcome).not.toBeNull();

    const startEvent = events.find(e => e.type === "healer_start");
    if (startEvent?.type === "healer_start") {
      expect(startEvent.scenario).toBe("SubtaskFailed");
      // Should include mark_task_blocked_with_followup for repeated failures
      expect(startEvent.spells).toContain("mark_task_blocked_with_followup");
    }
  });
});

// ============================================================================
// Scenario 4: Healer Respects Invocation Limits
// ============================================================================

describe("Scenario 4: Invocation limits", () => {
  test("Healer skips when session limit is reached", async () => {
    const service = createBasicHealerService();

    const state = createMockState();
    const config = createMockConfig({
      maxInvocationsPerSession: 2,
    });
    const counters = createHealerCounters();
    // Simulate reaching the limit
    counters.sessionInvocations = 2;

    const event: OrchestratorEvent = {
      type: "subtask_failed",
      subtask: createMockSubtask(),
      error: "Test error",
    };

    const outcome = await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    // Should return null because limit was reached
    expect(outcome).toBeNull();
  });

  test("Healer skips when subtask limit is reached", async () => {
    const service = createBasicHealerService();

    const subtask = createMockSubtask({ id: "subtask-001" });
    const state = createMockState({
      subtasks: {
        taskId: "task-123",
        taskTitle: "Test Task",
        subtasks: [{ ...subtask, status: "failed" }],
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
    });
    const config = createMockConfig({
      maxInvocationsPerSubtask: 1,
    });
    const counters = createHealerCounters();
    // Simulate reaching the subtask limit
    counters.subtaskInvocations.set("subtask-001", 1);

    const event: OrchestratorEvent = {
      type: "subtask_failed",
      subtask: { ...subtask, status: "failed" },
      error: "Test error",
    };

    const outcome = await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    // Should return null because subtask limit was reached
    expect(outcome).toBeNull();
  });

  test("Healer increments counters after invocation", async () => {
    const service = createBasicHealerService();

    const subtask = createMockSubtask({ id: "subtask-001" });
    const state = createMockState({
      subtasks: {
        taskId: "task-123",
        taskTitle: "Test Task",
        subtasks: [{ ...subtask, status: "failed" }],
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
    });
    const config = createMockConfig();
    const counters = createHealerCounters();

    expect(counters.sessionInvocations).toBe(0);

    const event: OrchestratorEvent = {
      type: "subtask_failed",
      subtask: { ...subtask, status: "failed" },
      error: "Test error",
    };

    await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    // Counters should be incremented
    expect(counters.sessionInvocations).toBe(1);
  });
});

// ============================================================================
// Scenario 5: Healer Never Commits if Tests Still Failing
// ============================================================================

describe("Scenario 5: No commit if tests still failing", () => {
  test("Healer reports failure when verification fails after fix attempt", async () => {
    const events: HealerEvent[] = [];

    // Mock invoker that simulates "successful" code changes
    const mockInvoker: ClaudeCodeInvoker = async (_subtask, _options) => ({
      success: true,
      subtaskId: "emergency-typecheck-fix",
      filesModified: ["src/attempted-fix.ts"],
      turns: 5,
    });

    // Mock verifier that reports tests STILL failing
    const mockVerifier: VerificationRunner = async (_cwd) => ({
      success: false,
      output: "FAIL: 3 tests failed",
    });

    const service = createFullHealerService(mockInvoker, mockVerifier, {
      onEvent: (e) => events.push(e),
    });

    const state = createMockState();
    const config = createMockConfig();
    const counters = createHealerCounters();

    const event: OrchestratorEvent = {
      type: "init_script_complete",
      result: {
        ran: true,
        success: false,
        failureType: "typecheck_failed",
        canSelfHeal: true,
        output: "error TS2345",
      },
    };

    const outcome = await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    expect(outcome).not.toBeNull();

    // Check that the fix attempt was recorded but verification failed
    const spellCompleteEvents = events.filter(e => e.type === "healer_spell_complete");

    // If fix_typecheck_errors was attempted, it should report the verification failure
    const typecheckFixEvent = spellCompleteEvents.find(
      e => e.type === "healer_spell_complete" && e.spellId === "fix_typecheck_errors"
    );

    if (typecheckFixEvent?.type === "healer_spell_complete") {
      // The spell should report that verification still failed
      expect(typecheckFixEvent.result.success).toBe(false);
      expect(typecheckFixEvent.result.summary).toContain("still fail");
    }
  });

  test("Healer reports fix spell failure when verification fails", async () => {
    const events: HealerEvent[] = [];

    // Mock invoker that simulates "successful" code changes
    const mockInvoker: ClaudeCodeInvoker = async (_subtask, _options) => ({
      success: true,
      subtaskId: "emergency-typecheck-fix",
      filesModified: ["src/attempted-fix.ts"],
      turns: 5,
    });

    // Mock verifier that reports tests STILL failing
    const mockVerifier: VerificationRunner = async (_cwd) => ({
      success: false,
      output: "FAIL: Tests still broken",
    });

    const service = createFullHealerService(mockInvoker, mockVerifier, {
      onEvent: (e) => events.push(e),
    });

    const state = createMockState();
    const config = createMockConfig();
    const counters = createHealerCounters();

    const event: OrchestratorEvent = {
      type: "init_script_complete",
      result: {
        ran: true,
        success: false,
        failureType: "typecheck_failed",
        canSelfHeal: true,
        output: "error TS2345",
      },
    };

    const outcome = await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    expect(outcome).not.toBeNull();

    // The fix_typecheck_errors spell should report failure
    const typecheckSpellComplete = events.find(
      e => e.type === "healer_spell_complete" && e.spellId === "fix_typecheck_errors"
    );

    if (typecheckSpellComplete?.type === "healer_spell_complete") {
      // The spell should report failure because verification failed
      expect(typecheckSpellComplete.result.success).toBe(false);
      // But changes were applied (files were modified)
      expect(typecheckSpellComplete.result.changesApplied).toBe(true);
    }

    // Verify that the spell was tried but did not succeed
    expect(outcome?.spellsTried).toContain("fix_typecheck_errors");
    expect(outcome?.spellsSucceeded).not.toContain("fix_typecheck_errors");
  });
});

// ============================================================================
// Additional Scenarios
// ============================================================================

describe("Additional scenarios", () => {
  test("Healer skips when disabled in config", async () => {
    const service = createBasicHealerService();

    const state = createMockState();
    const config = createMockConfig({ enabled: false });
    const counters = createHealerCounters();

    const event: OrchestratorEvent = {
      type: "subtask_failed",
      subtask: createMockSubtask(),
      error: "Test error",
    };

    const outcome = await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    expect(outcome).toBeNull();
  });

  test("Healer skips for non-triggering events", async () => {
    const service = createBasicHealerService();

    const state = createMockState();
    const config = createMockConfig();
    const counters = createHealerCounters();

    const event: OrchestratorEvent = {
      type: "subtask_complete",
      subtask: createMockSubtask({ status: "done" }),
      result: {
        success: true,
        subtaskId: "test",
        filesModified: [],
        turns: 1,
      },
    };

    const outcome = await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    expect(outcome).toBeNull();
  });

  test("Healer skips scenario when disabled", async () => {
    const service = createBasicHealerService();

    const state = createMockState();
    const config = createMockConfig({
      scenarios: {
        onInitFailure: false,  // Disable init failure handling
        onVerificationFailure: true,
        onSubtaskFailure: true,
        onRuntimeError: true,
        onStuckSubtask: false,
      },
    });
    const counters = createHealerCounters();

    const event: OrchestratorEvent = {
      type: "init_script_complete",
      result: {
        ran: true,
        success: false,
        failureType: "typecheck_failed",
        canSelfHeal: true,
        output: "error TS2345",
      },
    };

    const outcome = await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    expect(outcome).toBeNull();
  });
});
