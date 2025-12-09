#!/usr/bin/env bun
/**
 * Test testgen integration - validates flow end-to-end
 */

import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { loadTerminalBenchSuite } from "../src/bench/terminal-bench.js";
import { runTestGenForTask } from "../src/hillclimber/testgen-integration.js";
import { mkdirSync, existsSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

async function main() {
  console.log("\n=== Testing TestGen Integration ===\n");

  // Load regex-log task
  const suite = await Effect.runPromise(
    loadTerminalBenchSuite("tasks/terminal-bench-2.json").pipe(Effect.provide(BunContext.layer))
  );
  const task = suite.tasks.find((t) => t.id === "regex-log");
  if (!task) throw new Error("Task not found: regex-log");

  console.log(`Task: ${task.id}`);
  console.log(`Description: ${task.description.slice(0, 100)}...\n`);

  // Create temporary workspace
  const workspace = join(tmpdir(), `testgen-test-${Date.now()}`);
  mkdirSync(workspace, { recursive: true });
  console.log(`Workspace: ${workspace}\n`);

  // Run testgen integration
  console.log("Running testgen integration...\n");
  const result = await runTestGenForTask(task, workspace, {
    model: "local",
    verbose: true,
  });

  console.log("\n=== Results ===");
  console.log(`Tests generated: ${result.tests.length}`);
  console.log(`Comprehensiveness: ${result.comprehensivenessScore}/10`);
  console.log(`Duration: ${result.durationMs}ms`);
  console.log(`Test file: ${result.testFilePath}`);

  // Show generated pytest file
  if (existsSync(result.testFilePath)) {
    console.log("\n=== Generated pytest file ===");
    const content = readFileSync(result.testFilePath, "utf-8");
    console.log(content.split("\n").slice(0, 50).join("\n")); // First 50 lines
    console.log(`... (${content.split("\n").length} total lines)`);
  }

  console.log("\n✅ Integration test complete!");
}

main().catch(console.error);
