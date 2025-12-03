/**
 * Tests for Agent Lock Service
 *
 * Validates lock acquisition, release, stale detection, and concurrent access prevention.
 *
 * @see docs/mechacoder/GOLDEN-LOOP-v2.md Section 4.6.1 (Agent Lock Enforcement)
 * @see src/agent/orchestrator/agent-lock.ts (implementation)
 */
import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import {
  acquireLock,
  releaseLock,
  checkLock,
  readLock,
  forceRemoveLock,
  createLockGuard,
  getLockPath,
  parseLockFile,
  formatLockFile,
  isPidRunning,
  // Worktree lock functions
  acquireWorktreeLock,
  releaseWorktreeLock,
  listWorktreeLocks,
  pruneWorktreeLocks,
  createWorktreeLockGuard,
  getLocksDir,
  getWorktreeLockPath,
  readWorktreeLock,
  type AgentLock,
  type WorktreeLock,
} from "./agent-lock.js";

describe("agent-lock", () => {
  let testDir: string;

  beforeEach(() => {
    // Create a unique temp directory for each test
    testDir = fs.mkdtempSync(path.join(os.tmpdir(), "agent-lock-test-"));
  });

  afterEach(() => {
    // Clean up test directory
    try {
      fs.rmSync(testDir, { recursive: true, force: true });
    } catch {
      // Ignore cleanup errors
    }
  });

  describe("parseLockFile", () => {
    test("parses valid lock file with PID and timestamp", () => {
      const content = "12345\n2025-01-01T00:00:00.000Z";
      const lock = parseLockFile(content);
      expect(lock).toEqual({
        pid: 12345,
        timestamp: "2025-01-01T00:00:00.000Z",
      });
    });

    test("parses lock file with session ID", () => {
      const content = "12345\n2025-01-01T00:00:00.000Z\nsession-abc123";
      const lock = parseLockFile(content);
      expect(lock).toEqual({
        pid: 12345,
        timestamp: "2025-01-01T00:00:00.000Z",
        sessionId: "session-abc123",
      });
    });

    test("returns null for empty content", () => {
      expect(parseLockFile("")).toBeNull();
    });

    test("returns null for single line", () => {
      expect(parseLockFile("12345")).toBeNull();
    });

    test("returns null for invalid PID", () => {
      expect(parseLockFile("notanumber\n2025-01-01T00:00:00.000Z")).toBeNull();
    });
  });

  describe("formatLockFile", () => {
    test("formats lock without session ID", () => {
      const lock: AgentLock = {
        pid: 12345,
        timestamp: "2025-01-01T00:00:00.000Z",
      };
      expect(formatLockFile(lock)).toBe("12345\n2025-01-01T00:00:00.000Z");
    });

    test("formats lock with session ID", () => {
      const lock: AgentLock = {
        pid: 12345,
        timestamp: "2025-01-01T00:00:00.000Z",
        sessionId: "session-xyz",
      };
      expect(formatLockFile(lock)).toBe("12345\n2025-01-01T00:00:00.000Z\nsession-xyz");
    });

    test("roundtrips correctly", () => {
      const original: AgentLock = {
        pid: 99999,
        timestamp: "2025-12-03T10:30:00.000Z",
        sessionId: "test-session-001",
      };
      const formatted = formatLockFile(original);
      const parsed = parseLockFile(formatted);
      expect(parsed).toEqual(original);
    });
  });

  describe("isPidRunning", () => {
    test("returns true for current process", () => {
      expect(isPidRunning(process.pid)).toBe(true);
    });

    test("returns false for non-existent PID", () => {
      // Use a very high PID that's unlikely to exist
      expect(isPidRunning(999999999)).toBe(false);
    });
  });

  describe("acquireLock", () => {
    test("acquires lock when none exists", () => {
      const result = acquireLock(testDir, "test-session");

      expect(result.acquired).toBe(true);
      if (result.acquired) {
        expect(result.lock.pid).toBe(process.pid);
        expect(result.lock.sessionId).toBe("test-session");
      }

      // Verify file was created
      const lockPath = getLockPath(testDir);
      expect(fs.existsSync(lockPath)).toBe(true);
    });

    test("fails when lock is held by running process", () => {
      // First acquisition
      const first = acquireLock(testDir, "first");
      expect(first.acquired).toBe(true);

      // Second acquisition should fail (same process, but still "running")
      const second = acquireLock(testDir, "second");
      expect(second.acquired).toBe(false);
      if (!second.acquired && second.reason === "already_running") {
        expect(second.existingLock.sessionId).toBe("first");
      }
    });

    test("removes stale lock and acquires new one", () => {
      // Create a stale lock with a non-existent PID
      const lockPath = getLockPath(testDir);
      fs.mkdirSync(testDir, { recursive: true });
      fs.writeFileSync(
        lockPath,
        "999999999\n2025-01-01T00:00:00.000Z\nstale-session",
        "utf-8"
      );

      const result = acquireLock(testDir, "new-session");

      // Should have removed stale and acquired
      expect(result.acquired).toBe(false);
      if (!result.acquired && result.reason === "stale_removed") {
        expect(result.removedLock.pid).toBe(999999999);
        expect(result.newLock.pid).toBe(process.pid);
        expect(result.newLock.sessionId).toBe("new-session");
      }
    });

    test("creates .openagents directory if it doesn't exist", () => {
      const nestedDir = path.join(testDir, "nested", ".openagents");
      const result = acquireLock(nestedDir, "test");

      expect(result.acquired).toBe(true);
      expect(fs.existsSync(getLockPath(nestedDir))).toBe(true);
    });
  });

  describe("releaseLock", () => {
    test("releases lock owned by current process", () => {
      acquireLock(testDir, "test");
      const released = releaseLock(testDir);

      expect(released).toBe(true);
      expect(fs.existsSync(getLockPath(testDir))).toBe(false);
    });

    test("returns false when no lock exists", () => {
      expect(releaseLock(testDir)).toBe(false);
    });

    test("returns false when lock owned by different PID", () => {
      // Create lock with different PID
      const lockPath = getLockPath(testDir);
      fs.mkdirSync(testDir, { recursive: true });
      fs.writeFileSync(lockPath, "999999999\n2025-01-01T00:00:00.000Z", "utf-8");

      const released = releaseLock(testDir);
      expect(released).toBe(false);
      // Lock should still exist
      expect(fs.existsSync(lockPath)).toBe(true);
    });
  });

  describe("checkLock", () => {
    test("returns locked: false when no lock exists", () => {
      const result = checkLock(testDir);
      expect(result.locked).toBe(false);
    });

    test("returns locked: true with isStale: false for running process", () => {
      acquireLock(testDir, "test");
      const result = checkLock(testDir);

      expect(result.locked).toBe(true);
      if (result.locked) {
        expect(result.isStale).toBe(false);
        expect(result.lock.pid).toBe(process.pid);
      }
    });

    test("returns locked: true with isStale: true for dead process", () => {
      // Create lock with non-existent PID
      const lockPath = getLockPath(testDir);
      fs.mkdirSync(testDir, { recursive: true });
      fs.writeFileSync(lockPath, "999999999\n2025-01-01T00:00:00.000Z", "utf-8");

      const result = checkLock(testDir);
      expect(result.locked).toBe(true);
      if (result.locked) {
        expect(result.isStale).toBe(true);
        expect(result.lock.pid).toBe(999999999);
      }
    });
  });

  describe("readLock", () => {
    test("returns null when no lock exists", () => {
      expect(readLock(testDir)).toBeNull();
    });

    test("returns lock contents when lock exists", () => {
      acquireLock(testDir, "test-session");
      const lock = readLock(testDir);

      expect(lock).not.toBeNull();
      expect(lock?.pid).toBe(process.pid);
      expect(lock?.sessionId).toBe("test-session");
    });
  });

  describe("forceRemoveLock", () => {
    test("removes lock regardless of ownership", () => {
      // Create lock with different PID
      const lockPath = getLockPath(testDir);
      fs.mkdirSync(testDir, { recursive: true });
      fs.writeFileSync(
        lockPath,
        "999999999\n2025-01-01T00:00:00.000Z\nother-session",
        "utf-8"
      );

      const removed = forceRemoveLock(testDir);

      expect(removed).not.toBeNull();
      expect(removed?.pid).toBe(999999999);
      expect(fs.existsSync(lockPath)).toBe(false);
    });

    test("returns null when no lock exists", () => {
      expect(forceRemoveLock(testDir)).toBeNull();
    });
  });

  describe("createLockGuard", () => {
    test("creates guard that can be released", () => {
      const guard = createLockGuard(testDir, "guarded-session");

      expect(guard.acquired).toBe(true);
      expect(fs.existsSync(getLockPath(testDir))).toBe(true);

      const released = guard.release();
      expect(released).toBe(true);
      expect(fs.existsSync(getLockPath(testDir))).toBe(false);
    });

    test("guard reports not acquired when lock held", () => {
      // Create existing lock
      acquireLock(testDir, "existing");

      const guard = createLockGuard(testDir, "new");
      expect(guard.acquired).toBe(false);
    });

    test("guard acquires and reports stale removal", () => {
      // Create stale lock
      const lockPath = getLockPath(testDir);
      fs.mkdirSync(testDir, { recursive: true });
      fs.writeFileSync(lockPath, "999999999\n2025-01-01T00:00:00.000Z", "utf-8");

      const guard = createLockGuard(testDir, "new");

      // Should have acquired after removing stale
      expect(guard.acquired).toBe(true);
      expect(guard.result.acquired).toBe(false);
      if (!guard.result.acquired) {
        expect(guard.result.reason).toBe("stale_removed");
      }
    });
  });

  describe("getLockPath", () => {
    test("returns correct path", () => {
      expect(getLockPath("/path/to/.openagents")).toBe("/path/to/.openagents/agent.lock");
    });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Worktree Lock Tests (Parallel Mode)
// ─────────────────────────────────────────────────────────────────────────────

describe("worktree-locks", () => {
  let testDir: string;

  beforeEach(() => {
    testDir = fs.mkdtempSync(path.join(os.tmpdir(), "worktree-lock-test-"));
  });

  afterEach(() => {
    try {
      fs.rmSync(testDir, { recursive: true, force: true });
    } catch {
      // Ignore cleanup errors
    }
  });

  describe("getLocksDir", () => {
    test("returns correct path", () => {
      expect(getLocksDir("/path/.openagents")).toBe("/path/.openagents/locks");
    });
  });

  describe("getWorktreeLockPath", () => {
    test("returns correct path", () => {
      expect(getWorktreeLockPath("/path/.openagents", "oa-abc123")).toBe(
        "/path/.openagents/locks/oa-abc123.lock",
      );
    });
  });

  describe("acquireWorktreeLock", () => {
    test("acquires lock when none exists", () => {
      const acquired = acquireWorktreeLock(testDir, "oa-task1", "session-1");

      expect(acquired).toBe(true);
      expect(fs.existsSync(getWorktreeLockPath(testDir, "oa-task1"))).toBe(true);
    });

    test("fails when lock is held by running process", () => {
      // First acquisition
      expect(acquireWorktreeLock(testDir, "oa-task1", "session-1")).toBe(true);

      // Second acquisition should fail
      expect(acquireWorktreeLock(testDir, "oa-task1", "session-2")).toBe(false);
    });

    test("allows different worktrees to be locked simultaneously", () => {
      expect(acquireWorktreeLock(testDir, "oa-task1", "session-1")).toBe(true);
      expect(acquireWorktreeLock(testDir, "oa-task2", "session-2")).toBe(true);
      expect(acquireWorktreeLock(testDir, "oa-task3", "session-3")).toBe(true);

      // All three locks should exist
      expect(fs.existsSync(getWorktreeLockPath(testDir, "oa-task1"))).toBe(true);
      expect(fs.existsSync(getWorktreeLockPath(testDir, "oa-task2"))).toBe(true);
      expect(fs.existsSync(getWorktreeLockPath(testDir, "oa-task3"))).toBe(true);
    });

    test("removes stale lock and acquires new one", () => {
      // Create a stale lock with non-existent PID
      const locksDir = getLocksDir(testDir);
      fs.mkdirSync(locksDir, { recursive: true });
      fs.writeFileSync(
        getWorktreeLockPath(testDir, "oa-task1"),
        "999999999\n2025-01-01T00:00:00.000Z\nstale-session",
        "utf-8",
      );

      // Should succeed after removing stale
      expect(acquireWorktreeLock(testDir, "oa-task1", "new-session")).toBe(true);

      const lock = readWorktreeLock(testDir, "oa-task1");
      expect(lock?.pid).toBe(process.pid);
      expect(lock?.sessionId).toBe("new-session");
    });

    test("creates locks directory if it doesn't exist", () => {
      const locksDir = getLocksDir(testDir);
      expect(fs.existsSync(locksDir)).toBe(false);

      acquireWorktreeLock(testDir, "oa-task1", "session-1");

      expect(fs.existsSync(locksDir)).toBe(true);
    });
  });

  describe("releaseWorktreeLock", () => {
    test("releases lock owned by current process", () => {
      acquireWorktreeLock(testDir, "oa-task1", "session-1");
      const released = releaseWorktreeLock(testDir, "oa-task1");

      expect(released).toBe(true);
      expect(fs.existsSync(getWorktreeLockPath(testDir, "oa-task1"))).toBe(false);
    });

    test("returns false when no lock exists", () => {
      expect(releaseWorktreeLock(testDir, "oa-nonexistent")).toBe(false);
    });

    test("returns false when lock owned by different PID", () => {
      // Create lock with different PID
      const locksDir = getLocksDir(testDir);
      fs.mkdirSync(locksDir, { recursive: true });
      fs.writeFileSync(
        getWorktreeLockPath(testDir, "oa-task1"),
        "999999999\n2025-01-01T00:00:00.000Z\nother",
        "utf-8",
      );

      expect(releaseWorktreeLock(testDir, "oa-task1")).toBe(false);
      // Lock should still exist
      expect(fs.existsSync(getWorktreeLockPath(testDir, "oa-task1"))).toBe(true);
    });
  });

  describe("listWorktreeLocks", () => {
    test("returns empty array when no locks exist", () => {
      expect(listWorktreeLocks(testDir)).toEqual([]);
    });

    test("returns active locks only", () => {
      // Create active locks
      acquireWorktreeLock(testDir, "oa-task1", "session-1");
      acquireWorktreeLock(testDir, "oa-task2", "session-2");

      // Create a stale lock
      fs.writeFileSync(
        getWorktreeLockPath(testDir, "oa-stale"),
        "999999999\n2025-01-01T00:00:00.000Z\nstale",
        "utf-8",
      );

      const locks = listWorktreeLocks(testDir);

      expect(locks.length).toBe(2);
      expect(locks.map((l) => l.worktreeId).sort()).toEqual(["oa-task1", "oa-task2"]);
    });
  });

  describe("pruneWorktreeLocks", () => {
    test("removes stale locks", () => {
      const locksDir = getLocksDir(testDir);
      fs.mkdirSync(locksDir, { recursive: true });

      // Create stale locks
      fs.writeFileSync(
        getWorktreeLockPath(testDir, "oa-stale1"),
        "999999998\n2025-01-01T00:00:00.000Z\nstale1",
        "utf-8",
      );
      fs.writeFileSync(
        getWorktreeLockPath(testDir, "oa-stale2"),
        "999999997\n2025-01-01T00:00:00.000Z\nstale2",
        "utf-8",
      );

      // Create active lock
      acquireWorktreeLock(testDir, "oa-active", "active-session");

      const removed = pruneWorktreeLocks(testDir);

      expect(removed).toBe(2);
      expect(fs.existsSync(getWorktreeLockPath(testDir, "oa-stale1"))).toBe(false);
      expect(fs.existsSync(getWorktreeLockPath(testDir, "oa-stale2"))).toBe(false);
      expect(fs.existsSync(getWorktreeLockPath(testDir, "oa-active"))).toBe(true);
    });

    test("returns 0 when no locks exist", () => {
      expect(pruneWorktreeLocks(testDir)).toBe(0);
    });
  });

  describe("readWorktreeLock", () => {
    test("returns null when lock doesn't exist", () => {
      expect(readWorktreeLock(testDir, "oa-nonexistent")).toBeNull();
    });

    test("returns lock info when lock exists", () => {
      acquireWorktreeLock(testDir, "oa-task1", "session-123");
      const lock = readWorktreeLock(testDir, "oa-task1");

      expect(lock).not.toBeNull();
      expect(lock?.worktreeId).toBe("oa-task1");
      expect(lock?.pid).toBe(process.pid);
      expect(lock?.sessionId).toBe("session-123");
    });
  });

  describe("createWorktreeLockGuard", () => {
    test("creates guard that can be released", () => {
      const guard = createWorktreeLockGuard(testDir, "oa-task1", "session-1");

      expect(guard.acquired).toBe(true);
      expect(fs.existsSync(getWorktreeLockPath(testDir, "oa-task1"))).toBe(true);

      const released = guard.release();
      expect(released).toBe(true);
      expect(fs.existsSync(getWorktreeLockPath(testDir, "oa-task1"))).toBe(false);
    });

    test("guard reports not acquired when lock held", () => {
      acquireWorktreeLock(testDir, "oa-task1", "existing");

      const guard = createWorktreeLockGuard(testDir, "oa-task1", "new");
      expect(guard.acquired).toBe(false);
    });

    test("multiple guards for different worktrees", () => {
      const guard1 = createWorktreeLockGuard(testDir, "oa-task1", "session-1");
      const guard2 = createWorktreeLockGuard(testDir, "oa-task2", "session-2");
      const guard3 = createWorktreeLockGuard(testDir, "oa-task3", "session-3");

      expect(guard1.acquired).toBe(true);
      expect(guard2.acquired).toBe(true);
      expect(guard3.acquired).toBe(true);

      guard1.release();
      guard2.release();
      guard3.release();

      expect(listWorktreeLocks(testDir)).toEqual([]);
    });
  });
});
