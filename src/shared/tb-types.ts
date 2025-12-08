/**
 * Shared TB Types
 *
 * Single source of truth for TB run options used by:
 * - Frontend (effuse/socket service)
 * - Socket client (mainview)
 * - Server protocol (desktop)
 * - Handler (desktop)
 */

/**
 * Options for starting a TB run
 */
export interface TBRunOptions {
  suitePath: string
  taskIds?: string[]
  timeout?: number
  maxTurns?: number
  outputDir?: string
  sandbox?: boolean
  sandboxBackend?: "docker" | "macos-container"
  sandboxImage?: string
  subset?: string
  runAll?: boolean
  random?: boolean
  /** Model to use: "fm" (Foundation Model), "claude-code", or ollama:<model> */
  model?: "fm" | "claude-code" | string
}

/**
 * Result from starting a TB run
 */
export interface TBRunResult {
  runId: string
}

/**
 * TB run history item (returned from loadRecentTBRuns)
 */
export interface TBRunHistoryItem {
  runId: string
  suiteName: string
  suiteVersion: string
  timestamp: string
  passRate: number
  passed: number
  failed: number
  timeout: number
  error: number
  taskCount: number
  totalDurationMs: number
  totalTokens: number
  filepath: string
}

/**
 * TB suite info (returned from loadTBSuite)
 */
export interface TBSuiteInfo {
  name: string
  version: string
  tasks: Array<{
    id: string
    name: string
    category: string
    difficulty: string
  }>
}

/**
 * Full TB run details (returned from loadTBRunDetails)
 */
export interface TBRunDetails {
  meta: TBRunHistoryItem
  tasks: Array<{
    id: string
    name: string
    category: string
    difficulty: string
    outcome: string
    durationMs: number
    turns: number
    tokens: number
    outputLines?: number
  }>
}
