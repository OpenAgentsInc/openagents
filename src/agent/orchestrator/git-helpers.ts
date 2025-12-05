/**
 * Git Helper Functions
 *
 * Composable git operations for worktree and parallel runners.
 * These helpers are designed to be reused across different runner contexts.
 */
import { Effect } from "effect";

export interface GitResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

export class GitError extends Error {
  readonly _tag = "GitError";
  constructor(
    readonly reason:
      | "command_failed"
      | "merge_failed"
      | "checkout_failed"
      | "push_failed"
      | "pull_failed"
      | "fetch_failed"
      | "dirty_worktree",
    message: string,
    override readonly cause?: unknown,
  ) {
    super(message);
    this.name = "GitError";
  }
}

export interface MergeOptions {
  targetBranch: string;
  sourceBranch: string;
  push?: boolean;
}

export interface MergeResult {
  success: boolean;
  commitSha?: string;
  error?: string;
}

export const runGit = async (cwd: string, args: string[]): Promise<GitResult> => {
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

export const runGitEffect = (
  cwd: string,
  args: string[],
): Effect.Effect<GitResult, GitError> =>
  Effect.tryPromise({
    try: async () => {
      const result = await runGit(cwd, args);
      if (result.exitCode !== 0) {
        throw new Error(result.stderr || `Git command failed: git ${args.join(" ")}`);
      }
      return result;
    },
    catch: (e) => new GitError("command_failed", `Git command failed: ${e}`, e),
  });

export const isWorkingTreeDirty = async (cwd: string): Promise<boolean> => {
  const result = await runGit(cwd, ["status", "--porcelain"]);
  return result.stdout.length > 0;
};

export const getHeadSha = async (cwd: string): Promise<string> => {
  const result = await runGit(cwd, ["rev-parse", "HEAD"]);
  if (result.exitCode !== 0) {
    throw new GitError("command_failed", `Failed to get HEAD: ${result.stderr}`);
  }
  return result.stdout;
};

export const getCurrentBranch = async (cwd: string): Promise<string> => {
  const result = await runGit(cwd, ["rev-parse", "--abbrev-ref", "HEAD"]);
  if (result.exitCode !== 0) {
    throw new GitError("command_failed", `Failed to get current branch: ${result.stderr}`);
  }
  return result.stdout;
};

export const hasCommitsAhead = async (
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

export const fetchBranch = async (cwd: string, branch: string, remote = "origin"): Promise<void> => {
  const result = await runGit(cwd, ["fetch", remote, branch]);
  if (result.exitCode !== 0 && !result.stderr.includes("couldn't find remote ref")) {
    throw new GitError("fetch_failed", `Failed to fetch ${remote}/${branch}: ${result.stderr}`);
  }
};

export const pullBranch = async (cwd: string, branch: string, remote = "origin"): Promise<void> => {
  let result = await runGit(cwd, ["pull", "--ff-only", remote, branch]);
  if (result.exitCode !== 0) {
    result = await runGit(cwd, ["pull", remote, branch]);
    if (result.exitCode !== 0) {
      throw new GitError("pull_failed", `Failed to pull ${remote}/${branch}: ${result.stderr}`);
    }
  }
};

export const checkoutBranch = async (cwd: string, branch: string): Promise<void> => {
  const result = await runGit(cwd, ["checkout", branch]);
  if (result.exitCode !== 0) {
    throw new GitError("checkout_failed", `Failed to checkout ${branch}: ${result.stderr}`);
  }
};

export const pushBranch = async (cwd: string, branch: string, remote = "origin"): Promise<void> => {
  const result = await runGit(cwd, ["push", remote, branch]);
  if (result.exitCode !== 0) {
    throw new GitError("push_failed", `Failed to push to ${remote}/${branch}: ${result.stderr}`);
  }
};

export const mergeBranch = async (
  repoPath: string,
  options: MergeOptions,
): Promise<MergeResult> => {
  const { targetBranch, sourceBranch, push = true } = options;

  try {
    let result = await runGit(repoPath, ["checkout", targetBranch]);
    if (result.exitCode !== 0) {
      return { success: false, error: `Checkout ${targetBranch} failed: ${result.stderr}` };
    }

    result = await runGit(repoPath, ["pull", "--ff-only", "origin", targetBranch]);
    if (result.exitCode !== 0) {
      result = await runGit(repoPath, ["pull", "origin", targetBranch]);
      if (result.exitCode !== 0) {
        return { success: false, error: `Pull failed: ${result.stderr}` };
      }
    }

    result = await runGit(repoPath, ["merge", "--ff-only", sourceBranch]);
    if (result.exitCode !== 0) {
      result = await runGit(repoPath, ["merge", "--no-edit", sourceBranch]);
      if (result.exitCode !== 0) {
        return { success: false, error: `Merge failed: ${result.stderr}` };
      }
    }

    result = await runGit(repoPath, ["rev-parse", "HEAD"]);
    const commitSha = result.stdout;

    if (push) {
      result = await runGit(repoPath, ["push", "origin", targetBranch]);
      if (result.exitCode !== 0) {
        return { success: false, error: `Push failed: ${result.stderr}` };
      }
    }

    return { success: true, commitSha };
  } catch (error: any) {
    return { success: false, error: error.message };
  }
};

export const abortMergeState = async (repoPath: string, resetToSha?: string): Promise<void> => {
  await runGit(repoPath, ["merge", "--abort"]).catch(() => {});
  await runGit(repoPath, ["rebase", "--abort"]).catch(() => {});
  if (resetToSha) {
    await runGit(repoPath, ["reset", "--hard", resetToSha]).catch(() => {});
  }
};
