#!/usr/bin/env bun
/**
 * Test script for regex-log with visibility improvements
 *
 * Usage:
 *   bun scripts/test-progress-fix.ts          # quick mode (3 turns, 5 min timeout)
 *   bun scripts/test-progress-fix.ts --standard  # standard mode (10 turns, 15 min timeout)
 *   bun scripts/test-progress-fix.ts --full      # full mode (25 turns, 45 min timeout)
 */

import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { loadTerminalBenchSuite } from "../src/bench/terminal-bench.js";
import { runMAPOrchestrator } from "../src/hillclimber/map-orchestrator.js";
import { mkdirSync, createWriteStream, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";

// Mode configurations
const MODES = {
  quick: { turns: 3, timeoutSec: 300, name: "quick" },       // 5 min
  standard: { turns: 10, timeoutSec: 900, name: "standard" }, // 15 min
  full: { turns: 25, timeoutSec: 2700, name: "full" },        // 45 min
};

// Parse CLI args
const args = process.argv.slice(2);
const modeArg = args.find((a) => a.startsWith("--"));
const modeName = modeArg?.replace("--", "") ?? "quick";
const mode = MODES[modeName as keyof typeof MODES] ?? MODES.quick;

// Setup streaming log file
const logDir = join(process.cwd(), "logs");
if (!existsSync(logDir)) mkdirSync(logDir, { recursive: true });
const logFile = join(logDir, `live-run-${Date.now()}.log`);
const logStream = createWriteStream(logFile);

// Timestamp logger that writes to console AND file
const log = (msg: string) => {
  const line = `[${new Date().toISOString()}] ${msg}`;
  console.log(line);
  logStream.write(line + "\n");
};

// Global watchdog timeout
let watchdogTriggered = false;
const watchdogTimeout = setTimeout(() => {
  watchdogTriggered = true;
  log(`[WATCHDOG] ⚠️ TIMEOUT after ${mode.timeoutSec}s - forcing exit`);
  log(`[WATCHDOG] Check log file for details: ${logFile}`);
  logStream.end();
  process.exit(1);
}, mode.timeoutSec * 1000);

// Clear watchdog on normal exit
const clearWatchdog = () => {
  if (!watchdogTriggered) {
    clearTimeout(watchdogTimeout);
  }
};

async function main() {
  log(`\n=== Regex-Log Test (${mode.name} mode) ===`);
  log(`Turns: ${mode.turns}, Timeout: ${mode.timeoutSec}s`);
  log(`Log file: ${logFile}\n`);

  // Load regex-log task
  log("[INIT] Loading Terminal-Bench suite...");
  const suite = await Effect.runPromise(
    loadTerminalBenchSuite("tasks/terminal-bench-2.json").pipe(Effect.provide(BunContext.layer))
  );
  const task = suite.tasks.find((t) => t.id === "regex-log");
  if (!task) throw new Error("Task not found: regex-log");
  log("[INIT] Task loaded: regex-log");

  // Create temporary workspace
  const workspace = join(tmpdir(), `regex-test-${Date.now()}`);
  mkdirSync(workspace, { recursive: true });
  log(`[INIT] Workspace: ${workspace}`);

  const startTime = Date.now();
  const output: string[] = [];

  log(`\n[START] Running MAP orchestrator...`);

  try {
    const result = await runMAPOrchestrator(
      task,
      {} as any,
      {
        workspace,
        timeout: mode.timeoutSec,
        maxTurns: mode.turns,
        taskDescription: task.description,
        verbose: false, // Reduce verbosity - heartbeat provides visibility
        onOutput: (text) => {
          const trimmed = text.trim();
          if (trimmed) {
            log(trimmed);
            output.push(trimmed);
          }
        },
      }
    );

    const duration = Date.now() - startTime;

    log(`\n=== RESULTS ===`);
    log(`Passed: ${result.passed}`);
    log(`Turns: ${result.turns}/${mode.turns}`);
    log(`Progress: ${(result.progress * 100).toFixed(1)}%`);
    log(`Duration: ${(duration / 1000).toFixed(1)}s`);

    if (result.error) {
      log(`Error: ${result.error}`);
    }

    // Extract progress from logs for validation
    const progressLines = output.filter(line => line.includes("[MAP] Progress:"));
    const lastProgress = progressLines[progressLines.length - 1];

    log(`\n=== VALIDATION ===`);
    log(`Last progress during execution: ${lastProgress || "none"}`);
    log(`Final progress in result: ${(result.progress * 100).toFixed(1)}%`);

    // Check if progress fix works
    if (result.progress > 0) {
      log(`\n✅ PASS: Progress reporting works! ${(result.progress * 100).toFixed(1)}%`);
    } else if (progressLines.length > 0) {
      log(`\n⚠️ PARTIAL: Progress tracked during run but not in final result`);
    } else {
      log(`\n❌ FAIL: No progress detected`);
    }

    log(`\n[DONE] Workspace: ${workspace}`);
    log(`[DONE] Log file: ${logFile}`);

  } catch (error) {
    log(`\n[ERROR] ${error instanceof Error ? error.message : String(error)}`);
    throw error;
  } finally {
    clearWatchdog();
    logStream.end();
  }
}

// Handle uncaught errors
process.on("uncaughtException", (error) => {
  log(`[FATAL] Uncaught exception: ${error.message}`);
  clearWatchdog();
  logStream.end();
  process.exit(1);
});

main().catch((error) => {
  log(`[FATAL] ${error.message || error}`);
  clearWatchdog();
  logStream.end();
  process.exit(1);
});
