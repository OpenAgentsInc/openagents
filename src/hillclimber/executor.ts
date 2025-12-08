/**
 * HillClimber Executor
 *
 * Wraps the FM (Foundation Models) task runner for HillClimber execution.
 * Runs single TB tasks with custom configs and extracts structured results.
 */

import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { createModelRunner } from "../bench/model-adapter.js";
import {
  loadTerminalBenchSuite,
  runTaskSetup,
  runTaskVerification,
  type TerminalBenchTask,
} from "../bench/terminal-bench.js";
import type { HillClimberConfig, TaskRunResult } from "./types.js";
import { scoreResult } from "./scoring.js";

// ============================================================================
// Types
// ============================================================================

export interface ExecutorOptions {
  /** Path to Terminal-Bench suite JSON */
  suitePath: string;
  /** Base workspace directory for task execution */
  workspaceBase: string;
  /** Timeout in seconds per task */
  timeout: number;
  /** Callback for output streaming */
  onOutput?: (text: string) => void;
}

export interface ExecutionResult {
  success: boolean;
  result: TaskRunResult;
  score: number;
  task: TerminalBenchTask;
}

// ============================================================================
// Suite Cache
// ============================================================================

let cachedSuite: { tasks: readonly TerminalBenchTask[] } | null = null;
let cachedSuitePath: string | null = null;

/**
 * Load TB suite with caching to avoid repeated file reads
 */
const loadSuite = async (
  suitePath: string,
): Promise<{ tasks: readonly TerminalBenchTask[] }> => {
  if (cachedSuite && cachedSuitePath === suitePath) {
    return cachedSuite;
  }

  const suite = await Effect.runPromise(
    loadTerminalBenchSuite(suitePath).pipe(Effect.provide(BunContext.layer)),
  );

  cachedSuite = suite;
  cachedSuitePath = suitePath;
  return suite;
};

// ============================================================================
// Task Execution
// ============================================================================

/**
 * Run a single Terminal-Bench task with FM and the specified config.
 *
 * @param taskId The TB task ID to run
 * @param config The HillClimber config to apply (hint, skills, turns)
 * @param options Executor options
 * @returns Execution result with structured data
 */
export const runTask = async (
  taskId: string,
  config: HillClimberConfig,
  options: ExecutorOptions,
): Promise<ExecutionResult> => {
  const log = options.onOutput ?? console.log;

  // Load suite and find task
  const suite = await loadSuite(options.suitePath);
  const task = suite.tasks.find((t) => t.id === taskId);

  if (!task) {
    const errorResult: TaskRunResult = {
      passed: false,
      turns: 0,
      durationMs: 0,
      stepSummary: [],
      errorMessage: `Task not found: ${taskId}`,
      output: "",
    };
    return {
      success: false,
      result: errorResult,
      score: scoreResult(false, 0),
      task: { id: taskId, description: "", difficulty: "medium" } as TerminalBenchTask,
    };
  }

  // Create workspace for this run
  const fs = require("node:fs");
  const path = require("node:path");
  const workspace = path.join(options.workspaceBase, taskId);
  fs.mkdirSync(workspace, { recursive: true });

  // Modify task with config overrides
  const modifiedTask: TerminalBenchTask = {
    ...task,
    max_turns: config.maxTurnsOverride,
    // Inject hint into description if present
    description: config.hint
      ? `${task.description}\n\nHint: ${config.hint}`
      : task.description,
  };

  log(`[Executor] Running task: ${taskId}`);
  log(`[Executor] Config: hint=${config.hint ? "yes" : "no"}, skills=${config.useSkills}, maxTurns=${config.maxTurnsOverride}`);

  // Create FM runner
  const runner = createModelRunner({
    type: "foundation-models",
    useMicroTask: true,
    useSkills: config.useSkills,
  });

  // Check FM health
  const health = await runner.checkHealth();
  if (!health.available) {
    log(`[Executor] FM not available: ${health.error}`);
    const errorResult: TaskRunResult = {
      passed: false,
      turns: 0,
      durationMs: 0,
      stepSummary: [],
      errorMessage: `FM not available: ${health.error}`,
      output: "",
    };
    return {
      success: false,
      result: errorResult,
      score: scoreResult(false, 0),
      task: modifiedTask,
    };
  }

  // Run task setup if present
  if (task.setup && task.setup.length > 0) {
    log(`[Executor] Running setup commands...`);
    try {
      // Change to workspace directory before running setup
      const originalCwd = process.cwd();
      process.chdir(workspace);
      try {
        await Effect.runPromise(
          runTaskSetup(task).pipe(Effect.provide(BunContext.layer)),
        );
      } finally {
        process.chdir(originalCwd);
      }
    } catch (e) {
      log(`[Executor] Setup failed: ${e}`);
      // Continue anyway - some setups are optional
    }
  }

  // Execute task
  const startTime = Date.now();
  let output = "";
  let stepSummary: string[] = [];

  try {
    const result = await runner.runTask(modifiedTask, {
      workspace,
      timeout: options.timeout,
      maxTurns: config.maxTurnsOverride,
      suitePath: options.suitePath,
      onOutput: (text) => {
        output += text;
        options.onOutput?.(text);

        // Extract step summaries from output
        // Look for lines like "[FM] Turn X: tool_name - result"
        const turnMatch = text.match(/\[FM\] Turn (\d+): (\S+).*?-\s*(.+)/);
        if (turnMatch) {
          stepSummary.push(`Turn ${turnMatch[1]}: ${turnMatch[2]} - ${turnMatch[3].trim()}`);
          // Keep only last 3 summaries
          if (stepSummary.length > 3) {
            stepSummary = stepSummary.slice(-3);
          }
        }
      },
    });

    const durationMs = Date.now() - startTime;

    // Run verification
    let verificationPassed = result.success;
    if (task.verification) {
      log(`[Executor] Running verification...`);
      try {
        // Change to workspace directory before running verification
        const originalCwd = process.cwd();
        process.chdir(workspace);
        try {
          const verifyResult = await Effect.runPromise(
            runTaskVerification(task).pipe(
              Effect.provide(BunContext.layer),
            ),
          );
          verificationPassed = verifyResult.passed;
          log(`[Executor] Verification: ${verificationPassed ? "PASSED" : "FAILED"}`);
        } finally {
          process.chdir(originalCwd);
        }
      } catch (e) {
        log(`[Executor] Verification error: ${e}`);
        verificationPassed = false;
      }
    }

    const taskResult: TaskRunResult = {
      passed: verificationPassed,
      turns: result.turns,
      durationMs,
      stepSummary,
      errorMessage: result.error ?? null,
      output,
    };

    return {
      success: true,
      result: taskResult,
      score: scoreResult(verificationPassed, result.turns),
      task: modifiedTask,
    };
  } catch (e) {
    const durationMs = Date.now() - startTime;
    const errorMsg = e instanceof Error ? e.message : String(e);

    log(`[Executor] Task execution failed: ${errorMsg}`);

    const taskResult: TaskRunResult = {
      passed: false,
      turns: 0,
      durationMs,
      stepSummary,
      errorMessage: errorMsg,
      output,
    };

    return {
      success: false,
      result: taskResult,
      score: scoreResult(false, 0),
      task: modifiedTask,
    };
  }
};

// ============================================================================
// Task Discovery
// ============================================================================

/**
 * Get list of available task IDs from the TB suite.
 */
export const getAvailableTasks = async (
  suitePath: string,
): Promise<string[]> => {
  const suite = await loadSuite(suitePath);
  return suite.tasks.map((t) => t.id);
};

/**
 * Get task by ID from the TB suite.
 */
export const getTask = async (
  suitePath: string,
  taskId: string,
): Promise<TerminalBenchTask | null> => {
  const suite = await loadSuite(suitePath);
  return suite.tasks.find((t) => t.id === taskId) ?? null;
};

/**
 * Get easy tasks from the TB suite (difficulty <= "medium").
 */
export const getEasyTasks = async (suitePath: string): Promise<string[]> => {
  const suite = await loadSuite(suitePath);
  return suite.tasks
    .filter((t) => t.difficulty === "easy" || t.difficulty === "medium")
    .map((t) => t.id);
};
