//! Task executor - executes tools and manages agent loop

use crate::{OrchestratorError, OrchestratorResult};
use llm::{ChatOptions, ContentPart, LlmClient, Message, StopReason, ToolDefinition};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tools::{BashResult, BashTool, EditResult, EditTool, FindResult, FindTool, GrepResult, GrepTool, ReadResult, ReadTool, WriteResult, WriteTool};

/// Tool execution result
#[derive(Debug, Clone)]
pub enum ToolResult {
    Read(ReadResult),
    Write(WriteResult),
    Edit(EditResult),
    Grep(GrepResult),
    Find(FindResult),
    Bash(BashResult),
    Custom(Value),
}

impl ToolResult {
    /// Convert to a string representation for the LLM
    pub fn to_string_for_llm(&self) -> String {
        match self {
            ToolResult::Read(r) => {
                if r.truncated {
                    format!(
                        "{}\n\n[File has {} more lines]",
                        r.text,
                        r.remaining_lines.unwrap_or(0)
                    )
                } else {
                    r.text.clone()
                }
            }
            ToolResult::Write(r) => {
                format!(
                    "Wrote {} bytes to {}",
                    r.bytes_written,
                    r.path
                )
            }
            ToolResult::Edit(r) => {
                format!(
                    "Edited {} ({} replacements, +{} -{} lines)\n{}",
                    r.path, r.replacements, r.lines_added, r.lines_removed, r.diff
                )
            }
            ToolResult::Grep(r) => {
                if r.matches.is_empty() {
                    "No matches found".to_string()
                } else {
                    let matches_str: Vec<String> = r
                        .matches
                        .iter()
                        .take(50)
                        .map(|m| format!("{}:{}: {}", m.file, m.line, m.text))
                        .collect();
                    let result = matches_str.join("\n");
                    if r.truncated {
                        format!("{}\n[Results truncated]", result)
                    } else {
                        result
                    }
                }
            }
            ToolResult::Find(r) => {
                if r.files.is_empty() {
                    "No files found".to_string()
                } else {
                    let result = r.files.join("\n");
                    if r.truncated {
                        format!("{}\n[Results truncated]", result)
                    } else {
                        result
                    }
                }
            }
            ToolResult::Bash(r) => {
                if r.success {
                    r.output.clone()
                } else {
                    format!(
                        "Command failed (exit code {:?}):\n{}",
                        r.exit_code, r.output
                    )
                }
            }
            ToolResult::Custom(v) => v.to_string(),
        }
    }
}

/// Tool executor handles tool calls from the LLM
pub struct ToolExecutor {
    /// Working directory
    working_dir: String,
    /// Custom tool handlers
    custom_handlers: HashMap<String, Box<dyn ToolHandler>>,
    /// Safe mode (restrict dangerous operations)
    safe_mode: bool,
    /// Dry run mode (don't actually execute)
    dry_run: bool,
}

/// Trait for custom tool handlers
pub trait ToolHandler: Send + Sync {
    /// Execute the tool with given input
    fn execute(&self, input: &Value) -> OrchestratorResult<Value>;

    /// Get the tool definition
    fn definition(&self) -> ToolDefinition;
}

impl ToolExecutor {
    /// Create a new tool executor
    pub fn new(working_dir: impl Into<String>) -> Self {
        Self {
            working_dir: working_dir.into(),
            custom_handlers: HashMap::new(),
            safe_mode: true,
            dry_run: false,
        }
    }

    /// Enable/disable safe mode
    pub fn with_safe_mode(mut self, safe_mode: bool) -> Self {
        self.safe_mode = safe_mode;
        self
    }

    /// Enable/disable dry run
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Register a custom tool handler
    pub fn register_tool(&mut self, name: impl Into<String>, handler: Box<dyn ToolHandler>) {
        self.custom_handlers.insert(name.into(), handler);
    }

    /// Get all tool definitions
    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        let mut tools = vec![
            self.read_tool_definition(),
            self.write_tool_definition(),
            self.edit_tool_definition(),
            self.grep_tool_definition(),
            self.find_tool_definition(),
            self.bash_tool_definition(),
        ];

        // Add custom tools
        for handler in self.custom_handlers.values() {
            tools.push(handler.definition());
        }

        tools
    }

    /// Execute a tool call
    pub fn execute(&self, tool_name: &str, input: &Value) -> OrchestratorResult<ToolResult> {
        // Check for custom handler first
        if let Some(handler) = self.custom_handlers.get(tool_name) {
            let result = handler.execute(input)?;
            return Ok(ToolResult::Custom(result));
        }

        // Built-in tools
        match tool_name {
            "read" | "Read" => self.execute_read(input),
            "write" | "Write" => self.execute_write(input),
            "edit" | "Edit" => self.execute_edit(input),
            "grep" | "Grep" => self.execute_grep(input),
            "find" | "Find" => self.execute_find(input),
            "bash" | "Bash" => self.execute_bash(input),
            _ => Err(OrchestratorError::ToolError(format!(
                "Unknown tool: {}",
                tool_name
            ))),
        }
    }

    fn execute_read(&self, input: &Value) -> OrchestratorResult<ToolResult> {
        let path = input["file_path"]
            .as_str()
            .ok_or_else(|| OrchestratorError::ToolError("Missing file_path".into()))?;

        let offset = input["offset"].as_u64().map(|v| v as usize);
        let limit = input["limit"].as_u64().map(|v| v as usize);

        if self.dry_run {
            return Ok(ToolResult::Read(ReadResult {
                text: "[DRY RUN] Would read file".to_string(),
                path: path.to_string(),
                size_bytes: 0,
                total_lines: None,
                start_line: None,
                end_line: None,
                lines_returned: None,
                remaining_lines: None,
                truncated_lines: 0,
                truncated: false,
                mime_type: None,
            }));
        }

        let result = ReadTool::read(path, offset, limit)?;
        Ok(ToolResult::Read(result))
    }

    fn execute_write(&self, input: &Value) -> OrchestratorResult<ToolResult> {
        let path = input["file_path"]
            .as_str()
            .ok_or_else(|| OrchestratorError::ToolError("Missing file_path".into()))?;
        let content = input["content"]
            .as_str()
            .ok_or_else(|| OrchestratorError::ToolError("Missing content".into()))?;

        if self.safe_mode {
            // Check for potentially dangerous paths
            if path.contains("..") || path.starts_with("/etc") || path.starts_with("/usr") {
                return Err(OrchestratorError::SafeModeViolation(format!(
                    "Cannot write to path: {}",
                    path
                )));
            }
        }

        if self.dry_run {
            return Ok(ToolResult::Write(WriteResult {
                path: path.to_string(),
                bytes_written: content.len(),
                existed_before: false,
                previous_size: None,
                new_size: content.len() as u64,
                duration_ms: 0,
            }));
        }

        let result = WriteTool::write(path, content)?;
        Ok(ToolResult::Write(result))
    }

    fn execute_edit(&self, input: &Value) -> OrchestratorResult<ToolResult> {
        let path = input["file_path"]
            .as_str()
            .ok_or_else(|| OrchestratorError::ToolError("Missing file_path".into()))?;
        let old_string = input["old_string"]
            .as_str()
            .ok_or_else(|| OrchestratorError::ToolError("Missing old_string".into()))?;
        let new_string = input["new_string"]
            .as_str()
            .ok_or_else(|| OrchestratorError::ToolError("Missing new_string".into()))?;
        let replace_all = input["replace_all"].as_bool().unwrap_or(false);

        if self.safe_mode {
            if path.contains("..") || path.starts_with("/etc") || path.starts_with("/usr") {
                return Err(OrchestratorError::SafeModeViolation(format!(
                    "Cannot edit path: {}",
                    path
                )));
            }
        }

        if self.dry_run {
            return Ok(ToolResult::Edit(EditResult {
                path: path.to_string(),
                diff: "[DRY RUN] Would edit file".to_string(),
                old_length: old_string.len(),
                new_length: new_string.len(),
                delta: (new_string.len() as i64) - (old_string.len() as i64),
                lines_added: 0,
                lines_removed: 0,
                replacements: 1,
            }));
        }

        let result = EditTool::edit(path, old_string, new_string, replace_all)?;
        Ok(ToolResult::Edit(result))
    }

    fn execute_grep(&self, input: &Value) -> OrchestratorResult<ToolResult> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| OrchestratorError::ToolError("Missing pattern".into()))?;
        let path = input["path"]
            .as_str()
            .unwrap_or(&self.working_dir);
        let max_results = input["max_results"].as_u64().map(|v| v as usize);

        let result = GrepTool::search(pattern, path, max_results)?;
        Ok(ToolResult::Grep(result))
    }

    fn execute_find(&self, input: &Value) -> OrchestratorResult<ToolResult> {
        let path = input["path"]
            .as_str()
            .unwrap_or(&self.working_dir);
        let pattern = input["pattern"].as_str();
        let glob = input["glob"].as_str();
        let max_results = input["max_results"].as_u64().map(|v| v as usize);

        let result = if let Some(g) = glob {
            FindTool::find_glob(path, g, max_results)?
        } else {
            FindTool::find(path, pattern, max_results)?
        };

        Ok(ToolResult::Find(result))
    }

    fn execute_bash(&self, input: &Value) -> OrchestratorResult<ToolResult> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| OrchestratorError::ToolError("Missing command".into()))?;
        let timeout_ms = input["timeout"].as_u64();

        if self.safe_mode {
            // Check for dangerous commands
            let dangerous = [
                "rm -rf /",
                "rm -rf /*",
                "dd if=",
                "mkfs",
                "> /dev/",
                "chmod 777",
                "curl | sh",
                "wget | sh",
            ];
            for d in dangerous {
                if command.contains(d) {
                    return Err(OrchestratorError::SafeModeViolation(format!(
                        "Dangerous command blocked: {}",
                        command
                    )));
                }
            }
        }

        if self.dry_run {
            return Ok(ToolResult::Bash(BashResult {
                command: command.to_string(),
                exit_code: Some(0),
                stdout: "[DRY RUN] Would execute command".to_string(),
                stderr: String::new(),
                output: "[DRY RUN] Would execute command".to_string(),
                success: true,
                duration_ms: 0,
                truncated: false,
                timed_out: false,
                cwd: Some(self.working_dir.clone()),
            }));
        }

        let result = if let Some(timeout) = timeout_ms {
            BashTool::execute_with_timeout(command, timeout)?
        } else {
            BashTool::execute(command)?
        };

        Ok(ToolResult::Bash(result))
    }

    // Tool definitions

    fn read_tool_definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "read",
            "Read the contents of a file",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The absolute path to the file to read"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line number to start reading from (1-indexed)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read"
                    }
                },
                "required": ["file_path"]
            }),
        )
    }

    fn write_tool_definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "write",
            "Write content to a file, creating it if it doesn't exist",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The absolute path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    }
                },
                "required": ["file_path", "content"]
            }),
        )
    }

    fn edit_tool_definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "edit",
            "Edit a file by replacing text",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The absolute path to the file to edit"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "The text to find and replace"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "The replacement text"
                    },
                    "replace_all": {
                        "type": "boolean",
                        "description": "If true, replace all occurrences"
                    }
                },
                "required": ["file_path", "old_string", "new_string"]
            }),
        )
    }

    fn grep_tool_definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "grep",
            "Search for a pattern in files",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The regex pattern to search for"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory or file to search in"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return"
                    }
                },
                "required": ["pattern"]
            }),
        )
    }

    fn find_tool_definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "find",
            "Find files by name or glob pattern",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory to search in"
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Substring to match in filename"
                    },
                    "glob": {
                        "type": "string",
                        "description": "Glob pattern (e.g., **/*.rs)"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results"
                    }
                },
                "required": []
            }),
        )
    }

    fn bash_tool_definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "bash",
            "Execute a bash command",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in milliseconds"
                    }
                },
                "required": ["command"]
            }),
        )
    }
}

/// Agent executor - runs the agent loop with tool calling
pub struct AgentExecutor {
    llm: Arc<LlmClient>,
    tool_executor: ToolExecutor,
    max_turns: usize,
}

impl AgentExecutor {
    /// Create a new agent executor
    pub fn new(llm: Arc<LlmClient>, tool_executor: ToolExecutor) -> Self {
        Self {
            llm,
            tool_executor,
            max_turns: 50,
        }
    }

    /// Set maximum turns
    pub fn with_max_turns(mut self, max: usize) -> Self {
        self.max_turns = max;
        self
    }

    /// Execute a task with the agent
    pub async fn execute(
        &self,
        system_prompt: &str,
        initial_message: &str,
    ) -> OrchestratorResult<AgentResult> {
        let mut messages = vec![Message::user(initial_message)];
        let mut tool_calls = 0;
        let mut total_tokens = llm::Usage::default();

        let options = ChatOptions::default()
            .system(system_prompt)
            .tools(self.tool_executor.tool_definitions());

        for turn in 0..self.max_turns {
            let response = self.llm.chat(&messages, Some(options.clone())).await?;
            total_tokens.input_tokens += response.usage.input_tokens;
            total_tokens.output_tokens += response.usage.output_tokens;

            // Check for tool use
            if response.has_tool_use() {
                // Process each tool use
                let mut tool_results = Vec::new();

                for part in &response.content {
                    if let ContentPart::ToolUse { id, name, input } = part {
                        tool_calls += 1;
                        let result = self.tool_executor.execute(name, input);
                        let (content, is_error) = match result {
                            Ok(r) => (r.to_string_for_llm(), false),
                            Err(e) => (e.to_string(), true),
                        };
                        tool_results.push(ContentPart::tool_result(id, content, is_error));
                    }
                }

                // Add assistant message with tool uses
                messages.push(Message {
                    role: llm::Role::Assistant,
                    content: llm::Content::Parts(response.content.clone()),
                    name: None,
                });

                // Add tool results
                messages.push(Message {
                    role: llm::Role::User,
                    content: llm::Content::Parts(tool_results),
                    name: None,
                });
            } else {
                // No tool use - we're done
                return Ok(AgentResult {
                    success: true,
                    final_message: response.text(),
                    turns: turn + 1,
                    tool_calls,
                    tokens: total_tokens,
                });
            }

            // Check stop reason
            if response.stop_reason == Some(StopReason::EndTurn) {
                return Ok(AgentResult {
                    success: true,
                    final_message: response.text(),
                    turns: turn + 1,
                    tool_calls,
                    tokens: total_tokens,
                });
            }
        }

        // Exceeded max turns
        Err(OrchestratorError::MaxRetriesExceeded(
            "Agent exceeded maximum turns".to_string(),
        ))
    }
}

/// Result of an agent execution
#[derive(Debug)]
pub struct AgentResult {
    /// Whether the execution succeeded
    pub success: bool,
    /// Final message from the agent
    pub final_message: String,
    /// Number of turns taken
    pub turns: usize,
    /// Number of tool calls made
    pub tool_calls: usize,
    /// Token usage
    pub tokens: llm::Usage,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_executor_creation() {
        let executor = ToolExecutor::new("/tmp");
        let tools = executor.tool_definitions();
        assert_eq!(tools.len(), 6); // read, write, edit, grep, find, bash
    }

    #[test]
    fn test_safe_mode_blocks_dangerous() {
        let executor = ToolExecutor::new("/tmp").with_safe_mode(true);

        let result = executor.execute(
            "write",
            &serde_json::json!({
                "file_path": "/etc/passwd",
                "content": "bad"
            }),
        );

        assert!(matches!(result, Err(OrchestratorError::SafeModeViolation(_))));
    }

    #[test]
    fn test_dry_run_mode() {
        let executor = ToolExecutor::new("/tmp").with_dry_run(true);

        let result = executor
            .execute(
                "write",
                &serde_json::json!({
                    "file_path": "/tmp/test.txt",
                    "content": "hello"
                }),
            )
            .unwrap();

        if let ToolResult::Write(r) = result {
            assert!(r.path.contains("test.txt"));
        } else {
            panic!("Expected Write result");
        }
    }
}
