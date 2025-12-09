#!/usr/bin/env bun
/**
 * Test Generator CLI
 *
 * Standalone CLI to test the test generator against TB2 tasks.
 * Includes evolution commands for iterative optimization.
 *
 * Usage:
 *   bun run src/hillclimber/test-gen-cli.ts --task regex-log
 *   bun run src/hillclimber/test-gen-cli.ts:evolve --max-runs 50
 *   bun run src/hillclimber/test-gen-cli.ts:stats
 */

import { Effect } from "effect"
import { parseArgs } from "util"
import { BunContext } from "@effect/platform-bun"
import { loadTerminalBenchSuite } from "../bench/terminal-bench.js"
import {
    generateTestsFromDescription, summarizeCategories, type TestGeneratorOptions
} from "./test-generator.js"
import {
    runTestGenEvolution, type TestGenRunnerOptions
} from "./testgen-runner.js"
import { TestGenStore, TestGenStoreLive } from "./testgen-store.js"

const { values, positionals } = parseArgs({
  args: process.argv.slice(2),
  options: {
    task: { type: "string", short: "t" },
    model: { type: "string", short: "m", default: "claude" },
    "min-tests": { type: "string", default: "15" },
    "max-tests": { type: "string", default: "30" },
    verbose: { type: "boolean", short: "v", default: false },
    help: { type: "boolean", short: "h", default: false },
    // Evolution commands
    evolve: { type: "boolean", default: false },
    "max-runs": { type: "string", default: "100" },
    sleep: { type: "string", default: "10000" },
    "task-type": { type: "string" },
    "model-override": { type: "string" },
    "dry-run": { type: "boolean", default: false },
    stats: { type: "boolean", default: false },
    export: { type: "boolean", default: false },
    json: { type: "boolean", default: false },
  },
});


// Check for stats command first
if ((positionals as string[]).includes("stats") || values.stats) {
  Effect.runPromise(
    TestGenStore.pipe(
      Effect.flatMap((store) => store.getStats()),
      Effect.tap((stats) =>
        Effect.sync(() => {
          if (values.json) {
            console.log(JSON.stringify(stats, null, 2));
          } else {
            console.log("\n=== TestGen Evolution Stats ===");
            console.log(`Total runs: ${stats.totalRuns}`);
            console.log(`Total configs: ${stats.totalConfigs}`);
            console.log(`Average score: ${stats.averageScore.toFixed(0)}/1000`);
            console.log(`Best score: ${stats.bestScore}/1000`);
            console.log(`Average comprehensiveness: ${stats.averageComprehensiveness.toFixed(1)}`);
            console.log(`Average token efficiency: ${stats.averageTokenEfficiency.toFixed(2)}`);
            console.log(`Config evolutions: ${stats.configEvolutionCount}`);
          }
        })
      ),
      Effect.provide(TestGenStoreLive),
    )
  )
    .then(() => {
      process.exit(0);
    })
    .catch((error) => {
      console.error(error);
      process.exit(1);
    });
  // Don't continue - wait for promise
} else if ((positionals as string[]).includes("evolve") || values.evolve) {
  const maxRuns = parseInt(values["max-runs"] as string, 10);
  const sleepMs = parseInt(values.sleep as string, 10);
  const suitePath = "tasks/terminal-bench-2.json";

  const options: TestGenRunnerOptions = {
    maxRuns,
    sleepMs,
    suitePath,
    dryRun: values["dry-run"] as boolean,
  };
  if (values.task) options.taskId = values.task as string;
  if (values["task-type"]) options.taskType = values["task-type"] as string;
  if (values["model-override"]) options.modelOverride = values["model-override"] as string;

  runTestGenEvolution(options)
    .then(() => {
      process.exit(0);
    })
    .catch((error) => {
      console.error(error);
      process.exit(1);
    });
  // Don't continue - wait for promise
} else if (values.help) {
  console.log(`
Test Generator CLI - Generate test cases from task descriptions

Usage:
  bun run src/hillclimber/test-gen-cli.ts --task <task-id> [options]
  bun run src/hillclimber/test-gen-cli.ts:evolve [options]
  bun run src/hillclimber/test-gen-cli.ts:stats [options]

Generation Options:
  -t, --task <id>       Task ID (required for generation, e.g., regex-log)
  -m, --model <model>   Model to use: claude, local, or both (default: claude)
  --min-tests <n>       Minimum tests to generate (default: 15)
  --max-tests <n>       Maximum tests to generate (default: 30)
  -v, --verbose         Enable verbose logging
  -h, --help            Show this help

Evolution Options:
  --evolve              Run evolution loop
  --max-runs <n>        Maximum evolution runs (default: 100)
  --sleep <ms>          Sleep between runs in ms (default: 10000)
  --task <id>           Specific task ID (or random if not specified)
  --task-type <type>    Task type filter (conversion, implementation, etc.)
  --model-override <m>  Model override for meta-reasoning
  --dry-run             Preview without executing

Stats Options:
  --stats               Show current stats and exit
  --json                Output stats as JSON
  --task <id>           Stats for specific task

Examples:
  # Generate tests for a task
  bun run src/hillclimber/test-gen-cli.ts --task regex-log

  # Run evolution loop
  bun run src/hillclimber/test-gen-cli.ts:evolve --max-runs 50 --sleep 30000

  # Show stats
  bun run src/hillclimber/test-gen-cli.ts:stats
`);
  process.exit(0);
} else if (!values.task) {
  console.error("Error: --task is required for test generation");
  console.log("Use --help for usage information");
  process.exit(1);
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

// Only run main() if not in evolve or stats mode
if (!values.evolve && !values.stats && !(positionals as string[]).includes("evolve") && !(positionals as string[]).includes("stats")) {
  main().catch((error) => {
    console.error(error);
    process.exit(1);
  });
}
