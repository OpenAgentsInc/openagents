/**
 * HUD Message Factory Functions for E2E Tests
 *
 * Creates properly typed HUD messages for test scenarios.
 */

import type {
  HudMessage,
  HudTaskInfo,
  HudSubtaskInfo,
  HudSubagentResult,
  SessionStartMessage,
  SessionCompleteMessage,
  TaskSelectedMessage,
  TaskDecomposedMessage,
  SubtaskStartMessage,
  SubtaskCompleteMessage,
  SubtaskFailedMessage,
  VerificationStartMessage,
  VerificationCompleteMessage,
  CommitCreatedMessage,
  PushCompleteMessage,
  APMUpdateMessage,
  APMSnapshotMessage,
  ErrorMessage,
} from "../../src/hud/protocol.js";

// ============================================================================
// Factory Helpers
// ============================================================================

let counter = 0;
const generateId = (prefix: string) => `${prefix}-${++counter}-${Date.now().toString(36)}`;

// ============================================================================
// Session Message Factories
// ============================================================================

export const createSessionStart = (
  sessionId: string = generateId("session")
): SessionStartMessage => ({
  type: "session_start",
  sessionId,
  timestamp: new Date().toISOString(),
});

export const createSessionComplete = (
  success: boolean,
  summary: string
): SessionCompleteMessage => ({
  type: "session_complete",
  success,
  summary,
});

// ============================================================================
// Task Message Factories
// ============================================================================

export const createTaskInfo = (
  overrides: Partial<HudTaskInfo> = {}
): HudTaskInfo => ({
  id: generateId("oa"),
  title: "Test Task",
  status: "in_progress",
  priority: 1,
  ...overrides,
});

export const createTaskSelected = (
  task: Partial<HudTaskInfo> = {}
): TaskSelectedMessage => ({
  type: "task_selected",
  task: createTaskInfo(task),
});

export const createSubtaskInfo = (
  overrides: Partial<HudSubtaskInfo> = {}
): HudSubtaskInfo => ({
  id: generateId("sub"),
  description: "Implement feature",
  status: "pending",
  ...overrides,
});

export const createTaskDecomposed = (
  subtasks: HudSubtaskInfo[]
): TaskDecomposedMessage => ({
  type: "task_decomposed",
  subtasks,
});

// ============================================================================
// Subtask Message Factories
// ============================================================================

export const createSubtaskStart = (
  subtask: Partial<HudSubtaskInfo> = {}
): SubtaskStartMessage => ({
  type: "subtask_start",
  subtask: createSubtaskInfo({ ...subtask, status: "in_progress" }),
});

export const createSubagentResult = (
  overrides: Partial<HudSubagentResult> = {}
): HudSubagentResult => ({
  success: true,
  filesModified: [],
  turns: 1,
  ...overrides,
});

export const createSubtaskComplete = (
  subtask: Partial<HudSubtaskInfo> = {},
  result: Partial<HudSubagentResult> = {}
): SubtaskCompleteMessage => ({
  type: "subtask_complete",
  subtask: createSubtaskInfo({ ...subtask, status: "done" }),
  result: createSubagentResult(result),
});

export const createSubtaskFailed = (
  subtask: Partial<HudSubtaskInfo> = {},
  error: string = "Test error"
): SubtaskFailedMessage => ({
  type: "subtask_failed",
  subtask: createSubtaskInfo({ ...subtask, status: "failed" }),
  error,
});

// ============================================================================
// Verification Message Factories
// ============================================================================

export const createVerificationStart = (
  command: string = "bun test"
): VerificationStartMessage => ({
  type: "verification_start",
  command,
});

export const createVerificationComplete = (
  command: string = "bun test",
  passed: boolean = true,
  output?: string
): VerificationCompleteMessage => ({
  type: "verification_complete",
  command,
  passed,
  output,
});

// ============================================================================
// Git Message Factories
// ============================================================================

export const createCommitCreated = (
  sha: string = "abc123def456",
  message: string = "feat: add new feature"
): CommitCreatedMessage => ({
  type: "commit_created",
  sha,
  message,
});

export const createPushComplete = (
  branch: string = "main"
): PushCompleteMessage => ({
  type: "push_complete",
  branch,
});

// ============================================================================
// APM Message Factories
// ============================================================================

export const createAPMUpdate = (
  overrides: Partial<APMUpdateMessage> = {}
): APMUpdateMessage => ({
  type: "apm_update",
  sessionId: "test-session",
  sessionAPM: 15.5,
  recentAPM: 18.2,
  totalActions: 42,
  durationMinutes: 3,
  ...overrides,
});

export const createAPMSnapshot = (
  overrides: Partial<APMSnapshotMessage> = {}
): APMSnapshotMessage => ({
  type: "apm_snapshot",
  combined: {
    apm1h: 12.0,
    apm6h: 10.5,
    apm1d: 9.8,
    apm1w: 8.2,
    apm1m: 7.5,
    apmLifetime: 6.8,
    totalSessions: 100,
    totalActions: 5000,
    ...overrides.combined,
  },
  comparison: {
    claudeCodeAPM: 5.0,
    mechaCoderAPM: 15.0,
    efficiencyRatio: 3.0,
    ...overrides.comparison,
  },
});

// ============================================================================
// Error Message Factory
// ============================================================================

export const createError = (
  error: string = "Test error",
  phase: ErrorMessage["phase"] = "implementing"
): ErrorMessage => ({
  type: "error",
  phase,
  error,
});

// ============================================================================
// Sequence Builders
// ============================================================================

/**
 * Creates a complete Golden Loop message sequence for testing
 */
export const createGoldenLoopSequence = (taskId?: string): HudMessage[] => {
  const task = createTaskInfo({
    id: taskId ?? generateId("oa"),
    title: `Golden Loop Task ${taskId ?? "Test"}`,
  });
  const subtask1 = createSubtaskInfo({
    id: `${task.id}-sub-001`,
    description: "Implement feature",
  });
  const subtask2 = createSubtaskInfo({
    id: `${task.id}-sub-002`,
    description: "Add tests",
  });

  return [
    createSessionStart(),
    { type: "task_selected", task } as TaskSelectedMessage,
    {
      type: "task_decomposed",
      subtasks: [subtask1, subtask2],
    } as TaskDecomposedMessage,
    {
      type: "subtask_start",
      subtask: { ...subtask1, status: "in_progress" },
    } as SubtaskStartMessage,
    {
      type: "subtask_complete",
      subtask: { ...subtask1, status: "done" },
      result: createSubagentResult({
        success: true,
        filesModified: ["src/feature.ts"],
        turns: 3,
      }),
    } as SubtaskCompleteMessage,
    {
      type: "subtask_start",
      subtask: { ...subtask2, status: "in_progress" },
    } as SubtaskStartMessage,
    {
      type: "subtask_complete",
      subtask: { ...subtask2, status: "done" },
      result: createSubagentResult({
        success: true,
        filesModified: ["src/feature.test.ts"],
        turns: 2,
      }),
    } as SubtaskCompleteMessage,
    createVerificationStart("bun test"),
    createVerificationComplete("bun test", true, "42 tests passed"),
    createCommitCreated("abc123", `${task.id}: Implement feature`),
    createPushComplete("main"),
    createSessionComplete(true, "Task completed successfully"),
  ];
};

/**
 * Creates APM update sequence showing progress
 */
export const createAPMProgressSequence = (): APMUpdateMessage[] => [
  createAPMUpdate({ sessionAPM: 5.0, recentAPM: 5.0, totalActions: 5, durationMinutes: 1 }),
  createAPMUpdate({ sessionAPM: 10.0, recentAPM: 12.0, totalActions: 15, durationMinutes: 1.5 }),
  createAPMUpdate({ sessionAPM: 15.0, recentAPM: 18.0, totalActions: 30, durationMinutes: 2 }),
  createAPMUpdate({ sessionAPM: 20.0, recentAPM: 25.0, totalActions: 50, durationMinutes: 2.5 }),
  createAPMUpdate({ sessionAPM: 30.0, recentAPM: 35.0, totalActions: 100, durationMinutes: 3.3 }),
];
