//! Minimal Coding Subagent
//!
//! Implements one subtask at a time with a minimal prompt.
//! Following pi-mono's insight: models are RL-trained for coding,
//! they don't need 10K tokens of instructions.

use crate::agent_loop::{agent_loop, LoopConfig, LoopEvent, LoopResult, LoopToolCall, ToolExecutor};
use crate::error::{AgentError, AgentResult};
use crate::types::{AgentType, Subtask, SubagentResult, SUBAGENT_SYSTEM_PROMPT, build_subagent_prompt};
use llm::LlmProvider;
use std::collections::HashSet;

/// Configuration for the minimal subagent
#[derive(Debug, Clone)]
pub struct SubagentConfig {
    /// Working directory
    pub cwd: String,
    /// Model to use
    pub model: Option<String>,
    /// Maximum turns
    pub max_turns: Option<u32>,
}

impl Default for SubagentConfig {
    fn default() -> Self {
        Self {
            cwd: ".".to_string(),
            model: None,
            max_turns: Some(15),
        }
    }
}

impl SubagentConfig {
    pub fn new(cwd: impl Into<String>) -> Self {
        Self {
            cwd: cwd.into(),
            ..Default::default()
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_max_turns(mut self, max: u32) -> Self {
        self.max_turns = Some(max);
        self
    }
}

/// Detect if the subagent has completed its subtask.
/// Looks for "SUBTASK_COMPLETE" in the final message.
pub fn detect_subtask_complete(final_message: Option<&str>) -> bool {
    match final_message {
        Some(msg) => msg.contains("SUBTASK_COMPLETE"),
        None => false,
    }
}

/// Extract files modified from tool calls.
/// Looks for edit/write tool calls and extracts file paths.
pub fn extract_files_modified(tool_calls: &[LoopToolCall]) -> Vec<String> {
    let mut files: HashSet<String> = HashSet::new();

    for call in tool_calls {
        if call.name == "edit" || call.name == "write" {
            if let Ok(args) = serde_json::from_str::<serde_json::Value>(&call.arguments) {
                if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                    files.insert(path.to_string());
                }
            }
        }
    }

    files.into_iter().collect()
}

/// Extract all tool calls from a loop result
pub fn collect_tool_calls(result: &LoopResult) -> Vec<LoopToolCall> {
    result
        .turns
        .iter()
        .filter_map(|turn| turn.tool_calls.as_ref())
        .flat_map(|calls| calls.iter().cloned())
        .collect()
}

/// Run the minimal coding subagent to complete a single subtask.
///
/// The subagent:
/// - Gets a minimal system prompt (~50 tokens)
/// - Has 4 tools: read, write, edit, bash
/// - Works on one subtask at a time
/// - Outputs SUBTASK_COMPLETE when done
pub async fn run_subagent<P: LlmProvider, T: ToolExecutor>(
    subtask: &Subtask,
    provider: &P,
    tool_executor: &T,
    config: &SubagentConfig,
    mut on_event: Option<impl FnMut(LoopEvent)>,
) -> AgentResult<SubagentResult> {
    let user_prompt = build_subagent_prompt(subtask);

    let loop_config = LoopConfig {
        model: config.model.clone(),
        system_prompt: Some(SUBAGENT_SYSTEM_PROMPT.to_string()),
        max_turns: config.max_turns,
        temperature: Some(0.0),
    };

    let result = match agent_loop(&user_prompt, provider, tool_executor, &loop_config, on_event.as_mut()).await {
        Ok(result) => result,
        Err(e) => {
            return Ok(SubagentResult {
                success: false,
                subtask_id: subtask.id.clone(),
                files_modified: vec![],
                error: Some(e.to_string()),
                turns: 0,
                agent: Some(AgentType::Minimal),
                claude_code_session_id: None,
                claude_code_forked_from_session_id: None,
                token_usage: None,
                verification_outputs: None,
                session_metadata: None,
                learning_metrics: None,
            });
        }
    };

    let tool_calls = collect_tool_calls(&result);
    let files_modified = extract_files_modified(&tool_calls);
    let completed = detect_subtask_complete(result.final_message.as_deref());

    if !completed {
        return Ok(SubagentResult {
            success: false,
            subtask_id: subtask.id.clone(),
            files_modified,
            error: Some("Subtask did not complete - SUBTASK_COMPLETE not found in output".to_string()),
            turns: result.total_turns,
            agent: Some(AgentType::Minimal),
            claude_code_session_id: None,
            claude_code_forked_from_session_id: None,
            token_usage: None,
            verification_outputs: None,
            session_metadata: None,
            learning_metrics: None,
        });
    }

    Ok(SubagentResult {
        success: true,
        subtask_id: subtask.id.clone(),
        files_modified,
        error: None,
        turns: result.total_turns,
        agent: Some(AgentType::Minimal),
        claude_code_session_id: None,
        claude_code_forked_from_session_id: None,
        token_usage: None,
        verification_outputs: None,
        session_metadata: None,
        learning_metrics: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_subtask_complete() {
        assert!(detect_subtask_complete(Some("Done! SUBTASK_COMPLETE")));
        assert!(detect_subtask_complete(Some("SUBTASK_COMPLETE")));
        assert!(!detect_subtask_complete(Some("Not done yet")));
        assert!(!detect_subtask_complete(None));
    }

    #[test]
    fn test_extract_files_modified() {
        let tool_calls = vec![
            LoopToolCall {
                id: "1".to_string(),
                name: "edit".to_string(),
                arguments: r#"{"path": "/tmp/file1.rs"}"#.to_string(),
            },
            LoopToolCall {
                id: "2".to_string(),
                name: "write".to_string(),
                arguments: r#"{"path": "/tmp/file2.rs"}"#.to_string(),
            },
            LoopToolCall {
                id: "3".to_string(),
                name: "read".to_string(),
                arguments: r#"{"path": "/tmp/file3.rs"}"#.to_string(),
            },
        ];

        let files = extract_files_modified(&tool_calls);
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"/tmp/file1.rs".to_string()) || files.contains(&"/tmp/file2.rs".to_string()));
    }

    #[test]
    fn test_subagent_config_builder() {
        let config = SubagentConfig::new("/home/user/project")
            .with_model("claude-3")
            .with_max_turns(20);

        assert_eq!(config.cwd, "/home/user/project");
        assert_eq!(config.model, Some("claude-3".to_string()));
        assert_eq!(config.max_turns, Some(20));
    }
}
