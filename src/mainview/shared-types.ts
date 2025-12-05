/**
 * Shared Types for Mainview Modules
 *
 * Common interfaces, types, and constants used across mainview UI modules.
 */

import type { ContainerStreamType } from "../hud/protocol.js"

// ============================================================================
// Constants
// ============================================================================

export const ZINC = {
  50: "#fafafa",
  100: "#f4f4f5",
  200: "#e4e4e7",
  300: "#d4d4d8",
  400: "#a1a1aa",
  500: "#71717a",
  600: "#52525b",
  700: "#3f3f46",
  800: "#27272a",
  900: "#18181b",
  950: "#09090b",
}

// ============================================================================
// View Mode
// ============================================================================

export type ViewMode = "flow" | "tbench"

// ============================================================================
// MechaCoder Tasks
// ============================================================================

export interface MCTaskState {
  id: string
  title: string
  description: string
  status: string
  priority: number
  type: string
  labels: string[]
  createdAt: string
  updatedAt: string
}

// ============================================================================
// APM Widget
// ============================================================================

export interface APMState {
  sessionAPM: number
  recentAPM: number
  totalActions: number
  durationMinutes: number
  // Historical snapshot data
  apm1h: number
  apm6h: number
  apm1d: number
  apmLifetime: number
  claudeCodeAPM: number
  mechaCoderAPM: number
  efficiencyRatio: number
}

// ============================================================================
// Terminal-Bench
// ============================================================================

export type TBTaskStatus = "pending" | "running" | "passed" | "failed" | "timeout" | "error"

export interface TBTaskState {
  id: string
  name: string
  difficulty: string
  category: string
  status: TBTaskStatus
  durationMs?: number
  turns?: number
}

export interface TBOutputLine {
  text: string
  source: "agent" | "verification" | "system"
  timestamp: number
}

/**
 * Comparison data between current run and a baseline
 */
export interface TBComparison {
  baselineRunId: string
  baselineSuiteName: string
  baselineTimestamp: string
  baselinePassRate: number
  baselinePassed: number
  baselineFailed: number
  baselineTotalDurationMs: number
  // Deltas (positive = improvement for pass rate, negative = faster for duration)
  passRateDelta: number
  passedDelta: number
  failedDelta: number
  durationDelta: number
  // Task-level changes
  improved: string[]    // task IDs that went from fail->pass
  regressed: string[]   // task IDs that went from pass->fail
  unchanged: string[]   // task IDs with same outcome
}

export interface TBState {
  isRunning: boolean
  runId: string | null
  suiteName: string
  suiteVersion: string
  totalTasks: number
  tasks: Map<string, TBTaskState>
  currentTaskId: string | null
  currentPhase: string | null
  currentTurn: number
  passed: number
  failed: number
  timeout: number
  error: number
  passRate: number
  totalDurationMs: number
  // Output streaming
  outputBuffer: TBOutputLine[]
  maxOutputLines: number
  // Comparison with baseline
  comparison: TBComparison | null
  baselineRunId: string | null
}

// ============================================================================
// Container Panes
// ============================================================================

export interface ContainerOutputLine {
  text: string
  stream: ContainerStreamType
  sequence: number
}

export interface ContainerPane {
  executionId: string
  image: string
  command: string[]
  context: string
  sandboxed: boolean
  workdir: string
  status: "running" | "completed" | "error"
  exitCode?: number
  durationMs?: number
  outputLines: ContainerOutputLine[]
  startedAt: string
}

export const MAX_LINES_PER_PANE = 500
export const MAX_VISIBLE_PANES = 10

// ============================================================================
// RPC Schema (must match backend)
// ============================================================================

export interface TBRunOptions {
  suitePath: string
  taskIds: string[]
  timeout: number
  maxTurns: number
}

export interface TBSuiteInfo {
  name: string
  version: string
  tasks: Array<{
    id: string
    name: string
    difficulty: string
    category: string
  }>
}

export interface TBTaskInfo {
  id: string
  name: string
  difficulty: string
  category: string
}

// ============================================================================
// Category Data
// ============================================================================

export interface CategoryData {
  category: string
  tasks: TBTaskInfo[]
  passed: number
  failed: number
  pending: number
  total: number
}
