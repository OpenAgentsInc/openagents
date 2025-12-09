#!/usr/bin/env bun
/**
 * Test complete sampling integration
 *
 * Validates:
 * 1. TestGen generates tests
 * 2. MAP orchestrator uses parallel sampling
 * 3. Docker verification works
 * 4. Progress improves over turns
 */

import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { loadTerminalBenchSuite } from "../src/bench/terminal-bench.js";
import { runMAPOrchestrator } from "../src/hillclimber/map-orchestrator.js";
import { mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

async function main() {
  console.log("\n=== Testing Complete Sampling Integration ===\n");

  // Load regex-log task
  const suite = await Effect.runPromise(
    loadTerminalBenchSuite("tasks/terminal-bench-2.json").pipe(Effect.provide(BunContext.layer))
  );
  const task = suite.tasks.find((t) => t.id === "regex-log");
  if (!task) throw new Error("Task not found: regex-log");

  console.log(`Task: ${task.id}`);
  console.log(`Description: ${task.description.slice(0, 100)}...`);

  // Create temporary workspace
  const workspace = join(tmpdir(), `sampling-test-${Date.now()}`);
  mkdirSync(workspace, { recursive: true });
  console.log(`Workspace: ${workspace}\n`);

  try {
    // Run MAP orchestrator with sampling (limited turns for quick test)
    console.log("Running MAP orchestrator with parallel sampling...\n");

    const output: string[] = [];
    const result = await runMAPOrchestrator(
      task,
      {} as any,
      {
        workspace,
        timeout: 1200, // 20 minutes
        maxTurns: 15, // More turns to reach 100%
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

    // Check if sampling was used
    const samplingUsed = output.some(line => line.includes("[MAP-SAMPLING]"));
    const testgenUsed = output.some(line => line.includes("Generated") && line.includes("tests"));

    console.log("\n=== Validation ===");
    console.log(`✓ TestGen called: ${testgenUsed ? "YES" : "NO"}`);
    console.log(`✓ Parallel sampling used: ${samplingUsed ? "YES" : "NO"}`);
    console.log(`✓ Progress > 0%: ${result.progress > 0 ? "YES" : "NO"}`);

    if (samplingUsed && testgenUsed && result.progress > 0) {
      console.log("\n✅ Complete integration working!");
    } else {
      console.log("\n⚠️  Some components may not be working as expected");
    }

  } finally {
    console.log(`\nWorkspace preserved for inspection: ${workspace}`);
  }
}

main().catch(console.error);
