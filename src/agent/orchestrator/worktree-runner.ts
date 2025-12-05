#!/usr/bin/env bun
/**
 * Worktree Runner
 *
 * Runs MechaCoder in an isolated git worktree for a single task.
 * This is the integration between worktrees and the orchestrator.
 *
 * Usage:
 *   bun src/agent/orchestrator/worktree-runner.ts [--task-id <id>] [--dry-run]
 *
 * @see docs/claude/plans/containers-impl-v2.md
 */
import { Effect } from "effect";
import * as BunContext from "@effect/platform-bun/BunContext";
import * as path from "node:path";
import * as fs from "node:fs";
import {
  ensureValidWorktree,
  removeWorktree,
  type WorktreeConfig,
} from "./worktree.js";
import {
  acquireWorktreeLock,
  releaseWorktreeLock,
} from "./agent-lock.js";
import { runOrchestrator } from "./orchestrator.js";
import { makeReflectionService } from "./reflection/index.js";
import { loadProjectConfig } from "../../tasks/index.js";
import { openRouterClientLayer, openRouterConfigLayer } from "../../llm/openrouter.js";
import { Layer } from "effect";
import { createWorktreeGuardHook } from "./worktree-guards.js";
import {
  hasCommitsAhead,
  mergeBranch,
  type MergeResult,
} from "./git-helpers.js";
import { getInstallSettings, installDeps } from "./install-deps.js";

export { getInstallSettings } from "./install-deps.js";
export type { InstallSettings } from "./install-deps.js";

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

interface WorktreeRunnerOptions {
  repoPath: string;
  taskId?: string;
  dryRun?: boolean;
  sessionId?: string;
}

interface WorktreeRunResult {
  success: boolean;
  taskId?: string;
  worktreePath?: string;
  commitSha?: string;
  error?: string;
  merged: boolean;
}

// ─────────────────────────────────────────────────────────────────────────────
// Main Runner
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Run MechaCoder in a worktree for a single task.
 */
export const runInWorktree = async (
  options: WorktreeRunnerOptions,
): Promise<WorktreeRunResult> => {
  const { repoPath, dryRun = false } = options;
  const sessionId = options.sessionId || `worktree-${Date.now()}`;
  const openagentsDir = path.join(repoPath, ".openagents");

  console.log("\n🌳 Worktree Runner\n");
  console.log(`  Repo:      ${repoPath}`);
  console.log(`  Session:   ${sessionId}`);
  console.log(`  Dry Run:   ${dryRun}`);

  // Load project config
  const projectConfig = await Effect.runPromise(
    loadProjectConfig(repoPath).pipe(Effect.provide(BunContext.layer)),
  );
  if (!projectConfig) {
    console.error("  ❌ No project config found");
    return {
      success: false,
      error: "No .openagents/project.json found",
      merged: false,
    };
  }
  console.log(`  Project:   ${projectConfig.projectId}`);

  // Determine task ID (use provided or will be picked by orchestrator)
  const taskId = options.taskId || `task-${Date.now().toString(36)}`;
  console.log(`  Task ID:   ${taskId}`);

  const targetBranch = projectConfig.workBranch ?? projectConfig.defaultBranch ?? "main";
  const testCommands = [...(projectConfig.testCommands ?? [])];
  const typecheckCommands = [...(projectConfig.typecheckCommands ?? [])];
  const e2eCommands = [...(projectConfig.e2eCommands ?? [])];
  const sandboxTestCommands = [...(projectConfig.sandboxTestCommands ?? [])];

  // Create/validate worktree (self-healing)
  console.log("\n📁 Ensuring valid worktree...");
  const worktreeConfig: WorktreeConfig = {
    taskId,
    sessionId,
    baseBranch: targetBranch,
    timeoutMs: (projectConfig.maxRuntimeMinutes ?? 240) * 60 * 1000,
  };

  let worktreeInfo;
  try {
    // ensureValidWorktree will create if missing, validate if exists, and repair if corrupted
    worktreeInfo = await Effect.runPromise(ensureValidWorktree(repoPath, worktreeConfig));
    console.log(`  Path:      ${worktreeInfo.path}`);
    console.log(`  Branch:    ${worktreeInfo.branch}`);
  } catch (error: any) {
    console.error(`  ❌ Failed to ensure valid worktree: ${error.message}`);
    return {
      success: false,
      taskId,
      error: `Failed to ensure valid worktree: ${error.message}`,
      merged: false,
    };
  }

  // Acquire lock
  console.log("\n🔒 Acquiring lock...");
  const lockAcquired = acquireWorktreeLock(openagentsDir, taskId, sessionId);
  if (!lockAcquired) {
    console.error("  ❌ Failed to acquire lock");
    await Effect.runPromise(removeWorktree(repoPath, taskId)).catch(() => {});
    return {
      success: false,
      taskId,
      worktreePath: worktreeInfo.path,
      error: "Failed to acquire lock",
      merged: false,
    };
  }
  console.log("  ✅ Lock acquired");

  let result: WorktreeRunResult = {
    success: false,
    taskId,
    worktreePath: worktreeInfo.path,
    merged: false,
  };

  try {
    if (dryRun) {
      console.log("\n🏃 Dry run - skipping orchestrator execution");
      console.log("  Would run MechaCoder in:", worktreeInfo.path);
      result.success = true;
    } else {
      // Run orchestrator in worktree
      console.log("\n🤖 Running MechaCoder in worktree...");

      const worktreeOpenagentsDir = path.join(worktreeInfo.path, ".openagents");

      // Copy tasks.jsonl to worktree if it doesn't exist
      const srcTasksPath = path.join(openagentsDir, "tasks.jsonl");
      const dstTasksPath = path.join(worktreeOpenagentsDir, "tasks.jsonl");
      if (!fs.existsSync(dstTasksPath) && fs.existsSync(srcTasksPath)) {
        fs.mkdirSync(worktreeOpenagentsDir, { recursive: true });
        fs.copyFileSync(srcTasksPath, dstTasksPath);
      }

      // Install dependencies in worktree (required for typecheck)
      const installSettings = getInstallSettings(projectConfig);
      if (installSettings.skipInstall) {
        console.log("  Skipping dependency install (--skip-install provided).");
      } else {
        console.log(
          `  Installing dependencies (bun install ${installSettings.args.join(" ")})...`,
        );
        const installResult = await installDeps(worktreeInfo.path, installSettings);
        if (!installResult.success) {
          const message =
            installResult.error ??
            `bun install ${installSettings.args.join(" ")} failed${
              installResult.timedOut ? " (timed out)" : ""
            }`;
          console.error(`  ❌ ${message}`);
          result.error = message;
          return result;
        }
        console.log("  Dependencies installed.");
      }

      const reflectionService = makeReflectionService({
        openagentsDir: worktreeOpenagentsDir,
        cwd: worktreeInfo.path,
        config: projectConfig.reflexion,
      });

      const orchestratorConfig = {
        cwd: worktreeInfo.path,
        openagentsDir: worktreeOpenagentsDir,
        testCommands,
        ...(sandboxTestCommands.length > 0 ? { sandboxTestCommands } : {}),
        typecheckCommands,
        e2eCommands,
        allowPush: false, // Don't push from worktree - we'll merge and push from main
        claudeCode: projectConfig.claudeCode,
        subagentModel: projectConfig.defaultModel,
        ...(projectConfig.sandbox && { sandbox: projectConfig.sandbox }),
        reflectionService,
        reflexionConfig: projectConfig.reflexion,
        worktreeGuardHook: createWorktreeGuardHook(worktreeInfo.path),
      };

      // Merge layers to avoid chained Effect.provide
      const combinedLayer = Layer.merge(
        BunContext.layer,
        Layer.provide(openRouterClientLayer, openRouterConfigLayer),
      );

      const state = await Effect.runPromise(
        runOrchestrator(orchestratorConfig, (event) => {
          if (event.type === "session_start") {
            console.log(`  Session started: ${event.sessionId}`);
          } else if (event.type === "task_selected") {
            console.log(`  Task: ${event.task.title}`);
          } else if (event.type === "subtask_start") {
            console.log(`  Subtask: ${event.subtask.description.slice(0, 50)}...`);
          } else if (event.type === "subtask_complete") {
            console.log(`  ✅ Subtask complete`);
          } else if (event.type === "subtask_failed") {
            console.log(`  ❌ Subtask failed: ${event.error}`);
          } else if (event.type === "commit_created") {
            console.log(`  Commit: ${event.sha.slice(0, 8)}`);
          } else if (event.type === "session_complete") {
            console.log(`  ${event.success ? "✅" : "❌"} ${event.summary}`);
          }
        }).pipe(Effect.provide(combinedLayer)),
      );

      result.success = state.phase === "done";
      if (state.task) {
        result.taskId = state.task.id;
      }
      if (state.error) {
        result.error = state.error;
      }
    }

    // Check if worktree has commits to merge
    if (result.success && !dryRun) {
      const hasCommits = await hasCommitsAhead(worktreeInfo.path, targetBranch);

      if (hasCommits) {
        console.log(`\n🔀 Merging changes to ${targetBranch}...`);
        const mergeResult: MergeResult = await mergeBranch(repoPath, {
          targetBranch,
          sourceBranch: worktreeInfo.branch,
        });

        if (mergeResult.success) {
          result.merged = true;
          if (mergeResult.commitSha !== undefined) {
            result.commitSha = mergeResult.commitSha;
          }
          console.log(`  ✅ Merged: ${mergeResult.commitSha?.slice(0, 8)}`);
        } else {
          console.error(`  ❌ Merge failed: ${mergeResult.error}`);
          if (mergeResult.error !== undefined) {
            result.error = mergeResult.error;
          }
        }
      } else {
        console.log("\n  No commits to merge");
      }
    }
  } finally {
    // Cleanup
    console.log("\n🧹 Cleaning up...");

    // Release lock
    releaseWorktreeLock(openagentsDir, taskId);
    console.log("  Released lock");

    // Remove worktree
    await Effect.runPromise(removeWorktree(repoPath, taskId)).catch((e) => {
      console.error(`  Warning: Failed to remove worktree: ${e.message}`);
    });
    console.log("  Removed worktree");
  }

  console.log("\n" + (result.success ? "✅ Success" : "❌ Failed"));
  if (result.error) {
    console.log(`  Error: ${result.error}`);
  }
  console.log();

  return result;
};

// ─────────────────────────────────────────────────────────────────────────────
// CLI
// ─────────────────────────────────────────────────────────────────────────────

async function main() {
  const args = process.argv.slice(2);
  const taskIdIndex = args.indexOf("--task-id");
  const taskId = taskIdIndex !== -1 ? args[taskIdIndex + 1] : undefined;
  const dryRun = args.includes("--dry-run");

  const result = await runInWorktree({
    repoPath: process.cwd(),
    ...(taskId !== undefined ? { taskId } : {}),
    dryRun,
  });

  process.exit(result.success ? 0 : 1);
}

if (import.meta.main) {
  main().catch((error) => {
    console.error("Fatal error:", error);
    process.exit(1);
  });
}
