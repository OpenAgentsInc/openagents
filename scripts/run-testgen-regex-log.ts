#!/usr/bin/env bun
/**
 * Quick script to run testgen on regex-log and see output
 */

import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { loadTerminalBenchSuite } from "../src/bench/terminal-bench.js";
import { generateTestsIteratively } from "../src/hillclimber/test-generator-iterative.js";
import { emptyEnvironmentInfo } from "../src/hillclimber/environment-info.js";

const taskId = "regex-log";
const suitePath = "tasks/terminal-bench-2.json";

async function main() {
  console.log("\n=== TestGen: Converting regex-log → Test Suite ===\n");

  // Load task
  const suite = await Effect.runPromise(
    loadTerminalBenchSuite(suitePath).pipe(Effect.provide(BunContext.layer))
  );
  const task = suite.tasks.find((t) => t.id === taskId);
  if (!task) throw new Error(`Task not found: ${taskId}`);

  console.log(`Task: ${task.id}`);
  console.log(`Description: ${task.description.slice(0, 200)}...\n`);

  // Build mock environment
  const env = emptyEnvironmentInfo();
  env.platform = { type: "docker" };

  const tests: any[] = [];
  const reflections: any[] = [];

  // Generate tests with streaming console output
  await generateTestsIteratively(
    task.description,
    task.id,
    env,
    {
      onStart: (msg) => {
        console.log(`[START] Generating tests for ${msg.taskId}`);
      },
      onTest: (msg) => {
        tests.push(msg.test);
        console.log(`\n[TEST ${tests.length}] ${msg.test.category.toUpperCase()}`);
        console.log(`  Input: ${msg.test.input.slice(0, 100)}...`);
        console.log(`  Expected: ${msg.test.expectedOutput ?? "(null/no match)"}`);
        console.log(`  Reasoning: ${msg.test.reasoning}`);
        console.log(`  Confidence: ${msg.test.confidence}`);
      },
      onProgress: (msg) => {
        console.log(`[PROGRESS] ${msg.status}`);
      },
      onReflection: (msg) => {
        reflections.push(msg);
        console.log(`\n[REFLECTION] ${msg.category}`);
        console.log(`  ${msg.reflectionText}`);
        console.log(`  Action: ${msg.action}`);
      },
      onComplete: (msg) => {
        console.log(`\n=== COMPLETE ===`);
        console.log(`Total tests: ${msg.totalTests}`);
        console.log(`Total rounds: ${msg.totalRounds}`);
        console.log(`Comprehensiveness: ${msg.comprehensivenessScore}/10`);
        console.log(`Category rounds:`, msg.categoryRounds);
      },
      onError: (msg) => {
        console.error(`[ERROR] ${msg.error}`);
      },
    },
    { model: "local", verbose: true }
  );

  console.log(`\nDone! Generated ${tests.length} tests.`);
}

main().catch(console.error);
