/**
 * Micro-Task Types for FM Supervisor-Worker Architecture
 *
 * FM has a ~200-250 char context limit, making multi-turn conversation impossible.
 * This module defines types for the micro-task architecture where:
 * - Orchestrator decomposes tasks into micro-steps
 * - Worker FM executes ONE action per call (single-turn, stateless)
 * - State lives entirely outside FM
 */

// --- Micro-Step Types ---

export type MicroStepKind =
  | "READ_FILE_RANGE"
  | "WRITE_FILE"
  | "EDIT_FILE"
  | "COMPILE"
  | "RUN_COMMAND"
  | "FIX_ERROR"
  | "CHECK_OUTPUT";

export type MicroStepStatus = "pending" | "in_progress" | "done" | "failed";

export interface MicroStep {
  id: number;
  kind: MicroStepKind;
  action: string;
  params: Record<string, unknown>;
  status: MicroStepStatus;
  resultSummary?: string;
  errorSummary?: string;
}

// --- Plan Types ---

export interface MicroPlan {
  taskId: string;
  steps: MicroStep[];
}

// --- State Types ---

export interface TaskState {
  plan: MicroPlan;
  currentStep: number;
  files: Map<string, string>;
  workspace: string;
  history: string[];
  startTime: number;
  totalTokens: number;
}

// --- Worker Types ---

export interface WorkerInput {
  action: string;
  context: string;
  previous: string;
}

export interface WorkerOutput {
  toolName: string;
  toolArgs: Record<string, unknown>;
  raw: string;
}

// --- Tool Result Types ---

export interface ToolResult {
  success: boolean;
  output: string;
  condensed: string;
}

// --- Constants ---

export const MAX_ACTION_CHARS = 40;
export const MAX_CONTEXT_CHARS = 30;
export const MAX_PREVIOUS_CHARS = 30;
export const MAX_RESULT_SUMMARY_CHARS = 50;
export const MAX_ERROR_SUMMARY_CHARS = 50;

// --- Helpers ---

export function truncate(s: string, max: number): string {
  if (s.length <= max) return s;
  return s.slice(0, max - 3) + "...";
}

export function createMicroStep(
  id: number,
  kind: MicroStepKind,
  action: string,
  params: Record<string, unknown> = {},
): MicroStep {
  return {
    id,
    kind,
    action: truncate(action, MAX_ACTION_CHARS),
    params,
    status: "pending",
  };
}

export function condenseSummary(output: string, maxChars: number = MAX_RESULT_SUMMARY_CHARS): string {
  const firstLine = output.split("\n")[0] ?? output;
  const clean = firstLine.replace(/Working directory:.*$/i, "").trim();
  return truncate(clean || output.slice(0, maxChars), maxChars);
}

export function condenseError(error: string, maxChars: number = MAX_ERROR_SUMMARY_CHARS): string {
  const match = error.match(/error:\s*(.+?)(?:\n|$)/i);
  if (match) {
    return truncate(match[1].trim(), maxChars);
  }
  const firstLine = error.split("\n")[0] ?? error;
  return truncate(firstLine.trim(), maxChars);
}
