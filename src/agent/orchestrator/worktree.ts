/**
 * Git Worktree Management Service
 *
 * Manages isolated git worktrees for parallel agent execution.
 * Each agent gets its own worktree with a unique branch.
 *
 * Directory structure:
 * ```
 * repo/
 * ├── .git/                    # Shared object database
 * ├── .worktrees/              # Agent worktrees
 * │   ├── oa-abc123/           # Worktree for task oa-abc123
 * │   ├── oa-def456/           # Worktree for task oa-def456
 * │   └── ...
 * └── [main working tree]
 * ```
 *
 * @see docs/claude/plans/containers-impl-v2.md
 */
import * as fs from "node:fs";
import * as path from "node:path";
import { Effect } from "effect";
import * as S from "effect/Schema";

// ─────────────────────────────────────────────────────────────────────────────
// Schema
// ─────────────────────────────────────────────────────────────────────────────

export const WorktreeConfigSchema = S.Struct({
  taskId: S.String,
  sessionId: S.String,
  baseBranch: S.optionalWith(S.String, { default: () => "main" }),
  timeoutMs: S.optionalWith(S.Number, { default: () => 30 * 60 * 1000 }), // 30min
});

export type WorktreeConfig = S.Schema.Type<typeof WorktreeConfigSchema>;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

export interface WorktreeInfo {
  taskId: string;
  path: string; // Absolute path to worktree
  branch: string; // agent/{taskId}
  createdAt: string;
  baseBranch: string;
}

export class WorktreeError extends Error {
  readonly _tag = "WorktreeError";
  constructor(
    readonly reason:
      | "create_failed"
      | "remove_failed"
      | "list_failed"
      | "prune_failed"
      | "not_found"
      | "already_exists"
      | "git_error"
      | "validation_failed"
      | "repair_failed",
    message: string,
    override readonly cause?: unknown,
  ) {
    super(message);
    this.name = "WorktreeError";
  }
}

export interface WorktreeValidationResult {
  valid: boolean;
  issues: WorktreeIssue[];
}

export type WorktreeIssue =
  | { type: "missing_git"; message: string }
  | { type: "branch_mismatch"; expected: string; actual: string }
  | { type: "detached_head"; message: string }
  | { type: "missing_directory"; message: string };

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const WORKTREES_DIR = ".worktrees";
const BRANCH_PREFIX = "agent/";

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Run a git command and return the result
 */
const runGit = (
  repoPath: string,
  args: string[],
): Effect.Effect<{ exitCode: number; stdout: string; stderr: string }, WorktreeError> =>
  Effect.tryPromise({
    try: async () => {
      const proc = Bun.spawn(["git", ...args], {
        cwd: repoPath,
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
    catch: (e) => new WorktreeError("git_error", `Git command failed: ${e}`, e),
  });

/**
 * Get the worktrees directory path
 */
export const getWorktreesDir = (repoPath: string): string =>
  path.join(repoPath, WORKTREES_DIR);

/**
 * Get the path for a specific worktree
 */
export const getWorktreePath = (repoPath: string, taskId: string): string =>
  path.join(getWorktreesDir(repoPath), taskId);

/**
 * Get the branch name for a task
 */
export const getBranchName = (taskId: string): string => `${BRANCH_PREFIX}${taskId}`;

// ─────────────────────────────────────────────────────────────────────────────
// Core Operations
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Create a new worktree for a task.
 *
 * Creates a worktree at .worktrees/{taskId} with branch agent/{taskId}
 * based on the specified base branch.
 */
export const createWorktree = (
  repoPath: string,
  config: WorktreeConfig,
): Effect.Effect<WorktreeInfo, WorktreeError> =>
  Effect.gen(function* () {
    const worktreePath = getWorktreePath(repoPath, config.taskId);
    const branchName = getBranchName(config.taskId);

    // Check if worktree directory already exists (could be orphaned)
    if (fs.existsSync(worktreePath)) {
      const existingWorktrees = yield* listWorktrees(repoPath);
      const isRegistered = existingWorktrees.some(
        (wt) => path.resolve(wt.path) === path.resolve(worktreePath),
      );

      if (isRegistered) {
        return yield* Effect.fail(
          new WorktreeError(
            "already_exists",
            `Worktree already exists at ${worktreePath}`,
          ),
        );
      }

      // Orphaned directory leftover from a previous run - clean it up first
      fs.rmSync(worktreePath, { recursive: true, force: true });
    }

    // Ensure .worktrees directory exists
    const worktreesDir = getWorktreesDir(repoPath);
    if (!fs.existsSync(worktreesDir)) {
      fs.mkdirSync(worktreesDir, { recursive: true });
    }

    // Fetch latest from remote to ensure base branch is up to date
    yield* runGit(repoPath, ["fetch", "origin", config.baseBranch]).pipe(
      Effect.catchAll(() => Effect.succeed({ exitCode: 0, stdout: "", stderr: "" })),
    );

    // Create worktree with new branch
    // git worktree add -b agent/{taskId} .worktrees/{taskId} origin/{baseBranch}
    const result = yield* runGit(repoPath, [
      "worktree",
      "add",
      "-b",
      branchName,
      worktreePath,
      `origin/${config.baseBranch}`,
    ]);

    if (result.exitCode !== 0) {
      // If branch already exists, try without -b flag
      if (result.stderr.includes("already exists")) {
        const retryResult = yield* runGit(repoPath, [
          "worktree",
          "add",
          worktreePath,
          branchName,
        ]);
        if (retryResult.exitCode !== 0) {
          return yield* Effect.fail(
            new WorktreeError(
              "create_failed",
              `Failed to create worktree: ${retryResult.stderr}`,
            ),
          );
        }
      } else {
        return yield* Effect.fail(
          new WorktreeError(
            "create_failed",
            `Failed to create worktree: ${result.stderr}`,
          ),
        );
      }
    }

    return {
      taskId: config.taskId,
      path: worktreePath,
      branch: branchName,
      createdAt: new Date().toISOString(),
      baseBranch: config.baseBranch,
    } satisfies WorktreeInfo;
  });

/**
 * Remove a worktree and its branch.
 */
export const removeWorktree = (
  repoPath: string,
  taskId: string,
): Effect.Effect<void, WorktreeError> =>
  Effect.gen(function* () {
    const worktreePath = getWorktreePath(repoPath, taskId);
    const branchName = getBranchName(taskId);

    // Remove worktree (--force to handle uncommitted changes)
    const removeResult = yield* runGit(repoPath, [
      "worktree",
      "remove",
      "--force",
      worktreePath,
    ]);

    if (removeResult.exitCode !== 0 && !removeResult.stderr.includes("is not a working tree")) {
      return yield* Effect.fail(
        new WorktreeError(
          "remove_failed",
          `Failed to remove worktree: ${removeResult.stderr}`,
        ),
      );
    }

    // Delete the branch
    const branchResult = yield* runGit(repoPath, ["branch", "-D", branchName]);

    // Branch deletion failure is not fatal (branch may not exist)
    if (branchResult.exitCode !== 0 && !branchResult.stderr.includes("not found")) {
      // Log but don't fail
      console.warn(`Warning: Could not delete branch ${branchName}: ${branchResult.stderr}`);
    }

    // Prune worktree entries
    yield* runGit(repoPath, ["worktree", "prune"]);
  });

/**
 * List all worktrees in the repository.
 */
export const listWorktrees = (
  repoPath: string,
): Effect.Effect<WorktreeInfo[], WorktreeError> =>
  Effect.gen(function* () {
    const result = yield* runGit(repoPath, ["worktree", "list", "--porcelain"]);

    if (result.exitCode !== 0) {
      return yield* Effect.fail(
        new WorktreeError("list_failed", `Failed to list worktrees: ${result.stderr}`),
      );
    }

    const worktrees: WorktreeInfo[] = [];
    const entries = result.stdout.split("\n\n").filter(Boolean);

    for (const entry of entries) {
      const lines = entry.split("\n");
      let worktreePath = "";
      let branch = "";

      for (const line of lines) {
        if (line.startsWith("worktree ")) {
          worktreePath = line.substring(9);
        } else if (line.startsWith("branch refs/heads/")) {
          branch = line.substring(18);
        }
      }

      // Only include agent worktrees (those in .worktrees/ with agent/ branch)
      if (
        worktreePath.includes(WORKTREES_DIR) &&
        branch.startsWith(BRANCH_PREFIX)
      ) {
        const taskId = branch.substring(BRANCH_PREFIX.length);
        worktrees.push({
          taskId,
          path: worktreePath,
          branch,
          createdAt: "", // Not available from git worktree list
          baseBranch: "main", // Would need to inspect branch for actual base
        });
      }
    }

    return worktrees;
  });

/**
 * Prune stale worktrees (orphaned entries and old worktrees).
 *
 * @param repoPath - Path to the repository
 * @param maxAgeMs - Maximum age in milliseconds (worktrees older than this are removed)
 * @returns Number of worktrees pruned
 */
export const pruneStaleWorktrees = (
  repoPath: string,
  maxAgeMs: number,
): Effect.Effect<number, WorktreeError> =>
  Effect.gen(function* () {
    // First, run git worktree prune to clean up orphaned entries
    yield* runGit(repoPath, ["worktree", "prune"]);

    // Get all worktrees
    const worktrees = yield* listWorktrees(repoPath);
    let pruned = 0;

    // Remove orphaned directories that aren't registered worktrees
    const registeredPaths = new Set(worktrees.map((wt) => path.resolve(wt.path)));
    const worktreesDir = getWorktreesDir(repoPath);
    if (fs.existsSync(worktreesDir)) {
      const entries = fs.readdirSync(worktreesDir, { withFileTypes: true });
      for (const entry of entries) {
        if (!entry.isDirectory()) continue;
        const candidatePath = path.resolve(path.join(worktreesDir, entry.name));
        if (!registeredPaths.has(candidatePath)) {
          try {
            fs.rmSync(candidatePath, { recursive: true, force: true });
            pruned++;
          } catch {
            // Ignore failure; best-effort cleanup
          }
        }
      }
    }

    // Check each worktree for staleness
    for (const worktree of worktrees) {
      const worktreePath = worktree.path;

      // Check if worktree directory exists
      if (!fs.existsSync(worktreePath)) {
        // Already orphaned, prune will have handled it
        continue;
      }

      // Check modification time of the worktree
      try {
        const stats = fs.statSync(worktreePath);
        const age = Date.now() - stats.mtimeMs;

        if (age > maxAgeMs) {
          yield* removeWorktree(repoPath, worktree.taskId).pipe(
            Effect.catchAll(() => Effect.succeed(undefined)),
          );
          pruned++;
        }
      } catch {
        // If we can't stat, try to remove anyway
        yield* removeWorktree(repoPath, worktree.taskId).pipe(
          Effect.catchAll(() => Effect.succeed(undefined)),
        );
        pruned++;
      }
    }

    return pruned;
  });

/**
 * Check if a worktree exists for a task.
 */
export const worktreeExists = (repoPath: string, taskId: string): boolean => {
  const worktreePath = getWorktreePath(repoPath, taskId);
  return fs.existsSync(worktreePath);
};

/**
 * Get info about a specific worktree.
 */
export const getWorktreeInfo = (
  repoPath: string,
  taskId: string,
): Effect.Effect<WorktreeInfo, WorktreeError> =>
  Effect.gen(function* () {
    const worktreePath = getWorktreePath(repoPath, taskId);
    const branchName = getBranchName(taskId);

    if (!fs.existsSync(worktreePath)) {
      return yield* Effect.fail(
        new WorktreeError("not_found", `Worktree not found for task ${taskId}`),
      );
    }

    // Get the current branch in the worktree
    const result = yield* runGit(worktreePath, ["rev-parse", "--abbrev-ref", "HEAD"]);

    return {
      taskId,
      path: worktreePath,
      branch: result.exitCode === 0 ? result.stdout : branchName,
      createdAt: "",
      baseBranch: "main",
    } satisfies WorktreeInfo;
  });

// ─────────────────────────────────────────────────────────────────────────────
// Validation and Repair
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Validate a worktree for corruption issues.
 *
 * Checks for:
 * - Missing .git file (indicates worktree was partially deleted)
 * - Branch mismatch (worktree on wrong branch)
 * - Detached HEAD state
 * - Missing worktree directory
 */
export const validateWorktree = (
  repoPath: string,
  taskId: string,
): Effect.Effect<WorktreeValidationResult, WorktreeError> =>
  Effect.gen(function* () {
    const worktreePath = getWorktreePath(repoPath, taskId);
    const expectedBranch = getBranchName(taskId);
    const issues: WorktreeIssue[] = [];

    // Check if worktree directory exists
    if (!fs.existsSync(worktreePath)) {
      issues.push({
        type: "missing_directory",
        message: `Worktree directory not found at ${worktreePath}`,
      });
      return { valid: false, issues };
    }

    // Check for .git file (worktrees have a .git file, not directory)
    const gitPath = path.join(worktreePath, ".git");
    if (!fs.existsSync(gitPath)) {
      issues.push({
        type: "missing_git",
        message: `Missing .git file in worktree at ${worktreePath}`,
      });
      return { valid: false, issues };
    }

    // Check if .git is a file (valid worktree) not a directory
    const gitStat = fs.statSync(gitPath);
    if (!gitStat.isFile()) {
      issues.push({
        type: "missing_git",
        message: `.git is a directory instead of file at ${worktreePath} - not a valid worktree`,
      });
      return { valid: false, issues };
    }

    // Check current branch
    const branchResult = yield* runGit(worktreePath, [
      "rev-parse",
      "--abbrev-ref",
      "HEAD",
    ]);

    if (branchResult.exitCode !== 0) {
      issues.push({
        type: "missing_git",
        message: `Git error in worktree: ${branchResult.stderr}`,
      });
      return { valid: false, issues };
    }

    const currentBranch = branchResult.stdout;

    // Check for detached HEAD
    if (currentBranch === "HEAD") {
      issues.push({
        type: "detached_head",
        message: `Worktree is in detached HEAD state`,
      });
    }

    // Check for branch mismatch
    if (currentBranch !== expectedBranch && currentBranch !== "HEAD") {
      issues.push({
        type: "branch_mismatch",
        expected: expectedBranch,
        actual: currentBranch,
      });
    }

    return { valid: issues.length === 0, issues };
  });

/**
 * Repair a corrupted worktree by removing and recreating it.
 *
 * This is a destructive operation - any uncommitted changes in the
 * worktree will be lost.
 */
export const repairWorktree = (
  repoPath: string,
  config: WorktreeConfig,
): Effect.Effect<WorktreeInfo, WorktreeError> =>
  Effect.gen(function* () {
    const worktreePath = getWorktreePath(repoPath, config.taskId);
    const branchName = getBranchName(config.taskId);

    console.log(`[worktree] Repairing worktree for task ${config.taskId}...`);

    // Step 1: Try to cleanly remove existing worktree
    const removeResult = yield* runGit(repoPath, [
      "worktree",
      "remove",
      "--force",
      worktreePath,
    ]).pipe(Effect.catchAll(() => Effect.succeed({ exitCode: 1, stdout: "", stderr: "" })));

    // If git worktree remove failed, manually clean up
    if (removeResult.exitCode !== 0 && fs.existsSync(worktreePath)) {
      try {
        fs.rmSync(worktreePath, { recursive: true, force: true });
      } catch (e) {
        console.warn(`[worktree] Warning: Could not remove directory ${worktreePath}`);
      }
    }

    // Step 2: Prune stale worktree entries
    yield* runGit(repoPath, ["worktree", "prune"]);

    // Step 3: Try to delete the branch (may not exist or may be current elsewhere)
    yield* runGit(repoPath, ["branch", "-D", branchName]).pipe(
      Effect.catchAll(() => Effect.succeed({ exitCode: 0, stdout: "", stderr: "" })),
    );

    // Step 4: Recreate the worktree
    const newWorktree = yield* createWorktree(repoPath, config).pipe(
      Effect.mapError(
        (e) =>
          new WorktreeError(
            "repair_failed",
            `Failed to recreate worktree after repair: ${e.message}`,
            e,
          ),
      ),
    );

    console.log(`[worktree] Worktree repaired successfully at ${newWorktree.path}`);
    return newWorktree;
  });

/**
 * Validate worktree and repair if necessary.
 *
 * Returns the validated (and possibly repaired) worktree info.
 * Use this before running the orchestrator to ensure worktree health.
 */
export const ensureValidWorktree = (
  repoPath: string,
  config: WorktreeConfig,
): Effect.Effect<WorktreeInfo, WorktreeError> =>
  Effect.gen(function* () {
    const worktreePath = getWorktreePath(repoPath, config.taskId);

    // If worktree doesn't exist, create it
    if (!fs.existsSync(worktreePath)) {
      return yield* createWorktree(repoPath, config);
    }

    // Validate existing worktree
    const validation = yield* validateWorktree(repoPath, config.taskId);

    if (validation.valid) {
      // Worktree is healthy, return its info
      return yield* getWorktreeInfo(repoPath, config.taskId);
    }

    // Log issues and repair
    console.log(`[worktree] Validation issues for ${config.taskId}:`);
    for (const issue of validation.issues) {
      console.log(`  - ${issue.type}: ${JSON.stringify(issue)}`);
    }

    return yield* repairWorktree(repoPath, config);
  });

// ─────────────────────────────────────────────────────────────────────────────
// Effect-free versions for compatibility
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Run createWorktree and return a Promise
 */
export const createWorktreeAsync = (
  repoPath: string,
  config: WorktreeConfig,
): Promise<WorktreeInfo> => Effect.runPromise(createWorktree(repoPath, config));

/**
 * Run removeWorktree and return a Promise
 */
export const removeWorktreeAsync = (
  repoPath: string,
  taskId: string,
): Promise<void> => Effect.runPromise(removeWorktree(repoPath, taskId));

/**
 * Run listWorktrees and return a Promise
 */
export const listWorktreesAsync = (repoPath: string): Promise<WorktreeInfo[]> =>
  Effect.runPromise(listWorktrees(repoPath));

/**
 * Run pruneStaleWorktrees and return a Promise
 */
export const pruneStaleWorktreesAsync = (
  repoPath: string,
  maxAgeMs: number,
): Promise<number> => Effect.runPromise(pruneStaleWorktrees(repoPath, maxAgeMs));

/**
 * Run validateWorktree and return a Promise
 */
export const validateWorktreeAsync = (
  repoPath: string,
  taskId: string,
): Promise<WorktreeValidationResult> =>
  Effect.runPromise(validateWorktree(repoPath, taskId));

/**
 * Run repairWorktree and return a Promise
 */
export const repairWorktreeAsync = (
  repoPath: string,
  config: WorktreeConfig,
): Promise<WorktreeInfo> => Effect.runPromise(repairWorktree(repoPath, config));

/**
 * Run ensureValidWorktree and return a Promise
 */
export const ensureValidWorktreeAsync = (
  repoPath: string,
  config: WorktreeConfig,
): Promise<WorktreeInfo> => Effect.runPromise(ensureValidWorktree(repoPath, config));
