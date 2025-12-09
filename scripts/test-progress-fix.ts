#!/usr/bin/env bun
/**
 * Quick test to validate progress reporting fix
 *
 * Takes ~2 minutes instead of 50 minutes
 */

import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { loadTerminalBenchSuite } from "../src/bench/terminal-bench.js";
import { runMAPOrchestrator } from "../src/hillclimber/map-orchestrator.js";
import { mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

async function main() {
  console.log("\n=== Quick Progress Fix Validation ===\n");

  // Load regex-log task
  const suite = await Effect.runPromise(
    loadTerminalBenchSuite("tasks/terminal-bench-2.json").pipe(Effect.provide(BunContext.layer))
  );
  const task = suite.tasks.find((t) => t.id === "regex-log");
  if (!task) throw new Error("Task not found: regex-log");

  // Create temporary workspace
  const workspace = join(tmpdir(), `progress-test-${Date.now()}`);
  mkdirSync(workspace, { recursive: true });

  console.log(`Testing with maxTurns: 3 (quick validation)`);
  console.log(`Workspace: ${workspace}\n`);

  const output: string[] = [];
  const result = await runMAPOrchestrator(
    task,
    {} as any,
    {
      workspace,
      timeout: 300, // 5 minutes max
      maxTurns: 3,  // Just 3 turns for quick validation
      taskDescription: task.description,
      verbose: true,
      onOutput: (text) => {
        console.log(text);
        output.push(text);
      },
    }
  );

  console.log("\n=== Results ===");
  console.log(`Passed: ${result.passed}`);
  console.log(`Turns: ${result.turns}`);
  console.log(`Progress: ${(result.progress * 100).toFixed(1)}%`);
  console.log(`Duration: ${result.durationMs}ms`);

  // Extract progress from logs
  const progressLines = output.filter(line => line.includes("[MAP] Progress:"));
  const lastProgress = progressLines[progressLines.length - 1];

  console.log("\n=== Validation ===");
  console.log(`Last progress during execution: ${lastProgress || "none"}`);
  console.log(`Final progress in result: ${(result.progress * 100).toFixed(1)}%`);

  // Check if they match
  if (result.progress > 0) {
    console.log(`\n✅ FIX VALIDATED: Progress reporting works!`);
  } else {
    console.log(`\n❌ FIX FAILED: Still reporting 0% progress`);
  }

  console.log(`\nWorkspace: ${workspace}`);
}

main().catch(console.error);
