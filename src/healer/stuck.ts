/**
 * Stuck Task/Subtask Detection
 *
 * Detects tasks and subtasks that appear to be stuck based on:
 * - Time thresholds (in_progress for too long)
 * - Repeated failure patterns in ATIF trajectories
 * - No progress indicators (no commits, no file changes)
 */
import { Effect } from "effect";
import type { Task } from "../tasks/schema.js";
import type { Subtask } from "../agent/orchestrator/types.js";
import type { Trajectory } from "../atif/schema.js";

// ============================================================================
// Types
// ============================================================================

/**
 * Configuration for stuck detection.
 */
export interface StuckDetectionConfig {
  /** Hours before a task is considered stuck (default: 4) */
  stuckTaskThresholdHours?: number;
  /** Hours before a subtask is considered stuck (default: 2) */
  stuckSubtaskThresholdHours?: number;
  /** Minimum consecutive failures to flag as stuck pattern (default: 3) */
  minConsecutiveFailures?: number;
  /** Whether to scan ATIF trajectories for failure patterns */
  scanTrajectories?: boolean;
}

/**
 * Result from stuck detection scan.
 */
export interface StuckDetectionResult {
  /** Tasks that appear stuck */
  stuckTasks: StuckTaskInfo[];
  /** Subtasks that appear stuck */
  stuckSubtasks: StuckSubtaskInfo[];
  /** Summary statistics */
  stats: {
    tasksScanned: number;
    subtasksScanned: number;
    trajectoriesScanned: number;
    stuckTaskCount: number;
    stuckSubtaskCount: number;
  };
}

/**
 * Information about a stuck task.
 */
export interface StuckTaskInfo {
  task: Task;
  reason: StuckReason;
  /** How long the task has been in_progress (hours) */
  hoursStuck: number;
  /** Related failure patterns found */
  failurePatterns: FailurePattern[];
}

/**
 * Information about a stuck subtask.
 */
export interface StuckSubtaskInfo {
  subtask: Subtask;
  taskId: string;
  reason: StuckReason;
  /** How long the subtask has been in_progress (hours) */
  hoursStuck: number;
  /** Consecutive failure count */
  failureCount: number;
}

/**
 * Reasons why something is considered stuck.
 */
export type StuckReason =
  | "time_threshold_exceeded"
  | "consecutive_failures"
  | "repeated_same_error"
  | "no_progress"
  | "manual_flag";

/**
 * A detected failure pattern.
 */
export interface FailurePattern {
  /** The error message/pattern */
  pattern: string;
  /** How many times this pattern occurred */
  occurrences: number;
  /** Session IDs where this pattern was found */
  sessionIds: string[];
}

// ============================================================================
// Default Configuration
// ============================================================================

const DEFAULT_CONFIG: Required<StuckDetectionConfig> = {
  stuckTaskThresholdHours: 4,
  stuckSubtaskThresholdHours: 2,
  minConsecutiveFailures: 3,
  scanTrajectories: true,
};

// ============================================================================
// Stuck Detection Functions
// ============================================================================

/**
 * Check if a task is stuck based on time threshold.
 */
export const isTaskStuckByTime = (
  task: Task,
  config: StuckDetectionConfig = {}
): { stuck: boolean; hoursStuck: number } => {
  const thresholdHours = config.stuckTaskThresholdHours ?? DEFAULT_CONFIG.stuckTaskThresholdHours;

  // Only check in_progress tasks
  if (task.status !== "in_progress") {
    return { stuck: false, hoursStuck: 0 };
  }

  // Calculate hours since last update
  const updatedAt = new Date(task.updatedAt);
  const now = new Date();
  const hoursStuck = (now.getTime() - updatedAt.getTime()) / (1000 * 60 * 60);

  return {
    stuck: hoursStuck >= thresholdHours,
    hoursStuck: Math.round(hoursStuck * 10) / 10,
  };
};

/**
 * Check if a subtask is stuck based on time and failure count.
 */
export const isSubtaskStuck = (
  subtask: Subtask,
  config: StuckDetectionConfig = {}
): { stuck: boolean; reason: StuckReason | null; hoursStuck: number } => {
  const thresholdHours = config.stuckSubtaskThresholdHours ?? DEFAULT_CONFIG.stuckSubtaskThresholdHours;
  const minFailures = config.minConsecutiveFailures ?? DEFAULT_CONFIG.minConsecutiveFailures;

  // Only check in_progress or failed subtasks
  if (subtask.status !== "in_progress" && subtask.status !== "failed") {
    return { stuck: false, reason: null, hoursStuck: 0 };
  }

  // Check consecutive failures
  const failureCount = subtask.failureCount ?? 0;
  if (failureCount >= minFailures) {
    return {
      stuck: true,
      reason: "consecutive_failures",
      hoursStuck: 0,
    };
  }

  // Check time threshold
  if (subtask.startedAt) {
    const startedAt = new Date(subtask.startedAt);
    const now = new Date();
    const hoursStuck = (now.getTime() - startedAt.getTime()) / (1000 * 60 * 60);

    if (hoursStuck >= thresholdHours) {
      return {
        stuck: true,
        reason: "time_threshold_exceeded",
        hoursStuck: Math.round(hoursStuck * 10) / 10,
      };
    }
  }

  return { stuck: false, reason: null, hoursStuck: 0 };
};

/**
 * Extract failure patterns from ATIF trajectories.
 */
export const extractFailurePatterns = (
  trajectories: Trajectory[],
  config: StuckDetectionConfig = {}
): FailurePattern[] => {
  const minOccurrences = config.minConsecutiveFailures ?? DEFAULT_CONFIG.minConsecutiveFailures;
  const patternMap = new Map<string, { count: number; sessionIds: string[] }>();

  for (const traj of trajectories) {
    // Look for error patterns in steps
    for (const step of traj.steps) {
      // Check observation results for errors
      if (step.observation?.results) {
        for (const result of step.observation.results) {
          const content = result.content;
          if (typeof content === "object" && content !== null) {
            const obj = content as Record<string, unknown>;
            if (obj.error && typeof obj.error === "string") {
              const pattern = normalizeErrorPattern(obj.error);
              const existing = patternMap.get(pattern) ?? { count: 0, sessionIds: [] };
              existing.count++;
              if (!existing.sessionIds.includes(traj.session_id)) {
                existing.sessionIds.push(traj.session_id);
              }
              patternMap.set(pattern, existing);
            }
          }
        }
      }

      // Check step message for error indicators
      if (step.message && /error|failed|exception/i.test(step.message)) {
        const pattern = normalizeErrorPattern(step.message);
        const existing = patternMap.get(pattern) ?? { count: 0, sessionIds: [] };
        existing.count++;
        if (!existing.sessionIds.includes(traj.session_id)) {
          existing.sessionIds.push(traj.session_id);
        }
        patternMap.set(pattern, existing);
      }
    }
  }

  // Filter to patterns that occur enough times
  return Array.from(patternMap.entries())
    .filter(([_, info]) => info.count >= minOccurrences)
    .map(([pattern, info]) => ({
      pattern,
      occurrences: info.count,
      sessionIds: info.sessionIds,
    }))
    .sort((a, b) => b.occurrences - a.occurrences);
};

/**
 * Normalize an error message to create a pattern key.
 * Removes timestamps, line numbers, and variable parts.
 */
const normalizeErrorPattern = (error: string): string => {
  return error
    // Remove timestamps
    .replace(/\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}[.\d]*Z?/g, "<timestamp>")
    // Remove line numbers
    .replace(/:\d+:\d+/g, ":<line>")
    // Remove file paths with hashes
    .replace(/[a-f0-9]{8,}/gi, "<hash>")
    // Truncate to reasonable length
    .slice(0, 200);
};

// ============================================================================
// Scan Functions
// ============================================================================

/**
 * Scan tasks for stuck items.
 */
export const scanTasksForStuck = (
  tasks: Task[],
  config: StuckDetectionConfig = {}
): StuckTaskInfo[] => {
  const stuckTasks: StuckTaskInfo[] = [];

  for (const task of tasks) {
    const { stuck, hoursStuck } = isTaskStuckByTime(task, config);

    if (stuck) {
      stuckTasks.push({
        task,
        reason: "time_threshold_exceeded",
        hoursStuck,
        failurePatterns: [],
      });
    }
  }

  return stuckTasks;
};

/**
 * Scan subtasks for stuck items.
 */
export const scanSubtasksForStuck = (
  subtasks: Array<{ subtask: Subtask; taskId: string }>,
  config: StuckDetectionConfig = {}
): StuckSubtaskInfo[] => {
  const stuckSubtasks: StuckSubtaskInfo[] = [];

  for (const { subtask, taskId } of subtasks) {
    const { stuck, reason, hoursStuck } = isSubtaskStuck(subtask, config);

    if (stuck && reason) {
      stuckSubtasks.push({
        subtask,
        taskId,
        reason,
        hoursStuck,
        failureCount: subtask.failureCount ?? 0,
      });
    }
  }

  return stuckSubtasks;
};

/**
 * Full stuck detection scan.
 */
export const detectStuck = (
  tasks: Task[],
  subtasks: Array<{ subtask: Subtask; taskId: string }>,
  trajectories: Trajectory[],
  config: StuckDetectionConfig = {}
): StuckDetectionResult => {
  const stuckTasks = scanTasksForStuck(tasks, config);
  const stuckSubtasks = scanSubtasksForStuck(subtasks, config);

  // Extract failure patterns and associate with stuck tasks
  if (config.scanTrajectories ?? DEFAULT_CONFIG.scanTrajectories) {
    const failurePatterns = extractFailurePatterns(trajectories, config);
    for (const stuckTask of stuckTasks) {
      stuckTask.failurePatterns = failurePatterns;
    }
  }

  return {
    stuckTasks,
    stuckSubtasks,
    stats: {
      tasksScanned: tasks.length,
      subtasksScanned: subtasks.length,
      trajectoriesScanned: trajectories.length,
      stuckTaskCount: stuckTasks.length,
      stuckSubtaskCount: stuckSubtasks.length,
    },
  };
};

/**
 * Create a summary message for stuck detection results.
 */
export const summarizeStuckDetection = (result: StuckDetectionResult): string => {
  const { stats, stuckTasks, stuckSubtasks } = result;
  const lines: string[] = [];

  lines.push(`Scanned ${stats.tasksScanned} tasks, ${stats.subtasksScanned} subtasks, ${stats.trajectoriesScanned} trajectories`);

  if (stats.stuckTaskCount === 0 && stats.stuckSubtaskCount === 0) {
    lines.push("No stuck items detected.");
    return lines.join("\n");
  }

  if (stuckTasks.length > 0) {
    lines.push(`\nStuck tasks (${stuckTasks.length}):`);
    for (const stuck of stuckTasks) {
      lines.push(`  - ${stuck.task.id}: ${stuck.task.title} (${stuck.hoursStuck}h, ${stuck.reason})`);
    }
  }

  if (stuckSubtasks.length > 0) {
    lines.push(`\nStuck subtasks (${stuckSubtasks.length}):`);
    for (const stuck of stuckSubtasks) {
      lines.push(`  - ${stuck.subtask.id}: ${stuck.subtask.description.slice(0, 50)}... (${stuck.reason}, ${stuck.failureCount} failures)`);
    }
  }

  return lines.join("\n");
};
