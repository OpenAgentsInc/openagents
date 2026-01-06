//! Summary Agent method: Iterative context summarization.

use std::sync::Arc;

use async_trait::async_trait;

use bench_harness::{
    Method, MethodResult, StepType, TaskInstance, Trajectory,
};
use lm_router::{LmRouter, LmUsage};

use crate::prompts::{SUMMARY_ANSWER_PROMPT, SUMMARY_CHUNK_PROMPT, SUMMARY_COMBINE_PROMPT};

/// Summary Agent method - iterative context summarization.
///
/// This method:
/// 1. Chunks the context into manageable pieces
/// 2. Summarizes each chunk
/// 3. Iteratively combines summaries until they fit in context
/// 4. Uses the final summary to answer the question
pub struct SummaryAgentMethod {
    router: Arc<LmRouter>,
    model: String,
    chunk_size: usize,
    max_tokens: usize,
    target_summary_length: usize,
}

impl SummaryAgentMethod {
    /// Create a new summary agent method.
    pub fn new(router: Arc<LmRouter>, model: impl Into<String>) -> Self {
        Self {
            router,
            model: model.into(),
            chunk_size: 4000,           // Characters per chunk
            max_tokens: 2048,           // Max tokens for LLM response
            target_summary_length: 8000, // Target total summary length
        }
    }

    /// Set the chunk size for splitting context.
    pub fn with_chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = size;
        self
    }

    /// Set the maximum tokens for responses.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set the target summary length.
    pub fn with_target_summary_length(mut self, length: usize) -> Self {
        self.target_summary_length = length;
        self
    }

    /// Split text into chunks.
    fn chunk_text(&self, text: &str) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();

        for paragraph in text.split("\n\n") {
            if current_chunk.len() + paragraph.len() > self.chunk_size {
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk);
                    current_chunk = String::new();
                }
                // If a single paragraph is larger than chunk size, split it
                if paragraph.len() > self.chunk_size {
                    let words: Vec<&str> = paragraph.split_whitespace().collect();
                    let mut word_chunk = String::new();
                    for word in words {
                        if word_chunk.len() + word.len() + 1 > self.chunk_size {
                            chunks.push(word_chunk);
                            word_chunk = String::new();
                        }
                        if !word_chunk.is_empty() {
                            word_chunk.push(' ');
                        }
                        word_chunk.push_str(word);
                    }
                    if !word_chunk.is_empty() {
                        current_chunk = word_chunk;
                    }
                } else {
                    current_chunk = paragraph.to_string();
                }
            } else {
                if !current_chunk.is_empty() {
                    current_chunk.push_str("\n\n");
                }
                current_chunk.push_str(paragraph);
            }
        }

        if !current_chunk.is_empty() {
            chunks.push(current_chunk);
        }

        chunks
    }

    /// Summarize a single chunk.
    async fn summarize_chunk(
        &self,
        chunk: &str,
        trajectory: &mut Trajectory,
    ) -> bench_harness::Result<(String, LmUsage)> {
        let prompt = SUMMARY_CHUNK_PROMPT.replace("{text}", chunk);

        let response = self.router.complete(&self.model, &prompt, self.max_tokens).await
            .map_err(|e| bench_harness::Error::MethodError(e.to_string()))?;

        trajectory.add_step(
            StepType::SubQuery {
                prompt: "summarize chunk".to_string(),
            },
            &response.text,
        );

        Ok((response.text.trim().to_string(), response.usage))
    }

    /// Combine multiple summaries.
    async fn combine_summaries(
        &self,
        summaries: &[String],
        trajectory: &mut Trajectory,
    ) -> bench_harness::Result<(String, LmUsage)> {
        let combined = summaries.join("\n\n---\n\n");
        let prompt = SUMMARY_COMBINE_PROMPT.replace("{summaries}", &combined);

        let response = self.router.complete(&self.model, &prompt, self.max_tokens).await
            .map_err(|e| bench_harness::Error::MethodError(e.to_string()))?;

        trajectory.add_step(
            StepType::SubQuery {
                prompt: "combine summaries".to_string(),
            },
            &response.text,
        );

        Ok((response.text.trim().to_string(), response.usage))
    }
}

#[async_trait]
impl Method for SummaryAgentMethod {
    fn name(&self) -> &str {
        "summary_agent"
    }

    async fn solve(&self, task: &dyn TaskInstance) -> bench_harness::Result<MethodResult> {
        let context = task.context().unwrap_or("");
        let query = task.query();

        // Create trajectory
        let mut trajectory = Trajectory::new(task.id(), self.name());
        let mut total_usage = LmUsage::default();

        // Step 1: Chunk the context
        let chunks = self.chunk_text(context);

        // Step 2: Summarize each chunk
        let mut summaries = Vec::new();
        for chunk in &chunks {
            let (summary, usage) = self.summarize_chunk(chunk, &mut trajectory).await?;
            summaries.push(summary);
            total_usage = total_usage.combine(&usage);
        }

        // Step 3: Iteratively combine summaries until they fit
        while summaries.len() > 1 {
            let total_length: usize = summaries.iter().map(|s| s.len()).sum();
            if total_length <= self.target_summary_length {
                break;
            }

            // Combine pairs of summaries
            let mut new_summaries = Vec::new();
            for chunk in summaries.chunks(2) {
                if chunk.len() == 2 {
                    let (combined, usage) = self.combine_summaries(chunk, &mut trajectory).await?;
                    new_summaries.push(combined);
                    total_usage = total_usage.combine(&usage);
                } else {
                    new_summaries.push(chunk[0].clone());
                }
            }
            summaries = new_summaries;
        }

        // Final summary
        let final_summary = summaries.join("\n\n");

        // Step 4: Answer the question using the summary
        let prompt = SUMMARY_ANSWER_PROMPT
            .replace("{summary}", &final_summary)
            .replace("{query}", query);

        let response = self.router.complete(&self.model, &prompt, self.max_tokens).await
            .map_err(|e| bench_harness::Error::MethodError(e.to_string()))?;

        trajectory.add_llm_call(&self.model, &response.usage, &response.text);
        total_usage = total_usage.combine(&response.usage);

        let answer = response.text.trim().to_string();
        trajectory.add_final(&answer);

        Ok(MethodResult::new(answer, trajectory, total_usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bench_harness::{GroundTruth, SimpleTask};
    use lm_router::backends::MockBackend;

    #[tokio::test]
    async fn test_summary_agent_method() {
        // Create a mock backend
        let mock = MockBackend::new()
            .with_model("test-model")
            .with_response("Summary: The answer is 42");

        let router = LmRouter::builder()
            .add_backend(mock)
            .default_backend("mock")
            .build();

        let method = SummaryAgentMethod::new(Arc::new(router), "test-model")
            .with_chunk_size(100);

        // Short context that doesn't need chunking
        let task = SimpleTask::new("task-1", "What is the answer?", GroundTruth::exact("42"))
            .with_context("The answer to everything is 42.");

        let result = method.solve(&task).await.unwrap();
        assert!(!result.trajectory.steps.is_empty());
    }

    #[test]
    fn test_chunk_text() {
        let router = LmRouter::builder().build();
        let method = SummaryAgentMethod::new(Arc::new(router), "test")
            .with_chunk_size(50);

        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let chunks = method.chunk_text(text);
        assert!(!chunks.is_empty());
    }
}
