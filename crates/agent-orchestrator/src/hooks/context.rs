use async_trait::async_trait;
use std::path::{Path, PathBuf};

use super::{ContextBuilder, Hook, HookResult};

pub struct ContextInjectionHook {
    workspace: PathBuf,
    inject_agents_md: bool,
    inject_readme: bool,
    custom_files: Vec<PathBuf>,
}

impl ContextInjectionHook {
    pub fn new(workspace: impl Into<PathBuf>) -> Self {
        Self {
            workspace: workspace.into(),
            inject_agents_md: true,
            inject_readme: true,
            custom_files: Vec::new(),
        }
    }

    pub fn without_agents_md(mut self) -> Self {
        self.inject_agents_md = false;
        self
    }

    pub fn without_readme(mut self) -> Self {
        self.inject_readme = false;
        self
    }

    pub fn with_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.custom_files.push(path.into());
        self
    }

    fn read_file_if_exists(&self, path: &Path) -> Option<String> {
        if path.exists() {
            std::fs::read_to_string(path).ok()
        } else {
            None
        }
    }
}

#[async_trait]
impl Hook for ContextInjectionHook {
    fn name(&self) -> &str {
        "context-injection"
    }

    fn priority(&self) -> i32 {
        90
    }

    async fn inject_context(&self, context: &mut ContextBuilder) -> HookResult {
        if self.inject_agents_md {
            let agents_path = self.workspace.join("AGENTS.md");
            if let Some(content) = self.read_file_if_exists(&agents_path) {
                context.add_section("Agent Instructions", content, 100);
            }
        }

        if self.inject_readme {
            let readme_path = self.workspace.join("README.md");
            if let Some(content) = self.read_file_if_exists(&readme_path) {
                let truncated = if content.len() > 5000 {
                    format!("{}...\n\n(README truncated)", &content[..5000])
                } else {
                    content
                };
                context.add_section("Project README", truncated, 50);
            }
        }

        for file_path in &self.custom_files {
            let full_path = if file_path.is_absolute() {
                file_path.clone()
            } else {
                self.workspace.join(file_path)
            };

            if let Some(content) = self.read_file_if_exists(&full_path) {
                let name = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Custom File");
                context.add_section(name, content, 30);
            }
        }

        HookResult::Continue
    }
}

pub struct DirectiveInjectionHook {
    workspace: PathBuf,
}

impl DirectiveInjectionHook {
    pub fn new(workspace: impl Into<PathBuf>) -> Self {
        Self {
            workspace: workspace.into(),
        }
    }

    fn load_directives(&self) -> Vec<(String, String)> {
        let directives_dir = self.workspace.join(".openagents/directives");
        if !directives_dir.exists() {
            return Vec::new();
        }

        let mut directives = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&directives_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_markdown = path.extension().is_some_and(|e| e == "md");
                if !is_markdown {
                    continue;
                }
                let Ok(content) = std::fs::read_to_string(&path) else {
                    continue;
                };
                if content.contains("status: active") {
                    let name = path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("directive")
                        .to_string();
                    directives.push((name, content));
                }
            }
        }
        directives
    }
}

#[async_trait]
impl Hook for DirectiveInjectionHook {
    fn name(&self) -> &str {
        "directive-injection"
    }

    fn priority(&self) -> i32 {
        80
    }

    async fn inject_context(&self, context: &mut ContextBuilder) -> HookResult {
        let directives = self.load_directives();

        if !directives.is_empty() {
            let summary = directives
                .iter()
                .map(|(name, _)| format!("- {}", name))
                .collect::<Vec<_>>()
                .join("\n");

            context.add_section(
                "Active Directives",
                format!("The following directives are active:\n\n{}", summary),
                85,
            );
        }

        HookResult::Continue
    }
}

pub struct CompactionContextHook {
    preserved_sections: Vec<String>,
}

impl CompactionContextHook {
    pub fn new() -> Self {
        Self {
            preserved_sections: vec![
                "Current Task".to_string(),
                "Todo List".to_string(),
                "Working Files".to_string(),
            ],
        }
    }

    pub fn preserve_section(mut self, section: impl Into<String>) -> Self {
        self.preserved_sections.push(section.into());
        self
    }
}

impl Default for CompactionContextHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Hook for CompactionContextHook {
    fn name(&self) -> &str {
        "compaction-context"
    }

    fn priority(&self) -> i32 {
        95
    }

    async fn inject_context(&self, context: &mut ContextBuilder) -> HookResult {
        context.add_section(
            "Preserved Context Sections",
            format!(
                "The following sections should be preserved during context compaction:\n{}",
                self.preserved_sections
                    .iter()
                    .map(|s| format!("- {}", s))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            95,
        );

        HookResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_workspace() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# Agent Instructions\n\nTest content").unwrap();
        fs::write(
            dir.path().join("README.md"),
            "# Project\n\nThis is a test project.",
        )
        .unwrap();
        dir
    }

    #[tokio::test]
    async fn context_injection_adds_agents_md() {
        let workspace = setup_workspace();
        let hook = ContextInjectionHook::new(workspace.path());
        let mut context = ContextBuilder::new();

        hook.inject_context(&mut context).await;

        let built = context.build();
        assert!(built.contains("Agent Instructions"));
        assert!(built.contains("Test content"));
    }

    #[tokio::test]
    async fn context_injection_adds_readme() {
        let workspace = setup_workspace();
        let hook = ContextInjectionHook::new(workspace.path());
        let mut context = ContextBuilder::new();

        hook.inject_context(&mut context).await;

        let built = context.build();
        assert!(built.contains("Project README"));
        assert!(built.contains("This is a test project"));
    }

    #[tokio::test]
    async fn context_injection_without_agents_md() {
        let workspace = setup_workspace();
        let hook = ContextInjectionHook::new(workspace.path()).without_agents_md();
        let mut context = ContextBuilder::new();

        hook.inject_context(&mut context).await;

        let built = context.build();
        assert!(!built.contains("Agent Instructions"));
        assert!(built.contains("Project README"));
    }

    #[tokio::test]
    async fn context_injection_custom_file() {
        let workspace = setup_workspace();
        fs::write(workspace.path().join("CUSTOM.md"), "Custom content here").unwrap();

        let hook = ContextInjectionHook::new(workspace.path()).with_file("CUSTOM.md");
        let mut context = ContextBuilder::new();

        hook.inject_context(&mut context).await;

        let built = context.build();
        assert!(built.contains("Custom content here"));
    }

    #[tokio::test]
    async fn context_injection_missing_files_ok() {
        let workspace = TempDir::new().unwrap();
        let hook = ContextInjectionHook::new(workspace.path());
        let mut context = ContextBuilder::new();

        let result = hook.inject_context(&mut context).await;
        assert!(matches!(result, HookResult::Continue));
    }

    #[tokio::test]
    async fn directive_injection_loads_active() {
        let workspace = TempDir::new().unwrap();
        let directives_dir = workspace.path().join(".openagents/directives");
        fs::create_dir_all(&directives_dir).unwrap();
        fs::write(
            directives_dir.join("d-001.md"),
            "---\nstatus: active\n---\n\n# Directive 1",
        )
        .unwrap();
        fs::write(
            directives_dir.join("d-002.md"),
            "---\nstatus: completed\n---\n\n# Directive 2",
        )
        .unwrap();

        let hook = DirectiveInjectionHook::new(workspace.path());
        let mut context = ContextBuilder::new();

        hook.inject_context(&mut context).await;

        let built = context.build();
        assert!(built.contains("d-001"));
        assert!(!built.contains("d-002"));
    }

    #[tokio::test]
    async fn compaction_context_lists_preserved() {
        let hook = CompactionContextHook::new().preserve_section("Custom Section");
        let mut context = ContextBuilder::new();

        hook.inject_context(&mut context).await;

        let built = context.build();
        assert!(built.contains("Current Task"));
        assert!(built.contains("Todo List"));
        assert!(built.contains("Custom Section"));
    }
}
