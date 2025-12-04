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
import {
  createMockProjectConfig,
  createMockOrchestratorState,
} from "./test-helpers.js";

// ============================================================================
// Test Helpers
// ============================================================================

const createMockState = (): OrchestratorState => createMockOrchestratorState();

const createMockConfig = (): ProjectConfig => createMockProjectConfig();

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
