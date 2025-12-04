/**
 * Tests for Healer Service
 */
import { describe, test, expect } from "bun:test";
import { Effect } from "effect";
import {
  createHealerService,
  createBasicHealerService,
  createFullHealerService,
} from "../service.js";
import type { HealerEvent } from "../service.js";
import { createHealerCounters } from "../types.js";
import type { OrchestratorEvent, OrchestratorState } from "../../agent/orchestrator/types.js";
import type { ProjectConfig } from "../../tasks/schema.js";

// ============================================================================
// Test Helpers
// ============================================================================

const createMockState = (): OrchestratorState => ({
  sessionId: "session-123",
  phase: "executing",
  subtasks: {
    completed: [],
    subtasks: [],
  },
});

const createMockConfig = (): ProjectConfig => ({
  defaultBranch: "main",
  testCommands: ["bun test"],
  healer: {
    enabled: true,
    scenarios: {},
    spells: {},
    maxInvocationsPerSession: 5,
    maxInvocationsPerSubtask: 3,
  },
});

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
      type: "phase_transition",
      from: "orienting",
      to: "planning",
    };

    const result = await Effect.runPromise(
      service.maybeRun(event, state, config, counters)
    );

    expect(result).toBeNull();
  });

  test("maybeRun returns null when Healer is disabled", async () => {
    const service = createHealerService();
    const state = createMockState();
    const config: ProjectConfig = {
      ...createMockConfig(),
      healer: { enabled: false },
    };
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
