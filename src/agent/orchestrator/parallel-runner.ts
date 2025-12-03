/**
 * Parallel Agent Runner
 *
 * Coordinates N agents running on N isolated git worktrees simultaneously.
 * Each agent works on a different task in its own worktree.
 *
 * Features:
 * - Creates isolated worktrees for each agent
 * - Per-worktree locking to prevent conflicts
 * - Supports direct commit, queue, and PR merge strategies
 * - Integrates with container sandbox (optional)
 * - Enforces Golden Loop invariants per worktree
 *
 * @see docs/claude/plans/containers-impl-v2.md
 * @see docs/mechacoder/GOLDEN-LOOP-v2.md
 */
import * as fs from "node:fs";
import * as path from "node:path";
import * as BunContext from "@effect/platform-bun/BunContext";
import { Effect, Layer } from "effect";
import type { Task, ParallelExecutionConfig } from "../../tasks/index.js";
import { openRouterClientLayer, openRouterConfigLayer, OpenRouterClient } from "../../llm/openrouter.js";
import {
  createWorktree,
  removeWorktree,
  pruneStaleWorktrees,
  type WorktreeInfo,
  type WorktreeConfig,
} from "./worktree.js";
import {
  acquireWorktreeLock,
  releaseWorktreeLock,
  pruneWorktreeLocks,
} from "./agent-lock.js";
import { runOrchestrator } from "./orchestrator.js";
import type { OrchestratorConfig, ClaudeCodeSettings } from "./types.js";

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

export type MergeStrategy = "auto" | "direct" | "queue" | "pr";

export interface ParallelRunnerConfig {
  /** Path to the repository root */
  repoPath: string;
  /** Path to .openagents directory */
  openagentsDir: string;
  /** Maximum number of agents to run in parallel */
  maxAgents: number;
  /** Tasks to distribute to agents */
  tasks: Task[];
  /** Base branch to create worktrees from */
  baseBranch?: string;
  /** Session ID for lock tracking */
  sessionId: string;
  /** Merge strategy (auto-selected if "auto") */
  mergeStrategy?: MergeStrategy;
  /** Number of agents before switching from direct to queue (when auto) */
  mergeThreshold?: number;
  /** Number of agents before switching from queue to PR (when auto) */
  prThreshold?: number;
  /** Container image (optional - run in containers if provided) */
  containerImage?: string;
  /** Timeout per agent in ms */
  timeoutMs?: number;
  /** Event callback */
  onAgentEvent?: (agentId: string, event: AgentEvent) => void;
  // ─── Orchestrator Settings ─────────────────────────────────────────────────
  /** Test commands from project.json */
  testCommands?: string[];
  /** Typecheck commands from project.json */
  typecheckCommands?: string[];
  /** E2E commands from project.json */
  e2eCommands?: string[];
  /** Claude Code settings */
  claudeCode?: ClaudeCodeSettings;
  /** Model to use for subagents */
  subagentModel?: string;
  /** Use Claude Code only mode (no OpenRouter fallback) */
  ccOnly?: boolean;
  /** Verbose logging */
  verbose?: boolean;
  /** Allow push after merge */
  allowPush?: boolean;
}

export type AgentStatus = "pending" | "running" | "completed" | "failed";

export interface AgentSlot {
  id: string;
  worktree: WorktreeInfo | null;
  task: Task;
  status: AgentStatus;
  result?: AgentResult;
  startedAt?: string;
  completedAt?: string;
  error?: string;
}

export interface AgentResult {
  success: boolean;
  filesModified: string[];
  commitSha?: string;
  error?: string;
  turns?: number;
}

export type AgentEvent =
  | { type: "agent_started"; taskId: string; worktreePath: string }
  | { type: "agent_completed"; taskId: string; success: boolean; filesModified: string[] }
  | { type: "agent_failed"; taskId: string; error: string }
  | { type: "worktree_created"; taskId: string; path: string }
  | { type: "worktree_removed"; taskId: string }
  | { type: "merge_started"; taskId: string; strategy: MergeStrategy }
  | { type: "merge_completed"; taskId: string; commitSha?: string };

export class ParallelRunnerError extends Error {
  readonly _tag = "ParallelRunnerError";
  constructor(
    readonly reason:
      | "setup_failed"
      | "agent_failed"
      | "merge_failed"
      | "cleanup_failed"
      | "timeout",
    message: string,
    override readonly cause?: unknown,
  ) {
    super(message);
    this.name = "ParallelRunnerError";
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Merge Strategy Selection
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Select the optimal merge strategy based on agent count.
 *
 * - ≤mergeThreshold (default 4): Direct commit to main
 * - ≤prThreshold (default 50): Local merge queue
 * - >prThreshold: PR flow for rate limiting
 */
export const selectMergeStrategy = (config: ParallelRunnerConfig): MergeStrategy => {
  if (config.mergeStrategy && config.mergeStrategy !== "auto") {
    return config.mergeStrategy;
  }

  const agentCount = Math.min(config.tasks.length, config.maxAgents);
  const mergeThreshold = config.mergeThreshold ?? 4;
  const prThreshold = config.prThreshold ?? 50;

  if (agentCount <= mergeThreshold) return "direct";
  if (agentCount <= prThreshold) return "queue";
  return "pr";
};

// ─────────────────────────────────────────────────────────────────────────────
// Git Operations
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Run a git command in a directory
 */
const runGit = (
  cwd: string,
  args: string[],
): Effect.Effect<{ exitCode: number; stdout: string; stderr: string }, ParallelRunnerError> =>
  Effect.tryPromise({
    try: async () => {
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
    },
    catch: (e) => new ParallelRunnerError("merge_failed", `Git command failed: ${e}`, e),
  });

/**
 * Merge agent branch to main using fast-forward merge.
 */
const mergeAgentBranch = (
  repoPath: string,
  branch: string,
): Effect.Effect<string | null, ParallelRunnerError> =>
  Effect.gen(function* () {
    // Fetch latest main
    yield* runGit(repoPath, ["fetch", "origin", "main"]);

    // Checkout main
    yield* runGit(repoPath, ["checkout", "main"]);

    // Pull latest
    yield* runGit(repoPath, ["pull", "--ff-only", "origin", "main"]);

    // Merge agent branch
    const mergeResult = yield* runGit(repoPath, ["merge", "--ff-only", branch]);

    if (mergeResult.exitCode !== 0) {
      // Try rebase if ff-only fails
      const rebaseResult = yield* runGit(repoPath, ["rebase", "main", branch]);
      if (rebaseResult.exitCode !== 0) {
        return yield* Effect.fail(
          new ParallelRunnerError(
            "merge_failed",
            `Could not merge ${branch}: ${rebaseResult.stderr}`,
          ),
        );
      }

      // Retry merge after rebase
      yield* runGit(repoPath, ["checkout", "main"]);
      const retryResult = yield* runGit(repoPath, ["merge", "--ff-only", branch]);
      if (retryResult.exitCode !== 0) {
        return yield* Effect.fail(
          new ParallelRunnerError("merge_failed", `Merge failed after rebase: ${retryResult.stderr}`),
        );
      }
    }

    // Get the merge commit SHA
    const headResult = yield* runGit(repoPath, ["rev-parse", "HEAD"]);
    return headResult.stdout || null;
  });

// ─────────────────────────────────────────────────────────────────────────────
// Agent Execution
// ─────────────────────────────────────────────────────────────────────────────

// Noop OpenRouter layer for cc-only mode
const noopOpenRouterLayer = Layer.succeed(OpenRouterClient, {
  chat: () => Effect.fail(new Error("OpenRouter not available in cc-only mode")),
});

/**
 * Run a single agent in a worktree using the orchestrator.
 *
 * Implements Golden Loop invariants:
 * - Skip init script (worktree inherits from main repo)
 * - Force new subtasks (avoid stale state)
 * - Pre-assign task (prevent all agents picking same task)
 * - Don't push from worktree (merge handles push)
 */
const runAgentInWorktree = (
  worktree: WorktreeInfo,
  task: Task,
  config: ParallelRunnerConfig,
): Effect.Effect<AgentResult, ParallelRunnerError> =>
  Effect.gen(function* () {
    // Emit started event
    config.onAgentEvent?.(task.id, {
      type: "agent_started",
      taskId: task.id,
      worktreePath: worktree.path,
    });

    const worktreeOpenagentsDir = path.join(worktree.path, ".openagents");

    // Ensure .openagents directory exists in worktree
    if (!fs.existsSync(worktreeOpenagentsDir)) {
      fs.mkdirSync(worktreeOpenagentsDir, { recursive: true });
    }

    // Copy tasks.jsonl to worktree
    const srcTasksPath = path.join(config.openagentsDir, "tasks.jsonl");
    const dstTasksPath = path.join(worktreeOpenagentsDir, "tasks.jsonl");
    if (!fs.existsSync(dstTasksPath) && fs.existsSync(srcTasksPath)) {
      fs.copyFileSync(srcTasksPath, dstTasksPath);
    }

    // Copy project.json to worktree
    const srcProjectPath = path.join(config.openagentsDir, "project.json");
    const dstProjectPath = path.join(worktreeOpenagentsDir, "project.json");
    if (!fs.existsSync(dstProjectPath) && fs.existsSync(srcProjectPath)) {
      fs.copyFileSync(srcProjectPath, dstProjectPath);
    }

    // Install dependencies in worktree
    yield* Effect.tryPromise({
      try: async () => {
        const proc = Bun.spawn(["bun", "install"], {
          cwd: worktree.path,
          stdout: "pipe",
          stderr: "pipe",
        });
        await proc.exited;
        if (proc.exitCode !== 0) {
          const stderr = await new Response(proc.stderr).text();
          throw new Error(`bun install failed: ${stderr}`);
        }
      },
      catch: (e) => new ParallelRunnerError("agent_failed", `Dependency install failed: ${e}`, e),
    });

    // Build orchestrator config with Golden Loop invariants
    const orchestratorConfig: OrchestratorConfig = {
      cwd: worktree.path,
      openagentsDir: worktreeOpenagentsDir,
      testCommands: config.testCommands ?? ["bun test"],
      allowPush: false, // Don't push from worktree - merge handles this
      skipInitScript: true, // Worktree inherits validated state from main
      task, // Pre-assigned task (CRITICAL: prevents race condition)
      forceNewSubtasks: true, // Avoid stale subtask files from git
      claudeCode: config.claudeCode ?? { enabled: true },
    };

    // Only set optional properties if they have values
    if (config.typecheckCommands) {
      orchestratorConfig.typecheckCommands = config.typecheckCommands;
    }
    if (config.e2eCommands) {
      orchestratorConfig.e2eCommands = config.e2eCommands;
    }
    if (config.subagentModel) {
      orchestratorConfig.subagentModel = config.subagentModel;
    }

    // Select layer based on cc-only mode
    const combinedLayer = config.ccOnly
      ? Layer.merge(BunContext.layer, noopOpenRouterLayer)
      : Layer.merge(
          BunContext.layer,
          Layer.provide(openRouterClientLayer, openRouterConfigLayer),
        );

    // Run orchestrator
    const state = yield* Effect.tryPromise({
      try: async () => {
        return await Effect.runPromise(
          runOrchestrator(orchestratorConfig, (event) => {
            // Forward relevant events to parent
            if (event.type === "commit_created") {
              config.onAgentEvent?.(task.id, {
                type: "agent_completed",
                taskId: task.id,
                success: true,
                filesModified: [],
              });
            }
          }).pipe(Effect.provide(combinedLayer)),
        );
      },
      catch: (e: any) => new ParallelRunnerError("agent_failed", `Orchestrator failed: ${e.message}`, e),
    });

    const success = state.phase === "done";
    const filesModified: string[] = [];

    const result: AgentResult = {
      success,
      filesModified,
      turns: 0,
    };

    if (state.error) {
      result.error = state.error;
    }

    return result;
  });

// ─────────────────────────────────────────────────────────────────────────────
// Parallel Runner Core
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Run N agents in parallel on N isolated worktrees.
 *
 * Flow:
 * 1. Prune stale worktrees and locks
 * 2. Create worktrees for each task (up to maxAgents)
 * 3. Acquire per-worktree locks
 * 4. Spawn agents in parallel
 * 5. Collect results
 * 6. Merge changes based on strategy
 * 7. Cleanup worktrees
 */
export const runParallelAgents = (
  config: ParallelRunnerConfig,
): Effect.Effect<AgentSlot[], ParallelRunnerError> =>
  Effect.gen(function* () {
    const { repoPath, openagentsDir, maxAgents, tasks, baseBranch = "main" } = config;
    const strategy = selectMergeStrategy(config);

    // Step 1: Prune stale resources
    yield* pruneStaleWorktrees(repoPath, 3600000).pipe(
      Effect.mapError((e) => new ParallelRunnerError("setup_failed", `Prune failed: ${e.message}`, e)),
    );
    pruneWorktreeLocks(openagentsDir);

    // Step 2: Limit tasks to maxAgents
    const tasksToRun = tasks.slice(0, maxAgents);
    const slots: AgentSlot[] = tasksToRun.map((task) => ({
      id: task.id,
      worktree: null,
      task,
      status: "pending" as AgentStatus,
    }));

    // Step 3: Create worktrees and acquire locks
    for (const slot of slots) {
      const worktreeConfig: WorktreeConfig = {
        taskId: slot.task.id,
        sessionId: config.sessionId,
        baseBranch,
        timeoutMs: config.timeoutMs ?? 30 * 60 * 1000,
      };

      // Create worktree
      const worktree = yield* createWorktree(repoPath, worktreeConfig).pipe(
        Effect.mapError(
          (e) => new ParallelRunnerError("setup_failed", `Worktree creation failed: ${e.message}`, e),
        ),
      );

      slot.worktree = worktree;

      config.onAgentEvent?.(slot.task.id, {
        type: "worktree_created",
        taskId: slot.task.id,
        path: worktree.path,
      });

      // Acquire lock
      const lockAcquired = acquireWorktreeLock(openagentsDir, slot.task.id, config.sessionId);
      if (!lockAcquired) {
        return yield* Effect.fail(
          new ParallelRunnerError("setup_failed", `Could not acquire lock for ${slot.task.id}`),
        );
      }
    }

    // Step 4: Run agents in parallel
    const agentEffects = slots.map((slot) =>
      Effect.gen(function* () {
        if (!slot.worktree) {
          slot.status = "failed";
          slot.error = "No worktree available";
          return slot;
        }

        slot.status = "running";
        slot.startedAt = new Date().toISOString();

        const result = yield* runAgentInWorktree(slot.worktree, slot.task, config).pipe(
          Effect.catchAll((error) =>
            Effect.succeed({
              success: false,
              filesModified: [],
              error: error.message,
            } satisfies AgentResult),
          ),
        );

        slot.result = result;
        slot.status = result.success ? "completed" : "failed";
        slot.completedAt = new Date().toISOString();

        if (result.success) {
          config.onAgentEvent?.(slot.task.id, {
            type: "agent_completed",
            taskId: slot.task.id,
            success: true,
            filesModified: result.filesModified,
          });
        } else {
          if (result.error !== undefined) {
            slot.error = result.error;
          }
          config.onAgentEvent?.(slot.task.id, {
            type: "agent_failed",
            taskId: slot.task.id,
            error: result.error ?? "Unknown error",
          });
        }

        return slot;
      }),
    );

    // Wait for all agents with parallel execution
    const completedSlots = yield* Effect.all(agentEffects, { concurrency: maxAgents });

    // Step 5: Merge based on strategy (only for successful agents)
    const successfulSlots = completedSlots.filter((s) => s.status === "completed" && s.worktree);

    if (strategy === "direct") {
      // Direct merge: one at a time, fast-forward
      for (const slot of successfulSlots) {
        if (!slot.worktree) continue;

        config.onAgentEvent?.(slot.task.id, {
          type: "merge_started",
          taskId: slot.task.id,
          strategy: "direct",
        });

        const commitSha = yield* mergeAgentBranch(repoPath, slot.worktree.branch).pipe(
          Effect.catchAll((error) => {
            slot.error = error.message;
            return Effect.succeed(null);
          }),
        );

        if (commitSha && slot.result) {
          slot.result.commitSha = commitSha;
        }

        config.onAgentEvent?.(slot.task.id, {
          type: "merge_completed",
          taskId: slot.task.id,
          ...(commitSha ? { commitSha } : {}),
        });
      }
    } else if (strategy === "queue") {
      // Queue: Batch commits, merge sequentially
      // For now, same as direct - can optimize later for batched push
      for (const slot of successfulSlots) {
        if (!slot.worktree) continue;

        config.onAgentEvent?.(slot.task.id, {
          type: "merge_started",
          taskId: slot.task.id,
          strategy: "queue",
        });

        const commitShaQueue = yield* mergeAgentBranch(repoPath, slot.worktree.branch).pipe(
          Effect.catchAll((error) => {
            slot.error = error.message;
            return Effect.succeed(null);
          }),
        );

        if (commitShaQueue && slot.result) {
          slot.result.commitSha = commitShaQueue;
        }

        config.onAgentEvent?.(slot.task.id, {
          type: "merge_completed",
          taskId: slot.task.id,
          ...(commitShaQueue ? { commitSha: commitShaQueue } : {}),
        });
      }
    } else {
      // PR strategy: Create PRs via gh CLI
      for (const slot of successfulSlots) {
        if (!slot.worktree) continue;

        config.onAgentEvent?.(slot.task.id, {
          type: "merge_started",
          taskId: slot.task.id,
          strategy: "pr",
        });

        // Push the branch first
        yield* runGit(repoPath, ["push", "-u", "origin", slot.worktree.branch]).pipe(
          Effect.catchAll(() => Effect.succeed({ exitCode: 1, stdout: "", stderr: "" })),
        );

        // Create PR using gh CLI
        const prResult = yield* Effect.tryPromise({
          try: async () => {
            const title = `${slot.task.id}: ${slot.task.title}`;
            const body = `## Summary\n\n${slot.task.description || "Automated task completion"}\n\n🤖 Generated with [OpenAgents](https://openagents.com)`;
            const proc = Bun.spawn(["gh", "pr", "create", "--title", title, "--body", body, "--head", slot.worktree!.branch], {
              cwd: repoPath,
              stdout: "pipe",
              stderr: "pipe",
            });
            const stdout = await new Response(proc.stdout).text();
            const exitCode = await proc.exited;
            return { exitCode, stdout: stdout.trim() };
          },
          catch: (e) => new ParallelRunnerError("merge_failed", `PR creation failed: ${e}`, e),
        }).pipe(
          Effect.catchAll(() => Effect.succeed({ exitCode: 1, stdout: "" })),
        );

        // Store PR URL as commitSha for tracking
        if (prResult.exitCode === 0 && slot.result) {
          slot.result.commitSha = prResult.stdout; // PR URL
        }

        config.onAgentEvent?.(slot.task.id, {
          type: "merge_completed",
          taskId: slot.task.id,
          ...(prResult.exitCode === 0 ? { commitSha: prResult.stdout } : {}),
        });
      }
    }

    // Step 6: Cleanup worktrees and release locks
    for (const slot of slots) {
      releaseWorktreeLock(openagentsDir, slot.task.id);

      if (slot.worktree) {
        yield* removeWorktree(repoPath, slot.task.id).pipe(
          Effect.catchAll(() => Effect.succeed(undefined)),
        );

        config.onAgentEvent?.(slot.task.id, {
          type: "worktree_removed",
          taskId: slot.task.id,
        });
      }
    }

    return completedSlots;
  });

// ─────────────────────────────────────────────────────────────────────────────
// Convenience Functions
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Run parallel agents and return a Promise.
 */
export const runParallelAgentsAsync = (
  config: ParallelRunnerConfig,
): Promise<AgentSlot[]> => Effect.runPromise(runParallelAgents(config));

/**
 * Configuration for creating a parallel runner from ParallelExecutionConfig.
 */
export interface CreateParallelRunnerOptions {
  /** Path to the repository root */
  repoPath: string;
  /** Path to .openagents directory */
  openagentsDir: string;
  /** Base branch (from ProjectConfig.defaultBranch) */
  baseBranch: string;
  /** Tasks to run in parallel */
  tasks: Task[];
  /** ParallelExecutionConfig from project.json */
  parallelConfig: ParallelExecutionConfig;
  /** Test commands from project.json */
  testCommands?: string[];
  /** Typecheck commands from project.json */
  typecheckCommands?: string[];
  /** E2E commands from project.json */
  e2eCommands?: string[];
  /** Claude Code settings */
  claudeCode?: ClaudeCodeSettings;
  /** Model for subagents */
  subagentModel?: string;
  /** Use Claude Code only (no OpenRouter fallback) */
  ccOnly?: boolean;
  /** Allow push to remote */
  allowPush?: boolean;
  /** Event callback */
  onAgentEvent?: (agentId: string, event: AgentEvent) => void;
}

/**
 * Create and run a parallel runner from ParallelExecutionConfig.
 *
 * This is the main entry point for running parallel agents using the
 * configuration from .openagents/project.json.
 *
 * @example
 * ```typescript
 * const projectConfig = await loadProjectConfig(repoPath);
 * const tasks = await readyTasks(tasksPath);
 *
 * if (projectConfig.parallelExecution?.enabled) {
 *   const result = await runParallelFromConfig({
 *     repoPath,
 *     openagentsDir: `${repoPath}/.openagents`,
 *     baseBranch: projectConfig.defaultBranch,
 *     tasks: tasks.slice(0, projectConfig.parallelExecution.maxAgents),
 *     parallelConfig: projectConfig.parallelExecution,
 *     testCommands: projectConfig.testCommands,
 *     typecheckCommands: projectConfig.typecheckCommands,
 *     claudeCode: projectConfig.claudeCode,
 *   });
 * }
 * ```
 */
export const runParallelFromConfig = (
  options: CreateParallelRunnerOptions,
): Effect.Effect<AgentSlot[], ParallelRunnerError> => {
  const sessionId = `parallel-${Date.now()}`;

  const config: ParallelRunnerConfig = {
    repoPath: options.repoPath,
    openagentsDir: options.openagentsDir,
    maxAgents: options.parallelConfig.maxAgents ?? 4,
    tasks: options.tasks,
    baseBranch: options.baseBranch,
    sessionId,
    mergeStrategy: options.parallelConfig.mergeStrategy ?? "auto",
    mergeThreshold: options.parallelConfig.mergeThreshold ?? 4,
    prThreshold: options.parallelConfig.prThreshold ?? 50,
    timeoutMs: options.parallelConfig.worktreeTimeout ?? 30 * 60 * 1000,
  };

  // Only set optional properties if they have values
  if (options.testCommands) {
    config.testCommands = options.testCommands;
  }
  if (options.typecheckCommands) {
    config.typecheckCommands = options.typecheckCommands;
  }
  if (options.e2eCommands) {
    config.e2eCommands = options.e2eCommands;
  }
  if (options.claudeCode) {
    config.claudeCode = options.claudeCode;
  }
  if (options.subagentModel) {
    config.subagentModel = options.subagentModel;
  }
  if (options.ccOnly !== undefined) {
    config.ccOnly = options.ccOnly;
  }
  if (options.allowPush !== undefined) {
    config.allowPush = options.allowPush;
  }
  if (options.onAgentEvent) {
    config.onAgentEvent = options.onAgentEvent;
  }

  return runParallelAgents(config);
};

/**
 * Run parallel agents from config and return a Promise.
 */
export const runParallelFromConfigAsync = (
  options: CreateParallelRunnerOptions,
): Promise<AgentSlot[]> => Effect.runPromise(runParallelFromConfig(options));
