/**
 * Baseline comparison system for Terminal-Bench.
 *
 * Tracks pass rate deltas, detects regressions, and reports improvements.
 * Stores baselines persistently for comparison across runs.
 *
 * Storage: .openagents/gym/baselines.jsonl
 */

import { existsSync, mkdirSync, readFileSync, appendFileSync, writeFileSync } from "fs";
import { join, dirname } from "path";
import * as S from "effect/Schema";
import type { TerminalBenchResults, TerminalBenchResult } from "./terminal-bench.js";

// --- Baseline Schema ---

export const BaselineRecord = S.Struct({
  id: S.String,
  model: S.String,
  suiteName: S.String,
  suiteVersion: S.String,
  timestamp: S.String,
  passRate: S.Number,
  passed: S.Number,
  total: S.Number,
  taskResults: S.Record({ key: S.String, value: S.Literal("pass", "fail", "timeout", "error", "skip") }),
  // Metadata
  gitCommit: S.optional(S.String),
  gitBranch: S.optional(S.String),
  notes: S.optional(S.String),
});
export type BaselineRecord = S.Schema.Type<typeof BaselineRecord>;

// --- Comparison Schema ---

export const TaskDelta = S.Struct({
  taskId: S.String,
  baseline: S.Literal("pass", "fail", "timeout", "error", "skip", "N/A"),
  current: S.Literal("pass", "fail", "timeout", "error", "skip"),
  changed: S.Boolean,
  improved: S.Boolean,
  regressed: S.Boolean,
});
export type TaskDelta = S.Schema.Type<typeof TaskDelta>;

export const BaselineComparison = S.Struct({
  comparedAt: S.String,
  baseline: S.Struct({
    id: S.String,
    timestamp: S.String,
    passRate: S.Number,
  }),
  current: S.Struct({
    passRate: S.Number,
    passed: S.Number,
    total: S.Number,
  }),
  // Summary
  passRateDelta: S.Number,
  passRateDeltaPercent: S.Number,
  improved: S.Boolean,
  regressed: S.Boolean,
  verdict: S.Literal("improved", "regressed", "unchanged", "mixed"),
  // Task-level changes
  taskDeltas: S.Array(TaskDelta),
  improvedTasks: S.Array(S.String),
  regressedTasks: S.Array(S.String),
  // Regression alerts
  regressionAlert: S.optional(S.Struct({
    severity: S.Literal("warning", "critical"),
    message: S.String,
    affectedTasks: S.Array(S.String),
  })),
});
export type BaselineComparison = S.Schema.Type<typeof BaselineComparison>;

// --- Baseline Store ---

export class BaselineStore {
  private readonly storePath: string;

  constructor(gymDir: string) {
    this.storePath = join(gymDir, "baselines.jsonl");

    // Ensure directory exists
    const dir = dirname(this.storePath);
    if (!existsSync(dir)) {
      mkdirSync(dir, { recursive: true });
    }
  }

  /**
   * Save a new baseline record.
   */
  async save(baseline: BaselineRecord): Promise<void> {
    const line = JSON.stringify(baseline) + "\n";
    appendFileSync(this.storePath, line);
  }

  /**
   * Load all baselines.
   */
  async loadAll(): Promise<BaselineRecord[]> {
    if (!existsSync(this.storePath)) {
      return [];
    }

    const content = readFileSync(this.storePath, "utf-8");
    const lines = content.trim().split("\n").filter(Boolean);

    const baselines: BaselineRecord[] = [];
    for (const line of lines) {
      try {
        const parsed = JSON.parse(line);
        const decoded = S.decodeUnknownSync(BaselineRecord)(parsed);
        baselines.push(decoded);
      } catch (e) {
        console.warn(`Failed to parse baseline line: ${e}`);
      }
    }

    return baselines;
  }

  /**
   * Get the most recent baseline for a model/suite combination.
   */
  async getBaseline(model: string, suiteName?: string): Promise<BaselineRecord | null> {
    const baselines = await this.loadAll();

    let filtered = baselines.filter((b) => b.model === model);
    if (suiteName) {
      filtered = filtered.filter((b) => b.suiteName === suiteName);
    }

    // Sort by timestamp descending
    filtered.sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());

    return filtered[0] ?? null;
  }

  /**
   * Get all baselines for a model.
   */
  async getModelBaselines(model: string): Promise<BaselineRecord[]> {
    const baselines = await this.loadAll();
    return baselines
      .filter((b) => b.model === model)
      .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());
  }

  /**
   * Get baseline history (for trend analysis).
   */
  async getHistory(options: {
    model?: string;
    suiteName?: string;
    limit?: number;
  } = {}): Promise<BaselineRecord[]> {
    let baselines = await this.loadAll();

    if (options.model) {
      baselines = baselines.filter((b) => b.model === options.model);
    }
    if (options.suiteName) {
      baselines = baselines.filter((b) => b.suiteName === options.suiteName);
    }

    // Sort by timestamp ascending for trend
    baselines.sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());

    if (options.limit) {
      baselines = baselines.slice(-options.limit);
    }

    return baselines;
  }

  /**
   * Clear all baselines (use with caution).
   */
  async clear(): Promise<void> {
    if (existsSync(this.storePath)) {
      writeFileSync(this.storePath, "");
    }
  }
}

// --- Comparison Functions ---

/**
 * Compare Terminal-Bench results against a baseline.
 */
export const compareWithBaseline = (
  results: TerminalBenchResults,
  baseline: BaselineRecord,
): BaselineComparison => {
  const currentPassRate = results.summary.pass_rate;
  const baselinePassRate = baseline.passRate;

  const passRateDelta = currentPassRate - baselinePassRate;
  const passRateDeltaPercent = baselinePassRate > 0
    ? (passRateDelta / baselinePassRate) * 100
    : currentPassRate > 0 ? 100 : 0;

  // Build task-level comparison
  const taskDeltas: TaskDelta[] = [];
  const improvedTasks: string[] = [];
  const regressedTasks: string[] = [];

  for (const result of results.results) {
    const baselineStatus: TerminalBenchResult["status"] | "N/A" =
      baseline.taskResults[result.task_id] ?? "N/A";
    const currentStatus = result.status;

    const changed = baselineStatus !== currentStatus && baselineStatus !== "N/A";
    const wasPass = baselineStatus === "pass";
    const isPass = currentStatus === "pass";

    let improved = false;
    let regressed = false;

    if (changed) {
      if (!wasPass && isPass) {
        improved = true;
        improvedTasks.push(result.task_id);
      } else if (wasPass && !isPass) {
        regressed = true;
        regressedTasks.push(result.task_id);
      }
    }

    taskDeltas.push({
      taskId: result.task_id,
      baseline: baselineStatus,
      current: currentStatus,
      changed,
      improved,
      regressed,
    });
  }

  // Determine verdict
  let verdict: BaselineComparison["verdict"];
  if (improvedTasks.length > 0 && regressedTasks.length === 0) {
    verdict = "improved";
  } else if (regressedTasks.length > 0 && improvedTasks.length === 0) {
    verdict = "regressed";
  } else if (improvedTasks.length > 0 && regressedTasks.length > 0) {
    verdict = "mixed";
  } else {
    verdict = "unchanged";
  }

  // Generate regression alert if needed
  let regressionAlert: BaselineComparison["regressionAlert"];
  if (regressedTasks.length > 0) {
    const severity = regressedTasks.length >= 3 || passRateDelta <= -0.1 ? "critical" : "warning";
    regressionAlert = {
      severity,
      message: severity === "critical"
        ? `Critical regression: ${regressedTasks.length} tasks regressed (${(passRateDelta * 100).toFixed(1)}% pass rate drop)`
        : `Regression detected: ${regressedTasks.length} task(s) regressed`,
      affectedTasks: regressedTasks,
    };
  }

  return {
    comparedAt: new Date().toISOString(),
    baseline: {
      id: baseline.id,
      timestamp: baseline.timestamp,
      passRate: baseline.passRate,
    },
    current: {
      passRate: currentPassRate,
      passed: results.summary.passed,
      total: results.summary.total,
    },
    passRateDelta,
    passRateDeltaPercent,
    improved: improvedTasks.length > 0,
    regressed: regressedTasks.length > 0,
    verdict,
    taskDeltas,
    improvedTasks,
    regressedTasks,
    ...(regressionAlert ? { regressionAlert } : {}),
  };
};

/**
 * Create a baseline record from Terminal-Bench results.
 */
export const createBaseline = (
  results: TerminalBenchResults,
  options: {
    gitCommit?: string;
    gitBranch?: string;
    notes?: string;
  } = {},
): BaselineRecord => {
  const taskResults: Record<string, TerminalBenchResult["status"]> = {};
  for (const result of results.results) {
    taskResults[result.task_id] = result.status;
  }

  const id = `baseline-${results.model.replace(/[^a-zA-Z0-9]/g, "-")}-${Date.now()}`;

  return {
    id,
    model: results.model,
    suiteName: results.suite_name,
    suiteVersion: results.suite_version,
    timestamp: results.timestamp,
    passRate: results.summary.pass_rate,
    passed: results.summary.passed,
    total: results.summary.total,
    taskResults,
    ...(options.gitCommit ? { gitCommit: options.gitCommit } : {}),
    ...(options.gitBranch ? { gitBranch: options.gitBranch } : {}),
    ...(options.notes ? { notes: options.notes } : {}),
  };
};

// --- Report Formatting ---

/**
 * Format baseline comparison as markdown.
 */
export const formatComparisonMarkdown = (comparison: BaselineComparison): string => {
  const lines: string[] = [];

  // Header with verdict
  const verdictEmoji: Record<string, string> = {
    improved: "✅",
    regressed: "❌",
    unchanged: "➖",
    mixed: "⚠️",
  };
  lines.push(`# Baseline Comparison: ${verdictEmoji[comparison.verdict]} ${comparison.verdict.toUpperCase()}`);
  lines.push("");
  lines.push(`Generated: ${comparison.comparedAt}`);
  lines.push("");

  // Regression alert
  if (comparison.regressionAlert) {
    const alertEmoji = comparison.regressionAlert.severity === "critical" ? "🚨" : "⚠️";
    lines.push(`## ${alertEmoji} ${comparison.regressionAlert.severity.toUpperCase()} Alert`);
    lines.push("");
    lines.push(comparison.regressionAlert.message);
    lines.push("");
    if (comparison.regressionAlert.affectedTasks.length > 0) {
      lines.push("**Affected tasks:**");
      for (const task of comparison.regressionAlert.affectedTasks) {
        lines.push(`- ${task}`);
      }
      lines.push("");
    }
  }

  // Summary
  lines.push("## Summary");
  lines.push("");
  lines.push("| Metric | Baseline | Current | Delta |");
  lines.push("|--------|----------|---------|-------|");
  lines.push(
    `| Pass Rate | ${(comparison.baseline.passRate * 100).toFixed(1)}% | ${(comparison.current.passRate * 100).toFixed(1)}% | ${comparison.passRateDelta >= 0 ? "+" : ""}${(comparison.passRateDelta * 100).toFixed(1)}% |`,
  );
  lines.push(
    `| Tasks Passed | - | ${comparison.current.passed}/${comparison.current.total} | - |`,
  );
  lines.push("");

  // Task changes
  if (comparison.improvedTasks.length > 0) {
    lines.push("## ✅ Improved Tasks");
    lines.push("");
    for (const task of comparison.improvedTasks) {
      const delta = comparison.taskDeltas.find((d) => d.taskId === task);
      lines.push(`- **${task}**: ${delta?.baseline} → ${delta?.current}`);
    }
    lines.push("");
  }

  if (comparison.regressedTasks.length > 0) {
    lines.push("## ❌ Regressed Tasks");
    lines.push("");
    for (const task of comparison.regressedTasks) {
      const delta = comparison.taskDeltas.find((d) => d.taskId === task);
      lines.push(`- **${task}**: ${delta?.baseline} → ${delta?.current}`);
    }
    lines.push("");
  }

  // Full task delta table
  const changedTasks = comparison.taskDeltas.filter((d) => d.changed);
  if (changedTasks.length > 0) {
    lines.push("## All Task Changes");
    lines.push("");
    lines.push("| Task | Baseline | Current | Status |");
    lines.push("|------|----------|---------|--------|");
    for (const delta of changedTasks) {
      const status = delta.improved ? "✅ Improved" : delta.regressed ? "❌ Regressed" : "➖ Changed";
      lines.push(`| ${delta.taskId} | ${delta.baseline} | ${delta.current} | ${status} |`);
    }
    lines.push("");
  }

  return lines.join("\n");
};

/**
 * Format pass rate trend as markdown.
 */
export const formatTrendMarkdown = (baselines: BaselineRecord[]): string => {
  if (baselines.length === 0) {
    return "# Pass Rate Trend\n\nNo baseline data available.";
  }

  const lines: string[] = [];
  lines.push("# Pass Rate Trend");
  lines.push("");
  lines.push(`Model: ${baselines[0].model}`);
  lines.push(`Suite: ${baselines[0].suiteName}`);
  lines.push(`Data points: ${baselines.length}`);
  lines.push("");

  // Calculate trend
  const first = baselines[0].passRate;
  const last = baselines[baselines.length - 1].passRate;
  const trend = last - first;
  const trendEmoji = trend > 0 ? "📈" : trend < 0 ? "📉" : "➖";

  lines.push(`## Overall Trend: ${trendEmoji} ${trend >= 0 ? "+" : ""}${(trend * 100).toFixed(1)}%`);
  lines.push("");

  // Table
  lines.push("| Date | Pass Rate | Passed | Total | Change |");
  lines.push("|------|-----------|--------|-------|--------|");

  let prevRate = 0;
  for (const baseline of baselines) {
    const change = baseline === baselines[0]
      ? "-"
      : `${baseline.passRate - prevRate >= 0 ? "+" : ""}${((baseline.passRate - prevRate) * 100).toFixed(1)}%`;
    const date = baseline.timestamp.slice(0, 10);
    lines.push(
      `| ${date} | ${(baseline.passRate * 100).toFixed(1)}% | ${baseline.passed} | ${baseline.total} | ${change} |`,
    );
    prevRate = baseline.passRate;
  }
  lines.push("");

  return lines.join("\n");
};

// --- Convenience Functions ---

/**
 * Compare results against stored baseline, auto-saving if no baseline exists.
 */
export const compareOrCreateBaseline = async (
  store: BaselineStore,
  results: TerminalBenchResults,
  options: {
    gitCommit?: string;
    gitBranch?: string;
    autoSave?: boolean;
  } = {},
): Promise<{
  comparison: BaselineComparison | null;
  baseline: BaselineRecord;
  isNewBaseline: boolean;
}> => {
  const existingBaseline = await store.getBaseline(results.model, results.suite_name);

  if (existingBaseline) {
    const comparison = compareWithBaseline(results, existingBaseline);
    return {
      comparison,
      baseline: existingBaseline,
      isNewBaseline: false,
    };
  }

  // No baseline exists, create one
  const newBaseline = createBaseline(results, options);
  if (options.autoSave !== false) {
    await store.save(newBaseline);
  }

  return {
    comparison: null,
    baseline: newBaseline,
    isNewBaseline: true,
  };
};

/**
 * Update baseline if current results are better.
 */
export const updateBaselineIfImproved = async (
  store: BaselineStore,
  results: TerminalBenchResults,
  options: {
    gitCommit?: string;
    gitBranch?: string;
    minImprovement?: number; // minimum pass rate improvement to update (default 0.01)
  } = {},
): Promise<{
  updated: boolean;
  comparison: BaselineComparison | null;
  newBaseline: BaselineRecord | null;
}> => {
  const { comparison, baseline, isNewBaseline } = await compareOrCreateBaseline(
    store,
    results,
    { ...options, autoSave: true },
  );

  if (isNewBaseline) {
    return {
      updated: true,
      comparison: null,
      newBaseline: baseline,
    };
  }

  if (!comparison) {
    return {
      updated: false,
      comparison: null,
      newBaseline: null,
    };
  }

  const minImprovement = options.minImprovement ?? 0.01;
  if (comparison.passRateDelta >= minImprovement) {
    const newBaseline = createBaseline(results, options);
    await store.save(newBaseline);
    return {
      updated: true,
      comparison,
      newBaseline,
    };
  }

  return {
    updated: false,
    comparison,
    newBaseline: null,
  };
};
