#!/usr/bin/env bun
/**
 * Healer CLI
 *
 * Commands for managing and invoking the Healer subagent.
 *
 * Usage:
 *   bun run healer:scan [options]     - Scan for stuck tasks/subtasks
 *   bun run healer:invoke [options]   - Manually invoke Healer
 */
import { parseArgs } from "util";
import { Effect } from "effect";
import { FileSystem, Path } from "@effect/platform";
import { BunContext } from "@effect/platform-bun";
import { listTasks } from "../tasks/service.js";
import {
  detectStuck,
  summarizeStuckDetection,
  type StuckDetectionConfig,
} from "./stuck.js";
import type { Subtask } from "../agent/orchestrator/types.js";

const runEffect = <A, E>(
  effect: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path>
) => Effect.runPromise(effect.pipe(Effect.provide(BunContext.layer)));

// ============================================================================
// CLI Argument Parsing
// ============================================================================

const { values, positionals } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    help: { type: "boolean", short: "h" },
    json: { type: "boolean", short: "j" },
    "task-hours": { type: "string", default: "4" },
    "subtask-hours": { type: "string", default: "2" },
    "min-failures": { type: "string", default: "3" },
    verbose: { type: "boolean", short: "v" },
    "dry-run": { type: "boolean" },
  },
  allowPositionals: true,
});

const command = positionals[0] ?? "help";

// ============================================================================
// Help
// ============================================================================

const printHelp = () => {
  console.log(`
Healer CLI - Self-healing subagent management

USAGE:
  bun run healer:scan [options]     Scan for stuck tasks and subtasks
  bun run healer:invoke [options]   Manually invoke Healer (not yet implemented)

SCAN OPTIONS:
  --task-hours <N>      Hours before task is stuck (default: 4)
  --subtask-hours <N>   Hours before subtask is stuck (default: 2)
  --min-failures <N>    Min consecutive failures (default: 3)
  --json, -j            Output as JSON
  --verbose, -v         Show detailed output
  --dry-run             Show what would be done without acting

EXAMPLES:
  bun run healer:scan                    # Scan with defaults
  bun run healer:scan --task-hours 8     # Use 8 hour threshold
  bun run healer:scan --json             # JSON output for scripting
  bun run healer:scan --verbose          # Detailed output
`);
};

// ============================================================================
// Scan Command
// ============================================================================

const runScan = async () => {
  const config: StuckDetectionConfig = {
    stuckTaskThresholdHours: parseInt(values["task-hours"] ?? "4", 10),
    stuckSubtaskThresholdHours: parseInt(values["subtask-hours"] ?? "2", 10),
    minConsecutiveFailures: parseInt(values["min-failures"] ?? "3", 10),
    scanTrajectories: true,
  };

  const verbose = values.verbose ?? false;
  const jsonOutput = values.json ?? false;

  if (verbose && !jsonOutput) {
    console.log("Healer Stuck Detection Scan");
    console.log("===========================");
    console.log(`Task threshold: ${config.stuckTaskThresholdHours} hours`);
    console.log(`Subtask threshold: ${config.stuckSubtaskThresholdHours} hours`);
    console.log(`Min failures: ${config.minConsecutiveFailures}`);
    console.log("");
  }

  // Load tasks
  const projectRoot = process.cwd();
  const openagentsDir = `${projectRoot}/.openagents`;
  const tasksPath = `${openagentsDir}/tasks.jsonl`;

  const tasks = await runEffect(listTasks(tasksPath));

  // Gather subtasks from tasks (we don't have direct subtask access without orchestrator state)
  // For CLI scanning, we'll scan tasks that are in_progress
  const subtasks: Array<{ subtask: Subtask; taskId: string }> = [];

  // Note: In a full implementation, we would load subtask state from orchestrator
  // For now, we scan based on task-level information

  // Run detection
  const result = detectStuck(tasks, subtasks, [], config);

  if (jsonOutput) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    console.log(summarizeStuckDetection(result));

    if (result.stuckTasks.length > 0 || result.stuckSubtasks.length > 0) {
      console.log("\nTo invoke Healer for stuck items:");
      console.log("  bun run healer:invoke --scenario SubtaskStuck --task <task-id>");
    }
  }

  // Return appropriate exit code
  if (result.stuckTasks.length > 0 || result.stuckSubtasks.length > 0) {
    process.exit(1); // Exit with error to signal stuck items found
  }
};

// ============================================================================
// Invoke Command (placeholder)
// ============================================================================

const runInvoke = async () => {
  console.log("healer:invoke is not yet implemented.");
  console.log("Healer is automatically invoked by the orchestrator when failures occur.");
  console.log("");
  console.log("To trigger Healer manually, you can:");
  console.log("  1. Run the orchestrator with a stuck task");
  console.log("  2. Use the healer:scan command to identify stuck items");
  process.exit(1);
};

// ============================================================================
// Main
// ============================================================================

const main = async () => {
  if (values.help) {
    printHelp();
    process.exit(0);
  }

  switch (command) {
    case "scan":
      await runScan();
      break;

    case "invoke":
      await runInvoke();
      break;

    case "help":
    default:
      printHelp();
      break;
  }
};

main().catch((error) => {
  console.error("Error:", error.message);
  process.exit(1);
});
