/**
 * TB2 Structural Skills
 *
 * ============================================================================
 * GUARDRAIL: NO TASK-SPECIFIC HARDCODING
 *
 * This file must NEVER contain:
 * - Task IDs (e.g., "regex-log", "path-tracing")
 * - Task-specific patterns (e.g., IPv4 format, date format)
 * - Task-specific hints (e.g., "use lookahead for IPv4")
 * - Task-specific file paths (e.g., "/app/regex.txt")
 *
 * All knowledge must come from:
 * 1. The task description (passed as parameter)
 * 2. General process knowledge (TDD, iteration)
 *
 * If you're tempted to add task-specific code, you're defeating the thesis:
 * "Architecture beats model size"
 * ============================================================================
 *
 * NOTE: All task-specific skills have been removed.
 * This file is kept for backward compatibility but returns empty arrays.
 * Future general-purpose skills (if any) should be added here.
 */

import { createSkill, type Skill } from "../schema.js";

// ============================================================================
// Skill Collection Export
// ============================================================================

/**
 * All TB2 structural skills.
 * Currently empty - all task-specific skills have been removed.
 */
export const TB2_SKILLS: Skill[] = [];

/**
 * Get skills relevant to a specific task.
 * Returns empty array - task-specific skills have been removed.
 */
export function getSkillsForTask(_taskId: string): Skill[] {
  // All task-specific skills removed to prove architecture beats model size
  return [];
}

/**
 * Format TB2 skills as hints for FM prompt.
 * Returns empty string - no task-specific skills available.
 */
export function formatTB2SkillsAsHints(_skills: Skill[]): string {
  return "";
}
