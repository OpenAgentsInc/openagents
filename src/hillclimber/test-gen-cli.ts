#!/usr/bin/env bun
/**
 * Test Generator CLI
 *
 * Standalone CLI to test the test generator against TB2 tasks.
 *
 * Usage:
 *   bun run src/hillclimber/test-gen-cli.ts --task regex-log
 *   bun run src/hillclimber/test-gen-cli.ts --task regex-log --model local
 *   bun run src/hillclimber/test-gen-cli.ts --task regex-log --verbose
 */

import { parseArgs } from "util";
import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { loadTerminalBenchSuite } from "../bench/terminal-bench.js";
import {
  generateTestsFromDescription,
  summarizeCategories,
  type TestGeneratorOptions,
} from "./test-generator.js";

const { values } = parseArgs({
  args: process.argv.slice(2),
  options: {
    task: { type: "string", short: "t" },
    model: { type: "string", short: "m", default: "claude" },
    "min-tests": { type: "string", default: "15" },
    "max-tests": { type: "string", default: "30" },
    verbose: { type: "boolean", short: "v", default: false },
    help: { type: "boolean", short: "h", default: false },
  },
});

if (values.help || !values.task) {
  console.log(`
Test Generator CLI - Generate test cases from task descriptions

Usage:
  bun run src/hillclimber/test-gen-cli.ts --task <task-id> [options]

Options:
  -t, --task <id>       Task ID (required, e.g., regex-log)
  -m, --model <model>   Model to use: claude, local, or both (default: claude)
  --min-tests <n>       Minimum tests to generate (default: 15)
  --max-tests <n>       Maximum tests to generate (default: 30)
  -v, --verbose         Enable verbose logging
  -h, --help            Show this help

Examples:
  bun run src/hillclimber/test-gen-cli.ts --task regex-log
  bun run src/hillclimber/test-gen-cli.ts --task regex-log --model local
  bun run src/hillclimber/test-gen-cli.ts --task path-tracing --verbose
`);
  process.exit(0);
}

async function main() {
  const taskId = values.task!;
  const model = values.model as "claude" | "local" | "both";
  const minTests = parseInt(values["min-tests"] as string, 10);
  const maxTests = parseInt(values["max-tests"] as string, 10);
  const verbose = values.verbose;

  console.log(`\n=== Test Generator CLI ===`);
  console.log(`Task: ${taskId}`);
  console.log(`Model: ${model}`);
  console.log(`Min/Max Tests: ${minTests}/${maxTests}`);
  console.log(`Verbose: ${verbose}\n`);

  // Load the TB2 suite
  console.log("Loading TB2 suite...");
  const suitePath = "tasks/terminal-bench-2.json";
  const suite = await Effect.runPromise(
    loadTerminalBenchSuite(suitePath).pipe(Effect.provide(BunContext.layer))
  );

  // Find the task
  const task = suite.tasks.find((t) => t.id === taskId);
  if (!task) {
    console.error(`Task not found: ${taskId}`);
    console.log(`Available tasks: ${suite.tasks.map((t) => t.id).slice(0, 10).join(", ")}...`);
    process.exit(1);
  }

  console.log(`Found task: ${task.id}`);
  console.log(`Description length: ${task.description.length} chars`);
  console.log(`Description preview: ${task.description.slice(0, 200)}...\n`);

  // Generate tests
  console.log("Generating tests...\n");

  const options: TestGeneratorOptions = {
    model,
    minTests,
    maxTests,
    verbose,
  };

  try {
    const result = await generateTestsFromDescription(
      task.description,
      task.id,
      options,
    );

    console.log(`\n=== Generation Complete ===`);
    console.log(`Model: ${result.model}`);
    console.log(`Duration: ${result.durationMs}ms`);
    console.log(`Tests generated: ${result.tests.length}`);
    console.log(`Categories: ${summarizeCategories(result.tests)}`);

    console.log(`\n--- Requirements Identified ---`);
    for (const req of result.requirements) {
      console.log(`  - ${req}`);
    }

    if (result.assumptions.length > 0) {
      console.log(`\n--- Assumptions Made ---`);
      for (const assumption of result.assumptions) {
        console.log(`  - ${assumption}`);
      }
    }

    if (result.uncertainties.length > 0) {
      console.log(`\n--- Uncertainties ---`);
      for (const uncertainty of result.uncertainties) {
        console.log(`  - ${uncertainty}`);
      }
    }

    console.log(`\n--- Generated Tests ---`);
    for (const test of result.tests) {
      console.log(`\n[${test.category}] ${test.id} (confidence: ${test.confidence})`);
      console.log(`  Input: ${test.input.slice(0, 100)}${test.input.length > 100 ? "..." : ""}`);
      console.log(`  Expected: ${test.expectedOutput ?? "(null/no match)"}`);
      console.log(`  Reasoning: ${test.reasoning}`);
    }

    // Output JSON for further analysis
    const outputPath = `logs/test-gen-${taskId}-${Date.now()}.json`;
    await Bun.write(outputPath, JSON.stringify(result, null, 2));
    console.log(`\nFull output saved to: ${outputPath}`);

  } catch (error) {
    console.error(`\nGeneration failed:`);
    console.error(error);
    process.exit(1);
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
