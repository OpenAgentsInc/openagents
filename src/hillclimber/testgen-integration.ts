/**
 * TestGen Integration for HillClimber
 *
 * Runs testgen, converts output to pytest, writes to workspace.
 */

import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { mkdirSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import type { TerminalBenchTask } from "../bench/terminal-bench.js";
import { generateTestsIteratively } from "./test-generator-iterative.js";
import { emptyEnvironmentInfo } from "./environment-info.js";
import type { GeneratedTest, TestCategory } from "./test-generator.js";
import { convertTestsToPytest, generatePytestConftest, type TestGenToPytestOptions } from "./testgen-to-pytest.js";

export interface TestGenIntegrationResult {
  /** Tests generated */
  tests: GeneratedTest[];
  /** Comprehensiveness score */
  comprehensivenessScore: number;
  /** Path to generated test file */
  testFilePath: string;
  /** Total time taken */
  durationMs: number;
}

/**
 * Run testgen for a task and write tests to workspace.
 *
 * This integrates testgen into the hillclimber solving pipeline:
 * 1. Generate comprehensive tests for the task
 * 2. Convert tests to pytest format
 * 3. Write to workspace/tests/test_outputs.py
 * 4. Return tests for reference during solving
 */
export async function runTestGenForTask(
  task: TerminalBenchTask,
  workspace: string,
  options: {
    model?: "local" | "claude";
    verbose?: boolean;
    /** HUD emitter for real-time UI updates */
    hudEmitter?: import("./hud-emitter.js").HillClimberHudEmitter;
  } = {}
): Promise<TestGenIntegrationResult> {
  const startTime = Date.now();

  // Build mock environment
  const env = emptyEnvironmentInfo();
  env.platform = { type: "docker" };

  const tests: GeneratedTest[] = [];
  let comprehensivenessScore = 0;

  // Track test counts per category for HUD
  const categoryTestCounts: Record<string, number> = {};

  // Emit TestGen start to HUD
  options.hudEmitter?.onTestGenStart(task.id, task.description);

  // Generate tests using iterative testgen
  await generateTestsIteratively(
    task.description,
    task.id,
    env,
    {
      onStart: () => {
        if (options.verbose) {
          console.log(`[TestGen] Generating tests for ${task.id}...`);
        }
      },
      onTest: (msg) => {
        const test: GeneratedTest = {
          id: msg.test.id,
          category: msg.test.category as TestCategory,
          input: msg.test.input,
          expectedOutput: msg.test.expectedOutput,
          reasoning: msg.test.reasoning,
          confidence: msg.test.confidence,
        };
        tests.push(test);
        if (options.verbose) {
          console.log(`[TestGen] Generated test ${tests.length}: ${test.category}`);
        }
        // Emit category update to HUD
        categoryTestCounts[test.category] = (categoryTestCounts[test.category] || 0) + 1;
        options.hudEmitter?.onTestGenCategory(test.category, categoryTestCounts[test.category]);
      },
      onProgress: (msg) => {
        if (options.verbose) {
          console.log(`[TestGen] ${msg.status}`);
        }
      },
      onReflection: (msg) => {
        if (options.verbose) {
          console.log(`[TestGen] Reflection: ${msg.reflectionText}`);
        }
      },
      onComplete: (msg) => {
        comprehensivenessScore = msg.comprehensivenessScore || 0;
        if (options.verbose) {
          console.log(`[TestGen] Complete: ${msg.totalTests} tests, score ${comprehensivenessScore}/10`);
        }
        // Emit TestGen complete to HUD
        options.hudEmitter?.onTestGenComplete(msg.totalTests, comprehensivenessScore);
      },
      onError: (msg) => {
        console.error(`[TestGen] Error: ${msg.error}`);
      },
    },
    { model: options.model || "local", verbose: options.verbose || false }
  );

  // Determine task type for pytest conversion
  const taskType = determineTaskType(task);

  // Determine solution path based on task
  const solutionPath = determineSolutionPath(task, workspace);

  // Convert tests to pytest format
  const pytestOptions: TestGenToPytestOptions = {
    moduleName: task.id,
    solutionPath,
    taskType,
  };

  const pytestCode = convertTestsToPytest(tests, pytestOptions);

  // Write to workspace/tests/
  const testsDir = join(workspace, "tests");
  if (!existsSync(testsDir)) {
    mkdirSync(testsDir, { recursive: true });
  }

  const testFilePath = join(testsDir, "test_outputs.py");
  writeFileSync(testFilePath, pytestCode, "utf-8");

  // Also write conftest.py
  const conftestPath = join(testsDir, "conftest.py");
  writeFileSync(conftestPath, generatePytestConftest(), "utf-8");

  const durationMs = Date.now() - startTime;

  if (options.verbose) {
    console.log(`[TestGen] Wrote ${tests.length} tests to ${testFilePath}`);
    console.log(`[TestGen] Duration: ${durationMs}ms`);
  }

  return {
    tests,
    comprehensivenessScore,
    testFilePath,
    durationMs,
  };
}

/**
 * Determine task type from task description/metadata.
 */
function determineTaskType(task: TerminalBenchTask): "regex" | "script" | "code" {
  const desc = task.description.toLowerCase();

  if (desc.includes("regex") || desc.includes("regular expression")) {
    return "regex";
  }

  if (desc.includes("script") || desc.includes("bash") || desc.includes("shell")) {
    return "script";
  }

  return "code";
}

/**
 * Determine solution file path based on task type.
 */
function determineSolutionPath(task: TerminalBenchTask, workspace: string): string {
  const taskType = determineTaskType(task);

  switch (taskType) {
    case "regex":
      // Check common regex solution paths
      if (existsSync(join(workspace, "regex.txt"))) {
        return "/app/regex.txt";
      }
      if (existsSync(join(workspace, "pattern.txt"))) {
        return "/app/pattern.txt";
      }
      return "/app/regex.txt"; // Default

    case "script":
      return "/app/solve.sh";

    case "code":
      return "/app/solution.py"; // Common default
  }
}
