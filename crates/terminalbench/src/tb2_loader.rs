//! TB2 Task Loader - Scans and parses Terminal-Bench 2 task directories
//!
//! This module loads TB2 task definitions from the filesystem, parsing
//! task.toml configuration and instruction.md task descriptions.

use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Default TB2 root directory
pub const DEFAULT_TB2_ROOT: &str = "/home/christopherdavid/code/terminal-bench-2";

/// TB2 task configuration from task.toml
#[derive(Debug, Clone, Deserialize)]
pub struct TaskToml {
    pub version: String,
    pub metadata: TaskMetadata,
    pub verifier: VerifierConfig,
    pub agent: AgentConfig,
    pub environment: EnvironmentConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskMetadata {
    pub author_name: String,
    #[serde(default)]
    pub author_email: Option<String>,
    pub difficulty: String,
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub expert_time_estimate_min: Option<f64>,
    #[serde(default)]
    pub junior_time_estimate_min: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VerifierConfig {
    pub timeout_sec: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub timeout_sec: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnvironmentConfig {
    #[serde(default)]
    pub build_timeout_sec: Option<f64>,
    pub docker_image: String,
    #[serde(default = "default_cpus")]
    pub cpus: u32,
    #[serde(default = "default_memory")]
    pub memory: String,
    #[serde(default = "default_storage")]
    pub storage: String,
}

fn default_cpus() -> u32 {
    1
}

fn default_memory() -> String {
    "2G".to_string()
}

fn default_storage() -> String {
    "10G".to_string()
}

/// Summary of a TB2 task (for listing)
#[derive(Debug, Clone)]
pub struct TB2TaskSummary {
    pub id: String,
    pub name: String,
    pub difficulty: String,
    pub category: String,
    pub docker_image: String,
}

/// Complete TB2 task with all loaded content
#[derive(Debug, Clone)]
pub struct TB2Task {
    pub id: String,
    pub name: String,
    pub instruction: String,
    pub config: TaskToml,
    pub task_dir: PathBuf,
    pub dockerfile_path: PathBuf,
    pub tests_dir: PathBuf,
}

impl TB2Task {
    /// Get the docker image name
    pub fn docker_image(&self) -> &str {
        &self.config.environment.docker_image
    }

    /// Get agent timeout in seconds
    pub fn agent_timeout_sec(&self) -> f64 {
        self.config.agent.timeout_sec
    }

    /// Get verifier timeout in seconds
    pub fn verifier_timeout_sec(&self) -> f64 {
        self.config.verifier.timeout_sec
    }

    /// Get memory limit
    pub fn memory_limit(&self) -> &str {
        &self.config.environment.memory
    }

    /// Get CPU limit
    pub fn cpu_limit(&self) -> u32 {
        self.config.environment.cpus
    }

    /// Get the test.sh path
    pub fn test_script_path(&self) -> PathBuf {
        self.tests_dir.join("test.sh")
    }

    /// Get the test_outputs.py path
    pub fn test_outputs_path(&self) -> PathBuf {
        self.tests_dir.join("test_outputs.py")
    }
}

/// Errors that can occur during task loading
#[derive(Debug, Error)]
pub enum TB2Error {
    #[error("TB2 root directory not found: {0}")]
    RootNotFound(PathBuf),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Failed to read task.toml: {0}")]
    TaskTomlReadError(#[from] std::io::Error),

    #[error("Failed to parse task.toml: {0}")]
    TaskTomlParseError(#[from] toml::de::Error),

    #[error("Missing required file: {0}")]
    MissingFile(PathBuf),
}

/// TB2 Task Loader
pub struct TB2TaskLoader {
    pub tb2_root: PathBuf,
}

impl TB2TaskLoader {
    /// Create a new loader with the default TB2 root
    pub fn new_default() -> Self {
        Self::new(DEFAULT_TB2_ROOT)
    }

    /// Create a new loader with a custom TB2 root
    pub fn new(tb2_root: impl Into<PathBuf>) -> Self {
        let path = tb2_root.into();
        // Expand ~ to home directory
        let expanded = if path.starts_with("~") {
            if let Some(home) = dirs::home_dir() {
                home.join(path.strip_prefix("~").unwrap_or(&path))
            } else {
                path
            }
        } else {
            path
        };
        Self { tb2_root: expanded }
    }

    /// Check if the TB2 root exists
    pub fn is_available(&self) -> bool {
        self.tb2_root.exists() && self.tb2_root.is_dir()
    }

    /// Discover all available TB2 tasks
    pub fn discover_tasks(&self) -> Vec<TB2TaskSummary> {
        if !self.is_available() {
            tracing::warn!(
                target: "mechacoder::tb2",
                "TB2 root not found: {}",
                self.tb2_root.display()
            );
            return Vec::new();
        }

        let mut tasks = Vec::new();

        let entries = match fs::read_dir(&self.tb2_root) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::error!(
                    target: "mechacoder::tb2",
                    "Failed to read TB2 root: {}",
                    e
                );
                return Vec::new();
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Skip hidden directories and common non-task directories
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if dir_name.starts_with('.') || dir_name == "scripts" || dir_name == "docs" {
                continue;
            }

            // Check if task.toml exists
            let task_toml_path = path.join("task.toml");
            if !task_toml_path.exists() {
                continue;
            }

            // Try to parse task.toml for summary
            match self.parse_task_toml(&task_toml_path) {
                Ok(config) => {
                    let id = dir_name.to_string();
                    let name = id
                        .split('-')
                        .map(|s| {
                            let mut c = s.chars();
                            match c.next() {
                                None => String::new(),
                                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");

                    tasks.push(TB2TaskSummary {
                        id,
                        name,
                        difficulty: config.metadata.difficulty,
                        category: config.metadata.category,
                        docker_image: config.environment.docker_image,
                    });
                }
                Err(e) => {
                    tracing::warn!(
                        target: "mechacoder::tb2",
                        "Failed to parse task.toml for {}: {}",
                        dir_name,
                        e
                    );
                }
            }
        }

        // Sort by id
        tasks.sort_by(|a, b| a.id.cmp(&b.id));

        tracing::info!(
            target: "mechacoder::tb2",
            "Discovered {} TB2 tasks",
            tasks.len()
        );

        tasks
    }

    /// Load a specific task by ID
    pub fn load_task(&self, task_id: &str) -> Result<TB2Task, TB2Error> {
        if !self.is_available() {
            return Err(TB2Error::RootNotFound(self.tb2_root.clone()));
        }

        let task_dir = self.tb2_root.join(task_id);
        if !task_dir.exists() {
            return Err(TB2Error::TaskNotFound(task_id.to_string()));
        }

        // Parse task.toml
        let task_toml_path = task_dir.join("task.toml");
        if !task_toml_path.exists() {
            return Err(TB2Error::MissingFile(task_toml_path));
        }
        let config = self.parse_task_toml(&task_toml_path)?;

        // Load instruction.md
        let instruction_path = task_dir.join("instruction.md");
        if !instruction_path.exists() {
            return Err(TB2Error::MissingFile(instruction_path));
        }
        let instruction = fs::read_to_string(&instruction_path)?;

        // Check for tests directory
        let tests_dir = task_dir.join("tests");
        if !tests_dir.exists() {
            return Err(TB2Error::MissingFile(tests_dir.clone()));
        }

        // Check for Dockerfile
        let dockerfile_path = task_dir.join("environment").join("Dockerfile");
        if !dockerfile_path.exists() {
            return Err(TB2Error::MissingFile(dockerfile_path.clone()));
        }

        // Create nice name from ID
        let name = task_id
            .split('-')
            .map(|s| {
                let mut c = s.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        tracing::info!(
            target: "mechacoder::tb2",
            task_id,
            docker_image = %config.environment.docker_image,
            timeout_sec = config.agent.timeout_sec,
            "Loaded TB2 task"
        );

        Ok(TB2Task {
            id: task_id.to_string(),
            name,
            instruction,
            config,
            task_dir,
            dockerfile_path,
            tests_dir,
        })
    }

    /// Parse task.toml file
    fn parse_task_toml(&self, path: &Path) -> Result<TaskToml, TB2Error> {
        let content = fs::read_to_string(path)?;
        let config: TaskToml = toml::from_str(&content)?;
        Ok(config)
    }
}

impl Default for TB2TaskLoader {
    fn default() -> Self {
        Self::new_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loader_creation() {
        let loader = TB2TaskLoader::new("/tmp/tb2");
        assert_eq!(loader.tb2_root, PathBuf::from("/tmp/tb2"));
    }

    #[test]
    fn test_loader_tilde_expansion() {
        let loader = TB2TaskLoader::new("~/code/terminal-bench-2");
        // Should expand ~ to home directory
        assert!(!loader.tb2_root.starts_with("~"));
    }

    #[test]
    fn test_task_name_generation() {
        // Test the name generation logic
        let id = "regex-log";
        let name = id
            .split('-')
            .map(|s| {
                let mut c = s.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(name, "Regex Log");
    }
}
