//! Method trait for benchmark methods.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::task::TaskInstance;
use crate::trajectory::Trajectory;
use lm_router::LmUsage;

/// Result from running a method on a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodResult {
    /// The final answer.
    pub answer: String,
    /// Execution trajectory.
    pub trajectory: Trajectory,
    /// Token usage.
    pub usage: LmUsage,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
}

impl MethodResult {
    /// Create a new method result.
    pub fn new(answer: impl Into<String>, trajectory: Trajectory, usage: LmUsage) -> Self {
        Self {
            answer: answer.into(),
            trajectory,
            usage,
            duration_ms: 0,
        }
    }

    /// Set the duration.
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }
}

/// A method that can solve tasks.
///
/// Methods implement different approaches to solving benchmark tasks:
/// - Base model (direct LLM call)
/// - Summary agent (iterative summarization)
/// - CodeAct + BM25 (ReAct with retrieval)
/// - RLM (recursive language model)
#[async_trait]
pub trait Method: Send + Sync {
    /// Method name for logging.
    fn name(&self) -> &str;

    /// Run the method on a task, returning trajectory and answer.
    async fn solve(&self, task: &dyn TaskInstance) -> Result<MethodResult>;

    /// Optional warmup (e.g., load models).
    async fn warmup(&mut self) -> Result<()> {
        Ok(())
    }

    /// Reset state between tasks.
    async fn reset(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::{GroundTruth, SimpleTask};

    struct MockMethod {
        name: String,
    }

    #[async_trait]
    impl Method for MockMethod {
        fn name(&self) -> &str {
            &self.name
        }

        async fn solve(&self, task: &dyn TaskInstance) -> Result<MethodResult> {
            let trajectory = Trajectory::new(task.id(), self.name());
            Ok(MethodResult::new("mock answer", trajectory, LmUsage::default()))
        }
    }

    #[tokio::test]
    async fn test_mock_method() {
        let method = MockMethod {
            name: "mock".to_string(),
        };
        let task = SimpleTask::new("task-1", "What is 2+2?", GroundTruth::exact("4"));

        let result = method.solve(&task).await.unwrap();
        assert_eq!(result.answer, "mock answer");
    }
}
