/**
 * Shared test helpers for Healer tests
 */
import type { ProjectConfig } from "../../tasks/schema.js";
import type { OrchestratorState, Subtask } from "../../agent/orchestrator/types.js";
import type { HealerContext, HealerScenario } from "../types.js";
import { createHealerCounters } from "../types.js";

/**
 * Create a minimal mock ProjectConfig for tests
 */
export const createMockProjectConfig = (
  overrides: Partial<ProjectConfig> = {}
): ProjectConfig => ({
  version: 1,
  projectId: "test-project",
  defaultBranch: "main",
  defaultModel: "test-model",
  rootDir: "/test/root",
  typecheckCommands: ["bun run typecheck"],
  testCommands: ["bun test"],
  e2eCommands: [],
  allowPush: false,
  allowForcePush: false,
  maxTasksPerRun: 10,
  maxRuntimeMinutes: 120,
  idPrefix: "test",
  ...overrides,
} as ProjectConfig);

/**
 * Create a minimal mock OrchestratorState for tests
 */
export const createMockOrchestratorState = (
  overrides: Partial<OrchestratorState> = {}
): OrchestratorState => ({
  sessionId: "session-123",
  phase: "executing_subtask",
  task: null,
  subtasks: null,
  progress: null,
  ...overrides,
});

/**
 * Create a minimal mock HealerContext for tests
 */
export const createMockHealerContext = (
  scenario: HealerScenario = "SubtaskFailed",
  overrides: Partial<HealerContext> = {}
): HealerContext => ({
  projectRoot: "/test/root",
  projectConfig: createMockProjectConfig(),
  sessionId: "session-123",
  relatedTrajectories: [],
  progressMd: null,
  gitStatus: {
    isDirty: false,
    modifiedFiles: [],
    untrackedFiles: [],
    currentBranch: "main",
    lastCommitSha: "abc123",
    lastCommitMessage: "test commit",
  },
  heuristics: {
    scenario,
    failureCount: 0,
    isFlaky: false,
    hasMissingImports: false,
    hasTypeErrors: false,
    hasTestAssertions: false,
    errorPatterns: [],
    previousAttempts: 0,
  },
  triggerEvent: { type: "subtask_failed", subtask: {} as Subtask, error: "test error" },
  orchestratorState: createMockOrchestratorState(),
  counters: createHealerCounters(),
  ...overrides,
});

/**
 * Create a minimal mock Subtask for tests
 */
export const createMockSubtask = (
  overrides: Partial<Subtask> = {}
): Subtask => ({
  id: `subtask-${Date.now()}`,
  description: "Test subtask",
  status: "in_progress",
  startedAt: new Date().toISOString(),
  ...overrides,
});
