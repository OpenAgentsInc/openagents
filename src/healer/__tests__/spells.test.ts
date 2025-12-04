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
import type { HealerContext, HealerSpellId } from "../types.js";
import { createHealerCounters } from "../types.js";

// ============================================================================
// Test Helpers
// ============================================================================

const createMockContext = (overrides: Partial<HealerContext> = {}): HealerContext => ({
  projectRoot: "/tmp/test-project",
  projectConfig: {
    defaultBranch: "main",
    testCommands: ["bun test"],
    healer: {
      mode: "auto",
      scenarios: {},
      spells: {},
      maxPerSession: 5,
      maxPerSubtask: 3,
    },
  },
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
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  subtask: {
    id: "sub-1",
    description: "Test subtask",
    status: "failed",
    retryable: true,
    failureCount: 1,
  },
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
    scenario: "SubtaskFailed",
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
// Spell Registry Tests
// ============================================================================

describe("Spell Registry", () => {
  test("spellRegistry contains all core spells", () => {
    expect(spellRegistry.size).toBe(3);
    expect(spellRegistry.has("rewind_uncommitted_changes")).toBe(true);
    expect(spellRegistry.has("mark_task_blocked_with_followup")).toBe(true);
    expect(spellRegistry.has("update_progress_with_guidance")).toBe(true);
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
    expect(ids.length).toBe(3);
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
    const ctx = createMockContext();
    const results = await Effect.runPromise(
      executeSpells(
        ["mark_task_blocked_with_followup", "update_progress_with_guidance"],
        ctx,
        { continueOnFailure: true }
      )
    );

    expect(results.length).toBe(2);
    expect(results[0].success).toBe(true);
    expect(results[1].success).toBe(true);
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
      projectConfig: {
        defaultBranch: "main",
        testCommands: ["bun test"],
        healer: {
          mode: "auto",
          spells: {
            forbidden: ["rewind_uncommitted_changes"],
          },
        },
      },
    });

    expect(isSpellAllowed("rewind_uncommitted_changes", ctx)).toBe(false);
    expect(isSpellAllowed("mark_task_blocked_with_followup", ctx)).toBe(true);
  });

  test("isSpellAllowed respects allowed list", () => {
    const ctx = createMockContext({
      projectConfig: {
        defaultBranch: "main",
        testCommands: ["bun test"],
        healer: {
          mode: "auto",
          spells: {
            allowed: ["mark_task_blocked_with_followup"],
          },
        },
      },
    });

    expect(isSpellAllowed("rewind_uncommitted_changes", ctx)).toBe(false);
    expect(isSpellAllowed("mark_task_blocked_with_followup", ctx)).toBe(true);
  });

  test("filterAllowedSpells filters by config", () => {
    const ctx = createMockContext({
      projectConfig: {
        defaultBranch: "main",
        testCommands: ["bun test"],
        healer: {
          mode: "auto",
          spells: {
            forbidden: ["rewind_uncommitted_changes"],
          },
        },
      },
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
    const ctx = createMockContext();
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
