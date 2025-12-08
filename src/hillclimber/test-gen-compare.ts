#!/usr/bin/env bun
/**
 * Test Generation Comparison Script
 *
 * Picks a random TB2 task, generates tests, and compares to actual benchmark tests.
 *
 * Usage:
 *   bun run src/hillclimber/test-gen-compare.ts
 *   bun run src/hillclimber/test-gen-compare.ts --task regex-log  # specific task
 *   bun run src/hillclimber/test-gen-compare.ts --model claude    # use Claude
 */

import { parseArgs } from "util";
import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { loadTerminalBenchSuite } from "../bench/terminal-bench.js";
import { generateTestsFromDescription } from "./test-generator.js";

const { values } = parseArgs({
  args: process.argv.slice(2),
  options: {
    task: { type: "string", short: "t" },
    model: { type: "string", short: "m", default: "local" },
    help: { type: "boolean", short: "h", default: false },
  },
});

if (values.help) {
  console.log(`
Test Generation Comparison - Compare generated tests to actual TB2 tests

Usage:
  bun run src/hillclimber/test-gen-compare.ts [options]

Options:
  -t, --task <id>    Specific task ID (default: random)
  -m, --model        Model: local or claude (default: local)
  -h, --help         Show this help

Examples:
  bun run src/hillclimber/test-gen-compare.ts              # random task
  bun run src/hillclimber/test-gen-compare.ts --task regex-log
`);
  process.exit(0);
}

const TB2_PATH = "/Users/christopherdavid/code/terminal-bench-2";

async function main() {
  console.log("Loading TB2 suite...\n");

  // Load TB2 suite
  const suitePath = "tasks/terminal-bench-2.json";
  const suite = await Effect.runPromise(
    loadTerminalBenchSuite(suitePath).pipe(Effect.provide(BunContext.layer))
  );

  // Pick task (random or specified)
  let task;
  if (values.task) {
    task = suite.tasks.find((t) => t.id === values.task);
    if (!task) {
      console.error(`Task not found: ${values.task}`);
      console.log(`Available tasks: ${suite.tasks.slice(0, 10).map((t) => t.id).join(", ")}...`);
      process.exit(1);
    }
  } else {
    const randomIndex = Math.floor(Math.random() * suite.tasks.length);
    task = suite.tasks[randomIndex];
  }

  console.log("═".repeat(70));
  console.log(`TASK: ${task.id}`);
  console.log("═".repeat(70));
  console.log(`\nDescription:\n${task.description.slice(0, 500)}${task.description.length > 500 ? "..." : ""}\n`);

  // First, show actual TB2 tests (so we know what we're comparing against)
  console.log("─".repeat(70));
  console.log("ACTUAL TB2 BENCHMARK TESTS");
  console.log("─".repeat(70));

  const testFilePath = `${TB2_PATH}/${task.id}/tests/test_outputs.py`;
  const actualTests = await extractActualTestInfo(testFilePath, task.id);

  if (actualTests.error) {
    console.log(`\n⚠️  ${actualTests.error}`);
  } else {
    console.log(`\nFound ${actualTests.tests.length} test functions:\n`);
    for (const test of actualTests.tests) {
      console.log(`  • ${test.name}`);
      if (test.description) {
        console.log(`    ${test.description}`);
      }
    }

    if (actualTests.testData.length > 0) {
      console.log(`\nKey assertions/test data:`);
      for (const data of actualTests.testData.slice(0, 15)) {
        console.log(`  • ${data}`);
      }
      if (actualTests.testData.length > 15) {
        console.log(`  ... and ${actualTests.testData.length - 15} more`);
      }
    }
  }

  // Now generate our tests
  console.log("\n" + "─".repeat(70));
  console.log("GENERATING OUR TESTS (blind to above)...");
  console.log("─".repeat(70));

  const model = values.model as "local" | "claude";
  const startTime = Date.now();

  try {
    const result = await generateTestsFromDescription(task.description, task.id, {
      model,
      minTests: 10,
      maxTests: 20,
    });

    const duration = ((Date.now() - startTime) / 1000).toFixed(1);
    console.log(`\nGenerated ${result.tests.length} tests in ${duration}s using ${result.model}\n`);

    // Display generated tests
    console.log("─".repeat(70));
    console.log("OUR GENERATED TESTS");
    console.log("─".repeat(70));

    // Group by category
    const byCategory = new Map<string, typeof result.tests>();
    for (const test of result.tests) {
      const existing = byCategory.get(test.category) || [];
      existing.push(test);
      byCategory.set(test.category, existing);
    }

    for (const [category, tests] of byCategory) {
      console.log(`\n[${category.toUpperCase()}]`);
      for (const test of tests) {
        const input = test.input.length > 50 ? test.input.slice(0, 50) + "..." : test.input;
        const expected = test.expectedOutput
          ? (test.expectedOutput.length > 30 ? test.expectedOutput.slice(0, 30) + "..." : test.expectedOutput)
          : "null";
        console.log(`  • ${test.id}`);
        console.log(`    Input: ${input}`);
        console.log(`    Expected: ${expected}`);
      }
    }

    if (result.requirements.length > 0) {
      console.log(`\nRequirements we identified:`);
      for (const req of result.requirements.slice(0, 5)) {
        console.log(`  • ${req}`);
      }
    }

    // Summary comparison
    console.log("\n" + "═".repeat(70));
    console.log("COMPARISON SUMMARY");
    console.log("═".repeat(70));
    console.log(`\nActual TB2:  ${actualTests.tests.length} test functions`);
    console.log(`Generated:   ${result.tests.length} tests`);
    console.log(`\nOur categories: ${[...byCategory.keys()].join(", ")}`);

  } catch (error) {
    console.error(`\nGeneration failed: ${error instanceof Error ? error.message : error}`);
    process.exit(1);
  }
}

interface ActualTestInfo {
  tests: Array<{ name: string; description?: string }>;
  testData: string[];
  error?: string;
}

async function extractActualTestInfo(filePath: string, taskId: string): Promise<ActualTestInfo> {
  const fs = await import("node:fs");

  if (!fs.existsSync(filePath)) {
    return {
      tests: [],
      testData: [],
      error: `Test file not found: ${filePath}`,
    };
  }

  const content = fs.readFileSync(filePath, "utf-8");
  const tests: Array<{ name: string; description?: string }> = [];
  const testData: string[] = [];

  // Extract test function names and docstrings
  const testFuncPattern = /def (test_\w+)\([^)]*\):\s*(?:"""([^"]+)""")?/g;
  let match;
  while ((match = testFuncPattern.exec(content)) !== null) {
    tests.push({
      name: match[1],
      description: match[2]?.trim(),
    });
  }

  // Extract interesting assertions and test data
  const lines = content.split("\n");
  for (const line of lines) {
    const trimmed = line.trim();

    // Capture assert statements
    if (trimmed.startsWith("assert ")) {
      const assertion = trimmed.slice(7).slice(0, 80);
      if (!testData.includes(assertion)) {
        testData.push(assertion);
      }
    }

    // Capture test data definitions
    if (trimmed.match(/^(test_|expected_|valid_|invalid_)\w+\s*=/) ||
        trimmed.match(/^\w+_cases\s*=/) ||
        trimmed.match(/^cases\s*=/)) {
      const dataLine = trimmed.slice(0, 80);
      if (!testData.includes(dataLine)) {
        testData.push(dataLine);
      }
    }
  }

  return { tests, testData };
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
