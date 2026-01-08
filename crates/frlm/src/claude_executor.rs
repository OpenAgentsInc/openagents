//! Claude executor for FRLM.
//!
//! Wraps ClaudeLlmClient to implement LocalExecutor trait,
//! enabling Claude as a backend for FRLM fan-out queries.
//!
//! # Example
//!
//! ```rust,ignore
//! use frlm::{ClaudeLocalExecutor, FrlmConductor, FrlmProgram, FrlmPolicy};
//!
//! let executor = ClaudeLocalExecutor::new("/path/to/workspace");
//! let mut conductor = FrlmConductor::with_defaults();
//!
//! let program = FrlmProgram::new("Analyze this code");
//! let result = conductor.run(program, &submitter, Some(&executor)).await?;
//! ```

use async_trait::async_trait;
use rlm::{ClaudeLlmClient, LlmClient};
use std::path::PathBuf;

use crate::conductor::LocalExecutor;
use crate::error::Result;
use crate::Venue;

/// Local executor that uses Claude via claude-agent-sdk.
///
/// This executor wraps the ClaudeLlmClient from the rlm crate to provide
/// local execution for FRLM programs. It's useful when you want to use
/// Claude (Pro/Max) as the backend for recursive queries.
pub struct ClaudeLocalExecutor {
    client: ClaudeLlmClient,
    model: Option<String>,
}

impl ClaudeLocalExecutor {
    /// Create a new Claude executor.
    ///
    /// # Arguments
    /// * `workspace_root` - The working directory for Claude sessions
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            client: ClaudeLlmClient::new(workspace_root),
            model: None,
        }
    }

    /// Set the model to use (e.g., "claude-opus-4-5-20251101").
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        let model_str = model.into();
        self.model = Some(model_str.clone());
        self.client = self.client.with_model(model_str);
        self
    }

    /// Get the model ID being used.
    pub fn model_id(&self) -> Option<&str> {
        self.model.as_deref()
    }
}

#[async_trait]
impl LocalExecutor for ClaudeLocalExecutor {
    async fn execute(&self, query: &str) -> Result<String> {
        let response = self.client.complete(query, None).await?;
        Ok(response.content().to_string())
    }
}

/// Extension trait for LocalExecutor to get the venue type.
pub trait LocalExecutorExt: LocalExecutor {
    /// Get the execution venue for this executor.
    fn venue(&self) -> Venue;
}

impl LocalExecutorExt for ClaudeLocalExecutor {
    fn venue(&self) -> Venue {
        Venue::Claude
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_executor_builder() {
        let executor = ClaudeLocalExecutor::new("/tmp")
            .with_model("claude-sonnet-4-5-20250929");

        assert_eq!(executor.model_id(), Some("claude-sonnet-4-5-20250929"));
    }

    #[test]
    fn test_claude_executor_venue() {
        let executor = ClaudeLocalExecutor::new("/tmp");
        assert_eq!(executor.venue(), Venue::Claude);
    }
}
