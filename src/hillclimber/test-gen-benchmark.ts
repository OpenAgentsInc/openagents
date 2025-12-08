#!/usr/bin/env bun
/**
 * Test Generation Benchmark CLI
 *
 * Runs test generation across TB2 tasks and evaluates quality against actual tests.
 * This enables hill-climbing on the test generator itself.
 *
 * Usage:
 *   bun run src/hillclimber/test-gen-benchmark.ts --tasks regex-log,path-tracing
 *   bun run src/hillclimber/test-gen-benchmark.ts --all --model claude
 *   bun run src/hillclimber/test-gen-benchmark.ts --sample 10 --verbose
 */

import { Effect } from "effect"
import { parseArgs } from "util"
import { BunContext } from "@effect/platform-bun"
import {
    loadTerminalBenchSuite, type TerminalBenchTask
} from "../bench/terminal-bench.js"
import {
    evaluateTestGeneration, type TestGenerationScore
} from "./test-gen-evaluator.js"
import {
    generateTestsFromDescription, type TestGenerationResult, type TestGeneratorOptions
} from "./test-generator.js"

// ============================================================================
// CLI Arguments
// ============================================================================

const { values } = parseArgs({
  args: process.argv.slice(2),
  options: {
    tasks: { type: "string", short: "t", description: "Comma-separated task IDs" },
    all: { type: "boolean", short: "a", default: false, description: "Run on all tasks" },
    sample: { type: "string", short: "s", description: "Random sample of N tasks" },
    model: { type: "string", short: "m", default: "claude", description: "Model: claude, local, both" },
    "min-tests": { type: "string", default: "15", description: "Minimum tests to generate" },
    "max-tests": { type: "string", default: "30", description: "Maximum tests to generate" },
    "tb2-path": { type: "string", default: "/Users/christopherdavid/code/terminal-bench-2", description: "Path to TB2 repo" },
    verbose: { type: "boolean", short: "v", default: false },
    json: { type: "boolean", default: false, description: "Output as JSON" },
    compare: { type: "boolean", short: "c", default: false, description: "Show side-by-side comparison" },
    help: { type: "boolean", short: "h", default: false },
  },
});

if (values.help) {
  console.log(`
Test Generation Benchmark CLI - Evaluate test generation quality against TB2

Usage:
  bun run src/hillclimber/test-gen-benchmark.ts [options]

Options:
  -t, --tasks <ids>       Comma-separated task IDs (e.g., regex-log,path-tracing)
  -a, --all               Run on all TB2 tasks
  -s, --sample <n>        Random sample of N tasks
  -m, --model <model>     Model: claude, local, or both (default: claude)
  --min-tests <n>         Minimum tests to generate (default: 15)
  --max-tests <n>         Maximum tests to generate (default: 30)
  --tb2-path <path>       Path to TB2 repo
  -v, --verbose           Enable verbose logging
  --json                  Output as JSON
  -c, --compare           Show side-by-side comparison of generated vs actual
  -h, --help              Show this help

Examples:
  bun run src/hillclimber/test-gen-benchmark.ts --tasks regex-log --compare
  bun run src/hillclimber/test-gen-benchmark.ts --sample 5 --model local
  bun run src/hillclimber/test-gen-benchmark.ts --all --json > results.json
`);
  process.exit(0);
}

// ============================================================================
// Types
// ============================================================================

interface TaskBenchmarkResult {
  taskId: string;
  generated: TestGenerationResult;
  score: TestGenerationScore;
  actualTestCount: number;
  generatedTestCount: number;
  extractionErrors: string[];
  durationMs: number;
}

interface BenchmarkSummary {
  totalTasks: number;
  completedTasks: number;
  failedTasks: number;
  averageScore: number;
  averageCoverage: number;
  averageAccuracy: number;
  averageEdgeCaseDetection: number;
  taskResults: TaskBenchmarkResult[];
  failures: Array<{ taskId: string; error: string }>;
}

// ============================================================================
// Main
// ============================================================================

async function main() {
  const model = values.model as "claude" | "local" | "both";
  const minTests = parseInt(values["min-tests"] as string, 10);
  const maxTests = parseInt(values["max-tests"] as string, 10);
  const verbose = values.verbose;
  const tb2Path = values["tb2-path"] as string;
  const showComparison = values.compare;
  const outputJson = values.json;

  // Load TB2 suite
  if (!outputJson) console.log("Loading TB2 suite...");
  const suitePath = "tasks/terminal-bench-2.json";
  const suite = await Effect.runPromise(
    loadTerminalBenchSuite(suitePath).pipe(Effect.provide(BunContext.layer)),
  );

  // Determine which tasks to run
  let tasksToRun: TerminalBenchTask[] = [];

  if (values.tasks) {
    const taskIds = (values.tasks as string).split(",").map((t) => t.trim());
    tasksToRun = suite.tasks.filter((t) => taskIds.includes(t.id));
    if (tasksToRun.length === 0) {
      console.error(`No tasks found matching: ${values.tasks}`);
      console.log(`Available: ${suite.tasks.slice(0, 10).map((t) => t.id).join(", ")}...`);
      process.exit(1);
    }
  } else if (values.all) {
    tasksToRun = [...suite.tasks];
  } else if (values.sample) {
    const sampleSize = parseInt(values.sample as string, 10);
    const shuffled = [...suite.tasks].sort(() => Math.random() - 0.5);
    tasksToRun = shuffled.slice(0, sampleSize);
  } else {
    console.error("Specify --tasks, --all, or --sample");
    process.exit(1);
  }

  if (!outputJson) {
    console.log(`\n=== Test Generation Benchmark ===`);
    console.log(`Tasks: ${tasksToRun.length}`);
    console.log(`Model: ${model}`);
    console.log(`Min/Max Tests: ${minTests}/${maxTests}`);
    console.log(`TB2 Path: ${tb2Path}\n`);
  }

  // Run benchmark
  const results: TaskBenchmarkResult[] = [];
  const failures: Array<{ taskId: string; error: string }> = [];

  for (let i = 0; i < tasksToRun.length; i++) {
    const task = tasksToRun[i];

    if (!outputJson) {
      console.log(`[${i + 1}/${tasksToRun.length}] ${task.id}...`);
    }

    try {
      const startTime = Date.now();

      // Generate tests
      const options: TestGeneratorOptions = {
        model,
        minTests,
        maxTests,
        verbose,
      };

      const generated = await generateTestsFromDescription(
        task.description,
        task.id,
        options,
      );

      // Evaluate against actual TB2 tests
      const { score, actual } = await evaluateTestGeneration(
        task.id,
        generated,
        tb2Path,
      );

      const result: TaskBenchmarkResult = {
        taskId: task.id,
        generated,
        score,
        actualTestCount: actual.tests.length,
        generatedTestCount: generated.tests.length,
        extractionErrors: actual.extractionErrors,
        durationMs: Date.now() - startTime,
      };

      results.push(result);

      if (!outputJson) {
        console.log(
          `  Score: ${score.overall.toFixed(1)} | Coverage: ${score.coverage.toFixed(0)}% | ` +
          `Accuracy: ${score.accuracy.toFixed(0)}% | EdgeCase: ${score.edgeCaseDetection.toFixed(0)}%`,
        );
        console.log(
          `  Generated: ${generated.tests.length} | Actual: ${actual.tests.length} | ` +
          `Matches: ${score.exactMatches}E/${score.partialMatches}P | Missed: ${score.missedTests}`,
        );

        if (showComparison) {
          printComparison(generated, actual.tests.map(t => ({
            id: t.id,
            input: t.input,
            expectedOutput: t.expectedOutput,
            category: t.category,
          })), score);
        }

        if (actual.extractionErrors.length > 0) {
          console.log(`  Extraction errors: ${actual.extractionErrors.join(", ")}`);
        }
      }
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      failures.push({ taskId: task.id, error: errorMsg });

      if (!outputJson) {
        console.log(`  FAILED: ${errorMsg}`);
      }
    }
  }

  // Summary
  const summary = computeSummary(results, failures);

  if (outputJson) {
    console.log(JSON.stringify(summary, null, 2));
  } else {
    printSummary(summary);
  }

  // Save full results
  const outputPath = `logs/test-gen-benchmark-${Date.now()}.json`;
  await Bun.write(outputPath, JSON.stringify(summary, null, 2));
  if (!outputJson) {
    console.log(`\nFull results saved to: ${outputPath}`);
  }
}

// ============================================================================
// Helpers
// ============================================================================

function computeSummary(
  results: TaskBenchmarkResult[],
  failures: Array<{ taskId: string; error: string }>,
): BenchmarkSummary {
  const completedTasks = results.length;
  const failedTasks = failures.length;
  const totalTasks = completedTasks + failedTasks;

  const avgScore = completedTasks > 0
    ? results.reduce((sum, r) => sum + r.score.overall, 0) / completedTasks
    : 0;
  const avgCoverage = completedTasks > 0
    ? results.reduce((sum, r) => sum + r.score.coverage, 0) / completedTasks
    : 0;
  const avgAccuracy = completedTasks > 0
    ? results.reduce((sum, r) => sum + r.score.accuracy, 0) / completedTasks
    : 0;
  const avgEdgeCaseDetection = completedTasks > 0
    ? results.reduce((sum, r) => sum + r.score.edgeCaseDetection, 0) / completedTasks
    : 0;

  return {
    totalTasks,
    completedTasks,
    failedTasks,
    averageScore: avgScore,
    averageCoverage: avgCoverage,
    averageAccuracy: avgAccuracy,
    averageEdgeCaseDetection: avgEdgeCaseDetection,
    taskResults: results,
    failures,
  };
}

function printSummary(summary: BenchmarkSummary) {
  console.log(`\n${"=".repeat(60)}`);
  console.log(`BENCHMARK SUMMARY`);
  console.log(`${"=".repeat(60)}`);
  console.log(`Total Tasks: ${summary.totalTasks}`);
  console.log(`Completed: ${summary.completedTasks}`);
  console.log(`Failed: ${summary.failedTasks}`);
  console.log(`\nAverage Scores:`);
  console.log(`  Overall:         ${summary.averageScore.toFixed(1)}/100`);
  console.log(`  Coverage:        ${summary.averageCoverage.toFixed(1)}%`);
  console.log(`  Accuracy:        ${summary.averageAccuracy.toFixed(1)}%`);
  console.log(`  Edge Case:       ${summary.averageEdgeCaseDetection.toFixed(1)}%`);

  // Top/bottom performers
  if (summary.taskResults.length > 0) {
    const sorted = [...summary.taskResults].sort((a, b) => b.score.overall - a.score.overall);
    console.log(`\nTop 5 Tasks:`);
    for (const r of sorted.slice(0, 5)) {
      console.log(`  ${r.taskId}: ${r.score.overall.toFixed(1)}`);
    }

    if (sorted.length > 5) {
      console.log(`\nBottom 5 Tasks:`);
      for (const r of sorted.slice(-5).reverse()) {
        console.log(`  ${r.taskId}: ${r.score.overall.toFixed(1)}`);
      }
    }
  }

  if (summary.failures.length > 0) {
    console.log(`\nFailures:`);
    for (const f of summary.failures) {
      console.log(`  ${f.taskId}: ${f.error}`);
    }
  }
}

function printComparison(
  generated: TestGenerationResult,
  actualTests: Array<{ id: string; input: string; expectedOutput: string | null; category: string }>,
  score: TestGenerationScore,
) {
  console.log(`\n  --- Side-by-Side Comparison ---`);

  // Show matched tests
  console.log(`\n  Matched Tests:`);
  for (const comp of score.comparisons) {
    if (comp.matchedActual) {
      const match = comp.matchType === "exact" ? "=" : comp.matchType === "partial" ? "~" : "?";
      console.log(`    [${match}] Gen: ${truncate(comp.generated.input, 40)} → ${comp.generated.expectedOutput ?? "null"}`);
      console.log(`        Act: ${truncate(comp.matchedActual.input, 40)} → ${comp.matchedActual.expectedOutput ?? "null"}`);
      if (!comp.expectedCorrect) {
        console.log(`        ⚠️  Expected values differ`);
      }
    }
  }

  // Show unmatched generated tests
  const unmatched = score.comparisons.filter((c) => c.matchType === "no_match");
  if (unmatched.length > 0) {
    console.log(`\n  Unmatched Generated Tests (false positives):`);
    for (const comp of unmatched.slice(0, 5)) {
      console.log(`    [-] ${truncate(comp.generated.input, 50)} → ${comp.generated.expectedOutput ?? "null"}`);
    }
    if (unmatched.length > 5) {
      console.log(`    ... and ${unmatched.length - 5} more`);
    }
  }

  // Show missed actual tests
  const matchedIds = new Set(
    score.comparisons.filter((c) => c.matchedActual).map((c) => c.matchedActual!.id),
  );
  const missed = actualTests.filter((t) => !matchedIds.has(t.id));
  if (missed.length > 0) {
    console.log(`\n  Missed Actual Tests:`);
    for (const t of missed.slice(0, 5)) {
      console.log(`    [!] ${truncate(t.input, 50)} → ${t.expectedOutput ?? "null"}`);
    }
    if (missed.length > 5) {
      console.log(`    ... and ${missed.length - 5} more`);
    }
  }

  console.log();
}

function truncate(s: string, maxLen: number): string {
  if (s.length <= maxLen) return s;
  return s.slice(0, maxLen - 3) + "...";
}

// ============================================================================
// Run
// ============================================================================

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
