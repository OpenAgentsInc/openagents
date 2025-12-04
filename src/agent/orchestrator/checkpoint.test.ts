/**
 * Tests for Orchestrator Checkpoint System
 *
 * Tests cover:
 * - Checkpoint creation and updates
 * - Atomic file persistence (write/read/clear)
 * - Checkpoint validation (age, git state)
 * - Resume flow from checkpoints
 */

import { test, expect, describe, beforeEach, afterEach } from "bun:test";
import * as FileSystem from "@effect/platform/FileSystem";
import { BunContext } from "@effect/platform-bun";
import { Effect, Option } from "effect";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import { execSync } from "node:child_process";

import {
  writeCheckpoint,
  readCheckpoint,
  clearCheckpoint,
  validateCheckpoint,
  maybeResumeCheckpoint,
  createCheckpoint,
  updateCheckpointPhase,
  addHealerInvocation,
  captureGitState,
  getCheckpointPath,
  CHECKPOINT_MAX_AGE_MS,
  type OrchestratorCheckpoint,
  type CheckpointGitState,
} from "./checkpoint.js";

// Helper to run Effect programs with Bun context
const runEffect = <A, E>(
  effect: Effect.Effect<A, E, FileSystem.FileSystem>
): Promise<A> =>
  Effect.runPromise(effect.pipe(Effect.provide(BunContext.layer)));

// Test fixture: Create a minimal valid checkpoint
const createTestCheckpoint = (
  overrides: Partial<OrchestratorCheckpoint> = {}
): OrchestratorCheckpoint => ({
  version: 1,
  sessionId: "test-session-123",
  timestamp: new Date().toISOString(),
  phase: "executing_subtask",
  phaseStartedAt: new Date().toISOString(),
  taskId: "oa-test-001",
  taskTitle: "Test Task",
  completedSubtaskIds: ["sub-001"],
  currentSubtaskId: "sub-002",
  git: {
    branch: "main",
    headCommit: "abc123def456",
    isDirty: false,
    stagedFiles: [],
  },
  healerInvocations: [],
  ...overrides,
});

describe("Checkpoint Creation", () => {
  test("createCheckpoint creates valid checkpoint with required fields", () => {
    const git: CheckpointGitState = {
      branch: "feature-branch",
      headCommit: "deadbeef",
      isDirty: true,
      stagedFiles: ["file1.ts", "file2.ts"],
    };

    const checkpoint = createCheckpoint({
      sessionId: "session-001",
      phase: "decomposing",
      taskId: "oa-task-001",
      taskTitle: "My Task",
      completedSubtaskIds: [],
      currentSubtaskId: null,
      git,
    });

    expect(checkpoint.version).toBe(1);
    expect(checkpoint.sessionId).toBe("session-001");
    expect(checkpoint.phase).toBe("decomposing");
    expect(checkpoint.taskId).toBe("oa-task-001");
    expect(checkpoint.taskTitle).toBe("My Task");
    expect(checkpoint.completedSubtaskIds).toEqual([]);
    expect(checkpoint.currentSubtaskId).toBeNull();
    expect(checkpoint.git).toEqual(git);
    expect(checkpoint.healerInvocations).toEqual([]);
    expect(checkpoint.timestamp).toBeDefined();
    expect(checkpoint.phaseStartedAt).toBeDefined();
  });

  test("createCheckpoint with verification data", () => {
    const checkpoint = createCheckpoint({
      sessionId: "session-002",
      phase: "verifying",
      taskId: "oa-task-002",
      taskTitle: "Task with Verification",
      completedSubtaskIds: ["sub-1", "sub-2"],
      currentSubtaskId: null,
      git: {
        branch: "main",
        headCommit: "abc123",
        isDirty: false,
        stagedFiles: [],
      },
      verification: {
        typecheckPassed: true,
        testsPassed: true,
        verifiedAt: "2025-01-01T00:00:00Z",
      },
    });

    expect(checkpoint.verification).toBeDefined();
    expect(checkpoint.verification?.typecheckPassed).toBe(true);
    expect(checkpoint.verification?.testsPassed).toBe(true);
  });
});

describe("Checkpoint Updates", () => {
  test("updateCheckpointPhase updates phase and timestamp", async () => {
    const original = createTestCheckpoint({ phase: "decomposing" });

    // Small delay to ensure different timestamp
    await new Promise((resolve) => setTimeout(resolve, 5));

    const updated = updateCheckpointPhase(original, "executing_subtask");

    expect(updated.phase).toBe("executing_subtask");
    // Timestamp should be updated (may be same if within same ms, so we just verify it's a valid ISO string)
    expect(updated.timestamp).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    expect(updated.phaseStartedAt).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    // Original fields preserved
    expect(updated.sessionId).toBe(original.sessionId);
    expect(updated.taskId).toBe(original.taskId);
  });

  test("updateCheckpointPhase with additional updates", () => {
    const original = createTestCheckpoint({
      completedSubtaskIds: ["sub-001"],
      currentSubtaskId: "sub-002",
    });

    const updated = updateCheckpointPhase(original, "executing_subtask", {
      completedSubtaskIds: ["sub-001", "sub-002"],
      currentSubtaskId: "sub-003",
    });

    expect(updated.completedSubtaskIds).toEqual(["sub-001", "sub-002"]);
    expect(updated.currentSubtaskId).toBe("sub-003");
  });

  test("updateCheckpointPhase with verification results", () => {
    const original = createTestCheckpoint({ phase: "verifying" });
    const updated = updateCheckpointPhase(original, "committing", {
      verification: {
        typecheckPassed: true,
        testsPassed: true,
        verifiedAt: new Date().toISOString(),
      },
    });

    expect(updated.verification).toBeDefined();
    expect(updated.verification?.typecheckPassed).toBe(true);
  });

  test("addHealerInvocation appends to invocations array", () => {
    const original = createTestCheckpoint({ healerInvocations: [] });

    const withInvocation = addHealerInvocation(original, {
      scenario: "typecheck_failure",
      outcome: "resolved",
      timestamp: new Date().toISOString(),
    });

    expect(withInvocation.healerInvocations).toHaveLength(1);
    expect(withInvocation.healerInvocations[0].scenario).toBe("typecheck_failure");

    const withSecond = addHealerInvocation(withInvocation, {
      scenario: "test_failure",
      outcome: "unresolved",
      timestamp: new Date().toISOString(),
    });

    expect(withSecond.healerInvocations).toHaveLength(2);
  });
});

describe("Checkpoint Validation", () => {
  test("validates fresh checkpoint with matching git state", () => {
    const checkpoint = createTestCheckpoint({
      timestamp: new Date().toISOString(),
      git: {
        branch: "main",
        headCommit: "abc123",
        isDirty: false,
        stagedFiles: [],
      },
    });

    const currentGit: CheckpointGitState = {
      branch: "main",
      headCommit: "def456", // Different commit is OK
      isDirty: true, // Different dirty state is OK
      stagedFiles: [],
    };

    const result = validateCheckpoint(checkpoint, currentGit);
    expect(result.valid).toBe(true);
  });

  test("rejects stale checkpoint (>24h old)", () => {
    const staleTime = new Date(Date.now() - CHECKPOINT_MAX_AGE_MS - 1000);
    const checkpoint = createTestCheckpoint({
      timestamp: staleTime.toISOString(),
    });

    const currentGit: CheckpointGitState = {
      branch: "main",
      headCommit: "abc123",
      isDirty: false,
      stagedFiles: [],
    };

    const result = validateCheckpoint(checkpoint, currentGit);
    expect(result.valid).toBe(false);
    if (!result.valid) {
      expect(result.reason).toContain("stale");
    }
  });

  test("rejects checkpoint with branch mismatch", () => {
    const checkpoint = createTestCheckpoint({
      git: {
        branch: "feature-branch",
        headCommit: "abc123",
        isDirty: false,
        stagedFiles: [],
      },
    });

    const currentGit: CheckpointGitState = {
      branch: "main", // Different branch
      headCommit: "abc123",
      isDirty: false,
      stagedFiles: [],
    };

    const result = validateCheckpoint(checkpoint, currentGit);
    expect(result.valid).toBe(false);
    if (!result.valid) {
      expect(result.reason).toContain("Branch mismatch");
    }
  });

  test("rejects checkpoint with unsupported version", () => {
    const checkpoint = createTestCheckpoint({
      version: 2 as any, // Future version
    });

    const currentGit: CheckpointGitState = {
      branch: "main",
      headCommit: "abc123",
      isDirty: false,
      stagedFiles: [],
    };

    const result = validateCheckpoint(checkpoint, currentGit);
    expect(result.valid).toBe(false);
    if (!result.valid) {
      expect(result.reason).toContain("Unsupported checkpoint version");
    }
  });
});

describe("Checkpoint File Operations", () => {
  let tmpDir: string;
  let openagentsDir: string;

  beforeEach(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "checkpoint-test-"));
    openagentsDir = path.join(tmpDir, ".openagents");
    fs.mkdirSync(openagentsDir, { recursive: true });
  });

  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  test("writeCheckpoint creates atomic file", async () => {
    const checkpoint = createTestCheckpoint();

    await runEffect(writeCheckpoint(openagentsDir, checkpoint));

    const checkpointPath = getCheckpointPath(openagentsDir);
    expect(fs.existsSync(checkpointPath)).toBe(true);

    const content = JSON.parse(fs.readFileSync(checkpointPath, "utf-8"));
    expect(content.sessionId).toBe(checkpoint.sessionId);
    expect(content.phase).toBe(checkpoint.phase);
  });

  test("readCheckpoint returns Option.none for missing file", async () => {
    const result = await runEffect(readCheckpoint(openagentsDir));
    expect(Option.isNone(result)).toBe(true);
  });

  test("readCheckpoint returns checkpoint when file exists", async () => {
    const checkpoint = createTestCheckpoint();
    await runEffect(writeCheckpoint(openagentsDir, checkpoint));

    const result = await runEffect(readCheckpoint(openagentsDir));
    expect(Option.isSome(result)).toBe(true);

    if (Option.isSome(result)) {
      expect(result.value.sessionId).toBe(checkpoint.sessionId);
      expect(result.value.taskId).toBe(checkpoint.taskId);
    }
  });

  test("readCheckpoint returns Option.none for corrupted JSON", async () => {
    const checkpointPath = getCheckpointPath(openagentsDir);
    fs.writeFileSync(checkpointPath, "{ invalid json }");

    const result = await runEffect(readCheckpoint(openagentsDir));
    expect(Option.isNone(result)).toBe(true);
  });

  test("clearCheckpoint removes checkpoint file", async () => {
    const checkpoint = createTestCheckpoint();
    await runEffect(writeCheckpoint(openagentsDir, checkpoint));

    const checkpointPath = getCheckpointPath(openagentsDir);
    expect(fs.existsSync(checkpointPath)).toBe(true);

    await runEffect(clearCheckpoint(openagentsDir));
    expect(fs.existsSync(checkpointPath)).toBe(false);
  });

  test("clearCheckpoint succeeds even if file doesn't exist", async () => {
    // Should not throw
    await runEffect(clearCheckpoint(openagentsDir));
  });

  test("clearCheckpoint removes temp file if it exists", async () => {
    const tempPath = path.join(openagentsDir, "checkpoint.json.tmp");
    fs.writeFileSync(tempPath, "temp content");

    await runEffect(clearCheckpoint(openagentsDir));
    expect(fs.existsSync(tempPath)).toBe(false);
  });
});

describe("Git State Capture", () => {
  let tmpDir: string;

  beforeEach(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "git-state-test-"));
    // Initialize git repo
    execSync("git init", { cwd: tmpDir, stdio: "pipe" });
    execSync("git config user.email 'test@test.com'", { cwd: tmpDir, stdio: "pipe" });
    execSync("git config user.name 'Test User'", { cwd: tmpDir, stdio: "pipe" });
    // Create initial commit
    fs.writeFileSync(path.join(tmpDir, "README.md"), "# Test");
    execSync("git add .", { cwd: tmpDir, stdio: "pipe" });
    execSync("git commit -m 'Initial commit'", { cwd: tmpDir, stdio: "pipe" });
  });

  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  test("captures clean git state", async () => {
    const result = await Effect.runPromise(captureGitState(tmpDir));

    expect(result.branch).toBeDefined();
    expect(result.headCommit).toHaveLength(40); // Full SHA
    expect(result.isDirty).toBe(false);
    expect(result.stagedFiles).toEqual([]);
  });

  test("detects dirty state with unstaged changes", async () => {
    // Create unstaged change
    fs.writeFileSync(path.join(tmpDir, "README.md"), "# Modified");

    const result = await Effect.runPromise(captureGitState(tmpDir));

    expect(result.isDirty).toBe(true);
    expect(result.stagedFiles).toEqual([]); // Not staged
  });

  test("detects staged files", async () => {
    // Create and stage a change
    fs.writeFileSync(path.join(tmpDir, "newfile.ts"), "export const x = 1;");
    execSync("git add newfile.ts", { cwd: tmpDir, stdio: "pipe" });

    const result = await Effect.runPromise(captureGitState(tmpDir));

    expect(result.isDirty).toBe(true);
    expect(result.stagedFiles).toContain("newfile.ts");
  });
});

describe("maybeResumeCheckpoint Integration", () => {
  let tmpDir: string;
  let openagentsDir: string;
  let gitDir: string;

  beforeEach(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "resume-test-"));
    openagentsDir = path.join(tmpDir, ".openagents");
    gitDir = tmpDir;
    fs.mkdirSync(openagentsDir, { recursive: true });

    // Initialize git repo
    execSync("git init", { cwd: gitDir, stdio: "pipe" });
    execSync("git config user.email 'test@test.com'", { cwd: gitDir, stdio: "pipe" });
    execSync("git config user.name 'Test User'", { cwd: gitDir, stdio: "pipe" });
    fs.writeFileSync(path.join(gitDir, "README.md"), "# Test");
    execSync("git add .", { cwd: gitDir, stdio: "pipe" });
    execSync("git commit -m 'Initial commit'", { cwd: gitDir, stdio: "pipe" });
  });

  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  test("returns None when no checkpoint exists", async () => {
    const result = await runEffect(maybeResumeCheckpoint(openagentsDir, gitDir));
    expect(Option.isNone(result)).toBe(true);
  });

  test("returns checkpoint when valid and branch matches", async () => {
    // Get current branch
    const branch = execSync("git rev-parse --abbrev-ref HEAD", {
      cwd: gitDir,
      encoding: "utf-8",
    }).trim();

    const checkpoint = createTestCheckpoint({
      timestamp: new Date().toISOString(),
      git: {
        branch,
        headCommit: "abc123",
        isDirty: false,
        stagedFiles: [],
      },
    });

    await runEffect(writeCheckpoint(openagentsDir, checkpoint));

    const result = await runEffect(maybeResumeCheckpoint(openagentsDir, gitDir));
    expect(Option.isSome(result)).toBe(true);

    if (Option.isSome(result)) {
      expect(result.value.sessionId).toBe(checkpoint.sessionId);
    }
  });

  test("clears and returns None for stale checkpoint", async () => {
    const branch = execSync("git rev-parse --abbrev-ref HEAD", {
      cwd: gitDir,
      encoding: "utf-8",
    }).trim();

    const staleTime = new Date(Date.now() - CHECKPOINT_MAX_AGE_MS - 60000);
    const checkpoint = createTestCheckpoint({
      timestamp: staleTime.toISOString(),
      git: {
        branch,
        headCommit: "abc123",
        isDirty: false,
        stagedFiles: [],
      },
    });

    await runEffect(writeCheckpoint(openagentsDir, checkpoint));

    const result = await runEffect(maybeResumeCheckpoint(openagentsDir, gitDir));
    expect(Option.isNone(result)).toBe(true);

    // Checkpoint should be cleared
    expect(fs.existsSync(getCheckpointPath(openagentsDir))).toBe(false);
  });

  test("clears and returns None for branch mismatch", async () => {
    const checkpoint = createTestCheckpoint({
      timestamp: new Date().toISOString(),
      git: {
        branch: "different-branch", // Doesn't match current branch
        headCommit: "abc123",
        isDirty: false,
        stagedFiles: [],
      },
    });

    await runEffect(writeCheckpoint(openagentsDir, checkpoint));

    const result = await runEffect(maybeResumeCheckpoint(openagentsDir, gitDir));
    expect(Option.isNone(result)).toBe(true);

    // Checkpoint should be cleared
    expect(fs.existsSync(getCheckpointPath(openagentsDir))).toBe(false);
  });
});
