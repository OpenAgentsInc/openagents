#!/usr/bin/env bun
/**
 * Terminal-Bench Local CLI
 *
 * Run Terminal-Bench tasks locally without Harbor/Docker for internal evaluation.
 * Loads task definitions from JSON, runs selected tasks or full suite,
 * and generates comparison reports.
 *
 * Usage:
 *   bun src/cli/tbench-local.ts \
 *     --suite ./tasks/terminal-bench.json \
 *     --output ./results/$(date +%Y%m%d) \
 *     --tasks task1,task2 \
 *     --baseline ./previous-results.json
 */

import { parseArgs } from "util";
import { join } from "path";
import { existsSync, mkdirSync, writeFileSync, readFileSync } from "fs";
import { runClaudeCodeSubagent } from "../agent/orchestrator/claude-code-subagent.js";
import type { Subtask } from "../agent/orchestrator/types.js";
import {
  loadTerminalBenchSuite,
  runTaskSetup,
  toBenchmarkResults,
  type TerminalBenchTask,
  type TerminalBenchSuite,
  type TerminalBenchResults,
} from "../bench/terminal-bench.js";
import { BunContext } from "@effect/platform-bun";
import { Effect } from "effect";
import { cpSync, readdirSync, statSync } from "fs";

// ============================================================================
// CLI Argument Parsing
// ============================================================================

interface TBenchLocalArgs {
  suite: string;
  output: string;
  tasks: string | undefined;
  baseline: string | undefined;
  timeout: number | undefined;
  maxTurns: number | undefined;
  parallel: number | undefined;
  help: boolean | undefined;
}

const parseCliArgs = (): TBenchLocalArgs => {
  const { values } = parseArgs({
    args: process.argv.slice(2),
    options: {
      suite: { type: "string", short: "s" },
      output: { type: "string", short: "o" },
      tasks: { type: "string", short: "t" },
      baseline: { type: "string", short: "b" },
      timeout: { type: "string" },
      "max-turns": { type: "string" },
      parallel: { type: "string", short: "p" },
      help: { type: "boolean", short: "h" },
    },
  });

  if (values.help) {
    console.log(`
Terminal-Bench Local CLI

Run Terminal-Bench tasks locally without Harbor/Docker.

Usage:
  bun src/cli/tbench-local.ts --suite <file> --output <dir> [options]

Required:
  -s, --suite       Path to Terminal-Bench suite JSON file
  -o, --output      Output directory for results

Options:
  -t, --tasks       Comma-separated task IDs to run (default: all)
  -b, --baseline    Path to baseline results JSON for comparison
      --timeout     Task timeout in seconds (default: 3600)
      --max-turns   Max agent turns per task (default: 300)
  -p, --parallel    Run tasks in parallel (default: 1)
  -h, --help        Show this help message

Examples:
  # Run all tasks in suite
  bun src/cli/tbench-local.ts -s ./tasks/tb-2.0.json -o ./results

  # Run specific tasks
  bun src/cli/tbench-local.ts -s ./tasks/tb-2.0.json -o ./results -t task1,task2

  # Compare with baseline
  bun src/cli/tbench-local.ts -s ./tasks/tb-2.0.json -o ./results -b ./baseline.json
`);
    process.exit(0);
  }

  if (!values.suite) {
    console.error("Error: --suite is required");
    process.exit(1);
  }

  if (!values.output) {
    console.error("Error: --output is required");
    process.exit(1);
  }

  return {
    suite: values.suite,
    output: values.output,
    tasks: values.tasks,
    baseline: values.baseline,
    timeout: values.timeout ? parseInt(values.timeout, 10) : undefined,
    maxTurns: values["max-turns"] ? parseInt(values["max-turns"], 10) : undefined,
    parallel: values.parallel ? parseInt(values.parallel, 10) : undefined,
    help: values.help,
  };
};

// ============================================================================
// Task Runner
// ============================================================================

interface TaskResult {
  taskId: string;
  outcome: "success" | "failure" | "timeout" | "error";
  durationMs: number;
  turns: number;
  tokens: number;
  verificationOutput: string | undefined;
  errorMessage: string | undefined;
}

/**
 * Set up task workspace with environment files and tests
 */
const setupTaskWorkspace = (
  tbTask: TerminalBenchTask,
  workspaceDir: string,
  sourceRepo?: string
): void => {
  mkdirSync(workspaceDir, { recursive: true });

  // If source_path is available, copy environment and test files
  const sourcePath = tbTask.source_path ?? (sourceRepo ? join(sourceRepo, tbTask.id) : null);
  if (sourcePath && existsSync(sourcePath)) {
    // Copy environment files if they exist (excluding Dockerfile)
    const envDir = join(sourcePath, "environment");
    if (existsSync(envDir)) {
      const entries = readdirSync(envDir);
      for (const entry of entries) {
        if (entry === "Dockerfile") continue;
        const srcPath = join(envDir, entry);
        const destPath = join(workspaceDir, entry);
        if (statSync(srcPath).isDirectory()) {
          cpSync(srcPath, destPath, { recursive: true });
        } else {
          cpSync(srcPath, destPath);
        }
      }
    }

    // Copy test files, modifying /app/ paths to workspace
    const testsDir = join(sourcePath, "tests");
    const destTestsDir = join(workspaceDir, "tests");
    if (existsSync(testsDir)) {
      mkdirSync(destTestsDir, { recursive: true });
      const testFiles = readdirSync(testsDir);
      for (const file of testFiles) {
        const srcFile = join(testsDir, file);
        const destFile = join(destTestsDir, file);
        if (statSync(srcFile).isFile()) {
          let content = readFileSync(srcFile, "utf-8");
          // Replace /app/ with workspace path for local execution
          content = content.replace(/\/app\//g, `${workspaceDir}/`);
          content = content.replace(/\/app(?=["'])/g, workspaceDir);
          writeFileSync(destFile, content);
        }
      }
    }
  }
};

/**
 * Run verification for a task in its workspace
 */
const runLocalVerification = async (
  workspaceDir: string,
  tbTask: TerminalBenchTask
): Promise<{ passed: boolean; output: string }> => {
  const testsDir = join(workspaceDir, "tests");

  // Check if tests directory exists
  if (!existsSync(testsDir)) {
    // Fall back to simple verification if defined
    if (tbTask.verification.type === "output" && tbTask.verification.command) {
      const proc = Bun.spawn(["sh", "-c", tbTask.verification.command], {
        cwd: workspaceDir,
        stdout: "pipe",
        stderr: "pipe",
      });
      await proc.exited;
      const stdout = await new Response(proc.stdout).text();
      const expected = tbTask.verification.expected ?? "";
      const passed = stdout.trim() === expected.trim();
      return {
        passed,
        output: `Expected: ${expected}\nActual: ${stdout}`,
      };
    }
    return { passed: false, output: "No tests directory found" };
  }

  // Run pytest on the tests
  const proc = Bun.spawn(["python3", "-m", "pytest", "tests/", "-v", "--tb=short"], {
    cwd: workspaceDir,
    stdout: "pipe",
    stderr: "pipe",
    env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" },
  });

  const exitCode = await proc.exited;
  const stdout = await new Response(proc.stdout).text();
  const stderr = await new Response(proc.stderr).text();

  return {
    passed: exitCode === 0,
    output: stdout + (stderr ? `\nSTDERR:\n${stderr}` : ""),
  };
};

const runTask = async (
  tbTask: TerminalBenchTask,
  options: {
    cwd: string;
    timeout: number;
    maxTurns: number;
    outputDir: string;
    sourceRepo?: string;
  }
): Promise<TaskResult> => {
  const startTime = Date.now();
  const taskOutputDir = join(options.outputDir, tbTask.id);
  mkdirSync(taskOutputDir, { recursive: true });

  // Create workspace for the task
  const workspaceDir = join(taskOutputDir, "workspace");

  console.log(`\n=== Running Task: ${tbTask.id} ===`);
  console.log(`Name: ${tbTask.name}`);
  console.log(`Difficulty: ${tbTask.difficulty}`);
  console.log(`Category: ${tbTask.category}`);
  console.log(`Workspace: ${workspaceDir}`);

  // Set up workspace with environment and test files
  setupTaskWorkspace(tbTask, workspaceDir, options.sourceRepo);

  // Run setup if specified
  if (tbTask.setup?.length) {
    console.log("Running setup commands...");
    try {
      for (const cmd of tbTask.setup) {
        const proc = Bun.spawn(["sh", "-c", cmd], {
          cwd: workspaceDir,
          stdout: "pipe",
          stderr: "pipe",
        });
        const exitCode = await proc.exited;
        if (exitCode !== 0) {
          throw new Error(`Setup command failed: ${cmd}`);
        }
      }
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      console.error(`Setup failed: ${errorMsg}`);
      return {
        taskId: tbTask.id,
        outcome: "error",
        durationMs: Date.now() - startTime,
        turns: 0,
        tokens: 0,
        verificationOutput: undefined,
        errorMessage: `Setup failed: ${errorMsg}`,
      };
    }
  }

  // Create subtask for Claude Code
  const subtask: Subtask = {
    id: tbTask.id,
    description: tbTask.description,
    status: "in_progress",
    startedAt: new Date().toISOString(),
  };

  // Track output
  let outputText = "";
  const onOutput = (text: string): void => {
    process.stdout.write(text);
    outputText += text;
  };

  let result;
  try {
    result = await runClaudeCodeSubagent(subtask, {
      cwd: workspaceDir,
      maxTurns: tbTask.max_turns ?? options.maxTurns,
      permissionMode: "bypassPermissions",
      timeoutMs: (tbTask.timeout_seconds ?? options.timeout) * 1000,
      onOutput,
    });

    // Save raw output
    writeFileSync(join(taskOutputDir, "output.txt"), outputText);
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e);
    writeFileSync(join(taskOutputDir, "error.txt"), errorMsg);
    return {
      taskId: tbTask.id,
      outcome: "error",
      durationMs: Date.now() - startTime,
      turns: 0,
      tokens: 0,
      verificationOutput: undefined,
      errorMessage: errorMsg,
    };
  }

  const durationMs = Date.now() - startTime;
  const usage = result.sessionMetadata?.usage;
  const tokens = (usage?.inputTokens ?? 0) + (usage?.outputTokens ?? 0);

  // Run verification
  console.log("\nRunning verification...");
  let verificationResult;
  try {
    verificationResult = await runLocalVerification(workspaceDir, tbTask);
    writeFileSync(join(taskOutputDir, "verification.txt"), verificationResult.output);
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e);
    verificationResult = { passed: false, output: `Verification error: ${errorMsg}` };
  }

  // Determine outcome
  let outcome: TaskResult["outcome"];
  if (!result.success) {
    if (result.error?.includes("timeout") || result.error?.includes("timed out")) {
      outcome = "timeout";
    } else {
      outcome = "error";
    }
  } else if (verificationResult.passed) {
    outcome = "success";
  } else {
    outcome = "failure";
  }

  console.log(`\n=== Task ${tbTask.id} Complete ===`);
  console.log(`Outcome: ${outcome}`);
  console.log(`Duration: ${(durationMs / 1000).toFixed(1)}s`);
  console.log(`Turns: ${result.turns}`);
  console.log(`Tokens: ${tokens}`);
  console.log(`Verification: ${verificationResult.passed ? "PASSED" : "FAILED"}`);

  return {
    taskId: tbTask.id,
    outcome,
    durationMs,
    turns: result.turns,
    tokens,
    verificationOutput: verificationResult.output,
    errorMessage: result.error,
  };
};

// ============================================================================
// Report Generation
// ============================================================================

interface ComparisonReport {
  current: TerminalBenchResults;
  baseline: TerminalBenchResults | undefined;
  comparison: {
    passRateDelta: number | undefined;
    avgDurationDelta: number | undefined;
    avgTurnsDelta: number | undefined;
    totalTokensDelta: number | undefined;
    improved: string[];
    regressed: string[];
    unchanged: string[];
  } | undefined;
}

const generateComparisonReport = (
  current: TerminalBenchResults,
  baseline: TerminalBenchResults | undefined
): ComparisonReport => {
  if (!baseline) {
    return { current, baseline: undefined, comparison: undefined };
  }

  const baselineMap = new Map(baseline.results.map((r) => [r.task_id, r]));
  const improved: string[] = [];
  const regressed: string[] = [];
  const unchanged: string[] = [];

  for (const result of current.results) {
    const baselineResult = baselineMap.get(result.task_id);
    if (!baselineResult) continue;

    if (result.status === "pass" && baselineResult.status !== "pass") {
      improved.push(result.task_id);
    } else if (result.status !== "pass" && baselineResult.status === "pass") {
      regressed.push(result.task_id);
    } else {
      unchanged.push(result.task_id);
    }
  }

  return {
    current,
    baseline,
    comparison: {
      passRateDelta: current.summary.pass_rate - baseline.summary.pass_rate,
      avgDurationDelta: current.summary.avg_duration_ms - baseline.summary.avg_duration_ms,
      avgTurnsDelta: current.summary.avg_turns - baseline.summary.avg_turns,
      totalTokensDelta: current.summary.total_tokens - baseline.summary.total_tokens,
      improved,
      regressed,
      unchanged,
    },
  };
};

const formatMarkdownReport = (report: ComparisonReport): string => {
  const { current, comparison } = report;
  let md = `# Terminal-Bench Results

## Summary

| Metric | Value |
|--------|-------|
| Suite | ${current.suite_name} v${current.suite_version} |
| Model | ${current.model} |
| Timestamp | ${current.timestamp} |
| Pass Rate | ${(current.summary.pass_rate * 100).toFixed(1)}% |
| Total Tasks | ${current.summary.total} |
| Passed | ${current.summary.passed} |
| Failed | ${current.summary.failed} |
| Timeout | ${current.summary.timeout} |
| Error | ${current.summary.error} |
| Avg Duration | ${(current.summary.avg_duration_ms / 1000).toFixed(1)}s |
| Avg Turns | ${current.summary.avg_turns.toFixed(1)} |
| Total Tokens | ${current.summary.total_tokens.toLocaleString()} |

`;

  if (comparison) {
    const delta = (n: number | undefined, suffix = "") =>
      n === undefined ? "N/A" : `${n >= 0 ? "+" : ""}${n.toFixed(1)}${suffix}`;

    md += `## Comparison with Baseline

| Metric | Delta |
|--------|-------|
| Pass Rate | ${delta(comparison.passRateDelta !== undefined ? comparison.passRateDelta * 100 : undefined, "%")} |
| Avg Duration | ${delta(comparison.avgDurationDelta !== undefined ? comparison.avgDurationDelta / 1000 : undefined, "s")} |
| Avg Turns | ${delta(comparison.avgTurnsDelta)} |
| Total Tokens | ${delta(comparison.totalTokensDelta)} |

### Improved (${comparison.improved.length})
${comparison.improved.length > 0 ? comparison.improved.map((t) => `- ${t}`).join("\n") : "_None_"}

### Regressed (${comparison.regressed.length})
${comparison.regressed.length > 0 ? comparison.regressed.map((t) => `- ${t}`).join("\n") : "_None_"}

`;
  }

  md += `## Results by Task

| Task ID | Status | Duration | Turns | Tokens |
|---------|--------|----------|-------|--------|
`;

  for (const result of current.results) {
    const status = result.status === "pass" ? "✓" : result.status === "fail" ? "✗" : result.status;
    md += `| ${result.task_id} | ${status} | ${(result.duration_ms / 1000).toFixed(1)}s | ${result.turns} | ${result.tokens_used.toLocaleString()} |\n`;
  }

  return md;
};

// ============================================================================
// Main Execution
// ============================================================================

const main = async (): Promise<void> => {
  const args = parseCliArgs();

  // Load suite
  console.log(`Loading suite from ${args.suite}...`);
  let suite: TerminalBenchSuite;
  try {
    suite = await Effect.runPromise(
      loadTerminalBenchSuite(args.suite).pipe(Effect.provide(BunContext.layer))
    );
    console.log(`Loaded suite: ${suite.name} v${suite.version}`);
    console.log(`Tasks: ${suite.tasks.length}`);
  } catch (e) {
    console.error(`Failed to load suite: ${e}`);
    process.exit(1);
  }

  // Filter tasks if specified
  let tasksToRun = suite.tasks;
  if (args.tasks) {
    const taskIds = new Set(args.tasks.split(",").map((t) => t.trim()));
    tasksToRun = suite.tasks.filter((t) => taskIds.has(t.id));
    console.log(`Running ${tasksToRun.length} selected tasks`);
  }

  // Ensure output directory
  if (!existsSync(args.output)) {
    mkdirSync(args.output, { recursive: true });
  }

  // Load baseline if specified
  let baseline: TerminalBenchResults | undefined;
  if (args.baseline && existsSync(args.baseline)) {
    try {
      baseline = JSON.parse(readFileSync(args.baseline, "utf-8"));
      console.log(`Loaded baseline: ${baseline?.suite_name}`);
    } catch (e) {
      console.warn(`Failed to load baseline: ${e}`);
    }
  }

  // Run tasks
  const results: TaskResult[] = [];
  const timeout = args.timeout ?? 3600;
  const maxTurns = args.maxTurns ?? 300;
  const cwd = process.cwd();

  console.log(`\n=== Starting Terminal-Bench Run ===`);
  console.log(`Suite: ${suite.name}`);
  console.log(`Tasks: ${tasksToRun.length}`);
  console.log(`Timeout: ${timeout}s`);
  console.log(`Max Turns: ${maxTurns}`);
  console.log(`Output: ${args.output}`);
  console.log(`================================\n`);

  const startTime = Date.now();

  // Get source repo path from suite if available
  const sourceRepo = suite.source_repo;

  // Run tasks sequentially (parallel support can be added later)
  for (const task of tasksToRun) {
    const result = await runTask(task, {
      cwd,
      timeout,
      maxTurns,
      outputDir: args.output,
      sourceRepo,
    });
    results.push(result);

    // Save intermediate results
    const intermediateResults = toBenchmarkResults(suite, "claude-code", results);
    writeFileSync(
      join(args.output, "results.json"),
      JSON.stringify(intermediateResults, null, 2)
    );
  }

  const totalDuration = Date.now() - startTime;

  // Generate final results
  const finalResults = toBenchmarkResults(suite, "claude-code", results);
  writeFileSync(join(args.output, "results.json"), JSON.stringify(finalResults, null, 2));

  // Generate comparison report
  const report = generateComparisonReport(finalResults, baseline);
  writeFileSync(join(args.output, "report.json"), JSON.stringify(report, null, 2));
  writeFileSync(join(args.output, "report.md"), formatMarkdownReport(report));

  // Print summary
  console.log(`\n=== Terminal-Bench Run Complete ===`);
  console.log(`Total Duration: ${(totalDuration / 1000 / 60).toFixed(1)} minutes`);
  console.log(`Pass Rate: ${(finalResults.summary.pass_rate * 100).toFixed(1)}%`);
  console.log(`Passed: ${finalResults.summary.passed}/${finalResults.summary.total}`);
  console.log(`Failed: ${finalResults.summary.failed}`);
  console.log(`Timeout: ${finalResults.summary.timeout}`);
  console.log(`Error: ${finalResults.summary.error}`);
  console.log(`Total Tokens: ${finalResults.summary.total_tokens.toLocaleString()}`);
  console.log(`Output: ${args.output}`);

  if (report.comparison) {
    console.log(`\nComparison with baseline:`);
    console.log(`  Pass Rate Delta: ${((report.comparison.passRateDelta ?? 0) * 100).toFixed(1)}%`);
    console.log(`  Improved: ${report.comparison.improved.length} tasks`);
    console.log(`  Regressed: ${report.comparison.regressed.length} tasks`);
  }

  console.log(`===================================\n`);

  // Exit with appropriate code
  const allPassed = finalResults.summary.failed === 0 &&
                    finalResults.summary.timeout === 0 &&
                    finalResults.summary.error === 0;
  process.exit(allPassed ? 0 : 1);
};

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(2);
});
