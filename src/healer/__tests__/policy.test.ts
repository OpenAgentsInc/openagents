/**
 * Tests for Healer policy.
 */
import { describe, test, expect } from "bun:test";
import * as S from "effect/Schema";
import type { OrchestratorEvent } from "../../agent/orchestrator/types.js";
import { HealerConfig } from "../../tasks/schema.js";
import { createHealerCounters, type HealerCounters } from "../types.js";
import {
  mapEventToScenario,
  shouldRunHealer,
  isScenarioEnabled,
  hasExceededSessionLimit,
  hasExceededSubtaskLimit,
  incrementCounters,
  getErrorOutput,
  isHealableInitFailure,
} from "../policy.js";

// Helper to create default HealerConfig
const defaultConfig = (): HealerConfig => S.decodeUnknownSync(HealerConfig)({});

describe("Healer Policy", () => {
  describe("mapEventToScenario", () => {
    test("maps init_script_complete with typecheck failure", () => {
      const event: OrchestratorEvent = {
        type: "init_script_complete",
        result: {
          ran: true,
          success: false,
          failureType: "typecheck_failed",
          canSelfHeal: true,
        },
      };
      expect(mapEventToScenario(event)).toBe("InitScriptTypecheckFailure");
    });

    test("maps init_script_complete with test failure", () => {
      const event: OrchestratorEvent = {
        type: "init_script_complete",
        result: {
          ran: true,
          success: false,
          failureType: "test_failed",
          canSelfHeal: true,
        },
      };
      expect(mapEventToScenario(event)).toBe("InitScriptTestFailure");
    });

    test("maps init_script_complete with network error to environment failure", () => {
      const event: OrchestratorEvent = {
        type: "init_script_complete",
        result: {
          ran: true,
          success: false,
          failureType: "network_error",
          canSelfHeal: false,
        },
      };
      expect(mapEventToScenario(event)).toBe("InitScriptEnvironmentFailure");
    });

    test("returns null for successful init_script_complete", () => {
      const event: OrchestratorEvent = {
        type: "init_script_complete",
        result: { ran: true, success: true },
      };
      expect(mapEventToScenario(event)).toBeNull();
    });

    test("maps subtask_failed", () => {
      const event: OrchestratorEvent = {
        type: "subtask_failed",
        subtask: { id: "st-1", description: "test", status: "failed" },
        error: "Test error",
      };
      expect(mapEventToScenario(event)).toBe("SubtaskFailed");
    });

    test("maps verification_complete with failure", () => {
      const event: OrchestratorEvent = {
        type: "verification_complete",
        command: "bun test",
        passed: false,
        output: "1 test failed",
      };
      expect(mapEventToScenario(event)).toBe("VerificationFailed");
    });

    test("returns null for successful verification_complete", () => {
      const event: OrchestratorEvent = {
        type: "verification_complete",
        command: "bun test",
        passed: true,
        output: "All tests passed",
      };
      expect(mapEventToScenario(event)).toBeNull();
    });

    test("maps error event", () => {
      const event: OrchestratorEvent = {
        type: "error",
        phase: "executing_subtask",
        error: "Unexpected error",
      };
      expect(mapEventToScenario(event)).toBe("RuntimeError");
    });

    test("returns null for non-failure events", () => {
      const events: OrchestratorEvent[] = [
        { type: "session_start", sessionId: "s-1", timestamp: "2024-01-01" },
        { type: "task_selected", task: {} as any },
        { type: "subtask_start", subtask: {} as any },
        { type: "subtask_complete", subtask: {} as any, result: {} as any },
        { type: "session_complete", success: true, summary: "Done" },
      ];
      for (const event of events) {
        expect(mapEventToScenario(event)).toBeNull();
      }
    });
  });

  describe("isScenarioEnabled", () => {
    test("all scenarios enabled by default", () => {
      const config = defaultConfig();
      expect(isScenarioEnabled("InitScriptTypecheckFailure", config)).toBe(true);
      expect(isScenarioEnabled("InitScriptTestFailure", config)).toBe(true);
      expect(isScenarioEnabled("VerificationFailed", config)).toBe(true);
      expect(isScenarioEnabled("SubtaskFailed", config)).toBe(true);
      expect(isScenarioEnabled("RuntimeError", config)).toBe(true);
    });

    test("SubtaskStuck disabled by default", () => {
      const config = defaultConfig();
      expect(isScenarioEnabled("SubtaskStuck", config)).toBe(false);
    });

    test("respects explicit scenario config", () => {
      const config = S.decodeUnknownSync(HealerConfig)({
        scenarios: {
          onInitFailure: false,
          onVerificationFailure: false,
          onSubtaskFailure: true,
          onStuckSubtask: true,
        },
      });
      expect(isScenarioEnabled("InitScriptTypecheckFailure", config)).toBe(false);
      expect(isScenarioEnabled("VerificationFailed", config)).toBe(false);
      expect(isScenarioEnabled("SubtaskFailed", config)).toBe(true);
      expect(isScenarioEnabled("SubtaskStuck", config)).toBe(true);
    });
  });

  describe("hasExceededSessionLimit", () => {
    test("returns false when under limit", () => {
      const counters = createHealerCounters();
      counters.sessionInvocations = 1;
      const config = defaultConfig(); // limit = 2
      expect(hasExceededSessionLimit(counters, config)).toBe(false);
    });

    test("returns true when at limit", () => {
      const counters = createHealerCounters();
      counters.sessionInvocations = 2;
      const config = defaultConfig(); // limit = 2
      expect(hasExceededSessionLimit(counters, config)).toBe(true);
    });

    test("returns true when over limit", () => {
      const counters = createHealerCounters();
      counters.sessionInvocations = 5;
      const config = defaultConfig();
      expect(hasExceededSessionLimit(counters, config)).toBe(true);
    });

    test("respects custom limit", () => {
      const counters = createHealerCounters();
      counters.sessionInvocations = 3;
      const config = S.decodeUnknownSync(HealerConfig)({
        maxInvocationsPerSession: 5,
      });
      expect(hasExceededSessionLimit(counters, config)).toBe(false);
    });
  });

  describe("hasExceededSubtaskLimit", () => {
    test("returns false for undefined subtaskId", () => {
      const counters = createHealerCounters();
      const config = defaultConfig();
      expect(hasExceededSubtaskLimit(undefined, counters, config)).toBe(false);
    });

    test("returns false when no invocations for subtask", () => {
      const counters = createHealerCounters();
      const config = defaultConfig();
      expect(hasExceededSubtaskLimit("st-1", counters, config)).toBe(false);
    });

    test("returns true when at limit", () => {
      const counters = createHealerCounters();
      counters.subtaskInvocations.set("st-1", 1);
      const config = defaultConfig(); // limit = 1
      expect(hasExceededSubtaskLimit("st-1", counters, config)).toBe(true);
    });

    test("tracks different subtasks independently", () => {
      const counters = createHealerCounters();
      counters.subtaskInvocations.set("st-1", 1);
      counters.subtaskInvocations.set("st-2", 0);
      const config = defaultConfig();
      expect(hasExceededSubtaskLimit("st-1", counters, config)).toBe(true);
      expect(hasExceededSubtaskLimit("st-2", counters, config)).toBe(false);
    });
  });

  describe("incrementCounters", () => {
    test("increments session count", () => {
      const counters = createHealerCounters();
      incrementCounters(counters);
      expect(counters.sessionInvocations).toBe(1);
      incrementCounters(counters);
      expect(counters.sessionInvocations).toBe(2);
    });

    test("increments subtask count when provided", () => {
      const counters = createHealerCounters();
      incrementCounters(counters, "st-1");
      expect(counters.subtaskInvocations.get("st-1")).toBe(1);
      incrementCounters(counters, "st-1");
      expect(counters.subtaskInvocations.get("st-1")).toBe(2);
    });

    test("does not create subtask entry without subtaskId", () => {
      const counters = createHealerCounters();
      incrementCounters(counters);
      expect(counters.subtaskInvocations.size).toBe(0);
    });
  });

  describe("shouldRunHealer", () => {
    test("returns run=false when disabled", () => {
      const event: OrchestratorEvent = {
        type: "subtask_failed",
        subtask: { id: "st-1", description: "test", status: "failed" },
        error: "Error",
      };
      const config = S.decodeUnknownSync(HealerConfig)({ enabled: false });
      const counters = createHealerCounters();

      const decision = shouldRunHealer(event, config, counters);
      expect(decision.run).toBe(false);
      expect(decision.reason).toContain("disabled");
    });

    test("returns run=false for non-triggering events", () => {
      const event: OrchestratorEvent = {
        type: "session_start",
        sessionId: "s-1",
        timestamp: "2024-01-01",
      };
      const config = defaultConfig();
      const counters = createHealerCounters();

      const decision = shouldRunHealer(event, config, counters);
      expect(decision.run).toBe(false);
      expect(decision.reason).toContain("does not trigger");
    });

    test("returns run=false when scenario disabled", () => {
      const event: OrchestratorEvent = {
        type: "subtask_failed",
        subtask: { id: "st-1", description: "test", status: "failed" },
        error: "Error",
      };
      const config = S.decodeUnknownSync(HealerConfig)({
        scenarios: { onSubtaskFailure: false },
      });
      const counters = createHealerCounters();

      const decision = shouldRunHealer(event, config, counters);
      expect(decision.run).toBe(false);
      expect(decision.scenario).toBe("SubtaskFailed");
      expect(decision.reason).toContain("disabled");
    });

    test("returns run=false when session limit exceeded", () => {
      const event: OrchestratorEvent = {
        type: "subtask_failed",
        subtask: { id: "st-1", description: "test", status: "failed" },
        error: "Error",
      };
      const config = defaultConfig();
      const counters = createHealerCounters();
      counters.sessionInvocations = 2;

      const decision = shouldRunHealer(event, config, counters);
      expect(decision.run).toBe(false);
      expect(decision.reason).toContain("Session limit");
    });

    test("returns run=false when subtask limit exceeded", () => {
      const event: OrchestratorEvent = {
        type: "subtask_failed",
        subtask: { id: "st-1", description: "test", status: "failed" },
        error: "Error",
      };
      const config = defaultConfig();
      const counters = createHealerCounters();
      counters.subtaskInvocations.set("st-1", 1);

      const decision = shouldRunHealer(event, config, counters, "st-1");
      expect(decision.run).toBe(false);
      expect(decision.reason).toContain("Subtask limit");
    });

    test("returns run=true when all checks pass", () => {
      const event: OrchestratorEvent = {
        type: "subtask_failed",
        subtask: { id: "st-1", description: "test", status: "failed" },
        error: "Error",
      };
      const config = defaultConfig();
      const counters = createHealerCounters();

      const decision = shouldRunHealer(event, config, counters);
      expect(decision.run).toBe(true);
      expect(decision.scenario).toBe("SubtaskFailed");
      expect(decision.reason).toContain("Triggering Healer");
    });

    test("works for init script typecheck failure", () => {
      const event: OrchestratorEvent = {
        type: "init_script_complete",
        result: {
          ran: true,
          success: false,
          failureType: "typecheck_failed",
          canSelfHeal: true,
          output: "error TS2322: Type mismatch",
        },
      };
      const config = defaultConfig();
      const counters = createHealerCounters();

      const decision = shouldRunHealer(event, config, counters);
      expect(decision.run).toBe(true);
      expect(decision.scenario).toBe("InitScriptTypecheckFailure");
    });

    test("works for verification failure", () => {
      const event: OrchestratorEvent = {
        type: "verification_complete",
        command: "bun test",
        passed: false,
        output: "1 test failed",
      };
      const config = defaultConfig();
      const counters = createHealerCounters();

      const decision = shouldRunHealer(event, config, counters);
      expect(decision.run).toBe(true);
      expect(decision.scenario).toBe("VerificationFailed");
    });
  });

  describe("getErrorOutput", () => {
    test("extracts output from init_script_complete", () => {
      const event: OrchestratorEvent = {
        type: "init_script_complete",
        result: { ran: true, success: false, output: "Type error on line 5" },
      };
      expect(getErrorOutput(event)).toBe("Type error on line 5");
    });

    test("extracts error from subtask_failed", () => {
      const event: OrchestratorEvent = {
        type: "subtask_failed",
        subtask: { id: "st-1", description: "test", status: "failed" },
        error: "Subtask error message",
      };
      expect(getErrorOutput(event)).toBe("Subtask error message");
    });

    test("extracts output from verification_complete", () => {
      const event: OrchestratorEvent = {
        type: "verification_complete",
        command: "bun test",
        passed: false,
        output: "3 tests failed",
      };
      expect(getErrorOutput(event)).toBe("3 tests failed");
    });

    test("returns null for events without error output", () => {
      const event: OrchestratorEvent = {
        type: "session_start",
        sessionId: "s-1",
        timestamp: "2024-01-01",
      };
      expect(getErrorOutput(event)).toBeNull();
    });
  });

  describe("isHealableInitFailure", () => {
    test("returns true when canSelfHeal is true", () => {
      expect(isHealableInitFailure({
        ran: true,
        success: false,
        canSelfHeal: true,
      })).toBe(true);
    });

    test("returns false when canSelfHeal is false", () => {
      expect(isHealableInitFailure({
        ran: true,
        success: false,
        canSelfHeal: false,
      })).toBe(false);
    });

    test("returns false when canSelfHeal is undefined", () => {
      expect(isHealableInitFailure({
        ran: true,
        success: false,
      })).toBe(false);
    });
  });
});
