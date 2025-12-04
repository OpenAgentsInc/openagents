/**
 * Terminal-Bench HUD Emit Helpers
 *
 * Helper functions for emitting Terminal-Bench events to the HUD.
 * Use createTBEmitter() to get an emitter object for sending TB events.
 *
 * @example
 * ```typescript
 * import { createTBEmitter } from "./tbench-hud/emit.js";
 *
 * const tbEmit = createTBEmitter();
 *
 * // Emit run start
 * tbEmit.runStart(suite, selectedTaskIds);
 *
 * // For each task
 * tbEmit.taskStart(task, index, total);
 * tbEmit.taskProgress(taskId, "agent", turn, elapsed);
 * tbEmit.taskOutput(taskId, text, "agent");
 * tbEmit.taskComplete(taskId, result);
 *
 * // Emit run complete
 * tbEmit.runComplete(summary);
 *
 * // Clean up
 * tbEmit.close();
 * ```
 */

import { HudClient, getHudClient, type HudClientOptions } from "../hud/client.js";
import type {
  TBRunStartMessage,
  TBRunCompleteMessage,
  TBTaskStartMessage,
  TBTaskProgressMessage,
  TBTaskOutputMessage,
  TBTaskCompleteMessage,
  TBSuiteInfoMessage,
  TBDifficulty,
  TBTaskPhase,
  TBOutputSource,
  TBTaskOutcome,
} from "../hud/protocol.js";

// ============================================================================
// Types
// ============================================================================

export interface TBSuiteInfo {
  name: string;
  version: string;
  tasks: Array<{
    id: string;
    name: string;
    category: string;
    difficulty: string;
  }>;
}

export interface TBTaskInfo {
  id: string;
  name: string;
  category: string;
  difficulty: string;
}

export interface TBTaskResult {
  outcome: TBTaskOutcome;
  durationMs: number;
  turns: number;
  tokens: number;
  verificationOutput?: string;
}

export interface TBRunSummary {
  passRate: number;
  passed: number;
  failed: number;
  timeout: number;
  error: number;
  totalDurationMs: number;
}

// ============================================================================
// ID Generation
// ============================================================================

const generateRunId = (): string => {
  const timestamp = new Date().toISOString().replace(/[-:T]/g, "").slice(0, 14);
  const random = Math.random().toString(36).slice(2, 8);
  return `tb-${timestamp}-${random}`;
};

// ============================================================================
// TB Emitter Factory
// ============================================================================

export interface TBEmitter {
  /** Current run ID (null if no run active) */
  runId: string | null;

  /** Emit suite info (for UI to display available tasks) */
  suiteInfo: (suite: TBSuiteInfo) => void;

  /** Emit run start event */
  runStart: (suite: TBSuiteInfo, taskIds: string[]) => string;

  /** Emit run complete event */
  runComplete: (summary: TBRunSummary) => void;

  /** Emit task start event */
  taskStart: (task: TBTaskInfo, taskIndex: number, totalTasks: number) => void;

  /** Emit task progress update */
  taskProgress: (taskId: string, phase: TBTaskPhase, turn?: number, elapsedMs?: number) => void;

  /** Emit task output (streaming text) */
  taskOutput: (taskId: string, text: string, source: TBOutputSource) => void;

  /** Emit task complete event */
  taskComplete: (taskId: string, result: TBTaskResult) => void;

  /** Get the underlying HUD client */
  getClient: () => HudClient;

  /** Close the HUD client connection */
  close: () => void;
}

/**
 * Create a TB emitter for sending Terminal-Bench events to the HUD.
 *
 * The emitter manages a run ID and provides typed methods for each event type.
 * Events are silently dropped if the HUD server is not running.
 *
 * @param clientOptions - Optional HudClient configuration
 * @returns TBEmitter instance
 */
export const createTBEmitter = (clientOptions?: HudClientOptions): TBEmitter => {
  const client = clientOptions ? new HudClient(clientOptions) : getHudClient();
  let runId: string | null = null;

  const suiteInfo = (suite: TBSuiteInfo): void => {
    const message: TBSuiteInfoMessage = {
      type: "tb_suite_info",
      name: suite.name,
      version: suite.version,
      tasks: suite.tasks.map((t) => ({
        id: t.id,
        name: t.name,
        category: t.category,
        difficulty: t.difficulty as TBDifficulty,
      })),
    };
    client.send(message);
  };

  const runStart = (suite: TBSuiteInfo, taskIds: string[]): string => {
    runId = generateRunId();
    const message: TBRunStartMessage = {
      type: "tb_run_start",
      runId,
      suiteName: suite.name,
      suiteVersion: suite.version,
      totalTasks: taskIds.length,
      taskIds,
      timestamp: new Date().toISOString(),
    };
    client.send(message);
    return runId;
  };

  const runComplete = (summary: TBRunSummary): void => {
    if (!runId) return;
    const message: TBRunCompleteMessage = {
      type: "tb_run_complete",
      runId,
      passRate: summary.passRate,
      passed: summary.passed,
      failed: summary.failed,
      timeout: summary.timeout,
      error: summary.error,
      totalDurationMs: summary.totalDurationMs,
    };
    client.send(message);
    runId = null;
  };

  const taskStart = (task: TBTaskInfo, taskIndex: number, totalTasks: number): void => {
    if (!runId) return;
    const message: TBTaskStartMessage = {
      type: "tb_task_start",
      runId,
      taskId: task.id,
      taskName: task.name,
      category: task.category,
      difficulty: task.difficulty as TBDifficulty,
      taskIndex,
      totalTasks,
    };
    client.send(message);
  };

  const taskProgress = (
    taskId: string,
    phase: TBTaskPhase,
    currentTurn?: number,
    elapsedMs = 0
  ): void => {
    if (!runId) return;
    const message: TBTaskProgressMessage = {
      type: "tb_task_progress",
      runId,
      taskId,
      phase,
      ...(currentTurn !== undefined ? { currentTurn } : {}),
      elapsedMs,
    };
    client.send(message);
  };

  const taskOutput = (taskId: string, text: string, source: TBOutputSource): void => {
    if (!runId) return;
    const message: TBTaskOutputMessage = {
      type: "tb_task_output",
      runId,
      taskId,
      text,
      source,
    };
    client.send(message);
  };

  const taskComplete = (taskId: string, result: TBTaskResult): void => {
    if (!runId) return;
    const message: TBTaskCompleteMessage = {
      type: "tb_task_complete",
      runId,
      taskId,
      outcome: result.outcome,
      durationMs: result.durationMs,
      turns: result.turns,
      tokens: result.tokens,
      ...(result.verificationOutput !== undefined ? { verificationOutput: result.verificationOutput } : {}),
    };
    client.send(message);
  };

  return {
    get runId() {
      return runId;
    },
    suiteInfo,
    runStart,
    runComplete,
    taskStart,
    taskProgress,
    taskOutput,
    taskComplete,
    getClient: () => client,
    close: () => client.close(),
  };
};

/**
 * Create a simple output callback for streaming task output to the HUD.
 *
 * Use this when you just need to stream output without full emitter control.
 *
 * @example
 * ```typescript
 * const onOutput = createTBOutputCallback(tbEmitter, taskId);
 * runClaudeCodeSubagent(subtask, { onOutput });
 * ```
 */
export const createTBOutputCallback = (
  emitter: TBEmitter,
  taskId: string,
  source: TBOutputSource = "agent"
): ((text: string) => void) => {
  return (text: string) => {
    emitter.taskOutput(taskId, text, source);
  };
};
