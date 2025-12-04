/**
 * Tests for Healer Spells
 */
import { describe, test, expect } from "bun:test";
import { Effect } from "effect";
import {
  spellRegistry,
  getSpell,
  hasSpell,
  getRegisteredSpellIds,
  executeSpell,
  executeSpells,
  isSpellAllowed,
  filterAllowedSpells,
} from "../spells/index.js";
import { rewindUncommittedChanges } from "../spells/rewind.js";
import { markTaskBlockedWithFollowup } from "../spells/blocked.js";
import { updateProgressWithGuidance, generateHealerSummary } from "../spells/progress.js";
import type { HealerContext, HealerSpellId, HealerSpellResult } from "../types.js";
import {
  createMockProjectConfig,
  createMockHealerContext,
} from "./test-helpers.js";

// ============================================================================
// Test Helpers
// ============================================================================

const createMockContext = (overrides: Partial<HealerContext> = {}): HealerContext =>
  createMockHealerContext("SubtaskFailed", {
    projectConfig: createMockProjectConfig(),
    errorOutput: "Test error output",
    ...overrides,
  });

// ============================================================================
// Spell Registry Tests
// ============================================================================

describe("Spell Registry", () => {
  test("spellRegistry contains all core spells", () => {
    expect(spellRegistry.size).toBe(5);
    expect(spellRegistry.has("rewind_uncommitted_changes")).toBe(true);
    expect(spellRegistry.has("mark_task_blocked_with_followup")).toBe(true);
    expect(spellRegistry.has("update_progress_with_guidance")).toBe(true);
    expect(spellRegistry.has("fix_typecheck_errors")).toBe(true);
    expect(spellRegistry.has("fix_test_errors")).toBe(true);
  });

  test("getSpell returns spell by ID", () => {
    const spell = getSpell("rewind_uncommitted_changes");
    expect(spell).toBeDefined();
    expect(spell?.id).toBe("rewind_uncommitted_changes");
  });

  test("getSpell returns undefined for unknown spell", () => {
    const spell = getSpell("unknown_spell" as HealerSpellId);
    expect(spell).toBeUndefined();
  });

  test("hasSpell checks spell existence", () => {
    expect(hasSpell("rewind_uncommitted_changes")).toBe(true);
    expect(hasSpell("unknown_spell" as HealerSpellId)).toBe(false);
  });

  test("getRegisteredSpellIds returns all spell IDs", () => {
    const ids = getRegisteredSpellIds();
    expect(ids).toContain("rewind_uncommitted_changes");
    expect(ids).toContain("mark_task_blocked_with_followup");
    expect(ids).toContain("update_progress_with_guidance");
    expect(ids).toContain("fix_typecheck_errors");
    expect(ids).toContain("fix_test_errors");
    expect(ids.length).toBe(5);
  });
});

// ============================================================================
// Spell Execution Tests
// ============================================================================

describe("Spell Execution", () => {
  test("executeSpell returns error for unknown spell", async () => {
    const ctx = createMockContext();
    const result = await Effect.runPromise(
      executeSpell("unknown_spell" as HealerSpellId, ctx)
    );

    expect(result.success).toBe(false);
    expect(result.error).toContain("Unknown spell");
  });

  test("executeSpells runs multiple spells in sequence", async () => {
    const ctx = createMockContext({
      task: {
        id: "oa-test123",
        title: "Test Task",
        description: "A test task",
        status: "in_progress",
        priority: 2,
        type: "task",
        labels: [],
        deps: [],
        commits: [],
        comments: [],
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        closedAt: null,
      },
    });
    const results = await Effect.runPromise(
      executeSpells(
        ["mark_task_blocked_with_followup", "update_progress_with_guidance"],
        ctx,
        { continueOnFailure: true }
      )
    );

    expect(results.length).toBe(2);
    expect(results[0].success).toBe(true);
    // Second spell (update_progress_with_guidance) may fail in test environment
    // because it tries to write to /test/root/.openagents/progress.md
    // The important thing is that both spells were attempted
  });

  test("executeSpells stops on failure by default", async () => {
    const ctx = createMockContext();
    const results = await Effect.runPromise(
      executeSpells(
        ["unknown_spell" as HealerSpellId, "mark_task_blocked_with_followup"],
        ctx
      )
    );

    // Should only have 1 result because it stopped after the first failure
    expect(results.length).toBe(1);
    expect(results[0].success).toBe(false);
  });
});

// ============================================================================
// Spell Allowlist Tests
// ============================================================================

describe("Spell Allowlist", () => {
  test("isSpellAllowed returns true by default", () => {
    const ctx = createMockContext();
    expect(isSpellAllowed("rewind_uncommitted_changes", ctx)).toBe(true);
  });

  test("isSpellAllowed respects forbidden list", () => {
    const ctx = createMockContext({
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
            onStuckSubtask: false,
          },
          spells: {
            allowed: [],
            forbidden: ["rewind_uncommitted_changes"],
          },
        },
      }),
    });

    expect(isSpellAllowed("rewind_uncommitted_changes", ctx)).toBe(false);
    expect(isSpellAllowed("mark_task_blocked_with_followup", ctx)).toBe(true);
  });

  test("isSpellAllowed respects allowed list", () => {
    const ctx = createMockContext({
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
            onStuckSubtask: false,
          },
          spells: {
            allowed: ["mark_task_blocked_with_followup"],
            forbidden: [],
          },
        },
      }),
    });

    expect(isSpellAllowed("rewind_uncommitted_changes", ctx)).toBe(false);
    expect(isSpellAllowed("mark_task_blocked_with_followup", ctx)).toBe(true);
  });

  test("filterAllowedSpells filters by config", () => {
    const ctx = createMockContext({
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
            onStuckSubtask: false,
          },
          spells: {
            allowed: [],
            forbidden: ["rewind_uncommitted_changes"],
          },
        },
      }),
    });

    const filtered = filterAllowedSpells(
      ["rewind_uncommitted_changes", "mark_task_blocked_with_followup"],
      ctx
    );

    expect(filtered).toEqual(["mark_task_blocked_with_followup"]);
  });
});

// ============================================================================
// Individual Spell Tests
// ============================================================================

describe("rewindUncommittedChanges spell", () => {
  test("spell has correct properties", () => {
    expect(rewindUncommittedChanges.id).toBe("rewind_uncommitted_changes");
    expect(rewindUncommittedChanges.requiresLLM).toBe(false);
  });

  test("returns success when repo is clean", async () => {
    const ctx = createMockContext({
      gitStatus: {
        isDirty: false,
        modifiedFiles: [],
        untrackedFiles: [],
        currentBranch: "main",
        lastCommitSha: "abc123",
        lastCommitMessage: "Initial commit",
      },
    });

    const result = await Effect.runPromise(rewindUncommittedChanges.apply(ctx));
    expect(result.success).toBe(true);
    expect(result.changesApplied).toBe(false);
    expect(result.summary).toContain("already clean");
  });
});

describe("markTaskBlockedWithFollowup spell", () => {
  test("spell has correct properties", () => {
    expect(markTaskBlockedWithFollowup.id).toBe("mark_task_blocked_with_followup");
    expect(markTaskBlockedWithFollowup.requiresLLM).toBe(false);
  });

  test("prepares block reason and followup", async () => {
    const ctx = createMockContext({
      task: {
        id: "oa-test123",
        title: "Test Task",
        description: "A test task",
        status: "in_progress",
        priority: 2,
        type: "task",
        labels: [],
        deps: [],
        commits: [],
        comments: [],
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        closedAt: null,
      },
      heuristics: {
        scenario: "SubtaskFailed",
        failureCount: 3,
        isFlaky: false,
        hasMissingImports: true,
        hasTypeErrors: true,
        hasTestAssertions: false,
        errorPatterns: ["Type assignment error", "Missing module or name"],
        previousAttempts: 2,
      },
    });

    const result = await Effect.runPromise(markTaskBlockedWithFollowup.apply(ctx));
    expect(result.success).toBe(true);
    expect(result.summary).toContain("oa-test123");
  });
});

describe("updateProgressWithGuidance spell", () => {
  test("spell has correct properties", () => {
    expect(updateProgressWithGuidance.id).toBe("update_progress_with_guidance");
    expect(updateProgressWithGuidance.requiresLLM).toBe(false);
  });
});

describe("fixTypecheckErrors spell", () => {
  const { fixTypecheckErrors } = require("../spells/typecheck.js");

  test("spell has correct properties", () => {
    expect(fixTypecheckErrors.id).toBe("fix_typecheck_errors");
    expect(fixTypecheckErrors.requiresLLM).toBe(true);
  });

  test("fails without error output", async () => {
    const ctx = createMockHealerContext("InitScriptTypecheckFailure", {
      heuristics: {
        scenario: "InitScriptTypecheckFailure",
        failureCount: 1,
        isFlaky: false,
        hasMissingImports: false,
        hasTypeErrors: true,
        hasTestAssertions: false,
        errorPatterns: ["TypeScript compilation error"],
        previousAttempts: 0,
      },
    });

    const result: HealerSpellResult = await Effect.runPromise(fixTypecheckErrors.apply(ctx));
    expect(result.success).toBe(false);
    expect(result.error).toContain("Missing error output");
  });

  test("fails for wrong scenario", async () => {
    const ctx = createMockContext({
      errorOutput: "error TS2345: Type mismatch",
      heuristics: {
        scenario: "SubtaskFailed",
        failureCount: 1,
        isFlaky: false,
        hasMissingImports: false,
        hasTypeErrors: true,
        hasTestAssertions: false,
        errorPatterns: ["TypeScript compilation error"],
        previousAttempts: 0,
      },
    });

    const result: HealerSpellResult = await Effect.runPromise(fixTypecheckErrors.apply(ctx));
    expect(result.success).toBe(false);
    expect(result.error).toContain("Wrong scenario");
  });

  test("prepares subtask for correct scenario", async () => {
    const ctx = createMockContext({
      errorOutput: "error TS2345: Type mismatch",
      heuristics: {
        scenario: "InitScriptTypecheckFailure",
        failureCount: 1,
        isFlaky: false,
        hasMissingImports: false,
        hasTypeErrors: true,
        hasTestAssertions: false,
        errorPatterns: ["TypeScript compilation error"],
        previousAttempts: 0,
      },
    });

    const result: HealerSpellResult = await Effect.runPromise(fixTypecheckErrors.apply(ctx));
    expect(result.success).toBe(true);
    expect(result.summary).toContain("Prepared emergency typecheck fix subtask");
  });
});

describe("fixTestErrors spell", () => {
  const { fixTestErrors } = require("../spells/typecheck.js");

  test("spell has correct properties", () => {
    expect(fixTestErrors.id).toBe("fix_test_errors");
    expect(fixTestErrors.requiresLLM).toBe(true);
  });

  test("fails without error output", async () => {
    const ctx = createMockHealerContext("InitScriptTestFailure", {
      heuristics: {
        scenario: "InitScriptTestFailure",
        failureCount: 1,
        isFlaky: false,
        hasMissingImports: false,
        hasTypeErrors: false,
        hasTestAssertions: true,
        errorPatterns: ["Test failures"],
        previousAttempts: 0,
      },
    });

    const result: HealerSpellResult = await Effect.runPromise(fixTestErrors.apply(ctx));
    expect(result.success).toBe(false);
    expect(result.error).toContain("Missing error output");
  });

  test("fails for wrong scenario", async () => {
    const ctx = createMockContext({
      errorOutput: "3 tests failed",
      heuristics: {
        scenario: "SubtaskFailed",
        failureCount: 1,
        isFlaky: false,
        hasMissingImports: false,
        hasTypeErrors: false,
        hasTestAssertions: true,
        errorPatterns: ["Test failures"],
        previousAttempts: 0,
      },
    });

    const result: HealerSpellResult = await Effect.runPromise(fixTestErrors.apply(ctx));
    expect(result.success).toBe(false);
    expect(result.error).toContain("Wrong scenario");
  });

  test("prepares subtask for correct scenario", async () => {
    const ctx = createMockContext({
      errorOutput: "3 tests failed",
      heuristics: {
        scenario: "InitScriptTestFailure",
        failureCount: 1,
        isFlaky: false,
        hasMissingImports: false,
        hasTypeErrors: false,
        hasTestAssertions: true,
        errorPatterns: ["Test failures"],
        previousAttempts: 0,
      },
    });

    const result: HealerSpellResult = await Effect.runPromise(fixTestErrors.apply(ctx));
    expect(result.success).toBe(true);
    expect(result.summary).toContain("Prepared emergency test fix subtask");
  });
});

describe("generateTypecheckFixDescription", () => {
  const { generateTypecheckFixDescription, generateTestFixDescription } = require("../spells/typecheck.js");

  test("generates typecheck fix description", () => {
    const description = generateTypecheckFixDescription("error TS2345: Type mismatch", 0);

    expect(description).toContain("## EMERGENCY: Fix All TypeScript Errors");
    expect(description).toContain("error TS2345: Type mismatch");
    expect(description).toContain("bun run typecheck");
  });

  test("includes retry note when failure count > 0", () => {
    const description = generateTypecheckFixDescription("error TS2345", 2);

    expect(description).toContain("retry #3");
    expect(description).toContain("Previous fix attempts failed");
  });

  test("generates test fix description", () => {
    const description = generateTestFixDescription("3 tests failed", 0);

    expect(description).toContain("## EMERGENCY: Fix Failing Tests");
    expect(description).toContain("3 tests failed");
    expect(description).toContain("bun test");
  });
});

describe("createEmergencySubtask", () => {
  const { createEmergencySubtask } = require("../spells/typecheck.js");

  test("creates typecheck emergency subtask", () => {
    const ctx = createMockContext({
      errorOutput: "error TS2345",
      heuristics: {
        scenario: "InitScriptTypecheckFailure",
        failureCount: 1,
        isFlaky: false,
        hasMissingImports: false,
        hasTypeErrors: true,
        hasTestAssertions: false,
        errorPatterns: [],
        previousAttempts: 0,
      },
    });

    const subtask = createEmergencySubtask(ctx, "typecheck");

    expect(subtask.id).toContain("emergency-typecheck-fix");
    expect(subtask.description).toContain("EMERGENCY: Fix All TypeScript Errors");
    expect(subtask.status).toBe("in_progress");
    expect(subtask.failureCount).toBe(0);
  });

  test("creates test emergency subtask", () => {
    const ctx = createMockContext({
      errorOutput: "3 tests failed",
      heuristics: {
        scenario: "InitScriptTestFailure",
        failureCount: 0,
        isFlaky: false,
        hasMissingImports: false,
        hasTypeErrors: false,
        hasTestAssertions: true,
        errorPatterns: [],
        previousAttempts: 0,
      },
    });

    const subtask = createEmergencySubtask(ctx, "test");

    expect(subtask.id).toContain("emergency-test-fix");
    expect(subtask.description).toContain("EMERGENCY: Fix Failing Tests");
    expect(subtask.status).toBe("in_progress");
  });
});

// ============================================================================
// generateHealerSummary Tests
// ============================================================================

describe("generateHealerSummary", () => {
  test("generates summary with scenario and timestamp", () => {
    const ctx = createMockContext();
    const summary = generateHealerSummary(ctx, [], []);

    expect(summary).toContain("## Healer Summary");
    expect(summary).toContain("**Invoked at:**");
    expect(summary).toContain("**Scenario:** SubtaskFailed");
  });

  test("includes subtask details when available", () => {
    const ctx = createMockContext({
      subtask: {
        id: "sub-1",
        description: "Test subtask for summary",
        status: "in_progress",
        startedAt: new Date().toISOString(),
      },
    });
    const summary = generateHealerSummary(ctx, [], []);

    expect(summary).toContain("**Subtask:** sub-1");
  });

  test("includes spells tried with success/failure icons", () => {
    const ctx = createMockContext();
    const summary = generateHealerSummary(
      ctx,
      ["rewind_uncommitted_changes", "mark_task_blocked_with_followup"],
      ["mark_task_blocked_with_followup"]
    );

    expect(summary).toContain("✗ `rewind_uncommitted_changes`");
    expect(summary).toContain("✓ `mark_task_blocked_with_followup`");
  });

  test("includes error output when available", () => {
    const ctx = createMockContext({ errorOutput: "Error: Something went wrong" });
    const summary = generateHealerSummary(ctx, [], []);

    expect(summary).toContain("**Error excerpt:**");
    expect(summary).toContain("Error: Something went wrong");
  });

  test("includes git status when dirty", () => {
    const ctx = createMockContext({
      gitStatus: {
        isDirty: true,
        modifiedFiles: ["file1.ts", "file2.ts"],
        untrackedFiles: ["new.ts"],
        currentBranch: "feature",
        lastCommitSha: "abc123def",
        lastCommitMessage: "WIP",
      },
    });
    const summary = generateHealerSummary(ctx, [], []);

    expect(summary).toContain("### Git Status");
    expect(summary).toContain("**Modified files:** 2");
    expect(summary).toContain("**Untracked files:** 1");
  });

  test("includes recommended next steps", () => {
    const ctx = createMockContext({
      heuristics: {
        scenario: "InitScriptTypecheckFailure",
        failureCount: 1,
        isFlaky: false,
        hasMissingImports: true,
        hasTypeErrors: false,
        hasTestAssertions: false,
        errorPatterns: [],
        previousAttempts: 0,
      },
    });
    const summary = generateHealerSummary(ctx, [], []);

    expect(summary).toContain("### Recommended Next Steps");
    expect(summary).toContain("bun tsc --noEmit");
  });
});
