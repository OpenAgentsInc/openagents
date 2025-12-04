/**
 * Tests for Healer Service
 */
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import * as path from "node:path";
import { describe, test, expect } from "bun:test";
import { Effect } from "effect";
import {
  createHealerService,
  createBasicHealerService,
  createFullHealerService,
} from "../service.js";
import type { HealerEvent } from "../service.js";
import {
  createHealerCounters,
  type HealerContext,
  type HealerScenario,
  type HealerSpellId,
  type HealerOutcomeStatus,
} from "../types.js";
import type { OrchestratorEvent, OrchestratorState } from "../../agent/orchestrator/types.js";
import type { ProjectConfig } from "../../tasks/schema.js";
import {
  createMockProjectConfig,
  createMockOrchestratorState,
  createMockSubtask,
} from "./test-helpers.js";

// ============================================================================
// Test Helpers
// ============================================================================

const createMockState = (): OrchestratorState => createMockOrchestratorState();

const createMockConfig = (overrides: Partial<ProjectConfig> = {}): ProjectConfig =>
  createMockProjectConfig(overrides);

// ============================================================================
// createHealerService Tests
// ============================================================================

describe("createHealerService", () => {
  test("creates service with default options", () => {
    const service = createHealerService();
    expect(service.maybeRun).toBeDefined();
    expect(service.run).toBeDefined();
  });

  test("maybeRun returns null for non-healable events", async () => {
    const service = createHealerService();
    const state = createMockState();
    const config = createMockConfig();
    const counters = createHealerCounters();

    const event: OrchestratorEvent = {
      type: "session_start",
      sessionId: "session-123",
      timestamp: "2024-01-01T00:00:00Z",
    };

    const result = await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    expect(result).toBeNull();
  });

  test("maybeRun returns null when Healer is disabled", async () => {
    const service = createHealerService();
    const state = createMockState();
    const config = createMockProjectConfig({
      healer: {
        enabled: false,
        maxInvocationsPerSession: 5,
        maxInvocationsPerSubtask: 3,
        mode: "conservative",
        stuckThresholdHours: 2,
        scenarios: {
          onInitFailure: false,
          onVerificationFailure: false,
          onSubtaskFailure: false,
          onRuntimeError: false,
          onStuckSubtask: false,
        },
        spells: {
          allowed: [],
          forbidden: [],
        },
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

    const result = await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    expect(result).toBeNull();
  });
});

// ============================================================================
// createBasicHealerService Tests
// ============================================================================

describe("createBasicHealerService", () => {
  test("creates service that skips LLM spells", () => {
    const events: HealerEvent[] = [];
    const service = createBasicHealerService((event) => events.push(event));

    expect(service.maybeRun).toBeDefined();
    expect(service.run).toBeDefined();
  });
});

// ============================================================================
// createFullHealerService Tests
// ============================================================================

describe("createFullHealerService", () => {
  test("creates service with LLM capabilities", () => {
    const mockInvoker = async () => ({
      success: true,
      subtaskId: "test",
      filesModified: [],
      turns: 1,
    });

    const mockVerifier = async () => ({
      success: true,
      output: "All checks passed",
    });

    const service = createFullHealerService(mockInvoker, mockVerifier);

    expect(service.maybeRun).toBeDefined();
    expect(service.run).toBeDefined();
  });
});

// ============================================================================
// Deduplication
// ============================================================================

describe("healer invocation deduplication", () => {
  const buildStateWithFailure = (): OrchestratorState => {
    const state = createMockState();
    const subtask = createMockSubtask({
      id: "subtask-1",
      status: "failed",
      lastFailureReason: "boom",
      failureCount: 1,
    });
    state.subtasks = {
      taskId: "task-123",
      taskTitle: "Test task",
      subtasks: [subtask],
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };
    return state;
  };

  const event: OrchestratorEvent = {
    type: "verification_complete",
    command: "bun test",
    passed: false,
    output: "boom",
  };

  const makeRunner = (statuses: HealerOutcomeStatus[]) => {
    let calls = 0;
    const runner = (
      _ctx: HealerContext,
      scenario: HealerScenario,
      spells: HealerSpellId[]
    ) => {
      const status = statuses[Math.min(calls, statuses.length - 1)];
      calls += 1;
      return Effect.succeed({
        scenario,
        status,
        spellsTried: spells,
        spellsSucceeded:
          status === "resolved" || status === "contained" ? spells : [],
        summary: status,
      });
    };
    return { runner, getCalls: () => calls };
  };

  test("skips when the same failure was already resolved", async () => {
    const tempDir = await mkdtemp(path.join(tmpdir(), "healer-state-"));
    const openagentsDir = path.join(tempDir, ".openagents");
    const { runner, getCalls } = makeRunner(["resolved"]);

    const service = createHealerService({
      openagentsDir,
      skipLLMSpells: true,
      spellRunner: runner,
    });
    const config = createMockConfig({ rootDir: tempDir });
    const state = buildStateWithFailure();
    const counters = createHealerCounters();

    const first = await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    expect(first?.status).toBe("resolved");
    expect(getCalls()).toBe(1);

    // Simulate restart with fresh counters and service but reuse persisted state.
    const newCounters = createHealerCounters();
    const freshService = createHealerService({
      openagentsDir,
      skipLLMSpells: true,
      spellRunner: runner,
    });
    const second = await Effect.runPromise(
      freshService.maybeRun(event, state, config, newCounters)
    );

    expect(second?.status).toBe("skipped");
    expect(getCalls()).toBe(1);
  });

  test("retries when previous attempt was unresolved", async () => {
    const tempDir = await mkdtemp(path.join(tmpdir(), "healer-state-"));
    const openagentsDir = path.join(tempDir, ".openagents");
    const { runner, getCalls } = makeRunner(["unresolved", "resolved"]);

    const service = createHealerService({
      openagentsDir,
      skipLLMSpells: true,
      spellRunner: runner,
    });
    const config = createMockConfig({ rootDir: tempDir });
    const state = buildStateWithFailure();

    const first = await Effect.runPromise(
      service.maybeRun(event, state, config, createHealerCounters())
    );

    expect(first?.status).toBe("unresolved");
    expect(getCalls()).toBe(1);

    const second = await Effect.runPromise(
      service.maybeRun(event, state, config, createHealerCounters())
    );

    expect(second?.status).toBe("resolved");
    expect(getCalls()).toBe(2);
  });
});
