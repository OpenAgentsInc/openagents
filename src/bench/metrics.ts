/**
 * Benchmarking metrics for agent evaluation.
 *
 * Tracks: token usage, latency, success rate, tool call distribution, retry rate.
 */

import * as S from "effect/Schema";
import type { Usage } from "../llm/model-types.js";

// Token usage per turn
export const TurnTokenUsage = S.Struct({
  input: S.Number,
  output: S.Number,
  cacheRead: S.Number,
  cacheWrite: S.Number,
});
export type TurnTokenUsage = S.Schema.Type<typeof TurnTokenUsage>;

// Timing for a single operation
export const TimingMetric = S.Struct({
  startMs: S.Number,
  endMs: S.Number,
  durationMs: S.Number,
});
export type TimingMetric = S.Schema.Type<typeof TimingMetric>;

// Tool call record
export const ToolCallRecord = S.Struct({
  name: S.String,
  toolCallId: S.String,
  args: S.String, // JSON string
  success: S.Boolean,
  durationMs: S.Number,
  isRetry: S.Boolean, // true if this is a retry of a previous failed call
});
export type ToolCallRecord = S.Schema.Type<typeof ToolCallRecord>;

// Turn-level metrics
export const TurnMetrics = S.Struct({
  turnNumber: S.Number,
  timing: TimingMetric,
  tokenUsage: TurnTokenUsage,
  toolCalls: S.Array(ToolCallRecord),
  hasEdits: S.Boolean,
});
export type TurnMetrics = S.Schema.Type<typeof TurnMetrics>;

// Task-level outcome
export const TaskOutcome = S.Literal("success", "failure", "timeout", "error");
export type TaskOutcome = S.Schema.Type<typeof TaskOutcome>;

// Verification result
export const VerificationResult = S.Struct({
  typecheckPassed: S.Boolean,
  testsPassed: S.Boolean,
  verificationRan: S.Boolean,
});
export type VerificationResult = S.Schema.Type<typeof VerificationResult>;

// Complete task metrics
export const TaskMetrics = S.Struct({
  taskId: S.String,
  taskTitle: S.String,
  outcome: TaskOutcome,
  turns: S.Array(TurnMetrics),
  totalTiming: TimingMetric,
  totalTokenUsage: TurnTokenUsage,
  verification: VerificationResult,
  toolCallSummary: S.Struct({
    totalCalls: S.Number,
    successfulCalls: S.Number,
    failedCalls: S.Number,
    retryCalls: S.Number,
    byTool: S.Record({ key: S.String, value: S.Number }),
  }),
  errorMessage: S.optional(S.String),
});
export type TaskMetrics = S.Schema.Type<typeof TaskMetrics>;

// Benchmark run metadata
export const BenchmarkRunMeta = S.Struct({
  runId: S.String,
  startedAt: S.String, // ISO timestamp
  completedAt: S.String, // ISO timestamp
  model: S.String,
  projectId: S.String,
  gitBranch: S.optional(S.String),
  gitCommit: S.optional(S.String),
});
export type BenchmarkRunMeta = S.Schema.Type<typeof BenchmarkRunMeta>;

// Complete benchmark results
export const BenchmarkResults = S.Struct({
  meta: BenchmarkRunMeta,
  tasks: S.Array(TaskMetrics),
  summary: S.Struct({
    totalTasks: S.Number,
    successfulTasks: S.Number,
    failedTasks: S.Number,
    timeoutTasks: S.Number,
    errorTasks: S.Number,
    taskCompletionRate: S.Number, // successful / total
    verificationPassRate: S.Number, // tasks where verification passed / tasks where verification ran
    avgTokensPerTask: S.Number,
    avgTurnsPerTask: S.Number,
    avgToolCallsPerTask: S.Number,
    totalDurationMs: S.Number,
    toolDistribution: S.Record({ key: S.String, value: S.Number }),
    retryRate: S.Number, // retry calls / total calls
  }),
});
export type BenchmarkResults = S.Schema.Type<typeof BenchmarkResults>;

// --- Utility Functions ---

export const createEmptyTokenUsage = (): TurnTokenUsage => ({
  input: 0,
  output: 0,
  cacheRead: 0,
  cacheWrite: 0,
});

export const addTokenUsage = (a: TurnTokenUsage, b: TurnTokenUsage): TurnTokenUsage => ({
  input: a.input + b.input,
  output: a.output + b.output,
  cacheRead: a.cacheRead + b.cacheRead,
  cacheWrite: a.cacheWrite + b.cacheWrite,
});

export const usageToTokenUsage = (usage: Usage): TurnTokenUsage => ({
  input: usage.input,
  output: usage.output,
  cacheRead: usage.cacheRead,
  cacheWrite: usage.cacheWrite,
});

export const createTiming = (startMs: number, endMs: number): TimingMetric => ({
  startMs,
  endMs,
  durationMs: endMs - startMs,
});

// Aggregate metrics from multiple tasks
export const computeSummary = (
  tasks: TaskMetrics[],
): BenchmarkResults["summary"] => {
  const totalTasks = tasks.length;
  const successfulTasks = tasks.filter((t) => t.outcome === "success").length;
  const failedTasks = tasks.filter((t) => t.outcome === "failure").length;
  const timeoutTasks = tasks.filter((t) => t.outcome === "timeout").length;
  const errorTasks = tasks.filter((t) => t.outcome === "error").length;

  const verificationRan = tasks.filter((t) => t.verification.verificationRan);
  const verificationPassed = verificationRan.filter(
    (t) => t.verification.typecheckPassed && t.verification.testsPassed,
  );

  const totalTokens = tasks.reduce(
    (sum, t) => sum + t.totalTokenUsage.input + t.totalTokenUsage.output,
    0,
  );
  const totalTurns = tasks.reduce((sum, t) => sum + t.turns.length, 0);
  const totalToolCalls = tasks.reduce((sum, t) => sum + t.toolCallSummary.totalCalls, 0);
  const totalRetryCalls = tasks.reduce((sum, t) => sum + t.toolCallSummary.retryCalls, 0);
  const totalDurationMs = tasks.reduce((sum, t) => sum + t.totalTiming.durationMs, 0);

  // Aggregate tool distribution
  const toolDistribution: Record<string, number> = {};
  for (const task of tasks) {
    for (const [tool, count] of Object.entries(task.toolCallSummary.byTool)) {
      toolDistribution[tool] = (toolDistribution[tool] ?? 0) + count;
    }
  }

  return {
    totalTasks,
    successfulTasks,
    failedTasks,
    timeoutTasks,
    errorTasks,
    taskCompletionRate: totalTasks > 0 ? successfulTasks / totalTasks : 0,
    verificationPassRate:
      verificationRan.length > 0 ? verificationPassed.length / verificationRan.length : 0,
    avgTokensPerTask: totalTasks > 0 ? totalTokens / totalTasks : 0,
    avgTurnsPerTask: totalTasks > 0 ? totalTurns / totalTasks : 0,
    avgToolCallsPerTask: totalTasks > 0 ? totalToolCalls / totalTasks : 0,
    totalDurationMs,
    toolDistribution,
    retryRate: totalToolCalls > 0 ? totalRetryCalls / totalToolCalls : 0,
  };
};

// --- Metrics Collector Class ---

/**
 * Collects metrics during task execution.
 * Call methods as events happen, then finalize() to get TaskMetrics.
 */
export class MetricsCollector {
  private taskId: string;
  private taskTitle: string;
  private startMs: number;
  private turns: TurnMetrics[] = [];
  private currentTurnStartMs = 0;
  private currentTurnTokens: TurnTokenUsage = createEmptyTokenUsage();
  private currentTurnToolCalls: ToolCallRecord[] = [];
  private currentTurnHasEdits = false;
  private toolCallStartTimes: Map<string, number> = new Map();
  private previousToolCalls: Set<string> = new Set(); // track tool+args for retry detection

  constructor(taskId: string, taskTitle: string) {
    this.taskId = taskId;
    this.taskTitle = taskTitle;
    this.startMs = Date.now();
  }

  startTurn(): void {
    this.currentTurnStartMs = Date.now();
    this.currentTurnTokens = createEmptyTokenUsage();
    this.currentTurnToolCalls = [];
    this.currentTurnHasEdits = false;
  }

  recordTokenUsage(usage: TurnTokenUsage): void {
    this.currentTurnTokens = addTokenUsage(this.currentTurnTokens, usage);
  }

  startToolCall(toolCallId: string): void {
    this.toolCallStartTimes.set(toolCallId, Date.now());
  }

  endToolCall(
    toolCallId: string,
    name: string,
    args: string,
    success: boolean,
  ): void {
    const startTime = this.toolCallStartTimes.get(toolCallId) ?? Date.now();
    const durationMs = Date.now() - startTime;
    this.toolCallStartTimes.delete(toolCallId);

    // Check if this is a retry (same tool + args seen before)
    const callKey = `${name}:${args}`;
    const isRetry = this.previousToolCalls.has(callKey);
    this.previousToolCalls.add(callKey);

    this.currentTurnToolCalls.push({
      name,
      toolCallId,
      args,
      success,
      durationMs,
      isRetry,
    });

    if (name === "edit" || name === "write") {
      this.currentTurnHasEdits = true;
    }
  }

  endTurn(turnNumber: number): void {
    const endMs = Date.now();
    this.turns.push({
      turnNumber,
      timing: createTiming(this.currentTurnStartMs, endMs),
      tokenUsage: this.currentTurnTokens,
      toolCalls: this.currentTurnToolCalls,
      hasEdits: this.currentTurnHasEdits,
    });
  }

  finalize(
    outcome: TaskOutcome,
    verification: VerificationResult,
    errorMessage?: string,
  ): TaskMetrics {
    const endMs = Date.now();

    // Aggregate token usage
    const totalTokenUsage = this.turns.reduce(
      (acc, turn) => addTokenUsage(acc, turn.tokenUsage),
      createEmptyTokenUsage(),
    );

    // Aggregate tool calls
    let totalCalls = 0;
    let successfulCalls = 0;
    let failedCalls = 0;
    let retryCalls = 0;
    const byTool: Record<string, number> = {};

    for (const turn of this.turns) {
      for (const call of turn.toolCalls) {
        totalCalls++;
        if (call.success) successfulCalls++;
        else failedCalls++;
        if (call.isRetry) retryCalls++;
        byTool[call.name] = (byTool[call.name] ?? 0) + 1;
      }
    }

    return {
      taskId: this.taskId,
      taskTitle: this.taskTitle,
      outcome,
      turns: this.turns,
      totalTiming: createTiming(this.startMs, endMs),
      totalTokenUsage,
      verification,
      toolCallSummary: {
        totalCalls,
        successfulCalls,
        failedCalls,
        retryCalls,
        byTool,
      },
      ...(errorMessage ? { errorMessage } : {}),
    };
  }
}
