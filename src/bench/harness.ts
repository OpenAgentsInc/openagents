/**
 * Benchmarking harness for running tasks with metrics collection.
 *
 * Runs a set of tasks through an agent loop and collects comprehensive metrics.
 */

import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import type { Task } from "../tasks/schema.js";
import { readTasks } from "../tasks/service.js";
import type { Tool } from "../tools/schema.js";
import { agentLoop, type LoopEvent, type AgentConfig, type AgentResult } from "../agent/loop.js";
import { OpenRouterClient } from "../llm/openrouter.js";
import { DatabaseService } from "../storage/database.js";
import {
  MetricsCollector,
  computeSummary,
  type TaskMetrics,
  type BenchmarkResults,
  type BenchmarkRunMeta,
  type TaskOutcome,
  type VerificationResult,
} from "./metrics.js";

export class BenchmarkError extends Error {
  readonly _tag = "BenchmarkError";
  constructor(
    readonly reason: "no_tasks" | "task_error" | "config_error" | "io_error",
    message: string,
  ) {
    super(message);
    this.name = "BenchmarkError";
  }
}

export interface BenchmarkConfig {
  tasksPath: string;
  projectId: string;
  model: string;
  tools: Tool<any, any, any, any>[];
  systemPrompt?: string;
  maxTurns?: number;
  maxTasks?: number; // limit how many tasks to run
  taskFilter?: {
    labels?: string[];
    priority?: number;
  };
  onTaskStart?: (task: Task) => void;
  onTaskComplete?: (task: Task, metrics: TaskMetrics) => void;
  onLoopEvent?: (task: Task, event: LoopEvent) => void;
}

export interface BenchmarkRunContext {
  runId: string;
  gitBranch?: string;
  gitCommit?: string;
}

// Generate a unique run ID
const generateRunId = (): string => {
  const now = new Date();
  const dateStr = now.toISOString().slice(0, 10).replace(/-/g, "");
  const timeStr = now.toISOString().slice(11, 19).replace(/:/g, "");
  const rand = Math.random().toString(36).slice(2, 6);
  return `bench-${dateStr}-${timeStr}-${rand}`;
};

// Get git info if available
const getGitInfo = (): Effect.Effect<
  { branch: string | undefined; commit: string | undefined },
  never,
  never
> =>
  Effect.gen(function* () {
    // Try to get git branch
    const branchResult = yield* Effect.tryPromise({
      try: async () => {
        const proc = Bun.spawn(["git", "rev-parse", "--abbrev-ref", "HEAD"], {
          stdout: "pipe",
          stderr: "pipe",
        });
        const output = await new Response(proc.stdout).text();
        return output.trim();
      },
      catch: () => undefined,
    }).pipe(Effect.catchAll(() => Effect.succeed(undefined)));

    // Try to get git commit
    const commitResult = yield* Effect.tryPromise({
      try: async () => {
        const proc = Bun.spawn(["git", "rev-parse", "--short", "HEAD"], {
          stdout: "pipe",
          stderr: "pipe",
        });
        const output = await new Response(proc.stdout).text();
        return output.trim();
      },
      catch: () => undefined,
    }).pipe(Effect.catchAll(() => Effect.succeed(undefined)));

    return { branch: branchResult, commit: commitResult };
  });

// Run a single task through the agent loop with metrics collection
const runTaskWithMetrics = (
  task: Task,
  config: BenchmarkConfig,
): Effect.Effect<TaskMetrics, BenchmarkError, OpenRouterClient> =>
  Effect.gen(function* () {
    const collector = new MetricsCollector(task.id, task.title);

    // Build the user message from task
    const userMessage = buildTaskPrompt(task);

    // Set up event handler to collect metrics
    const onEvent = (event: LoopEvent) => {
      config.onLoopEvent?.(task, event);

      switch (event.type) {
        case "turn_start":
          collector.startTurn();
          break;
        case "tool_call":
          collector.startToolCall(event.toolCallId);
          break;
        case "tool_result":
          collector.endToolCall(
            event.toolCallId,
            event.tool,
            "", // args not available in tool_result event
            event.ok,
          );
          break;
        case "llm_response":
          // Token usage would come from response if available
          // For now, estimate based on message length
          // Real implementation would use actual usage from LLM response
          break;
      }
    };

    const agentConfig: AgentConfig = {
      model: config.model,
      maxTurns: config.maxTurns ?? 20,
      onEvent,
      ...(config.systemPrompt ? { systemPrompt: config.systemPrompt } : {}),
    };

    // Run the agent loop
    const result = yield* agentLoop(userMessage, config.tools, agentConfig).pipe(
      Effect.map((res): [AgentResult, TaskOutcome] => {
        // Determine outcome based on verify state
        const outcome: TaskOutcome =
          res.verifyState.typecheckOk && res.verifyState.testsOk
            ? "success"
            : res.verifyState.dirtySinceVerify
              ? "failure"
              : "success"; // No edits, so no verification needed
        return [res, outcome];
      }),
      Effect.catchTag("AgentLoopError", (error) =>
        Effect.succeed([
          {
            turns: [],
            finalMessage: error.message,
            totalTurns: 0,
            verifyState: {
              dirtySinceVerify: false,
              typecheckOk: false,
              testsOk: false,
            },
          },
          error.reason === "max_turns_exceeded" ? "timeout" : "error",
        ] as [AgentResult, TaskOutcome]),
      ),
    );

    const [agentResult, outcome] = result;

    // End any remaining turns
    if (agentResult.totalTurns > 0) {
      collector.endTurn(agentResult.totalTurns);
    }

    const verification: VerificationResult = {
      typecheckPassed: agentResult.verifyState.typecheckOk,
      testsPassed: agentResult.verifyState.testsOk,
      verificationRan: agentResult.verifyState.dirtySinceVerify ||
        agentResult.verifyState.typecheckOk ||
        agentResult.verifyState.testsOk,
    };

    const errorMessage =
      outcome === "error" || outcome === "timeout"
        ? agentResult.finalMessage ?? undefined
        : undefined;

    return collector.finalize(outcome, verification, errorMessage);
  });

// Build a prompt from a task
const buildTaskPrompt = (task: Task): string => {
  const parts = [`Task: ${task.title}`];

  if (task.description) {
    parts.push(`\nDescription: ${task.description}`);
  }

  if (task.acceptanceCriteria) {
    parts.push(`\nAcceptance Criteria: ${task.acceptanceCriteria}`);
  }

  if (task.design) {
    parts.push(`\nDesign Notes: ${task.design}`);
  }

  return parts.join("\n");
};

/**
 * Run a benchmark against a set of tasks.
 */
export const runBenchmark = (
  config: BenchmarkConfig,
  context?: BenchmarkRunContext,
): Effect.Effect<
  BenchmarkResults,
  BenchmarkError,
  FileSystem.FileSystem | Path.Path | OpenRouterClient | DatabaseService
> =>
  Effect.gen(function* () {
    const runId = context?.runId ?? generateRunId();
    const startedAt = new Date().toISOString();

    // Get git info
    const gitInfo = yield* getGitInfo();

    // Load tasks
    const allTasks = yield* readTasks(config.tasksPath).pipe(
      Effect.mapError(
        (e) => new BenchmarkError("io_error", `Failed to read tasks: ${e.message}`),
      ),
    );

    // Filter tasks
    let tasks = allTasks.filter((t) => t.status === "open" || t.status === "in_progress");

    if (config.taskFilter?.labels?.length) {
      const labels = config.taskFilter.labels;
      tasks = tasks.filter((t) =>
        labels.some((label) => t.labels?.includes(label)),
      );
    }

    if (config.taskFilter?.priority !== undefined) {
      const priority = config.taskFilter.priority;
      tasks = tasks.filter((t) => t.priority === priority);
    }

    // Sort by priority
    tasks.sort((a, b) => a.priority - b.priority);

    // Apply max tasks limit
    if (config.maxTasks && config.maxTasks > 0) {
      tasks = tasks.slice(0, config.maxTasks);
    }

    if (tasks.length === 0) {
      return yield* Effect.fail(
        new BenchmarkError("no_tasks", "No tasks found matching filter criteria"),
      );
    }

    // Run each task
    const taskMetrics: TaskMetrics[] = [];

    for (const task of tasks) {
      config.onTaskStart?.(task);

      const metrics = yield* runTaskWithMetrics(task, config);
      taskMetrics.push(metrics);

      config.onTaskComplete?.(task, metrics);
    }

    const completedAt = new Date().toISOString();

    // Compute summary
    const summary = computeSummary(taskMetrics);

    // Build metadata
    const meta: BenchmarkRunMeta = {
      runId,
      startedAt,
      completedAt,
      model: config.model,
      projectId: config.projectId,
      ...(context?.gitBranch ?? gitInfo.branch
        ? { gitBranch: context?.gitBranch ?? gitInfo.branch }
        : {}),
      ...(context?.gitCommit ?? gitInfo.commit
        ? { gitCommit: context?.gitCommit ?? gitInfo.commit }
        : {}),
    };

    return {
      meta,
      tasks: taskMetrics,
      summary,
    };
  });

/**
 * Save benchmark results to a file.
 */
export const saveBenchmarkResults = (
  results: BenchmarkResults,
  outputPath: string,
): Effect.Effect<void, BenchmarkError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;

    // Ensure directory exists
    const dir = path.dirname(outputPath);
    yield* fs.makeDirectory(dir, { recursive: true }).pipe(
      Effect.mapError(
        (e) => new BenchmarkError("io_error", `Failed to create directory: ${e.message}`),
      ),
    );

    // Write results
    const content = JSON.stringify(results, null, 2);
    yield* fs.writeFile(outputPath, new TextEncoder().encode(content)).pipe(
      Effect.mapError(
        (e) => new BenchmarkError("io_error", `Failed to write results: ${e.message}`),
      ),
    );
  });

/**
 * Load benchmark results from a file.
 */
export const loadBenchmarkResults = (
  inputPath: string,
): Effect.Effect<BenchmarkResults, BenchmarkError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;

    const content = yield* fs.readFileString(inputPath).pipe(
      Effect.mapError(
        (e) => new BenchmarkError("io_error", `Failed to read results: ${e.message}`),
      ),
    );

    const parsed = yield* Effect.try({
      try: () => JSON.parse(content) as BenchmarkResults,
      catch: (e) => new BenchmarkError("io_error", `Failed to parse results: ${e}`),
    });

    return parsed;
  });
