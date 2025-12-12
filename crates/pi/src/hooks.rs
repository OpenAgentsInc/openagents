//! Hooks system for extending Pi agent behavior
//!
//! Hooks allow custom code to run at various points in the agent lifecycle:
//! - Before/after tool execution
//! - Before/after LLM calls
//! - On turn completion
//! - On session events

use async_trait::async_trait;
use llm::Usage;
use serde_json::Value;

/// Result of a hook execution
#[derive(Debug, Clone)]
pub enum HookResult {
    /// Continue with the original operation
    Continue,

    /// Skip the operation (e.g., skip tool execution)
    Skip { reason: String },

    /// Modify the input and continue
    ModifyInput(Value),

    /// Return a custom result instead of executing
    CustomResult { output: String, is_error: bool },
}

impl Default for HookResult {
    fn default() -> Self {
        Self::Continue
    }
}

/// Hook trait for extending agent behavior
///
/// Implement this trait to add custom behaviors at various agent lifecycle points.
/// All methods have default implementations that do nothing, so you only need to
/// implement the hooks you care about.
#[async_trait]
pub trait PiHook: Send + Sync {
    /// Called before a tool is executed
    ///
    /// Return `HookResult::Skip` to prevent execution, or
    /// `HookResult::ModifyInput` to change the input.
    async fn before_tool_call(
        &self,
        _tool_name: &str,
        _input: &Value,
    ) -> HookResult {
        HookResult::Continue
    }

    /// Called after a tool is executed
    async fn after_tool_call(
        &self,
        _tool_name: &str,
        _input: &Value,
        _output: &str,
        _is_error: bool,
    ) {
    }

    /// Called before an LLM request
    async fn before_llm_call(&self, _messages_count: usize) -> HookResult {
        HookResult::Continue
    }

    /// Called after an LLM response is complete
    async fn after_llm_call(&self, _usage: &Usage) {}

    /// Called when a turn completes
    async fn on_turn_complete(&self, _turn: u32, _usage: &Usage, _cost_usd: f64) {}

    /// Called when the agent session starts
    async fn on_session_start(&self, _session_id: &str) {}

    /// Called when the agent session ends
    async fn on_session_end(&self, _session_id: &str, _total_turns: u32, _total_cost_usd: f64) {}

    /// Called when an error occurs
    async fn on_error(&self, _error: &str, _retryable: bool) {}

    /// Called when the agent is cancelled
    async fn on_cancelled(&self) {}
}

/// A collection of hooks
pub struct HookRegistry {
    hooks: Vec<Box<dyn PiHook>>,
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HookRegistry {
    /// Create a new empty hook registry
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// Add a hook to the registry
    pub fn add(&mut self, hook: impl PiHook + 'static) {
        self.hooks.push(Box::new(hook));
    }

    /// Run before_tool_call hooks
    ///
    /// Returns the first non-Continue result, or Continue if all hooks continue.
    pub async fn before_tool_call(&self, tool_name: &str, input: &Value) -> HookResult {
        for hook in &self.hooks {
            let result = hook.before_tool_call(tool_name, input).await;
            if !matches!(result, HookResult::Continue) {
                return result;
            }
        }
        HookResult::Continue
    }

    /// Run after_tool_call hooks
    pub async fn after_tool_call(
        &self,
        tool_name: &str,
        input: &Value,
        output: &str,
        is_error: bool,
    ) {
        for hook in &self.hooks {
            hook.after_tool_call(tool_name, input, output, is_error).await;
        }
    }

    /// Run before_llm_call hooks
    pub async fn before_llm_call(&self, messages_count: usize) -> HookResult {
        for hook in &self.hooks {
            let result = hook.before_llm_call(messages_count).await;
            if !matches!(result, HookResult::Continue) {
                return result;
            }
        }
        HookResult::Continue
    }

    /// Run after_llm_call hooks
    pub async fn after_llm_call(&self, usage: &Usage) {
        for hook in &self.hooks {
            hook.after_llm_call(usage).await;
        }
    }

    /// Run on_turn_complete hooks
    pub async fn on_turn_complete(&self, turn: u32, usage: &Usage, cost_usd: f64) {
        for hook in &self.hooks {
            hook.on_turn_complete(turn, usage, cost_usd).await;
        }
    }

    /// Run on_session_start hooks
    pub async fn on_session_start(&self, session_id: &str) {
        for hook in &self.hooks {
            hook.on_session_start(session_id).await;
        }
    }

    /// Run on_session_end hooks
    pub async fn on_session_end(&self, session_id: &str, total_turns: u32, total_cost_usd: f64) {
        for hook in &self.hooks {
            hook.on_session_end(session_id, total_turns, total_cost_usd).await;
        }
    }

    /// Run on_error hooks
    pub async fn on_error(&self, error: &str, retryable: bool) {
        for hook in &self.hooks {
            hook.on_error(error, retryable).await;
        }
    }

    /// Run on_cancelled hooks
    pub async fn on_cancelled(&self) {
        for hook in &self.hooks {
            hook.on_cancelled().await;
        }
    }

    /// Get the number of registered hooks
    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    /// Check if no hooks are registered
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }
}

/// A simple logging hook for debugging
pub struct LoggingHook {
    /// Log level (for filtering)
    pub level: LogLevel,
}

/// Log levels for the logging hook
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Log everything
    Verbose,
    /// Log tool calls and turns
    Normal,
    /// Only log errors
    Errors,
}

impl Default for LoggingHook {
    fn default() -> Self {
        Self {
            level: LogLevel::Normal,
        }
    }
}

#[async_trait]
impl PiHook for LoggingHook {
    async fn before_tool_call(&self, tool_name: &str, input: &Value) -> HookResult {
        if self.level == LogLevel::Verbose {
            tracing::debug!(tool = tool_name, input = ?input, "Tool call starting");
        }
        HookResult::Continue
    }

    async fn after_tool_call(&self, tool_name: &str, _input: &Value, output: &str, is_error: bool) {
        if self.level == LogLevel::Verbose || (is_error && self.level != LogLevel::Errors) {
            let output_preview = if output.len() > 200 {
                format!("{}...", &output[..200])
            } else {
                output.to_string()
            };
            tracing::debug!(
                tool = tool_name,
                is_error = is_error,
                output = output_preview,
                "Tool call completed"
            );
        }
    }

    async fn on_turn_complete(&self, turn: u32, usage: &Usage, cost_usd: f64) {
        if self.level != LogLevel::Errors {
            tracing::info!(
                turn = turn,
                input_tokens = usage.input_tokens,
                output_tokens = usage.output_tokens,
                cost_usd = format!("{:.4}", cost_usd),
                "Turn completed"
            );
        }
    }

    async fn on_error(&self, error: &str, retryable: bool) {
        tracing::error!(error = error, retryable = retryable, "Agent error");
    }

    async fn on_cancelled(&self) {
        tracing::info!("Agent cancelled");
    }
}

/// A hook that tracks file modifications
pub struct FileTrackingHook {
    /// Files that have been modified
    pub modified_files: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl Default for FileTrackingHook {
    fn default() -> Self {
        Self {
            modified_files: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

impl FileTrackingHook {
    /// Get the list of modified files
    pub fn get_modified_files(&self) -> Vec<String> {
        self.modified_files.lock().unwrap().clone()
    }
}

#[async_trait]
impl PiHook for FileTrackingHook {
    async fn after_tool_call(
        &self,
        tool_name: &str,
        input: &Value,
        _output: &str,
        is_error: bool,
    ) {
        if is_error {
            return;
        }

        // Track file-modifying tools
        match tool_name {
            "write" | "edit" => {
                if let Some(path) = input.get("file_path").and_then(|p| p.as_str()) {
                    let mut files = self.modified_files.lock().unwrap();
                    if !files.contains(&path.to_string()) {
                        files.push(path.to_string());
                    }
                }
            }
            "bash" => {
                // Could analyze command for file modifications, but that's complex
            }
            _ => {}
        }
    }
}

/// A hook that enforces cost limits
pub struct CostLimitHook {
    /// Maximum cost in USD
    pub max_cost_usd: f64,
    /// Current accumulated cost
    current_cost: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl CostLimitHook {
    /// Create a new cost limit hook
    pub fn new(max_cost_usd: f64) -> Self {
        Self {
            max_cost_usd,
            current_cost: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Get current accumulated cost
    pub fn current_cost(&self) -> f64 {
        let bits = self
            .current_cost
            .load(std::sync::atomic::Ordering::Relaxed);
        f64::from_bits(bits)
    }
}

#[async_trait]
impl PiHook for CostLimitHook {
    async fn on_turn_complete(&self, _turn: u32, _usage: &Usage, cost_usd: f64) {
        let current = self.current_cost();
        let new_cost = current + cost_usd;
        self.current_cost.store(
            new_cost.to_bits(),
            std::sync::atomic::Ordering::Relaxed,
        );
    }

    async fn before_llm_call(&self, _messages_count: usize) -> HookResult {
        let current = self.current_cost();
        if current >= self.max_cost_usd {
            return HookResult::Skip {
                reason: format!(
                    "Cost limit exceeded: ${:.4} >= ${:.4}",
                    current, self.max_cost_usd
                ),
            };
        }
        HookResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHook {
        call_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait]
    impl PiHook for TestHook {
        async fn before_tool_call(&self, _tool_name: &str, _input: &Value) -> HookResult {
            self.call_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            HookResult::Continue
        }
    }

    #[tokio::test]
    async fn test_hook_registry() {
        let mut registry = HookRegistry::new();
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        registry.add(TestHook {
            call_count: call_count.clone(),
        });

        assert_eq!(registry.len(), 1);

        registry
            .before_tool_call("test", &serde_json::json!({}))
            .await;
        assert_eq!(call_count.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_file_tracking_hook() {
        let hook = FileTrackingHook::default();

        hook.after_tool_call(
            "write",
            &serde_json::json!({"file_path": "/test/file.rs"}),
            "success",
            false,
        )
        .await;

        let files = hook.get_modified_files();
        assert_eq!(files, vec!["/test/file.rs"]);
    }

    #[tokio::test]
    async fn test_cost_limit_hook() {
        let hook = CostLimitHook::new(0.01);

        // Simulate a turn that costs more than the limit
        hook.on_turn_complete(1, &Usage::default(), 0.02).await;

        let result = hook.before_llm_call(5).await;
        assert!(matches!(result, HookResult::Skip { .. }));
    }
}
