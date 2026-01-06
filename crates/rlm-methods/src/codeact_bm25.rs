//! CodeAct + BM25 method implementation.
//!
//! ReAct-style agent that combines BM25 retrieval over context with Python code execution.
//! This is a baseline method from the RLM paper that predates the recursive llm_query approach.

use std::sync::Arc;

use async_trait::async_trait;
use bench_harness::{Method, MethodResult, StepType, TaskInstance, Trajectory};
use lm_router::{LmRouter, LmUsage};
use rlm::{ExecutionEnvironment, PythonExecutor};

use crate::retrieval::Bm25Index;

/// Actions the agent can take in the ReAct loop.
#[derive(Debug, Clone)]
enum Action {
    /// Search the context using BM25.
    Search(String),
    /// Execute Python code.
    Execute(String),
    /// Return the final answer.
    Final(String),
}

/// CodeAct + BM25 method.
///
/// Implements a ReAct-style agent with:
/// - `search(query)`: BM25 retrieval over the context
/// - `execute(code)`: Python REPL execution
/// - `final(answer)`: Return the final answer
///
/// # Example
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use rlm_methods::CodeActBm25Method;
/// use lm_router::LmRouter;
///
/// let router = Arc::new(LmRouter::builder()
///     .add_backend(backend)
///     .build());
///
/// let method = CodeActBm25Method::new(router, "model-name");
/// let result = method.solve(&task).await?;
/// ```
pub struct CodeActBm25Method {
    router: Arc<LmRouter>,
    model: String,
    max_iterations: usize,
    top_k_results: usize,
    python_binary: String,
}

impl CodeActBm25Method {
    /// Create a new CodeAct+BM25 method.
    pub fn new(router: Arc<LmRouter>, model: impl Into<String>) -> Self {
        Self {
            router,
            model: model.into(),
            max_iterations: 10,
            top_k_results: 5,
            python_binary: "python3".to_string(),
        }
    }

    /// Set the maximum number of iterations.
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Set the number of top-k results for BM25 search.
    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k_results = top_k;
        self
    }

    /// Set the Python binary path.
    pub fn with_python(mut self, python: impl Into<String>) -> Self {
        self.python_binary = python.into();
        self
    }

    /// Generate the system prompt for the ReAct agent.
    fn system_prompt(&self) -> String {
        r#"You are a ReAct agent that answers questions by searching context and executing code.

You have three actions available:

1. SEARCH: <query>
   Search the context for relevant information using the given query.

2. EXECUTE:
```python
<code>
```
   Execute Python code. The output will be shown to you.

3. FINAL: <answer>
   Provide your final answer to the question.

Rules:
- Always think step by step before taking an action.
- Use SEARCH to find relevant information in the context.
- Use EXECUTE to perform calculations or process data.
- Use FINAL only when you have enough information to answer confidently.
- Each response should contain exactly ONE action.

Format your response as:
THOUGHT: <your reasoning>
ACTION: <action type>
<action content>
"#
        .to_string()
    }

    /// Parse an action from the LLM response.
    fn parse_action(&self, response: &str) -> Option<Action> {
        let response_upper = response.to_uppercase();

        // Check for FINAL action
        if let Some(pos) = response_upper.find("FINAL:") {
            let answer_start = pos + 6;
            let answer = response[answer_start..].trim().to_string();
            return Some(Action::Final(answer));
        }

        // Check for SEARCH action
        if let Some(pos) = response_upper.find("SEARCH:") {
            let query_start = pos + 7;
            let query_end = response[query_start..]
                .find('\n')
                .map(|p| query_start + p)
                .unwrap_or(response.len());
            let query = response[query_start..query_end].trim().to_string();
            if !query.is_empty() {
                return Some(Action::Search(query));
            }
        }

        // Check for EXECUTE action with code block
        if response_upper.contains("EXECUTE:") {
            // Look for Python code block
            if let Some(start) = response.find("```python") {
                let code_start = start + 9;
                if let Some(end) = response[code_start..].find("```") {
                    let code = response[code_start..code_start + end].trim().to_string();
                    return Some(Action::Execute(code));
                }
            }
            // Also try generic code block
            if let Some(start) = response.find("```\n") {
                let code_start = start + 4;
                if let Some(end) = response[code_start..].find("```") {
                    let code = response[code_start..code_start + end].trim().to_string();
                    return Some(Action::Execute(code));
                }
            }
        }

        None
    }

    /// Build the prompt for the current iteration.
    fn build_prompt(&self, query: &str, context: &str, history: &[String]) -> String {
        let mut prompt = self.system_prompt();

        prompt.push_str("\n\n--- CONTEXT ---\n");
        // Truncate context if too long
        if context.len() > 50000 {
            prompt.push_str(&context[..50000]);
            prompt.push_str("\n... [truncated]");
        } else {
            prompt.push_str(context);
        }

        prompt.push_str("\n\n--- QUESTION ---\n");
        prompt.push_str(query);

        if !history.is_empty() {
            prompt.push_str("\n\n--- HISTORY ---\n");
            for entry in history {
                prompt.push_str(entry);
                prompt.push_str("\n\n");
            }
        }

        prompt.push_str("\n\nProvide your next action:");
        prompt
    }
}

#[async_trait]
impl Method for CodeActBm25Method {
    fn name(&self) -> &str {
        "codeact-bm25"
    }

    async fn solve(&self, task: &dyn TaskInstance) -> bench_harness::Result<MethodResult> {
        let mut trajectory = Trajectory::new(task.id(), self.name());
        let mut history: Vec<String> = Vec::new();
        let mut total_prompt_tokens = 0usize;
        let mut total_completion_tokens = 0usize;

        let context = task.context().unwrap_or("");
        let query = task.query();

        // Build BM25 index from context
        let index = Bm25Index::new(context);

        // Create Python executor
        let executor = PythonExecutor::with_binary(&self.python_binary);

        let start = std::time::Instant::now();

        for iteration in 0..self.max_iterations {
            // Build prompt with current history
            let prompt = self.build_prompt(query, context, &history);

            // Call LLM
            let response = self
                .router
                .complete(&self.model, &prompt, 2048)
                .await
                .map_err(|e| {
                    bench_harness::Error::MethodError(format!("LLM call failed: {}", e))
                })?;

            let llm_output = &response.text;
            total_prompt_tokens += response.usage.prompt_tokens;
            total_completion_tokens += response.usage.completion_tokens;

            // Add LLM step to trajectory
            trajectory.add_step(
                StepType::LlmCall {
                    model: self.model.clone(),
                    prompt_tokens: response.usage.prompt_tokens,
                    completion_tokens: response.usage.completion_tokens,
                },
                llm_output,
            );

            // Parse action
            let action = self.parse_action(llm_output);

            match action {
                Some(Action::Search(search_query)) => {
                    let results = index.search(&search_query, self.top_k_results);

                    // Format results
                    let mut result_text = format!("Search results for '{}':\n", search_query);
                    if results.is_empty() {
                        result_text.push_str("No matching segments found.");
                    } else {
                        for (i, result) in results.iter().enumerate() {
                            result_text.push_str(&format!(
                                "\n[{}] (score: {:.3})\n{}\n",
                                i + 1,
                                result.score,
                                result.text
                            ));
                        }
                    }

                    // Add retrieval step to trajectory
                    trajectory.add_step(
                        StepType::Retrieval {
                            query: search_query.clone(),
                            num_results: results.len(),
                        },
                        &result_text,
                    );

                    history.push(format!("SEARCH: {}\n{}", search_query, result_text));
                }

                Some(Action::Execute(code)) => {
                    // Execute Python code
                    let exec_result = executor.execute(&code).await.map_err(|e| {
                        bench_harness::Error::MethodError(format!("Python execution failed: {}", e))
                    })?;

                    let output = if exec_result.stdout.is_empty() && exec_result.stderr.is_empty() {
                        "(no output)".to_string()
                    } else if exec_result.stderr.is_empty() {
                        exec_result.stdout.clone()
                    } else {
                        format!(
                            "stdout:\n{}\nstderr:\n{}",
                            exec_result.stdout, exec_result.stderr
                        )
                    };

                    // Add code execution step to trajectory
                    trajectory.add_step(
                        StepType::CodeExecution {
                            language: "python".to_string(),
                            exit_code: exec_result.exit_code,
                        },
                        &format!("Code:\n```python\n{}\n```\nOutput:\n{}", code, output),
                    );

                    history.push(format!("EXECUTE:\n```python\n{}\n```\nOutput: {}", code, output));
                }

                Some(Action::Final(answer)) => {
                    trajectory.add_final(&answer);

                    let duration_ms = start.elapsed().as_millis() as u64;
                    let usage = LmUsage::new(total_prompt_tokens, total_completion_tokens);

                    return Ok(
                        MethodResult::new(answer, trajectory, usage).with_duration(duration_ms)
                    );
                }

                None => {
                    // Could not parse action, add to history and continue
                    history.push(format!(
                        "Previous response (no valid action detected):\n{}",
                        llm_output
                    ));

                    // If we've had too many unparseable responses, force a final
                    if iteration >= self.max_iterations - 2 {
                        // Try to extract any answer-like content
                        let forced_answer = llm_output.lines().last().unwrap_or("").to_string();
                        trajectory.add_final(&forced_answer);

                        let duration_ms = start.elapsed().as_millis() as u64;
                        let usage = LmUsage::new(total_prompt_tokens, total_completion_tokens);

                        return Ok(MethodResult::new(forced_answer, trajectory, usage)
                            .with_duration(duration_ms));
                    }
                }
            }
        }

        // Max iterations reached without final answer
        let fallback_answer = history
            .last()
            .map(|s| s.lines().last().unwrap_or("").to_string())
            .unwrap_or_default();

        trajectory.add_final(&fallback_answer);

        let duration_ms = start.elapsed().as_millis() as u64;
        let usage = LmUsage::new(total_prompt_tokens, total_completion_tokens);

        Ok(MethodResult::new(fallback_answer, trajectory, usage).with_duration(duration_ms))
    }

    async fn warmup(&mut self) -> bench_harness::Result<()> {
        // Verify LLM backend is available
        let _ = self
            .router
            .complete(&self.model, "Hello", 10)
            .await
            .map_err(|e| {
                bench_harness::Error::MethodError(format!("Warmup LLM call failed: {}", e))
            })?;

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
            .with_response("THOUGHT: I need to search for information.\nACTION: FINAL: 42");

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
        let method = CodeActBm25Method::new(router, "test-model");
        assert_eq!(method.name(), "codeact-bm25");
    }

    #[test]
    fn test_parse_search_action() {
        let router = create_test_router();
        let method = CodeActBm25Method::new(router, "test-model");

        let response = "THOUGHT: I need to find info\nACTION: SEARCH: important query";
        let action = method.parse_action(response);
        assert!(matches!(action, Some(Action::Search(q)) if q == "important query"));
    }

    #[test]
    fn test_parse_execute_action() {
        let router = create_test_router();
        let method = CodeActBm25Method::new(router, "test-model");

        let response = r#"THOUGHT: Let me calculate
ACTION: EXECUTE:
```python
print(2 + 2)
```"#;
        let action = method.parse_action(response);
        assert!(matches!(action, Some(Action::Execute(code)) if code == "print(2 + 2)"));
    }

    #[test]
    fn test_parse_final_action() {
        let router = create_test_router();
        let method = CodeActBm25Method::new(router, "test-model");

        let response = "THOUGHT: I have the answer\nACTION: FINAL: The answer is 42";
        let action = method.parse_action(response);
        assert!(matches!(action, Some(Action::Final(a)) if a == "The answer is 42"));
    }

    #[test]
    fn test_method_config() {
        let router = create_test_router();
        let method = CodeActBm25Method::new(router, "test-model")
            .with_max_iterations(20)
            .with_top_k(10)
            .with_python("python");

        assert_eq!(method.max_iterations, 20);
        assert_eq!(method.top_k_results, 10);
        assert_eq!(method.python_binary, "python");
    }
}
