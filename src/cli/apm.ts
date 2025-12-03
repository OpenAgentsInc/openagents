#!/usr/bin/env bun
/**
 * APM CLI - Display Actions Per Minute statistics
 *
 * Usage:
 *   bun src/cli/apm.ts              # Show all APM stats
 *   bun src/cli/apm.ts --project .  # Show APM for current project
 *   bun src/cli/apm.ts --json       # Output as JSON
 *
 * @see docs/apm.md for specification
 */

import * as BunContext from "@effect/platform-bun/BunContext";
import * as Effect from "effect/Effect";
import * as path from "node:path";
import {
  parseClaudeConversations,
  parseProjectConversations,
} from "../agent/apm-parser.js";
import type { APMStats, APMBySource } from "../agent/apm.js";

// --- CLI Argument Parsing ---

interface CliArgs {
  project?: string | undefined;
  json: boolean;
  help: boolean;
}

const parseArgs = (): CliArgs => {
  const args = process.argv.slice(2);
  let project: string | undefined;
  let json = false;
  let help = false;

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === "--help" || arg === "-h") {
      help = true;
    } else if (arg === "--json") {
      json = true;
    } else if ((arg === "--project" || arg === "-p") && args[i + 1]) {
      project = args[i + 1];
      if (project === ".") {
        project = process.cwd();
      } else if (project.startsWith("~")) {
        project = project.replace("~", process.env.HOME || "");
      } else if (!path.isAbsolute(project)) {
        project = path.resolve(project);
      }
      i++;
    }
  }

  return { project, json, help };
};

// --- Formatting Functions ---

const formatAPM = (apm: number): string => {
  return apm.toFixed(3);
};

const formatDuration = (minutes: number): string => {
  if (minutes < 60) {
    return `${minutes.toFixed(0)}m`;
  }
  const hours = Math.floor(minutes / 60);
  const mins = Math.round(minutes % 60);
  if (hours < 24) {
    return `${hours}h ${mins}m`;
  }
  const days = Math.floor(hours / 24);
  const remainingHours = hours % 24;
  return `${days}d ${remainingHours}h`;
};

const printStatsTable = (label: string, stats: APMStats): void => {
  console.log(`\n## ${label}`);
  console.log("─".repeat(50));

  // Time window APMs
  console.log("\n### APM by Time Window");
  console.log(`  1 hour:   ${formatAPM(stats.apm1h)}`);
  console.log(`  6 hours:  ${formatAPM(stats.apm6h)}`);
  console.log(`  1 day:    ${formatAPM(stats.apm1d)}`);
  console.log(`  1 week:   ${formatAPM(stats.apm1w)}`);
  console.log(`  1 month:  ${formatAPM(stats.apm1m)}`);
  console.log(`  Lifetime: ${formatAPM(stats.apmLifetime)}`);

  // Totals
  console.log("\n### Totals");
  console.log(`  Sessions:     ${stats.totalSessions}`);
  console.log(`  Messages:     ${stats.totalMessages}`);
  console.log(`  Tool Calls:   ${stats.totalToolCalls}`);
  console.log(`  Duration:     ${formatDuration(stats.totalDurationMinutes)}`);

  // Productivity by time of day
  console.log("\n### Productivity by Time of Day");
  console.log(`  Morning (6am-12pm):   ${formatAPM(stats.productivityByTime.morning)}`);
  console.log(`  Afternoon (12pm-6pm): ${formatAPM(stats.productivityByTime.afternoon)}`);
  console.log(`  Evening (6pm-12am):   ${formatAPM(stats.productivityByTime.evening)}`);
  console.log(`  Night (12am-6am):     ${formatAPM(stats.productivityByTime.night)}`);

  // Recent sessions
  if (stats.recentSessions.length > 0) {
    console.log("\n### Recent Sessions (last 5)");
    for (const session of stats.recentSessions.slice(-5)) {
      const date = new Date(session.timestamp).toLocaleString();
      console.log(
        `  ${date} | APM: ${formatAPM(session.apm)} | ${session.project}`,
      );
    }
  }
};

const printComparison = (stats: APMBySource): void => {
  console.log("\n## Comparison: MechaCoder vs Claude Code");
  console.log("═".repeat(50));

  const ccAPM = stats.claudeCode.apmLifetime;
  const mcAPM = stats.mechaCoder.apmLifetime;
  const delta = stats.comparison.apmDelta;
  const ratio = stats.comparison.efficiencyRatio;

  console.log(`\n  Claude Code APM:   ${formatAPM(ccAPM)}`);
  console.log(`  MechaCoder APM:    ${formatAPM(mcAPM)}`);
  console.log(`  Delta:             ${delta >= 0 ? "+" : ""}${formatAPM(delta)}`);
  if (ccAPM > 0) {
    const percentage = ((ratio - 1) * 100).toFixed(1);
    console.log(
      `  Efficiency:        ${formatAPM(ratio)}x (${Number(percentage) >= 0 ? "+" : ""}${percentage}%)`,
    );
  }

  // Session counts
  console.log(`\n  Claude Code sessions: ${stats.claudeCode.totalSessions}`);
  console.log(`  MechaCoder sessions:  ${stats.mechaCoder.totalSessions}`);
};

const printHelp = (): void => {
  console.log(`
APM CLI - Actions Per Minute Statistics

Usage:
  bun src/cli/apm.ts [options]

Options:
  --help, -h          Show this help message
  --project, -p PATH  Show APM for a specific project (use "." for current)
  --json              Output as JSON

Examples:
  bun src/cli/apm.ts                    # Show all APM stats
  bun src/cli/apm.ts --project .        # Show APM for current project
  bun src/cli/apm.ts --json             # Output as JSON

APM Formula:
  APM = (messages + tool_calls) / duration_minutes

Data Source:
  ~/.claude/projects/<project>/*.jsonl

Time Windows:
  - 1h, 6h, 1d, 1w, 1m: Actions in window / window minutes
  - Lifetime: All actions / wall-clock time from first to last session
`);
};

// --- Main Program ---

const program = Effect.gen(function* () {
  const args = parseArgs();

  if (args.help) {
    printHelp();
    return;
  }

  console.log("# APM Statistics");
  console.log(`Generated: ${new Date().toISOString()}`);
  console.log("═".repeat(50));

  let stats: APMBySource;

  if (args.project) {
    console.log(`\nProject: ${args.project}`);
    stats = yield* parseProjectConversations(args.project);
  } else {
    stats = yield* parseClaudeConversations;
  }

  if (args.json) {
    console.log(JSON.stringify(stats, null, 2));
    return;
  }

  // Print comparison first (most interesting)
  if (stats.mechaCoder.totalSessions > 0 && stats.claudeCode.totalSessions > 0) {
    printComparison(stats);
  }

  // Print combined stats
  printStatsTable("Combined (All Sources)", stats.combined);

  // Print breakdown by source
  if (stats.claudeCode.totalSessions > 0) {
    printStatsTable("Claude Code (Direct)", stats.claudeCode);
  }

  if (stats.mechaCoder.totalSessions > 0) {
    printStatsTable("MechaCoder (Autonomous)", stats.mechaCoder);
  }

  // Summary line
  console.log("\n" + "═".repeat(50));
  console.log(
    `Lifetime APM: ${formatAPM(stats.combined.apmLifetime)} (${stats.combined.totalSessions} sessions, ${stats.combined.totalMessages + stats.combined.totalToolCalls} actions)`,
  );
});

// Run
Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)))
  .then(() => {
    process.exit(0);
  })
  .catch((error) => {
    console.error("Error:", error);
    process.exit(1);
  });
