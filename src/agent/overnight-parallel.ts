#!/usr/bin/env bun
/**
 * Parallel Overnight Agent - Run multiple MechaCoder agents in parallel
 *
 * Usage: bun src/agent/overnight-parallel.ts [options]
 *
 * Options:
 *   --cwd, --dir    Target repo directory (default: current directory)
 *   --max-agents    Maximum parallel agents (default: 2)
 *   --max-tasks     Maximum total tasks to complete (default: 10)
 *   --dry-run       Print what would happen without executing
 *   --cc-only       Use Claude Code only (no Grok fallback)
 *
 * Each agent runs in an isolated git worktree, enabling true parallel execution
 * without conflicts. Changes are merged back to main sequentially after completion.
 *
 * @see docs/claude/plans/containers-impl-v2.md
 */
import { Effect, Layer } from "effect";
import * as BunContext from "@effect/platform-bun/BunContext";
import * as path from "node:path";
import * as fs from "node:fs";
import {
  createWorktree,
  removeWorktree,
  pruneStaleWorktrees,
  type WorktreeConfig,
  type WorktreeInfo,
} from "./orchestrator/worktree.js";
import {
  acquireWorktreeLock,
  releaseWorktreeLock,
  pruneWorktreeLocks,
} from "./orchestrator/agent-lock.js";
import { runOrchestrator } from "./orchestrator/orchestrator.js";
import { loadProjectConfig } from "../tasks/project.js";
import { readTasks } from "../tasks/service.js";
import type { Task } from "../tasks/index.js";
import { openRouterClientLayer, openRouterConfigLayer } from "../llm/openrouter.js";

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

interface ParallelConfig {
  workDir: string;
  maxAgents: number;
  maxTasks: number;
  dryRun: boolean;
  ccOnly: boolean;
  verbose: boolean;
}

// Agent colors for terminal output
const AGENT_COLORS = [
  "\x1b[36m", // cyan
  "\x1b[33m", // yellow
  "\x1b[35m", // magenta
  "\x1b[32m", // green
  "\x1b[34m", // blue
  "\x1b[91m", // bright red
  "\x1b[92m", // bright green
  "\x1b[93m", // bright yellow
];
const RESET = "\x1b[0m";
const DIM = "\x1b[2m";

const getAgentColor = (agentId: number): string => {
  return AGENT_COLORS[(agentId - 1) % AGENT_COLORS.length];
};

interface AgentSlot {
  id: number;
  taskId: string | null;
  task: Task | null;
  worktree: WorktreeInfo | null;
  status: "idle" | "running" | "completed" | "failed";
  error?: string;
  commitSha?: string;
}

// ─────────────────────────────────────────────────────────────────────────────
// Logging
// ─────────────────────────────────────────────────────────────────────────────

let _logWorkDir = process.cwd();
let logFilePath: string | null = null;

const getLogDir = () => {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return path.join(_logWorkDir, "docs", "logs", `${year}${month}${day}`);
};

const getLogPath = (sessionId: string) => {
  const now = new Date();
  const hours = String(now.getHours()).padStart(2, "0");
  const mins = String(now.getMinutes()).padStart(2, "0");
  return path.join(getLogDir(), `${hours}${mins}-parallel-${sessionId}.md`);
};

const initLog = (sessionId: string) => {
  const logDir = getLogDir();
  if (!fs.existsSync(logDir)) {
    fs.mkdirSync(logDir, { recursive: true });
  }
  logFilePath = getLogPath(sessionId);
  fs.writeFileSync(
    logFilePath,
    `# Parallel Agent Log\n\nSession: ${sessionId}\nStarted: ${new Date().toISOString()}\n\n`,
  );
};

const log = (msg: string) => {
  const ts = new Date().toISOString();
  const line = `[${ts}] ${msg}`;
  console.log(line);
  if (logFilePath) {
    fs.appendFileSync(logFilePath, line + "\n");
  }
};

// ─────────────────────────────────────────────────────────────────────────────
// CLI Parsing
// ─────────────────────────────────────────────────────────────────────────────

const parseArgs = (): ParallelConfig => {
  const args = process.argv.slice(2);
  let workDir = process.cwd();
  let maxAgents = 2;
  let maxTasks = 10;
  let dryRun = false;
  let ccOnly = false;
  let verbose = false;

  for (let i = 0; i < args.length; i++) {
    if ((args[i] === "--dir" || args[i] === "--cwd") && args[i + 1]) {
      workDir = args[i + 1].startsWith("~")
        ? args[i + 1].replace("~", process.env.HOME || "")
        : path.resolve(args[i + 1]);
      i++;
    } else if (args[i] === "--max-agents" && args[i + 1]) {
      maxAgents = parseInt(args[i + 1], 10);
      i++;
    } else if (args[i] === "--max-tasks" && args[i + 1]) {
      maxTasks = parseInt(args[i + 1], 10);
      i++;
    } else if (args[i] === "--dry-run") {
      dryRun = true;
    } else if (args[i] === "--cc-only") {
      ccOnly = true;
    } else if (args[i] === "--verbose" || args[i] === "-v") {
      verbose = true;
    }
  }

  return { workDir, maxAgents, maxTasks, dryRun, ccOnly, verbose };
};

// ─────────────────────────────────────────────────────────────────────────────
// Git Operations
// ─────────────────────────────────────────────────────────────────────────────

const runGit = async (
  cwd: string,
  args: string[],
): Promise<{ exitCode: number; stdout: string; stderr: string }> => {
  const proc = Bun.spawn(["git", ...args], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  const exitCode = await proc.exited;
  return { exitCode, stdout: stdout.trim(), stderr: stderr.trim() };
};

const hasCommitsAhead = async (
  worktreePath: string,
  baseBranch: string,
): Promise<boolean> => {
  const result = await runGit(worktreePath, [
    "rev-list",
    "--count",
    `origin/${baseBranch}..HEAD`,
  ]);
  return parseInt(result.stdout, 10) > 0;
};

const mergeToMain = async (
  repoPath: string,
  branch: string,
): Promise<{ success: boolean; commitSha?: string; error?: string }> => {
  try {
    // Check for uncommitted changes and stash them
    let result = await runGit(repoPath, ["status", "--porcelain"]);
    const hasChanges = result.stdout.trim().length > 0;
    if (hasChanges) {
      // Stash any uncommitted changes (likely log files from parallel runs)
      result = await runGit(repoPath, ["stash", "push", "-m", "parallel-merge-stash"]);
      if (result.exitCode !== 0) {
        return { success: false, error: `Stash failed: ${result.stderr}` };
      }
    }

    // Checkout main
    result = await runGit(repoPath, ["checkout", "main"]);
    if (result.exitCode !== 0) {
      if (hasChanges) await runGit(repoPath, ["stash", "pop"]);
      return { success: false, error: `Checkout main failed: ${result.stderr}` };
    }

    // Pull latest
    result = await runGit(repoPath, ["pull", "--ff-only", "origin", "main"]);
    if (result.exitCode !== 0) {
      result = await runGit(repoPath, ["pull", "--rebase", "origin", "main"]);
      if (result.exitCode !== 0) {
        if (hasChanges) await runGit(repoPath, ["stash", "pop"]);
        return { success: false, error: `Pull failed: ${result.stderr}` };
      }
    }

    // Merge branch
    result = await runGit(repoPath, ["merge", "--ff-only", branch]);
    if (result.exitCode !== 0) {
      result = await runGit(repoPath, ["merge", "--no-edit", branch]);
      if (result.exitCode !== 0) {
        if (hasChanges) await runGit(repoPath, ["stash", "pop"]);
        return { success: false, error: `Merge failed: ${result.stderr}` };
      }
    }

    // Get commit SHA
    result = await runGit(repoPath, ["rev-parse", "HEAD"]);
    const commitSha = result.stdout;

    // Push
    result = await runGit(repoPath, ["push", "origin", "main"]);
    if (result.exitCode !== 0) {
      if (hasChanges) await runGit(repoPath, ["stash", "pop"]);
      return { success: false, error: `Push failed: ${result.stderr}` };
    }

    // Pop stash if we had changes
    if (hasChanges) {
      await runGit(repoPath, ["stash", "pop"]);
    }

    return { success: true, commitSha };
  } catch (error: any) {
    return { success: false, error: error.message };
  }
};

// ─────────────────────────────────────────────────────────────────────────────
// Agent Runner
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Run orchestrator in a single worktree for one task.
 */
const runAgentInWorktree = async (
  slot: AgentSlot,
  repoPath: string,
  openagentsDir: string,
  projectConfig: any,
  ccOnly: boolean,
  verbose: boolean,
): Promise<void> => {
  if (!slot.worktree || !slot.task) return;

  const color = getAgentColor(slot.id);
  const prefix = `${color}[Agent ${slot.id}]${RESET}`;

  const agentLog = (msg: string) => log(`${prefix} ${msg}`);
  const verboseLog = (msg: string) => {
    if (verbose) log(`${prefix} ${DIM}${msg}${RESET}`);
  };

  agentLog(`Starting task: ${slot.task.id} - ${slot.task.title}`);

  const worktreeOpenagentsDir = path.join(slot.worktree.path, ".openagents");

  // Ensure .openagents directory exists in worktree
  fs.mkdirSync(worktreeOpenagentsDir, { recursive: true });

  // Copy tasks.jsonl to worktree
  const srcTasksPath = path.join(openagentsDir, "tasks.jsonl");
  const dstTasksPath = path.join(worktreeOpenagentsDir, "tasks.jsonl");
  if (!fs.existsSync(dstTasksPath) && fs.existsSync(srcTasksPath)) {
    fs.copyFileSync(srcTasksPath, dstTasksPath);
  }

  // Copy project.json to worktree
  const srcProjectPath = path.join(openagentsDir, "project.json");
  const dstProjectPath = path.join(worktreeOpenagentsDir, "project.json");
  if (!fs.existsSync(dstProjectPath) && fs.existsSync(srcProjectPath)) {
    fs.copyFileSync(srcProjectPath, dstProjectPath);
  }

  // Create empty progress.md so orchestrator can write to it
  const progressPath = path.join(worktreeOpenagentsDir, "progress.md");
  if (!fs.existsSync(progressPath)) {
    fs.writeFileSync(progressPath, "");
  }

  // Install dependencies
  agentLog(`Installing dependencies...`);
  const bunInstall = Bun.spawn(["bun", "install"], {
    cwd: slot.worktree.path,
    stdout: "pipe",
    stderr: "pipe",
  });
  await bunInstall.exited;
  if (bunInstall.exitCode !== 0) {
    const stderr = await new Response(bunInstall.stderr).text();
    slot.status = "failed";
    slot.error = `bun install failed: ${stderr}`;
    agentLog(`❌ ${slot.error}`);
    return;
  }
  agentLog(`Dependencies installed.`);

  // Build orchestrator config
  const claudeCodeConfig = ccOnly
    ? { enabled: true, preferForComplexTasks: false, fallbackToMinimal: false }
    : projectConfig.claudeCode;

  const orchestratorConfig = {
    cwd: slot.worktree.path,
    openagentsDir: worktreeOpenagentsDir,
    testCommands: [...(projectConfig.testCommands ?? ["bun test"])],
    typecheckCommands: [...(projectConfig.typecheckCommands ?? [])],
    e2eCommands: [...(projectConfig.e2eCommands ?? [])],
    allowPush: false, // Don't push from worktree
    claudeCode: claudeCodeConfig,
    subagentModel: projectConfig.defaultModel,
    // Force picking specific task
    taskId: slot.task.id,
    // Skip init script in worktrees - main repo is already validated
    skipInitScript: true,
    // Stream Claude Code output when verbose
    onOutput: verbose
      ? (text: string) => {
          // Stream CC output line by line with agent prefix
          const lines = text.split("\n").filter((l) => l.trim());
          for (const line of lines) {
            process.stdout.write(`${prefix} ${DIM}${line}${RESET}\n`);
          }
        }
      : undefined,
  };

  // Merge layers
  const combinedLayer = Layer.merge(
    BunContext.layer,
    Layer.provide(openRouterClientLayer, openRouterConfigLayer),
  );

  try {
    const state = await Effect.runPromise(
      runOrchestrator(orchestratorConfig, (event) => {
        if (event.type === "session_start") {
          agentLog(`Session: ${event.sessionId}`);
        } else if (event.type === "task_selected") {
          verboseLog(`Task selected: ${event.task.title}`);
        } else if (event.type === "task_decomposed") {
          verboseLog(`Decomposed into ${event.subtasks.length} subtask(s)`);
          if (verbose) {
            for (const st of event.subtasks) {
              verboseLog(`  → ${st.id}: ${st.description.slice(0, 60)}...`);
            }
          }
        } else if (event.type === "subtask_start") {
          agentLog(`▶ Subtask: ${event.subtask.id}`);
          verboseLog(`  ${event.subtask.description.slice(0, 100)}...`);
        } else if (event.type === "subtask_complete") {
          agentLog(`✅ Subtask complete: ${event.subtask.id}`);
        } else if (event.type === "subtask_failed") {
          agentLog(`❌ Subtask failed: ${event.error}`);
        } else if (event.type === "verification_start") {
          verboseLog(`Running: ${event.command}`);
        } else if (event.type === "verification_complete") {
          agentLog(`${event.passed ? "✅" : "❌"} Verification: ${event.command}`);
          if (verbose && !event.passed && event.output) {
            const lines = event.output.split("\n").slice(0, 10);
            for (const line of lines) {
              verboseLog(`  ${line}`);
            }
          }
        } else if (event.type === "commit_created") {
          agentLog(`📝 Commit: ${event.sha.slice(0, 8)} - ${event.message.split("\n")[0]}`);
        } else if (event.type === "error") {
          agentLog(`⚠️ Error in ${event.phase}: ${event.error}`);
        } else if (event.type === "session_complete") {
          agentLog(`${event.success ? "✅" : "❌"} ${event.summary}`);
        }
      }).pipe(Effect.provide(combinedLayer)),
    );

    if (state.phase === "done") {
      slot.status = "completed";
      agentLog(`✅ Task completed`);
    } else {
      slot.status = "failed";
      slot.error = state.error || "Unknown error";
      agentLog(`❌ Task failed: ${slot.error}`);
    }
  } catch (error: any) {
    slot.status = "failed";
    slot.error = error.message;
    agentLog(`❌ Exception: ${error.message}`);
  }
};

// ─────────────────────────────────────────────────────────────────────────────
// Main Loop
// ─────────────────────────────────────────────────────────────────────────────

const parallelOvernightLoop = async (config: ParallelConfig) => {
  const openagentsDir = path.join(config.workDir, ".openagents");
  const sessionId = `parallel-${Date.now()}`;

  // Initialize logging
  _logWorkDir = config.workDir;
  initLog(sessionId);

  log(`${"#".repeat(60)}`);
  log("PARALLEL OVERNIGHT AGENT STARTING");
  log(`Session: ${sessionId}`);
  log(`Work directory: ${config.workDir}`);
  log(`Max agents: ${config.maxAgents}`);
  log(`Max tasks: ${config.maxTasks}`);
  log(`Claude Code only: ${config.ccOnly}`);
  log(`Verbose: ${config.verbose}`);
  log(`Dry run: ${config.dryRun}`);
  log(`${"#".repeat(60)}\n`);

  // Change to work directory
  process.chdir(config.workDir);

  // Load project config
  const defaultProjectConfig = {
    projectId: "unknown",
    defaultBranch: "main",
    testCommands: ["bun test"],
    typecheckCommands: [],
    e2eCommands: [],
    allowPush: true,
    claudeCode: { enabled: true, preferForComplexTasks: true, fallbackToMinimal: true },
  };

  const loadedConfig = await Effect.runPromise(
    loadProjectConfig(config.workDir).pipe(
      Effect.provide(BunContext.layer),
      Effect.catchAll(() => Effect.succeed(null)),
    ),
  );
  const projectConfig = loadedConfig ?? defaultProjectConfig;
  log(`Project: ${projectConfig.projectId}`);

  // Load ready tasks
  const tasksPath = path.join(openagentsDir, "tasks.jsonl");
  const allTasks = await Effect.runPromise(
    readTasks(tasksPath).pipe(
      Effect.provide(BunContext.layer),
      Effect.catchAll(() => Effect.succeed([] as Task[])),
    ),
  );

  // Filter to ready tasks (open, no blocking deps)
  const readyTasks = allTasks.filter(
    (t) =>
      t.status === "open" &&
      !t.deps?.some((d) => {
        if (d.type !== "blocks") return false;
        const blocker = allTasks.find((x) => x.id === d.id);
        return blocker && blocker.status !== "closed";
      }),
  );

  // Sort by priority (lower = higher priority)
  readyTasks.sort((a, b) => (a.priority ?? 99) - (b.priority ?? 99));

  const tasksToProcess = readyTasks.slice(0, config.maxTasks);
  log(`Ready tasks: ${readyTasks.length}, will process: ${tasksToProcess.length}`);

  if (tasksToProcess.length === 0) {
    log("No ready tasks to process");
    return { tasksCompleted: 0, sessionId };
  }

  if (config.dryRun) {
    log("\n[DRY RUN] Would process tasks:");
    for (const t of tasksToProcess) {
      log(`  - ${t.id}: ${t.title}`);
    }
    return { tasksCompleted: 0, sessionId };
  }

  // Prune stale worktrees and locks
  await Effect.runPromise(pruneStaleWorktrees(config.workDir, 3600000).pipe(Effect.catchAll(() => Effect.void)));
  pruneWorktreeLocks(openagentsDir);

  // Create agent slots
  const slots: AgentSlot[] = [];
  let taskIndex = 0;
  let tasksCompleted = 0;

  // Process tasks in batches of maxAgents
  while (taskIndex < tasksToProcess.length) {
    const batchSize = Math.min(config.maxAgents, tasksToProcess.length - taskIndex);
    const batchTasks = tasksToProcess.slice(taskIndex, taskIndex + batchSize);

    log(`\n${"=".repeat(60)}`);
    log(`BATCH: Tasks ${taskIndex + 1}-${taskIndex + batchSize} of ${tasksToProcess.length}`);
    log(`${"=".repeat(60)}\n`);

    // Create worktrees for this batch
    const batchSlots: AgentSlot[] = [];
    for (let i = 0; i < batchTasks.length; i++) {
      const task = batchTasks[i];
      const slotId = taskIndex + i + 1;

      const worktreeConfig: WorktreeConfig = {
        taskId: task.id,
        sessionId,
        baseBranch: projectConfig.defaultBranch ?? "main",
        timeoutMs: 30 * 60 * 1000, // 30 minutes
      };

      log(`[Agent ${slotId}] Creating worktree for ${task.id}...`);

      try {
        // Try to remove existing worktree first (in case of stale state)
        await Effect.runPromise(
          removeWorktree(config.workDir, task.id).pipe(Effect.catchAll(() => Effect.void)),
        );

        const worktree = await Effect.runPromise(createWorktree(config.workDir, worktreeConfig));
        log(`[Agent ${slotId}] Worktree: ${worktree.path}`);

        // Acquire lock
        const lockAcquired = acquireWorktreeLock(openagentsDir, task.id, sessionId);
        if (!lockAcquired) {
          log(`[Agent ${slotId}] ❌ Failed to acquire lock`);
          continue;
        }

        batchSlots.push({
          id: slotId,
          taskId: task.id,
          task,
          worktree,
          status: "idle",
        });
      } catch (error: any) {
        log(`[Agent ${slotId}] ❌ Failed to create worktree: ${error.message}`);
      }
    }

    // Run agents in parallel
    log(`\nRunning ${batchSlots.length} agents in parallel...`);
    await Promise.all(
      batchSlots.map((slot) =>
        runAgentInWorktree(slot, config.workDir, openagentsDir, projectConfig, config.ccOnly, config.verbose),
      ),
    );

    // Merge completed worktrees sequentially
    log("\nMerging completed worktrees...");
    for (const slot of batchSlots) {
      if (slot.status === "completed" && slot.worktree) {
        const hasCommits = await hasCommitsAhead(
          slot.worktree.path,
          projectConfig.defaultBranch ?? "main",
        );

        if (hasCommits) {
          log(`[Agent ${slot.id}] Merging ${slot.worktree.branch} to main...`);
          const mergeResult = await mergeToMain(config.workDir, slot.worktree.branch);
          if (mergeResult.success) {
            if (mergeResult.commitSha) {
              slot.commitSha = mergeResult.commitSha;
            }
            log(`[Agent ${slot.id}] ✅ Merged: ${mergeResult.commitSha?.slice(0, 8)}`);
            tasksCompleted++;
          } else {
            log(`[Agent ${slot.id}] ❌ Merge failed: ${mergeResult.error}`);
          }
        } else {
          log(`[Agent ${slot.id}] No commits to merge`);
          tasksCompleted++; // Still count as completed
        }
      }

      // Cleanup worktree
      if (slot.worktree) {
        releaseWorktreeLock(openagentsDir, slot.taskId!);
        await Effect.runPromise(
          removeWorktree(config.workDir, slot.taskId!).pipe(Effect.catchAll(() => Effect.void)),
        );
        log(`[Agent ${slot.id}] Worktree cleaned up`);
      }
    }

    slots.push(...batchSlots);
    taskIndex += batchSize;
  }

  log(`\n${"#".repeat(60)}`);
  log("PARALLEL OVERNIGHT AGENT FINISHED");
  log(`Tasks completed: ${tasksCompleted}/${tasksToProcess.length}`);
  log(`Log file: ${logFilePath}`);
  log(`${"#".repeat(60)}\n`);

  return { tasksCompleted, sessionId };
};

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

const config = parseArgs();

console.log(`[Parallel mode: ${config.maxAgents} agents, ${config.maxTasks} max tasks]`);
if (config.ccOnly) {
  console.log("[Claude Code ONLY - no Grok fallback]");
}

parallelOvernightLoop(config)
  .then((result) => {
    console.log(`\nDone! Completed ${result.tasksCompleted} tasks.`);
    process.exit(0);
  })
  .catch((error) => {
    console.error("Fatal error:", error);
    process.exit(1);
  });
