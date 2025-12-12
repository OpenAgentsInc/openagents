//! System prompt construction for Pi agent
//!
//! Builds system prompts including:
//! - Base instructions
//! - Tool descriptions
//! - Project context (git status, file tree)
//! - CLAUDE.md / AGENTS.md files

use std::path::Path;

use tokio::fs;
use tracing::debug;

/// System prompt builder for Pi agent
pub struct SystemPromptBuilder {
    /// Base instructions
    base_instructions: String,

    /// Working directory for context
    working_dir: String,

    /// Whether to include git context
    include_git: bool,

    /// Whether to include file tree
    include_tree: bool,

    /// Max tree depth
    tree_depth: usize,

    /// Custom context sections
    custom_sections: Vec<(String, String)>,
}

impl Default for SystemPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemPromptBuilder {
    /// Create a new system prompt builder
    pub fn new() -> Self {
        Self {
            base_instructions: DEFAULT_BASE_INSTRUCTIONS.to_string(),
            working_dir: ".".to_string(),
            include_git: true,
            include_tree: true,
            tree_depth: 3,
            custom_sections: Vec::new(),
        }
    }

    /// Set base instructions
    pub fn base_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.base_instructions = instructions.into();
        self
    }

    /// Set working directory
    pub fn working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = dir.into();
        self
    }

    /// Include git context
    pub fn include_git(mut self, include: bool) -> Self {
        self.include_git = include;
        self
    }

    /// Include file tree
    pub fn include_tree(mut self, include: bool) -> Self {
        self.include_tree = include;
        self
    }

    /// Set tree depth
    pub fn tree_depth(mut self, depth: usize) -> Self {
        self.tree_depth = depth;
        self
    }

    /// Add a custom section
    pub fn add_section(mut self, title: impl Into<String>, content: impl Into<String>) -> Self {
        self.custom_sections.push((title.into(), content.into()));
        self
    }

    /// Build the system prompt
    pub async fn build(&self, tool_descriptions: &str) -> String {
        let mut sections = Vec::new();

        // Base instructions
        sections.push(self.base_instructions.clone());

        // Tool descriptions
        if !tool_descriptions.is_empty() {
            sections.push(format!("# Available Tools\n\n{}", tool_descriptions));
        }

        // Project context
        let project_context = self.build_project_context().await;
        if !project_context.is_empty() {
            sections.push(format!("# Project Context\n\n{}", project_context));
        }

        // CLAUDE.md / AGENTS.md
        let instructions_file = self.load_instructions_file().await;
        if !instructions_file.is_empty() {
            sections.push(format!("# Project Instructions\n\n{}", instructions_file));
        }

        // Custom sections
        for (title, content) in &self.custom_sections {
            sections.push(format!("# {}\n\n{}", title, content));
        }

        sections.join("\n\n")
    }

    /// Build project context (git status, tree)
    async fn build_project_context(&self) -> String {
        let mut parts = Vec::new();

        // Git status
        if self.include_git {
            if let Some(git_info) = self.get_git_info().await {
                parts.push(git_info);
            }
        }

        // File tree
        if self.include_tree {
            if let Some(tree) = self.get_file_tree().await {
                parts.push(format!("## File Structure\n\n```\n{}\n```", tree));
            }
        }

        parts.join("\n\n")
    }

    /// Get git information
    async fn get_git_info(&self) -> Option<String> {
        let path = Path::new(&self.working_dir);

        // Check if this is a git repo
        if !path.join(".git").exists() {
            return None;
        }

        let mut info = String::new();
        info.push_str("## Git Status\n\n");

        // Get current branch
        if let Ok(output) = tokio::process::Command::new("git")
            .arg("branch")
            .arg("--show-current")
            .current_dir(&self.working_dir)
            .output()
            .await
        {
            if output.status.success() {
                let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !branch.is_empty() {
                    info.push_str(&format!("**Branch:** {}\n", branch));
                }
            }
        }

        // Get short status
        if let Ok(output) = tokio::process::Command::new("git")
            .arg("status")
            .arg("--short")
            .current_dir(&self.working_dir)
            .output()
            .await
        {
            if output.status.success() {
                let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !status.is_empty() {
                    info.push_str(&format!("\n**Changes:**\n```\n{}\n```\n", status));
                } else {
                    info.push_str("\n**Changes:** Working tree clean\n");
                }
            }
        }

        Some(info)
    }

    /// Get simplified file tree
    async fn get_file_tree(&self) -> Option<String> {
        // Use tree command if available, otherwise build manually
        if let Ok(output) = tokio::process::Command::new("tree")
            .arg("-L")
            .arg(self.tree_depth.to_string())
            .arg("--noreport")
            .arg("-I")
            .arg("node_modules|target|.git|__pycache__|.venv|dist|build")
            .current_dir(&self.working_dir)
            .output()
            .await
        {
            if output.status.success() {
                let tree = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !tree.is_empty() {
                    return Some(tree);
                }
            }
        }

        // Fallback: simple ls
        if let Ok(output) = tokio::process::Command::new("ls")
            .arg("-la")
            .current_dir(&self.working_dir)
            .output()
            .await
        {
            if output.status.success() {
                let listing = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Some(listing);
            }
        }

        None
    }

    /// Load CLAUDE.md or AGENTS.md if present
    async fn load_instructions_file(&self) -> String {
        let path = Path::new(&self.working_dir);

        // Try CLAUDE.md first
        let claude_path = path.join("CLAUDE.md");
        if claude_path.exists() {
            if let Ok(content) = fs::read_to_string(&claude_path).await {
                debug!("Loaded CLAUDE.md from {:?}", claude_path);
                return content;
            }
        }

        // Try AGENTS.md
        let agents_path = path.join("AGENTS.md");
        if agents_path.exists() {
            if let Ok(content) = fs::read_to_string(&agents_path).await {
                debug!("Loaded AGENTS.md from {:?}", agents_path);
                return content;
            }
        }

        // Try .claude/instructions.md
        let instructions_path = path.join(".claude/instructions.md");
        if instructions_path.exists() {
            if let Ok(content) = fs::read_to_string(&instructions_path).await {
                debug!("Loaded .claude/instructions.md from {:?}", instructions_path);
                return content;
            }
        }

        String::new()
    }
}

/// Format tool definitions for system prompt
pub fn format_tool_descriptions(tools: &[crate::ToolDefinition]) -> String {
    let mut output = String::new();

    for tool in tools {
        output.push_str(&format!("## {}\n\n", tool.name));
        output.push_str(&format!("{}\n\n", tool.description));

        // Format parameters
        if let Some(properties) = tool.parameters.get("properties") {
            if let Some(props) = properties.as_object() {
                if !props.is_empty() {
                    output.push_str("**Parameters:**\n");
                    for (name, schema) in props {
                        let desc = schema
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("");
                        let typ = schema
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("any");
                        output.push_str(&format!("- `{}` ({}): {}\n", name, typ, desc));
                    }
                    output.push('\n');
                }
            }
        }
    }

    output
}

/// Default base instructions for Pi agent
pub const DEFAULT_BASE_INSTRUCTIONS: &str = r#"You are Pi, an autonomous coding assistant. You help users with software engineering tasks by:

1. Understanding the user's request
2. Planning an approach
3. Using tools to inspect code, make changes, and run commands
4. Iterating until the task is complete

## Guidelines

- **Read before you write**: Always read relevant files before making changes
- **Make targeted changes**: Only modify what's necessary to complete the task
- **Test your work**: Run tests or verify changes work as expected
- **Be iterative**: If something doesn't work, analyze the error and try again
- **Ask for clarification**: If the request is ambiguous, ask the user

## Tool Usage

- Use `bash` to run commands and scripts
- Use `read` to view file contents
- Use `write` to create new files
- Use `edit` to modify existing files

When making changes:
- Start by reading the existing code
- Make minimal, focused changes
- Verify changes work before reporting success

## Output Format

Explain your reasoning before using tools. After making changes, summarize what you did.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_builder() {
        let builder = SystemPromptBuilder::new();
        assert!(builder.include_git);
        assert!(builder.include_tree);
        assert_eq!(builder.tree_depth, 3);
    }

    #[test]
    fn test_builder_chain() {
        let builder = SystemPromptBuilder::new()
            .working_dir("/tmp")
            .include_git(false)
            .include_tree(false)
            .tree_depth(5)
            .add_section("Custom", "Content");

        assert_eq!(builder.working_dir, "/tmp");
        assert!(!builder.include_git);
        assert!(!builder.include_tree);
        assert_eq!(builder.tree_depth, 5);
        assert_eq!(builder.custom_sections.len(), 1);
    }

    #[test]
    fn test_format_tool_descriptions() {
        let tools = vec![crate::ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "arg1": {
                        "type": "string",
                        "description": "First argument"
                    }
                }
            }),
        }];

        let output = format_tool_descriptions(&tools);
        assert!(output.contains("## test_tool"));
        assert!(output.contains("A test tool"));
        assert!(output.contains("`arg1`"));
    }

    #[tokio::test]
    async fn test_build_simple() {
        let builder = SystemPromptBuilder::new()
            .include_git(false)
            .include_tree(false);

        let prompt = builder.build("").await;
        assert!(prompt.contains("You are Pi"));
    }
}
