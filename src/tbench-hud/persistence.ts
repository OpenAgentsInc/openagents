/**
 * Terminal-Bench Run Persistence
 *
 * Saves and loads TB run data to/from .openagents/tb-runs/ directory.
 * Each run is stored as a JSON file with metadata + ATIF trajectory.
 *
 * @module tbench-hud/persistence
 */

import { join } from "path";
import { readdirSync, existsSync, mkdirSync } from "fs";
import type { Trajectory } from "../atif/schema.js";
import type { TBTaskOutcome, TBDifficulty } from "../hud/protocol.js";
import type { TerminalBenchResults, TerminalBenchSuite } from "../bench/terminal-bench.js";

// ============================================================================
// Types
// ============================================================================

/**
 * Metadata for a TB run (quick-loadable header)
 */
export interface TBRunMeta {
  readonly runId: string;
  readonly suiteName: string;
  readonly suiteVersion: string;
  readonly timestamp: string;
  readonly passRate: number;
  readonly passed: number;
  readonly failed: number;
  readonly timeout: number;
  readonly error: number;
  readonly totalDurationMs: number;
  readonly totalTokens: number;
  readonly taskCount: number;
}

/**
 * Task result within a TB run
 */
export interface TBTaskResult {
  readonly id: string;
  readonly name: string;
  readonly category: string;
  readonly difficulty: TBDifficulty;
  readonly outcome: TBTaskOutcome;
  readonly durationMs: number;
  readonly turns: number;
  readonly tokens: number;
  readonly outputLines?: number;
}

/**
 * Complete TB run file structure
 */
export interface TBRunFile {
  /** Metadata header for quick loading */
  readonly meta: TBRunMeta;
  /** Per-task results */
  readonly tasks: readonly TBTaskResult[];
  /** Full ATIF trajectory for detailed analysis (optional) */
  readonly trajectory?: Trajectory;
}

/**
 * Loaded run with file path
 */
export interface TBRunWithPath extends TBRunMeta {
  readonly filepath: string;
}

// ============================================================================
// Constants
// ============================================================================

/** Default directory for TB run storage */
export const DEFAULT_TB_RUNS_DIR = ".openagents/tb-runs";

// ============================================================================
// Persistence Functions
// ============================================================================

/**
 * Generate a filename for a TB run.
 *
 * Format: YYYYMMDD-tb-HHMMSS-<shortId>.json
 *
 * @param runId - The run ID
 * @param timestamp - ISO timestamp (defaults to now)
 * @returns Filename string
 */
export const generateRunFilename = (
  runId: string,
  timestamp = new Date().toISOString()
): string => {
  const date = timestamp.slice(0, 10).replace(/-/g, "");
  const time = timestamp.slice(11, 19).replace(/:/g, "");
  const shortId = runId.slice(-6);
  return `${date}-tb-${time}-${shortId}.json`;
};

/**
 * Ensure the TB runs directory exists.
 *
 * @param baseDir - Base directory (defaults to DEFAULT_TB_RUNS_DIR)
 */
export const ensureRunsDir = (baseDir = DEFAULT_TB_RUNS_DIR): void => {
  if (!existsSync(baseDir)) {
    mkdirSync(baseDir, { recursive: true });
  }
};

/**
 * Save a TB run to disk.
 *
 * @param run - The run file to save
 * @param baseDir - Base directory (defaults to DEFAULT_TB_RUNS_DIR)
 * @returns Full path to saved file
 */
export const saveTBRun = async (
  run: TBRunFile,
  baseDir = DEFAULT_TB_RUNS_DIR
): Promise<string> => {
  ensureRunsDir(baseDir);

  const filename = generateRunFilename(run.meta.runId, run.meta.timestamp);
  const filepath = join(baseDir, filename);

  await Bun.file(filepath).write(JSON.stringify(run, null, 2));
  return filepath;
};

/**
 * Load a single TB run file.
 *
 * @param filepath - Path to the run file
 * @returns Parsed run file
 */
export const loadTBRun = async (filepath: string): Promise<TBRunFile> => {
  const content = await Bun.file(filepath).json();
  return content as TBRunFile;
};

/**
 * Load only metadata for a TB run (without trajectory).
 *
 * @param filepath - Path to the run file
 * @returns Run metadata with filepath
 */
export const loadTBRunMeta = async (filepath: string): Promise<TBRunWithPath> => {
  const content = await Bun.file(filepath).json();
  return { ...(content as TBRunFile).meta, filepath };
};

/**
 * List all TB runs in the directory (metadata only).
 *
 * @param baseDir - Base directory (defaults to DEFAULT_TB_RUNS_DIR)
 * @returns Array of run metadata sorted by timestamp (newest first)
 */
export const listTBRuns = async (
  baseDir = DEFAULT_TB_RUNS_DIR
): Promise<TBRunWithPath[]> => {
  ensureRunsDir(baseDir);

  const files = readdirSync(baseDir)
    .filter((f) => f.endsWith(".json") && f.includes("-tb-"));

  const runs = await Promise.all(
    files.map((f) => loadTBRunMeta(join(baseDir, f)))
  );

  // Sort by timestamp (newest first)
  return runs.sort((a, b) =>
    new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
  );
};

/**
 * Load the most recent N runs.
 *
 * @param count - Number of runs to load
 * @param baseDir - Base directory
 * @returns Array of run metadata
 */
export const loadRecentRuns = async (
  count: number,
  baseDir = DEFAULT_TB_RUNS_DIR
): Promise<TBRunWithPath[]> => {
  const runs = await listTBRuns(baseDir);
  return runs.slice(0, count);
};

/**
 * Delete a TB run file.
 *
 * @param filepath - Path to the run file
 */
export const deleteTBRun = async (filepath: string): Promise<void> => {
  await Bun.file(filepath).unlink?.();
};

/**
 * Get run by ID (searches all runs).
 *
 * @param runId - The run ID to find
 * @param baseDir - Base directory
 * @returns Run file or null if not found
 */
export const getTBRunById = async (
  runId: string,
  baseDir = DEFAULT_TB_RUNS_DIR
): Promise<TBRunFile | null> => {
  const runs = await listTBRuns(baseDir);
  const match = runs.find((r) => r.runId === runId);
  if (!match) return null;
  return loadTBRun(match.filepath);
};

// ============================================================================
// Builder Helpers
// ============================================================================

/**
 * Create a TBRunFile from run state.
 *
 * @param meta - Run metadata
 * @param tasks - Task results
 * @param trajectory - Optional ATIF trajectory
 * @returns Complete run file
 */
export const buildTBRunFile = (
  meta: TBRunMeta,
  tasks: TBTaskResult[],
  trajectory?: Trajectory
): TBRunFile => ({
  meta,
  tasks,
  ...(trajectory !== undefined ? { trajectory } : {}),
});

/**
 * Create run metadata from run summary.
 */
export const buildTBRunMeta = (params: {
  runId: string;
  suiteName: string;
  suiteVersion: string;
  timestamp?: string;
  passRate: number;
  passed: number;
  failed: number;
  timeout: number;
  error: number;
  totalDurationMs: number;
  totalTokens: number;
  taskCount: number;
}): TBRunMeta => ({
  runId: params.runId,
  suiteName: params.suiteName,
  suiteVersion: params.suiteVersion,
  timestamp: params.timestamp ?? new Date().toISOString(),
  passRate: params.passRate,
  passed: params.passed,
  failed: params.failed,
  timeout: params.timeout,
  error: params.error,
  totalDurationMs: params.totalDurationMs,
  totalTokens: params.totalTokens,
  taskCount: params.taskCount,
});

/**
 * Map TerminalBenchResult status to TBTaskOutcome.
 */
const mapStatusToOutcome = (
  status: "pass" | "fail" | "timeout" | "error" | "skip"
): TBTaskOutcome => {
  switch (status) {
    case "pass":
      return "success";
    case "fail":
      return "failure";
    case "timeout":
      return "timeout";
    case "error":
      return "error";
    case "skip":
      return "error"; // Skip treated as error
    default:
      return "error";
  }
};

/**
 * Convert TerminalBenchResults to TBRunFile format.
 *
 * This function converts the results.json format produced by tbench-local.ts
 * into the TBRunFile format expected by the UI run browser.
 *
 * @param results - TerminalBenchResults from tbench-local.ts
 * @param runId - Run ID (from --run-id flag or generated)
 * @param suite - Suite metadata (full suite object for task metadata lookup)
 * @param trajectory - Optional ATIF trajectory for detailed analysis
 * @returns TBRunFile ready for saveTBRun()
 */
export const convertResultsToTBRunFile = (
  results: TerminalBenchResults,
  runId: string,
  suite: TerminalBenchSuite | { name: string; version: string },
  trajectory?: Trajectory
): TBRunFile => {
  // Create lookup map for task metadata if full suite provided
  const taskMap =
    "tasks" in suite && Array.isArray(suite.tasks)
      ? new Map(suite.tasks.map((t) => [t.id, t]))
      : new Map<string, { name: string; category: string; difficulty: string }>();

  // Calculate total duration from individual task durations
  const totalDurationMs = results.results.reduce((sum, r) => sum + r.duration_ms, 0);

  // Build metadata
  const meta = buildTBRunMeta({
    runId,
    suiteName: results.suite_name,
    suiteVersion: results.suite_version,
    timestamp: results.timestamp,
    passRate: results.summary.pass_rate,
    passed: results.summary.passed,
    failed: results.summary.failed,
    timeout: results.summary.timeout,
    error: results.summary.error,
    totalDurationMs,
    totalTokens: results.summary.total_tokens,
    taskCount: results.summary.total,
  });

  // Convert task results
  const tasks: TBTaskResult[] = results.results.map((r) => {
    const taskDef = taskMap.get(r.task_id);
    return {
      id: r.task_id,
      name: taskDef?.name ?? r.task_id,
      category: taskDef?.category ?? "",
      difficulty: (taskDef?.difficulty ?? "medium") as TBDifficulty,
      outcome: mapStatusToOutcome(r.status),
      durationMs: r.duration_ms,
      turns: r.turns,
      tokens: r.tokens_used,
    };
  });

  return buildTBRunFile(meta, tasks, trajectory);
};
