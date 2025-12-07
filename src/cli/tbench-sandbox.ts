#!/usr/bin/env bun
/**
 * Terminal-Bench Sandbox CLI
 *
 * Run Terminal-Bench tasks with hybrid container execution:
 * - Setup commands run in CONTAINER (isolated)
 * - Claude Code SDK runs on HOST (access to MCP, workspace)
 * - Verification runs in CONTAINER (isolated)
 *
 * Uses src/sandbox/ infrastructure for credential mounting and container execution.
 *
 * Usage:
 *   bun src/cli/tbench-sandbox.ts \
 *     --suite ./tasks/terminal-bench.json \
 *     --output ./results/$(date +%Y%m%d) \
 *     --sandbox-backend docker \
 *     --sandbox-image oven/bun:latest
 */

import { parseArgs } from "util";
import { join, resolve } from "path";
import { existsSync, mkdirSync, writeFileSync, readFileSync, cpSync, readdirSync, statSync } from "fs";
import { runClaudeCodeSubagent } from "../agent/orchestrator/claude-code-subagent.js";
import type { Subtask } from "../agent/orchestrator/types.js";
import {
  loadTerminalBenchSuite,
  type TerminalBenchTask,
  type TerminalBenchSuite,
  type TerminalBenchResults,
} from "../bench/terminal-bench.js";
import { BunContext } from "@effect/platform-bun";
import { Effect } from "effect";
import { createTBEmitter, type TBEmitter } from "../tbench-hud/emit.js";
import {
  runInContainer,
  createCredentialMount,
  cleanupCredentialMount,
  autoDetectLayer,
  type ContainerConfig,
  type CredentialMount,
} from "../sandbox/index.js";

// ============================================================================
// CLI Argument Parsing
// ============================================================================

interface TBenchSandboxArgs {
  suite: string;
  output: string;
  tasks: string | undefined;
  baseline: string | undefined;
  timeout: number | undefined;
  maxTurns: number | undefined;
  parallel: number | undefined;
  sandboxBackend: "docker" | "macos-container" | undefined;
  sandboxImage: string | undefined;
  runId: string | undefined;
  help: boolean | undefined;
}

const parseCliArgs = (): TBenchSandboxArgs => {
  const { values } = parseArgs({
    args: process.argv.slice(2),
    strict: false,
    options: {
      suite: { type: "string", short: "s" },
      output: { type: "string", short: "o" },
      tasks: { type: "string", short: "t" },
      baseline: { type: "string", short: "b" },
      timeout: { type: "string" },
      "max-turns": { type: "string" },
      parallel: { type: "string", short: "p" },
      "sandbox-backend": { type: "string" },
      "sandbox-image": { type: "string" },
      "run-id": { type: "string" },
      help: { type: "boolean", short: "h" },
    },
  });

  if (values.help) {
    console.log(`
Terminal-Bench Sandbox CLI

Run Terminal-Bench tasks with hybrid container execution.
SDK runs on host, setup/verification run in containers.

Usage:
  bun src/cli/tbench-sandbox.ts --suite <file> --output <dir> [options]

Required:
  -s, --suite            Path to Terminal-Bench suite JSON file
  -o, --output           Output directory for results

Options:
  -t, --tasks            Comma-separated task IDs to run (default: all)
  -b, --baseline         Path to baseline results JSON for comparison
      --timeout          Task timeout in seconds (default: 3600)
      --max-turns        Max agent turns per task (default: 300)
  -p, --parallel         Run tasks in parallel (default: 1)
      --sandbox-backend  Container backend: docker | macos-container (auto-detect)
      --sandbox-image    Container image (default: oven/bun:latest)
  -h, --help             Show this help message

Examples:
  # Run with auto-detected backend
  bun src/cli/tbench-sandbox.ts -s ./tasks/tb-2.0.json -o ./results

  # Run with Docker backend and custom image
  bun src/cli/tbench-sandbox.ts -s ./tasks/tb-2.0.json -o ./results \
    --sandbox-backend docker --sandbox-image ubuntu:22.04

  # Run specific tasks
  bun src/cli/tbench-sandbox.ts -s ./tasks/tb-2.0.json -o ./results -t task1,task2
`);
    process.exit(0);
  }

  const suiteValue = typeof values.suite === "string" ? values.suite : undefined;
  if (!suiteValue) {
    console.error("Error: --suite is required");
    process.exit(1);
  }

  const outputValue = typeof values.output === "string" ? values.output : undefined;
  if (!outputValue) {
    console.error("Error: --output is required");
    process.exit(1);
  }

  return {
    suite: suiteValue,
    output: outputValue,
    tasks: typeof values.tasks === "string" ? values.tasks : undefined,
    baseline: typeof values.baseline === "string" ? values.baseline : undefined,
    timeout:
      typeof values.timeout === "string" ? parseInt(values.timeout, 10) : undefined,
    maxTurns:
      typeof values["max-turns"] === "string"
        ? parseInt(values["max-turns"], 10)
        : undefined,
    parallel:
      typeof values.parallel === "string" ? parseInt(values.parallel, 10) : undefined,
    sandboxBackend:
      typeof values["sandbox-backend"] === "string"
        ? (values["sandbox-backend"] as "docker" | "macos-container")
        : undefined,
    sandboxImage:
      typeof values["sandbox-image"] === "string" ? values["sandbox-image"] : undefined,
    runId: typeof values["run-id"] === "string" ? values["run-id"] : undefined,
    help: typeof values.help === "boolean" ? values.help : undefined,
  };
};

// ============================================================================
// Task Runner
// ============================================================================

interface TaskResult {
  taskId: string;
  outcome: "success" | "failure" | "timeout" | "error" | "skip";
  durationMs: number;
  turns: number;
  tokens: number;
  verificationOutput: string | undefined;
  errorMessage: string | undefined;
}

/**
 * Set up task workspace with environment files and tests (same as local)
 */
const setupTaskWorkspace = (
  tbTask: TerminalBenchTask,
  workspaceDir: string,
  sourceRepo?: string
): void => {
  mkdirSync(workspaceDir, { recursive: true });

  const sourcePath = tbTask.source_path ?? (sourceRepo ? join(sourceRepo, tbTask.id) : null);
  if (sourcePath && existsSync(sourcePath)) {
    // Copy environment files (excluding Dockerfile)
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

    // Copy test files, modifying /app/ paths to /workspace
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
          // Replace /app/ with /workspace for container execution
          content = content.replace(/\/app\//g, "/workspace/");
          content = content.replace(/\/app(?=["'])/g, "/workspace");
          writeFileSync(destFile, content);
        }
      }
    }
  }
};

/**
 * Run verification in CONTAINER
 */
const runContainerVerification = async (
  workspaceDir: string,
  tbTask: TerminalBenchTask,
  containerConfig: ContainerConfig
): Promise<{ passed: boolean; output: string }> => {
  const testsDir = join(workspaceDir, "tests");

  // Check if tests directory exists
  if (!existsSync(testsDir)) {
    // Fall back to simple verification if defined
    if (tbTask.verification.type === "output" && tbTask.verification.command) {
      const result = await Effect.runPromise(
        Effect.provide(
          runInContainer(["/bin/sh", "-c", tbTask.verification.command], containerConfig),
          autoDetectLayer
        )
      );
      const expected = tbTask.verification.expected ?? "";
      const passed = result.stdout.trim() === expected.trim();
      return {
        passed,
        output: `Expected: ${expected}\nActual: ${result.stdout}`,
      };
    }
    return { passed: false, output: "No tests directory found" };
  }

  // Install pytest and run tests in a single container invocation
  // (combining install + test ensures pytest persists for the test run)
  console.log("Installing pytest and running verification in container...");
  const result = await Effect.runPromise(
    Effect.provide(
      runInContainer(
        [
          "/bin/sh",
          "-c",
          "python3 -m pip install --quiet pytest 2>/dev/null || pip install --quiet pytest 2>/dev/null && python3 -m pytest /workspace/tests/ -v --tb=short"
        ],
        {
          ...containerConfig,
          env: {
            ...(containerConfig.env ?? {}),
            PYTHONDONTWRITEBYTECODE: "1",
          },
        }
      ),
      autoDetectLayer
    )
  );

  return {
    passed: result.exitCode === 0,
    output: result.stdout + (result.stderr ? `\nSTDERR:\n${result.stderr}` : ""),
  };
};

/**
 * Run setup commands in CONTAINER
 */
const runContainerSetup = async (
  setupCommands: string[],
  containerConfig: ContainerConfig
): Promise<void> => {
  for (const cmd of setupCommands) {
    console.log(`  Running setup: ${cmd}`);
    const result = await Effect.runPromise(
      Effect.provide(
        runInContainer(["/bin/sh", "-c", cmd], containerConfig),
        autoDetectLayer
      )
    );
    if (result.exitCode !== 0) {
      throw new Error(`Setup command failed: ${cmd}\nOutput: ${result.stderr || result.stdout}`);
    }
  }
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
    sandboxImage: string;
  }
): Promise<TaskResult> => {
  const startTime = Date.now();
  const taskOutputDir = join(options.outputDir, tbTask.id);
  mkdirSync(taskOutputDir, { recursive: true });

  // Convert to absolute path for Docker volume mounts
  const workspaceDir = resolve(join(taskOutputDir, "workspace"));

  console.log(`\n=== Running Task (Sandbox Mode): ${tbTask.id} ===`);
  console.log(`Name: ${tbTask.name}`);
  console.log(`Difficulty: ${tbTask.difficulty}`);
  console.log(`Category: ${tbTask.category}`);
  console.log(`Workspace: ${workspaceDir}`);
  console.log(`Container Image: ${options.sandboxImage}`);

  // Emit task start to HUD
  if (options.tbEmitter && options.taskIndex !== undefined && options.totalTasks !== undefined) {
    options.tbEmitter.taskStart(
      { id: tbTask.id, name: tbTask.name, category: tbTask.category, difficulty: tbTask.difficulty },
      options.taskIndex,
      options.totalTasks
    );
    options.tbEmitter.taskProgress(tbTask.id, "setup", undefined, 0);
  }

  // Set up workspace
  setupTaskWorkspace(tbTask, workspaceDir, options.sourceRepo);

  // Create credential mount for SDK authentication
  console.log("Creating credential mount...");
  let credentialMount: CredentialMount | null = null;
  try {
    credentialMount = await Effect.runPromise(
      Effect.provide(createCredentialMount(), BunContext.layer)
    );
    console.log(`  Credentials mounted: ${credentialMount.volumeMount}`);
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e);
    console.error(`Failed to create credential mount: ${errorMsg}`);
    return {
      taskId: tbTask.id,
      outcome: "error",
      durationMs: Date.now() - startTime,
      turns: 0,
      tokens: 0,
      verificationOutput: undefined,
      errorMessage: `Credential mount failed: ${errorMsg}`,
    };
  }

  // Build container config
  const containerConfig: ContainerConfig = {
    image: options.sandboxImage,
    workspaceDir,
    workdir: "/workspace",
    volumeMounts: [credentialMount.volumeMount],
    timeoutMs: (tbTask.timeout_seconds ?? options.timeout) * 1000,
    autoRemove: true,
  };

  // Run setup in CONTAINER
  if (tbTask.setup?.length) {
    console.log("Running setup commands in container...");
    try {
      await runContainerSetup(Array.from(tbTask.setup), containerConfig);
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      console.error(`Setup failed: ${errorMsg}`);
      await Effect.runPromise(
        Effect.provide(cleanupCredentialMount(credentialMount), BunContext.layer)
      );
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
  let turnCount = 0;
  const onOutput = (text: string): void => {
    process.stdout.write(text);
    outputText += text;
    if (options.tbEmitter) {
      options.tbEmitter.taskOutput(tbTask.id, text, "agent");
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

  // Run Claude Code SDK on HOST (not in container)
  let result;
  try {
    console.log("[TB Sandbox] Calling runClaudeCodeSubagent on HOST for task:", tbTask.id);
    console.log("[TB Sandbox] Workspace (host path):", workspaceDir);
    console.log("[TB Sandbox] MaxTurns:", tbTask.max_turns ?? options.maxTurns);
    result = await runClaudeCodeSubagent(subtask, {
      cwd: workspaceDir,
      maxTurns: tbTask.max_turns ?? options.maxTurns,
      permissionMode: "bypassPermissions",
      timeoutMs: (tbTask.timeout_seconds ?? options.timeout) * 1000,
      ...(options.runId ? { runId: options.runId } : {}), // Conditional spread for exactOptionalPropertyTypes
      onOutput,
    });

    // Give SDK time to clean up
    await new Promise(resolve => setTimeout(resolve, 250));

    // Save raw output
    writeFileSync(join(taskOutputDir, "output.txt"), outputText);

    // Check for silent failures
    if (!result.success && result.turns === 0) {
      const errorMsg = result.error || "Agent session started but did not process any turns (SDK silent failure)";
      console.error(`\n❌ Agent failure: ${errorMsg}`);
      if (result.error) {
        console.error(`   Details: ${result.error}`);
      }
      writeFileSync(join(taskOutputDir, "error.txt"), errorMsg);
      await Effect.runPromise(
        Effect.provide(cleanupCredentialMount(credentialMount), BunContext.layer)
      );
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
    await Effect.runPromise(
      Effect.provide(cleanupCredentialMount(credentialMount), BunContext.layer)
    );
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

  // Run verification in CONTAINER
  console.log("\nRunning verification in container...");
  if (options.tbEmitter) {
    options.tbEmitter.taskProgress(tbTask.id, "verification", undefined, Date.now() - startTime);
  }

  let verificationResult;
  try {
    verificationResult = await runContainerVerification(workspaceDir, tbTask, containerConfig);
    writeFileSync(join(taskOutputDir, "verification.txt"), verificationResult.output);
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e);
    console.error(`\n❌ Verification failed: ${errorMsg}`);
    verificationResult = { passed: false, output: `Verification error: ${errorMsg}` };
    writeFileSync(join(taskOutputDir, "verification.txt"), verificationResult.output);
  }

  // Cleanup credential mount
  try {
    await Effect.runPromise(
      Effect.provide(cleanupCredentialMount(credentialMount), BunContext.layer)
    );
  } catch (e) {
    console.warn(`Warning: Failed to cleanup credential mount: ${e instanceof Error ? e.message : String(e)}`);
  }

  // Determine final outcome
  let outcome: TaskResult["outcome"];
  if (verificationResult.passed) {
    outcome = "success";
    console.log("\n✅ Task PASSED");
  } else {
    outcome = "failure";
    console.log("\n❌ Task FAILED");
    console.log(verificationResult.output);
  }

  // Emit task completion
  if (options.tbEmitter) {
    options.tbEmitter.taskComplete(tbTask.id, {
      outcome,
      durationMs,
      turns: result.turns,
      tokens,
      verificationOutput: verificationResult.output,
    });
  }

  console.log(`Duration: ${(durationMs / 1000).toFixed(1)}s`);
  console.log(`Turns: ${result.turns}`);
  console.log(`Tokens: ${tokens}`);

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
// Main Execution
// ============================================================================

const main = async (): Promise<void> => {
  const args = parseCliArgs();
  const startTime = Date.now();

  console.log("=== Terminal-Bench Sandbox Runner ===");
  console.log(`Suite: ${args.suite}`);
  console.log(`Output: ${args.output}`);
  console.log(`Sandbox Backend: ${args.sandboxBackend ?? "auto-detect"}`);
  console.log(`Sandbox Image: ${args.sandboxImage ?? "oven/bun:latest"}`);
  console.log("=====================================\n");

  // Load suite
  console.log(`Loading suite from ${args.suite}...`);
  let suite: TerminalBenchSuite;
  try {
    suite = await Effect.runPromise(
      Effect.provide(loadTerminalBenchSuite(args.suite), BunContext.layer)
    );
  } catch (e) {
    console.error(`Failed to load suite: ${e instanceof Error ? e.message : String(e)}`);
    process.exit(1);
  }

  console.log(`Loaded suite: ${suite.name} v${suite.version}`);
  console.log(`Total tasks: ${suite.tasks.length}`);

  // Filter tasks if specified
  const taskIds = args.tasks?.split(",").map((s) => s.trim());
  const tasksToRun = taskIds
    ? suite.tasks.filter((t) => taskIds.includes(t.id))
    : suite.tasks;

  if (tasksToRun.length === 0) {
    console.error("No tasks to run!");
    process.exit(1);
  }

  console.log(`Running ${tasksToRun.length} task(s)`);

  // Set up output directory
  mkdirSync(args.output, { recursive: true });

  // Create TB emitter for HUD events
  const tbEmitter = createTBEmitter();
  const tbSuiteInfo = {
    name: suite.name,
    version: suite.version,
    tasks: tasksToRun.map((task) => ({
      id: task.id,
      name: task.name,
      category: task.category,
      difficulty: String(task.difficulty),
    })),
  };
  const suiteTaskIds = tasksToRun.map((task) => task.id);
  tbEmitter.runStart(tbSuiteInfo, suiteTaskIds);

  // Run tasks sequentially (parallel support can be added later)
  const results: TaskResult[] = [];
  const sandboxImage = args.sandboxImage ?? "oven/bun:latest";

  for (let i = 0; i < tasksToRun.length; i++) {
    const task = tasksToRun[i];
    const result = await runTask(task, {
      cwd: process.cwd(),
      timeout: args.timeout ?? 3600,
      maxTurns: args.maxTurns ?? 300,
      outputDir: args.output,
      ...(args.runId ? { runId: args.runId } : {}), // Conditional spread
      tbEmitter,
      taskIndex: i,
      totalTasks: tasksToRun.length,
      sandboxImage,
    });
    results.push(result);
  }

  // Calculate summary
  const passed = results.filter((r) => r.outcome === "success").length;
  const failed = results.filter((r) => r.outcome === "failure").length;
  const errors = results.filter((r) => r.outcome === "error").length;
  const timeouts = results.filter((r) => r.outcome === "timeout").length;
  const skipped = results.filter((r) => r.outcome === "skip").length;
  const passRate = tasksToRun.length > 0 ? (passed / tasksToRun.length) * 100 : 0;
  const totalDurationMs = Date.now() - startTime;
  const totalTokens = results.reduce((sum, r) => sum + r.tokens, 0);
  const avgDurationMs = tasksToRun.length > 0 ? totalDurationMs / tasksToRun.length : 0;
  const avgTurns =
    tasksToRun.length > 0
      ? results.reduce((sum, r) => sum + r.turns, 0) / tasksToRun.length
      : 0;

  // Emit run completion
  tbEmitter.runComplete(Date.now(), {
    passed,
    failed,
    timeout: timeouts,
    error: errors,
    passRate,
    totalDurationMs,
  });

  // Write results JSON
  const benchResults: TerminalBenchResults = {
    suite_name: suite.name,
    suite_version: suite.version,
    model: args.suite,
    timestamp: new Date().toISOString(),
    results: results.map((r) => {
      const status:
        | "pass"
        | "fail"
        | "timeout"
        | "error"
        | "skip" = r.outcome === "success"
        ? "pass"
        : r.outcome === "failure"
        ? "fail"
        : r.outcome === "timeout"
        ? "timeout"
        : r.outcome === "skip"
        ? "skip"
        : "error";
      return {
        task_id: r.taskId,
        status,
        duration_ms: r.durationMs,
        turns: r.turns,
        tokens_used: r.tokens,
        verification_output: r.verificationOutput,
        error_message: r.errorMessage,
      };
    }),
    summary: {
      total: tasksToRun.length,
      passed,
      failed,
      timeout: timeouts,
      error: errors,
      skipped,
      pass_rate: passRate,
      avg_duration_ms: avgDurationMs,
      avg_turns: avgTurns,
      total_tokens: totalTokens,
    },
  };

  writeFileSync(join(args.output, "results.json"), JSON.stringify(benchResults, null, 2));

  // Print summary
  console.log("\n=== Run Summary ===");
  console.log(`Pass Rate: ${passRate.toFixed(1)}% (${passed}/${tasksToRun.length})`);
  console.log(`Passed: ${passed}`);
  console.log(`Failed: ${failed}`);
  console.log(`Timeout: ${timeouts}`);
  console.log(`Error: ${errors}`);
  console.log(`Total Duration: ${(totalDurationMs / 1000).toFixed(1)}s`);
  console.log(`Total Tokens: ${totalTokens}`);
  console.log(`Results: ${args.output}/results.json`);
  console.log("===================\n");

  process.exit(failed + errors + timeouts > 0 ? 1 : 0);
};

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(2);
});
