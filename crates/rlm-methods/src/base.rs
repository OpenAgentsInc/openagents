//! Base method: Direct LLM call with full context.

use std::sync::Arc;

use async_trait::async_trait;

use bench_harness::{
    Method, MethodResult, TaskInstance, Trajectory,
};
use lm_router::LmRouter;

use crate::prompts::{format_prompt, BASE_PROMPT};

/// Base method - direct LLM call with full context.
///
/// This is the simplest baseline: concatenate context and query,
/// send to the LLM, and return the response as the answer.
pub struct BaseMethod {
    router: Arc<LmRouter>,
    model: String,
    max_tokens: usize,
}

impl BaseMethod {
    /// Create a new base method.
    pub fn new(router: Arc<LmRouter>, model: impl Into<String>) -> Self {
        Self {
            router,
            model: model.into(),
            max_tokens: 4096,
        }
    }

    /// Set the maximum tokens for the response.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }
}

#[async_trait]
impl Method for BaseMethod {
    fn name(&self) -> &str {
        "base"
    }

    async fn solve(&self, task: &dyn TaskInstance) -> bench_harness::Result<MethodResult> {
        let context = task.context().unwrap_or("");
        let query = task.query();

        // Format the prompt
        let prompt = format_prompt(BASE_PROMPT, context, query);

        // Create trajectory
        let mut trajectory = Trajectory::new(task.id(), self.name());

        // Call the LLM
        let response = self.router.complete(&self.model, &prompt, self.max_tokens).await
            .map_err(|e| bench_harness::Error::MethodError(e.to_string()))?;

        // Record the LLM call
        trajectory.add_llm_call(&self.model, &response.usage, &response.text);

        // Extract the answer (the full response is the answer for base method)
        let answer = response.text.trim().to_string();
        trajectory.add_final(&answer);

        Ok(MethodResult::new(answer, trajectory, response.usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bench_harness::{GroundTruth, SimpleTask};
    use lm_router::backends::MockBackend;

    #[tokio::test]
    async fn test_base_method() {
        // Create a mock backend
        let mock = MockBackend::new()
            .with_model("test-model")
            .with_response("The answer is 42");

        let router = LmRouter::builder()
            .add_backend(mock)
            .default_backend("mock")
            .build();

        let method = BaseMethod::new(Arc::new(router), "test-model");

        let task = SimpleTask::new("task-1", "What is the answer?", GroundTruth::exact("42"))
            .with_context("The answer to everything is 42.");

        let result = method.solve(&task).await.unwrap();
        assert!(result.answer.contains("42"));
        assert!(!result.trajectory.steps.is_empty());
    }
}
