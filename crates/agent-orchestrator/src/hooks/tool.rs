use async_trait::async_trait;

use super::{Hook, HookResult, ToolCall, ToolOutput};

type BeforeToolCallback = Box<dyn Fn(&ToolCall) + Send + Sync>;
type AfterToolCallback = Box<dyn Fn(&ToolCall, &ToolOutput) + Send + Sync>;

pub struct ToolOutputTruncatorHook {
    max_length: usize,
    truncation_message: String,
}

impl ToolOutputTruncatorHook {
    pub fn new(max_length: usize) -> Self {
        Self {
            max_length,
            truncation_message: format!("\n\n... (output truncated at {} characters)", max_length),
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.truncation_message = message.into();
        self
    }
}

impl Default for ToolOutputTruncatorHook {
    fn default() -> Self {
        Self::new(30000)
    }
}

#[async_trait]
impl Hook for ToolOutputTruncatorHook {
    fn name(&self) -> &str {
        "tool-output-truncator"
    }

    fn priority(&self) -> i32 {
        50
    }

    async fn after_tool(&self, _call: &ToolCall, output: &mut ToolOutput) -> HookResult {
        if output.content.len() > self.max_length {
            let truncated = &output.content[..self.max_length];
            output.content = format!("{}{}", truncated, self.truncation_message);
            tracing::debug!(
                original_len = output.content.len() + self.truncation_message.len(),
                truncated_len = self.max_length,
                "Tool output truncated"
            );
            HookResult::Modify
        } else {
            HookResult::Continue
        }
    }
}

pub struct ToolExecutionLoggerHook {
    on_before: Option<BeforeToolCallback>,
    on_after: Option<AfterToolCallback>,
}

impl ToolExecutionLoggerHook {
    pub fn new() -> Self {
        Self {
            on_before: None,
            on_after: None,
        }
    }

    pub fn on_before<F>(mut self, callback: F) -> Self
    where
        F: Fn(&ToolCall) + Send + Sync + 'static,
    {
        self.on_before = Some(Box::new(callback));
        self
    }

    pub fn on_after<F>(mut self, callback: F) -> Self
    where
        F: Fn(&ToolCall, &ToolOutput) + Send + Sync + 'static,
    {
        self.on_after = Some(Box::new(callback));
        self
    }
}

impl Default for ToolExecutionLoggerHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Hook for ToolExecutionLoggerHook {
    fn name(&self) -> &str {
        "tool-execution-logger"
    }

    fn priority(&self) -> i32 {
        1
    }

    async fn before_tool(&self, call: &mut ToolCall) -> HookResult {
        tracing::debug!(
            tool = call.name,
            session_id = call.session_id,
            "Tool execution starting"
        );

        if let Some(ref callback) = self.on_before {
            callback(call);
        }

        HookResult::Continue
    }

    async fn after_tool(&self, call: &ToolCall, output: &mut ToolOutput) -> HookResult {
        tracing::debug!(
            tool = call.name,
            session_id = call.session_id,
            is_error = output.is_error,
            output_len = output.content.len(),
            "Tool execution completed"
        );

        if let Some(ref callback) = self.on_after {
            callback(call, output);
        }

        HookResult::Continue
    }
}

pub struct DangerousToolBlockerHook {
    blocked_tools: Vec<String>,
    blocked_patterns: Vec<String>,
}

impl DangerousToolBlockerHook {
    pub fn new() -> Self {
        Self {
            blocked_tools: Vec::new(),
            blocked_patterns: vec![
                "rm -rf /".to_string(),
                "sudo".to_string(),
                "> /dev/".to_string(),
            ],
        }
    }

    pub fn block_tool(mut self, tool_name: impl Into<String>) -> Self {
        self.blocked_tools.push(tool_name.into());
        self
    }

    pub fn block_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.blocked_patterns.push(pattern.into());
        self
    }
}

impl Default for DangerousToolBlockerHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Hook for DangerousToolBlockerHook {
    fn name(&self) -> &str {
        "dangerous-tool-blocker"
    }

    fn priority(&self) -> i32 {
        1000
    }

    async fn before_tool(&self, call: &mut ToolCall) -> HookResult {
        if self.blocked_tools.contains(&call.name) {
            return HookResult::Block {
                message: format!("Tool '{}' is blocked by security policy", call.name),
            };
        }

        let is_bash = call.name == "bash" || call.name == "Bash";
        if !is_bash {
            return HookResult::Continue;
        }

        let Some(command) = call.parameters.get("command").and_then(|v| v.as_str()) else {
            return HookResult::Continue;
        };

        for pattern in &self.blocked_patterns {
            if command.contains(pattern) {
                return HookResult::Block {
                    message: format!("Command contains blocked pattern: '{}'", pattern),
                };
            }
        }

        HookResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    fn make_tool_call(name: &str) -> ToolCall {
        ToolCall {
            name: name.to_string(),
            parameters: HashMap::new(),
            session_id: "test-session".to_string(),
        }
    }

    fn make_bash_call(command: &str) -> ToolCall {
        let mut params = HashMap::new();
        params.insert("command".to_string(), serde_json::json!(command));
        ToolCall {
            name: "bash".to_string(),
            parameters: params,
            session_id: "test-session".to_string(),
        }
    }

    #[tokio::test]
    async fn truncator_leaves_short_output() {
        let hook = ToolOutputTruncatorHook::new(100);
        let call = make_tool_call("test");
        let mut output = ToolOutput {
            content: "short output".to_string(),
            is_error: false,
        };

        let result = hook.after_tool(&call, &mut output).await;
        assert!(matches!(result, HookResult::Continue));
        assert_eq!(output.content, "short output");
    }

    #[tokio::test]
    async fn truncator_truncates_long_output() {
        let hook = ToolOutputTruncatorHook::new(10);
        let call = make_tool_call("test");
        let mut output = ToolOutput {
            content: "this is a very long output that should be truncated".to_string(),
            is_error: false,
        };

        let result = hook.after_tool(&call, &mut output).await;
        assert!(matches!(result, HookResult::Modify));
        assert!(output.content.starts_with("this is a "));
        assert!(output.content.contains("truncated"));
    }

    #[tokio::test]
    async fn truncator_custom_message() {
        let hook = ToolOutputTruncatorHook::new(10).with_message(" [TRUNCATED]");
        let call = make_tool_call("test");
        let mut output = ToolOutput {
            content: "this is a very long output".to_string(),
            is_error: false,
        };

        hook.after_tool(&call, &mut output).await;
        assert!(output.content.ends_with(" [TRUNCATED]"));
    }

    #[tokio::test]
    async fn logger_calls_callbacks() {
        let before_called = Arc::new(AtomicBool::new(false));
        let after_called = Arc::new(AtomicBool::new(false));
        let before_clone = before_called.clone();
        let after_clone = after_called.clone();

        let hook = ToolExecutionLoggerHook::new()
            .on_before(move |_| before_clone.store(true, Ordering::SeqCst))
            .on_after(move |_, _| after_clone.store(true, Ordering::SeqCst));

        let mut call = make_tool_call("test");
        let mut output = ToolOutput {
            content: "result".to_string(),
            is_error: false,
        };

        hook.before_tool(&mut call).await;
        assert!(before_called.load(Ordering::SeqCst));

        hook.after_tool(&call, &mut output).await;
        assert!(after_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn blocker_allows_safe_tools() {
        let hook = DangerousToolBlockerHook::new();
        let mut call = make_tool_call("read");

        let result = hook.before_tool(&mut call).await;
        assert!(!result.is_blocked());
    }

    #[tokio::test]
    async fn blocker_blocks_dangerous_tools() {
        let hook = DangerousToolBlockerHook::new().block_tool("dangerous_tool");
        let mut call = make_tool_call("dangerous_tool");

        let result = hook.before_tool(&mut call).await;
        assert!(result.is_blocked());
    }

    #[tokio::test]
    async fn blocker_blocks_dangerous_commands() {
        let hook = DangerousToolBlockerHook::new();
        let mut call = make_bash_call("rm -rf / --no-preserve-root");

        let result = hook.before_tool(&mut call).await;
        assert!(result.is_blocked());
    }

    #[tokio::test]
    async fn blocker_allows_safe_commands() {
        let hook = DangerousToolBlockerHook::new();
        let mut call = make_bash_call("ls -la");

        let result = hook.before_tool(&mut call).await;
        assert!(!result.is_blocked());
    }

    #[tokio::test]
    async fn blocker_custom_pattern() {
        let hook = DangerousToolBlockerHook::new().block_pattern("dangerous_pattern");
        let mut call = make_bash_call("echo dangerous_pattern");

        let result = hook.before_tool(&mut call).await;
        assert!(result.is_blocked());
    }

    #[tokio::test]
    async fn tool_call_count_tracking() {
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();

        let hook = ToolExecutionLoggerHook::new().on_before(move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        let mut call = make_tool_call("test");
        hook.before_tool(&mut call).await;
        hook.before_tool(&mut call).await;
        hook.before_tool(&mut call).await;

        assert_eq!(count.load(Ordering::SeqCst), 3);
    }
}
