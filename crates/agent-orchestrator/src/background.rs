//! Background task management for agent orchestration.
//!
//! This module provides the `BackgroundTaskManager` for spawning, tracking,
//! and managing background agent tasks. It supports the `background_task`,
//! `background_output`, and `background_cancel` tool patterns.

use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Unique identifier for a background task.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(String);

impl TaskId {
    /// Create a new random task ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create a task ID from a string.
    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the task ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Session identifier type alias.
pub type SessionId = String;

/// Status of a background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    /// Task has been created but not yet started.
    Pending,
    /// Task is currently running.
    Running,
    /// Task completed successfully.
    Completed,
    /// Task failed with an error.
    Error,
    /// Task was cancelled.
    Cancelled,
}

impl TaskStatus {
    /// Returns true if the task is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Error | Self::Cancelled)
    }

    /// Returns true if the task is still active.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Pending | Self::Running)
    }
}

/// A background task spawned by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundTask {
    /// Unique task identifier.
    pub id: TaskId,
    /// Session ID of this background task.
    pub session_id: SessionId,
    /// Session ID of the parent that spawned this task.
    pub parent_session_id: SessionId,
    /// Short description of what this task does.
    pub description: String,
    /// Agent type handling this task.
    pub agent: String,
    /// Prompt sent to the agent.
    pub prompt: String,
    /// Current status of the task.
    pub status: TaskStatus,
    /// When the task was created.
    pub created_at: DateTime<Utc>,
    /// When the task started running.
    pub started_at: Option<DateTime<Utc>>,
    /// When the task completed (successfully or with error).
    pub completed_at: Option<DateTime<Utc>>,
    /// Result output if task completed successfully.
    pub result: Option<String>,
    /// Error message if task failed.
    pub error: Option<String>,
}

impl BackgroundTask {
    /// Create a new pending background task.
    pub fn new(
        parent_session_id: impl Into<String>,
        agent: impl Into<String>,
        prompt: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: TaskId::new(),
            session_id: Uuid::new_v4().to_string(),
            parent_session_id: parent_session_id.into(),
            description: description.into(),
            agent: agent.into(),
            prompt: prompt.into(),
            status: TaskStatus::Pending,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
        }
    }

    /// Mark the task as running.
    pub fn mark_running(&mut self) {
        self.status = TaskStatus::Running;
        self.started_at = Some(Utc::now());
    }

    /// Mark the task as completed with a result.
    pub fn mark_completed(&mut self, result: impl Into<String>) {
        self.status = TaskStatus::Completed;
        self.completed_at = Some(Utc::now());
        self.result = Some(result.into());
    }

    /// Mark the task as failed with an error.
    pub fn mark_error(&mut self, error: impl Into<String>) {
        self.status = TaskStatus::Error;
        self.completed_at = Some(Utc::now());
        self.error = Some(error.into());
    }

    /// Mark the task as cancelled.
    pub fn mark_cancelled(&mut self) {
        self.status = TaskStatus::Cancelled;
        self.completed_at = Some(Utc::now());
    }

    /// Get the duration of the task if it has started.
    pub fn duration(&self) -> Option<chrono::Duration> {
        let start = self.started_at?;
        let end = self.completed_at.unwrap_or_else(Utc::now);
        Some(end - start)
    }
}

/// Manager for background agent tasks.
///
/// The `BackgroundTaskManager` handles spawning, tracking, and managing
/// background tasks. It provides thread-safe access to task state and
/// supports blocking and non-blocking output retrieval.
///
/// # Example
///
/// ```rust,ignore
/// use agent_orchestrator::background::BackgroundTaskManager;
///
/// let manager = BackgroundTaskManager::new();
///
/// // Spawn a background task
/// let task_id = manager.spawn(
///     "parent-session-123",
///     "explore",
///     "Find all authentication code",
///     "Find auth patterns"
/// ).await?;
///
/// // Check if task is complete
/// if let Some(result) = manager.get_output(&task_id, false).await? {
///     println!("Task completed: {}", result);
/// }
/// ```
pub struct BackgroundTaskManager {
    tasks: Arc<RwLock<HashMap<TaskId, BackgroundTask>>>,
}

impl BackgroundTaskManager {
    /// Create a new background task manager.
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Spawn a new background task.
    ///
    /// Creates and registers a new background task. The task starts in
    /// `Pending` status. The caller is responsible for actually executing
    /// the task and updating its status.
    ///
    /// Returns the `TaskId` that can be used to track the task.
    pub async fn spawn(
        &self,
        parent_session_id: &str,
        agent: &str,
        prompt: &str,
        description: &str,
    ) -> Result<TaskId> {
        let task = BackgroundTask::new(parent_session_id, agent, prompt, description);
        let task_id = task.id.clone();

        let mut tasks = self.tasks.write().await;
        tasks.insert(task_id.clone(), task);

        tracing::info!(
            task_id = %task_id,
            agent = agent,
            description = description,
            "Background task spawned"
        );

        Ok(task_id)
    }

    /// Get the output of a background task.
    ///
    /// If `block` is true, waits for the task to complete before returning.
    /// If `block` is false, returns `None` if the task is not yet complete.
    ///
    /// Returns:
    /// - `Ok(Some(result))` if task completed successfully
    /// - `Ok(None)` if task is not complete and `block` is false
    /// - `Err(Error::TaskNotFound)` if task doesn't exist
    /// - `Err(Error::TaskFailed)` if task failed with an error
    /// - `Err(Error::TaskCancelled)` if task was cancelled
    pub async fn get_output(&self, task_id: &TaskId, block: bool) -> Result<Option<String>> {
        if block {
            self.wait_for_completion(task_id).await
        } else {
            self.try_get_output(task_id).await
        }
    }

    /// Get the output of a background task, but stop waiting after a timeout.
    ///
    /// Returns:
    /// - `Ok(Some(result))` if task completed successfully before the timeout
    /// - `Ok(None)` if the timeout elapsed before the task completed
    /// - `Err(Error::TaskNotFound)` if task doesn't exist
    /// - `Err(Error::TaskFailed)` if task failed with an error
    /// - `Err(Error::TaskCancelled)` if task was cancelled
    pub async fn get_output_with_timeout(
        &self,
        task_id: &TaskId,
        timeout: std::time::Duration,
    ) -> Result<Option<String>> {
        match tokio::time::timeout(timeout, self.wait_for_completion(task_id)).await {
            Ok(result) => result,
            Err(_) => Ok(None),
        }
    }

    /// Try to get task output without blocking.
    async fn try_get_output(&self, task_id: &TaskId) -> Result<Option<String>> {
        let tasks = self.tasks.read().await;
        let task = tasks
            .get(task_id)
            .ok_or(Error::TaskNotFound(task_id.to_string()))?;

        match task.status {
            TaskStatus::Completed => Ok(task.result.clone()),
            TaskStatus::Error => Err(Error::TaskFailed(
                task.error
                    .clone()
                    .unwrap_or_else(|| "Unknown error".to_string()),
            )),
            TaskStatus::Cancelled => Err(Error::TaskCancelled(task_id.to_string())),
            TaskStatus::Pending | TaskStatus::Running => Ok(None),
        }
    }

    /// Wait for a task to complete, polling periodically.
    async fn wait_for_completion(&self, task_id: &TaskId) -> Result<Option<String>> {
        loop {
            let result = self.try_get_output(task_id).await?;
            if result.is_some() {
                return Ok(result);
            }

            // Check if task is still active
            {
                let tasks = self.tasks.read().await;
                if let Some(task) = tasks.get(task_id) {
                    if task.status.is_terminal() {
                        // Task is in terminal state, return the result
                        return self.try_get_output(task_id).await;
                    }
                } else {
                    return Err(Error::TaskNotFound(task_id.to_string()));
                }
            }

            // Wait a bit before polling again
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    /// Cancel a running task.
    ///
    /// Returns `Ok(())` if the task was cancelled or was already in a terminal state.
    /// Returns `Err(Error::TaskNotFound)` if the task doesn't exist.
    pub async fn cancel(&self, task_id: &TaskId) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or(Error::TaskNotFound(task_id.to_string()))?;

        if task.status.is_active() {
            task.mark_cancelled();
            tracing::info!(task_id = %task_id, "Background task cancelled");
        }

        Ok(())
    }

    /// Cancel all tasks for a parent session.
    ///
    /// Returns the number of tasks that were cancelled.
    pub async fn cancel_all(&self, parent_session_id: &str) -> Result<usize> {
        let mut tasks = self.tasks.write().await;
        let mut cancelled = 0;

        for task in tasks.values_mut() {
            if task.parent_session_id == parent_session_id && task.status.is_active() {
                task.mark_cancelled();
                cancelled += 1;
            }
        }

        if cancelled > 0 {
            tracing::info!(
                parent_session_id = parent_session_id,
                count = cancelled,
                "Background tasks cancelled"
            );
        }

        Ok(cancelled)
    }

    /// List all tasks for a parent session.
    pub async fn list(&self, parent_session_id: &str) -> Vec<BackgroundTask> {
        let tasks = self.tasks.read().await;
        tasks
            .values()
            .filter(|t| t.parent_session_id == parent_session_id)
            .cloned()
            .collect()
    }

    /// List all active tasks for a parent session.
    pub async fn list_active(&self, parent_session_id: &str) -> Vec<BackgroundTask> {
        let tasks = self.tasks.read().await;
        tasks
            .values()
            .filter(|t| t.parent_session_id == parent_session_id && t.status.is_active())
            .cloned()
            .collect()
    }

    /// Get a specific task by ID.
    pub async fn get(&self, task_id: &TaskId) -> Option<BackgroundTask> {
        let tasks = self.tasks.read().await;
        tasks.get(task_id).cloned()
    }

    /// Update a task's status to running.
    pub async fn mark_running(&self, task_id: &TaskId) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or(Error::TaskNotFound(task_id.to_string()))?;
        task.mark_running();
        Ok(())
    }

    /// Update a task with a successful result.
    pub async fn complete(&self, task_id: &TaskId, result: impl Into<String>) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or(Error::TaskNotFound(task_id.to_string()))?;
        task.mark_completed(result);
        tracing::info!(task_id = %task_id, "Background task completed");
        Ok(())
    }

    /// Update a task with an error.
    pub async fn fail(&self, task_id: &TaskId, error: impl Into<String>) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or(Error::TaskNotFound(task_id.to_string()))?;
        task.mark_error(error);
        tracing::warn!(task_id = %task_id, "Background task failed");
        Ok(())
    }

    /// Remove completed/cancelled tasks older than the given duration.
    pub async fn cleanup(&self, max_age: chrono::Duration) -> usize {
        let mut tasks = self.tasks.write().await;
        let now = Utc::now();
        let initial_count = tasks.len();

        tasks.retain(|_, task| {
            if let Some(completed_at) = task.completed_at {
                now - completed_at < max_age
            } else {
                true // Keep active tasks
            }
        });

        initial_count - tasks.len()
    }

    /// Get the total number of tasks.
    pub async fn count(&self) -> usize {
        self.tasks.read().await.len()
    }

    /// Get the number of active tasks.
    pub async fn active_count(&self) -> usize {
        self.tasks
            .read()
            .await
            .values()
            .filter(|t| t.status.is_active())
            .count()
    }
}

impl Default for BackgroundTaskManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for BackgroundTaskManager {
    fn clone(&self) -> Self {
        Self {
            tasks: Arc::clone(&self.tasks),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn task_id_new() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn task_id_from_string() {
        let id = TaskId::from_string("test-id");
        assert_eq!(id.as_str(), "test-id");
    }

    #[test]
    fn task_status_terminal() {
        assert!(!TaskStatus::Pending.is_terminal());
        assert!(!TaskStatus::Running.is_terminal());
        assert!(TaskStatus::Completed.is_terminal());
        assert!(TaskStatus::Error.is_terminal());
        assert!(TaskStatus::Cancelled.is_terminal());
    }

    #[test]
    fn task_status_active() {
        assert!(TaskStatus::Pending.is_active());
        assert!(TaskStatus::Running.is_active());
        assert!(!TaskStatus::Completed.is_active());
        assert!(!TaskStatus::Error.is_active());
        assert!(!TaskStatus::Cancelled.is_active());
    }

    #[test]
    fn background_task_new() {
        let task = BackgroundTask::new("parent-123", "explore", "Find code", "Find patterns");
        assert_eq!(task.parent_session_id, "parent-123");
        assert_eq!(task.agent, "explore");
        assert_eq!(task.prompt, "Find code");
        assert_eq!(task.description, "Find patterns");
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.started_at.is_none());
        assert!(task.completed_at.is_none());
    }

    #[test]
    fn background_task_lifecycle() {
        let mut task = BackgroundTask::new("parent", "agent", "prompt", "desc");

        // Start running
        task.mark_running();
        assert_eq!(task.status, TaskStatus::Running);
        assert!(task.started_at.is_some());

        // Complete
        task.mark_completed("result");
        assert_eq!(task.status, TaskStatus::Completed);
        assert!(task.completed_at.is_some());
        assert_eq!(task.result, Some("result".to_string()));
    }

    #[test]
    fn background_task_error() {
        let mut task = BackgroundTask::new("parent", "agent", "prompt", "desc");
        task.mark_running();
        task.mark_error("something went wrong");
        assert_eq!(task.status, TaskStatus::Error);
        assert_eq!(task.error, Some("something went wrong".to_string()));
    }

    #[test]
    fn background_task_cancel() {
        let mut task = BackgroundTask::new("parent", "agent", "prompt", "desc");
        task.mark_cancelled();
        assert_eq!(task.status, TaskStatus::Cancelled);
        assert!(task.completed_at.is_some());
    }

    #[tokio::test]
    async fn manager_spawn() {
        let manager = BackgroundTaskManager::new();
        let task_id = manager
            .spawn("parent-123", "explore", "Find code", "Find patterns")
            .await
            .unwrap();

        let task = manager.get(&task_id).await.unwrap();
        assert_eq!(task.parent_session_id, "parent-123");
        assert_eq!(task.status, TaskStatus::Pending);
    }

    #[tokio::test]
    async fn manager_spawn_records_agent_details() {
        let manager = BackgroundTaskManager::new();
        let task_id = manager
            .spawn(
                "parent-xyz",
                "oracle",
                "Review architecture",
                "Design review",
            )
            .await
            .unwrap();

        let task = manager.get(&task_id).await.expect("task exists");
        assert_eq!(task.agent, "oracle");
        assert_eq!(task.prompt, "Review architecture");
        assert_eq!(task.description, "Design review");
    }

    #[tokio::test]
    async fn manager_get_output_blocks_until_complete() {
        let manager = BackgroundTaskManager::new();
        let task_id = manager
            .spawn("parent", "agent", "prompt", "desc")
            .await
            .unwrap();

        let manager_clone = manager.clone();
        let task_id_clone = task_id.clone();
        tokio::spawn(async move {
            manager_clone.mark_running(&task_id_clone).await.unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;
            manager_clone
                .complete(&task_id_clone, "done")
                .await
                .unwrap();
        });

        let result = manager.get_output(&task_id, true).await.unwrap();
        assert_eq!(result, Some("done".to_string()));
    }

    #[tokio::test]
    async fn manager_get_output_times_out() {
        let manager = BackgroundTaskManager::new();
        let task_id = manager
            .spawn("parent", "agent", "prompt", "desc")
            .await
            .unwrap();

        manager.mark_running(&task_id).await.unwrap();
        let result = manager
            .get_output_with_timeout(&task_id, Duration::from_millis(20))
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn manager_get_output_completes_before_timeout() {
        let manager = BackgroundTaskManager::new();
        let task_id = manager
            .spawn("parent", "agent", "prompt", "desc")
            .await
            .unwrap();

        let manager_clone = manager.clone();
        let task_id_clone = task_id.clone();
        tokio::spawn(async move {
            manager_clone.mark_running(&task_id_clone).await.unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
            manager_clone
                .complete(&task_id_clone, "done")
                .await
                .unwrap();
        });

        let result = manager
            .get_output_with_timeout(&task_id, Duration::from_millis(200))
            .await
            .unwrap();
        assert_eq!(result, Some("done".to_string()));
    }

    #[tokio::test]
    async fn manager_complete() {
        let manager = BackgroundTaskManager::new();
        let task_id = manager
            .spawn("parent", "agent", "prompt", "desc")
            .await
            .unwrap();

        manager.mark_running(&task_id).await.unwrap();
        manager.complete(&task_id, "done").await.unwrap();

        let result = manager.get_output(&task_id, false).await.unwrap();
        assert_eq!(result, Some("done".to_string()));
    }

    #[tokio::test]
    async fn manager_fail() {
        let manager = BackgroundTaskManager::new();
        let task_id = manager
            .spawn("parent", "agent", "prompt", "desc")
            .await
            .unwrap();

        manager.mark_running(&task_id).await.unwrap();
        manager.fail(&task_id, "oops").await.unwrap();

        let result = manager.get_output(&task_id, false).await;
        assert!(matches!(result, Err(Error::TaskFailed(_))));
    }

    #[tokio::test]
    async fn manager_cancel() {
        let manager = BackgroundTaskManager::new();
        let task_id = manager
            .spawn("parent", "agent", "prompt", "desc")
            .await
            .unwrap();

        manager.cancel(&task_id).await.unwrap();

        let result = manager.get_output(&task_id, false).await;
        assert!(matches!(result, Err(Error::TaskCancelled(_))));
    }

    #[tokio::test]
    async fn manager_cancel_all() {
        let manager = BackgroundTaskManager::new();

        // Spawn multiple tasks for same parent
        manager
            .spawn("parent-1", "agent", "prompt1", "desc1")
            .await
            .unwrap();
        manager
            .spawn("parent-1", "agent", "prompt2", "desc2")
            .await
            .unwrap();
        manager
            .spawn("parent-2", "agent", "prompt3", "desc3")
            .await
            .unwrap();

        let cancelled = manager.cancel_all("parent-1").await.unwrap();
        assert_eq!(cancelled, 2);

        // parent-2's task should still be active
        let active = manager.list_active("parent-2").await;
        assert_eq!(active.len(), 1);
    }

    #[tokio::test]
    async fn manager_list() {
        let manager = BackgroundTaskManager::new();

        manager
            .spawn("parent-1", "agent", "prompt1", "desc1")
            .await
            .unwrap();
        manager
            .spawn("parent-1", "agent", "prompt2", "desc2")
            .await
            .unwrap();
        manager
            .spawn("parent-2", "agent", "prompt3", "desc3")
            .await
            .unwrap();

        let tasks = manager.list("parent-1").await;
        assert_eq!(tasks.len(), 2);

        let tasks = manager.list("parent-2").await;
        assert_eq!(tasks.len(), 1);
    }

    #[tokio::test]
    async fn manager_count() {
        let manager = BackgroundTaskManager::new();

        assert_eq!(manager.count().await, 0);
        assert_eq!(manager.active_count().await, 0);

        let id1 = manager
            .spawn("parent", "agent", "prompt1", "desc1")
            .await
            .unwrap();
        let _id2 = manager
            .spawn("parent", "agent", "prompt2", "desc2")
            .await
            .unwrap();

        assert_eq!(manager.count().await, 2);
        assert_eq!(manager.active_count().await, 2);

        manager.complete(&id1, "done").await.unwrap();
        assert_eq!(manager.count().await, 2);
        assert_eq!(manager.active_count().await, 1);
    }

    #[tokio::test]
    async fn manager_task_not_found() {
        let manager = BackgroundTaskManager::new();
        let fake_id = TaskId::from_string("nonexistent");

        let result = manager.get_output(&fake_id, false).await;
        assert!(matches!(result, Err(Error::TaskNotFound(_))));
    }

    #[tokio::test]
    async fn manager_clone_shares_state() {
        let manager1 = BackgroundTaskManager::new();
        let manager2 = manager1.clone();

        let task_id = manager1
            .spawn("parent", "agent", "prompt", "desc")
            .await
            .unwrap();

        // Both managers should see the same task
        let task = manager2.get(&task_id).await;
        assert!(task.is_some());
    }

    #[test]
    fn task_serialize() {
        let task = BackgroundTask::new("parent", "agent", "prompt", "desc");
        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("parent"));
        assert!(json.contains("agent"));
    }

    #[test]
    fn task_deserialize() {
        let task = BackgroundTask::new("parent", "agent", "prompt", "desc");
        let json = serde_json::to_string(&task).unwrap();
        let restored: BackgroundTask = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.parent_session_id, "parent");
        assert_eq!(restored.agent, "agent");
    }
}
