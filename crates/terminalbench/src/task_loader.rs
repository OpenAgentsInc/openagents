//! Task loader - loads Terminal-Bench tasks from JSON suite files

use std::path::{Path, PathBuf};
use std::fs;
use serde::Deserialize;

use crate::types::{TBTask, TBDifficulty};

/// Raw task as stored in JSON suite files
#[derive(Debug, Clone, Deserialize)]
pub struct RawTask {
    pub id: String,
    pub name: String,
    pub description: String,
    pub difficulty: String,
    pub category: Option<String>,
    pub verification: serde_json::Value,
    pub timeout_seconds: Option<u32>,
    pub max_turns: Option<u32>,
    pub tags: Option<Vec<String>>,
    pub setup: Option<Vec<String>>,
    pub hints: Option<Vec<String>>,
}

/// Terminal-Bench suite file format
#[derive(Debug, Clone, Deserialize)]
pub struct TaskSuite {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub tasks: Vec<RawTask>,
}

/// Loaded suite with converted tasks
#[derive(Debug, Clone)]
pub struct LoadedSuite {
    pub name: String,
    pub version: String,
    pub description: String,
    pub tasks: Vec<TBTask>,
    pub source_path: PathBuf,
}

/// Task loader service
pub struct TaskLoader {
    /// Default paths to search for task suites
    search_paths: Vec<PathBuf>,
}

impl TaskLoader {
    pub fn new() -> Self {
        let mut search_paths = vec![];

        // Add common task suite locations
        if let Ok(cwd) = std::env::current_dir() {
            // Primary location: docs/tb-tasks
            search_paths.push(cwd.join("docs/tb-tasks"));
            // Also check legacy locations
            search_paths.push(cwd.join("tasks"));
            search_paths.push(cwd.join("suites"));
        }

        // Add relative to exe location
        if let Ok(exe) = std::env::current_exe()
            && let Some(parent) = exe.parent() {
                search_paths.push(parent.join("../docs/tb-tasks"));
                search_paths.push(parent.join("tasks"));
                search_paths.push(parent.join("../tasks"));
            }

        Self { search_paths }
    }

    /// Create with specific search paths
    pub fn with_paths(paths: Vec<PathBuf>) -> Self {
        Self { search_paths: paths }
    }

    /// Add a custom search path
    pub fn add_search_path(&mut self, path: impl Into<PathBuf>) {
        self.search_paths.push(path.into());
    }

    /// List available suite files
    pub fn list_available_suites(&self) -> Vec<PathBuf> {
        let mut suites = vec![];

        eprintln!("[TaskLoader] Searching for task suites in {} paths:", self.search_paths.len());
        for search_path in &self.search_paths {
            eprintln!("  - Checking: {}", search_path.display());
            if let Ok(entries) = fs::read_dir(search_path) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.extension().map(|e| e == "json").unwrap_or(false) {
                        // Quick check if it looks like a suite file
                        if let Ok(content) = fs::read_to_string(&path)
                            && content.contains("\"tasks\"") {
                                eprintln!("    ✓ Found suite: {}", path.display());
                                suites.push(path);
                            }
                    }
                }
            } else {
                eprintln!("    ✗ Path not accessible");
            }
        }

        eprintln!("[TaskLoader] Found {} task suite(s)", suites.len());
        suites
    }

    /// Load a specific suite file
    pub fn load_suite(&self, path: impl AsRef<Path>) -> Result<LoadedSuite, TaskLoadError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .map_err(|e| TaskLoadError::IoError(e.to_string()))?;

        let raw_suite: TaskSuite = serde_json::from_str(&content)
            .map_err(|e| TaskLoadError::ParseError(e.to_string()))?;

        let tasks = raw_suite.tasks.iter()
            .map(convert_task)
            .collect();

        Ok(LoadedSuite {
            name: raw_suite.name,
            version: raw_suite.version,
            description: raw_suite.description.unwrap_or_default(),
            tasks,
            source_path: path.to_path_buf(),
        })
    }

    /// Load all available suites
    pub fn load_all_suites(&self) -> Vec<LoadedSuite> {
        self.list_available_suites()
            .into_iter()
            .filter_map(|path| self.load_suite(&path).ok())
            .collect()
    }

    /// Get all tasks from all suites
    pub fn load_all_tasks(&self) -> Vec<TBTask> {
        self.load_all_suites()
            .into_iter()
            .flat_map(|suite| suite.tasks)
            .collect()
    }

    /// Find a suite by name
    pub fn find_suite(&self, name: &str) -> Option<PathBuf> {
        self.list_available_suites()
            .into_iter()
            .find(|path| {
                path.file_stem()
                    .map(|s| s.to_string_lossy().contains(name))
                    .unwrap_or(false)
            })
    }

    /// Find a task by ID across all suites
    pub fn find_task(&self, task_id: &str) -> Option<TBTask> {
        self.load_all_tasks()
            .into_iter()
            .find(|t| t.id == task_id)
    }
}

impl Default for TaskLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert raw task to TBTask
fn convert_task(raw: &RawTask) -> TBTask {
    TBTask {
        id: raw.id.clone(),
        name: raw.name.clone(),
        description: raw.description.clone(),
        difficulty: parse_difficulty(&raw.difficulty),
        timeout_ms: raw.timeout_seconds.unwrap_or(120) * 1000,
        max_turns: raw.max_turns.unwrap_or(50),
        tags: raw.tags.clone().unwrap_or_default(),
    }
}

/// Parse difficulty string to enum
fn parse_difficulty(s: &str) -> TBDifficulty {
    match s.to_lowercase().as_str() {
        "easy" => TBDifficulty::Easy,
        "medium" => TBDifficulty::Medium,
        "hard" => TBDifficulty::Hard,
        "expert" => TBDifficulty::Expert,
        _ => TBDifficulty::Unknown,
    }
}

#[derive(Debug)]
pub enum TaskLoadError {
    IoError(String),
    ParseError(String),
}

impl std::fmt::Display for TaskLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "IO error: {}", e),
            Self::ParseError(e) => write!(f, "Parse error: {}", e),
        }
    }
}

impl std::error::Error for TaskLoadError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_difficulty() {
        assert_eq!(parse_difficulty("easy"), TBDifficulty::Easy);
        assert_eq!(parse_difficulty("MEDIUM"), TBDifficulty::Medium);
        assert_eq!(parse_difficulty("Hard"), TBDifficulty::Hard);
        assert_eq!(parse_difficulty("expert"), TBDifficulty::Expert);
        assert_eq!(parse_difficulty("unknown"), TBDifficulty::Unknown);
    }
}
