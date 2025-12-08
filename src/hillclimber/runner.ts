/**
 * HillClimber Runner
 *
 * Main loop for the overnight hill-climbing optimization.
 * Runs tasks, proposes config changes, and tracks progress.
 */

import { Effect } from "effect";
import { HillClimberStore, HillClimberStoreLive } from "./store.js";
import { runTask, getAvailableTasks, getTask } from "./executor.js";
import {
  proposeConfigChange,
  proposeHeuristicChange,
  applyConfigChange,
} from "./meta-reasoner.js";
import { formatRunSummary, isBetterScore } from "./scoring.js";
import { exportTaskHint } from "./exporter.js";
import type { HillClimberOptions, HillClimberRunInput } from "./types.js";
import { generateRunId as genRunId } from "./types.js";
import { log, logError } from "./logger.js";

// ============================================================================
// Types
// ============================================================================

export interface RunnerState {
  totalRuns: number;
  taskIndex: number;
  running: boolean;
}

// ============================================================================
// Main Runner
// ============================================================================

/**
 * Run the HillClimber optimization loop.
 */
export const runHillClimber = async (
  options: HillClimberOptions,
): Promise<void> => {
  const state: RunnerState = {
    totalRuns: 0,
    taskIndex: 0,
    running: true,
  };

  // Handle graceful shutdown
  const shutdown = () => {
    log("\n[HillClimber] Shutting down gracefully...");
    state.running = false;
  };

  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);

  // Determine tasks to optimize
  let tasks = options.tasks;
  if (tasks.length === 0) {
    log("[HillClimber] No tasks specified, discovering from suite...");
    tasks = await getAvailableTasks(options.suitePath);
    // Filter to easy/medium tasks for initial runs
    const availableTasks = tasks.slice(0, 5); // Start with first 5 tasks
    log(
      `[HillClimber] Found ${tasks.length} tasks, using first ${availableTasks.length}`,
    );
    tasks = availableTasks;
  }

  if (tasks.length === 0) {
    logError("[HillClimber] No tasks to optimize!");
    return;
  }

  log(`[HillClimber] Starting optimization loop`);
  log(`[HillClimber] Tasks: ${tasks.join(", ")}`);
  log(`[HillClimber] Max runs: ${options.maxRuns}`);
  log(`[HillClimber] Sleep interval: ${options.sleepMs}ms`);

  // Create workspace base
  const fs = require("node:fs");
  const path = require("node:path");
  const workspaceBase = path.join(process.cwd(), ".hillclimber-workspaces");
  fs.mkdirSync(workspaceBase, { recursive: true });

  // Main loop
  while (state.running && state.totalRuns < options.maxRuns) {
    // Pick next task (round-robin)
    const taskId = tasks[state.taskIndex % tasks.length];
    state.taskIndex++;
    state.totalRuns++;

    log(
      `\n${"=".repeat(60)}\n[HillClimber] Run #${state.totalRuns}/${options.maxRuns} - Task: ${taskId}\n${"=".repeat(60)}`,
    );

    try {
      await runSingleIteration(taskId, state.totalRuns, options, workspaceBase);
    } catch (e) {
      logError(
        `[HillClimber] Error in iteration: ${e instanceof Error ? e.message : String(e)}`,
        e instanceof Error ? e : undefined,
      );
    }

    // Sleep between runs
    if (state.running && state.totalRuns < options.maxRuns) {
      log(`[HillClimber] Sleeping ${options.sleepMs}ms...`);
      await new Promise((resolve) => setTimeout(resolve, options.sleepMs));
    }
  }

  log(`\n[HillClimber] Completed ${state.totalRuns} runs`);

  // Show final stats
  await showStats();
};

/**
 * Run a single iteration of the optimization loop.
 */
const runSingleIteration = async (
  taskId: string,
  runNumber: number,
  options: HillClimberOptions,
  workspaceBase: string,
): Promise<void> => {
  const program = Effect.gen(function* () {
    const store = yield* HillClimberStore;

    // Get or create current config for this task
    const config = yield* store.ensureDefaultConfig(taskId);
    log(`[HillClimber] Current config: hint=${config.hint ? "yes" : "no"}, skills=${config.useSkills}, maxTurns=${config.maxTurnsOverride}`);

    // Get the TB task
    const task = yield* Effect.promise(() => getTask(options.suitePath, taskId));
    if (!task) {
      logError(`[HillClimber] Task not found: ${taskId}`);
      return;
    }

    // Run the task
    log(`[HillClimber] Executing task...`);
    const execution = yield* Effect.promise(() =>
      runTask(taskId, config, {
        suitePath: options.suitePath,
        workspaceBase,
        timeout: 120, // 2 minutes per task
        onOutput: (text) => {
          // Only log important lines
          if (
            text.includes("[FM]") ||
            text.includes("[Executor]") ||
            text.includes("PASSED") ||
            text.includes("FAILED")
          ) {
            process.stdout.write(text);
          }
        },
      }),
    );

    const { result, score } = execution;
    log(`\n[HillClimber] ${formatRunSummary(result.passed, result.turns, score)}`);

    // Propose config change
    log(`[HillClimber] Analyzing result with meta-reasoner...`);
    let change;
    try {
      change = yield* Effect.promise(() =>
        Effect.runPromise(proposeConfigChange(task, config, result, runNumber)),
      );
    } catch (e) {
      log(
        `[HillClimber] Meta-reasoner failed, using heuristics: ${e instanceof Error ? e.message : String(e)}`,
      );
      change = proposeHeuristicChange(task, config, result);
    }

    log(`[HillClimber] Proposed change: ${change.type}`);
    if (change.type === "update_hint") {
      log(`[HillClimber] New hint: ${change.newHint?.slice(0, 100)}...`);
    }

    // Save new config if changed
    let finalConfigId = config.id;
    if (change.type !== "keep") {
      const newConfigInput = applyConfigChange(config, change);
      const newConfig = yield* store.saveConfig(newConfigInput);
      yield* store.setCurrentConfig(taskId, newConfig.id);
      finalConfigId = newConfig.id;
      log(`[HillClimber] Saved new config (id=${newConfig.id})`);
    }

    // Save run record
    const runId = genRunId();
    const runInput: HillClimberRunInput = {
      runId,
      taskId,
      configId: finalConfigId,
      passed: result.passed,
      turns: result.turns,
      durationMs: result.durationMs,
      stepSummary: result.stepSummary,
      errorMessage: result.errorMessage,
      metaModel: change.model ?? null, // Always record the model used, even for "keep"
      proposedChange: change.reasoning ?? null,
      changeAccepted: change.type !== "keep",
      score,
    };

    const savedRun = yield* store.saveRun(runInput);
    log(`[HillClimber] Saved run (id=${savedRun.id})`);

    // Update best config if this is better
    const currentBest = yield* store.getBestConfigForTask(taskId);
    if (!currentBest || isBetterScore(score, currentBest.score)) {
      yield* store.updateBestConfig(
        taskId,
        finalConfigId,
        savedRun.id,
        score,
        result.passed,
      );
      log(`[HillClimber] New best score for task: ${score}`);

      // Auto-export if stable enough
      yield* exportTaskHint(taskId).pipe(
        Effect.catchAll(() => Effect.succeed(null)),
      );
    } else {
      // Still update pass/total counts for tracking
      yield* store.updateBestConfig(
        taskId,
        currentBest.configId,
        currentBest.runId,
        currentBest.score,
        result.passed,
      );
    }
  });

  await Effect.runPromise(program.pipe(Effect.provide(HillClimberStoreLive)));
};

/**
 * Show current stats.
 */
export const showStats = async (): Promise<void> => {
  const program = Effect.gen(function* () {
    const store = yield* HillClimberStore;
    const stats = yield* store.getStats();

    console.log(`\n${"=".repeat(60)}`);
    console.log(`HillClimber Statistics`);
    console.log(`${"=".repeat(60)}`);
    console.log(`Total runs: ${stats.totalRuns}`);
    console.log(`Total passes: ${stats.totalPasses}`);
    console.log(`Overall pass rate: ${(stats.overallPassRate * 100).toFixed(1)}%`);
    console.log(`Unique tasks: ${stats.uniqueTasks}`);
    console.log(`Unique configs: ${stats.uniqueConfigs}`);

    if (Object.keys(stats.byTask).length > 0) {
      console.log(`\nPer-task stats:`);
      for (const [taskId, taskStats] of Object.entries(stats.byTask)) {
        console.log(
          `  ${taskId}: ${taskStats.passCount}/${taskStats.totalRuns} passes (${(taskStats.passRate * 100).toFixed(0)}%), best=${taskStats.bestScore}, avg_turns=${taskStats.avgTurns.toFixed(1)}`,
        );
      }
    }

    // Show best configs
    const bestConfigs = yield* store.getBestConfigs();
    if (bestConfigs.length > 0) {
      console.log(`\nBest configs:`);
      for (const best of bestConfigs) {
        const config = yield* store.getConfigById(best.configId);
        const hintPreview = config?.hint?.slice(0, 50) ?? "none";
        console.log(
          `  ${best.taskId}: score=${best.score}, passes=${best.passCount}/${best.totalRuns}, hint="${hintPreview}${config?.hint && config.hint.length > 50 ? "..." : ""}"`,
        );
      }
    }

    console.log(`${"=".repeat(60)}\n`);
  });

  await Effect.runPromise(program.pipe(Effect.provide(HillClimberStoreLive)));
};

/**
 * Run in dry-run mode (show what would happen).
 */
export const dryRun = async (options: HillClimberOptions): Promise<void> => {
  console.log(`[HillClimber] Dry run mode`);
  console.log(`[HillClimber] Suite: ${options.suitePath}`);

  // Discover tasks
  let tasks = options.tasks;
  if (tasks.length === 0) {
    tasks = await getAvailableTasks(options.suitePath);
    tasks = tasks.slice(0, 5);
  }

  console.log(`[HillClimber] Would optimize ${tasks.length} tasks:`);
  for (const taskId of tasks) {
    const task = await getTask(options.suitePath, taskId);
    console.log(
      `  - ${taskId}: ${task?.description?.slice(0, 80) ?? "unknown"}...`,
    );
  }

  console.log(`[HillClimber] Max runs: ${options.maxRuns}`);
  console.log(`[HillClimber] Sleep interval: ${options.sleepMs}ms`);

  // Show current stats if any
  await showStats();
};
