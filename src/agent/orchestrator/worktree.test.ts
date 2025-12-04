/**
 * Worktree Management Tests
 *
 * Tests for git worktree operations used in parallel agent execution.
 */
import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import { Effect } from "effect";
import * as fs from "node:fs";
import * as path from "node:path";
import {
  createWorktree,
  removeWorktree,
  listWorktrees,
  pruneStaleWorktrees,
  worktreeExists,
  getWorktreePath,
  getBranchName,
  getWorktreesDir,
  validateWorktree,
  repairWorktree,
  ensureValidWorktree,
  type WorktreeConfig,
} from "./worktree.js";

// ─────────────────────────────────────────────────────────────────────────────
// Test Fixtures
// ─────────────────────────────────────────────────────────────────────────────

const TEST_REPO = process.cwd(); // Use current repo for testing
const TEST_TASK_PREFIX = "test-worktree-";
let testTaskId: string;

const generateTestTaskId = () =>
  `${TEST_TASK_PREFIX}${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

const runEffect = <A>(effect: Effect.Effect<A, any, never>): Promise<A> =>
  Effect.runPromise(effect);

// ─────────────────────────────────────────────────────────────────────────────
// Helper Tests
// ─────────────────────────────────────────────────────────────────────────────

describe("Worktree Helpers", () => {
  test("getWorktreesDir returns correct path", () => {
    const dir = getWorktreesDir("/repo/path");
    expect(dir).toBe("/repo/path/.worktrees");
  });

  test("getWorktreePath returns correct path", () => {
    const worktreePath = getWorktreePath("/repo/path", "oa-abc123");
    expect(worktreePath).toBe("/repo/path/.worktrees/oa-abc123");
  });

  test("getBranchName returns correct branch", () => {
    expect(getBranchName("oa-abc123")).toBe("agent/oa-abc123");
    expect(getBranchName("test-task")).toBe("agent/test-task");
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Worktree Lifecycle Tests
// ─────────────────────────────────────────────────────────────────────────────

describe("Worktree Lifecycle", () => {
  beforeEach(() => {
    testTaskId = generateTestTaskId();
  });

  afterEach(async () => {
    // Cleanup: try to remove test worktree if it exists
    if (worktreeExists(TEST_REPO, testTaskId)) {
      await runEffect(removeWorktree(TEST_REPO, testTaskId)).catch(() => {});
    }
    // Also cleanup any stale test worktrees
    const worktreesDir = getWorktreesDir(TEST_REPO);
    if (fs.existsSync(worktreesDir)) {
      const entries = fs.readdirSync(worktreesDir);
      for (const entry of entries) {
        if (entry.startsWith(TEST_TASK_PREFIX)) {
          await runEffect(removeWorktree(TEST_REPO, entry)).catch(() => {});
        }
      }
    }
  });

  test("creates isolated worktree", async () => {
    const config: WorktreeConfig = {
      taskId: testTaskId,
      sessionId: "test-session",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    const info = await runEffect(createWorktree(TEST_REPO, config));

    expect(info.taskId).toBe(testTaskId);
    expect(info.branch).toBe(`agent/${testTaskId}`);
    expect(info.path).toBe(getWorktreePath(TEST_REPO, testTaskId));
    expect(fs.existsSync(info.path)).toBe(true);
    expect(worktreeExists(TEST_REPO, testTaskId)).toBe(true);
  });

  test("worktree has independent working directory", async () => {
    const config: WorktreeConfig = {
      taskId: testTaskId,
      sessionId: "test-session",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    const info = await runEffect(createWorktree(TEST_REPO, config));

    // The worktree should have its own .git file (not directory)
    const gitPath = path.join(info.path, ".git");
    expect(fs.existsSync(gitPath)).toBe(true);

    // .git in a worktree is a file, not a directory
    const gitStat = fs.statSync(gitPath);
    expect(gitStat.isFile()).toBe(true);

    // The worktree should contain project files
    expect(fs.existsSync(path.join(info.path, "package.json"))).toBe(true);
  });

  test("multiple worktrees can exist simultaneously", async () => {
    const taskId1 = generateTestTaskId();
    const taskId2 = generateTestTaskId();

    const config1: WorktreeConfig = {
      taskId: taskId1,
      sessionId: "test-session-1",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    const config2: WorktreeConfig = {
      taskId: taskId2,
      sessionId: "test-session-2",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    try {
      const info1 = await runEffect(createWorktree(TEST_REPO, config1));
      const info2 = await runEffect(createWorktree(TEST_REPO, config2));

      expect(info1.path).not.toBe(info2.path);
      expect(info1.branch).not.toBe(info2.branch);
      expect(fs.existsSync(info1.path)).toBe(true);
      expect(fs.existsSync(info2.path)).toBe(true);
    } finally {
      // Cleanup
      await runEffect(removeWorktree(TEST_REPO, taskId1)).catch(() => {});
      await runEffect(removeWorktree(TEST_REPO, taskId2)).catch(() => {});
    }
  });

  test("cleanup removes worktree and branch", async () => {
    const config: WorktreeConfig = {
      taskId: testTaskId,
      sessionId: "test-session",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    // Create
    const info = await runEffect(createWorktree(TEST_REPO, config));
    expect(fs.existsSync(info.path)).toBe(true);

    // Remove
    await runEffect(removeWorktree(TEST_REPO, testTaskId));

    // Verify removal
    expect(fs.existsSync(info.path)).toBe(false);
    expect(worktreeExists(TEST_REPO, testTaskId)).toBe(false);
  });

  test("fails gracefully when worktree already exists", async () => {
    const config: WorktreeConfig = {
      taskId: testTaskId,
      sessionId: "test-session",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    // Create first worktree
    await runEffect(createWorktree(TEST_REPO, config));

    // Try to create again - should fail
    const result = await Effect.runPromiseExit(createWorktree(TEST_REPO, config));

    expect(result._tag).toBe("Failure");
    if (result._tag === "Failure") {
      const error = result.cause;
      expect(error._tag).toBe("Fail");
    }
  });

  test("cleans up orphan directory before creating worktree", async () => {
    const config: WorktreeConfig = {
      taskId: testTaskId,
      sessionId: "test-session",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    const orphanPath = getWorktreePath(TEST_REPO, testTaskId);
    fs.mkdirSync(orphanPath, { recursive: true });
    fs.writeFileSync(path.join(orphanPath, "orphan.txt"), "orphan");
    expect(fs.existsSync(orphanPath)).toBe(true);

    const info = await runEffect(createWorktree(TEST_REPO, config));

    expect(info.path).toBe(orphanPath);
    expect(fs.existsSync(path.join(info.path, ".git"))).toBe(true);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// List and Prune Tests
// ─────────────────────────────────────────────────────────────────────────────

describe("Worktree List and Prune", () => {
  beforeEach(() => {
    testTaskId = generateTestTaskId();
  });

  afterEach(async () => {
    // Cleanup test worktrees
    const worktreesDir = getWorktreesDir(TEST_REPO);
    if (fs.existsSync(worktreesDir)) {
      const entries = fs.readdirSync(worktreesDir);
      for (const entry of entries) {
        if (entry.startsWith(TEST_TASK_PREFIX)) {
          await runEffect(removeWorktree(TEST_REPO, entry)).catch(() => {});
        }
      }
    }
  });

  test("listWorktrees returns created worktrees", async () => {
    const config: WorktreeConfig = {
      taskId: testTaskId,
      sessionId: "test-session",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    await runEffect(createWorktree(TEST_REPO, config));

    const worktrees = await runEffect(listWorktrees(TEST_REPO));

    const testWorktree = worktrees.find((w) => w.taskId === testTaskId);
    expect(testWorktree).toBeDefined();
    expect(testWorktree?.branch).toBe(`agent/${testTaskId}`);
  });

  test("prune removes stale worktrees", async () => {
    const config: WorktreeConfig = {
      taskId: testTaskId,
      sessionId: "test-session",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    await runEffect(createWorktree(TEST_REPO, config));

    // Set maxAge to 0 to prune immediately
    const pruned = await runEffect(pruneStaleWorktrees(TEST_REPO, 0));

    // Should have pruned the worktree we just created
    expect(pruned).toBeGreaterThanOrEqual(1);
    expect(worktreeExists(TEST_REPO, testTaskId)).toBe(false);
  });

  test("prune removes orphan directories not registered as worktrees", async () => {
    const activeConfig: WorktreeConfig = {
      taskId: testTaskId,
      sessionId: "test-session",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    const activeWorktree = await runEffect(createWorktree(TEST_REPO, activeConfig));

    const orphanId = generateTestTaskId();
    const orphanPath = getWorktreePath(TEST_REPO, orphanId);
    fs.mkdirSync(orphanPath, { recursive: true });
    fs.writeFileSync(path.join(orphanPath, "stale.txt"), "stale");
    expect(fs.existsSync(orphanPath)).toBe(true);

    const pruned = await runEffect(pruneStaleWorktrees(TEST_REPO, 60 * 60 * 1000));

    expect(fs.existsSync(orphanPath)).toBe(false);
    expect(worktreeExists(TEST_REPO, activeWorktree.taskId)).toBe(true);
    expect(pruned).toBeGreaterThanOrEqual(1);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Validation and Repair Tests
// ─────────────────────────────────────────────────────────────────────────────

describe("Worktree Validation", () => {
  beforeEach(() => {
    testTaskId = generateTestTaskId();
  });

  afterEach(async () => {
    // Cleanup test worktrees
    const worktreesDir = getWorktreesDir(TEST_REPO);
    if (fs.existsSync(worktreesDir)) {
      const entries = fs.readdirSync(worktreesDir);
      for (const entry of entries) {
        if (entry.startsWith(TEST_TASK_PREFIX)) {
          await runEffect(removeWorktree(TEST_REPO, entry)).catch(() => {});
        }
      }
    }
  });

  test("validates healthy worktree as valid", async () => {
    const config: WorktreeConfig = {
      taskId: testTaskId,
      sessionId: "test-session",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    await runEffect(createWorktree(TEST_REPO, config));

    const result = await runEffect(validateWorktree(TEST_REPO, testTaskId));

    expect(result.valid).toBe(true);
    expect(result.issues).toHaveLength(0);
  });

  test("detects missing directory", async () => {
    // Don't create the worktree - just validate non-existent task
    const result = await runEffect(validateWorktree(TEST_REPO, testTaskId));

    expect(result.valid).toBe(false);
    expect(result.issues).toHaveLength(1);
    expect(result.issues[0].type).toBe("missing_directory");
  });

  test("detects missing .git file", async () => {
    const config: WorktreeConfig = {
      taskId: testTaskId,
      sessionId: "test-session",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    // Create worktree
    const info = await runEffect(createWorktree(TEST_REPO, config));

    // Corrupt it by removing .git file
    const gitPath = path.join(info.path, ".git");
    fs.unlinkSync(gitPath);

    const result = await runEffect(validateWorktree(TEST_REPO, testTaskId));

    expect(result.valid).toBe(false);
    expect(result.issues.some((i) => i.type === "missing_git")).toBe(true);
  });
});

describe("Worktree Repair", () => {
  beforeEach(() => {
    testTaskId = generateTestTaskId();
  });

  afterEach(async () => {
    // Cleanup test worktrees
    const worktreesDir = getWorktreesDir(TEST_REPO);
    if (fs.existsSync(worktreesDir)) {
      const entries = fs.readdirSync(worktreesDir);
      for (const entry of entries) {
        if (entry.startsWith(TEST_TASK_PREFIX)) {
          await runEffect(removeWorktree(TEST_REPO, entry)).catch(() => {});
        }
      }
    }
  });

  test("repairs corrupted worktree", async () => {
    const config: WorktreeConfig = {
      taskId: testTaskId,
      sessionId: "test-session",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    // Create and corrupt worktree
    const info = await runEffect(createWorktree(TEST_REPO, config));
    const gitPath = path.join(info.path, ".git");
    fs.unlinkSync(gitPath);

    // Verify it's corrupted
    const beforeRepair = await runEffect(validateWorktree(TEST_REPO, testTaskId));
    expect(beforeRepair.valid).toBe(false);

    // Repair it
    const repairedInfo = await runEffect(repairWorktree(TEST_REPO, config));

    // Verify repair worked
    const afterRepair = await runEffect(validateWorktree(TEST_REPO, testTaskId));
    expect(afterRepair.valid).toBe(true);
    expect(fs.existsSync(repairedInfo.path)).toBe(true);
  });

  test("ensureValidWorktree creates missing worktree", async () => {
    const config: WorktreeConfig = {
      taskId: testTaskId,
      sessionId: "test-session",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    // Worktree doesn't exist yet
    expect(worktreeExists(TEST_REPO, testTaskId)).toBe(false);

    // ensureValidWorktree should create it
    const info = await runEffect(ensureValidWorktree(TEST_REPO, config));

    expect(info.taskId).toBe(testTaskId);
    expect(fs.existsSync(info.path)).toBe(true);
    expect(worktreeExists(TEST_REPO, testTaskId)).toBe(true);
  });

  test("ensureValidWorktree returns existing valid worktree", async () => {
    const config: WorktreeConfig = {
      taskId: testTaskId,
      sessionId: "test-session",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    // Create worktree first
    const originalInfo = await runEffect(createWorktree(TEST_REPO, config));

    // ensureValidWorktree should return existing
    const info = await runEffect(ensureValidWorktree(TEST_REPO, config));

    expect(info.taskId).toBe(testTaskId);
    expect(info.path).toBe(originalInfo.path);
  });

  test("ensureValidWorktree repairs corrupted worktree", async () => {
    const config: WorktreeConfig = {
      taskId: testTaskId,
      sessionId: "test-session",
      baseBranch: "main",
      timeoutMs: 30000,
    };

    // Create and corrupt worktree
    const originalInfo = await runEffect(createWorktree(TEST_REPO, config));
    fs.unlinkSync(path.join(originalInfo.path, ".git"));

    // ensureValidWorktree should repair it
    const info = await runEffect(ensureValidWorktree(TEST_REPO, config));

    // Verify worktree is now valid
    const validation = await runEffect(validateWorktree(TEST_REPO, testTaskId));
    expect(validation.valid).toBe(true);
    expect(fs.existsSync(info.path)).toBe(true);
  });
});
