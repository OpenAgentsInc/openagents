/**
 * Rewind Uncommitted Changes Spell
 *
 * Restores the repository to a clean state by reverting all uncommitted changes.
 * This is a safe operation that can help recover from broken code states.
 */
import { Effect } from "effect";
import type { HealerSpell, HealerSpellResult, HealerContext } from "../types.js";

/**
 * Execute git commands to rewind uncommitted changes.
 */
const executeGitRewind = async (
  cwd: string
): Promise<{ success: boolean; output: string; filesReverted: string[] }> => {
  const filesReverted: string[] = [];
  let output = "";

  try {
    // Get list of modified files before reverting
    const statusProc = Bun.spawn(["git", "status", "--porcelain"], { cwd });
    const statusOutput = await new Response(statusProc.stdout).text();
    await statusProc.exited;

    // Parse modified files
    const lines = statusOutput.trim().split("\n").filter(Boolean);
    for (const line of lines) {
      const file = line.slice(3).trim();
      if (file) filesReverted.push(file);
    }

    if (filesReverted.length === 0) {
      return {
        success: true,
        output: "No uncommitted changes to revert",
        filesReverted: [],
      };
    }

    // Restore tracked files
    const restoreProc = Bun.spawn(["git", "restore", "."], { cwd });
    const restoreOutput = await new Response(restoreProc.stdout).text();
    const restoreExitCode = await restoreProc.exited;
    output += restoreOutput;

    if (restoreExitCode !== 0) {
      const stderr = await new Response(restoreProc.stderr).text();
      return {
        success: false,
        output: `git restore failed: ${stderr}`,
        filesReverted: [],
      };
    }

    // Clean untracked files
    const cleanProc = Bun.spawn(["git", "clean", "-fd"], { cwd });
    const cleanOutput = await new Response(cleanProc.stdout).text();
    const cleanExitCode = await cleanProc.exited;
    output += cleanOutput;

    if (cleanExitCode !== 0) {
      const stderr = await new Response(cleanProc.stderr).text();
      return {
        success: false,
        output: `git clean failed: ${stderr}`,
        filesReverted,
      };
    }

    return {
      success: true,
      output: `Reverted ${filesReverted.length} files`,
      filesReverted,
    };
  } catch (error) {
    return {
      success: false,
      output: `Error executing git commands: ${error}`,
      filesReverted: [],
    };
  }
};

/**
 * Rewind Uncommitted Changes spell.
 *
 * Executes:
 * 1. git restore . - Revert all tracked file changes
 * 2. git clean -fd - Remove untracked files and directories
 *
 * Local-context: runs in-process and must be safe/idempotent because it cannot be suspended mid-call.
 */
export const rewindUncommittedChanges: HealerSpell = {
  id: "rewind_uncommitted_changes",
  description: "Restore repo to clean state since last commit",
  requiresLLM: false,

  apply: (ctx: HealerContext): Effect.Effect<HealerSpellResult, Error, never> =>
    Effect.gen(function* () {
      // Check if repo is dirty
      if (!ctx.gitStatus.isDirty) {
        return {
          success: true,
          changesApplied: false,
          summary: "Repository is already clean, no changes to revert",
        };
      }

      const modifiedCount = ctx.gitStatus.modifiedFiles.length;
      const untrackedCount = ctx.gitStatus.untrackedFiles.length;

      // Execute git rewind
      const result = yield* Effect.tryPromise({
        try: () => executeGitRewind(ctx.projectRoot),
        catch: (error) => new Error(`Git rewind failed: ${error}`),
      });

      if (!result.success) {
        return {
          success: false,
          changesApplied: false,
          summary: `Failed to rewind changes: ${result.output}`,
          error: result.output,
        };
      }

      return {
        success: true,
        changesApplied: true,
        summary: `Reverted ${modifiedCount} modified and ${untrackedCount} untracked files`,
        filesModified: result.filesReverted,
      };
    }),
};
