//! Tool registry - Read, Edit, Bash, Glob, Grep.
//!
//! These are the core tools Adjutant uses to do work.

use crate::AdjutantError;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

/// Available tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// Read file contents
    Read,
    /// Edit file contents
    Edit,
    /// Execute bash commands
    Bash,
    /// Find files by pattern
    Glob,
    /// Search file contents
    Grep,
}

/// Result of a tool operation.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    /// Whether the operation succeeded
    pub success: bool,
    /// Output content
    pub content: String,
    /// Error message if failed
    pub error: Option<String>,
}

impl ToolOutput {
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
            error: None,
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            content: String::new(),
            error: Some(error.into()),
        }
    }
}

/// Registry of available tools.
#[derive(Clone)]
pub struct ToolRegistry {
    /// Workspace root for relative paths
    workspace_root: PathBuf,
}

impl ToolRegistry {
    /// Create a new tool registry.
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }

    /// Get the workspace root path.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Read a file's contents.
    pub async fn read(&self, path: &Path) -> Result<ToolOutput, AdjutantError> {
        let full_path = self.resolve_path(path);
        tracing::debug!("Reading file: {}", full_path.display());

        match std::fs::read_to_string(&full_path) {
            Ok(content) => Ok(ToolOutput::success(content)),
            Err(e) => Ok(ToolOutput::failure(format!(
                "Failed to read {}: {}",
                full_path.display(),
                e
            ))),
        }
    }

    /// Edit a file by replacing old_string with new_string.
    pub async fn edit(
        &self,
        path: &Path,
        old_string: &str,
        new_string: &str,
    ) -> Result<ToolOutput, AdjutantError> {
        let full_path = self.resolve_path(path);
        tracing::debug!("Editing file: {}", full_path.display());

        // Read current content
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolOutput::failure(format!(
                    "Failed to read {}: {}",
                    full_path.display(),
                    e
                )))
            }
        };

        // Check if old_string exists
        if !content.contains(old_string) {
            return Ok(ToolOutput::failure(format!(
                "old_string not found in {}",
                full_path.display()
            )));
        }

        // Check for uniqueness
        let count = content.matches(old_string).count();
        if count > 1 {
            return Ok(ToolOutput::failure(format!(
                "old_string matches {} times in {} - must be unique",
                count,
                full_path.display()
            )));
        }

        // Replace and write
        let new_content = content.replace(old_string, new_string);
        match std::fs::write(&full_path, &new_content) {
            Ok(_) => Ok(ToolOutput::success(format!(
                "Edited {}",
                full_path.display()
            ))),
            Err(e) => Ok(ToolOutput::failure(format!(
                "Failed to write {}: {}",
                full_path.display(),
                e
            ))),
        }
    }

    /// Write a new file.
    pub async fn write(&self, path: &Path, content: &str) -> Result<ToolOutput, AdjutantError> {
        let full_path = self.resolve_path(path);
        tracing::debug!("Writing file: {}", full_path.display());

        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            if !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return Ok(ToolOutput::failure(format!(
                        "Failed to create directories for {}: {}",
                        full_path.display(),
                        e
                    )));
                }
            }
        }

        match std::fs::write(&full_path, content) {
            Ok(_) => Ok(ToolOutput::success(format!(
                "Wrote {}",
                full_path.display()
            ))),
            Err(e) => Ok(ToolOutput::failure(format!(
                "Failed to write {}: {}",
                full_path.display(),
                e
            ))),
        }
    }

    /// Execute a bash command.
    pub async fn bash(&self, command: &str) -> Result<ToolOutput, AdjutantError> {
        tracing::debug!("Executing: {}", command);

        let output = Command::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir(&self.workspace_root)
            .output()
            .map_err(|e| AdjutantError::ToolError(format!("Failed to execute bash: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(ToolOutput::success(stdout))
        } else {
            Ok(ToolOutput {
                success: false,
                content: stdout,
                error: Some(stderr),
            })
        }
    }

    /// Find files matching a glob pattern.
    pub async fn glob(&self, pattern: &str) -> Result<ToolOutput, AdjutantError> {
        tracing::debug!("Globbing: {}", pattern);

        let matcher = match glob::Pattern::new(pattern) {
            Ok(m) => m,
            Err(e) => return Ok(ToolOutput::failure(format!("Invalid pattern: {}", e))),
        };

        let mut matches = Vec::new();

        for entry in WalkDir::new(&self.workspace_root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // Skip hidden directories and common ignore patterns
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') && name != "target" && name != "node_modules"
            })
        {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            if path.is_file() {
                let relative = path
                    .strip_prefix(&self.workspace_root)
                    .unwrap_or(path);
                let relative_str = relative.to_string_lossy();

                if matcher.matches(&relative_str) {
                    matches.push(relative.to_path_buf());
                }
            }
        }

        // Sort by path
        matches.sort();

        let result = matches
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolOutput::success(result))
    }

    /// Search for a pattern in files.
    pub async fn grep(&self, pattern: &str, path: Option<&Path>) -> Result<ToolOutput, AdjutantError> {
        tracing::debug!("Grepping: {} in {:?}", pattern, path);

        let search_path = path
            .map(|p| self.resolve_path(p))
            .unwrap_or_else(|| self.workspace_root.clone());

        // Use ripgrep if available, otherwise fall back to grep
        let rg_path = which::which("rg").ok();

        let output = if let Some(rg) = rg_path {
            Command::new(rg)
                .arg("--line-number")
                .arg("--no-heading")
                .arg("--color=never")
                .arg(pattern)
                .arg(&search_path)
                .output()
        } else {
            Command::new("grep")
                .arg("-r")
                .arg("-n")
                .arg(pattern)
                .arg(&search_path)
                .output()
        };

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                // grep returns exit code 1 for no matches, which is not an error
                if out.status.success() || out.status.code() == Some(1) {
                    Ok(ToolOutput::success(stdout))
                } else {
                    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                    Ok(ToolOutput::failure(stderr))
                }
            }
            Err(e) => Ok(ToolOutput::failure(format!("grep failed: {}", e))),
        }
    }

    /// List files in a directory.
    pub async fn ls(&self, path: &Path) -> Result<ToolOutput, AdjutantError> {
        let full_path = self.resolve_path(path);
        tracing::debug!("Listing: {}", full_path.display());

        let entries = match std::fs::read_dir(&full_path) {
            Ok(e) => e,
            Err(e) => {
                return Ok(ToolOutput::failure(format!(
                    "Failed to list {}: {}",
                    full_path.display(),
                    e
                )))
            }
        };

        let mut files = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                files.push(format!("{}/", name));
            } else {
                files.push(name);
            }
        }

        files.sort();
        Ok(ToolOutput::success(files.join("\n")))
    }

    /// Resolve a path relative to workspace root.
    fn resolve_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_root.join(path)
        }
    }
}
