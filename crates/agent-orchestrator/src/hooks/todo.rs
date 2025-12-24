use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{ContextBuilder, Hook, HookResult, SessionEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
    pub priority: Priority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Medium,
    High,
}

pub struct TodoContinuationHook {
    todos: Arc<RwLock<Vec<TodoItem>>>,
    enforce_completion: bool,
}

impl TodoContinuationHook {
    pub fn new() -> Self {
        Self {
            todos: Arc::new(RwLock::new(Vec::new())),
            enforce_completion: true,
        }
    }

    pub fn without_enforcement(mut self) -> Self {
        self.enforce_completion = false;
        self
    }

    pub async fn add_todo(&self, id: impl Into<String>, content: impl Into<String>, priority: Priority) {
        let mut todos = self.todos.write().await;
        todos.push(TodoItem {
            id: id.into(),
            content: content.into(),
            status: TodoStatus::Pending,
            priority,
        });
    }

    pub async fn set_status(&self, id: &str, status: TodoStatus) -> bool {
        let mut todos = self.todos.write().await;
        if let Some(todo) = todos.iter_mut().find(|t| t.id == id) {
            todo.status = status;
            true
        } else {
            false
        }
    }

    pub async fn get_todos(&self) -> Vec<TodoItem> {
        self.todos.read().await.clone()
    }

    pub async fn pending_count(&self) -> usize {
        self.todos
            .read()
            .await
            .iter()
            .filter(|t| matches!(t.status, TodoStatus::Pending | TodoStatus::InProgress))
            .count()
    }

    pub async fn clear(&self) {
        self.todos.write().await.clear();
    }

    fn format_todo_list(&self, todos: &[TodoItem]) -> String {
        if todos.is_empty() {
            return "No active todos.".to_string();
        }

        let mut lines = Vec::new();
        for todo in todos {
            let status_marker = match todo.status {
                TodoStatus::Pending => "[ ]",
                TodoStatus::InProgress => "[*]",
                TodoStatus::Completed => "[x]",
                TodoStatus::Cancelled => "[-]",
            };
            let priority_marker = match todo.priority {
                Priority::High => "!",
                Priority::Medium => "",
                Priority::Low => ".",
            };
            lines.push(format!(
                "{} {}{} ({})",
                status_marker, priority_marker, todo.content, todo.id
            ));
        }
        lines.join("\n")
    }
}

impl Default for TodoContinuationHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Hook for TodoContinuationHook {
    fn name(&self) -> &str {
        "todo-continuation"
    }

    fn priority(&self) -> i32 {
        200
    }

    async fn on_session(&self, event: &SessionEvent) -> HookResult {
        let SessionEvent::Idle { session_id } = event else {
            return HookResult::Continue;
        };

        if !self.enforce_completion {
            return HookResult::Continue;
        }

        let pending = self.pending_count().await;
        if pending > 0 {
            tracing::warn!(
                session_id = session_id,
                pending_todos = pending,
                "Session idle with pending todos"
            );
            return HookResult::Block {
                message: format!(
                    "Cannot complete session: {} todo item(s) still pending. Please complete all tasks before finishing.",
                    pending
                ),
            };
        }

        HookResult::Continue
    }

    async fn inject_context(&self, context: &mut ContextBuilder) -> HookResult {
        let todos = self.todos.read().await;
        if !todos.is_empty() {
            let formatted = self.format_todo_list(&todos);
            context.add_section("Current Todo List", formatted, 200);
        }
        HookResult::Continue
    }
}

pub struct ContextWindowMonitorHook {
    max_tokens: usize,
    warning_threshold: f32,
    current_usage: Arc<RwLock<usize>>,
}

impl ContextWindowMonitorHook {
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            warning_threshold: 0.8,
            current_usage: Arc::new(RwLock::new(0)),
        }
    }

    pub fn with_warning_threshold(mut self, threshold: f32) -> Self {
        self.warning_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub async fn update_usage(&self, tokens: usize) {
        *self.current_usage.write().await = tokens;
    }

    pub async fn get_usage(&self) -> usize {
        *self.current_usage.read().await
    }

    pub async fn usage_percentage(&self) -> f32 {
        let current = *self.current_usage.read().await;
        current as f32 / self.max_tokens as f32
    }
}

#[async_trait]
impl Hook for ContextWindowMonitorHook {
    fn name(&self) -> &str {
        "context-window-monitor"
    }

    fn priority(&self) -> i32 {
        150
    }

    async fn inject_context(&self, context: &mut ContextBuilder) -> HookResult {
        let usage = self.usage_percentage().await;

        if usage >= self.warning_threshold {
            let percentage = (usage * 100.0) as u32;
            context.add_section(
                "Context Window Warning",
                format!(
                    "WARNING: Context window is {}% full ({}/{}). Consider compacting or starting a new session.",
                    percentage,
                    *self.current_usage.read().await,
                    self.max_tokens
                ),
                250,
            );
        }

        HookResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn todo_hook_add_and_get() {
        let hook = TodoContinuationHook::new();
        hook.add_todo("1", "First task", Priority::High).await;
        hook.add_todo("2", "Second task", Priority::Low).await;

        let todos = hook.get_todos().await;
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].content, "First task");
        assert_eq!(todos[0].priority, Priority::High);
    }

    #[tokio::test]
    async fn todo_hook_set_status() {
        let hook = TodoContinuationHook::new();
        hook.add_todo("1", "Task", Priority::Medium).await;

        let success = hook.set_status("1", TodoStatus::Completed).await;
        assert!(success);

        let todos = hook.get_todos().await;
        assert_eq!(todos[0].status, TodoStatus::Completed);
    }

    #[tokio::test]
    async fn todo_hook_pending_count() {
        let hook = TodoContinuationHook::new();
        hook.add_todo("1", "Task 1", Priority::High).await;
        hook.add_todo("2", "Task 2", Priority::High).await;
        hook.add_todo("3", "Task 3", Priority::High).await;

        assert_eq!(hook.pending_count().await, 3);

        hook.set_status("1", TodoStatus::Completed).await;
        assert_eq!(hook.pending_count().await, 2);

        hook.set_status("2", TodoStatus::InProgress).await;
        assert_eq!(hook.pending_count().await, 2);
    }

    #[tokio::test]
    async fn todo_hook_blocks_with_pending() {
        let hook = TodoContinuationHook::new();
        hook.add_todo("1", "Pending task", Priority::High).await;

        let event = SessionEvent::Idle {
            session_id: "test".to_string(),
        };

        let result = hook.on_session(&event).await;
        assert!(result.is_blocked());
    }

    #[tokio::test]
    async fn todo_hook_allows_when_complete() {
        let hook = TodoContinuationHook::new();
        hook.add_todo("1", "Task", Priority::High).await;
        hook.set_status("1", TodoStatus::Completed).await;

        let event = SessionEvent::Idle {
            session_id: "test".to_string(),
        };

        let result = hook.on_session(&event).await;
        assert!(!result.is_blocked());
    }

    #[tokio::test]
    async fn todo_hook_without_enforcement() {
        let hook = TodoContinuationHook::new().without_enforcement();
        hook.add_todo("1", "Pending task", Priority::High).await;

        let event = SessionEvent::Idle {
            session_id: "test".to_string(),
        };

        let result = hook.on_session(&event).await;
        assert!(!result.is_blocked());
    }

    #[tokio::test]
    async fn todo_hook_injects_context() {
        let hook = TodoContinuationHook::new();
        hook.add_todo("1", "First task", Priority::High).await;
        hook.add_todo("2", "Second task", Priority::Low).await;

        let mut context = ContextBuilder::new();
        hook.inject_context(&mut context).await;

        let built = context.build();
        assert!(built.contains("Current Todo List"));
        assert!(built.contains("First task"));
        assert!(built.contains("Second task"));
    }

    #[tokio::test]
    async fn context_monitor_tracks_usage() {
        let hook = ContextWindowMonitorHook::new(100_000);
        hook.update_usage(50_000).await;

        assert_eq!(hook.get_usage().await, 50_000);
        assert!((hook.usage_percentage().await - 0.5).abs() < 0.01);
    }

    #[tokio::test]
    async fn context_monitor_warns_at_threshold() {
        let hook = ContextWindowMonitorHook::new(100_000).with_warning_threshold(0.8);
        hook.update_usage(85_000).await;

        let mut context = ContextBuilder::new();
        hook.inject_context(&mut context).await;

        let built = context.build();
        assert!(built.contains("WARNING"));
        assert!(built.contains("85%"));
    }

    #[tokio::test]
    async fn context_monitor_no_warning_below_threshold() {
        let hook = ContextWindowMonitorHook::new(100_000).with_warning_threshold(0.8);
        hook.update_usage(50_000).await;

        let mut context = ContextBuilder::new();
        hook.inject_context(&mut context).await;

        let built = context.build();
        assert!(!built.contains("WARNING"));
    }

    #[tokio::test]
    async fn todo_clear() {
        let hook = TodoContinuationHook::new();
        hook.add_todo("1", "Task", Priority::High).await;
        hook.add_todo("2", "Task 2", Priority::Low).await;

        hook.clear().await;
        assert_eq!(hook.get_todos().await.len(), 0);
    }
}
