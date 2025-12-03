/**
 * Agent Lock Service
 *
 * Prevents multiple orchestrator instances from running concurrently on the same repository.
 *
 * Two lock modes:
 * 1. Single-agent mode: Uses .openagents/agent.lock (legacy, backwards compatible)
 * 2. Parallel mode: Uses .openagents/locks/{worktreeId}.lock (one per worktree)
 *
 * Lock file format:
 * ```
 * <pid>
 * <iso-timestamp>
 * <optional-session-id>
 * ```
 *
 * @see docs/mechacoder/GOLDEN-LOOP-v2.md Section 4.6.1 (Agent Lock Enforcement)
 * @see docs/claude/plans/containers-impl-v2.md (Parallel execution plan)
 */
import * as fs from "node:fs";
import * as path from "node:path";

/** Lock file content structure */
export interface AgentLock {
  pid: number;
  timestamp: string;
  sessionId?: string;
}

/** Result of attempting to acquire a lock */
export type AcquireLockResult =
  | { acquired: true; lock: AgentLock }
  | { acquired: false; reason: "already_running"; existingLock: AgentLock }
  | { acquired: false; reason: "stale_removed"; removedLock: AgentLock; newLock: AgentLock };

/** Result of checking lock status */
export type CheckLockResult =
  | { locked: false }
  | { locked: true; lock: AgentLock; isStale: boolean };

/**
 * Get the path to the agent lock file
 */
export const getLockPath = (openagentsDir: string): string =>
  path.join(openagentsDir, "agent.lock");

/**
 * Parse lock file content into structured data
 */
export const parseLockFile = (content: string): AgentLock | null => {
  const lines = content.trim().split("\n");
  if (lines.length < 2) return null;

  const pid = parseInt(lines[0], 10);
  if (isNaN(pid)) return null;

  const sessionId = lines[2];
  const lock: AgentLock = {
    pid,
    timestamp: lines[1],
  };
  if (sessionId) {
    lock.sessionId = sessionId;
  }
  return lock;
};

/**
 * Format lock data for writing to file
 */
export const formatLockFile = (lock: AgentLock): string => {
  const lines = [String(lock.pid), lock.timestamp];
  if (lock.sessionId) {
    lines.push(lock.sessionId);
  }
  return lines.join("\n");
};

/**
 * Check if a PID is still running
 */
export const isPidRunning = (pid: number): boolean => {
  try {
    // Signal 0 doesn't send a signal but checks if process exists
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

/**
 * Read the current lock file, if it exists
 */
export const readLock = (openagentsDir: string): AgentLock | null => {
  const lockPath = getLockPath(openagentsDir);
  try {
    const content = fs.readFileSync(lockPath, "utf-8");
    return parseLockFile(content);
  } catch {
    return null;
  }
};

/**
 * Check if a lock exists and whether it's stale
 */
export const checkLock = (openagentsDir: string): CheckLockResult => {
  const lock = readLock(openagentsDir);
  if (!lock) {
    return { locked: false };
  }

  const isStale = !isPidRunning(lock.pid);
  return { locked: true, lock, isStale };
};

/**
 * Attempt to acquire the lock.
 *
 * - If no lock exists, creates one and returns success
 * - If lock exists but PID is not running (stale), removes it and creates new lock
 * - If lock exists and PID is running, returns failure with existing lock info
 *
 * @param openagentsDir - Path to .openagents directory
 * @param sessionId - Optional session ID to include in lock
 * @returns Result indicating whether lock was acquired
 */
export const acquireLock = (
  openagentsDir: string,
  sessionId?: string
): AcquireLockResult => {
  const lockPath = getLockPath(openagentsDir);
  const existingLock = readLock(openagentsDir);

  if (existingLock) {
    if (isPidRunning(existingLock.pid)) {
      // Lock is held by a running process
      return {
        acquired: false,
        reason: "already_running",
        existingLock,
      };
    }

    // Lock is stale - remove it and create new one
    const newLock: AgentLock = {
      pid: process.pid,
      timestamp: new Date().toISOString(),
    };
    if (sessionId) {
      newLock.sessionId = sessionId;
    }

    fs.writeFileSync(lockPath, formatLockFile(newLock), "utf-8");

    return {
      acquired: false,
      reason: "stale_removed",
      removedLock: existingLock,
      newLock,
    };
  }

  // No existing lock - create one
  const newLock: AgentLock = {
    pid: process.pid,
    timestamp: new Date().toISOString(),
  };
  if (sessionId) {
    newLock.sessionId = sessionId;
  }

  // Ensure .openagents directory exists
  if (!fs.existsSync(openagentsDir)) {
    fs.mkdirSync(openagentsDir, { recursive: true });
  }

  fs.writeFileSync(lockPath, formatLockFile(newLock), "utf-8");

  return { acquired: true, lock: newLock };
};

/**
 * Release the lock.
 *
 * Only releases if the current process owns the lock.
 *
 * @param openagentsDir - Path to .openagents directory
 * @returns true if lock was released, false if not owned or doesn't exist
 */
export const releaseLock = (openagentsDir: string): boolean => {
  const lockPath = getLockPath(openagentsDir);
  const existingLock = readLock(openagentsDir);

  if (!existingLock) {
    // No lock to release
    return false;
  }

  if (existingLock.pid !== process.pid) {
    // Lock owned by different process - don't release
    return false;
  }

  try {
    fs.unlinkSync(lockPath);
    return true;
  } catch {
    return false;
  }
};

/**
 * Force remove a lock file (for manual cleanup)
 *
 * Use with caution - only for recovering from stale locks
 *
 * @param openagentsDir - Path to .openagents directory
 * @returns The removed lock info, or null if no lock existed
 */
export const forceRemoveLock = (openagentsDir: string): AgentLock | null => {
  const lockPath = getLockPath(openagentsDir);
  const existingLock = readLock(openagentsDir);

  if (!existingLock) {
    return null;
  }

  try {
    fs.unlinkSync(lockPath);
    return existingLock;
  } catch {
    return null;
  }
};

/**
 * Create a lock guard that automatically releases on scope exit.
 *
 * Usage:
 * ```typescript
 * const guard = createLockGuard(openagentsDir, sessionId);
 * if (!guard.acquired) {
 *   console.error("Another agent is running");
 *   return;
 * }
 * try {
 *   // ... do work ...
 * } finally {
 *   guard.release();
 * }
 * ```
 */
export const createLockGuard = (
  openagentsDir: string,
  sessionId?: string
): {
  acquired: boolean;
  result: AcquireLockResult;
  release: () => boolean;
} => {
  const result = acquireLock(openagentsDir, sessionId);
  const acquired = result.acquired || result.reason === "stale_removed";

  return {
    acquired,
    result,
    release: () => releaseLock(openagentsDir),
  };
};

// ─────────────────────────────────────────────────────────────────────────────
// Worktree-specific Locking (Parallel Mode)
// ─────────────────────────────────────────────────────────────────────────────

/** Lock info for a worktree */
export interface WorktreeLock {
  worktreeId: string;
  pid: number;
  sessionId: string;
  createdAt: string;
}

/** Directory containing worktree locks */
const LOCKS_DIR = "locks";

/**
 * Get the path to the locks directory
 */
export const getLocksDir = (openagentsDir: string): string =>
  path.join(openagentsDir, LOCKS_DIR);

/**
 * Get the path to a worktree lock file
 */
export const getWorktreeLockPath = (
  openagentsDir: string,
  worktreeId: string,
): string => path.join(getLocksDir(openagentsDir), `${worktreeId}.lock`);

/**
 * Read a worktree lock file
 */
export const readWorktreeLock = (
  openagentsDir: string,
  worktreeId: string,
): WorktreeLock | null => {
  const lockPath = getWorktreeLockPath(openagentsDir, worktreeId);
  try {
    const content = fs.readFileSync(lockPath, "utf-8");
    const parsed = parseLockFile(content);
    if (!parsed) return null;
    return {
      worktreeId,
      pid: parsed.pid,
      sessionId: parsed.sessionId ?? "",
      createdAt: parsed.timestamp,
    };
  } catch {
    return null;
  }
};

/**
 * Acquire a lock for a specific worktree.
 *
 * @param openagentsDir - Path to .openagents directory
 * @param worktreeId - ID of the worktree (typically task ID)
 * @param sessionId - Session ID for tracking
 * @returns true if lock was acquired, false if already locked
 */
export const acquireWorktreeLock = (
  openagentsDir: string,
  worktreeId: string,
  sessionId: string,
): boolean => {
  const locksDir = getLocksDir(openagentsDir);
  const lockPath = getWorktreeLockPath(openagentsDir, worktreeId);

  // Check existing lock
  const existingLock = readWorktreeLock(openagentsDir, worktreeId);
  if (existingLock) {
    if (isPidRunning(existingLock.pid)) {
      // Lock is held by a running process
      return false;
    }
    // Stale lock - remove it
    try {
      fs.unlinkSync(lockPath);
    } catch {
      // Ignore removal errors
    }
  }

  // Ensure locks directory exists
  if (!fs.existsSync(locksDir)) {
    fs.mkdirSync(locksDir, { recursive: true });
  }

  // Create new lock
  const lock: AgentLock = {
    pid: process.pid,
    timestamp: new Date().toISOString(),
    sessionId,
  };

  fs.writeFileSync(lockPath, formatLockFile(lock), "utf-8");
  return true;
};

/**
 * Release a worktree lock.
 *
 * Only releases if the current process owns the lock.
 *
 * @param openagentsDir - Path to .openagents directory
 * @param worktreeId - ID of the worktree
 * @returns true if lock was released, false if not owned or doesn't exist
 */
export const releaseWorktreeLock = (
  openagentsDir: string,
  worktreeId: string,
): boolean => {
  const lockPath = getWorktreeLockPath(openagentsDir, worktreeId);
  const existingLock = readWorktreeLock(openagentsDir, worktreeId);

  if (!existingLock) {
    return false;
  }

  if (existingLock.pid !== process.pid) {
    return false;
  }

  try {
    fs.unlinkSync(lockPath);
    return true;
  } catch {
    return false;
  }
};

/**
 * List all active worktree locks.
 *
 * @param openagentsDir - Path to .openagents directory
 * @returns Array of active locks (with running PIDs)
 */
export const listWorktreeLocks = (openagentsDir: string): WorktreeLock[] => {
  const locksDir = getLocksDir(openagentsDir);
  const locks: WorktreeLock[] = [];

  if (!fs.existsSync(locksDir)) {
    return locks;
  }

  const entries = fs.readdirSync(locksDir);
  for (const entry of entries) {
    if (!entry.endsWith(".lock")) continue;

    const worktreeId = entry.slice(0, -5); // Remove .lock extension
    const lock = readWorktreeLock(openagentsDir, worktreeId);

    if (lock && isPidRunning(lock.pid)) {
      locks.push(lock);
    }
  }

  return locks;
};

/**
 * Remove all stale worktree locks (where PID is no longer running).
 *
 * @param openagentsDir - Path to .openagents directory
 * @returns Number of stale locks removed
 */
export const pruneWorktreeLocks = (openagentsDir: string): number => {
  const locksDir = getLocksDir(openagentsDir);
  let removed = 0;

  if (!fs.existsSync(locksDir)) {
    return removed;
  }

  const entries = fs.readdirSync(locksDir);
  for (const entry of entries) {
    if (!entry.endsWith(".lock")) continue;

    const worktreeId = entry.slice(0, -5);
    const lock = readWorktreeLock(openagentsDir, worktreeId);

    if (lock && !isPidRunning(lock.pid)) {
      try {
        fs.unlinkSync(path.join(locksDir, entry));
        removed++;
      } catch {
        // Ignore removal errors
      }
    }
  }

  return removed;
};

/**
 * Create a worktree lock guard for scoped locking.
 *
 * Usage:
 * ```typescript
 * const guard = createWorktreeLockGuard(openagentsDir, "oa-abc123", "session-1");
 * if (!guard.acquired) {
 *   console.error("Worktree is already locked");
 *   return;
 * }
 * try {
 *   // ... do work in worktree ...
 * } finally {
 *   guard.release();
 * }
 * ```
 */
export const createWorktreeLockGuard = (
  openagentsDir: string,
  worktreeId: string,
  sessionId: string,
): {
  acquired: boolean;
  release: () => boolean;
} => {
  const acquired = acquireWorktreeLock(openagentsDir, worktreeId, sessionId);

  return {
    acquired,
    release: () => releaseWorktreeLock(openagentsDir, worktreeId),
  };
};
