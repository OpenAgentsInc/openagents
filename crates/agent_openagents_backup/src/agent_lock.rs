//! Agent Lock Service
//!
//! Prevents multiple orchestrator instances from running concurrently on the same repository.
//!
//! Lock file format:
//! ```text
//! <pid>
//! <iso-timestamp>
//! <optional-session-id>
//! ```

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process;

/// Lock file content structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLock {
    pub pid: u32,
    pub timestamp: String,
    pub session_id: Option<String>,
}

/// Result of attempting to acquire a lock
#[derive(Debug, Clone)]
pub enum AcquireLockResult {
    Acquired { lock: AgentLock },
    AlreadyRunning { existing_lock: AgentLock },
    StaleRemoved { removed_lock: AgentLock, new_lock: AgentLock },
}

impl AcquireLockResult {
    pub fn acquired(&self) -> bool {
        matches!(self, Self::Acquired { .. } | Self::StaleRemoved { .. })
    }
}

/// Result of checking lock status
#[derive(Debug, Clone)]
pub enum CheckLockResult {
    NotLocked,
    Locked { lock: AgentLock, is_stale: bool },
}

/// Get the path to the agent lock file
pub fn get_lock_path(openagents_dir: &str) -> String {
    format!("{}/agent.lock", openagents_dir)
}

/// Parse lock file content into structured data
pub fn parse_lock_file(content: &str) -> Option<AgentLock> {
    let lines: Vec<&str> = content.trim().split('\n').collect();
    if lines.len() < 2 {
        return None;
    }

    let pid = lines[0].parse::<u32>().ok()?;
    let timestamp = lines[1].to_string();
    let session_id = lines.get(2).map(|s| s.to_string());

    Some(AgentLock {
        pid,
        timestamp,
        session_id,
    })
}

/// Format lock data for writing to file
pub fn format_lock_file(lock: &AgentLock) -> String {
    let mut lines = vec![lock.pid.to_string(), lock.timestamp.clone()];
    if let Some(ref session_id) = lock.session_id {
        lines.push(session_id.clone());
    }
    lines.join("\n")
}

/// Check if a PID is still running
pub fn is_pid_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // On Unix, signal 0 doesn't send a signal but checks if process exists
        unsafe {
            libc::kill(pid as libc::pid_t, 0) == 0
        }
    }
    #[cfg(not(unix))]
    {
        // On other platforms, assume running (conservative)
        true
    }
}

/// Read the current lock file, if it exists
pub fn read_lock(openagents_dir: &str) -> Option<AgentLock> {
    let lock_path = get_lock_path(openagents_dir);
    let content = fs::read_to_string(&lock_path).ok()?;
    parse_lock_file(&content)
}

/// Check if a lock exists and whether it's stale
pub fn check_lock(openagents_dir: &str) -> CheckLockResult {
    match read_lock(openagents_dir) {
        None => CheckLockResult::NotLocked,
        Some(lock) => {
            let is_stale = !is_pid_running(lock.pid);
            CheckLockResult::Locked { lock, is_stale }
        }
    }
}

/// Attempt to acquire the lock.
///
/// - If no lock exists, creates one and returns success
/// - If lock exists but PID is not running (stale), removes it and creates new lock
/// - If lock exists and PID is running, returns failure with existing lock info
pub fn acquire_lock(openagents_dir: &str, session_id: Option<String>) -> AcquireLockResult {
    let lock_path = get_lock_path(openagents_dir);
    let existing_lock = read_lock(openagents_dir);

    if let Some(existing) = existing_lock {
        if is_pid_running(existing.pid) {
            return AcquireLockResult::AlreadyRunning {
                existing_lock: existing,
            };
        }

        // Lock is stale - remove it and create new one
        let new_lock = AgentLock {
            pid: process::id(),
            timestamp: Utc::now().to_rfc3339(),
            session_id,
        };

        let _ = fs::write(&lock_path, format_lock_file(&new_lock));

        return AcquireLockResult::StaleRemoved {
            removed_lock: existing,
            new_lock,
        };
    }

    // No existing lock - create one
    let new_lock = AgentLock {
        pid: process::id(),
        timestamp: Utc::now().to_rfc3339(),
        session_id,
    };

    // Ensure directory exists
    if let Some(parent) = Path::new(&lock_path).parent() {
        let _ = fs::create_dir_all(parent);
    }

    let _ = fs::write(&lock_path, format_lock_file(&new_lock));

    AcquireLockResult::Acquired { lock: new_lock }
}

/// Release the lock.
///
/// Only releases if the current process owns the lock.
pub fn release_lock(openagents_dir: &str) -> bool {
    let lock_path = get_lock_path(openagents_dir);
    let existing_lock = read_lock(openagents_dir);

    match existing_lock {
        None => false,
        Some(lock) if lock.pid != process::id() => false,
        Some(_) => {
            fs::remove_file(&lock_path).is_ok()
        }
    }
}

/// Force remove a lock file (for manual cleanup)
pub fn force_remove_lock(openagents_dir: &str) -> Option<AgentLock> {
    let lock_path = get_lock_path(openagents_dir);
    let existing_lock = read_lock(openagents_dir);

    if let Some(lock) = existing_lock {
        if fs::remove_file(&lock_path).is_ok() {
            return Some(lock);
        }
    }
    None
}

/// Lock guard that automatically releases on drop
pub struct LockGuard {
    openagents_dir: String,
    acquired: bool,
}

impl LockGuard {
    /// Create a new lock guard
    pub fn new(openagents_dir: &str, session_id: Option<String>) -> (Self, AcquireLockResult) {
        let result = acquire_lock(openagents_dir, session_id);
        let acquired = result.acquired();
        (
            Self {
                openagents_dir: openagents_dir.to_string(),
                acquired,
            },
            result,
        )
    }

    pub fn acquired(&self) -> bool {
        self.acquired
    }

    pub fn release(&mut self) -> bool {
        if self.acquired {
            self.acquired = false;
            release_lock(&self.openagents_dir)
        } else {
            false
        }
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if self.acquired {
            let _ = release_lock(&self.openagents_dir);
        }
    }
}

// ============================================================================
// Worktree-specific Locking (Parallel Mode)
// ============================================================================

/// Lock info for a worktree
#[derive(Debug, Clone)]
pub struct WorktreeLock {
    pub worktree_id: String,
    pub pid: u32,
    pub session_id: String,
    pub created_at: String,
}

const LOCKS_DIR: &str = "locks";

/// Get the path to the locks directory
pub fn get_locks_dir(openagents_dir: &str) -> String {
    format!("{}/{}", openagents_dir, LOCKS_DIR)
}

/// Get the path to a worktree lock file
pub fn get_worktree_lock_path(openagents_dir: &str, worktree_id: &str) -> String {
    format!("{}/{}.lock", get_locks_dir(openagents_dir), worktree_id)
}

/// Read a worktree lock file
pub fn read_worktree_lock(openagents_dir: &str, worktree_id: &str) -> Option<WorktreeLock> {
    let lock_path = get_worktree_lock_path(openagents_dir, worktree_id);
    let content = fs::read_to_string(&lock_path).ok()?;
    let parsed = parse_lock_file(&content)?;

    Some(WorktreeLock {
        worktree_id: worktree_id.to_string(),
        pid: parsed.pid,
        session_id: parsed.session_id.unwrap_or_default(),
        created_at: parsed.timestamp,
    })
}

/// Acquire a lock for a specific worktree
pub fn acquire_worktree_lock(openagents_dir: &str, worktree_id: &str, session_id: &str) -> bool {
    let locks_dir = get_locks_dir(openagents_dir);
    let lock_path = get_worktree_lock_path(openagents_dir, worktree_id);

    // Check existing lock
    if let Some(existing) = read_worktree_lock(openagents_dir, worktree_id) {
        if is_pid_running(existing.pid) {
            return false;
        }
        // Stale lock - remove it
        let _ = fs::remove_file(&lock_path);
    }

    // Ensure locks directory exists
    let _ = fs::create_dir_all(&locks_dir);

    // Create new lock
    let lock = AgentLock {
        pid: process::id(),
        timestamp: Utc::now().to_rfc3339(),
        session_id: Some(session_id.to_string()),
    };

    fs::write(&lock_path, format_lock_file(&lock)).is_ok()
}

/// Release a worktree lock
pub fn release_worktree_lock(openagents_dir: &str, worktree_id: &str) -> bool {
    let lock_path = get_worktree_lock_path(openagents_dir, worktree_id);

    if let Some(existing) = read_worktree_lock(openagents_dir, worktree_id) {
        if existing.pid != process::id() {
            return false;
        }
        return fs::remove_file(&lock_path).is_ok();
    }
    false
}

/// List all active worktree locks
pub fn list_worktree_locks(openagents_dir: &str) -> Vec<WorktreeLock> {
    let locks_dir = get_locks_dir(openagents_dir);
    let mut locks = Vec::new();

    if let Ok(entries) = fs::read_dir(&locks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "lock" {
                    if let Some(stem) = path.file_stem() {
                        let worktree_id = stem.to_string_lossy().to_string();
                        if let Some(lock) = read_worktree_lock(openagents_dir, &worktree_id) {
                            if is_pid_running(lock.pid) {
                                locks.push(lock);
                            }
                        }
                    }
                }
            }
        }
    }

    locks
}

/// Remove all stale worktree locks
pub fn prune_worktree_locks(openagents_dir: &str) -> usize {
    let locks_dir = get_locks_dir(openagents_dir);
    let mut removed = 0;

    if let Ok(entries) = fs::read_dir(&locks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "lock" {
                    if let Some(stem) = path.file_stem() {
                        let worktree_id = stem.to_string_lossy().to_string();
                        if let Some(lock) = read_worktree_lock(openagents_dir, &worktree_id) {
                            if !is_pid_running(lock.pid) {
                                if fs::remove_file(&path).is_ok() {
                                    removed += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_lock_path() {
        assert_eq!(
            get_lock_path("/home/user/.openagents"),
            "/home/user/.openagents/agent.lock"
        );
    }

    #[test]
    fn test_parse_lock_file() {
        let content = "12345\n2025-01-01T00:00:00Z\nsession-1";
        let lock = parse_lock_file(content).unwrap();
        assert_eq!(lock.pid, 12345);
        assert_eq!(lock.timestamp, "2025-01-01T00:00:00Z");
        assert_eq!(lock.session_id, Some("session-1".to_string()));
    }

    #[test]
    fn test_parse_lock_file_no_session() {
        let content = "12345\n2025-01-01T00:00:00Z";
        let lock = parse_lock_file(content).unwrap();
        assert_eq!(lock.pid, 12345);
        assert_eq!(lock.session_id, None);
    }

    #[test]
    fn test_parse_lock_file_invalid() {
        assert!(parse_lock_file("invalid").is_none());
        assert!(parse_lock_file("").is_none());
        assert!(parse_lock_file("not-a-number\ntimestamp").is_none());
    }

    #[test]
    fn test_format_lock_file() {
        let lock = AgentLock {
            pid: 12345,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            session_id: Some("session-1".to_string()),
        };
        let content = format_lock_file(&lock);
        assert_eq!(content, "12345\n2025-01-01T00:00:00Z\nsession-1");
    }

    #[test]
    fn test_format_lock_file_no_session() {
        let lock = AgentLock {
            pid: 12345,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            session_id: None,
        };
        let content = format_lock_file(&lock);
        assert_eq!(content, "12345\n2025-01-01T00:00:00Z");
    }

    #[test]
    fn test_get_worktree_lock_path() {
        assert_eq!(
            get_worktree_lock_path("/home/user/.openagents", "task-123"),
            "/home/user/.openagents/locks/task-123.lock"
        );
    }

    #[test]
    fn test_acquire_lock_result_acquired() {
        let result = AcquireLockResult::Acquired {
            lock: AgentLock {
                pid: 1,
                timestamp: "t".to_string(),
                session_id: None,
            },
        };
        assert!(result.acquired());
    }

    #[test]
    fn test_acquire_lock_result_already_running() {
        let result = AcquireLockResult::AlreadyRunning {
            existing_lock: AgentLock {
                pid: 1,
                timestamp: "t".to_string(),
                session_id: None,
            },
        };
        assert!(!result.acquired());
    }
}
