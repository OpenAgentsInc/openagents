//! Progress tracking with taskmaster integration.

use crate::Result;
use crate::discovery::DiscoveredTask;
use std::path::PathBuf;
use std::sync::Arc;
use taskmaster::{IssueRepository, SqliteRepository};

/// Tracks progress of task execution and updates taskmaster.
pub struct ProgressTracker {
    /// Taskmaster repository (if available).
    repo: Option<Arc<SqliteRepository>>,
    /// Actor name for audit trail.
    actor: String,
}

impl ProgressTracker {
    /// Create a new progress tracker.
    pub fn new(db_path: Option<&PathBuf>, actor: impl Into<String>) -> Result<Self> {
        let repo = if let Some(path) = db_path {
            if path.exists() {
                Some(Arc::new(SqliteRepository::open(path)?))
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            repo,
            actor: actor.into(),
        })
    }

    /// Create a tracker without taskmaster integration.
    pub fn without_taskmaster(actor: impl Into<String>) -> Self {
        Self {
            repo: None,
            actor: actor.into(),
        }
    }

    /// Check if taskmaster integration is available.
    pub fn has_taskmaster(&self) -> bool {
        self.repo.is_some()
    }

    /// Mark a task as started (in progress).
    pub fn task_started(&self, task: &DiscoveredTask) -> Result<()> {
        if let Some(repo) = &self.repo {
            // Only update if this is a taskmaster task
            if let crate::discovery::TaskDiscoverySource::Taskmaster { issue_id } = &task.source {
                repo.start(issue_id, Some(&self.actor))?;
                tracing::info!(task_id = %issue_id, "Task started in taskmaster");
            }
        }
        Ok(())
    }

    /// Add a commit to a task.
    pub fn add_commit(&self, task: &DiscoveredTask, sha: &str) -> Result<()> {
        if let Some(repo) = &self.repo {
            if let crate::discovery::TaskDiscoverySource::Taskmaster { issue_id } = &task.source {
                repo.add_commit(issue_id, sha)?;
                tracing::info!(task_id = %issue_id, sha = %sha, "Commit added to task");
            }
        }
        Ok(())
    }

    /// Mark a task as completed successfully.
    pub fn task_completed(&self, task: &DiscoveredTask, commits: Vec<String>) -> Result<()> {
        if let Some(repo) = &self.repo {
            if let crate::discovery::TaskDiscoverySource::Taskmaster { issue_id } = &task.source {
                repo.close(
                    issue_id,
                    Some("Completed by auto mode"),
                    commits,
                    Some(&self.actor),
                )?;
                tracing::info!(task_id = %issue_id, "Task completed in taskmaster");
            }
        }
        Ok(())
    }

    /// Mark a task as failed.
    pub fn task_failed(&self, task: &DiscoveredTask, reason: &str) -> Result<()> {
        if let Some(repo) = &self.repo {
            if let crate::discovery::TaskDiscoverySource::Taskmaster { issue_id } = &task.source {
                // Add a comment about the failure but don't close
                repo.add_comment(
                    issue_id,
                    taskmaster::CommentCreate::new(
                        &self.actor,
                        format!("Auto mode failed: {}", reason),
                    ),
                )?;
                tracing::warn!(task_id = %issue_id, reason = %reason, "Task failed in auto mode");
            }
        }
        Ok(())
    }

    /// Block a task (e.g., due to dependency issues).
    pub fn task_blocked(&self, task: &DiscoveredTask, reason: &str) -> Result<()> {
        if let Some(repo) = &self.repo {
            if let crate::discovery::TaskDiscoverySource::Taskmaster { issue_id } = &task.source {
                repo.block(issue_id, Some(reason), Some(&self.actor))?;
                tracing::info!(task_id = %issue_id, reason = %reason, "Task blocked in taskmaster");
            }
        }
        Ok(())
    }
}
