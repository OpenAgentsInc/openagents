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
 *
 * @see docs/claude/plans/containers-impl-v2.md
 */
import { Effect } from "effect";
import * as path from "node:path";
import type { Task } from "../../tasks/index.js";
import {
  createWorktree,
  removeWorktree,
  listWorktrees,
  pruneStaleWorktrees,
  type WorktreeInfo,
  type WorktreeConfig,
} from "./worktree.js";
import {
  acquireWorktreeLock,
  releaseWorktreeLock,
  pruneWorktreeLocks,
} from "./agent-lock.js";

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
    readonly cause?: unknown,
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
        yield* Effect.fail(
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
        yield* Effect.fail(
          new ParallelRunnerError("merge_failed", `Merge failed after rebase: ${retryResult.stderr}`),
        );
      }
    }

    // Get the merge commit SHA
    const headResult = yield* runGit(repoPath, ["rev-parse", "HEAD"]);
    return headResult.stdout || null;
  });

// ─────────────────────────────────────────────────────────────────────────────
// Agent Execution (Placeholder)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Run a single agent in a worktree.
 *
 * This is a placeholder that will be implemented to integrate with
 * claude-code-subagent or minimal subagent.
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

    // TODO: Integrate with actual agent execution
    // For now, this is a placeholder that simulates work
    yield* Effect.tryPromise({
      try: () => new Promise((resolve) => setTimeout(resolve, 100)),
      catch: (e) => new ParallelRunnerError("agent_failed", `Agent failed: ${e}`, e),
    });

    // The real implementation would:
    // 1. Create SubagentConfig with cwd = worktree.path
    // 2. Run runSubagent or runClaudeCodeSubagent
    // 3. Collect results

    return {
      success: true,
      filesModified: [],
      turns: 0,
    } satisfies AgentResult;
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
        yield* Effect.fail(
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
          slot.error = result.error;
          config.onAgentEvent?.(slot.task.id, {
            type: "agent_failed",
            taskId: slot.task.id,
            error: result.error || "Unknown error",
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
          commitSha: commitSha || undefined,
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
          commitSha: commitSha || undefined,
        });
      }
    } else {
      // PR strategy: Create PRs instead of merging
      // TODO: Implement PR creation via gh CLI
      for (const slot of successfulSlots) {
        config.onAgentEvent?.(slot.task.id, {
          type: "merge_started",
          taskId: slot.task.id,
          strategy: "pr",
        });

        // PR creation would go here
        // For now, just mark as complete without merge

        config.onAgentEvent?.(slot.task.id, {
          type: "merge_completed",
          taskId: slot.task.id,
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
