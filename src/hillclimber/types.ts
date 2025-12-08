/**
 * HillClimber Types
 *
 * Type definitions for the TBHillClimber overnight optimization system.
 * These types map to the SQLite schema in .openagents/migrations/003_hillclimber.sql
 */

// ============================================================================
// Core Domain Types
// ============================================================================

/**
 * Task configuration - the "knobs" we're tuning
 */
export interface HillClimberConfig {
  id: number;
  taskId: string;
  hint: string | null;
  useSkills: boolean;
  maxTurnsOverride: number;
  configHash: string;
  isCurrent: boolean;
  createdAt: string;
}

/**
 * Input for creating a new config (without auto-generated fields)
 */
export interface HillClimberConfigInput {
  taskId: string;
  hint: string | null;
  useSkills: boolean;
  maxTurnsOverride: number;
}

/**
 * Run record - every execution attempt
 */
export interface HillClimberRun {
  id: number;
  runId: string;
  taskId: string;
  configId: number;
  passed: boolean;
  turns: number;
  durationMs: number;
  stepSummary: string[] | null;
  errorMessage: string | null;
  metaModel: string | null;
  proposedChange: string | null;
  changeAccepted: boolean;
  score: number;
  isBest: boolean;
  createdAt: string;
}

/**
 * Input for creating a new run (without auto-generated fields)
 */
export interface HillClimberRunInput {
  runId: string;
  taskId: string;
  configId: number;
  passed: boolean;
  turns: number;
  durationMs: number;
  stepSummary: string[] | null;
  errorMessage: string | null;
  metaModel: string | null;
  proposedChange: string | null;
  changeAccepted: boolean;
  score: number;
}

/**
 * Best config per task (for quick lookup and export)
 */
export interface BestConfig {
  taskId: string;
  configId: number;
  runId: number;
  score: number;
  passCount: number;
  totalRuns: number;
  updatedAt: string;
}

// ============================================================================
// Execution Types
// ============================================================================

/**
 * Result from running a single TB task
 */
export interface TaskRunResult {
  passed: boolean;
  turns: number;
  durationMs: number;
  stepSummary: string[];
  errorMessage: string | null;
  output: string;
}

/**
 * Proposed config change from meta-reasoner
 */
export interface ConfigChange {
  type: "keep" | "update_hint" | "toggle_skills" | "adjust_turns";
  newHint?: string;
  newUseSkills?: boolean;
  newMaxTurns?: number;
  reasoning?: string;
  model?: string; // Actual model that was used for this change
}

// ============================================================================
// Statistics Types
// ============================================================================

/**
 * Stats for a single task
 */
export interface TaskStats {
  taskId: string;
  totalRuns: number;
  passCount: number;
  passRate: number;
  bestScore: number;
  avgTurns: number;
  lastRunAt: string | null;
  currentConfigId: number | null;
  bestConfigId: number | null;
}

/**
 * Aggregate stats across all tasks
 */
export interface HillClimberStats {
  totalRuns: number;
  totalPasses: number;
  overallPassRate: number;
  uniqueTasks: number;
  uniqueConfigs: number;
  byTask: Record<string, TaskStats>;
}

// ============================================================================
// CLI Options
// ============================================================================

/**
 * CLI options for the hillclimber command
 */
export interface HillClimberOptions {
  tasks: string[];           // Task IDs to optimize (empty = all)
  maxRuns: number;           // Max runs before stopping
  sleepMs: number;           // Sleep between runs (ms)
  dryRun: boolean;           // Show what would happen without executing
  showStats: boolean;        // Show stats and exit
  exportHints: boolean;      // Export best hints to hints.ts
  suitePath: string;         // Path to Terminal-Bench suite
}

// ============================================================================
// Row Converters (SQLite row -> TypeScript object)
// ============================================================================

/**
 * Convert SQLite row to HillClimberConfig
 */
export const rowToConfig = (row: any): HillClimberConfig => ({
  id: row.id,
  taskId: row.task_id,
  hint: row.hint,
  useSkills: Boolean(row.use_skills),
  maxTurnsOverride: row.max_turns_override,
  configHash: row.config_hash,
  isCurrent: Boolean(row.is_current),
  createdAt: row.created_at,
});

/**
 * Convert SQLite row to HillClimberRun
 */
export const rowToRun = (row: any): HillClimberRun => ({
  id: row.id,
  runId: row.run_id,
  taskId: row.task_id,
  configId: row.config_id,
  passed: Boolean(row.passed),
  turns: row.turns,
  durationMs: row.duration_ms,
  stepSummary: row.step_summary ? JSON.parse(row.step_summary) : null,
  errorMessage: row.error_message,
  metaModel: row.meta_model,
  proposedChange: row.proposed_change,
  changeAccepted: Boolean(row.change_accepted),
  score: row.score,
  isBest: Boolean(row.is_best),
  createdAt: row.created_at,
});

/**
 * Convert SQLite row to BestConfig
 */
export const rowToBestConfig = (row: any): BestConfig => ({
  taskId: row.task_id,
  configId: row.config_id,
  runId: row.run_id,
  score: row.score,
  passCount: row.pass_count,
  totalRuns: row.total_runs,
  updatedAt: row.updated_at,
});

// ============================================================================
// Helpers
// ============================================================================

/**
 * Generate a unique run ID
 */
export const generateRunId = (): string => {
  const now = new Date();
  const dateStr = now.toISOString().slice(0, 10).replace(/-/g, "");
  const timeStr = now.toISOString().slice(11, 19).replace(/:/g, "");
  const random = Math.random().toString(36).slice(2, 8);
  return `hc-${dateStr}-${timeStr}-${random}`;
};

/**
 * Default starting tasks for the HillClimber
 * These are relatively simple TB2 tasks that FM should be able to handle
 */
export const DEFAULT_STARTING_TASKS = [
  "regex-log",
  // Add more task IDs as we identify them from the TB2 suite
];
