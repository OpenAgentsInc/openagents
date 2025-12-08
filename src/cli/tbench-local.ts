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
// import { runClaudeCodeSubagent } from "../agent/orchestrator/claude-code-subagent.js";
import {
  loadTerminalBenchSuite,
  toBenchmarkResults,
  type TerminalBenchTask,
  type TerminalBenchSuite,
  type TerminalBenchResults,
} from "../bench/terminal-bench.js";
import {
  createModelRunner,
  parseModelString,
  type ModelRunner,
} from "../bench/model-adapter.js";
import { BunContext } from "@effect/platform-bun";
import { Effect } from "effect";
import { cpSync, readdirSync, statSync } from "fs";
import { createTBEmitter, type TBEmitter } from "../tbench-hud/emit.js";
import {
  saveTBRun,
  convertResultsToTBRunFile,
} from "../tbench-hud/persistence.js";
import type { Trajectory } from "../atif/schema.js";

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
  runId: string | undefined;
  model: string;
  hudUrl: string | undefined;
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
      "run-id": { type: "string" },
      model: { type: "string", short: "m" },
      "hud-url": { type: "string" },
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
  -m, --model       Model to use: claude-code, fm, foundation-models, ollama:<model>
                    (default: claude-code)
      --timeout     Task timeout in seconds (default: 3600)
      --max-turns   Max agent turns per task (default: 300)
  -p, --parallel    Run tasks in parallel (default: 1)
      --hud-url     WebSocket URL for HUD events (default: ws://localhost:8080/ws)
  -h, --help        Show this help message

Examples:
  # Run all tasks with Claude Code (default)
  bun src/cli/tbench-local.ts -s ./tasks/tb-2.0.json -o ./results

  # Run with Apple Foundation Models
  bun src/cli/tbench-local.ts -s ./tasks/fm-mini-suite.json -o ./results -m fm

  # Run specific tasks with FM
  bun src/cli/tbench-local.ts -s ./tasks/fm-mini-suite.json -o ./results -t hello-world -m fm

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
    runId: values["run-id"],
    model: values.model ?? "claude-code",
    hudUrl: values["hud-url"],
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

  // Handle inline setup files (from FM mini-suite format)
  const setupConfig = tbTask.setup as { files?: Record<string, string> } | string[] | undefined;
  if (setupConfig && typeof setupConfig === "object" && !Array.isArray(setupConfig) && setupConfig.files) {
    for (const [filename, content] of Object.entries(setupConfig.files)) {
      const filePath = join(workspaceDir, filename);
      const dir = join(filePath, "..");
      if (!existsSync(dir)) {
        mkdirSync(dir, { recursive: true });
      }
      writeFileSync(filePath, content);
    }
  }

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
  // Handle custom verification (shell script)
  if (tbTask.verification.type === "custom") {
    const script = tbTask.verification.script ?? tbTask.verification.command ?? "exit 1";
    const proc = Bun.spawn(["sh", "-c", script], {
      cwd: workspaceDir,
      stdout: "pipe",
      stderr: "pipe",
    });
    const exitCode = await proc.exited;
    const stdout = await new Response(proc.stdout).text();
    const stderr = await new Response(proc.stderr).text();
    return {
      passed: exitCode === 0,
      output: `Script: ${script}\nExit code: ${exitCode}\n${stdout}${stderr ? `\nSTDERR:\n${stderr}` : ""}`,
    };
  }

  // Handle output verification (command with expected output)
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

  // Handle test verification (pytest)
  const testsDir = join(workspaceDir, "tests");
  if (existsSync(testsDir)) {
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
  }

  return { passed: false, output: "No verification method available" };
};

const runTask = async (
  tbTask: TerminalBenchTask,
  options: {
    cwd: string;
    timeout: number;
    maxTurns: number;
    outputDir: string;
    sourceRepo?: string;
    runId?: string; // TB run ID for ATIF step emission to HUD
    tbEmitter?: TBEmitter;
    taskIndex?: number;
    totalTasks?: number;
    modelRunner: ModelRunner;
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

  // Emit task start to HUD
  if (options.tbEmitter && options.taskIndex !== undefined && options.totalTasks !== undefined) {
    options.tbEmitter.taskStart(
      { id: tbTask.id, name: tbTask.name, category: tbTask.category, difficulty: tbTask.difficulty },
      options.taskIndex,
      options.totalTasks
    );
    options.tbEmitter.taskProgress(tbTask.id, "setup", undefined, 0);
  }

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
  // TODO: Use this subtask to track Claude Code execution (need Subtask type)
  /*
  const subtask: Subtask = {
    id: tbTask.id,
    description: tbTask.description,
    status: "in_progress",
    startedAt: new Date().toISOString(),
  };
  */

  // Track output
  let outputText = "";
  let turnCount = 0;
  const onOutput = (text: string): void => {
    process.stdout.write(text);
    outputText += text;
    // Send output to HUD
    if (options.tbEmitter) {
      options.tbEmitter.taskOutput(tbTask.id, text, "agent");
      // Track turns (rough estimate based on output patterns)
      if (text.includes("[TOOL_USE]") || text.includes("[RESULT]")) {
        turnCount++;
        options.tbEmitter.taskProgress(tbTask.id, "agent", turnCount, Date.now() - startTime);
      }
    }
  };

  // Emit agent phase start
  if (options.tbEmitter) {
    options.tbEmitter.taskProgress(tbTask.id, "agent", 0, Date.now() - startTime);
  }

  let result;
  try {
    console.log("[TB] Running task with model:", options.modelRunner.modelName);
    console.log("[TB] Task:", tbTask.id);
    console.log("[TB] Workspace:", workspaceDir);
    console.log("[TB] MaxTurns:", tbTask.max_turns ?? options.maxTurns);

    // Use the model runner (supports Claude Code, FM, Ollama)
    const runResult = await options.modelRunner.runTask(tbTask, {
      workspace: workspaceDir,
      timeout: tbTask.timeout_seconds ?? options.timeout,
      maxTurns: tbTask.max_turns ?? options.maxTurns,
      runId: options.runId,
      onOutput,
    });

    // Convert TaskRunResult to the expected format
    result = {
      success: runResult.success,
      turns: runResult.turns,
      error: runResult.error,
      sessionMetadata: {
        usage: {
          inputTokens: runResult.tokens,
          outputTokens: 0,
        },
        ...runResult.sessionMetadata,
      },
    };

    // Give time to clean up background processes (prevent AbortError during cleanup)
    await new Promise(resolve => setTimeout(resolve, 250));

    // Save raw output
    writeFileSync(join(taskOutputDir, "output.txt"), outputText);

    // Check for silent failures (0 turns = agent didn't run)
    if (!result.success && result.turns === 0) {
      const errorMsg = result.error || "Agent session started but did not process any turns (SDK silent failure)";
      console.error(`\n❌ Agent failure: ${errorMsg}`);
      if (result.error) {
        console.error(`   Details: ${result.error}`);
      }
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
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e);
    const fullError = e instanceof Error && e.stack ? `${errorMsg}\n\nStack:\n${e.stack}` : errorMsg;
    console.error(`\n❌ Exception during agent run: ${errorMsg}`);
    writeFileSync(join(taskOutputDir, "error.txt"), fullError);
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

  // Surface error details if agent failed
  if (!result.success && result.error) {
    console.error(`\n⚠️  Agent completed unsuccessfully: ${result.error}`);
  }

  // Run verification
  console.log("\nRunning verification...");
  if (options.tbEmitter) {
    options.tbEmitter.taskProgress(tbTask.id, "verification", undefined, Date.now() - startTime);
  }
  let verificationResult;
  try {
    verificationResult = await runLocalVerification(workspaceDir, tbTask);
    writeFileSync(join(taskOutputDir, "verification.txt"), verificationResult.output);
    if (options.tbEmitter) {
      options.tbEmitter.taskOutput(tbTask.id, verificationResult.output, "verification");
    }
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e);
    verificationResult = { passed: false, output: `Verification error: ${errorMsg}` };
    if (options.tbEmitter) {
      options.tbEmitter.taskOutput(tbTask.id, verificationResult.output, "verification");
    }
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
  if (result.error) {
    console.log(`Error: ${result.error}`);
  }

  // Emit task complete to HUD
  if (options.tbEmitter) {
    options.tbEmitter.taskComplete(tbTask.id, {
      outcome,
      durationMs,
      turns: result.turns,
      tokens,
      verificationOutput: verificationResult.output,
    });
  }

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

/**
 * Load ATIF trajectory if available.
 * Trajectory may be saved to output directory during run, or not available.
 *
 * @param outputDir - Output directory where trajectory might be saved
 * @returns Trajectory if found, undefined otherwise
 */
const loadTrajectoryIfAvailable = async (
  outputDir: string
): Promise<Trajectory | undefined> => {
  // Try to load from output directory
  const trajectoryPath = join(outputDir, "trajectory.json");
  if (existsSync(trajectoryPath)) {
    try {
      const content = JSON.parse(readFileSync(trajectoryPath, "utf-8"));
      return content as Trajectory;
    } catch (err) {
      console.warn(`[TB] Failed to load trajectory: ${err}`);
    }
  }

  // TODO: In the future, could collect from TBEmitter if it buffers trajectory steps
  return undefined;
};

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

  // Create model runner from args.model
  const modelConfig = parseModelString(args.model);
  const modelRunner = createModelRunner(modelConfig);

  // Check model health
  console.log(`\nChecking model health: ${modelRunner.modelName}...`);
  const healthCheck = await modelRunner.checkHealth();
  if (!healthCheck.available) {
    console.error(`Error: Model ${modelRunner.modelName} is not available`);
    console.error(`  ${healthCheck.error}`);
    process.exit(1);
  }
  console.log(`Model ${modelRunner.modelName} is available`);

  console.log(`\n=== Starting Terminal-Bench Run ===`);
  console.log(`Suite: ${suite.name}`);
  console.log(`Model: ${modelRunner.modelName}`);
  console.log(`Tasks: ${tasksToRun.length}`);
  console.log(`Timeout: ${timeout}s`);
  console.log(`Max Turns: ${maxTurns}`);
  console.log(`Output: ${args.output}`);
  console.log(`================================\n`);

  const startTime = Date.now();

  // Create TB HUD emitter (silently fails if HUD not running)
  const tbEmitter = createTBEmitter(args.hudUrl ? { url: args.hudUrl } : undefined);

  // Emit run start to HUD
  const suiteInfo = {
    name: suite.name,
    version: suite.version,
    tasks: suite.tasks.map((t) => ({
      id: t.id,
      name: t.name,
      category: t.category,
      difficulty: t.difficulty,
    })),
  };
  tbEmitter.runStart(suiteInfo, tasksToRun.map((t) => t.id));

  // Get source repo path from suite if available
  const sourceRepo = suite.source_repo;

  // Run tasks sequentially (parallel support can be added later)
  for (let i = 0; i < tasksToRun.length; i++) {
    const task = tasksToRun[i];
    const result = await runTask(task, {
      cwd,
      timeout,
      maxTurns,
      outputDir: args.output,
      modelRunner,
      ...(sourceRepo !== undefined ? { sourceRepo } : {}),
      ...(args.runId ? { runId: args.runId } : {}), // Pass runId for ATIF step emission to HUD
      tbEmitter,
      taskIndex: i,
      totalTasks: tasksToRun.length,
    });
    results.push(result);

    // Save intermediate results
    const intermediateResults = toBenchmarkResults(suite, modelRunner.modelName, results);
    writeFileSync(
      join(args.output, "results.json"),
      JSON.stringify(intermediateResults, null, 2)
    );
  }

  const totalDuration = Date.now() - startTime;

  // Generate final results
  const finalResults = toBenchmarkResults(suite, modelRunner.modelName, results);
  writeFileSync(join(args.output, "results.json"), JSON.stringify(finalResults, null, 2));

  // Generate comparison report
  const report = generateComparisonReport(finalResults, baseline);
  writeFileSync(join(args.output, "report.json"), JSON.stringify(report, null, 2));
  writeFileSync(join(args.output, "report.md"), formatMarkdownReport(report));

  // Emit run complete to HUD
  tbEmitter.runComplete({
    passRate: finalResults.summary.pass_rate,
    passed: finalResults.summary.passed,
    failed: finalResults.summary.failed,
    timeout: finalResults.summary.timeout,
    error: finalResults.summary.error,
    totalDurationMs: totalDuration,
  });
  tbEmitter.close();

  // Save TBRunFile format to .openagents/tb-runs/ for UI run browser
  if (args.runId) {
    try {
      // Load ATIF trajectory if available
      const trajectory = await loadTrajectoryIfAvailable(args.output);

      // Convert to TBRunFile format
      const runFile = convertResultsToTBRunFile(
        finalResults,
        args.runId,
        suite, // Full suite object for task metadata lookup
        trajectory
      );

      // Save to .openagents/tb-runs/
      const savedPath = await saveTBRun(runFile);
      console.log(`[TB] Run saved to: ${savedPath}`);
    } catch (err) {
      console.warn(`[TB] Failed to save TBRunFile: ${err}`);
      // Don't fail the run if persistence fails
    }
  } else {
    console.warn(`[TB] No runId provided, skipping TBRunFile save (use --run-id flag)`);
  }

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
