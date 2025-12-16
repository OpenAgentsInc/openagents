//! System prompt builder for Coder sessions.

use crate::AgentConfig;
use chrono::Utc;
use std::path::Path;

/// Build the system prompt for a session.
pub struct PromptBuilder {
    /// Working directory.
    working_dir: String,
    /// Agent configuration.
    agent_config: AgentConfig,
    /// Custom instructions (from CLAUDE.md files).
    custom_instructions: Vec<String>,
    /// Git status information.
    git_status: Option<String>,
    /// Platform information.
    platform: String,
}

impl PromptBuilder {
    /// Create a new prompt builder.
    pub fn new(working_dir: impl AsRef<Path>, agent_config: AgentConfig) -> Self {
        Self {
            working_dir: working_dir.as_ref().to_string_lossy().to_string(),
            agent_config,
            custom_instructions: Vec::new(),
            git_status: None,
            platform: std::env::consts::OS.to_string(),
        }
    }

    /// Add custom instructions (e.g., from CLAUDE.md).
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.custom_instructions.push(instructions.into());
        self
    }

    /// Add git status information.
    pub fn with_git_status(mut self, status: impl Into<String>) -> Self {
        self.git_status = Some(status.into());
        self
    }

    /// Build the complete system prompt.
    pub fn build(&self) -> String {
        let mut prompt = String::new();

        // Core identity
        prompt.push_str(&format!(
            "You are an AI assistant helping with software development tasks.\n\
             You are running as agent \"{}\" using model \"{}\".\n\n",
            self.agent_config.agent_id, self.agent_config.model_id
        ));

        // Environment context
        prompt.push_str(&format!(
            "## Environment\n\
             - Working directory: {}\n\
             - Platform: {}\n\
             - Date: {}\n\n",
            self.working_dir,
            self.platform,
            Utc::now().format("%Y-%m-%d")
        ));

        // Git status if available
        if let Some(ref git_status) = self.git_status {
            prompt.push_str(&format!("## Git Status\n{}\n\n", git_status));
        }

        // Tool usage guidelines
        prompt.push_str(TOOL_GUIDELINES);

        // Code editing guidelines
        prompt.push_str(EDITING_GUIDELINES);

        // Custom instructions
        if !self.custom_instructions.is_empty() {
            prompt.push_str("\n## Project Instructions\n\n");
            for instruction in &self.custom_instructions {
                prompt.push_str(instruction);
                prompt.push_str("\n\n");
            }
        }

        prompt
    }
}

/// Standard tool usage guidelines.
const TOOL_GUIDELINES: &str = r#"## Tool Usage Guidelines

- Use tools to accomplish tasks. Don't just describe what you would do.
- Read files before editing them to understand the context.
- When editing files, use the edit tool with old_string/new_string.
- Make sure old_string matches exactly what's in the file.
- For searches, use grep for content and glob for file patterns.
- Use bash for running commands, builds, and tests.
- Always check tool results before proceeding.

"#;

/// Code editing guidelines.
const EDITING_GUIDELINES: &str = r#"## Code Editing Guidelines

- Prefer small, focused edits over large rewrites.
- Only edit code that's directly related to the task.
- Don't add unnecessary features or "improvements".
- Keep existing code style and conventions.
- Test changes when possible.

"#;

/// Load CLAUDE.md files from a directory hierarchy.
pub fn load_instructions(working_dir: &Path) -> Vec<String> {
    let mut instructions = Vec::new();
    let mut current = Some(working_dir.to_path_buf());

    // Walk up the directory tree
    while let Some(dir) = current {
        let claude_md = dir.join("CLAUDE.md");
        if claude_md.exists() {
            if let Ok(content) = std::fs::read_to_string(&claude_md) {
                instructions.push(format!(
                    "<!-- From {} -->\n{}",
                    claude_md.display(),
                    content
                ));
            }
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }

    // Reverse so root instructions come first
    instructions.reverse();
    instructions
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_prompt_builder() {
        let config = AgentConfig::default();
        let builder = PromptBuilder::new(PathBuf::from("/home/user/project"), config);

        let prompt = builder.build();

        assert!(prompt.contains("Working directory: /home/user/project"));
        assert!(prompt.contains("Tool Usage Guidelines"));
        assert!(prompt.contains("Code Editing Guidelines"));
    }

    #[test]
    fn test_with_custom_instructions() {
        let config = AgentConfig::default();
        let builder = PromptBuilder::new(PathBuf::from("/project"), config)
            .with_instructions("Use Rust 2024 edition.")
            .with_instructions("Prefer async/await.");

        let prompt = builder.build();

        assert!(prompt.contains("Project Instructions"));
        assert!(prompt.contains("Use Rust 2024 edition"));
        assert!(prompt.contains("Prefer async/await"));
    }

    #[test]
    fn test_with_git_status() {
        let config = AgentConfig::default();
        let builder = PromptBuilder::new(PathBuf::from("/project"), config)
            .with_git_status("On branch main\nnothing to commit");

        let prompt = builder.build();

        assert!(prompt.contains("Git Status"));
        assert!(prompt.contains("On branch main"));
    }
}
