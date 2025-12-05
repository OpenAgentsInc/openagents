/**
 * HUD WebSocket Protocol Types
 *
 * Defines the message types exchanged between the agent process
 * and the Electrobun HUD UI over WebSocket.
 *
 * Architecture:
 * - Agent process (overnight.ts) -> WebSocket CLIENT (HudClient)
 * - Electrobun mainview -> WebSocket SERVER (port 4242)
 *
 * The agent pushes events; the HUD receives and renders them.
 */

// ============================================================================
// Orchestrator Events (mirroring types from orchestrator/types.ts)
// ============================================================================

import type { OrchestratorPhase, SubtaskStatus } from "../agent/orchestrator/types.js";
import type { HealerScenario, HealerSpellId, HealerOutcomeStatus } from "../healer/types.js";

export interface HudTaskInfo {
  id: string;
  title: string;
  status: string;
  priority: number;
}

export interface HudSubtaskInfo {
  id: string;
  description: string;
  status: SubtaskStatus;
}

export interface HudSubagentResult {
  success: boolean;
  agent?: "claude-code" | "minimal";
  filesModified: string[];
  turns: number;
  error?: string;
}

// ============================================================================
// HUD Message Types
// ============================================================================

/**
 * Session lifecycle events
 */
export interface SessionStartMessage {
  type: "session_start";
  sessionId: string;
  timestamp: string;
}

export interface SessionCompleteMessage {
  type: "session_complete";
  success: boolean;
  summary: string;
}

/**
 * Task flow events
 */
export interface TaskSelectedMessage {
  type: "task_selected";
  task: HudTaskInfo;
}

export interface TaskDecomposedMessage {
  type: "task_decomposed";
  subtasks: HudSubtaskInfo[];
}

export interface SubtaskStartMessage {
  type: "subtask_start";
  subtask: HudSubtaskInfo;
}

export interface SubtaskCompleteMessage {
  type: "subtask_complete";
  subtask: HudSubtaskInfo;
  result: HudSubagentResult;
}

export interface SubtaskFailedMessage {
  type: "subtask_failed";
  subtask: HudSubtaskInfo;
  error: string;
}

/**
 * Verification events
 */
export interface VerificationStartMessage {
  type: "verification_start";
  command: string;
}

export interface VerificationCompleteMessage {
  type: "verification_complete";
  command: string;
  passed: boolean;
  output?: string;
}

/**
 * Git events
 */
export interface CommitCreatedMessage {
  type: "commit_created";
  sha: string;
  message: string;
}

export interface PushCompleteMessage {
  type: "push_complete";
  branch: string;
}

/**
 * Phase change
 */
export interface PhaseChangeMessage {
  type: "phase_change";
  phase: OrchestratorPhase;
}

/**
 * Error events
 */
export interface ErrorMessage {
  type: "error";
  phase: OrchestratorPhase;
  error: string;
}

// ============================================================================
// Streaming Output Events
// ============================================================================

/**
 * Text output from Claude Code (streaming)
 */
export interface TextOutputMessage {
  type: "text_output";
  text: string;
  /** Optional source identifier */
  source?: "claude-code" | "minimal" | "orchestrator";
}

/**
 * Tool call JSON from Claude Code
 */
export interface ToolCallMessage {
  type: "tool_call";
  toolName: string;
  /** Stringified JSON arguments */
  arguments: string;
  /** Tool call ID for correlation */
  callId?: string;
}

/**
 * Tool result from Claude Code
 */
export interface ToolResultMessage {
  type: "tool_result";
  toolName: string;
  /** Stringified JSON result */
  result: string;
  /** Whether the tool call errored */
  isError: boolean;
  /** Tool call ID for correlation */
  callId?: string;
}

// ============================================================================
// ATIF (Agent Trajectory) Events
// ============================================================================

/**
 * ATIF trajectory started
 */
export interface ATIFTrajectoryStartMessage {
  type: "atif_trajectory_start";
  sessionId: string;
  agentName: string;
  agentType: "orchestrator" | "claude-code" | "minimal";
  parentSessionId?: string;
}

/**
 * ATIF step recorded
 */
export interface ATIFStepRecordedMessage {
  type: "atif_step_recorded";
  sessionId: string;
  stepId: number;
  source: "user" | "agent" | "system";
  hasToolCalls: boolean;
  hasObservation: boolean;
}

/**
 * Subagent spawned (for hierarchy tracking)
 */
export interface ATIFSubagentSpawnedMessage {
  type: "atif_subagent_spawned";
  parentSessionId: string;
  childSessionId: string;
  subtaskId: string;
  agentType: "claude-code" | "minimal";
}

/**
 * ATIF trajectory complete
 */
export interface ATIFTrajectoryCompleteMessage {
  type: "atif_trajectory_complete";
  sessionId: string;
  totalSteps: number;
  totalTokens?: {
    prompt: number;
    completion: number;
    cached?: number;
  };
  totalCostUsd?: number;
  trajectoryPath: string;
}

// ============================================================================
// APM (Actions Per Minute) Events
// ============================================================================

/**
 * Real-time APM update during session
 */
export interface APMUpdateMessage {
  type: "apm_update";
  sessionId: string;
  /** Current session APM */
  sessionAPM: number;
  /** APM over last 5 minutes */
  recentAPM: number;
  /** Total actions (messages + tool calls) this session */
  totalActions: number;
  /** Session duration in minutes */
  durationMinutes: number;
}

/**
 * APM snapshot with historical context (sent at session boundaries)
 */
export interface APMSnapshotMessage {
  type: "apm_snapshot";
  /** Combined APM across all sources */
  combined: {
    apm1h: number;
    apm6h: number;
    apm1d: number;
    apm1w: number;
    apm1m: number;
    apmLifetime: number;
    totalSessions: number;
    totalActions: number;
  };
  /** MechaCoder vs Claude Code comparison */
  comparison: {
    claudeCodeAPM: number;
    mechaCoderAPM: number;
    /** mechaCoder / claudeCode */
    efficiencyRatio: number;
  };
}

/**
 * Tool usage breakdown
 */
export interface APMToolUsageMessage {
  type: "apm_tool_usage";
  tools: Array<{
    name: string;
    count: number;
    percentage: number;
    category: string;
  }>;
}

// Usage metrics
export interface UsageUpdateMessage {
  type: "usage_update";
  usage: {
    sessionId: string;
    projectId: string;
    timestamp: string;
    inputTokens: number;
    outputTokens: number;
    cacheReadTokens: number;
    cacheCreationTokens: number;
    totalCostUsd: number;
    subtasks: number;
    durationMs: number;
    agent: string;
  };
}

// ============================================================================
// Healer (Self-Healing Subagent) Events
// ============================================================================

/**
 * Healer invocation started
 */
export interface HealerInvocationStartMessage {
  type: "healer_invocation_start";
  sessionId: string;
  scenario: HealerScenario;
  plannedSpells: HealerSpellId[];
  /** Parent orchestrator session for linking */
  parentSessionId?: string;
}

/**
 * Healer spell applied (one per spell execution)
 */
export interface HealerSpellAppliedMessage {
  type: "healer_spell_applied";
  sessionId: string;
  spellId: HealerSpellId;
  success: boolean;
  changesApplied: boolean;
  summary: string;
  filesModified?: string[];
  error?: string;
}

/**
 * Healer invocation complete
 */
export interface HealerInvocationCompleteMessage {
  type: "healer_invocation_complete";
  sessionId: string;
  status: HealerOutcomeStatus;
  reason: string;
  spellsExecuted: number;
  successfulSpells: number;
  failedSpells: number;
}

// ============================================================================
// Terminal-Bench Events
// ============================================================================

/** TB task difficulty levels */
export type TBDifficulty = "easy" | "medium" | "hard" | "expert";

/** TB task outcome */
export type TBTaskOutcome = "success" | "failure" | "timeout" | "error";

/** TB task phase during execution */
export type TBTaskPhase = "setup" | "agent" | "verification";

/** TB output source */
export type TBOutputSource = "agent" | "verification" | "system";

/**
 * TB run started
 */
export interface TBRunStartMessage {
  type: "tb_run_start";
  runId: string;
  suiteName: string;
  suiteVersion: string;
  totalTasks: number;
  taskIds: string[];
  timestamp: string;
}

/**
 * TB run completed
 */
export interface TBRunCompleteMessage {
  type: "tb_run_complete";
  runId: string;
  passRate: number;
  passed: number;
  failed: number;
  timeout: number;
  error: number;
  totalDurationMs: number;
}

/**
 * TB task started
 */
export interface TBTaskStartMessage {
  type: "tb_task_start";
  runId: string;
  taskId: string;
  taskName: string;
  category: string;
  difficulty: TBDifficulty;
  taskIndex: number;
  totalTasks: number;
}

/**
 * TB task progress update
 */
export interface TBTaskProgressMessage {
  type: "tb_task_progress";
  runId: string;
  taskId: string;
  phase: TBTaskPhase;
  currentTurn?: number;
  elapsedMs: number;
}

/**
 * TB task output (streaming)
 */
export interface TBTaskOutputMessage {
  type: "tb_task_output";
  runId: string;
  taskId: string;
  text: string;
  source: TBOutputSource;
}

/**
 * TB task completed
 */
export interface TBTaskCompleteMessage {
  type: "tb_task_complete";
  runId: string;
  taskId: string;
  outcome: TBTaskOutcome;
  durationMs: number;
  turns: number;
  tokens: number;
  verificationOutput?: string;
}

/**
 * TB suite info (for UI)
 */
export interface TBSuiteInfoMessage {
  type: "tb_suite_info";
  name: string;
  version: string;
  tasks: Array<{
    id: string;
    name: string;
    category: string;
    difficulty: TBDifficulty;
  }>;
}

/**
 * TB run request (from UI to trigger run)
 */
export interface TBRunRequestMessage {
  type: "tb_run_request";
  suitePath: string;
  taskIds?: string[];
  timeout?: number;
  maxTurns?: number;
}

export interface TBRunHistoryMessage {
  type: "tb_run_history";
  runs: Array<{
    runId: string;
    suiteName: string;
    suiteVersion: string;
    timestamp: string;
    passRate: number;
    passed: number;
    failed: number;
    timeout: number;
    error: number;
    totalDurationMs: number;
    totalTokens: number;
    taskCount: number;
    filepath: string;
  }>;
}

// ============================================================================
// Container Execution Events
// ============================================================================

/** Output stream type from container execution */
export type ContainerStreamType = "stdout" | "stderr";

/** Context in which container execution is happening */
export type ExecutionContext = "verification" | "init" | "subagent" | "custom";

/**
 * Container execution started
 */
export interface ContainerStartMessage {
  type: "container_start";
  /** Unique execution ID for correlation */
  executionId: string;
  /** Container image or "host" for non-sandboxed execution */
  image: string;
  /** Command being executed */
  command: string[];
  /** Execution context for UI grouping */
  context: ExecutionContext;
  /** Whether execution is sandboxed (container) or on host */
  sandboxed: boolean;
  /** Working directory */
  workdir: string;
  /** ISO timestamp */
  timestamp: string;
}

/**
 * Streaming output from container (emitted as chunks arrive)
 */
export interface ContainerOutputMessage {
  type: "container_output";
  /** Correlates to container_start executionId */
  executionId: string;
  /** Output chunk text */
  text: string;
  /** Which stream this came from */
  stream: ContainerStreamType;
  /** Sequence number for ordering (monotonic per executionId+stream) */
  sequence: number;
  /** Whether execution is sandboxed */
  sandboxed: boolean;
}

/**
 * Container execution completed
 */
export interface ContainerCompleteMessage {
  type: "container_complete";
  /** Correlates to container_start executionId */
  executionId: string;
  /** Exit code from command */
  exitCode: number;
  /** Total duration in milliseconds */
  durationMs: number;
  /** Whether execution was sandboxed */
  sandboxed: boolean;
}

/**
 * Container execution error (non-exit-code failure like timeout)
 */
export interface ContainerErrorMessage {
  type: "container_error";
  /** Correlates to container_start executionId */
  executionId: string;
  /** Error type */
  reason: "timeout" | "start_failed" | "aborted";
  /** Error message */
  error: string;
}

// ============================================================================
// Union Type for All Messages
// ============================================================================

export type HudMessage =
  | SessionStartMessage
  | SessionCompleteMessage
  | TaskSelectedMessage
  | TaskDecomposedMessage
  | SubtaskStartMessage
  | SubtaskCompleteMessage
  | SubtaskFailedMessage
  | VerificationStartMessage
  | VerificationCompleteMessage
  | CommitCreatedMessage
  | PushCompleteMessage
  | PhaseChangeMessage
  | ErrorMessage
  | TextOutputMessage
  | ToolCallMessage
  | ToolResultMessage
  | ATIFTrajectoryStartMessage
  | ATIFStepRecordedMessage
  | ATIFSubagentSpawnedMessage
  | ATIFTrajectoryCompleteMessage
  | APMUpdateMessage
  | APMSnapshotMessage
  | APMToolUsageMessage
  | UsageUpdateMessage
  | HealerInvocationStartMessage
  | HealerSpellAppliedMessage
  | HealerInvocationCompleteMessage
  | TBRunStartMessage
  | TBRunCompleteMessage
  | TBTaskStartMessage
  | TBTaskProgressMessage
  | TBTaskOutputMessage
  | TBTaskCompleteMessage
  | TBSuiteInfoMessage
  | TBRunRequestMessage
  | TBRunHistoryMessage
  | ContainerStartMessage
  | ContainerOutputMessage
  | ContainerCompleteMessage
  | ContainerErrorMessage;

/**
 * Status stream payloads (headless RPC-compatible)
 */
export type StatusStreamMessage = HudMessage | { type: "status_heartbeat"; ts: string };

// ============================================================================
// Protocol Constants
// ============================================================================

/** Default WebSocket port for HUD server */
export const HUD_WS_PORT = 4242;

/** Default WebSocket URL */
export const HUD_WS_URL = `ws://localhost:${HUD_WS_PORT}`;

// ============================================================================
// Helpers
// ============================================================================

export const isHudMessage = (data: unknown): data is HudMessage => {
  if (typeof data !== "object" || data === null) return false;
  const obj = data as Record<string, unknown>;
  return typeof obj.type === "string";
};

export const serializeHudMessage = (msg: HudMessage): string =>
  JSON.stringify(msg);

export const parseHudMessage = (data: string): HudMessage | null => {
  try {
    const parsed = JSON.parse(data);
    if (isHudMessage(parsed)) return parsed;
    return null;
  } catch {
    return null;
  }
};

// ============================================================================
// Terminal-Bench Type Guards
// ============================================================================

export const isTBRunStart = (msg: HudMessage): msg is TBRunStartMessage =>
  msg.type === "tb_run_start";

export const isTBRunComplete = (msg: HudMessage): msg is TBRunCompleteMessage =>
  msg.type === "tb_run_complete";

export const isTBTaskStart = (msg: HudMessage): msg is TBTaskStartMessage =>
  msg.type === "tb_task_start";

export const isTBTaskProgress = (msg: HudMessage): msg is TBTaskProgressMessage =>
  msg.type === "tb_task_progress";

export const isTBTaskOutput = (msg: HudMessage): msg is TBTaskOutputMessage =>
  msg.type === "tb_task_output";

export const isTBTaskComplete = (msg: HudMessage): msg is TBTaskCompleteMessage =>
  msg.type === "tb_task_complete";

export const isTBSuiteInfo = (msg: HudMessage): msg is TBSuiteInfoMessage =>
  msg.type === "tb_suite_info";

export const isTBRunRequest = (msg: HudMessage): msg is TBRunRequestMessage =>
  msg.type === "tb_run_request";

export const isTBRunHistory = (msg: HudMessage): msg is TBRunHistoryMessage =>
  msg.type === "tb_run_history";

/** Check if message is any TB-related message */
export const isTBMessage = (msg: HudMessage): boolean =>
  msg.type.startsWith("tb_");

// ============================================================================
// Container Event Type Guards
// ============================================================================

export const isContainerStart = (msg: HudMessage): msg is ContainerStartMessage =>
  msg.type === "container_start";

export const isContainerOutput = (msg: HudMessage): msg is ContainerOutputMessage =>
  msg.type === "container_output";

export const isContainerComplete = (msg: HudMessage): msg is ContainerCompleteMessage =>
  msg.type === "container_complete";

export const isContainerError = (msg: HudMessage): msg is ContainerErrorMessage =>
  msg.type === "container_error";

/** Check if message is any container-related message */
export const isContainerMessage = (msg: HudMessage): boolean =>
  msg.type.startsWith("container_");
