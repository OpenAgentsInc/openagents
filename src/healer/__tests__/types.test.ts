/**
 * Tests for Healer types and helpers.
 */
import { describe, test, expect } from "bun:test";
import {
  createHealerCounters,
  isResolved,
  isContained,
  spellRequiresLLM,
  mapInitFailureToScenario,
  type HealerOutcome,
  type HealerSpellId,
} from "../types.js";

describe("Healer Types", () => {
  describe("createHealerCounters", () => {
    test("creates fresh counters", () => {
      const counters = createHealerCounters();
      expect(counters.sessionInvocations).toBe(0);
      expect(counters.subtaskInvocations.size).toBe(0);
      expect(counters.spellsAttempted.size).toBe(0);
      expect(counters.healingAttempts.size).toBe(0);
    });

    test("counters are mutable", () => {
      const counters = createHealerCounters();
      counters.sessionInvocations = 5;
      counters.subtaskInvocations.set("subtask-1", 2);
      counters.spellsAttempted.set("fix_typecheck_errors", 1);
      counters.healingAttempts.set("task:subtask:scenario:hash", {
        key: "task:subtask:scenario:hash",
        scenario: "SubtaskFailed",
        taskId: "task",
        subtaskId: "subtask",
        errorHash: "hash",
        timestamp: "2024-01-01T00:00:00Z",
        outcome: "resolved",
        spellsTried: [],
        spellsSucceeded: [],
        summary: "resolved",
      });

      expect(counters.sessionInvocations).toBe(5);
      expect(counters.subtaskInvocations.get("subtask-1")).toBe(2);
      expect(counters.spellsAttempted.get("fix_typecheck_errors")).toBe(1);
      expect(counters.healingAttempts.get("task:subtask:scenario:hash")).toBeDefined();
    });
  });

  describe("isResolved", () => {
    test("returns true for resolved outcomes", () => {
      const outcome: HealerOutcome = {
        scenario: "SubtaskFailed",
        status: "resolved",
        spellsTried: ["fix_typecheck_errors"],
        spellsSucceeded: ["fix_typecheck_errors"],
        summary: "Fixed typecheck errors",
      };
      expect(isResolved(outcome)).toBe(true);
    });

    test("returns false for non-resolved outcomes", () => {
      const outcomes: HealerOutcome["status"][] = ["contained", "unresolved", "skipped"];
      for (const status of outcomes) {
        const outcome: HealerOutcome = {
          scenario: "SubtaskFailed",
          status,
          spellsTried: [],
          spellsSucceeded: [],
          summary: "Test",
        };
        expect(isResolved(outcome)).toBe(false);
      }
    });
  });

  describe("isContained", () => {
    test("returns true for contained outcomes", () => {
      const outcome: HealerOutcome = {
        scenario: "VerificationFailed",
        status: "contained",
        spellsTried: ["mark_task_blocked_with_followup"],
        spellsSucceeded: ["mark_task_blocked_with_followup"],
        summary: "Marked task as blocked",
      };
      expect(isContained(outcome)).toBe(true);
    });

    test("returns false for non-contained outcomes", () => {
      const outcomes: HealerOutcome["status"][] = ["resolved", "unresolved", "skipped"];
      for (const status of outcomes) {
        const outcome: HealerOutcome = {
          scenario: "SubtaskFailed",
          status,
          spellsTried: [],
          spellsSucceeded: [],
          summary: "Test",
        };
        expect(isContained(outcome)).toBe(false);
      }
    });
  });

  describe("spellRequiresLLM", () => {
    test("returns true for LLM spells", () => {
      const llmSpells: HealerSpellId[] = [
        "fix_typecheck_errors",
        "fix_test_errors",
        "retry_with_claude_code_resume",
        "retry_with_minimal_subagent",
      ];
      for (const spell of llmSpells) {
        expect(spellRequiresLLM(spell)).toBe(true);
      }
    });

    test("returns false for non-LLM spells", () => {
      const nonLlmSpells: HealerSpellId[] = [
        "rewind_uncommitted_changes",
        "rewind_to_last_green_commit",
        "mark_task_blocked_with_followup",
        "update_progress_with_guidance",
        "run_tasks_doctor_like_checks",
      ];
      for (const spell of nonLlmSpells) {
        expect(spellRequiresLLM(spell)).toBe(false);
      }
    });
  });

  describe("mapInitFailureToScenario", () => {
    test("maps typecheck_failed to InitScriptTypecheckFailure", () => {
      expect(mapInitFailureToScenario("typecheck_failed")).toBe("InitScriptTypecheckFailure");
    });

    test("maps test_failed to InitScriptTestFailure", () => {
      expect(mapInitFailureToScenario("test_failed")).toBe("InitScriptTestFailure");
    });

    test("maps environment failures to InitScriptEnvironmentFailure", () => {
      expect(mapInitFailureToScenario("network_error")).toBe("InitScriptEnvironmentFailure");
      expect(mapInitFailureToScenario("disk_full")).toBe("InitScriptEnvironmentFailure");
      expect(mapInitFailureToScenario("permission_denied")).toBe("InitScriptEnvironmentFailure");
      expect(mapInitFailureToScenario("unknown")).toBe("InitScriptEnvironmentFailure");
    });
  });
});
