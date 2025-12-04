/**
 * Benchmark comparison reports.
 *
 * Generates human-readable and machine-readable comparison reports
 * between benchmark runs.
 */

import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import * as S from "effect/Schema";
import type { BenchmarkResults, TaskMetrics } from "./metrics.js";
import type {
  TerminalBenchResult,
  TerminalBenchResults,
  TerminalBenchSuite,
} from "./terminal-bench.js";

// --- Comparison Schema ---

export const MetricDelta = S.Struct({
  metric: S.String,
  baseline: S.Number,
  current: S.Number,
  delta: S.Number,
  deltaPercent: S.Number,
  improved: S.Boolean,
});
export type MetricDelta = S.Schema.Type<typeof MetricDelta>;

export const TaskComparison = S.Struct({
  taskId: S.String,
  taskTitle: S.String,
  baselineOutcome: S.String,
  currentOutcome: S.String,
  outcomeChanged: S.Boolean,
  improved: S.Boolean,
  regressed: S.Boolean,
  turnsDelta: S.Number,
  tokensDelta: S.Number,
  durationDelta: S.Number,
});
export type TaskComparison = S.Schema.Type<typeof TaskComparison>;

export const ComparisonReport = S.Struct({
  generatedAt: S.String,
  baseline: S.Struct({
    runId: S.String,
    model: S.String,
    completedAt: S.String,
  }),
  current: S.Struct({
    runId: S.String,
    model: S.String,
    completedAt: S.String,
  }),
  summaryDeltas: S.Array(MetricDelta),
  taskComparisons: S.Array(TaskComparison),
  overallVerdict: S.Literal("improved", "regressed", "unchanged", "mixed"),
});
export type ComparisonReport = S.Schema.Type<typeof ComparisonReport>;

// --- Report Generation ---

export class ReporterError extends Error {
  readonly _tag = "ReporterError";
  constructor(
    readonly reason: "io_error" | "incompatible_runs" | "no_common_tasks",
    message: string,
  ) {
    super(message);
    this.name = "ReporterError";
  }
}

const computeDelta = (
  metric: string,
  baseline: number,
  current: number,
  lowerIsBetter: boolean,
): MetricDelta => {
  const delta = current - baseline;
  const deltaPercent = baseline !== 0 ? (delta / baseline) * 100 : 0;
  const improved = lowerIsBetter ? delta < 0 : delta > 0;

  return {
    metric,
    baseline,
    current,
    delta,
    deltaPercent,
    improved,
  };
};

/**
 * Compare two benchmark runs and generate a report.
 */
export const compareRuns = (
  baseline: BenchmarkResults,
  current: BenchmarkResults,
): Effect.Effect<ComparisonReport, ReporterError, never> =>
  Effect.gen(function* () {
    // Compute summary deltas
    const summaryDeltas: MetricDelta[] = [
      computeDelta(
        "taskCompletionRate",
        baseline.summary.taskCompletionRate,
        current.summary.taskCompletionRate,
        false, // higher is better
      ),
      computeDelta(
        "verificationPassRate",
        baseline.summary.verificationPassRate,
        current.summary.verificationPassRate,
        false, // higher is better
      ),
      computeDelta(
        "avgTokensPerTask",
        baseline.summary.avgTokensPerTask,
        current.summary.avgTokensPerTask,
        true, // lower is better
      ),
      computeDelta(
        "avgTurnsPerTask",
        baseline.summary.avgTurnsPerTask,
        current.summary.avgTurnsPerTask,
        true, // lower is better
      ),
      computeDelta(
        "avgToolCallsPerTask",
        baseline.summary.avgToolCallsPerTask,
        current.summary.avgToolCallsPerTask,
        true, // lower is better (efficiency)
      ),
      computeDelta(
        "retryRate",
        baseline.summary.retryRate,
        current.summary.retryRate,
        true, // lower is better
      ),
      computeDelta(
        "totalDurationMs",
        baseline.summary.totalDurationMs,
        current.summary.totalDurationMs,
        true, // lower is better
      ),
    ];

    // Build task comparison map
    const baselineTaskMap = new Map<string, TaskMetrics>();
    for (const task of baseline.tasks) {
      baselineTaskMap.set(task.taskId, task);
    }

    const taskComparisons: TaskComparison[] = [];
    let improvements = 0;
    let regressions = 0;

    for (const currentTask of current.tasks) {
      const baselineTask = baselineTaskMap.get(currentTask.taskId);

      if (!baselineTask) {
        // New task, not in baseline
        taskComparisons.push({
          taskId: currentTask.taskId,
          taskTitle: currentTask.taskTitle,
          baselineOutcome: "N/A",
          currentOutcome: currentTask.outcome,
          outcomeChanged: true,
          improved: currentTask.outcome === "success",
          regressed: false,
          turnsDelta: 0,
          tokensDelta: 0,
          durationDelta: 0,
        });
        if (currentTask.outcome === "success") improvements++;
        continue;
      }

      const outcomeChanged = baselineTask.outcome !== currentTask.outcome;
      const baselineSuccess = baselineTask.outcome === "success";
      const currentSuccess = currentTask.outcome === "success";

      let improved = false;
      let regressed = false;

      if (outcomeChanged) {
        if (!baselineSuccess && currentSuccess) {
          improved = true;
          improvements++;
        } else if (baselineSuccess && !currentSuccess) {
          regressed = true;
          regressions++;
        }
      }

      const baselineTokens =
        baselineTask.totalTokenUsage.input + baselineTask.totalTokenUsage.output;
      const currentTokens =
        currentTask.totalTokenUsage.input + currentTask.totalTokenUsage.output;

      taskComparisons.push({
        taskId: currentTask.taskId,
        taskTitle: currentTask.taskTitle,
        baselineOutcome: baselineTask.outcome,
        currentOutcome: currentTask.outcome,
        outcomeChanged,
        improved,
        regressed,
        turnsDelta: currentTask.turns.length - baselineTask.turns.length,
        tokensDelta: currentTokens - baselineTokens,
        durationDelta:
          currentTask.totalTiming.durationMs - baselineTask.totalTiming.durationMs,
      });
    }

    // Determine overall verdict
    let overallVerdict: ComparisonReport["overallVerdict"];
    const completionDelta =
      current.summary.taskCompletionRate - baseline.summary.taskCompletionRate;

    if (improvements > 0 && regressions === 0 && completionDelta >= 0) {
      overallVerdict = "improved";
    } else if (regressions > 0 && improvements === 0 && completionDelta <= 0) {
      overallVerdict = "regressed";
    } else if (improvements > 0 || regressions > 0) {
      overallVerdict = "mixed";
    } else {
      overallVerdict = "unchanged";
    }

    return {
      generatedAt: new Date().toISOString(),
      baseline: {
        runId: baseline.meta.runId,
        model: baseline.meta.model,
        completedAt: baseline.meta.completedAt,
      },
      current: {
        runId: current.meta.runId,
        model: current.meta.model,
        completedAt: current.meta.completedAt,
      },
      summaryDeltas,
      taskComparisons,
      overallVerdict,
    };
  });

// --- Formatters ---

const formatPercent = (value: number): string => {
  const sign = value >= 0 ? "+" : "";
  return `${sign}${value.toFixed(1)}%`;
};

const formatNumber = (value: number, decimals = 1): string => {
  const sign = value >= 0 ? "+" : "";
  return `${sign}${value.toFixed(decimals)}`;
};

const formatDuration = (ms: number): string => {
  if (Math.abs(ms) < 1000) return `${ms.toFixed(0)}ms`;
  if (Math.abs(ms) < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60000).toFixed(1)}m`;
};

/**
 * Format comparison report as markdown.
 */
export const formatMarkdownReport = (report: ComparisonReport): string => {
  const lines: string[] = [];

  lines.push("# Benchmark Comparison Report");
  lines.push("");
  lines.push(`Generated: ${report.generatedAt}`);
  lines.push("");

  // Run info
  lines.push("## Runs Compared");
  lines.push("");
  lines.push("| | Baseline | Current |");
  lines.push("|---|---|---|");
  lines.push(`| Run ID | ${report.baseline.runId} | ${report.current.runId} |`);
  lines.push(`| Model | ${report.baseline.model} | ${report.current.model} |`);
  lines.push(`| Completed | ${report.baseline.completedAt} | ${report.current.completedAt} |`);
  lines.push("");

  // Overall verdict
  const verdictEmoji: Record<string, string> = {
    improved: "✅",
    regressed: "❌",
    unchanged: "➖",
    mixed: "⚠️",
  };
  lines.push(`## Overall Verdict: ${verdictEmoji[report.overallVerdict]} ${report.overallVerdict.toUpperCase()}`);
  lines.push("");

  // Summary deltas
  lines.push("## Summary Metrics");
  lines.push("");
  lines.push("| Metric | Baseline | Current | Delta | Trend |");
  lines.push("|--------|----------|---------|-------|-------|");

  for (const delta of report.summaryDeltas) {
    const trend = delta.improved ? "⬆️" : delta.delta === 0 ? "➖" : "⬇️";
    const baselineStr =
      delta.metric.includes("Rate") || delta.metric.includes("Percent")
        ? `${(delta.baseline * 100).toFixed(1)}%`
        : delta.metric.includes("Duration")
          ? formatDuration(delta.baseline)
          : delta.baseline.toFixed(1);
    const currentStr =
      delta.metric.includes("Rate") || delta.metric.includes("Percent")
        ? `${(delta.current * 100).toFixed(1)}%`
        : delta.metric.includes("Duration")
          ? formatDuration(delta.current)
          : delta.current.toFixed(1);

    lines.push(
      `| ${delta.metric} | ${baselineStr} | ${currentStr} | ${formatPercent(delta.deltaPercent)} | ${trend} |`,
    );
  }
  lines.push("");

  // Task comparisons
  const changedTasks = report.taskComparisons.filter((t) => t.outcomeChanged);
  if (changedTasks.length > 0) {
    lines.push("## Task Outcome Changes");
    lines.push("");
    lines.push("| Task | Baseline | Current | Status |");
    lines.push("|------|----------|---------|--------|");

    for (const task of changedTasks) {
      const status = task.improved ? "✅ Improved" : task.regressed ? "❌ Regressed" : "➖ Changed";
      lines.push(
        `| ${task.taskTitle} | ${task.baselineOutcome} | ${task.currentOutcome} | ${status} |`,
      );
    }
    lines.push("");
  }

  // Efficiency changes for successful tasks
  const successfulTasks = report.taskComparisons.filter(
    (t) => t.currentOutcome === "success" && t.baselineOutcome === "success",
  );
  if (successfulTasks.length > 0) {
    lines.push("## Efficiency Changes (Successful Tasks)");
    lines.push("");
    lines.push("| Task | Turns Δ | Tokens Δ | Duration Δ |");
    lines.push("|------|---------|----------|------------|");

    for (const task of successfulTasks) {
      lines.push(
        `| ${task.taskTitle} | ${formatNumber(task.turnsDelta, 0)} | ${formatNumber(task.tokensDelta, 0)} | ${formatDuration(task.durationDelta)} |`,
      );
    }
    lines.push("");
  }

  return lines.join("\n");
};

/**
 * Format a single benchmark run as a summary markdown.
 */
export const formatRunSummary = (results: BenchmarkResults): string => {
  const lines: string[] = [];

  lines.push("# Benchmark Run Summary");
  lines.push("");
  lines.push(`- **Run ID:** ${results.meta.runId}`);
  lines.push(`- **Model:** ${results.meta.model}`);
  lines.push(`- **Project:** ${results.meta.projectId}`);
  lines.push(`- **Started:** ${results.meta.startedAt}`);
  lines.push(`- **Completed:** ${results.meta.completedAt}`);
  if (results.meta.gitBranch) {
    lines.push(`- **Branch:** ${results.meta.gitBranch}`);
  }
  if (results.meta.gitCommit) {
    lines.push(`- **Commit:** ${results.meta.gitCommit}`);
  }
  lines.push("");

  lines.push("## Summary");
  lines.push("");
  lines.push(`| Metric | Value |`);
  lines.push(`|--------|-------|`);
  lines.push(`| Total Tasks | ${results.summary.totalTasks} |`);
  lines.push(`| Successful | ${results.summary.successfulTasks} |`);
  lines.push(`| Failed | ${results.summary.failedTasks} |`);
  lines.push(`| Timeout | ${results.summary.timeoutTasks} |`);
  lines.push(`| Error | ${results.summary.errorTasks} |`);
  lines.push(`| Completion Rate | ${(results.summary.taskCompletionRate * 100).toFixed(1)}% |`);
  lines.push(`| Verification Pass Rate | ${(results.summary.verificationPassRate * 100).toFixed(1)}% |`);
  lines.push(`| Avg Tokens/Task | ${results.summary.avgTokensPerTask.toFixed(0)} |`);
  lines.push(`| Avg Turns/Task | ${results.summary.avgTurnsPerTask.toFixed(1)} |`);
  lines.push(`| Avg Tool Calls/Task | ${results.summary.avgToolCallsPerTask.toFixed(1)} |`);
  lines.push(`| Retry Rate | ${(results.summary.retryRate * 100).toFixed(1)}% |`);
  lines.push(`| Total Duration | ${formatDuration(results.summary.totalDurationMs)} |`);
  lines.push("");

  // Tool distribution
  if (Object.keys(results.summary.toolDistribution).length > 0) {
    lines.push("## Tool Distribution");
    lines.push("");
    lines.push("| Tool | Calls |");
    lines.push("|------|-------|");

    const sorted = Object.entries(results.summary.toolDistribution).sort(
      ([, a], [, b]) => b - a,
    );
    for (const [tool, count] of sorted) {
      lines.push(`| ${tool} | ${count} |`);
    }
    lines.push("");
  }

  // Task details
  lines.push("## Task Results");
  lines.push("");
  lines.push("| Task | Outcome | Turns | Tokens | Duration |");
  lines.push("|------|---------|-------|--------|----------|");

  for (const task of results.tasks) {
    const tokens = task.totalTokenUsage.input + task.totalTokenUsage.output;
    const outcomeEmoji: Record<string, string> = {
      success: "✅",
      failure: "❌",
      timeout: "⏱️",
      error: "💥",
    };
    lines.push(
      `| ${task.taskTitle} | ${outcomeEmoji[task.outcome]} ${task.outcome} | ${task.turns.length} | ${tokens} | ${formatDuration(task.totalTiming.durationMs)} |`,
    );
  }
  lines.push("");

  return lines.join("\n");
};

// --- Terminal-Bench Reporting ---

export const TerminalBenchCategorySummary = S.Struct({
  category: S.String,
  total: S.Number,
  passed: S.Number,
  failed: S.Number,
  timeout: S.Number,
  error: S.Number,
  skipped: S.Number,
  passRate: S.Number,
  avgDurationMs: S.Number,
  avgTurns: S.Number,
  totalTokens: S.Number,
});
export type TerminalBenchCategorySummary = S.Schema.Type<typeof TerminalBenchCategorySummary>;

export const TerminalBenchReport = S.Struct({
  suiteName: S.String,
  suiteVersion: S.String,
  model: S.String,
  timestamp: S.String,
  overall: TerminalBenchCategorySummary,
  categories: S.Array(TerminalBenchCategorySummary),
});
export type TerminalBenchReport = S.Schema.Type<typeof TerminalBenchReport>;

const computeTbSummary = (
  category: string,
  results: ReadonlyArray<TerminalBenchResult>,
): TerminalBenchCategorySummary => {
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
    category,
    total,
    passed,
    failed,
    timeout,
    error,
    skipped,
    passRate: total > 0 ? passed / total : 0,
    avgDurationMs: total > 0 ? totalDuration / total : 0,
    avgTurns: total > 0 ? totalTurns / total : 0,
    totalTokens,
  };
};

export const buildTerminalBenchReport = (
  suite: TerminalBenchSuite,
  results: TerminalBenchResults,
): TerminalBenchReport => {
  const categoryMap = new Map<string, string>();
  for (const task of suite.tasks) {
    categoryMap.set(task.id, task.category ?? "uncategorized");
  }

  const bucket = new Map<string, TerminalBenchResult[]>();
  for (const result of results.results) {
    const category = categoryMap.get(result.task_id) ?? "uncategorized";
    if (!bucket.has(category)) bucket.set(category, []);
    bucket.get(category)!.push(result);
  }

  const categories = Array.from(bucket.entries())
    .map(([category, res]) => computeTbSummary(category, res))
    .sort((a, b) => a.category.localeCompare(b.category));

  return {
    suiteName: results.suite_name,
    suiteVersion: results.suite_version,
    model: results.model,
    timestamp: results.timestamp,
    overall: computeTbSummary("overall", results.results),
    categories,
  };
};

export const formatTerminalBenchMarkdown = (report: TerminalBenchReport): string => {
  const lines: string[] = [];

  lines.push("# Terminal-Bench Report");
  lines.push("");
  lines.push(`- **Suite:** ${report.suiteName} (v${report.suiteVersion})`);
  lines.push(`- **Model:** ${report.model}`);
  lines.push(`- **Timestamp:** ${report.timestamp}`);
  lines.push("");

  const overall = report.overall;
  lines.push("## Overall Summary");
  lines.push("");
  lines.push("| Total | Passed | Failed | Timeout | Error | Skipped | Pass Rate | Avg Duration (ms) | Avg Turns | Total Tokens |");
  lines.push("|-------|--------|--------|---------|-------|---------|-----------|-------------------|-----------|--------------|");
  lines.push(
    `| ${overall.total} | ${overall.passed} | ${overall.failed} | ${overall.timeout} | ${overall.error} | ${overall.skipped} | ${(overall.passRate * 100).toFixed(1)}% | ${overall.avgDurationMs.toFixed(1)} | ${overall.avgTurns.toFixed(1)} | ${overall.totalTokens} |`,
  );
  lines.push("");

  if (report.categories.length > 0) {
    lines.push("## By Category");
    lines.push("");
    lines.push("| Category | Total | Passed | Failed | Timeout | Error | Skipped | Pass Rate | Avg Duration (ms) | Avg Turns | Total Tokens |");
    lines.push("|----------|-------|--------|--------|---------|-------|---------|-----------|-------------------|-----------|--------------|");
    for (const category of report.categories) {
      lines.push(
        `| ${category.category} | ${category.total} | ${category.passed} | ${category.failed} | ${category.timeout} | ${category.error} | ${category.skipped} | ${(category.passRate * 100).toFixed(1)}% | ${category.avgDurationMs.toFixed(1)} | ${category.avgTurns.toFixed(1)} | ${category.totalTokens} |`,
      );
    }
    lines.push("");
  }

  return lines.join("\n");
};

/**
 * Save report to file.
 */
export const saveReport = (
  content: string,
  outputPath: string,
): Effect.Effect<void, ReporterError, FileSystem.FileSystem | Path.Path> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;

    const dir = path.dirname(outputPath);
    yield* fs.makeDirectory(dir, { recursive: true }).pipe(
      Effect.mapError(
        (e) => new ReporterError("io_error", `Failed to create directory: ${e.message}`),
      ),
    );

    yield* fs.writeFile(outputPath, new TextEncoder().encode(content)).pipe(
      Effect.mapError(
        (e) => new ReporterError("io_error", `Failed to write report: ${e.message}`),
      ),
    );
  });
