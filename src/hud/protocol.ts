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
  | HealerInvocationStartMessage
  | HealerSpellAppliedMessage
  | HealerInvocationCompleteMessage;

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
