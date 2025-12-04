/**
 * Tests for Healer ATIF Integration
 */
import { describe, test, expect } from "bun:test";
import {
  createHealerAgent,
  healerEventToStep,
  healerEventsToSteps,
  createHealerTrajectory,
  createMinimalHealerTrajectory,
  createHealerTrajectoryRef,
  createHealerObservation,
  createHealerInvocationStep,
  HealerEventCollector,
} from "../atif.js";
import type { HealerEvent } from "../service.js";
import type { HealerOutcome, HealerContext } from "../types.js";
import { createHealerCounters } from "../types.js";
import { createMockProjectConfig } from "./test-helpers.js";

// ============================================================================
// Test Helpers
// ============================================================================

const createMockContext = (): HealerContext => ({
  projectRoot: "/tmp/test",
  projectConfig: createMockProjectConfig({
    defaultBranch: "main",
    testCommands: ["bun test"],
  }),
  sessionId: "session-123",
  relatedTrajectories: [],
  progressMd: null,
  gitStatus: {
    isDirty: false,
    modifiedFiles: [],
    untrackedFiles: [],
    currentBranch: "main",
    lastCommitSha: "abc123",
    lastCommitMessage: "Initial",
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
  triggerEvent: { type: "subtask_failed" } as any,
  orchestratorState: {} as any,
  errorOutput: "Error",
  counters: createHealerCounters(),
});

const createMockOutcome = (status: "resolved" | "contained" | "unresolved"): HealerOutcome => ({
  scenario: "SubtaskFailed",
  status,
  spellsTried: ["rewind_uncommitted_changes"],
  spellsSucceeded: status !== "unresolved" ? ["rewind_uncommitted_changes"] : [],
  summary: "Test outcome",
});

// ============================================================================
// createHealerAgent Tests
// ============================================================================

describe("createHealerAgent", () => {
  test("creates agent with default values", () => {
    const agent = createHealerAgent();

    expect(agent.name).toBe("healer");
    expect(agent.model_name).toBe("healer-v1");
    expect(agent.extra?.type).toBe("subagent");
    expect(agent.extra?.purpose).toBe("self-healing");
  });

  test("creates agent with custom values", () => {
    const agent = createHealerAgent("healer-v2", "2.0.0");

    expect(agent.model_name).toBe("healer-v2");
    expect(agent.version).toBe("2.0.0");
  });
});

// ============================================================================
// healerEventToStep Tests
// ============================================================================

describe("healerEventToStep", () => {
  test("converts healer_start event", () => {
    const event: HealerEvent = {
      type: "healer_start",
      scenario: "SubtaskFailed",
      spells: ["rewind_uncommitted_changes", "update_progress_with_guidance"],
    };

    const step = healerEventToStep(event, 1);

    expect(step.step_id).toBe(1);
    expect(step.source).toBe("system");
    expect(step.message).toContain("SubtaskFailed");
    expect(step.extra?.planned_spells).toEqual([
      "rewind_uncommitted_changes",
      "update_progress_with_guidance",
    ]);
  });

  test("converts healer_spell_start event", () => {
    const event: HealerEvent = {
      type: "healer_spell_start",
      spellId: "rewind_uncommitted_changes",
    };

    const step = healerEventToStep(event, 2);

    expect(step.step_id).toBe(2);
    expect(step.source).toBe("agent");
    expect(step.tool_calls).toHaveLength(1);
    expect(step.tool_calls?.[0].function_name).toBe("rewind_uncommitted_changes");
  });

  test("converts healer_spell_complete event (success)", () => {
    const event: HealerEvent = {
      type: "healer_spell_complete",
      spellId: "rewind_uncommitted_changes",
      result: {
        success: true,
        changesApplied: true,
        summary: "Reverted 3 files",
        filesModified: ["a.ts", "b.ts", "c.ts"],
      },
    };

    const step = healerEventToStep(event, 3);

    expect(step.step_id).toBe(3);
    expect(step.source).toBe("agent");
    expect(step.message).toContain("succeeded");
    expect(step.observation?.results).toHaveLength(1);
    expect((step.observation?.results[0].content as any).success).toBe(true);
  });

  test("converts healer_spell_complete event (failure)", () => {
    const event: HealerEvent = {
      type: "healer_spell_complete",
      spellId: "fix_typecheck_errors",
      result: {
        success: false,
        changesApplied: false,
        summary: "LLM not available",
        error: "No invoker",
      },
    };

    const step = healerEventToStep(event, 4);

    expect(step.message).toContain("failed");
    expect((step.observation?.results[0].content as any).error).toBe("No invoker");
  });

  test("converts healer_complete event", () => {
    const event: HealerEvent = {
      type: "healer_complete",
      outcome: createMockOutcome("resolved"),
    };

    const step = healerEventToStep(event, 5);

    expect(step.source).toBe("system");
    expect(step.message).toContain("resolved");
    expect(step.extra?.status).toBe("resolved");
  });
});

// ============================================================================
// healerEventsToSteps Tests
// ============================================================================

describe("healerEventsToSteps", () => {
  test("converts multiple events to steps", () => {
    const events: HealerEvent[] = [
      { type: "healer_start", scenario: "SubtaskFailed", spells: ["rewind_uncommitted_changes"] },
      { type: "healer_spell_start", spellId: "rewind_uncommitted_changes" },
      {
        type: "healer_spell_complete",
        spellId: "rewind_uncommitted_changes",
        result: { success: true, changesApplied: true, summary: "Done" },
      },
      { type: "healer_complete", outcome: createMockOutcome("resolved") },
    ];

    const steps = healerEventsToSteps(events);

    expect(steps).toHaveLength(4);
    expect(steps[0].step_id).toBe(1);
    expect(steps[3].step_id).toBe(4);
  });
});

// ============================================================================
// createHealerTrajectory Tests
// ============================================================================

describe("createHealerTrajectory", () => {
  test("creates trajectory from outcome and events", () => {
    const events: HealerEvent[] = [
      { type: "healer_start", scenario: "SubtaskFailed", spells: ["rewind_uncommitted_changes"] },
      { type: "healer_complete", outcome: createMockOutcome("resolved") },
    ];
    const outcome = createMockOutcome("resolved");

    const trajectory = createHealerTrajectory(outcome, events);

    expect(trajectory.schema_version).toBe("ATIF-v1.4");
    expect(trajectory.session_id).toContain("session-");
    expect(trajectory.agent.name).toBe("healer");
    expect(trajectory.steps).toHaveLength(2);
    expect(trajectory.extra?.status).toBe("resolved");
  });

  test("includes parent session ID when provided", () => {
    const events: HealerEvent[] = [];
    const outcome = createMockOutcome("resolved");

    const trajectory = createHealerTrajectory(outcome, events, "parent-session-123");

    expect(trajectory.extra?.parent_session_id).toBe("parent-session-123");
  });
});

// ============================================================================
// createMinimalHealerTrajectory Tests
// ============================================================================

describe("createMinimalHealerTrajectory", () => {
  test("creates trajectory from outcome only", () => {
    const outcome = createMockOutcome("contained");

    const trajectory = createMinimalHealerTrajectory(outcome);

    expect(trajectory.agent.name).toBe("healer");
    // Start step + spell step + complete step
    expect(trajectory.steps.length).toBeGreaterThanOrEqual(3);
    expect(trajectory.extra?.status).toBe("contained");
  });
});

// ============================================================================
// createHealerTrajectoryRef Tests
// ============================================================================

describe("createHealerTrajectoryRef", () => {
  test("creates reference from trajectory", () => {
    const trajectory = createMinimalHealerTrajectory(createMockOutcome("resolved"));

    const ref = createHealerTrajectoryRef(trajectory, "/trajectories/healer-123.json");

    expect(ref.session_id).toBe(trajectory.session_id);
    expect(ref.trajectory_path).toBe("/trajectories/healer-123.json");
    expect(ref.extra?.agent_name).toBe("healer");
  });
});

// ============================================================================
// createHealerObservation Tests
// ============================================================================

describe("createHealerObservation", () => {
  test("creates observation from outcome", () => {
    const outcome = createMockOutcome("resolved");

    const observation = createHealerObservation(outcome);

    expect(observation.source_call_id).toContain("healer");
    expect((observation.content as any).status).toBe("resolved");
    expect((observation.content as any).spells_executed).toContain("rewind_uncommitted_changes");
  });

  test("includes trajectory ref when provided", () => {
    const outcome = createMockOutcome("resolved");
    const trajectory = createMinimalHealerTrajectory(outcome);
    const ref = createHealerTrajectoryRef(trajectory);

    const observation = createHealerObservation(outcome, ref);

    expect(observation.subagent_trajectory_ref).toHaveLength(1);
    expect(observation.subagent_trajectory_ref?.[0].session_id).toBe(trajectory.session_id);
  });
});

// ============================================================================
// createHealerInvocationStep Tests
// ============================================================================

describe("createHealerInvocationStep", () => {
  test("creates step for orchestrator trajectory", () => {
    const outcome = createMockOutcome("resolved");

    const step = createHealerInvocationStep(outcome, 10);

    expect(step.step_id).toBe(10);
    expect(step.source).toBe("agent");
    expect(step.message).toContain("Healer invoked");
    expect(step.observation?.results).toHaveLength(1);
    expect(step.extra?.event_type).toBe("healer_invocation");
  });
});

// ============================================================================
// HealerEventCollector Tests
// ============================================================================

describe("HealerEventCollector", () => {
  test("collects events via callback", () => {
    const collector = new HealerEventCollector("parent-123");

    collector.onEvent({ type: "healer_start", scenario: "SubtaskFailed", spells: [] });
    collector.onEvent({ type: "healer_complete", outcome: createMockOutcome("resolved") });

    const events = collector.getEvents();
    expect(events).toHaveLength(2);
  });

  test("creates trajectory from collected events", () => {
    const collector = new HealerEventCollector("parent-123");
    const outcome = createMockOutcome("resolved");

    collector.onEvent({ type: "healer_start", scenario: "SubtaskFailed", spells: [] });
    collector.onEvent({ type: "healer_complete", outcome });

    const trajectory = collector.createTrajectory(outcome);

    expect(trajectory.steps).toHaveLength(2);
    expect(trajectory.extra?.parent_session_id).toBe("parent-123");
  });

  test("reset clears collected events", () => {
    const collector = new HealerEventCollector();

    collector.onEvent({ type: "healer_start", scenario: "SubtaskFailed", spells: [] });
    collector.reset();

    expect(collector.getEvents()).toHaveLength(0);
  });
});
