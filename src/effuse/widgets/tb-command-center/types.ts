/**
 * TB Command Center - Shared Types
 *
 * Domain types for the TerminalBench Command Center widgets.
 */

// ============================================================================
// Navigation
// ============================================================================

export type TabId = "dashboard" | "tasks" | "runs" | "settings"

export interface TabConfig {
  id: TabId
  label: string
  icon: string // Lucide icon name or emoji
}

export const TABS: TabConfig[] = [
  { id: "dashboard", label: "Dashboard", icon: "layout-dashboard" },
  { id: "tasks", label: "Tasks", icon: "list-checks" },
  { id: "runs", label: "Runs", icon: "play-circle" },
  { id: "settings", label: "Settings", icon: "settings" },
]

// ============================================================================
// Task Types
// ============================================================================

export type TBDifficulty = "easy" | "medium" | "hard" | "expert" | "unknown"
export type TBTaskStatus = "unattempted" | "passed" | "failed" | "in_progress"

export interface TBTask {
  id: string
  name: string
  slug: string
  description: string
  difficulty: TBDifficulty
  category: string
  tags: string[]
  timeoutSeconds: number
  maxTurns: number
  status: TBTaskStatus
  lastRunId: string | null
  attemptCount: number
  passCount: number
}

export interface TBTaskMetadata {
  containerImage?: string
  allowedTools?: string[]
  verificationCommand?: string
  sourcePath?: string
}

// ============================================================================
// Run Types
// ============================================================================

export type TBRunOutcome = "success" | "failure" | "timeout" | "error" | "aborted"
export type TBRunStatus = "queued" | "running" | "completed" | "error"
export type TBRunSource = "local" | "hf" // Local TB runs vs HuggingFace trajectories

export interface TBRunSummary {
  id: string
  source: TBRunSource
  taskId: string
  taskName: string
  outcome: TBRunOutcome | null // null if still running
  status: TBRunStatus
  startedAt: string // ISO timestamp
  finishedAt: string | null
  durationMs: number | null
  stepsCount: number
  tokensUsed: number | null
  // HF-specific
  agentName?: string
  modelName?: string
  episode?: string
}

export interface TBRunDetail extends TBRunSummary {
  steps: TBRunStep[]
  terminalOutput: {
    stdout: string[]
    stderr: string[]
  }
}

// ============================================================================
// Step Types
// ============================================================================

export type TBActionType =
  | "RUN_COMMAND"
  | "WRITE_FILE"
  | "READ_FILE"
  | "EDIT_FILE"
  | "ASSERT_TEST"
  | "THOUGHT"
  | "CUSTOM"

export interface TBRunStep {
  id: string
  index: number
  actionType: TBActionType
  actionLabel: string // e.g., "Run: make test"
  shortReason: string // 1-line summary
  details: string | null
  timestamp: string
  success: boolean
  durationMs: number | null
  toolCall?: {
    functionName: string
    arguments: unknown
  }
  observation?: {
    content: unknown
    truncated: boolean
  }
  diff?: {
    filePath: string
    before: string
    after: string
  }[]
}

// ============================================================================
// Dashboard Types
// ============================================================================

export interface DashboardStats {
  overallSuccessRate: number // 0-1
  last50SuccessRate: number // 0-1
  avgStepsPerRun: number
  avgDurationSeconds: number
  totalRuns: number
  byDifficulty: Record<TBDifficulty, { passed: number; total: number }>
}

export interface CurrentRunInfo {
  runId: string
  taskId: string
  taskName: string
  attempt: number
  maxAttempts: number
  status: "idle" | "running" | "completed" | "failed"
  startedAt: number // timestamp ms
  currentStep: number
  totalSteps: number | null
}

// ============================================================================
// Settings Types
// ============================================================================

/** Model options for TB execution */
export type TBModelOption = "fm" | "claude-code"

export interface ExecutionSettings {
  maxAttempts: number
  maxStepsPerRun: number
  timeoutSeconds: number
  deepComputeEnabled: boolean
  recursionLimitN: number
  innerIterationsT: number
  earlyStopOnHighConfidence: boolean
  /** Model to use: "fm" (Foundation Model - default) or "claude-code" */
  model: TBModelOption
}

export interface LoggingSettings {
  saveTrajectories: boolean
  saveTerminalOutput: boolean
  saveAtifTraces: boolean
  autoPruneDays: number | null
}

// ============================================================================
// Defaults
// ============================================================================

export const DEFAULT_EXECUTION_SETTINGS: ExecutionSettings = {
  maxAttempts: 5,
  maxStepsPerRun: 50,
  timeoutSeconds: 300,
  deepComputeEnabled: false,
  recursionLimitN: 3,
  innerIterationsT: 5,
  earlyStopOnHighConfidence: true,
  model: "fm", // Foundation Model is the default
}

export const DEFAULT_LOGGING_SETTINGS: LoggingSettings = {
  saveTrajectories: true,
  saveTerminalOutput: true,
  saveAtifTraces: true,
  autoPruneDays: 30,
}

// ============================================================================
// Utility Types
// ============================================================================

/** Difficulty color mapping */
export const DIFFICULTY_COLORS: Record<TBDifficulty, { bg: string; text: string; border: string }> = {
  easy: { bg: "bg-emerald-900/40", text: "text-emerald-300", border: "border-emerald-700/50" },
  medium: { bg: "bg-yellow-900/40", text: "text-yellow-300", border: "border-yellow-700/50" },
  hard: { bg: "bg-orange-900/40", text: "text-orange-300", border: "border-orange-700/50" },
  expert: { bg: "bg-red-900/40", text: "text-red-300", border: "border-red-700/50" },
  unknown: { bg: "bg-zinc-800/40", text: "text-zinc-400", border: "border-zinc-700/50" },
}

/** Outcome color mapping */
export const OUTCOME_COLORS: Record<TBRunOutcome, { bg: string; text: string; border: string }> = {
  success: { bg: "bg-emerald-900/40", text: "text-emerald-300", border: "border-emerald-700/50" },
  failure: { bg: "bg-red-900/40", text: "text-red-300", border: "border-red-700/50" },
  timeout: { bg: "bg-yellow-900/40", text: "text-yellow-300", border: "border-yellow-700/50" },
  error: { bg: "bg-red-900/40", text: "text-red-300", border: "border-red-700/50" },
  aborted: { bg: "bg-zinc-800/40", text: "text-zinc-400", border: "border-zinc-700/50" },
}

/** Status color mapping */
export const STATUS_COLORS: Record<TBRunStatus, { bg: string; text: string; border: string }> = {
  queued: { bg: "bg-zinc-800/40", text: "text-zinc-400", border: "border-zinc-700/50" },
  running: { bg: "bg-blue-900/40", text: "text-blue-300", border: "border-blue-700/50" },
  completed: { bg: "bg-emerald-900/40", text: "text-emerald-300", border: "border-emerald-700/50" },
  error: { bg: "bg-red-900/40", text: "text-red-300", border: "border-red-700/50" },
}
