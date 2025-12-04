/**
 * Terminal-Bench 2.0 adapter for agent evaluation.
 *
 * Adapts Terminal-Bench task format to OpenAgents benchmark harness.
 * Terminal-Bench provides standardized coding tasks for evaluating
 * AI coding agents.
 *
 * Reference: https://github.com/Terminal-Bench/Terminal-Bench
 */

import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import * as S from "effect/Schema";
import type { Task, TaskCreate } from "../tasks/schema.js";
import { createTask } from "../tasks/service.js";

// --- Terminal-Bench Task Schema ---

// Terminal-Bench task difficulty levels
export const TerminalBenchDifficulty = S.Literal("easy", "medium", "hard", "expert");
export type TerminalBenchDifficulty = S.Schema.Type<typeof TerminalBenchDifficulty>;

// Terminal-Bench task categories (accept any string for flexibility)
export const TerminalBenchCategory = S.String;
export type TerminalBenchCategory = S.Schema.Type<typeof TerminalBenchCategory>;

// Terminal-Bench verification method
export const TerminalBenchVerification = S.Struct({
  type: S.Literal("test", "output", "diff", "custom"),
  command: S.optional(S.String),
  expected: S.optional(S.String),
  script: S.optional(S.String),
});
export type TerminalBenchVerification = S.Schema.Type<typeof TerminalBenchVerification>;

// Terminal-Bench task format
export const TerminalBenchTask = S.Struct({
  id: S.String,
  name: S.String,
  description: S.String,
  difficulty: TerminalBenchDifficulty,
  category: TerminalBenchCategory,
  repository: S.optional(S.String), // git URL or local path
  branch: S.optional(S.String),
  setup: S.optional(S.Array(S.String)), // commands to run before task
  verification: TerminalBenchVerification,
  timeout_seconds: S.optional(S.Number),
  max_turns: S.optional(S.Number),
  tags: S.optional(S.Array(S.String)),
  // Additional context for the agent
  files_to_modify: S.optional(S.Array(S.String)),
  hints: S.optional(S.Array(S.String)),
  // Source path for importing from external repos
  source_path: S.optional(S.String),
});
export type TerminalBenchTask = S.Schema.Type<typeof TerminalBenchTask>;

// Terminal-Bench suite format
export const TerminalBenchSuite = S.Struct({
  name: S.String,
  version: S.String,
  description: S.optional(S.String),
  source_repo: S.optional(S.String),
  tasks: S.Array(TerminalBenchTask),
});
export type TerminalBenchSuite = S.Schema.Type<typeof TerminalBenchSuite>;

// --- Result Schema (for Terminal-Bench compatibility) ---

export const TerminalBenchResult = S.Struct({
  task_id: S.String,
  status: S.Literal("pass", "fail", "timeout", "error", "skip"),
  duration_ms: S.Number,
  turns: S.Number,
  tokens_used: S.Number,
  verification_output: S.optional(S.String),
  error_message: S.optional(S.String),
});
export type TerminalBenchResult = S.Schema.Type<typeof TerminalBenchResult>;

export const TerminalBenchResults = S.Struct({
  suite_name: S.String,
  suite_version: S.String,
  model: S.String,
  timestamp: S.String,
  results: S.Array(TerminalBenchResult),
  summary: S.Struct({
    total: S.Number,
    passed: S.Number,
    failed: S.Number,
    timeout: S.Number,
    error: S.Number,
    skipped: S.Number,
    pass_rate: S.Number,
    avg_duration_ms: S.Number,
    avg_turns: S.Number,
    total_tokens: S.Number,
  }),
});
export type TerminalBenchResults = S.Schema.Type<typeof TerminalBenchResults>;

// --- Adapter Functions ---

export class TerminalBenchError extends Error {
  readonly _tag = "TerminalBenchError";
  constructor(
    readonly reason: "parse_error" | "io_error" | "setup_error" | "verification_error",
    message: string,
  ) {
    super(message);
    this.name = "TerminalBenchError";
  }
}

// Map difficulty to priority (0 = highest)
const difficultyToPriority = (difficulty: TerminalBenchDifficulty): number => {
  switch (difficulty) {
    case "easy":
      return 3;
    case "medium":
      return 2;
    case "hard":
      return 1;
    case "expert":
      return 0;
  }
};

/**
 * Convert a Terminal-Bench task to an OpenAgents task.
 */
export const terminalBenchToTask = (tbTask: TerminalBenchTask): TaskCreate => {
  const labels = [
    "terminal-bench",
    tbTask.category,
    tbTask.difficulty,
    ...(tbTask.tags ?? []),
  ];

  // Build description with hints and file info
  const descParts = [tbTask.description];

  if (tbTask.files_to_modify?.length) {
    descParts.push(`\nFiles to modify: ${tbTask.files_to_modify.join(", ")}`);
  }

  if (tbTask.hints?.length) {
    descParts.push(`\nHints:\n${tbTask.hints.map((h) => `- ${h}`).join("\n")}`);
  }

  // Build acceptance criteria from verification
  let acceptanceCriteria: string | undefined;
  switch (tbTask.verification.type) {
    case "test":
      acceptanceCriteria = `Run tests: ${tbTask.verification.command ?? "default test command"}`;
      break;
    case "output":
      acceptanceCriteria = `Expected output: ${tbTask.verification.expected ?? "see verification"}`;
      break;
    case "diff":
      acceptanceCriteria = "Changes must match expected diff";
      break;
    case "custom":
      acceptanceCriteria = `Custom verification: ${tbTask.verification.script ?? "see verification"}`;
      break;
  }

  return {
    title: tbTask.name,
    description: descParts.join("\n"),
    status: "open" as const,
    type: "task" as const,
    priority: difficultyToPriority(tbTask.difficulty),
    labels,
    deps: [],
    ...(acceptanceCriteria ? { acceptanceCriteria } : {}),
  };
};

/**
 * Load a Terminal-Bench suite from a JSON file.
 */
export const loadTerminalBenchSuite = (
  suitePath: string,
): Effect.Effect<TerminalBenchSuite, TerminalBenchError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;

    const content = yield* fs.readFileString(suitePath).pipe(
      Effect.mapError(
        (e) => new TerminalBenchError("io_error", `Failed to read suite: ${e.message}`),
      ),
    );

    const parsed = yield* Effect.try({
      try: () => JSON.parse(content),
      catch: (e) => new TerminalBenchError("parse_error", `Invalid JSON: ${e}`),
    });

    const decoded = yield* Effect.try({
      try: () => S.decodeUnknownSync(TerminalBenchSuite)(parsed),
      catch: (e) =>
        new TerminalBenchError("parse_error", `Invalid Terminal-Bench suite: ${e}`),
    });

    return decoded;
  });

/**
 * Import Terminal-Bench tasks into the OpenAgents task system.
 */
export const importTerminalBenchSuite = (
  suite: TerminalBenchSuite,
  tasksPath: string,
  idPrefix = "tb",
): Effect.Effect<Task[], TerminalBenchError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const importedTasks: Task[] = [];

    for (const tbTask of suite.tasks) {
      const taskCreate = terminalBenchToTask(tbTask);

      const task = yield* createTask({
        tasksPath,
        task: taskCreate,
        idPrefix,
      }).pipe(
        Effect.mapError(
          (e) =>
            new TerminalBenchError(
              "io_error",
              `Failed to create task ${tbTask.id}: ${e.message}`,
            ),
        ),
      );

      importedTasks.push(task);
    }

    return importedTasks;
  });

/**
 * Run setup commands for a Terminal-Bench task.
 */
export const runTaskSetup = (
  tbTask: TerminalBenchTask,
): Effect.Effect<void, TerminalBenchError, never> =>
  Effect.gen(function* () {
    if (!tbTask.setup?.length) return;

    for (const cmd of tbTask.setup) {
      yield* Effect.tryPromise({
        try: async () => {
          const proc = Bun.spawn(["sh", "-c", cmd], {
            stdout: "pipe",
            stderr: "pipe",
          });
          const exitCode = await proc.exited;
          if (exitCode !== 0) {
            const stderr = await new Response(proc.stderr).text();
            throw new Error(`Setup command failed: ${cmd}\n${stderr}`);
          }
        },
        catch: (e) =>
          new TerminalBenchError("setup_error", `Setup failed: ${e}`),
      });
    }
  });

/**
 * Run verification for a Terminal-Bench task.
 */
export const runTaskVerification = (
  tbTask: TerminalBenchTask,
): Effect.Effect<{ passed: boolean; output: string }, TerminalBenchError, never> =>
  Effect.gen(function* () {
    const verification = tbTask.verification;

    switch (verification.type) {
      case "test":
      case "custom": {
        const cmd = verification.command ?? verification.script ?? "exit 1";
        const result = yield* Effect.tryPromise({
          try: async () => {
            const proc = Bun.spawn(["sh", "-c", cmd], {
              stdout: "pipe",
              stderr: "pipe",
            });
            const exitCode = await proc.exited;
            const stdout = await new Response(proc.stdout).text();
            const stderr = await new Response(proc.stderr).text();
            return {
              passed: exitCode === 0,
              output: stdout + (stderr ? `\nSTDERR:\n${stderr}` : ""),
            };
          },
          catch: (e) =>
            new TerminalBenchError(
              "verification_error",
              `Verification failed: ${e}`,
            ),
        });
        return result;
      }

      case "output": {
        const cmd = verification.command ?? "exit 1";
        const expected = verification.expected ?? "";
        const result = yield* Effect.tryPromise({
          try: async () => {
            const proc = Bun.spawn(["sh", "-c", cmd], {
              stdout: "pipe",
              stderr: "pipe",
            });
            await proc.exited;
            const stdout = await new Response(proc.stdout).text();
            const passed = stdout.trim() === expected.trim();
            return {
              passed,
              output: `Expected: ${expected}\nActual: ${stdout}`,
            };
          },
          catch: (e) =>
            new TerminalBenchError(
              "verification_error",
              `Verification failed: ${e}`,
            ),
        });
        return result;
      }

      case "diff": {
        // Diff verification would compare file changes
        // For now, return not implemented
        return {
          passed: false,
          output: "Diff verification not yet implemented",
        };
      }
    }
  });

/**
 * Convert OpenAgents benchmark results to Terminal-Bench format.
 */
export const toBenchmarkResults = (
  suite: TerminalBenchSuite,
  model: string,
  taskResults: ReadonlyArray<{
    taskId: string;
    outcome: "success" | "failure" | "timeout" | "error";
    durationMs: number;
    turns: number;
    tokens: number;
    verificationOutput: string | undefined;
    errorMessage: string | undefined;
  }>,
): TerminalBenchResults => {
  const timestamp = new Date().toISOString();

  const byTaskId = new Map<string, typeof taskResults[number]>();
  for (const result of taskResults) {
    byTaskId.set(result.taskId, result);
  }

  const results: TerminalBenchResult[] = suite.tasks.map((task) => {
    const r = byTaskId.get(task.id);
    if (!r) {
      return {
        task_id: task.id,
        status: "skip",
        duration_ms: 0,
        turns: 0,
        tokens_used: 0,
      };
    }

    let status: TerminalBenchResult["status"];
    switch (r.outcome) {
      case "success":
        status = "pass";
        break;
      case "failure":
        status = "fail";
        break;
      case "timeout":
        status = "timeout";
        break;
      case "error":
        status = "error";
        break;
    }

    return {
      task_id: r.taskId,
      status,
      duration_ms: r.durationMs,
      turns: r.turns,
      tokens_used: r.tokens,
      ...(r.verificationOutput ? { verification_output: r.verificationOutput } : {}),
      ...(r.errorMessage ? { error_message: r.errorMessage } : {}),
    };
  });

  // Include any extra results not present in suite (defensive)
  for (const r of taskResults) {
    if (!byTaskId.has(r.taskId)) continue;
    if (suite.tasks.find((t) => t.id === r.taskId)) continue;
    let status: TerminalBenchResult["status"];
    switch (r.outcome) {
      case "success":
        status = "pass";
        break;
      case "failure":
        status = "fail";
        break;
      case "timeout":
        status = "timeout";
        break;
      case "error":
        status = "error";
        break;
    }
    results.push({
      task_id: r.taskId,
      status,
      duration_ms: r.durationMs,
      turns: r.turns,
      tokens_used: r.tokens,
      ...(r.verificationOutput ? { verification_output: r.verificationOutput } : {}),
      ...(r.errorMessage ? { error_message: r.errorMessage } : {}),
    });
  }

  const total = results.length;
  const passed = results.filter((r) => r.status === "pass").length;
  const failed = results.filter((r) => r.status === "fail").length;
  const timeout = results.filter((r) => r.status === "timeout").length;
  const error = results.filter((r) => r.status === "error").length;
  const skipped = results.filter((r) => r.status === "skip").length;

  const totalDuration = results.reduce((sum, r) => sum + r.duration_ms, 0);
  const totalTurns = results.reduce((sum, r) => sum + r.turns, 0);
  const totalTokens = results.reduce((sum, r) => sum + r.tokens_used, 0);

  return {
    suite_name: suite.name,
    suite_version: suite.version,
    model,
    timestamp,
    results,
    summary: {
      total,
      passed,
      failed,
      timeout,
      error,
      skipped,
      pass_rate: total > 0 ? passed / total : 0,
      avg_duration_ms: total > 0 ? totalDuration / total : 0,
      avg_turns: total > 0 ? totalTurns / total : 0,
      total_tokens: totalTokens,
    },
  };
};
