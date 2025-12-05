/**
 * Orchestrator/Subagent Architecture Types
 * 
 * Following Anthropic's "Effective Harnesses for Long-Running Agents" pattern:
 * - Orchestrator: Manages task selection, decomposition, verification, session coordination
 * - Subagent: Minimal coding agent that implements one subtask at a time
 */
import type { Task, SandboxConfig, ProjectConfig } from "../../tasks/index.js";
import type { Tool } from "../../tools/schema.js";
import type { HealerOutcome, HealerCounters } from "../../healer/index.js";
import type { Effect } from "effect";
import type { UsageRecord } from "../../usage/types.js";
import type {
  ReflectionType,
  FailureContextType,
  ReflexionConfigType,
} from "./reflection/index.js";
import type { HudMessage } from "../../hud/protocol.js";
import type { HookCallback } from "@anthropic-ai/claude-agent-sdk";

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
  /** Number of consecutive failures on this subtask */
  failureCount?: number;
  /** Last failure reason (for context when resuming) */
  lastFailureReason?: string;
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
    e2eRun: boolean;
    e2ePassingAfterWork: boolean;
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
  /** Streaming output callback (HUD/logging) */
  onOutput?: (text: string) => void;
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
  /** Test commands for sandbox execution (container-safe subset) */
  sandboxTestCommands?: string[];
  /** E2E commands from project.json */
  e2eCommands?: string[];
  /** Whether to push after commit */
  allowPush: boolean;
  /** Max subtasks per task */
  maxSubtasksPerTask?: number;
  /** Claude Code integration settings */
  claudeCode?: ClaudeCodeSettings;
  /** Sandbox execution settings */
  sandbox?: SandboxConfig;
  /** Abort signal */
  signal?: AbortSignal;
  /** Callback for streaming text output from Claude Code */
  onOutput?: (text: string) => void;
  /** Callback for emitting HUD messages directly (container events, etc.) */
  emitHud?: (message: HudMessage) => void;
  /** Enable self-healing for init script failures (typecheck, etc.) */
  safeMode?: boolean;
  /** Additional context (e.g., AGENTS.md content) to prepend to subagent prompts */
  additionalContext?: string;
  /** PreToolUse hook to enforce worktree isolation for Claude Code */
  worktreeGuardHook?: HookCallback;
  /** Skip init script (useful for worktree runs where main repo is already validated) */
  skipInitScript?: boolean;
  /** Pre-assigned task (skip pickNextTask if provided) - used by parallel runner */
  task?: Task;
  /** Force creating new subtasks instead of reading existing ones - used by parallel runner */
  forceNewSubtasks?: boolean;

  // Healer integration (NEW)
  /** Healer service for self-healing on failures */
  healerService?: {
    maybeRun: (
      event: OrchestratorEvent,
      state: OrchestratorState,
      config: ProjectConfig,
      counters: HealerCounters
    ) => Effect.Effect<HealerOutcome | null, Error, never>;
  };
  /** Healer invocation counters (per-session, per-subtask limits) */
  healerCounters?: HealerCounters;
  /** Full project config (needed for Healer's policy decisions) */
  projectConfig?: ProjectConfig;

  // Reflexion integration (verbal self-reflection on failures)
  /** ReflectionService for generating reflections after failures */
  reflectionService?: {
    generate: (failure: FailureContextType) => Effect.Effect<ReflectionType | null, Error>;
    getRecent: (subtaskId: string, limit?: number) => Effect.Effect<ReflectionType[], Error>;
    save: (reflection: ReflectionType) => Effect.Effect<void, Error>;
    formatForPrompt: (reflections: ReflectionType[]) => Effect.Effect<string, Error>;
  };
  /** Reflexion configuration */
  reflexionConfig?: ReflexionConfigType;
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
  | { type: "lock_acquired"; pid: number; sessionId?: string }
  | { type: "lock_stale_removed"; stalePid: number; newPid: number }
  | { type: "lock_failed"; reason: string; existingPid?: number; existingSessionId?: string }
  | { type: "lock_released" }
  | { type: "init_script_start"; path: string }
  | { type: "init_script_complete"; result: InitScriptResult }
  | { type: "orientation_complete"; repoState: string; testsPassingAtStart: boolean; initScript?: InitScriptResult }
  // Recovery events (two-phase commit crash recovery)
  | { type: "recovery_start"; pendingCount: number }
  | { type: "recovery_task_closed"; taskId: string; sha: string }
  | { type: "recovery_task_reset"; taskId: string }
  | { type: "recovery_complete"; closedCount: number; resetCount: number; failedCount: number }
  // Checkpoint events (phase checkpoint crash recovery)
  | { type: "checkpoint_found"; sessionId: string; phase: OrchestratorPhase; taskId: string }
  | { type: "checkpoint_resuming"; phase: OrchestratorPhase; taskId: string }
  | { type: "checkpoint_invalid"; reason: string }
  | { type: "checkpoint_written"; phase: OrchestratorPhase }
  | { type: "checkpoint_cleared" }
  | { type: "task_selected"; task: Task }
  | { type: "task_decomposed"; subtasks: Subtask[] }
  | { type: "subtask_start"; subtask: Subtask }
  | { type: "subtask_complete"; subtask: Subtask; result: SubagentResult }
  | { type: "subtask_failed"; subtask: Subtask; error: string }
  | { type: "verification_start"; command: string }
  | { type: "verification_complete"; command: string; passed: boolean; output: string }
  | { type: "verification_output"; command: string; chunk: string; stream: "stdout" | "stderr" }
  | { type: "e2e_start"; command: string }
  | { type: "e2e_complete"; command: string; passed: boolean; output: string }
  | { type: "e2e_skipped"; reason: string }
  | { type: "commit_created"; sha: string; message: string }
  | { type: "push_complete"; branch: string }
  | { type: "task_updated"; task: Task; status: string }
  | { type: "progress_written"; path: string }
  | { type: "usage_recorded"; usage: UsageRecord }
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

export const getAgentLockPath = (openagentsDir: string): string =>
  `${openagentsDir}/agent.lock`;

// ============================================================================
// Init Script Types
// ============================================================================

/**
 * Structured failure types for init script errors.
 * Allows safe mode to determine appropriate recovery strategy.
 */
export type InitScriptFailureType =
  | "typecheck_failed"      // TypeScript/type errors - can self-heal
  | "test_failed"           // Tests failing - can attempt fix
  | "network_error"         // Network issues - can continue in offline mode
  | "disk_full"             // Disk space issues - cannot self-heal
  | "permission_denied"     // Permission issues - cannot self-heal
  | "unknown";              // Unknown error - fallback

/**
 * Result from running the preflight init.sh script.
 *
 * Exit codes (per GOLDEN-LOOP-v2.md Section 2.2.1):
 * - 0: All checks passed → success=true, hasWarnings=false
 * - 1: Fatal error → success=false (abort session)
 * - 2: Warnings only → success=true, hasWarnings=true (continue with caution)
 */
export interface InitScriptResult {
  ran: boolean;
  /** true if script exited with 0 or 2 (proceed), false if exit 1 (abort) */
  success: boolean;
  /** true if script exited with 2 (warnings present but proceed) */
  hasWarnings?: boolean;
  /** exit code from the script (0, 1, or 2) */
  exitCode?: number;
  output?: string;
  durationMs?: number;
  error?: string;
  /** Structured failure type for safe mode recovery */
  failureType?: InitScriptFailureType;
  /** Whether this failure type can potentially be self-healed */
  canSelfHeal?: boolean;
}
