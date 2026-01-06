//! RLM No Sub-calls method implementation (ablation study).
//!
//! This method is the same as RLM Full but with `llm_query()` disabled.
//! It tests the contribution of recursive sub-calls vs. just having a REPL environment.

use std::sync::Arc;

use async_trait::async_trait;
use bench_harness::{Method, MethodResult, StepType, TaskInstance, Trajectory};
use fm_bridge::FMClient;
use lm_router::{LmRouter, LmUsage};
use rlm::{Context, PromptTier, PythonExecutor, RlmConfig, RlmEngine, RlmResult};

/// RLM No Sub-calls method - ablation study.
///
/// Uses the RLM engine's prompt-execute loop but with `llm_query()` disabled.
/// This tests the contribution of recursive sub-calls by comparing performance
/// against the full RLM method.
///
/// # Example
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use rlm_methods::RlmNoSubcallsMethod;
/// use lm_router::LmRouter;
///
/// let router = Arc::new(LmRouter::builder()
///     .add_backend(backend)
///     .build());
///
/// let method = RlmNoSubcallsMethod::new(router, "model-name");
/// let result = method.solve(&task).await?;
/// ```
pub struct RlmNoSubcallsMethod {
    #[allow(dead_code)]
    router: Arc<LmRouter>,
    model: String,
    max_iterations: u32,
    prompt_tier: PromptTier,
    python_binary: String,
}

impl RlmNoSubcallsMethod {
    /// Create a new RLM No Sub-calls method.
    pub fn new(router: Arc<LmRouter>, model: impl Into<String>) -> Self {
        Self {
            router,
            model: model.into(),
            max_iterations: 10,
            prompt_tier: PromptTier::Guided,
            python_binary: "python3".to_string(),
        }
    }

    /// Set the maximum number of iterations.
    pub fn with_max_iterations(mut self, max_iterations: u32) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Set the prompt tier.
    pub fn with_prompt_tier(mut self, tier: PromptTier) -> Self {
        self.prompt_tier = tier;
        self
    }

    /// Set the Python binary path.
    pub fn with_python(mut self, python: impl Into<String>) -> Self {
        self.python_binary = python.into();
        self
    }

    /// Convert RlmResult execution log to Trajectory.
    fn convert_to_trajectory(&self, task_id: &str, result: &RlmResult) -> Trajectory {
        let mut trajectory = Trajectory::new(task_id, self.name());

        for entry in &result.execution_log {
            let step_type = match entry.command_type.as_str() {
                "FINAL" => StepType::Final {
                    answer: entry.result.clone(),
                },
                "RunCode" | "RunCode+FINAL" => StepType::CodeExecution {
                    language: "python".to_string(),
                    exit_code: 0,
                },
                "RUN" => StepType::CodeExecution {
                    language: "shell".to_string(),
                    exit_code: 0,
                },
                "ChunkAnalysis" => StepType::SubQuery {
                    prompt: entry.executed.clone(),
                },
                _ => StepType::LlmCall {
                    model: self.model.clone(),
                    prompt_tokens: 0,
                    completion_tokens: 0,
                },
            };

            trajectory.add_step(step_type, &entry.result);
        }

        // Ensure we have a final step
        if !result.output.is_empty() {
            let has_final = result.execution_log.iter().any(|e| e.command_type == "FINAL");
            if !has_final {
                trajectory.add_final(&result.output);
            }
        }

        trajectory
    }
}

#[async_trait]
impl Method for RlmNoSubcallsMethod {
    fn name(&self) -> &str {
        "rlm-no-subcalls"
    }

    async fn solve(&self, task: &dyn TaskInstance) -> bench_harness::Result<MethodResult> {
        // Create FMClient for the RlmEngine
        let client = FMClient::new().map_err(|e| {
            bench_harness::Error::MethodError(format!("Failed to create FM client: {}", e))
        })?;

        // Create Python executor
        let executor = PythonExecutor::with_binary(&self.python_binary);

        // Create config with subqueries DISABLED (ablation study)
        let config = RlmConfig {
            max_iterations: self.max_iterations,
            allow_shell: false,
            verbose: false,
            prompt_tier: self.prompt_tier,
            enable_stuck_detection: true,
            disable_subqueries: true, // KEY DIFFERENCE: Disable llm_query for ablation
        };

        // Create engine
        let mut engine = RlmEngine::with_config(client, executor, config);

        // Set context if available
        if let Some(context_str) = task.context() {
            let context = Context::from_text(context_str);
            engine.set_context(context);
        }

        // Run the engine
        let start = std::time::Instant::now();
        let result = engine.run(task.query()).await.map_err(|e| {
            bench_harness::Error::MethodError(format!("RLM engine error: {}", e))
        })?;
        let duration_ms = start.elapsed().as_millis() as u64;

        // Convert to trajectory
        let trajectory = self.convert_to_trajectory(task.id(), &result);

        // Estimate usage
        let estimated_tokens = result.iterations as usize * 1000;
        let usage = LmUsage::new(estimated_tokens / 2, estimated_tokens / 2);

        Ok(MethodResult::new(result.output, trajectory, usage).with_duration(duration_ms))
    }

    async fn warmup(&mut self) -> bench_harness::Result<()> {
        // Verify FM Bridge is available
        let client = FMClient::new().map_err(|e| {
            bench_harness::Error::MethodError(format!("FM Bridge not available: {}", e))
        })?;

        // Simple health check
        let _ = client
            .complete("Hello", None)
            .await
            .map_err(|e| bench_harness::Error::MethodError(format!("Warmup failed: {}", e)))?;

        Ok(())
    }

    async fn reset(&mut self) -> bench_harness::Result<()> {
        // No state to reset
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lm_router::backends::MockBackend;

    fn create_test_router() -> Arc<LmRouter> {
        let mock = MockBackend::new()
            .with_model("test-model")
            .with_response("Test response");

        Arc::new(
            LmRouter::builder()
                .add_backend(mock)
                .default_backend("mock")
                .build(),
        )
    }

    #[test]
    fn test_method_name() {
        let router = create_test_router();
        let method = RlmNoSubcallsMethod::new(router, "test-model");
        assert_eq!(method.name(), "rlm-no-subcalls");
    }

    #[test]
    fn test_method_config() {
        let router = create_test_router();
        let method = RlmNoSubcallsMethod::new(router, "test-model")
            .with_max_iterations(20)
            .with_prompt_tier(PromptTier::Full)
            .with_python("python");

        assert_eq!(method.max_iterations, 20);
        assert_eq!(method.python_binary, "python");
    }
}
