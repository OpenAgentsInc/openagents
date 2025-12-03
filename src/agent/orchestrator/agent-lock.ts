/**
 * Agent Lock Service
 *
 * Prevents multiple orchestrator instances from running concurrently on the same repository.
 * Uses a lock file (.openagents/agent.lock) containing the PID and timestamp.
 *
 * Lock file format:
 * ```
 * <pid>
 * <iso-timestamp>
 * <optional-session-id>
 * ```
 *
 * @see docs/mechacoder/GOLDEN-LOOP-v2.md Section 4.6.1 (Agent Lock Enforcement)
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
