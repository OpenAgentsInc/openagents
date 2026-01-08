//! Claude LLM client for RLM.
//!
//! Allows RlmEngine to use Claude (Pro/Max) as its inference backend.
//! This implements Mode B of the RLM + Claude integration: Claude IS the RLM.
//!
//! Uses structured outputs to enforce the RLM response format, ensuring Claude
//! always responds with either code to execute or a final answer.
//!
//! # Example
//!
//! ```rust,ignore
//! use rlm::{RlmEngine, ClaudeLlmClient, PythonExecutor, RlmConfig};
//!
//! let client = ClaudeLlmClient::new("/path/to/workspace");
//! let executor = PythonExecutor::new()?;
//! let engine = RlmEngine::new(client, executor);
//!
//! let result = engine.run("Analyze this codebase").await?;
//! ```

use async_trait::async_trait;
use claude_agent_sdk::{query, OutputFormat, QueryOptions, SdkMessage, SdkResultMessage, ToolsConfig};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;

use crate::client::{LlmClient, LlmResponse};
use crate::error::RlmError;

/// Structured response from Claude for RLM.
///
/// This enforces one of two actions:
/// - `execute`: Run Python code and wait for results
/// - `final`: Return the final answer and stop
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RlmStructuredResponse {
    /// The action to take: "execute" or "final"
    action: String,
    /// Python code to execute (when action is "execute")
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    /// Final answer (when action is "final")
    #[serde(skip_serializing_if = "Option::is_none")]
    answer: Option<String>,
    /// Brief reasoning for this step
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<String>,
}

/// JSON schema for RLM structured output.
fn rlm_output_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["execute", "final"],
                "description": "Whether to execute code or return final answer"
            },
            "code": {
                "type": "string",
                "description": "Python code to execute (required when action is 'execute')"
            },
            "answer": {
                "type": "string",
                "description": "Final answer (required when action is 'final')"
            },
            "reasoning": {
                "type": "string",
                "description": "Brief explanation of your reasoning for this step"
            }
        },
        "required": ["action"],
        "additionalProperties": false
    })
}

/// LLM client that uses Claude via claude-agent-sdk.
///
/// This client wraps the Claude CLI to provide completions for the RLM engine.
/// It disables tools for raw completion mode, allowing Claude to focus on
/// generating code and reasoning without invoking external tools.
#[derive(Clone)]
pub struct ClaudeLlmClient {
    workspace_root: PathBuf,
    model: Option<String>,
    /// Whether to persist sessions (default: false for RLM mode)
    persist_session: bool,
}

impl ClaudeLlmClient {
    /// Create a new Claude LLM client.
    ///
    /// # Arguments
    /// * `workspace_root` - The working directory for Claude sessions
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            model: None,
            persist_session: false,
        }
    }

    /// Set the model to use (e.g., "claude-sonnet-4-5-20250929").
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Enable session persistence.
    pub fn with_persistence(mut self, persist: bool) -> Self {
        self.persist_session = persist;
        self
    }
}

#[async_trait]
impl LlmClient for ClaudeLlmClient {
    async fn complete(
        &self,
        prompt: &str,
        _max_tokens: Option<usize>,
    ) -> Result<LlmResponse, RlmError> {
        // Build query options with structured output to enforce RLM format
        let mut options = QueryOptions::new()
            .cwd(&self.workspace_root)
            .max_turns(1) // Single turn for RLM queries
            .tools(ToolsConfig::none()); // No tools for raw completion

        // Set structured output format to enforce RLM response schema
        options.output_format = Some(OutputFormat {
            format_type: "json_schema".to_string(),
            schema: rlm_output_schema(),
        });

        if let Some(ref model) = self.model {
            options = options.model(model.clone());
        }

        if !self.persist_session {
            // Don't persist sessions for RLM mode
            options.persist_session = false;
        }

        // Execute the query
        let mut stream = query(prompt, options)
            .await
            .map_err(|e| RlmError::ClientError(format!("Claude query failed: {}", e)))?;

        let mut structured_output: Option<serde_json::Value> = None;
        let mut prompt_tokens = 0usize;
        let mut completion_tokens = 0usize;

        // Process the stream
        while let Some(msg_result) = stream.next().await {
            match msg_result {
                Ok(SdkMessage::Result(result)) => {
                    match result {
                        SdkResultMessage::Success(s) => {
                            // Get structured output if available
                            structured_output = s.structured_output;
                            // Extract usage
                            prompt_tokens = s.usage.input_tokens as usize;
                            completion_tokens = s.usage.output_tokens as usize;
                            break;
                        }
                        SdkResultMessage::ErrorDuringExecution(e) => {
                            return Err(RlmError::ClientError(format!(
                                "Claude execution error: {}",
                                e.errors.join("; ")
                            )));
                        }
                        SdkResultMessage::ErrorMaxTurns(e) => {
                            return Err(RlmError::ClientError(format!(
                                "Claude max turns exceeded: {}",
                                e.errors.join("; ")
                            )));
                        }
                        SdkResultMessage::ErrorMaxBudget(e) => {
                            return Err(RlmError::ClientError(format!(
                                "Claude budget exceeded: {}",
                                e.errors.join("; ")
                            )));
                        }
                        SdkResultMessage::ErrorMaxStructuredOutputRetries(e) => {
                            return Err(RlmError::ClientError(format!(
                                "Claude structured output retries exceeded: {}",
                                e.errors.join("; ")
                            )));
                        }
                    }
                }
                Ok(SdkMessage::System(sys)) => {
                    tracing::debug!("Claude system message: {:?}", sys);
                }
                Err(e) => {
                    return Err(RlmError::ClientError(format!("Stream error: {}", e)));
                }
                _ => {
                    // Ignore other message types
                }
            }
        }

        // Convert structured output to text format expected by RLM engine
        let content = match structured_output {
            Some(output) => {
                let parsed: RlmStructuredResponse = serde_json::from_value(output)
                    .map_err(|e| RlmError::ClientError(format!("Failed to parse structured output: {}", e)))?;

                convert_structured_to_text(&parsed)
            }
            None => {
                return Err(RlmError::ClientError("No structured output received".to_string()));
            }
        };

        // Build response
        let mut response = LlmResponse::new(content);
        if prompt_tokens > 0 || completion_tokens > 0 {
            response = response.with_usage(prompt_tokens, completion_tokens);
        }

        Ok(response)
    }
}

/// Convert structured RLM response to text format for the RLM engine parser.
fn convert_structured_to_text(response: &RlmStructuredResponse) -> String {
    let mut output = String::new();

    // Add reasoning if present
    if let Some(ref reasoning) = response.reasoning {
        output.push_str(reasoning);
        output.push_str("\n\n");
    }

    match response.action.as_str() {
        "execute" => {
            // Format as code block that the RLM engine expects
            if let Some(ref code) = response.code {
                output.push_str("```repl\n");
                output.push_str(code);
                output.push_str("\n```");
            }
        }
        "final" => {
            // Format as FINAL command
            if let Some(ref answer) = response.answer {
                output.push_str("FINAL ");
                output.push_str(answer);
            }
        }
        _ => {
            // Unknown action - return as-is
            output.push_str(&format!("Unknown action: {}", response.action));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_client_builder() {
        let client = ClaudeLlmClient::new("/tmp")
            .with_model("claude-sonnet-4-5-20250929")
            .with_persistence(true);

        assert_eq!(
            client.model,
            Some("claude-sonnet-4-5-20250929".to_string())
        );
        assert!(client.persist_session);
    }

    #[test]
    fn test_claude_client_default() {
        let client = ClaudeLlmClient::new("/workspace");

        assert_eq!(client.workspace_root, PathBuf::from("/workspace"));
        assert!(client.model.is_none());
        assert!(!client.persist_session);
    }
}
