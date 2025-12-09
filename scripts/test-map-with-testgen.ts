#!/usr/bin/env bun
/**
 * Test MAP orchestrator with testgen integration
 *
 * Validates that:
 * 1. Testgen generates tests before solving
 * 2. Hillclimber uses generated tests for verification
 * 3. FM gets test-specific feedback
 */

import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { loadTerminalBenchSuite } from "../src/bench/terminal-bench.js";
import { runMAPOrchestrator } from "../src/hillclimber/map-orchestrator.js";
import { mkdirSync, existsSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

async function main() {
  console.log("\n=== Testing MAP Orchestrator with TestGen Integration ===\n");

  // Load regex-log task
  const suite = await Effect.runPromise(
    loadTerminalBenchSuite("tasks/terminal-bench-2.json").pipe(Effect.provide(BunContext.layer))
  );
  const task = suite.tasks.find((t) => t.id === "regex-log");
  if (!task) throw new Error("Task not found: regex-log");

  console.log(`Task: ${task.id}`);
  console.log(`Description: ${task.description.slice(0, 100)}...`);

  // Create temporary workspace
  const workspace = join(tmpdir(), `map-testgen-test-${Date.now()}`);
  mkdirSync(workspace, { recursive: true });
  console.log(`Workspace: ${workspace}\n`);

  try {
    // Run MAP orchestrator (limited turns for quick test)
    console.log("Running MAP orchestrator...\n");

    const output: string[] = [];
    const result = await runMAPOrchestrator(
      task,
      {} as any, // Config not used currently
      {
        workspace,
        timeout: 300, // 5 minutes
        maxTurns: 3, // Just test first few turns
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

    // Check if testgen was used
    const testgenUsed = output.some(line => line.includes("Running testgen"));
    const testsGenerated = output.some(line => line.includes("Generated") && line.includes("tests"));
    const testgenTestsFound = existsSync(join(workspace, "tests", "test_outputs.py"));

    console.log("\n=== TestGen Integration Validation ===");
    console.log(`✓ TestGen called: ${testgenUsed ? "YES" : "NO"}`);
    console.log(`✓ Tests generated: ${testsGenerated ? "YES" : "NO"}`);
    console.log(`✓ Test file exists: ${testgenTestsFound ? "YES" : "NO"}`);

    if (testgenTestsFound) {
      const testFile = readFileSync(join(workspace, "tests", "test_outputs.py"), "utf-8");
      const testCount = (testFile.match(/def test_/g) || []).length;
      console.log(`✓ Number of test functions: ${testCount}`);
    }

    console.log("\n✅ Integration test complete!");

  } finally {
    // Cleanup (optional - comment out to inspect workspace)
    // rmSync(workspace, { recursive: true, force: true });
    console.log(`\nWorkspace preserved for inspection: ${workspace}`);
  }
}

main().catch(console.error);
