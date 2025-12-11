//! Claude Code MCP Tools
//!
//! MCP (Model Context Protocol) tool definitions for Claude Code integration.
//! These tools allow the subagent to signal completion, request help, and read progress.

use serde::{Deserialize, Serialize};

/// MCP server name for MechaCoder
pub const CLAUDE_CODE_MCP_SERVER_NAME: &str = "mechacoder";
/// MCP server version
pub const CLAUDE_CODE_MCP_VERSION: &str = "1.0.0";

/// Options for MechaCoder MCP tools
#[derive(Debug, Clone, Default)]
pub struct MechaCoderMcpOptions {
    /// Path to .openagents directory
    pub openagents_dir: Option<String>,
}

/// Subtask completion signal
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtaskCompleteArgs {
    /// Brief summary of what was done
    pub summary: String,
    /// List of modified files
    #[serde(default)]
    pub files_modified: Vec<String>,
}

/// Help request from subagent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestHelpArgs {
    /// Issue encountered
    pub issue: String,
    /// Suggested resolution
    pub suggestion: Option<String>,
}

/// MCP tool response content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// MCP tool response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResponse {
    pub content: Vec<McpToolContent>,
}

impl McpToolResponse {
    /// Create a text response
    pub fn text(message: impl Into<String>) -> Self {
        Self {
            content: vec![McpToolContent {
                content_type: "text".to_string(),
                text: message.into(),
            }],
        }
    }
}

/// Handle subtask_complete tool call
pub fn handle_subtask_complete(args: &SubtaskCompleteArgs) -> McpToolResponse {
    let text = if args.files_modified.is_empty() {
        format!("Subtask complete: {}", args.summary)
    } else {
        format!(
            "Subtask complete: {} (files: {})",
            args.summary,
            args.files_modified.join(", ")
        )
    };
    McpToolResponse::text(text)
}

/// Handle request_help tool call
pub fn handle_request_help(args: &RequestHelpArgs) -> McpToolResponse {
    let text = match &args.suggestion {
        Some(suggestion) => format!("Help requested: {}\nSuggested: {}", args.issue, suggestion),
        None => format!("Help requested: {}", args.issue),
    };
    McpToolResponse::text(text)
}

/// Handle read_progress tool call
pub fn handle_read_progress(openagents_dir: Option<&str>) -> McpToolResponse {
    let text = match openagents_dir {
        Some(dir) => {
            match crate::progress::read_progress(dir) {
                Some(progress) => {
                    serde_json::to_string_pretty(&progress)
                        .unwrap_or_else(|_| "Error serializing progress".to_string())
                }
                None => {
                    let path = crate::types::get_progress_path(dir);
                    format!("No progress file found at {}", path)
                }
            }
        }
        None => "Progress unavailable: openagentsDir not provided.".to_string(),
    };
    McpToolResponse::text(text)
}

/// Get the list of allowed Claude Code tools
pub fn get_allowed_claude_code_tools(server_name: Option<&str>) -> Vec<String> {
    let server = server_name.unwrap_or(CLAUDE_CODE_MCP_SERVER_NAME);
    vec![
        "Read".to_string(),
        "Write".to_string(),
        "Edit".to_string(),
        "Bash".to_string(),
        "Glob".to_string(),
        "Grep".to_string(),
        format!("mcp__{}__subtask_complete", server),
        format!("mcp__{}__request_help", server),
        format!("mcp__{}__read_progress", server),
    ]
}

/// MCP tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// JSON schema for input parameters
    pub input_schema: serde_json::Value,
}

/// Get MechaCoder MCP tool definitions
pub fn get_mechacoder_mcp_tools() -> Vec<McpToolDefinition> {
    vec![
        McpToolDefinition {
            name: "subtask_complete".to_string(),
            description: "Signal that the current subtask is complete".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "summary": {
                        "type": "string",
                        "description": "Brief summary of what was done"
                    },
                    "filesModified": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of modified files"
                    }
                },
                "required": ["summary"]
            }),
        },
        McpToolDefinition {
            name: "request_help".to_string(),
            description: "Request orchestrator intervention when stuck".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "issue": {
                        "type": "string",
                        "description": "Issue encountered"
                    },
                    "suggestion": {
                        "type": "string",
                        "description": "Suggested resolution"
                    }
                },
                "required": ["issue"]
            }),
        },
        McpToolDefinition {
            name: "read_progress".to_string(),
            description: "Read the current session progress file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subtask_complete_without_files() {
        let args = SubtaskCompleteArgs {
            summary: "Fixed the bug".to_string(),
            files_modified: vec![],
        };
        let response = handle_subtask_complete(&args);
        assert_eq!(response.content.len(), 1);
        assert!(response.content[0].text.contains("Fixed the bug"));
        assert!(!response.content[0].text.contains("files:"));
    }

    #[test]
    fn test_subtask_complete_with_files() {
        let args = SubtaskCompleteArgs {
            summary: "Added feature".to_string(),
            files_modified: vec!["src/main.rs".to_string(), "src/lib.rs".to_string()],
        };
        let response = handle_subtask_complete(&args);
        assert!(response.content[0].text.contains("Added feature"));
        assert!(response.content[0].text.contains("src/main.rs"));
        assert!(response.content[0].text.contains("src/lib.rs"));
    }

    #[test]
    fn test_request_help_without_suggestion() {
        let args = RequestHelpArgs {
            issue: "Compilation error".to_string(),
            suggestion: None,
        };
        let response = handle_request_help(&args);
        assert!(response.content[0].text.contains("Compilation error"));
        assert!(!response.content[0].text.contains("Suggested:"));
    }

    #[test]
    fn test_request_help_with_suggestion() {
        let args = RequestHelpArgs {
            issue: "Missing dependency".to_string(),
            suggestion: Some("Add to Cargo.toml".to_string()),
        };
        let response = handle_request_help(&args);
        assert!(response.content[0].text.contains("Missing dependency"));
        assert!(response.content[0].text.contains("Add to Cargo.toml"));
    }

    #[test]
    fn test_read_progress_no_dir() {
        let response = handle_read_progress(None);
        assert!(response.content[0].text.contains("Progress unavailable"));
    }

    #[test]
    fn test_get_allowed_tools_default() {
        let tools = get_allowed_claude_code_tools(None);
        assert!(tools.contains(&"Read".to_string()));
        assert!(tools.contains(&"mcp__mechacoder__subtask_complete".to_string()));
    }

    #[test]
    fn test_get_allowed_tools_custom_server() {
        let tools = get_allowed_claude_code_tools(Some("custom"));
        assert!(tools.contains(&"mcp__custom__subtask_complete".to_string()));
    }

    #[test]
    fn test_mechacoder_mcp_tools() {
        let tools = get_mechacoder_mcp_tools();
        assert_eq!(tools.len(), 3);
        assert!(tools.iter().any(|t| t.name == "subtask_complete"));
        assert!(tools.iter().any(|t| t.name == "request_help"));
        assert!(tools.iter().any(|t| t.name == "read_progress"));
    }
}
