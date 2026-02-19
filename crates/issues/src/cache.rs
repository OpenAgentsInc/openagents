//! Issue state caching to avoid duplicate work
//!
//! This module provides an in-memory cache of issue states with git-aware
//! invalidation to prevent autopilot from re-doing completed work.

use crate::{Issue, Status};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{Duration, Instant};

fn read_lock<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn write_lock<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Cache entry for an issue
#[derive(Debug, Clone)]
struct CachedIssue {
    issue: Issue,
    cached_at: Instant,
    #[expect(dead_code)]
    git_head: Option<String>,
}

/// Configuration for issue cache
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Time-to-live for cache entries
    pub ttl: Duration,
    /// Whether to invalidate cache on git state changes
    pub git_aware: bool,
    /// Maximum cache size (number of issues)
    pub max_size: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(60), // 1 minute TTL
            git_aware: true,              // Enable git-aware invalidation
            max_size: 1000,               // Cache up to 1000 issues
        }
    }
}

/// In-memory issue cache with git-aware invalidation
pub struct IssueCache {
    cache: Arc<RwLock<HashMap<String, CachedIssue>>>,
    config: CacheConfig,
    last_git_head: Arc<RwLock<Option<String>>>,
}

impl IssueCache {
    /// Create a new issue cache
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
            last_git_head: Arc::new(RwLock::new(None)),
        }
    }

    /// Get current git HEAD commit hash
    fn get_git_head() -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .context("Failed to run git rev-parse")?;

        if !output.status.success() {
            anyhow::bail!("git rev-parse failed");
        }

        let head = String::from_utf8(output.stdout)
            .context("Invalid UTF-8 in git output")?
            .trim()
            .to_string();

        Ok(head)
    }

    /// Check if cache should be invalidated due to git state change
    fn should_invalidate_for_git(&self) -> bool {
        if !self.config.git_aware {
            return false;
        }

        let Ok(current_head) = Self::get_git_head() else {
            // If we can't get git head, don't invalidate
            return false;
        };

        let mut last_head = write_lock(&self.last_git_head);
        if let Some(ref last) = *last_head {
            if last != &current_head {
                *last_head = Some(current_head);
                return true;
            }
        } else {
            *last_head = Some(current_head);
        }

        false
    }

    /// Get an issue from cache if valid
    pub fn get(&self, issue_id: &str) -> Option<Issue> {
        // Check if we need to invalidate due to git changes
        if self.should_invalidate_for_git() {
            self.clear();
            return None;
        }

        let cache = read_lock(&self.cache);
        if let Some(entry) = cache.get(issue_id) {
            // Check if entry is still valid (not expired)
            if entry.cached_at.elapsed() < self.config.ttl {
                return Some(entry.issue.clone());
            }
        }

        None
    }

    /// Put an issue into the cache
    pub fn put(&self, issue: Issue) {
        let mut cache = write_lock(&self.cache);

        // Enforce max size by removing oldest entries
        if cache.len() >= self.config.max_size {
            // Find oldest entry
            if let Some(oldest_id) = cache
                .iter()
                .min_by_key(|(_, entry)| entry.cached_at)
                .map(|(id, _)| id.clone())
            {
                cache.remove(&oldest_id);
            }
        }

        let git_head = if self.config.git_aware {
            Self::get_git_head().ok()
        } else {
            None
        };

        cache.insert(
            issue.id.clone(),
            CachedIssue {
                issue,
                cached_at: Instant::now(),
                git_head,
            },
        );
    }

    /// Invalidate a specific issue
    pub fn invalidate(&self, issue_id: &str) {
        let mut cache = write_lock(&self.cache);
        cache.remove(issue_id);
    }

    /// Invalidate issues by status
    pub fn invalidate_by_status(&self, status: Status) {
        let mut cache = write_lock(&self.cache);
        cache.retain(|_, entry| entry.issue.status != status);
    }

    /// Clear all cache entries
    pub fn clear(&self) {
        let mut cache = write_lock(&self.cache);
        cache.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let cache = read_lock(&self.cache);
        CacheStats {
            size: cache.len(),
            max_size: self.config.max_size,
            git_head: read_lock(&self.last_git_head).clone(),
        }
    }
}

impl Default for IssueCache {
    fn default() -> Self {
        Self::new(CacheConfig::default())
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Current cache size
    pub size: usize,
    /// Maximum cache size
    pub max_size: usize,
    /// Current git HEAD
    pub git_head: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Priority;

    fn create_test_issue(id: &str, status: Status) -> Issue {
        Issue {
            id: id.to_string(),
            number: 1,
            title: "Test Issue".to_string(),
            description: None,
            status,
            priority: Priority::Medium,
            issue_type: crate::IssueType::Task,
            is_blocked: false,
            blocked_reason: None,
            claimed_by: None,
            claimed_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            completed_at: None,
            agent: "codex".to_string(),
            directive_id: None,
            project_id: None,
            auto_created: false,
        }
    }

    #[test]
    fn test_cache_put_and_get() {
        let cache = IssueCache::new(CacheConfig {
            ttl: Duration::from_secs(60),
            git_aware: false,
            max_size: 10,
        });

        let issue = create_test_issue("issue-1", Status::Open);
        cache.put(issue.clone());

        let cached = cache.get("issue-1");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().id, "issue-1");
    }

    #[test]
    fn test_cache_expiration() {
        let cache = IssueCache::new(CacheConfig {
            ttl: Duration::from_millis(10), // Very short TTL
            git_aware: false,
            max_size: 10,
        });

        let issue = create_test_issue("issue-1", Status::Open);
        cache.put(issue);

        // Should be cached immediately
        assert!(cache.get("issue-1").is_some());

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(20));

        // Should be expired
        assert!(cache.get("issue-1").is_none());
    }

    #[test]
    fn test_cache_invalidation() {
        let cache = IssueCache::new(CacheConfig {
            ttl: Duration::from_secs(60),
            git_aware: false,
            max_size: 10,
        });

        let issue = create_test_issue("issue-1", Status::Open);
        cache.put(issue);

        assert!(cache.get("issue-1").is_some());

        cache.invalidate("issue-1");

        assert!(cache.get("issue-1").is_none());
    }

    #[test]
    fn test_cache_max_size() {
        let cache = IssueCache::new(CacheConfig {
            ttl: Duration::from_secs(60),
            git_aware: false,
            max_size: 3,
        });

        // Add 4 issues (exceeds max size)
        for i in 1..=4 {
            let issue = create_test_issue(&format!("issue-{}", i), Status::Open);
            cache.put(issue);
            std::thread::sleep(Duration::from_millis(10)); // Ensure different timestamps
        }

        let stats = cache.stats();
        assert_eq!(stats.size, 3); // Should only keep 3
    }

    #[test]
    fn test_invalidate_by_status() {
        let cache = IssueCache::new(CacheConfig {
            ttl: Duration::from_secs(60),
            git_aware: false,
            max_size: 10,
        });

        let issue1 = create_test_issue("issue-1", Status::Open);
        let issue2 = create_test_issue("issue-2", Status::Done);
        let issue3 = create_test_issue("issue-3", Status::Open);

        cache.put(issue1);
        cache.put(issue2);
        cache.put(issue3);

        // Invalidate all done issues
        cache.invalidate_by_status(Status::Done);

        assert!(cache.get("issue-1").is_some());
        assert!(cache.get("issue-2").is_none());
        assert!(cache.get("issue-3").is_some());
    }
}
