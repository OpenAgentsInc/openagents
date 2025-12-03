/**
 * Orchestrator/Subagent Architecture Types
 * 
 * Following Anthropic's "Effective Harnesses for Long-Running Agents" pattern:
 * - Orchestrator: Manages task selection, decomposition, verification, session coordination
 * - Subagent: Minimal coding agent that implements one subtask at a time
 */
import type { Task } from "../../tasks/index.js";
import type { Tool } from "../../tools/schema.js";

// ============================================================================
// Subtask Types
// ============================================================================

export type SubtaskStatus = "pending" | "in_progress" | "done" | "verified" | "failed";

export interface Subtask {
  id: string;
  description: string;
  status: SubtaskStatus;
  startedAt?: string;
  completedAt?: string;
  verifiedAt?: string;
  error?: string;
  /** Claude Code session tracking for resumption across orchestrator runs */
  claudeCode?: {
    /** Active Claude Code session ID used for this subtask */
    sessionId?: string;
    /** Session ID this run was forked from (when branching) */
    forkedFromSessionId?: string;
    /** Whether the next resume should fork instead of continue */
    resumeStrategy?: "continue" | "fork";
  };
}

export interface SubtaskList {
  taskId: string;
  taskTitle: string;
  subtasks: Subtask[];
  createdAt: string;
  updatedAt: string;
}

// ============================================================================
// Progress File Types
// ============================================================================

export type ClaudeCodePermissionMode =
  | "default"
  | "acceptEdits"
  | "bypassPermissions"
  | "plan"
  | "dontAsk";

export interface ClaudeCodeSettings {
  enabled?: boolean;
  preferForComplexTasks?: boolean;
  maxTurnsPerSubtask?: number;
  permissionMode?: ClaudeCodePermissionMode;
  fallbackToMinimal?: boolean;
  /** Abort Claude Code runs that exceed this duration to avoid stuck sessions */
  timeoutMsPerSubtask?: number;
}

export interface SessionProgress {
  sessionId: string;
  startedAt: string;
  taskId: string;
  taskTitle: string;
  orientation: {
    repoState: string;
    previousSessionSummary?: string;
    testsPassingAtStart: boolean;
    initScript?: InitScriptResult;
  };
  work: {
    subtasksCompleted: string[];
    subtasksInProgress: string[];
    filesModified: string[];
    testsRun: boolean;
    testsPassingAfterWork: boolean;
    /** Claude Code session metadata for context bridging */
    claudeCodeSession?: {
      sessionId?: string;
      forkedFromSessionId?: string;
      toolsUsed?: Record<string, number>;
      summary?: string;
      usage?: {
        inputTokens?: number;
        outputTokens?: number;
        cacheReadInputTokens?: number;
        cacheCreationInputTokens?: number;
      };
      totalCostUsd?: number;
    };
  };
  nextSession: {
    suggestedNextSteps: string[];
    blockers?: string[];
    notes?: string;
  };
  completedAt?: string;
}

// ============================================================================
// Subagent Types
// ============================================================================

export interface SubagentConfig {
  /** The subtask to complete */
  subtask: Subtask;
  /** Working directory */
  cwd: string;
  /** Available tools (should be minimal: read, write, edit, bash) */
  tools: Tool<any, any, any, any>[];
  /** Model to use */
  model?: string;
  /** Max turns before giving up */
  maxTurns?: number;
  /** Abort signal */
  signal?: AbortSignal;
}

export interface SubagentResult {
  success: boolean;
  subtaskId: string;
  filesModified: string[];
  error?: string;
  turns: number;
  agent?: "claude-code" | "minimal";
  /** Session ID returned by Claude Code for resumption */
  claudeCodeSessionId?: string;
  /** Original session ID when a forked branch was created */
  claudeCodeForkedFromSessionId?: string;
  tokenUsage?: {
    input: number;
    output: number;
  };
  verificationOutputs?: string[];
  /** Claude Code session metadata for progress.md bridging */
  sessionMetadata?: {
    sessionId?: string;
    forkedFromSessionId?: string;
    /** Tools used during session with counts */
    toolsUsed?: Record<string, number>;
    /** Blockers or errors encountered */
    blockers?: string[];
    /** Suggested next steps from agent */
    suggestedNextSteps?: string[];
    /** Final assistant message or summary */
    summary?: string;
    /** Token usage from Claude API */
    usage?: {
      inputTokens?: number;
      outputTokens?: number;
      cacheReadInputTokens?: number;
      cacheCreationInputTokens?: number;
    };
    /** Total cost in USD from Claude API */
    totalCostUsd?: number;
  };
}

// ============================================================================
// Orchestrator Types
// ============================================================================

export interface OrchestratorConfig {
  /** Working directory (repo root) */
  cwd: string;
  /** Path to .openagents directory */
  openagentsDir: string;
  /** Model to use for orchestrator decisions */
  model?: string;
  /** Model to use for coding subagent */
  subagentModel?: string;
  /** Typecheck commands from project.json */
  typecheckCommands?: string[];
  /** Test commands from project.json */
  testCommands: string[];
  /** E2E commands from project.json */
  e2eCommands?: string[];
  /** Whether to push after commit */
  allowPush: boolean;
  /** Max subtasks per task */
  maxSubtasksPerTask?: number;
  /** Claude Code integration settings */
  claudeCode?: ClaudeCodeSettings;
  /** Abort signal */
  signal?: AbortSignal;
}

export interface OrchestratorState {
  sessionId: string;
  task: Task | null;
  subtasks: SubtaskList | null;
  progress: SessionProgress | null;
  phase: OrchestratorPhase;
  error?: string;
}

export type OrchestratorPhase =
  | "idle"
  | "orienting"
  | "selecting_task"
  | "decomposing"
  | "executing_subtask"
  | "verifying"
  | "committing"
  | "updating_task"
  | "logging"
  | "done"
  | "failed";

// ============================================================================
// Event Types
// ============================================================================

export type OrchestratorEvent =
  | { type: "session_start"; sessionId: string; timestamp: string }
  | { type: "init_script_start"; path: string }
  | { type: "init_script_complete"; result: InitScriptResult }
  | { type: "orientation_complete"; repoState: string; testsPassingAtStart: boolean; initScript?: InitScriptResult }
  | { type: "task_selected"; task: Task }
  | { type: "task_decomposed"; subtasks: Subtask[] }
  | { type: "subtask_start"; subtask: Subtask }
  | { type: "subtask_complete"; subtask: Subtask; result: SubagentResult }
  | { type: "subtask_failed"; subtask: Subtask; error: string }
  | { type: "verification_start"; command: string }
  | { type: "verification_complete"; command: string; passed: boolean; output: string }
  | { type: "commit_created"; sha: string; message: string }
  | { type: "push_complete"; branch: string }
  | { type: "task_updated"; task: Task; status: string }
  | { type: "progress_written"; path: string }
  | { type: "session_complete"; success: boolean; summary: string }
  | { type: "error"; phase: OrchestratorPhase; error: string };

// ============================================================================
// Minimal Subagent Prompt
// ============================================================================

/**
 * The subagent prompt should be minimal (~50 tokens).
 * The model is RL-trained for coding - it doesn't need extensive instructions.
 */
export const SUBAGENT_SYSTEM_PROMPT = `You are an expert coding assistant. Complete the subtask below.

Tools: read, write, edit, bash

When done, output: SUBTASK_COMPLETE`;

export const buildSubagentPrompt = (subtask: Subtask): string => {
  return `## Subtask

${subtask.description}

Complete this subtask. When finished, output SUBTASK_COMPLETE on its own line.`;
};

// ============================================================================
// Coordination File Paths
// ============================================================================

export const getSubtasksPath = (openagentsDir: string, taskId: string): string =>
  `${openagentsDir}/subtasks/${taskId}.json`;

export const getProgressPath = (openagentsDir: string): string =>
  `${openagentsDir}/progress.md`;

export const getInitScriptPath = (openagentsDir: string): string =>
  `${openagentsDir}/init.sh`;

// ============================================================================
// Init Script Types
// ============================================================================

export interface InitScriptResult {
  ran: boolean;
  success: boolean;
  output?: string;
  durationMs?: number;
  error?: string;
}
