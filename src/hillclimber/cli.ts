#!/usr/bin/env bun
/**
 * HillClimber CLI
 *
 * Overnight optimization loop for Terminal-Bench tasks.
 * Uses Apple FM for task execution and OpenRouter for meta-reasoning.
 *
 * Usage:
 *   bun run hillclimber                    # Run on default tasks
 *   bun run hillclimber --task regex-log   # Single task mode
 *   bun run hillclimber --max-runs 100     # Limit runs
 *   bun run hillclimber --sleep 30000      # 30s between runs
 *   bun run hillclimber --stats            # Show stats and exit
 *   bun run hillclimber --export           # Export learned hints
 *   bun run hillclimber --dry-run          # Preview without executing
 */

import { parseArgs } from "util";
import { runHillClimber, showStats, dryRun } from "./runner.js";
import { runExport, generateHintsCode } from "./exporter.js";
import { Effect } from "effect";
import { HillClimberStoreLive } from "./store.js";
import type { HillClimberOptions } from "./types.js";
import { getLogPath } from "./logger.js";

// ============================================================================
// Default Values
// ============================================================================

const DEFAULT_SUITE_PATH = "tasks/terminal-bench-2.json";
const DEFAULT_MAX_RUNS = 100;
const DEFAULT_SLEEP_MS = 30000; // 30 seconds

// ============================================================================
// CLI Parsing
// ============================================================================

const USAGE = `
HillClimber - Overnight TB optimization loop

Usage:
  bun run hillclimber [options]

Options:
  --task, -t <id>       Task ID to optimize (can specify multiple)
  --max-runs, -n <num>  Maximum number of runs (default: ${DEFAULT_MAX_RUNS})
  --sleep, -s <ms>      Sleep between runs in ms (default: ${DEFAULT_SLEEP_MS})
  --suite <path>        Path to TB suite JSON (default: ${DEFAULT_SUITE_PATH})
  --model, -m <model>   Override model to use (default: uses FREE_MODELS[0])
  --map                 Use new MAP orchestrator (iterative verification)
  --verbose, -v         Enable verbose logging
  --stats               Show current stats and exit
  --export              Export learned hints and exit
  --export-code         Generate TypeScript code for hints
  --dry-run             Preview what would happen without executing
  --help, -h            Show this help message

Examples:
  # Run overnight on default tasks
  bun run hillclimber --max-runs 500 --sleep 30000

  # Optimize a single task with MAP architecture
  bun run hillclimber --task regex-log --max-runs 50 --map

  # Multiple tasks with verbose output
  bun run hillclimber --task regex-log --task word-count --map --verbose

  # Check progress
  bun run hillclimber --stats

  # Export learned hints
  bun run hillclimber --export
`;

const parseCliArgs = (): {
  options: HillClimberOptions;
  showStatsOnly: boolean;
  exportOnly: boolean;
  exportCode: boolean;
  help: boolean;
} => {
  const { values, positionals } = parseArgs({
    args: process.argv.slice(2),
    options: {
      task: {
        type: "string",
        short: "t",
        multiple: true,
        default: [],
      },
      "max-runs": {
        type: "string",
        short: "n",
        default: String(DEFAULT_MAX_RUNS),
      },
      sleep: {
        type: "string",
        short: "s",
        default: String(DEFAULT_SLEEP_MS),
      },
      suite: {
        type: "string",
        default: DEFAULT_SUITE_PATH,
      },
      model: {
        type: "string",
        short: "m",
        default: "",
      },
      stats: {
        type: "boolean",
        default: false,
      },
      export: {
        type: "boolean",
        default: false,
      },
      "export-code": {
        type: "boolean",
        default: false,
      },
      "dry-run": {
        type: "boolean",
        default: false,
      },
      map: {
        type: "boolean",
        default: false,
      },
      verbose: {
        type: "boolean",
        short: "v",
        default: false,
      },
      help: {
        type: "boolean",
        short: "h",
        default: false,
      },
    },
    allowPositionals: true,
  });

  // Handle positional args as task IDs
  const tasks = [...(values.task as string[]), ...positionals];

  const modelOverride = values.model as string;
  return {
    options: {
      tasks,
      maxRuns: parseInt(values["max-runs"] as string, 10),
      sleepMs: parseInt(values.sleep as string, 10),
      suitePath: values.suite as string,
      ...(modelOverride ? { modelOverride } : {}),
      dryRun: values["dry-run"] as boolean,
      showStats: false,
      exportHints: false,
      useMAP: values.map as boolean,
      verbose: values.verbose as boolean,
    },
    showStatsOnly: values.stats as boolean,
    exportOnly: values.export as boolean,
    exportCode: values["export-code"] as boolean,
    help: values.help as boolean,
  };
};

// ============================================================================
// Main
// ============================================================================

const main = async (): Promise<void> => {
  const { options, showStatsOnly, exportOnly, exportCode, help } = parseCliArgs();

  if (help) {
    console.log(USAGE);
    process.exit(0);
  }

  console.log(`
╔═══════════════════════════════════════════════════════════╗
║              HillClimber - TB Optimization                ║
╚═══════════════════════════════════════════════════════════╝
`);
  console.log(`[HillClimber] Logs: ${getLogPath()}`);

  // Stats only mode
  if (showStatsOnly) {
    await showStats();
    process.exit(0);
  }

  // Export only mode
  if (exportOnly) {
    await runExport();
    process.exit(0);
  }

  // Export code mode
  if (exportCode) {
    const program = generateHintsCode().pipe(
      Effect.provide(HillClimberStoreLive),
    );
    const code = await Effect.runPromise(program);
    console.log(code);
    process.exit(0);
  }

  // Dry run mode
  if (options.dryRun) {
    await dryRun(options);
    process.exit(0);
  }

  // Main optimization loop
  try {
    await runHillClimber(options);
  } catch (e) {
    console.error(
      `[HillClimber] Fatal error: ${e instanceof Error ? e.message : String(e)}`,
    );
    process.exit(1);
  }
};

// Run
main().catch((e) => {
  console.error(`Unhandled error: ${e}`);
  process.exit(1);
});
