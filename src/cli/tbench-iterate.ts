#!/usr/bin/env bun
/**
 * TerminalBench Iterate CLI
 *
 * Run multiple iterations of TerminalBench overnight with either Claude Code or Ollama.
 * Collects results as Episodes for future learning and improvement.
 *
 * Usage:
 *   # With Claude Code (default)
 *   bun src/cli/tbench-iterate.ts --suite ./tasks/tb-2.0.json --iterations 10
 *
 *   # With Ollama
 *   bun src/cli/tbench-iterate.ts --suite ./tasks/tb-2.0.json --model ollama:codellama:34b --iterations 20
 *
 *   # Mixed (Ollama primary, Claude for validation)
 *   bun src/cli/tbench-iterate.ts --suite ./tasks/tb-2.0.json --model ollama:codellama:34b \
 *     --claude-validation-rate 0.1 --iterations 20
 */

import { parseArgs } from "util";
import { join, resolve } from "path";
import { existsSync, mkdirSync, writeFileSync, readFileSync, cpSync, readdirSync, statSync } from "fs";
import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
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
  type ModelConfig,
  type TaskRunResult,
} from "../bench/model-adapter.js";
import {
  EpisodeStore,
  createEpisode,
  generateRunId,
  type Episode,
} from "../bench/episode-store.js";
import { createTBEmitter, type TBEmitter } from "../tbench-hud/emit.js";
import { loadProjectConfig, type ProjectServiceError } from "../tasks/project.js";
import type { ProjectConfig } from "../tasks/schema.js";
import { StreamingWriter } from "../atif/streaming-writer.js";
import { registerATIFDiskWriter, unregisterATIFDiskWriter } from "../atif/hud-emitter.js";
import {
  createEpisodeLearner,
  type LearningResult,
  type LearningSummary,
} from "../training/episode-learner.js";
import { SkillService, makeSkillServiceLive, type Skill } from "../skills/index.js";
import { ArchivistService, makeArchivistServiceLive } from "../archivist/service.js";
import type { Trajectory } from "../archivist/schema.js";

// ============================================================================
// CLI Argument Parsing
// ============================================================================

interface TBenchIterateArgs {
  suite: string;
  output: string;
  model: string;
  ollamaEndpoint: string | undefined;
  iterations: number;
  tasks: string | undefined;
  timeout: number;
  maxTurns: number;
  claudeValidationRate: number;
  resume: string | undefined;
  help: boolean;
  // Learning flags
  skills: boolean;
  memory: boolean;
  reflect: boolean;
  maxRetries: number;
  // Post-run learning
  learn: boolean;
}

const parseCliArgs = (tbenchDefaults?: ProjectConfig["tbench"] | null): TBenchIterateArgs => {
  const { values } = parseArgs({
    args: process.argv.slice(2),
    options: {
      suite: { type: "string", short: "s" },
      output: { type: "string", short: "o" },
      model: { type: "string", short: "m" },
      "ollama-endpoint": { type: "string" },
      iterations: { type: "string", short: "i" },
      tasks: { type: "string", short: "t" },
      timeout: { type: "string" },
      "max-turns": { type: "string" },
      "claude-validation-rate": { type: "string" },
      resume: { type: "string", short: "r" },
      help: { type: "boolean", short: "h" },
      // Learning flags
      skills: { type: "boolean" },
      "no-skills": { type: "boolean" },
      memory: { type: "boolean" },
      reflect: { type: "boolean" },
      "max-retries": { type: "string" },
      // Post-run learning
      learn: { type: "boolean" },
    },
  });

  if (values.help) {
    console.log(`
TerminalBench Iterate CLI

Run multiple iterations of TerminalBench overnight with Claude Code or Ollama.

Usage:
  bun src/cli/tbench-iterate.ts --suite <file> [options]

Required:
  -s, --suite           Path to Terminal-Bench suite JSON file

Options:
  -o, --output          Output directory (default: ./results/YYYYMMDD)
  -m, --model           Model to use (default: claude-code)
                        Examples: claude-code, ollama:codellama:34b, ollama:deepseek-coder:33b
      --ollama-endpoint Ollama server endpoint (default: http://localhost:11434)
  -i, --iterations      Number of iterations to run (default: 10)
  -t, --tasks           Comma-separated task IDs to run (default: all)
      --timeout         Task timeout in seconds (default: 3600)
      --max-turns       Max agent turns per task (default: 300)
      --claude-validation-rate  Use Claude Code for N% of iterations (0.0-1.0)
  -r, --resume          Resume from state file
  -h, --help            Show this help message

Learning Options (Foundation Models only):
      --skills          Enable skill injection (default: true for FM)
      --no-skills       Disable skill injection
      --memory          Enable memory retrieval and injection
      --reflect         Enable reflexion on failures (FM-generated reflections)
      --max-retries     Max reflection-based retries per task (default: 2)
      --learn           Enable post-iteration learning (extract skills, generate reflections)

Examples:
  # Run 10 iterations with Claude Code
  bun src/cli/tbench-iterate.ts -s ./tasks/tb-2.0.json -i 10

  # Run 20 iterations with Ollama codellama
  bun src/cli/tbench-iterate.ts -s ./tasks/tb-2.0.json -m ollama:codellama:34b -i 20

  # Mixed: 90% Ollama, 10% Claude for validation
  bun src/cli/tbench-iterate.ts -s ./tasks/tb-2.0.json -m ollama:codellama:34b \\
    --claude-validation-rate 0.1 -i 20

  # FM with full learning stack (skills + memory + reflexion)
  bun src/cli/tbench-iterate.ts -s ./tasks/tb-2.0.json -m fm \\
    --skills --memory --reflect -i 10

  # Overnight learning sweep (extract skills after each iteration)
  bun src/cli/tbench-iterate.ts -s ./tasks/tb-2.0.json -m fm \\
    --skills --memory --reflect --learn -i 10

  # Resume interrupted run
  bun src/cli/tbench-iterate.ts --resume ./results/20251205/state.json
`);
    process.exit(0);
  }

  if (!values.suite && !values.resume) {
    console.error("Error: --suite or --resume is required");
    process.exit(1);
  }

  // Default output directory
  const today = new Date().toISOString().slice(0, 10).replace(/-/g, "");
  const defaultOutput = `./results/${today}`;

  // Apply project tbench defaults
  const projectModel = tbenchDefaults?.defaultModel ?? "claude-code";
  const projectSuite = tbenchDefaults?.defaultSuite;
  const projectTimeout = tbenchDefaults?.defaultTimeout ?? 3600;
  const projectMaxTurns = tbenchDefaults?.defaultMaxTurns ?? 300;
  const projectLearning = tbenchDefaults?.defaultLearning;

  // Use CLI value > project default > hardcoded default
  const model = values.model ?? projectModel;

  // Skills: default true for FM, false otherwise. --no-skills overrides
  // Project config takes precedence over FM default
  const isFM = model.startsWith("fm");
  const skillsDefault = projectLearning?.skills ?? isFM;
  const skillsEnabled = values["no-skills"] ? false : (values.skills ?? skillsDefault);

  return {
    suite: values.suite ?? projectSuite ?? "",
    output: values.output ?? defaultOutput,
    model,
    ollamaEndpoint: values["ollama-endpoint"],
    iterations: values.iterations ? parseInt(values.iterations, 10) : 10,
    tasks: values.tasks,
    timeout: values.timeout ? parseInt(values.timeout, 10) : projectTimeout,
    maxTurns: values["max-turns"] ? parseInt(values["max-turns"], 10) : projectMaxTurns,
    claudeValidationRate: values["claude-validation-rate"]
      ? parseFloat(values["claude-validation-rate"])
      : 0,
    resume: values.resume,
    help: values.help ?? false,
    // Learning flags - CLI > project config > defaults
    skills: skillsEnabled,
    memory: values.memory ?? projectLearning?.memory ?? false,
    reflect: values.reflect ?? projectLearning?.reflexion ?? false,
    maxRetries: values["max-retries"] ? parseInt(values["max-retries"], 10) : 2,
    // Post-run learning
    learn: values.learn ?? projectLearning?.learn ?? false,
  };
};

// ============================================================================
// State Management (for resume capability)
// ============================================================================

interface RunState {
  runId: string;
  suite: string;
  model: string;
  ollamaEndpoint?: string;
  iterations: number;
  completedIterations: number;
  tasks?: string;
  timeout: number;
  maxTurns: number;
  claudeValidationRate: number;
  startedAt: string;
  lastUpdatedAt: string;
  // Learning flags
  skills?: boolean;
  memory?: boolean;
  reflect?: boolean;
  maxRetries?: number;
  // Post-run learning
  learn?: boolean;
}

const saveState = (outputDir: string, state: RunState): void => {
  const statePath = join(outputDir, "state.json");
  writeFileSync(statePath, JSON.stringify(state, null, 2));
};

const loadState = (statePath: string): RunState | null => {
  if (!existsSync(statePath)) return null;
  try {
    return JSON.parse(readFileSync(statePath, "utf-8"));
  } catch {
    return null;
  }
};

// ============================================================================
// Task Workspace Setup (from tbench-local.ts)
// ============================================================================

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

    // Copy test files, modifying /app/ paths
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
          content = content.replace(/\/app\//g, `${workspaceDir}/`);
          content = content.replace(/\/app(?=["'])/g, workspaceDir);
          writeFileSync(destFile, content);
        }
      }
    }
  }
};

// ============================================================================
// Verification (from tbench-local.ts)
// ============================================================================

const runLocalVerification = async (
  workspaceDir: string,
  tbTask: TerminalBenchTask
): Promise<{ passed: boolean; output: string }> => {
  const testsDir = join(workspaceDir, "tests");

  if (!existsSync(testsDir)) {
    // Handle "output" verification type
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
      return { passed, output: `Expected: ${expected}\nActual: ${stdout}` };
    }
    // Handle "custom" verification type with script
    if (tbTask.verification.type === "custom") {
      const cmd = tbTask.verification.script ?? tbTask.verification.command ?? "exit 1";
      const proc = Bun.spawn(["sh", "-c", cmd], {
        cwd: workspaceDir,
        stdout: "pipe",
        stderr: "pipe",
      });
      const exitCode = await proc.exited;
      const stdout = await new Response(proc.stdout).text();
      const stderr = await new Response(proc.stderr).text();
      const passed = exitCode === 0;
      const output = stdout + (stderr ? `\nSTDERR:\n${stderr}` : "");
      return { passed, output: output || (passed ? "Verification passed" : "Verification failed") };
    }
    return { passed: false, output: "No tests directory found and no custom verification" };
  }

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

// ============================================================================
// Single Task Runner
// ============================================================================

interface TaskResult {
  taskId: string;
  outcome: "success" | "failure" | "timeout" | "error";
  durationMs: number;
  turns: number;
  tokens: number;
  verificationOutput: string | undefined;
  errorMessage: string | undefined;
  /** Learning metrics from FM model (if applicable) */
  learningMetrics?: {
    skillsUsed: string[];
    memoriesUsed: number;
    reflectionsGenerated: number;
  };
}

/**
 * Record a task trajectory to the Archivist for future learning.
 * This creates a simplified trajectory record for TB runs.
 */
const recordTrajectoryToArchivist = async (
  task: TerminalBenchTask,
  result: TaskResult,
  model: string,
  projectRoot: string,
): Promise<void> => {
  try {
    // Map TB outcome to Archivist outcome
    const outcomeMap: Record<TaskResult["outcome"], Trajectory["outcome"]> = {
      success: "success",
      failure: "failure",
      timeout: "timeout",
      error: "failure", // Map error to failure for Archivist
    };

    const program = Effect.gen(function* () {
      const archivist = yield* ArchivistService;
      yield* archivist.recordTrajectory(task.id, task.name, {
        actions: [
          {
            type: "tool_call",
            tool: "terminal-bench-task",
            content: `Run task: ${task.name} (${task.difficulty})`,
            result: `outcome: ${result.outcome}${result.errorMessage ? ` - ${result.errorMessage}` : ""}`,
            success: result.outcome === "success",
            durationMs: result.durationMs,
            timestamp: new Date().toISOString(),
          },
        ],
        outcome: outcomeMap[result.outcome],
        ...(result.errorMessage ? { errorMessage: result.errorMessage } : {}),
        skillsUsed: result.learningMetrics?.skillsUsed ?? [],
        filesModified: [],
        totalDurationMs: result.durationMs,
        model,
        tokens: {
          input: result.tokens,
          output: 0,
          total: result.tokens,
        },
      });
    });

    await Effect.runPromise(
      program.pipe(
        Effect.provide(makeArchivistServiceLive(projectRoot)),
        Effect.catchAll(() => Effect.void), // Best-effort, don't fail on archivist errors
      ),
    );
  } catch {
    // Silently ignore archivist errors
  }
};

const runSingleTask = async (
  tbTask: TerminalBenchTask,
  runner: ModelRunner,
  options: {
    outputDir: string;
    timeout: number;
    maxTurns: number;
    sourceRepo?: string;
    runId?: string;
    tbEmitter?: TBEmitter;
    taskIndex?: number;
    totalTasks?: number;
  }
): Promise<TaskResult> => {
  const startTime = Date.now();
  const taskOutputDir = join(options.outputDir, tbTask.id);
  mkdirSync(taskOutputDir, { recursive: true });

  const workspaceDir = join(taskOutputDir, "workspace");

  console.log(`\n  Task: ${tbTask.id} (${tbTask.difficulty})`);

  // Emit task start to HUD
  if (options.tbEmitter && options.taskIndex !== undefined && options.totalTasks !== undefined) {
    options.tbEmitter.taskStart(
      { id: tbTask.id, name: tbTask.name, category: tbTask.category, difficulty: tbTask.difficulty },
      options.taskIndex,
      options.totalTasks
    );
    options.tbEmitter.taskProgress(tbTask.id, "setup", undefined, 0);
  }

  // Setup workspace
  setupTaskWorkspace(tbTask, workspaceDir, options.sourceRepo);

  // Run setup commands
  if (tbTask.setup?.length) {
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

  // Emit agent phase start
  if (options.tbEmitter) {
    options.tbEmitter.taskProgress(tbTask.id, "agent", 0, Date.now() - startTime);
  }

  // ATIF disk persistence - will be initialized when sessionId is available
  // Use object wrapper to allow TypeScript to track mutations from async callback
  const atifState: { writer: StreamingWriter | null; sessionId: string | null } = {
    writer: null,
    sessionId: null,
  };

  // Run task with model
  let result: TaskRunResult;
  try {
    result = await runner.runTask(tbTask, {
      workspace: workspaceDir,
      timeout: options.timeout,
      maxTurns: options.maxTurns,
      runId: options.runId,
      onSessionId: async (sessionId) => {
        // Create and register ATIF disk writer when sessionId is available
        // IMPORTANT: Register BEFORE initialize() to avoid race condition
        // The StreamingWriter.writeStep() will wait for init to complete
        atifState.sessionId = sessionId;
        atifState.writer = new StreamingWriter({
          sessionId,
          agent: {
            name: "tbench-claude-code",
            version: "1.0.0",
            model_name: "claude-code",
          },
          baseDir: join(taskOutputDir, "atif"),
          agentType: "claude-code",
          emitHudEvents: false, // We're already emitting to HUD via sdk-adapter
        });
        // Register immediately so steps can be queued during initialization
        registerATIFDiskWriter(sessionId, atifState.writer);
        // Initialize asynchronously - writeStep will wait for this
        await atifState.writer.initialize();
        console.log(`  [ATIF] Disk persistence enabled for session: ${sessionId}`);
      },
      onOutput: (text) => {
        if (options.tbEmitter) {
          options.tbEmitter.taskOutput(tbTask.id, text, "agent");
        }
      },
    });

    // Save raw output
    writeFileSync(join(taskOutputDir, "output.txt"), result.output);

    if (!result.success && result.turns === 0) {
      const errorMsg = result.error || "Agent did not process any turns";
      writeFileSync(join(taskOutputDir, "error.txt"), errorMsg);
      // Cleanup ATIF writer on error
      if (atifState.writer && atifState.sessionId) {
        try {
          await atifState.writer.close({ total_prompt_tokens: 0, total_completion_tokens: 0, total_steps: 0 }, "failed");
          unregisterATIFDiskWriter(atifState.sessionId);
        } catch { /* ignore cleanup errors */ }
      }
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
    writeFileSync(join(taskOutputDir, "error.txt"), errorMsg);
    // Cleanup ATIF writer on error
    if (atifState.writer && atifState.sessionId) {
      try {
        await atifState.writer.close({ total_prompt_tokens: 0, total_completion_tokens: 0, total_steps: 0 }, "failed");
        unregisterATIFDiskWriter(atifState.sessionId);
      } catch { /* ignore cleanup errors */ }
    }
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

  // Run verification
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

  const statusIcon = outcome === "success" ? "✓" : outcome === "failure" ? "✗" : "⚠";
  console.log(`    ${statusIcon} ${outcome} (${(durationMs / 1000).toFixed(1)}s, ${result.turns} turns)`);

  // Emit task complete to HUD
  if (options.tbEmitter) {
    options.tbEmitter.taskComplete(tbTask.id, {
      outcome,
      durationMs,
      turns: result.turns,
      tokens: result.tokens,
      verificationOutput: verificationResult.output,
    });
  }

  // Close ATIF disk writer if it was initialized
  if (atifState.writer && atifState.sessionId) {
    try {
      const finalMetrics = {
        total_prompt_tokens: result.tokens, // Approximation - we don't have split tokens
        total_completion_tokens: 0,
        total_steps: result.turns,
      };
      const atifPaths = await atifState.writer.close(finalMetrics, outcome === "success" ? "complete" : "failed");
      unregisterATIFDiskWriter(atifState.sessionId);
      console.log(`  [ATIF] Saved trajectory to: ${atifPaths.jsonl}`);
    } catch (e) {
      console.error(`  [ATIF] Failed to close writer:`, e);
    }
  }

  // Extract learning metrics from FM model result (if available)
  const learningMetrics = result.sessionMetadata?.skillsUsed
    ? {
        skillsUsed: result.sessionMetadata.skillsUsed,
        memoriesUsed: 0, // FM runner doesn't track this separately yet
        reflectionsGenerated: 0, // FM runner doesn't track this separately yet
      }
    : undefined;

  // Emit learning metrics to HUD if available
  if (options.tbEmitter && learningMetrics && learningMetrics.skillsUsed.length > 0) {
    options.tbEmitter.learningMetrics(tbTask.id, {
      model: runner.modelName,
      skillsUsed: learningMetrics.skillsUsed.length,
      skillIds: learningMetrics.skillsUsed,
      memoriesUsed: learningMetrics.memoriesUsed,
      reflexionEnabled: runner.config.type === "foundation-models"
        ? Boolean((runner.config as import("../bench/model-adapter.js").FMModelConfig).useReflection)
        : false,
      reflectionsGenerated: learningMetrics.reflectionsGenerated,
      newSkillsLearned: 0, // Tracked at iteration level
    });
  }

  return {
    taskId: tbTask.id,
    outcome,
    durationMs,
    turns: result.turns,
    tokens: result.tokens,
    verificationOutput: verificationResult.output,
    errorMessage: result.error,
    learningMetrics,
  };
};

// ============================================================================
// Report Generation
// ============================================================================

const generateIterationReport = (
  iteration: number,
  results: TerminalBenchResults,
  baseline: Episode | null,
  model: string
): string => {
  let report = `# Iteration ${iteration} Report

## Summary

| Metric | Value |
|--------|-------|
| Model | ${model} |
| Pass Rate | ${(results.summary.pass_rate * 100).toFixed(1)}% |
| Passed | ${results.summary.passed}/${results.summary.total} |
| Failed | ${results.summary.failed} |
| Timeout | ${results.summary.timeout} |
| Error | ${results.summary.error} |
| Avg Duration | ${(results.summary.avg_duration_ms / 1000).toFixed(1)}s |
| Avg Turns | ${results.summary.avg_turns.toFixed(1)} |
| Total Tokens | ${results.summary.total_tokens.toLocaleString()} |

`;

  if (baseline) {
    const delta = results.summary.pass_rate - baseline.summary.passRate;
    const sign = delta >= 0 ? "+" : "";
    report += `## Comparison with Baseline

| Metric | Delta |
|--------|-------|
| Pass Rate | ${sign}${(delta * 100).toFixed(1)}% |
| Baseline | ${(baseline.summary.passRate * 100).toFixed(1)}% (${baseline.id}) |

`;
  }

  report += `## Results by Task

| Task | Status | Duration | Turns | Tokens |
|------|--------|----------|-------|--------|
`;

  for (const r of results.results) {
    const status = r.status === "pass" ? "✓" : r.status === "fail" ? "✗" : r.status;
    report += `| ${r.task_id} | ${status} | ${(r.duration_ms / 1000).toFixed(1)}s | ${r.turns} | ${r.tokens_used.toLocaleString()} |\n`;
  }

  return report;
};

const generateSummaryReport = (
  episodes: Episode[],
  suite: TerminalBenchSuite
): string => {
  if (episodes.length === 0) return "# Summary\n\nNo iterations completed.\n";

  const avgPassRate = episodes.reduce((sum, e) => sum + e.summary.passRate, 0) / episodes.length;
  const bestEpisode = episodes.reduce((best, e) =>
    e.summary.passRate > best.summary.passRate ? e : best
  );
  const worstEpisode = episodes.reduce((worst, e) =>
    e.summary.passRate < worst.summary.passRate ? e : worst
  );

  const totalDuration = episodes.reduce((sum, e) => sum + e.summary.totalDurationMs, 0);
  const totalTokens = episodes.reduce((sum, e) => sum + e.summary.avgTokens * e.summary.total, 0);

  let report = `# Overnight Run Summary

## Overview

| Metric | Value |
|--------|-------|
| Suite | ${suite.name} v${suite.version} |
| Iterations | ${episodes.length} |
| Total Duration | ${(totalDuration / 1000 / 60).toFixed(1)} minutes |
| Total Tokens | ${totalTokens.toLocaleString()} |

## Pass Rate

| Metric | Value |
|--------|-------|
| Average | ${(avgPassRate * 100).toFixed(1)}% |
| Best | ${(bestEpisode.summary.passRate * 100).toFixed(1)}% (iter ${bestEpisode.iteration}) |
| Worst | ${(worstEpisode.summary.passRate * 100).toFixed(1)}% (iter ${worstEpisode.iteration}) |

## Iterations

| Iteration | Model | Pass Rate | Passed | Duration |
|-----------|-------|-----------|--------|----------|
`;

  for (const ep of episodes) {
    report += `| ${ep.iteration} | ${ep.model} | ${(ep.summary.passRate * 100).toFixed(1)}% | ${ep.summary.passed}/${ep.summary.total} | ${(ep.summary.totalDurationMs / 1000 / 60).toFixed(1)}m |\n`;
  }

  // Pass rate trend
  report += `\n## Pass Rate Trend\n\n`;
  for (const ep of episodes) {
    const bar = "█".repeat(Math.round(ep.summary.passRate * 20));
    const empty = "░".repeat(20 - Math.round(ep.summary.passRate * 20));
    report += `${String(ep.iteration).padStart(3)}: ${bar}${empty} ${(ep.summary.passRate * 100).toFixed(0)}%\n`;
  }

  return report;
};

// ============================================================================
// Project Config Loading
// ============================================================================

const loadTBenchDefaults = async (): Promise<ProjectConfig["tbench"] | null> => {
  try {
    const config = await Effect.runPromise(
      loadProjectConfig(process.cwd()).pipe(
        Effect.provide(BunContext.layer),
        Effect.catchAll(() => Effect.succeed(null))
      )
    );
    return config?.tbench ?? null;
  } catch {
    return null;
  }
};

// ============================================================================
// Main Execution
// ============================================================================

const main = async (): Promise<void> => {
  // Load project tbench defaults before parsing args
  const tbenchDefaults = await loadTBenchDefaults();
  const args = parseCliArgs(tbenchDefaults);

  // Handle resume
  let state: RunState | null = null;
  if (args.resume) {
    state = loadState(args.resume);
    if (!state) {
      console.error(`Failed to load state from ${args.resume}`);
      process.exit(1);
    }
    console.log(`Resuming run ${state.runId} from iteration ${state.completedIterations + 1}`);
    args.suite = state.suite;
    args.model = state.model;
    args.ollamaEndpoint = state.ollamaEndpoint;
    args.iterations = state.iterations;
    args.tasks = state.tasks;
    args.timeout = state.timeout;
    args.maxTurns = state.maxTurns;
    args.claudeValidationRate = state.claudeValidationRate;
    // Restore learning flags
    args.skills = state.skills ?? args.skills;
    args.memory = state.memory ?? args.memory;
    args.reflect = state.reflect ?? args.reflect;
    args.maxRetries = state.maxRetries ?? args.maxRetries;
    args.learn = state.learn ?? args.learn;
  }

  // Load suite
  console.log(`Loading suite from ${args.suite}...`);
  let suite: TerminalBenchSuite;
  try {
    suite = await Effect.runPromise(
      loadTerminalBenchSuite(args.suite).pipe(Effect.provide(BunContext.layer))
    );
    console.log(`Loaded: ${suite.name} v${suite.version} (${suite.tasks.length} tasks)`);
  } catch (e) {
    console.error(`Failed to load suite: ${e}`);
    process.exit(1);
  }

  // Filter tasks
  let tasksToRun = suite.tasks;
  if (args.tasks) {
    const taskIds = new Set(args.tasks.split(",").map(t => t.trim()));
    tasksToRun = suite.tasks.filter(t => taskIds.has(t.id));
    console.log(`Running ${tasksToRun.length} selected tasks`);
  }

  // Parse model config
  const modelConfig = parseModelString(args.model);
  if (modelConfig.type === "ollama" && args.ollamaEndpoint) {
    modelConfig.endpoint = args.ollamaEndpoint;
  }
  // Pass learning flags to FM config
  if (modelConfig.type === "foundation-models") {
    modelConfig.useSkills = args.skills;
    modelConfig.useMemory = args.memory;
    modelConfig.useReflection = args.reflect;
    modelConfig.maxReflectionRetries = args.maxRetries;
    modelConfig.projectRoot = process.cwd();
  }

  // Create model runners
  const primaryRunner = createModelRunner(modelConfig);
  const claudeRunner = args.claudeValidationRate > 0
    ? createModelRunner({ type: "claude-code" })
    : null;

  // Check health
  console.log(`\nChecking model availability...`);
  const health = await primaryRunner.checkHealth();
  if (!health.available) {
    console.error(`Model ${primaryRunner.modelName} not available: ${health.error}`);
    process.exit(1);
  }
  console.log(`  ${primaryRunner.modelName}: available`);

  if (claudeRunner) {
    const claudeHealth = await claudeRunner.checkHealth();
    if (!claudeHealth.available) {
      console.warn(`Claude Code not available, disabling validation runs`);
    } else {
      console.log(`  claude-code: available (validation rate: ${args.claudeValidationRate * 100}%)`);
    }
  }

  // Setup output directory
  const outputDir = resolve(args.output);
  mkdirSync(outputDir, { recursive: true });

  // Initialize state
  const runId = state?.runId ?? generateRunId();
  const startIteration = state?.completedIterations ? state.completedIterations + 1 : 1;

  state = {
    runId,
    suite: args.suite,
    model: args.model,
    ollamaEndpoint: args.ollamaEndpoint,
    iterations: args.iterations,
    completedIterations: startIteration - 1,
    tasks: args.tasks,
    timeout: args.timeout,
    maxTurns: args.maxTurns,
    claudeValidationRate: args.claudeValidationRate,
    startedAt: state?.startedAt ?? new Date().toISOString(),
    lastUpdatedAt: new Date().toISOString(),
    // Learning flags
    skills: args.skills,
    memory: args.memory,
    reflect: args.reflect,
    maxRetries: args.maxRetries,
    // Post-run learning
    learn: args.learn,
  };
  saveState(outputDir, state);

  // Initialize episode store
  const projectRoot = process.cwd();
  const episodeStore = new EpisodeStore(join(projectRoot, ".openagents", "gym"));

  // Get baseline
  const baseline = await episodeStore.getBaseline(primaryRunner.modelName);
  if (baseline) {
    console.log(`Baseline: ${baseline.id} (${(baseline.summary.passRate * 100).toFixed(1)}% pass rate)`);
  }

  // Create HUD emitter
  const tbEmitter = createTBEmitter();

  console.log(`\n${"=".repeat(60)}`);
  console.log(`Starting overnight run: ${runId}`);
  console.log(`Suite: ${suite.name} v${suite.version}`);
  console.log(`Tasks: ${tasksToRun.length}`);
  console.log(`Iterations: ${args.iterations}`);
  console.log(`Model: ${primaryRunner.modelName}`);
  console.log(`Output: ${outputDir}`);
  console.log(`${"=".repeat(60)}\n`);

  const runStartTime = Date.now();
  const episodes: Episode[] = [];

  // Emit run start to HUD
  const suiteInfo = {
    name: suite.name,
    version: suite.version,
    tasks: suite.tasks.map(t => ({
      id: t.id,
      name: t.name,
      category: t.category,
      difficulty: t.difficulty,
    })),
  };
  tbEmitter.runStart(suiteInfo, tasksToRun.map(t => t.id));

  // Run iterations
  for (let iter = startIteration; iter <= args.iterations; iter++) {
    const iterStartTime = Date.now();

    // Select model for this iteration
    const useClaudeValidation = claudeRunner &&
      args.claudeValidationRate > 0 &&
      Math.random() < args.claudeValidationRate;
    const runner = useClaudeValidation ? claudeRunner : primaryRunner;
    const modelName = runner.modelName;

    console.log(`\n${"─".repeat(60)}`);
    console.log(`Iteration ${iter}/${args.iterations} [${modelName}]`);
    console.log(`${"─".repeat(60)}`);

    const iterOutputDir = join(outputDir, "iterations", String(iter).padStart(3, "0"));
    mkdirSync(iterOutputDir, { recursive: true });

    // Run all tasks
    const results: TaskResult[] = [];
    for (let i = 0; i < tasksToRun.length; i++) {
      const task = tasksToRun[i];
      const result = await runSingleTask(task, runner, {
        outputDir: iterOutputDir,
        timeout: args.timeout,
        maxTurns: args.maxTurns,
        sourceRepo: suite.source_repo,
        runId,
        tbEmitter,
        taskIndex: i,
        totalTasks: tasksToRun.length,
      });
      results.push(result);

      // Record trajectory to Archivist for learning (best-effort)
      if (args.learn) {
        await recordTrajectoryToArchivist(task, result, modelName, projectRoot);
      }

      // Save intermediate results
      const intermediateResults = toBenchmarkResults(suite, modelName, results);
      writeFileSync(
        join(iterOutputDir, "results.json"),
        JSON.stringify(intermediateResults, null, 2)
      );
    }

    const iterEndTime = Date.now();

    // Generate final results for this iteration
    const iterResults = toBenchmarkResults(suite, modelName, results);
    writeFileSync(join(iterOutputDir, "results.json"), JSON.stringify(iterResults, null, 2));

    // Generate iteration report
    const iterReport = generateIterationReport(iter, iterResults, baseline, modelName);
    writeFileSync(join(iterOutputDir, "report.md"), iterReport);

    // Create and store episode
    const episode = createEpisode({
      runId,
      iteration: iter,
      model: modelName,
      suiteVersion: suite.version,
      startedAt: new Date(iterStartTime),
      finishedAt: new Date(iterEndTime),
      results: {
        total: iterResults.summary.total,
        passed: iterResults.summary.passed,
        failed: iterResults.summary.failed,
        timeout: iterResults.summary.timeout,
        error: iterResults.summary.error,
        avgTurns: iterResults.summary.avg_turns,
        avgTokens: iterResults.summary.total_tokens / Math.max(1, iterResults.summary.total),
        totalDurationMs: iterEndTime - iterStartTime,
      },
      resultsPath: join(iterOutputDir, "results.json"),
      baselineEpisode: baseline ?? undefined,
    });

    await episodeStore.record(episode);
    episodes.push(episode);

    // Post-iteration learning (extract skills, generate reflections)
    let learningResult: LearningResult | null = null;
    if (args.learn) {
      console.log(`  [Learning] Processing episode for skill extraction...`);
      try {
        const learner = createEpisodeLearner({ projectRoot });
        learningResult = await Effect.runPromise(
          learner.processEpisode(episode).pipe(
            Effect.catchAll((e) => {
              console.log(`    [Learning] Warning: ${e.message}`);
              return Effect.succeed({
                episodeId: episode.id,
                skillsExtracted: [] as Skill[],
                reflectionsGenerated: [],
                patternsIdentified: [],
                durationMs: 0,
                processedAt: new Date().toISOString(),
              } as LearningResult);
            })
          )
        );

        if (learningResult.skillsExtracted.length > 0) {
          console.log(`    [Learning] Extracted ${learningResult.skillsExtracted.length} skills`);
          // Register skills with SkillService
          try {
            const skillProgram = Effect.gen(function* () {
              const service = yield* SkillService;
              for (const skill of learningResult.skillsExtracted) {
                yield* service.registerSkill(skill);
              }
            });
            await Effect.runPromise(
              skillProgram.pipe(
                Effect.provide(makeSkillServiceLive(projectRoot)),
                Effect.catchAll(() => Effect.void)
              )
            );
          } catch {
            // Skill registration is best-effort
          }
        }
        if (learningResult.reflectionsGenerated.length > 0) {
          console.log(`    [Learning] Generated ${learningResult.reflectionsGenerated.length} reflections`);
        }
      } catch (e) {
        console.log(`    [Learning] Error: ${e instanceof Error ? e.message : String(e)}`);
      }

      // Run Archivist quick archive to extract patterns from trajectories
      console.log(`  [Archivist] Processing recorded trajectories...`);
      try {
        const archiveResult = await Effect.runPromise(
          Effect.gen(function* () {
            const archivist = yield* ArchivistService;
            return yield* archivist.runQuickArchive();
          }).pipe(
            Effect.provide(makeArchivistServiceLive(projectRoot)),
            Effect.catchAll((e) => {
              console.log(`    [Archivist] Warning: ${e.message}`);
              return Effect.succeed({
                id: "",
                trajectoriesProcessed: 0,
                patternsExtracted: 0,
                skillsCreated: 0,
                memoriesCreated: 0,
                itemsPruned: 0,
                durationMs: 0,
                timestamp: new Date().toISOString(),
              });
            }),
          ),
        );

        if (archiveResult.trajectoriesProcessed > 0) {
          console.log(`    [Archivist] Processed ${archiveResult.trajectoriesProcessed} trajectories`);
          console.log(`    [Archivist] Extracted ${archiveResult.patternsExtracted} patterns, created ${archiveResult.skillsCreated} skills`);
        }
      } catch (e) {
        console.log(`    [Archivist] Error: ${e instanceof Error ? e.message : String(e)}`);
      }
    }

    // Emit learning summary to HUD
    const totalSkillsUsed = results.reduce((sum, r) => sum + (r.learningMetrics?.skillsUsed.length ?? 0), 0);
    const totalMemoriesUsed = results.reduce((sum, r) => sum + (r.learningMetrics?.memoriesUsed ?? 0), 0);
    const totalReflectionsGenerated = results.reduce((sum, r) => sum + (r.learningMetrics?.reflectionsGenerated ?? 0), 0);
    const newSkillsLearned = args.learn ? (learningResult?.skillsExtracted?.length ?? 0) : 0;

    if (modelConfig.type === "foundation-models") {
      tbEmitter.learningSummary({
        totalTasks: results.length,
        passed: iterResults.summary.passed,
        passRate: iterResults.summary.pass_rate,
        model: modelName,
        learningFlags: {
          skills: args.skills,
          memory: args.memory,
          reflexion: args.reflect,
          learn: args.learn,
        },
        totalSkillsUsed,
        totalMemoriesUsed,
        totalReflectionsGenerated,
        newSkillsLearned,
      });
    }

    // Update state
    state.completedIterations = iter;
    state.lastUpdatedAt = new Date().toISOString();
    saveState(outputDir, state);

    // Print iteration summary
    console.log(`\n  Summary: ${iterResults.summary.passed}/${iterResults.summary.total} passed (${(iterResults.summary.pass_rate * 100).toFixed(1)}%)`);
    console.log(`  Duration: ${((iterEndTime - iterStartTime) / 1000 / 60).toFixed(1)} minutes`);
    if (totalSkillsUsed > 0) {
      console.log(`  Learning: ${totalSkillsUsed} skills used, ${newSkillsLearned} new skills learned`);
    }
  }

  const totalDuration = Date.now() - runStartTime;

  // Generate final summary
  const summaryReport = generateSummaryReport(episodes, suite);
  writeFileSync(join(outputDir, "summary.md"), summaryReport);
  writeFileSync(join(outputDir, "episodes.json"), JSON.stringify(episodes, null, 2));

  // Emit run complete to HUD
  const avgPassRate = episodes.length > 0
    ? episodes.reduce((sum, e) => sum + e.summary.passRate, 0) / episodes.length
    : 0;
  tbEmitter.runComplete({
    passRate: avgPassRate,
    passed: episodes.reduce((sum, e) => sum + e.summary.passed, 0),
    failed: episodes.reduce((sum, e) => sum + e.summary.failed, 0),
    timeout: episodes.reduce((sum, e) => sum + e.summary.timeout, 0),
    error: episodes.reduce((sum, e) => sum + e.summary.error, 0),
    totalDurationMs: totalDuration,
  });
  tbEmitter.close();

  // Print final summary
  console.log(`\n${"=".repeat(60)}`);
  console.log(`Overnight Run Complete: ${runId}`);
  console.log(`${"=".repeat(60)}`);
  console.log(`\nIterations: ${episodes.length}`);
  console.log(`Total Duration: ${(totalDuration / 1000 / 60).toFixed(1)} minutes`);
  console.log(`Average Pass Rate: ${(avgPassRate * 100).toFixed(1)}%`);
  console.log(`Output: ${outputDir}`);
  console.log(`\nReports:`);
  console.log(`  - ${join(outputDir, "summary.md")}`);
  console.log(`  - ${join(outputDir, "episodes.json")}`);

  // Get episode stats
  const stats = await episodeStore.getStats();
  console.log(`\nEpisode Store:`);
  console.log(`  - Total episodes: ${stats.totalEpisodes}`);
  console.log(`  - Total runs: ${stats.totalRuns}`);

  console.log(`\n${"=".repeat(60)}\n`);
};

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(2);
});
