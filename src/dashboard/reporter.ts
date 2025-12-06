/**
 * Dashboard Reporter
 *
 * Formats and outputs dashboard information.
 * Supports both terminal and structured output.
 */

import { Effect, Context, Layer } from "effect";
import type { LearningStats, LoopState } from "../learning/index.js";
import type {
  DashboardMetrics,
  DashboardState,
  DisplayMode,
  SystemHealth,
  RunRecord,
} from "./schema.js";
import {
  formatDuration,
  formatPercent,
  formatNumber,
  buildProgressBar,
  getHealthColor,
  resetColor,
  calculateHealth,
} from "./schema.js";

// --- Reporter Interface ---

export interface IReporter {
  /** Render overview to string */
  readonly renderOverview: (
    stats: LearningStats,
    loop: LoopState | null,
    mode: DisplayMode,
  ) => Effect.Effect<string, never>;

  /** Render skills view to string */
  readonly renderSkills: (
    stats: LearningStats,
    mode: DisplayMode,
  ) => Effect.Effect<string, never>;

  /** Render memory view to string */
  readonly renderMemory: (
    stats: LearningStats,
    mode: DisplayMode,
  ) => Effect.Effect<string, never>;

  /** Render training view to string */
  readonly renderTraining: (
    stats: LearningStats,
    loop: LoopState | null,
    mode: DisplayMode,
  ) => Effect.Effect<string, never>;

  /** Render full dashboard */
  readonly renderDashboard: (
    metrics: DashboardMetrics,
    state: DashboardState,
  ) => Effect.Effect<string, never>;

  /** Render progress update */
  readonly renderProgress: (
    loop: LoopState,
  ) => Effect.Effect<string, never>;

  /** Render history */
  readonly renderHistory: (
    records: RunRecord[],
    mode: DisplayMode,
  ) => Effect.Effect<string, never>;
}

// --- Service Tag ---

export class Reporter extends Context.Tag("Reporter")<Reporter, IReporter>() {}

// --- Implementation ---

const makeReporter = (): IReporter => {
  const header = (title: string): string => {
    const line = "═".repeat(50);
    return `╔${line}╗\n║ ${title.padEnd(48)} ║\n╚${line}╝`;
  };

  const section = (title: string): string => {
    return `\n${"─".repeat(50)}\n${title}\n${"─".repeat(50)}`;
  };

  const renderOverview = (
    stats: LearningStats,
    loop: LoopState | null,
    mode: DisplayMode,
  ): Effect.Effect<string, never> =>
    Effect.gen(function* () {
      if (mode === "json") {
        return JSON.stringify({ stats, loop }, null, 2);
      }

      const health = calculateHealth(stats);
      const lines: string[] = [];

      lines.push(header("MechaCoder Learning Dashboard"));
      lines.push("");

      // Health status
      const healthIcon = health.status === "healthy" ? "✓" : health.status === "degraded" ? "!" : "✗";
      lines.push(
        `Status: ${getHealthColor(health.status)}${healthIcon} ${health.status.toUpperCase()}${resetColor()}`,
      );

      // Loop status
      if (loop) {
        lines.push(`Loop: ${loop.status} (iteration ${loop.iteration})`);
        lines.push(
          `Progress: ${buildProgressBar({
            total: 100,
            completed: Math.round(loop.overallSuccessRate * 100),
          })}`,
        );
      } else {
        lines.push("Loop: not running");
      }

      lines.push("");

      // Quick stats
      lines.push(section("Quick Stats"));
      lines.push(`Skills:      ${stats.skills.total} (${stats.skills.learned} learned)`);
      lines.push(`Memories:    ${stats.memories.total}`);
      lines.push(`Runs:        ${stats.training.totalRuns}`);
      lines.push(`Tasks:       ${stats.training.totalTasksCompleted}`);
      lines.push(`Success:     ${formatPercent(stats.training.overallSuccessRate)}`);
      lines.push(`Tier:        ${stats.training.currentTier}`);

      if (mode === "detailed") {
        lines.push("");
        lines.push(section("Reflexion"));
        lines.push(`Failures:    ${stats.reflexion.totalFailures}`);
        lines.push(`Reflections: ${stats.reflexion.totalReflections}`);
        lines.push(`Successful:  ${stats.reflexion.successfulReflections}`);

        lines.push("");
        lines.push(section("Archivist"));
        lines.push(`Trajectories: ${stats.archivist.totalTrajectories}`);
        lines.push(`Patterns:     ${stats.archivist.patternsExtracted}`);
        lines.push(`Skills:       ${stats.archivist.skillsCreated}`);

        if (health.issues.length > 0) {
          lines.push("");
          lines.push(section("Issues"));
          for (const issue of health.issues) {
            lines.push(`  ⚠ ${issue}`);
          }
        }
      }

      return lines.join("\n");
    });

  const renderSkills = (
    stats: LearningStats,
    mode: DisplayMode,
  ): Effect.Effect<string, never> =>
    Effect.gen(function* () {
      if (mode === "json") {
        return JSON.stringify(stats.skills, null, 2);
      }

      const lines: string[] = [];
      lines.push(header("Skill Library"));
      lines.push("");
      lines.push(`Total Skills:      ${stats.skills.total}`);
      lines.push(`Bootstrapped:      ${stats.skills.bootstrapped}`);
      lines.push(`Learned:           ${stats.skills.learned}`);
      lines.push("");
      lines.push(section("By Category"));

      for (const [category, count] of Object.entries(stats.skills.byCategory)) {
        const bar = buildProgressBar({
          total: stats.skills.total,
          completed: count,
          width: 20,
        });
        lines.push(`${category.padEnd(15)} ${bar} ${count}`);
      }

      return lines.join("\n");
    });

  const renderMemory = (
    stats: LearningStats,
    mode: DisplayMode,
  ): Effect.Effect<string, never> =>
    Effect.gen(function* () {
      if (mode === "json") {
        return JSON.stringify(stats.memories, null, 2);
      }

      const lines: string[] = [];
      lines.push(header("Memory System"));
      lines.push("");
      lines.push(`Total Memories:    ${stats.memories.total}`);
      lines.push("");

      const total = stats.memories.total || 1;
      lines.push(
        `Episodic:   ${buildProgressBar({ total, completed: stats.memories.episodic, width: 20 })} ${stats.memories.episodic}`,
      );
      lines.push(
        `Semantic:   ${buildProgressBar({ total, completed: stats.memories.semantic, width: 20 })} ${stats.memories.semantic}`,
      );
      lines.push(
        `Procedural: ${buildProgressBar({ total, completed: stats.memories.procedural, width: 20 })} ${stats.memories.procedural}`,
      );

      return lines.join("\n");
    });

  const renderTraining = (
    stats: LearningStats,
    loop: LoopState | null,
    mode: DisplayMode,
  ): Effect.Effect<string, never> =>
    Effect.gen(function* () {
      if (mode === "json") {
        return JSON.stringify({ training: stats.training, loop }, null, 2);
      }

      const lines: string[] = [];
      lines.push(header("Training Progress"));
      lines.push("");

      if (loop) {
        lines.push(`Status:        ${loop.status}`);
        lines.push(`Iteration:     ${loop.iteration}`);
        lines.push(`Current Tier:  ${loop.currentSubset}`);
        lines.push(`Duration:      ${formatDuration(loop.totalDurationMs)}`);
        lines.push("");
        lines.push(
          `Success Rate:  ${buildProgressBar({
            total: 100,
            completed: Math.round(loop.overallSuccessRate * 100),
            width: 30,
          })}`,
        );
        lines.push("");
        lines.push(`Tasks:         ${loop.totalTasksCompleted}`);
        lines.push(`Successful:    ${loop.totalSuccessful}`);
        lines.push(`Skills:        ${loop.skillsLearned}`);
        lines.push(`Patterns:      ${loop.patternsExtracted}`);
      } else {
        lines.push("No active training loop");
        lines.push("");
        lines.push(`Historical Runs: ${stats.training.totalRuns}`);
        lines.push(`Total Tasks:     ${stats.training.totalTasksCompleted}`);
        lines.push(`Success Rate:    ${formatPercent(stats.training.overallSuccessRate)}`);
      }

      return lines.join("\n");
    });

  const renderProgress = (loop: LoopState): Effect.Effect<string, never> =>
    Effect.gen(function* () {
      const bar = buildProgressBar({
        total: 100,
        completed: Math.round(loop.overallSuccessRate * 100),
        width: 30,
      });

      return [
        `[${loop.status}] Iteration ${loop.iteration} | ${loop.currentSubset}`,
        bar,
        `Tasks: ${loop.totalTasksCompleted} | Success: ${formatPercent(loop.overallSuccessRate)} | Skills: ${loop.skillsLearned}`,
      ].join("\n");
    });

  const renderHistory = (
    records: RunRecord[],
    mode: DisplayMode,
  ): Effect.Effect<string, never> =>
    Effect.gen(function* () {
      if (mode === "json") {
        return JSON.stringify(records, null, 2);
      }

      const lines: string[] = [];
      lines.push(header("Run History"));
      lines.push("");

      if (records.length === 0) {
        lines.push("No runs recorded yet.");
        return lines.join("\n");
      }

      // Table header
      lines.push(
        "ID".padEnd(15) +
          "Subset".padEnd(8) +
          "Success".padEnd(10) +
          "Tasks".padEnd(8) +
          "Duration".padEnd(12) +
          "Date",
      );
      lines.push("─".repeat(70));

      for (const record of records.slice(0, 20)) {
        lines.push(
          record.id.slice(0, 12).padEnd(15) +
            record.subset.padEnd(8) +
            formatPercent(record.successRate).padEnd(10) +
            record.tasksCompleted.toString().padEnd(8) +
            formatDuration(record.durationMs).padEnd(12) +
            new Date(record.timestamp).toLocaleDateString(),
        );
      }

      if (records.length > 20) {
        lines.push(`... and ${records.length - 20} more`);
      }

      return lines.join("\n");
    });

  const renderDashboard = (
    metrics: DashboardMetrics,
    state: DashboardState,
  ): Effect.Effect<string, never> =>
    Effect.gen(function* () {
      switch (state.currentView) {
        case "overview":
          return yield* renderOverview(metrics.stats, metrics.loop, state.mode);
        case "skills":
          return yield* renderSkills(metrics.stats, state.mode);
        case "memory":
          return yield* renderMemory(metrics.stats, state.mode);
        case "training":
          return yield* renderTraining(metrics.stats, metrics.loop, state.mode);
        default:
          return yield* renderOverview(metrics.stats, metrics.loop, state.mode);
      }
    });

  return {
    renderOverview,
    renderSkills,
    renderMemory,
    renderTraining,
    renderDashboard,
    renderProgress,
    renderHistory,
  };
};

// --- Layer ---

export const ReporterLive: Layer.Layer<Reporter, never, never> = Layer.succeed(
  Reporter,
  makeReporter(),
);
