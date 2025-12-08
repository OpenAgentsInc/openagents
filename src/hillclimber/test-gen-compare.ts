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
 *   bun run src/hillclimber/test-gen-compare.ts --env-aware       # use environment-aware generation
 */

import { parseArgs } from "util";
import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { loadTerminalBenchSuite } from "../bench/terminal-bench.js";
import { generateTestsFromDescription, generateTestsFromEnvironment, getAllTestsFromEnvironmentResult } from "./test-generator.js";
import { inferProhibitedTools, emptyEnvironmentInfo, detectFileType } from "./environment-info.js";
import type { EnvironmentInfo, FilePreview } from "./environment-info.js";

const { values } = parseArgs({
  args: process.argv.slice(2),
  options: {
    task: { type: "string", short: "t" },
    model: { type: "string", short: "m", default: "local" },
    "env-aware": { type: "boolean", short: "e", default: false },
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
  -e, --env-aware    Use environment-aware generation (default: false)
  -h, --help         Show this help

Examples:
  bun run src/hillclimber/test-gen-compare.ts                    # random task
  bun run src/hillclimber/test-gen-compare.ts --task regex-log
  bun run src/hillclimber/test-gen-compare.ts --env-aware        # with environment context
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
  const envAware = values["env-aware"];
  console.log(`GENERATING OUR TESTS ${envAware ? "(ENV-AWARE)" : ""} (blind to above)...`);
  console.log("─".repeat(70));

  const model = values.model as "local" | "claude";
  const startTime = Date.now();

  try {
    if (envAware) {
      // Build mock environment from task folder
      const env = await buildMockEnvironmentFromTask(task.id, task.description);
      console.log(`\nEnvironment context:`);
      console.log(`  Platform: ${env.platform.type}`);
      console.log(`  Prohibited tools: ${env.tools.prohibited.map(t => t.name).join(", ") || "none"}`);
      console.log(`  File previews: ${env.files.taskFiles.length}`);

      const result = await generateTestsFromEnvironment(task.description, task.id, env, {
        model,
        verbose: true,
      });

      const allTests = getAllTestsFromEnvironmentResult(result);
      const duration = ((Date.now() - startTime) / 1000).toFixed(1);
      console.log(`\nGenerated ${allTests.length} tests in ${duration}s using ${result.model}\n`);

      // Display generated tests by category
      console.log("─".repeat(70));
      console.log("OUR GENERATED TESTS (ENV-AWARE)");
      console.log("─".repeat(70));

      console.log(`\n[ANTI-CHEAT] (${result.antiCheatTests.length})`);
      for (const test of result.antiCheatTests) {
        console.log(`  • ${test.id}`);
        console.log(`    Input: ${test.input.slice(0, 60)}${test.input.length > 60 ? "..." : ""}`);
        console.log(`    Expected: ${test.expectedOutput?.slice(0, 30) ?? "null"}`);
        console.log(`    Reasoning: ${test.reasoning.slice(0, 50)}...`);
      }

      console.log(`\n[EXISTENCE] (${result.existenceTests.length})`);
      for (const test of result.existenceTests.slice(0, 3)) {
        console.log(`  • ${test.id}: ${test.reasoning.slice(0, 50)}...`);
      }

      console.log(`\n[CORRECTNESS] (${result.correctnessTests.length})`);
      for (const test of result.correctnessTests.slice(0, 3)) {
        console.log(`  • ${test.id}: ${test.reasoning.slice(0, 50)}...`);
      }

      console.log(`\n[BOUNDARY] (${result.boundaryTests.length})`);
      for (const test of result.boundaryTests.slice(0, 3)) {
        console.log(`  • ${test.id}: ${test.reasoning.slice(0, 50)}...`);
      }

      console.log(`\n[INTEGRATION] (${result.integrationTests.length})`);
      for (const test of result.integrationTests.slice(0, 3)) {
        console.log(`  • ${test.id}: ${test.reasoning.slice(0, 50)}...`);
      }

      // Summary comparison
      console.log("\n" + "═".repeat(70));
      console.log("COMPARISON SUMMARY (ENV-AWARE)");
      console.log("═".repeat(70));
      console.log(`\nActual TB2:  ${actualTests.tests.length} test functions`);
      console.log(`Generated:   ${allTests.length} tests total`);
      console.log(`  Anti-cheat: ${result.antiCheatTests.length}`);
      console.log(`  Existence:  ${result.existenceTests.length}`);
      console.log(`  Correctness: ${result.correctnessTests.length}`);
      console.log(`  Boundary:   ${result.boundaryTests.length}`);
      console.log(`  Integration: ${result.integrationTests.length}`);

      // Check for anti-cheat alignment
      const hasAntiCheatInActual = actualTests.tests.some(t =>
        t.name.includes("not_installed") || t.name.includes("prohibited") || t.name.includes("cheat")
      );
      const hasAntiCheatInGenerated = result.antiCheatTests.length > 0;
      console.log(`\nAnti-cheat alignment:`);
      console.log(`  Actual has anti-cheat: ${hasAntiCheatInActual ? "✅" : "❌"}`);
      console.log(`  Generated has anti-cheat: ${hasAntiCheatInGenerated ? "✅" : "❌"}`);

    } else {
      // Original description-only generation
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
    }

  } catch (error) {
    console.error(`\nGeneration failed: ${error instanceof Error ? error.message : error}`);
    process.exit(1);
  }
}

/**
 * Build a mock environment from the task folder in TB2.
 */
async function buildMockEnvironmentFromTask(taskId: string, description: string): Promise<EnvironmentInfo> {
  const fs = await import("node:fs");
  const path = await import("node:path");

  const taskDir = `${TB2_PATH}/${taskId}`;
  const env = emptyEnvironmentInfo();
  env.platform = { type: "docker" };

  // Infer prohibited tools from description
  env.tools.prohibited = inferProhibitedTools(description);
  env.tools.prohibitedCheck = {};
  for (const tool of env.tools.prohibited) {
    env.tools.prohibitedCheck[tool.name] = false; // Assume not found (correct for conversion tasks)
  }

  // Look for common files in task directory
  if (fs.existsSync(taskDir)) {
    const files = fs.readdirSync(taskDir);
    env.files.workdir = "/app";
    env.files.listing = files.slice(0, 20).map(name => {
      const fullPath = path.join(taskDir, name);
      const stats = fs.statSync(fullPath);
      return {
        name,
        path: `/app/${name}`,
        type: stats.isDirectory() ? "directory" as const : "file" as const,
        size: stats.size,
        permissions: "-rw-r--r--",
      };
    });

    // Get file previews for source files
    const previewExtensions = [".py", ".r", ".R", ".stan", ".c", ".rs", ".go", ".java", ".js", ".ts"];
    const sourceFiles = files.filter(f =>
      previewExtensions.some(ext => f.endsWith(ext))
    ).slice(0, 5);

    env.files.taskFiles = [];
    for (const file of sourceFiles) {
      const filePath = path.join(taskDir, file);
      try {
        const content = fs.readFileSync(filePath, "utf-8");
        const lines = content.split("\n");
        const preview = lines.slice(0, 50).join("\n");
        const extension = file.split(".").pop() || "";

        const filePreview: FilePreview = {
          path: `/app/${file}`,
          extension,
          lineCount: lines.length,
          preview,
          detectedType: detectFileType(file, preview),
        };

        // Extract structure from preview
        const structure = extractStructureFromContent(preview, filePreview.detectedType);
        if (structure) filePreview.structure = structure;

        env.files.taskFiles.push(filePreview);
      } catch {
        // Skip files that can't be read
      }
    }
  }

  // Set some common tools based on description
  if (description.toLowerCase().includes("python")) {
    env.languages.python = { version: "3.11.0", packages: [], executable: "/usr/bin/python3" };
  }
  if (description.toLowerCase().includes("rust")) {
    env.languages.rust = { version: "1.75.0" };
  }
  if (description.toLowerCase().includes("node") || description.toLowerCase().includes("javascript")) {
    env.languages.node = { version: "20.10.0", packages: [] };
  }

  return env;
}

/**
 * Extract structure from file content.
 */
function extractStructureFromContent(content: string, detectedType: string | undefined): { variables?: string[]; functions?: string[]; parameters?: string[] } | undefined {
  const result: { variables?: string[]; functions?: string[]; parameters?: string[] } = {};

  // Python/R variables
  const varMatches = content.match(/^(\w+)\s*=\s*/gm);
  if (varMatches) {
    result.variables = varMatches.map(m => m.split("=")[0].trim()).filter(v => !v.startsWith("_"));
  }

  // Python/JS functions
  const funcMatches = content.match(/def\s+(\w+)|function\s+(\w+)|fn\s+(\w+)/g);
  if (funcMatches) {
    result.functions = funcMatches.map(m => m.split(/\s+/)[1]).filter(Boolean);
  }

  // Stan parameters
  if (detectedType === "stan_model") {
    const paramBlock = content.match(/parameters\s*\{([^}]+)\}/);
    if (paramBlock) {
      const params = paramBlock[1].match(/\b(\w+)\s*[;\n]/g);
      if (params) {
        result.parameters = params.map(p => p.trim().replace(/[;\n]/g, ""));
      }
    }
  }

  return Object.keys(result).length > 0 ? result : undefined;
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
