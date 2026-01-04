//! Mock executor for testing RLM without a real sandbox.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::error::Result;
use crate::executor::{ExecutionEnvironment, ExecutionResult, ExecutorCapabilities};

/// Mock executor that returns predefined responses.
///
/// Useful for testing the RLM engine without a real execution environment.
pub struct MockExecutor {
    /// Map of code patterns to expected outputs.
    responses: Mutex<HashMap<String, String>>,
    /// Default response when no match is found.
    default_response: String,
    /// Record of all executed code.
    execution_log: Mutex<Vec<String>>,
}

impl Default for MockExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl MockExecutor {
    /// Create a new mock executor with no predefined responses.
    pub fn new() -> Self {
        Self {
            responses: Mutex::new(HashMap::new()),
            default_response: "OK".to_string(),
            execution_log: Mutex::new(Vec::new()),
        }
    }

    /// Add an expected code pattern and its response.
    ///
    /// When the executor receives code containing `pattern`, it returns `response`.
    pub fn expect(self, pattern: impl Into<String>, response: impl Into<String>) -> Self {
        self.responses
            .lock()
            .unwrap()
            .insert(pattern.into(), response.into());
        self
    }

    /// Set the default response when no pattern matches.
    pub fn default_response(mut self, response: impl Into<String>) -> Self {
        self.default_response = response.into();
        self
    }

    /// Get all code that was executed (for assertions).
    pub fn execution_log(&self) -> Vec<String> {
        self.execution_log.lock().unwrap().clone()
    }

    /// Find a response for the given code.
    fn find_response(&self, code: &str) -> String {
        let responses = self.responses.lock().unwrap();

        for (pattern, response) in responses.iter() {
            if code.contains(pattern) {
                return response.clone();
            }
        }

        self.default_response.clone()
    }
}

#[async_trait]
impl ExecutionEnvironment for MockExecutor {
    async fn execute(&self, code: &str) -> Result<ExecutionResult> {
        // Record the execution
        self.execution_log.lock().unwrap().push(code.to_string());

        // Find matching response
        let output = self.find_response(code);

        Ok(ExecutionResult::success(output))
    }

    fn capabilities(&self) -> ExecutorCapabilities {
        ExecutorCapabilities {
            languages: vec!["mock".to_string()],
            has_filesystem: false,
            has_network: false,
            max_memory_bytes: None,
            max_time_ms: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_executor_default() {
        let executor = MockExecutor::new();
        let result = executor.execute("some code").await.unwrap();
        assert_eq!(result.stdout, "OK");
    }

    #[tokio::test]
    async fn test_mock_executor_with_pattern() {
        let executor = MockExecutor::new()
            .expect("2 + 2", "4")
            .expect("hello", "world");

        let result = executor.execute("print(2 + 2)").await.unwrap();
        assert_eq!(result.stdout, "4");

        let result = executor.execute("say hello").await.unwrap();
        assert_eq!(result.stdout, "world");
    }

    #[tokio::test]
    async fn test_mock_executor_logs_execution() {
        let executor = MockExecutor::new();

        executor.execute("code1").await.unwrap();
        executor.execute("code2").await.unwrap();

        let log = executor.execution_log();
        assert_eq!(log, vec!["code1", "code2"]);
    }
}
