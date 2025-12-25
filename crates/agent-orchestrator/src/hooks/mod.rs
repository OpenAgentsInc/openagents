pub mod context;
pub mod session;
pub mod todo;
pub mod tool;

pub use context::{CompactionContextHook, ContextInjectionHook, DirectiveInjectionHook};
pub use session::{SessionNotificationHook, SessionRecoveryHook};
pub use todo::{ContextWindowMonitorHook, Priority, TodoContinuationHook, TodoItem, TodoStatus};
pub use tool::{DangerousToolBlockerHook, ToolExecutionLoggerHook, ToolOutputTruncatorHook};

use async_trait::async_trait;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum SessionEvent {
    Created { session_id: String, agent: String },
    Idle { session_id: String },
    Error { session_id: String, error: String },
    Aborted { session_id: String },
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub session_id: String,
}

#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone)]
pub struct ContextBuilder {
    pub sections: Vec<ContextSection>,
}

#[derive(Debug, Clone)]
pub struct ContextSection {
    pub name: String,
    pub content: String,
    pub priority: i32,
}

impl ContextBuilder {
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    pub fn add_section(
        &mut self,
        name: impl Into<String>,
        content: impl Into<String>,
        priority: i32,
    ) {
        self.sections.push(ContextSection {
            name: name.into(),
            content: content.into(),
            priority,
        });
    }

    pub fn build(&self) -> String {
        let mut sorted = self.sections.clone();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
            .into_iter()
            .map(|s| format!("# {}\n\n{}", s.name, s.content))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    }
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum HookResult {
    Continue,
    Block { message: String },
    Modify,
}

impl HookResult {
    pub fn is_blocked(&self) -> bool {
        matches!(self, HookResult::Block { .. })
    }
}

#[async_trait]
pub trait Hook: Send + Sync {
    fn name(&self) -> &str;

    fn priority(&self) -> i32 {
        0
    }

    async fn on_session(&self, _event: &SessionEvent) -> HookResult {
        HookResult::Continue
    }

    async fn before_tool(&self, _call: &mut ToolCall) -> HookResult {
        HookResult::Continue
    }

    async fn after_tool(&self, _call: &ToolCall, _output: &mut ToolOutput) -> HookResult {
        HookResult::Continue
    }

    async fn inject_context(&self, _context: &mut ContextBuilder) -> HookResult {
        HookResult::Continue
    }
}

pub struct HookManager {
    hooks: Vec<Box<dyn Hook>>,
    disabled: std::collections::HashSet<String>,
}

impl HookManager {
    pub fn new() -> Self {
        Self {
            hooks: Vec::new(),
            disabled: std::collections::HashSet::new(),
        }
    }

    pub fn register(&mut self, hook: impl Hook + 'static) {
        self.hooks.push(Box::new(hook));
        self.hooks.sort_by_key(|h| std::cmp::Reverse(h.priority()));
    }

    pub fn disable(&mut self, name: &str) {
        self.disabled.insert(name.to_string());
    }

    pub fn enable(&mut self, name: &str) {
        self.disabled.remove(name);
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        !self.disabled.contains(name)
    }

    pub fn list(&self) -> Vec<&str> {
        self.hooks
            .iter()
            .filter(|h| !self.disabled.contains(h.name()))
            .map(|h| h.name())
            .collect()
    }

    pub async fn dispatch_session(&self, event: &SessionEvent) {
        for hook in &self.hooks {
            if self.disabled.contains(hook.name()) {
                continue;
            }
            let result = hook.on_session(event).await;
            if result.is_blocked() {
                tracing::warn!("Hook {} blocked session event", hook.name());
                break;
            }
        }
    }

    pub async fn dispatch_before_tool(&self, call: &mut ToolCall) -> HookResult {
        for hook in &self.hooks {
            if self.disabled.contains(hook.name()) {
                continue;
            }
            let result = hook.before_tool(call).await;
            if result.is_blocked() {
                return result;
            }
        }
        HookResult::Continue
    }

    pub async fn dispatch_after_tool(&self, call: &ToolCall, output: &mut ToolOutput) {
        for hook in &self.hooks {
            if self.disabled.contains(hook.name()) {
                continue;
            }
            let result = hook.after_tool(call, output).await;
            if result.is_blocked() {
                break;
            }
        }
    }

    pub async fn dispatch_inject_context(&self, context: &mut ContextBuilder) {
        for hook in &self.hooks {
            if self.disabled.contains(hook.name()) {
                continue;
            }
            let _ = hook.inject_context(context).await;
        }
    }
}

impl Default for HookManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHook {
        name: String,
        priority: i32,
    }

    #[async_trait]
    impl Hook for TestHook {
        fn name(&self) -> &str {
            &self.name
        }

        fn priority(&self) -> i32 {
            self.priority
        }
    }

    #[test]
    fn hook_manager_register() {
        let mut manager = HookManager::new();
        manager.register(TestHook {
            name: "test".to_string(),
            priority: 0,
        });
        assert_eq!(manager.list().len(), 1);
    }

    #[test]
    fn hook_manager_disable() {
        let mut manager = HookManager::new();
        manager.register(TestHook {
            name: "test".to_string(),
            priority: 0,
        });
        manager.disable("test");
        assert!(!manager.is_enabled("test"));
        assert!(manager.list().is_empty());
    }

    #[test]
    fn hook_manager_priority_order() {
        let mut manager = HookManager::new();
        manager.register(TestHook {
            name: "low".to_string(),
            priority: 1,
        });
        manager.register(TestHook {
            name: "high".to_string(),
            priority: 10,
        });
        let list = manager.list();
        assert_eq!(list[0], "high");
        assert_eq!(list[1], "low");
    }

    #[test]
    fn context_builder() {
        let mut builder = ContextBuilder::new();
        builder.add_section("Low Priority", "Content A", 1);
        builder.add_section("High Priority", "Content B", 10);
        let result = builder.build();
        assert!(result.starts_with("# High Priority"));
    }

    #[test]
    fn hook_result_blocked() {
        assert!(!HookResult::Continue.is_blocked());
        assert!(
            HookResult::Block {
                message: "test".to_string()
            }
            .is_blocked()
        );
        assert!(!HookResult::Modify.is_blocked());
    }
}
