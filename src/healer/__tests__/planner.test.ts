/**
 * Tests for Healer Spell Planner
 */
import { describe, test, expect } from "bun:test";
import {
  planSpells,
  getScenarioSpells,
  hasScenarioSpells,
  getScenariosUsingSpell,
} from "../planner.js";
import type { HealerContext, HealerScenario } from "../types.js";
import { createMockProjectConfig, createMockHealerContext } from "./test-helpers.js";

// ============================================================================
// Test Helpers
// ============================================================================

const createMockContext = (
  scenario: HealerScenario,
  overrides: Partial<HealerContext> = {}
): HealerContext => createMockHealerContext(scenario, {
  projectConfig: createMockProjectConfig({
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
        onStuckSubtask: true,
      },
      spells: {
        allowed: [],
        forbidden: [],
      },
    },
  }),
  ...overrides,
});

// ============================================================================
// getScenarioSpells Tests
// ============================================================================

describe("getScenarioSpells", () => {
  test("returns expected spells for SubtaskFailed", () => {
    const spells = getScenarioSpells("SubtaskFailed");
    expect(spells).toContain("rewind_uncommitted_changes");
    expect(spells).toContain("mark_task_blocked_with_followup");
  });

  test("returns expected spells for InitScriptTypecheckFailure", () => {
    const spells = getScenarioSpells("InitScriptTypecheckFailure");
    expect(spells).toContain("fix_typecheck_errors");
  });

  test("returns expected spells for VerificationFailed", () => {
    const spells = getScenarioSpells("VerificationFailed");
    expect(spells).toContain("rewind_uncommitted_changes");
  });
});

// ============================================================================
// hasScenarioSpells Tests
// ============================================================================

describe("hasScenarioSpells", () => {
  test("returns true for known scenarios", () => {
    expect(hasScenarioSpells("SubtaskFailed")).toBe(true);
    expect(hasScenarioSpells("InitScriptTypecheckFailure")).toBe(true);
  });

  test("returns true for RuntimeError", () => {
    expect(hasScenarioSpells("RuntimeError")).toBe(true);
  });
});

// ============================================================================
// getScenariosUsingSpell Tests
// ============================================================================

describe("getScenariosUsingSpell", () => {
  test("returns scenarios that use a given spell", () => {
    const scenarios = getScenariosUsingSpell("rewind_uncommitted_changes");
    expect(scenarios).toContain("SubtaskFailed");
    expect(scenarios).toContain("VerificationFailed");
  });

  test("returns empty array for unused spell", () => {
    const scenarios = getScenariosUsingSpell("nonexistent_spell" as any);
    expect(scenarios).toHaveLength(0);
  });
});

// ============================================================================
// planSpells Tests
// ============================================================================

describe("planSpells", () => {
  test("returns default spells for scenario", () => {
    const ctx = createMockContext("SubtaskFailed");
    const spells = planSpells(ctx);
    expect(spells).toContain("rewind_uncommitted_changes");
  });

  test("filters out forbidden spells", () => {
    const ctx = createMockContext("SubtaskFailed", {
      projectConfig: createMockProjectConfig({
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
            onStuckSubtask: true,
          },
          spells: {
            allowed: [],
            forbidden: ["rewind_uncommitted_changes"],
          },
        },
      }),
    });
    const spells = planSpells(ctx);
    expect(spells).not.toContain("rewind_uncommitted_changes");
  });

  test("skips LLM spells when requested", () => {
    const ctx = createMockContext("InitScriptTypecheckFailure");
    const spells = planSpells(ctx, { skipLLMSpells: true });
    expect(spells).not.toContain("fix_typecheck_errors");
  });

  test("limits number of spells", () => {
    const ctx = createMockContext("SubtaskFailed");
    const spells = planSpells(ctx);
    // Default limit is 3
    expect(spells.length).toBeLessThanOrEqual(3);
  });
});
