/**
 * Healer Subagent Types
 *
 * A self-healing subagent that wakes up automatically when agent trajectories
 * go off the rails, diagnoses what went wrong, and tries to repair or safely
 * contain the damage.
 */
import type { Effect } from "effect";
import type { Task, ProjectConfig } from "../tasks/schema.js";
import type { Trajectory } from "../atif/schema.js";
import type {
  OrchestratorEvent,
  OrchestratorState,
  Subtask,
  InitScriptFailureType,
} from "../agent/orchestrator/types.js";

// ============================================================================
// Healer Scenarios
// ============================================================================

/**
 * Scenarios that trigger Healer invocation.
 * Each scenario maps to a specific failure pattern in the orchestrator.
 */
export type HealerScenario =
  | "InitScriptTypecheckFailure"   // Init script failed due to TypeScript errors
  | "InitScriptTestFailure"        // Init script failed due to test failures
  | "InitScriptEnvironmentFailure" // Init script failed due to env issues (network, disk, etc.)
  | "VerificationFailed"           // Post-work verification (tests/typecheck) failed
  | "SubtaskFailed"                // A subtask failed during execution
  | "SubtaskStuck"                 // A subtask is stuck (no progress for N minutes)
  | "RuntimeError";                // Unexpected runtime error in orchestrator

// ============================================================================
// Healer Spells
// ============================================================================

/**
 * Spell identifiers - each spell is a controlled recovery operation.
 * Spells are designed to never make things worse.
 */
export type HealerSpellId =
  | "rewind_uncommitted_changes"      // git restore + git clean -fd
  | "rewind_to_last_green_commit"     // git reset to last known-good commit
  | "mark_task_blocked_with_followup" // Mark task blocked, create follow-up
  | "retry_with_minimal_subagent"     // Fall back to minimal subagent
  | "retry_with_claude_code_resume"   // Resume Claude Code session for recovery
  | "fix_typecheck_errors"            // Emergency typecheck fix (from safe-mode)
  | "fix_test_errors"                 // Emergency test fix
  | "update_progress_with_guidance"   // Update progress.md with failure details
  | "run_tasks_doctor_like_checks";   // Validate tasks.jsonl structure

/**
 * Result from executing a spell.
 */
export interface HealerSpellResult {
  /** Whether the spell executed successfully */
  success: boolean;
  /** Whether the spell made changes to the repo/tasks */
  changesApplied?: boolean;
  /** Human-readable summary of what happened */
  summary: string;
  /** Optional error message if spell failed */
  error?: string;
  /** Files modified by this spell */
  filesModified?: string[];
  /** Tasks created or modified by this spell */
  tasksAffected?: string[];
}

/**
 * A spell definition with its implementation.
 */
export interface HealerSpell {
  /** Unique identifier for this spell */
  id: HealerSpellId;
  /** Human-readable description */
  description: string;
  /** Whether this spell requires LLM invocation (vs pure code) */
  requiresLLM: boolean;
  /** Apply the spell to the given context */
  apply: (ctx: HealerContext) => Effect.Effect<HealerSpellResult, Error, never>;
}

// ============================================================================
// Healer Outcome
// ============================================================================

/**
 * Outcome status after Healer runs.
 */
export type HealerOutcomeStatus =
  | "resolved"    // Problem fixed, orchestrator can continue
  | "contained"   // Problem contained (task blocked, progress updated), stop gracefully
  | "unresolved"  // Could not fix or contain, orchestrator should abort
  | "skipped";    // Healer chose not to run (policy/limits)

/**
 * Overall result from a Healer invocation.
 */
export interface HealerOutcome {
  /** The scenario that triggered Healer */
  scenario: HealerScenario;
  /** Final status */
  status: HealerOutcomeStatus;
  /** Spells that were attempted */
  spellsTried: HealerSpellId[];
  /** Spells that succeeded */
  spellsSucceeded: HealerSpellId[];
  /** Human-readable summary */
  summary: string;
  /** Whether verification passed after healing (if applicable) */
  verificationPassed?: boolean;
  /** ATIF trajectory session ID for this Healer invocation */
  trajectorySessionId?: string;
}

// ============================================================================
// Healer Context
// ============================================================================

/**
 * Git repository status for Healer context.
 */
export interface GitStatus {
  /** Whether the repo has uncommitted changes */
  isDirty: boolean;
  /** List of modified files */
  modifiedFiles: string[];
  /** List of untracked files */
  untrackedFiles: string[];
  /** Current branch name */
  currentBranch: string;
  /** Last commit SHA */
  lastCommitSha: string;
  /** Last commit message */
  lastCommitMessage: string;
}

/**
 * Heuristics computed from available data to inform spell selection.
 */
export interface HealerHeuristics {
  /** The detected scenario */
  scenario: HealerScenario;
  /** Number of times this subtask has failed */
  failureCount: number;
  /** Whether this appears to be a flaky failure (different errors each time) */
  isFlaky: boolean;
  /** Whether the failure is likely due to missing imports */
  hasMissingImports: boolean;
  /** Whether the failure is likely due to type errors */
  hasTypeErrors: boolean;
  /** Whether the failure is likely due to test assertions */
  hasTestAssertions: boolean;
  /** Detected error patterns from output */
  errorPatterns: string[];
  /** Previous attempts made (if any) */
  previousAttempts: number;
}

/**
 * Full context passed to Healer for diagnosis and repair.
 */
export interface HealerContext {
  /** Project root directory */
  projectRoot: string;
  /** Loaded project configuration */
  projectConfig: ProjectConfig;
  /** Current task being worked on (if any) */
  task?: Task;
  /** Current subtask (if applicable) */
  subtask?: Subtask;
  /** Current orchestrator session ID */
  sessionId: string;
  /** Run ID for this orchestrator run */
  runId?: string;
  /** ATIF trajectory for current session (if available) */
  trajectory?: Trajectory;
  /** Related trajectories (parent, siblings) */
  relatedTrajectories: Trajectory[];
  /** Content of progress.md (if exists) */
  progressMd: string | null;
  /** Current git status */
  gitStatus: GitStatus;
  /** Computed heuristics for spell selection */
  heuristics: HealerHeuristics;
  /** The event that triggered Healer */
  triggerEvent: OrchestratorEvent;
  /** Current orchestrator state */
  orchestratorState: OrchestratorState;
  /** Init script failure type (if applicable) */
  initFailureType?: InitScriptFailureType;
  /** Raw error output from the failure */
  errorOutput?: string;
  /** Healer invocation counters for this session */
  counters: HealerCounters;
}

// ============================================================================
// Healer Counters (for policy enforcement)
// ============================================================================

/**
 * Counters to track Healer invocations for rate limiting.
 */
export interface HealerCounters {
  /** Total Healer invocations this session */
  sessionInvocations: number;
  /** Healer invocations per subtask (keyed by subtask ID) */
  subtaskInvocations: Map<string, number>;
  /** Spells attempted this session (keyed by spell ID) */
  spellsAttempted: Map<HealerSpellId, number>;
  /** Follow-up containment tasks created (keyed by taskId:scenario) */
  followupKeys: Set<string>;
}

/**
 * Create fresh counters for a new session.
 */
export const createHealerCounters = (): HealerCounters => ({
  sessionInvocations: 0,
  subtaskInvocations: new Map(),
  spellsAttempted: new Map(),
  followupKeys: new Set(),
});

// ============================================================================
// Healer Trigger
// ============================================================================

/**
 * Information about what triggered Healer.
 */
export interface HealerTrigger {
  /** The scenario that was detected */
  scenario: HealerScenario;
  /** The orchestrator event that triggered Healer */
  event: OrchestratorEvent;
  /** Current orchestrator state */
  state: OrchestratorState;
  /** Raw error output (if available) */
  errorOutput?: string;
}

// ============================================================================
// Policy Decision
// ============================================================================

/**
 * Result of checking whether Healer should run.
 */
export interface HealerPolicyDecision {
  /** Whether Healer should run */
  run: boolean;
  /** The detected scenario (if run=true) */
  scenario?: HealerScenario;
  /** Reason for the decision (especially if run=false) */
  reason: string;
}

// ============================================================================
// Type Guards
// ============================================================================

/**
 * Check if an outcome indicates the problem was resolved.
 */
export const isResolved = (outcome: HealerOutcome): boolean =>
  outcome.status === "resolved";

/**
 * Check if an outcome indicates the problem was contained.
 */
export const isContained = (outcome: HealerOutcome): boolean =>
  outcome.status === "contained";

/**
 * Check if a spell requires LLM invocation.
 */
export const spellRequiresLLM = (spellId: HealerSpellId): boolean => {
  const llmSpells: HealerSpellId[] = [
    "fix_typecheck_errors",
    "fix_test_errors",
    "retry_with_claude_code_resume",
    "retry_with_minimal_subagent",
  ];
  return llmSpells.includes(spellId);
};

/**
 * Map InitScriptFailureType to HealerScenario.
 */
export const mapInitFailureToScenario = (
  failureType: InitScriptFailureType
): HealerScenario => {
  switch (failureType) {
    case "typecheck_failed":
      return "InitScriptTypecheckFailure";
    case "test_failed":
      return "InitScriptTestFailure";
    case "network_error":
    case "disk_full":
    case "permission_denied":
    case "unknown":
    default:
      return "InitScriptEnvironmentFailure";
  }
};
