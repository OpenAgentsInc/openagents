/**
 * HillClimber Task Decomposer Module
 *
 * Breaks complex tasks into subtasks with verification checkpoints.
 * Task-specific decomposition rules for all Terminal-Bench 2 tasks.
 *
 * Part of the MAP-inspired architecture for 10x better HillClimber.
 */

import type { TerminalBenchTask } from "../bench/terminal-bench.js";

// ============================================================================
// Types
// ============================================================================

export interface Subtask {
  /** Unique subtask ID within the task */
  id: number;
  /** Short name for the subtask */
  name: string;
  /** Detailed goal description */
  goal: string;
  /** Verification checkpoint (what to check after completing) */
  checkpoint: string;
  /** Expected output files or artifacts */
  expectedArtifacts: string[];
  /** Dependencies on previous subtasks */
  dependsOn: number[];
  /** Hints specific to this subtask */
  hints: string[];
  /** Maximum turns to spend on this subtask */
  maxTurns: number;
}

export interface TaskDecomposition {
  /** Original task ID */
  taskId: string;
  /** Total number of subtasks */
  subtaskCount: number;
  /** Ordered list of subtasks */
  subtasks: Subtask[];
  /** Overall task hints (apply to all subtasks) */
  globalHints: string[];
  /** Files to read before starting */
  filesToRead: string[];
  /** Output files that must exist for success */
  requiredOutputs: string[];
}

// ============================================================================
// GUARDRAIL: NO TASK-SPECIFIC HARDCODING
//
// This file must NEVER contain:
// - Task IDs (e.g., "regex-log", "path-tracing")
// - Task-specific patterns (e.g., IPv4 format, date format)
// - Task-specific hints (e.g., "use lookahead for IPv4")
// - Task-specific file paths (e.g., "/app/regex.txt")
//
// All knowledge must come from:
// 1. The task description (passed as parameter)
// 2. General process knowledge (TDD, iteration)
//
// If you're tempted to add task-specific code, you're defeating the thesis:
// "Architecture beats model size"
// ============================================================================

// ============================================================================
// Helper Functions: Extract Information from Task Description
// ============================================================================

/**
 * Extract files mentioned in the task description.
 * Looks for patterns like /app/filename or /app/path/to/file
 */
function extractFilesToRead(description: string): string[] {
  const filePattern = /\/app\/[\w\-\.\/]+/g;
  const matches = description.match(filePattern) || [];
  return [...new Set(matches)];
}

/**
 * Extract required output files from task description.
 * Looks for patterns like "write to /app/X" or "output file: /app/X"
 */
function extractRequiredOutputs(description: string): string[] {
  // Look for patterns like "write to /app/X" or "output file: /app/X"
  const outputPattern = /(?:write|output|create|save).*?(\/app\/[\w\-\.]+)/gi;
  const matches = [...description.matchAll(outputPattern)];
  return matches.map(m => m[1]);
}

// ============================================================================
// Main Decomposer
// ============================================================================

/**
 * Decompose a Terminal-Bench task into subtasks.
 *
 * This is a GENERAL-PURPOSE decomposer that works for ANY task.
 * It uses ONLY the task description to generate subtasks.
 *
 * @param task Terminal-Bench task
 * @returns Task decomposition with generic subtasks
 */
export function decomposeTask(task: TerminalBenchTask): TaskDecomposition {
  // Extract information from task description
  const filesToRead = extractFilesToRead(task.description);
  const requiredOutputs = extractRequiredOutputs(task.description);

  // Generate GENERIC subtasks that work for any task
  return {
    taskId: task.id,
    subtaskCount: 4,
    subtasks: [
      {
        id: 1,
        name: "understand-requirements",
        goal: "Read the task description carefully. Identify: (1) required output files, (2) success criteria, (3) any constraints mentioned.",
        checkpoint: "Task requirements are understood",
        expectedArtifacts: [],
        dependsOn: [],
        hints: [
          "Use read_file to examine any example files mentioned",
          "Note the exact output format required",
        ],
        maxTurns: 3,
      },
      {
        id: 2,
        name: "write-initial-solution",
        goal: "Write an initial solution based on your understanding of the requirements.",
        checkpoint: "Initial solution file exists",
        expectedArtifacts: requiredOutputs.length > 0 ? requiredOutputs : [],
        dependsOn: [1],
        hints: [
          "Start simple - get something working first",
          "Use write_file to create the required output",
        ],
        maxTurns: 5,
      },
      {
        id: 3,
        name: "test-and-iterate",
        goal: "Run verify_progress to see test results. Analyze failures and fix issues.",
        checkpoint: "At least 50% of test cases passing",
        expectedArtifacts: requiredOutputs.length > 0 ? requiredOutputs : [],
        dependsOn: [2],
        hints: [
          "Read the failure messages carefully",
          "Make ONE targeted change per iteration",
          "False positives: tighten constraints",
          "False negatives: loosen constraints",
        ],
        maxTurns: 10,
      },
      {
        id: 4,
        name: "final-validation",
        goal: "Ensure all tests pass. Fix any remaining edge cases.",
        checkpoint: "100% test cases passing",
        expectedArtifacts: requiredOutputs.length > 0 ? requiredOutputs : [],
        dependsOn: [3],
        hints: [
          "Check boundary conditions",
          "Test edge cases mentioned in failures",
        ],
        maxTurns: 5,
      },
    ],
    globalHints: [
      // ONLY general process knowledge
      "Use verify_progress after each change to get feedback",
      "Read failure messages to understand what's wrong",
      "Iterate until all tests pass",
    ],
    filesToRead,
    requiredOutputs,
  };
}

/**
 * Get the current subtask based on execution state.
 *
 * @param decomposition Task decomposition
 * @param completedSubtasks IDs of completed subtasks
 * @returns Current subtask to work on, or null if all complete
 */
export function getCurrentSubtask(
  decomposition: TaskDecomposition,
  completedSubtasks: number[]
): Subtask | null {
  for (const subtask of decomposition.subtasks) {
    // Check if already completed
    if (completedSubtasks.includes(subtask.id)) {
      continue;
    }

    // Check if dependencies are met
    const depsComplete = subtask.dependsOn.every((dep) => completedSubtasks.includes(dep));
    if (depsComplete) {
      return subtask;
    }
  }

  return null;
}

/**
 * Build a prompt for the current subtask.
 */
export function buildSubtaskPrompt(
  decomposition: TaskDecomposition,
  subtask: Subtask,
  previousFeedback?: string
): string {
  const lines: string[] = [];

  lines.push(`## Current Subtask: ${subtask.name} (${subtask.id}/${decomposition.subtaskCount})`);
  lines.push("");
  lines.push(`**Goal:** ${subtask.goal}`);
  lines.push("");
  lines.push(`**Checkpoint:** ${subtask.checkpoint}`);
  lines.push("");

  if (subtask.hints.length > 0) {
    lines.push("**Hints:**");
    for (const hint of subtask.hints) {
      lines.push(`- ${hint}`);
    }
    lines.push("");
  }

  if (subtask.expectedArtifacts.length > 0) {
    lines.push(`**Expected outputs:** ${subtask.expectedArtifacts.join(", ")}`);
    lines.push("");
  }

  if (previousFeedback) {
    lines.push("**Previous attempt feedback:**");
    lines.push(previousFeedback);
    lines.push("");
  }

  return lines.join("\n");
}

/**
 * Check if a subtask is complete based on evaluation results.
 */
export function isSubtaskComplete(
  subtask: Subtask,
  progress: number,
  artifacts: string[]
): boolean {
  // Check if all expected artifacts exist
  const hasAllArtifacts = subtask.expectedArtifacts.every((a) =>
    artifacts.some((artifact) => artifact.endsWith(a) || artifact === a)
  );

  if (!hasAllArtifacts) {
    return false;
  }

  // For final subtasks, require full progress
  if (subtask.checkpoint.includes("pass") && progress < 1) {
    return false;
  }

  // For intermediate subtasks, check progress threshold
  // Subtask 3 (test-and-iterate) needs at least 50% progress
  if (subtask.id === 3 && progress < 0.5) {
    return false;
  }

  return true;
}

/**
 * Create a fallback decomposition for unknown tasks.
 */
export function createFallbackDecomposition(task: TerminalBenchTask): TaskDecomposition {
  return {
    taskId: task.id,
    subtaskCount: 3,
    subtasks: [
      {
        id: 1,
        name: "understand",
        goal: "Read and understand the task requirements",
        checkpoint: "Task requirements are clear",
        expectedArtifacts: [],
        dependsOn: [],
        hints: ["Read the task description carefully", "Identify input and output files"],
        maxTurns: 3,
      },
      {
        id: 2,
        name: "implement",
        goal: "Implement the solution",
        checkpoint: "Solution file exists",
        expectedArtifacts: [],
        dependsOn: [1],
        hints: ["Write the solution code", "Use verify_progress to check progress"],
        maxTurns: 15,
      },
      {
        id: 3,
        name: "verify",
        goal: "Verify the solution passes all tests",
        checkpoint: "All tests pass",
        expectedArtifacts: [],
        dependsOn: [2],
        hints: ["Run verification", "Fix any remaining issues"],
        maxTurns: 10,
      },
    ],
    globalHints: [task.description.slice(0, 200)],
    filesToRead: [],
    requiredOutputs: [],
  };
}
