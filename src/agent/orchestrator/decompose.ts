/**
 * Subtask Decomposition
 * 
 * Breaks tasks into implementable subtasks to prevent "one-shot" failures.
 * Uses heuristics and optionally LLM for complex task decomposition.
 */
import { Effect } from "effect";
import * as fs from "node:fs";
import * as path from "node:path";
import type { Task } from "../../tasks/index.js";
import {
  type Subtask,
  type SubtaskList,
  getSubtasksPath,
} from "./types.js";

/**
 * Generate a unique subtask ID
 */
export const generateSubtaskId = (taskId: string, index: number): string =>
  `${taskId}-sub-${String(index + 1).padStart(3, "0")}`;

/**
 * Heuristics for detecting if a task needs decomposition
 */
export interface DecompositionHeuristics {
  /** Task mentions multiple files/components */
  hasMultipleTargets: boolean;
  /** Task has multiple distinct actions (add X, update Y, test Z) */
  hasMultipleActions: boolean;
  /** Task description is long (>500 chars) */
  isComplex: boolean;
  /** Task mentions testing explicitly */
  requiresTesting: boolean;
  /** Task mentions documentation */
  requiresDocs: boolean;
}

/**
 * Analyze a task to determine decomposition needs
 */
export const analyzeTask = (task: Task): DecompositionHeuristics => {
  const text = `${task.title} ${task.description || ""}`.toLowerCase();
  
  // Check for multiple file/component mentions
  const filePatterns = /\b(file|component|module|service|class|function|test|spec)\b/gi;
  const fileMatches = text.match(filePatterns) || [];
  const hasMultipleTargets = fileMatches.length > 2;
  
  // Check for multiple action words
  const actionPatterns = /\b(add|create|update|modify|fix|remove|delete|implement|refactor|test|document)\b/gi;
  const actionMatches = text.match(actionPatterns) || [];
  const uniqueActions = new Set(actionMatches.map(a => a.toLowerCase()));
  const hasMultipleActions = uniqueActions.size > 2;
  
  // Check complexity by length
  const isComplex = (task.description?.length || 0) > 500;
  
  // Check for explicit testing requirement (match plurals too)
  const requiresTesting = /\b(tests?|specs?|coverage|verify|validate|unit test|e2e)\b/i.test(text);
  
  // Check for documentation requirement (match plurals and variations)
  const requiresDocs = /\b(docs?|documentation|readme|comments?|jsdoc|tsdoc)\b/i.test(text);
  
  return {
    hasMultipleTargets,
    hasMultipleActions,
    isComplex,
    requiresTesting,
    requiresDocs,
  };
};

/**
 * Simple rule-based decomposition for common patterns
 */
export const decomposeByRules = (task: Task): Subtask[] => {
  const heuristics = analyzeTask(task);
  const subtasks: Subtask[] = [];
  let index = 0;
  
  // If task is simple, just create one subtask
  if (!heuristics.hasMultipleActions && !heuristics.isComplex && !heuristics.hasMultipleTargets) {
    return [
      {
        id: generateSubtaskId(task.id, 0),
        description: `${task.title}\n\n${task.description || ""}`.trim(),
        status: "pending",
      },
    ];
  }
  
  // For complex tasks, break into logical phases
  
  // Phase 1: Implementation
  subtasks.push({
    id: generateSubtaskId(task.id, index++),
    description: `Implement: ${task.title}\n\n${task.description || ""}`,
    status: "pending",
  });
  
  // Phase 2: Testing (if not explicitly part of implementation)
  if (heuristics.requiresTesting && !task.title.toLowerCase().includes("test")) {
    subtasks.push({
      id: generateSubtaskId(task.id, index++),
      description: `Add tests for: ${task.title}\n\nVerify the implementation works correctly with unit tests.`,
      status: "pending",
    });
  }
  
  // Phase 3: Documentation (if required)
  if (heuristics.requiresDocs) {
    subtasks.push({
      id: generateSubtaskId(task.id, index++),
      description: `Document: ${task.title}\n\nAdd appropriate documentation/comments.`,
      status: "pending",
    });
  }
  
  return subtasks;
};

/**
 * Decompose a task into subtasks.
 * Uses heuristics for simple decomposition.
 * 
 * @param task - The task to decompose
 * @param options - Decomposition options
 * @returns Array of subtasks
 */
export const decomposeTask = (
  task: Task,
  options?: {
    /** Maximum number of subtasks to create */
    maxSubtasks?: number;
    /** Force single subtask (no decomposition) */
    forceSingle?: boolean;
  }
): Subtask[] => {
  const { maxSubtasks = 5, forceSingle = false } = options || {};
  
  if (forceSingle) {
    return [
      {
        id: generateSubtaskId(task.id, 0),
        description: `${task.title}\n\n${task.description || ""}`.trim(),
        status: "pending",
      },
    ];
  }
  
  const subtasks = decomposeByRules(task);
  
  // Limit to maxSubtasks
  return subtasks.slice(0, maxSubtasks);
};

/**
 * Read subtasks file for a task
 */
export const readSubtasks = (openagentsDir: string, taskId: string): SubtaskList | null => {
  const subtasksPath = getSubtasksPath(openagentsDir, taskId);
  if (!fs.existsSync(subtasksPath)) return null;
  
  try {
    const content = fs.readFileSync(subtasksPath, "utf-8");
    return JSON.parse(content) as SubtaskList;
  } catch {
    return null;
  }
};

/**
 * Write subtasks file for a task
 */
export const writeSubtasks = (openagentsDir: string, subtaskList: SubtaskList): void => {
  const subtasksDir = path.join(openagentsDir, "subtasks");
  if (!fs.existsSync(subtasksDir)) {
    fs.mkdirSync(subtasksDir, { recursive: true });
  }
  
  const subtasksPath = getSubtasksPath(openagentsDir, subtaskList.taskId);
  fs.writeFileSync(subtasksPath, JSON.stringify(subtaskList, null, 2));
};

/**
 * Update a subtask's status
 */
export const updateSubtaskStatus = (
  openagentsDir: string,
  taskId: string,
  subtaskId: string,
  status: Subtask["status"],
  error?: string
): SubtaskList | null => {
  const subtaskList = readSubtasks(openagentsDir, taskId);
  if (!subtaskList) return null;
  
  const subtask = subtaskList.subtasks.find(s => s.id === subtaskId);
  if (!subtask) return null;
  
  subtask.status = status;
  subtask.error = error;
  
  if (status === "in_progress") {
    subtask.startedAt = new Date().toISOString();
  } else if (status === "done") {
    subtask.completedAt = new Date().toISOString();
  } else if (status === "verified") {
    subtask.verifiedAt = new Date().toISOString();
  }
  
  subtaskList.updatedAt = new Date().toISOString();
  writeSubtasks(openagentsDir, subtaskList);
  
  return subtaskList;
};

/**
 * Create a new subtask list for a task
 */
export const createSubtaskList = (
  task: Task,
  options?: {
    maxSubtasks?: number;
    forceSingle?: boolean;
  }
): SubtaskList => {
  const now = new Date().toISOString();
  const subtasks = decomposeTask(task, options);
  
  return {
    taskId: task.id,
    taskTitle: task.title,
    subtasks,
    createdAt: now,
    updatedAt: now,
  };
};

/**
 * Get pending subtasks from a list
 */
export const getPendingSubtasks = (subtaskList: SubtaskList): Subtask[] =>
  subtaskList.subtasks.filter(s => s.status === "pending");

/**
 * Get the next subtask to work on
 */
export const getNextSubtask = (subtaskList: SubtaskList): Subtask | null => {
  // First, check for in_progress subtasks (resume)
  const inProgress = subtaskList.subtasks.find(s => s.status === "in_progress");
  if (inProgress) return inProgress;
  
  // Then, get first pending subtask
  const pending = subtaskList.subtasks.find(s => s.status === "pending");
  return pending || null;
};

/**
 * Check if all subtasks are complete
 */
export const isAllSubtasksComplete = (subtaskList: SubtaskList): boolean =>
  subtaskList.subtasks.every(s => s.status === "done" || s.status === "verified");

/**
 * Check if any subtask failed
 */
export const hasFailedSubtasks = (subtaskList: SubtaskList): boolean =>
  subtaskList.subtasks.some(s => s.status === "failed");
