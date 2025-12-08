/**
 * HillClimber Module
 *
 * Overnight optimization loop for Terminal-Bench tasks.
 * Uses Apple FM for task execution and OpenRouter for meta-reasoning.
 */

// Types
export * from "./types.js";

// Store
export {
  HillClimberStore,
  HillClimberStoreError,
  HillClimberStoreLive,
  makeHillClimberStoreLive,
  hashConfig,
} from "./store.js";

// Scoring
export {
  scoreResult,
  isBetterScore,
  isBetterResult,
  formatScore,
  formatRunSummary,
  isStableForExport,
  PASS_BONUS,
  TURN_BASE,
  MIN_SCORE,
  EXPORT_THRESHOLD,
  MIN_CONSECUTIVE_PASSES,
} from "./scoring.js";

// Executor
export {
  runTask,
  getAvailableTasks,
  getTask,
  getEasyTasks,
  type ExecutorOptions,
  type ExecutionResult,
} from "./executor.js";

// Meta-Reasoner
export {
  proposeConfigChange,
  proposeHeuristicChange,
  applyConfigChange,
  FREE_MODELS,
  BLOCKLIST,
  AUTO_MODEL,
  AUTO_MODEL_FREQUENCY,
} from "./meta-reasoner.js";

// Exporter
export {
  exportHints,
  exportTaskHint,
  getExportableHints,
  loadLearnedHints,
  getLearnedHint,
  generateHintsCode,
  runExport,
  LEARNED_HINTS_PATH,
  type LearnedHint,
  type LearnedHintsExport,
} from "./exporter.js";

// Runner
export {
  runHillClimber,
  showStats,
  dryRun,
  type RunnerState,
} from "./runner.js";
