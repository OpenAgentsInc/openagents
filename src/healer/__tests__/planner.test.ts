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
import { createHealerCounters } from "../types.js";

// ============================================================================
// Test Helpers
// ============================================================================

const createMockContext = (
  scenario: HealerScenario,
  overrides: Partial<HealerContext> = {}
): HealerContext => ({
  projectRoot: "/tmp/test-project",
  projectConfig: {
    defaultBranch: "main",
    testCommands: ["bun test"],
    healer: {
      enabled: true,
      scenarios: {},
      spells: {},
      maxInvocationsPerSession: 5,
      maxInvocationsPerSubtask: 3,
    },
  },
  task: undefined,
  subtask: undefined,
  sessionId: "session-123",
  runId: undefined,
  trajectory: undefined,
  relatedTrajectories: [],
  progressMd: null,
  gitStatus: {
    isDirty: false,
    modifiedFiles: [],
    untrackedFiles: [],
    currentBranch: "main",
    lastCommitSha: "abc123",
    lastCommitMessage: "Initial commit",
  },
  heuristics: {
    scenario,
    failureCount: 1,
    isFlaky: false,
    hasMissingImports: false,
    hasTypeErrors: false,
    hasTestAssertions: false,
    errorPatterns: [],
    previousAttempts: 0,
  },
  triggerEvent: { type: "subtask-failed" } as any,
  orchestratorState: {} as any,
  initFailureType: undefined,
  errorOutput: "Test error output",
  counters: createHealerCounters(),
  ...overrides,
});

// ============================================================================
// getScenarioSpells Tests
// ============================================================================

describe("getScenarioSpells", () => {
  test("returns spells for InitScriptTypecheckFailure", () => {
    const spells = getScenarioSpells("InitScriptTypecheckFailure");
    expect(spells).toContain("fix_typecheck_errors");
    expect(spells).toContain("update_progress_with_guidance");
  });

  test("returns spells for SubtaskFailed", () => {
    const spells = getScenarioSpells("SubtaskFailed");
    expect(spells).toContain("rewind_uncommitted_changes");
    expect(spells).toContain("update_progress_with_guidance");
  });

  test("returns empty array for unknown scenario", () => {
    const spells = getScenarioSpells("UnknownScenario" as HealerScenario);
    expect(spells).toEqual([]);
  });
});

// ============================================================================
// hasScenarioSpells Tests
// ============================================================================

describe("hasScenarioSpells", () => {
  test("returns true for known scenarios", () => {
    expect(hasScenarioSpells("InitScriptTypecheckFailure")).toBe(true);
    expect(hasScenarioSpells("SubtaskFailed")).toBe(true);
  });

  test("returns false for unknown scenarios", () => {
    expect(hasScenarioSpells("UnknownScenario" as HealerScenario)).toBe(false);
  });
});

// ============================================================================
// getScenariosUsingSpell Tests
// ============================================================================

describe("getScenariosUsingSpell", () => {
  test("finds scenarios using rewind_uncommitted_changes", () => {
    const scenarios = getScenariosUsingSpell("rewind_uncommitted_changes");
    expect(scenarios).toContain("SubtaskFailed");
    expect(scenarios).toContain("VerificationFailed");
  });

  test("finds scenarios using update_progress_with_guidance", () => {
    const scenarios = getScenariosUsingSpell("update_progress_with_guidance");
    expect(scenarios.length).toBeGreaterThan(3);
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
      projectConfig: {
        defaultBranch: "main",
        testCommands: ["bun test"],
        healer: {
          enabled: true,
          spells: {
            forbidden: ["rewind_uncommitted_changes"],
          },
        },
      },
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
    const spells = planSpells(ctx, { maxSpells: 1 });
    expect(spells.length).toBe(1);
  });
});
