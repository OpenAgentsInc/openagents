//! Task discovery from various sources.

use crate::config::{AutoConfig, TaskSource};
use crate::Result;
use std::path::PathBuf;
use taskmaster::{Issue, IssueFilter, IssueRepository, SqliteRepository};

/// A discovered task ready for execution.
#[derive(Debug, Clone)]
pub struct DiscoveredTask {
    /// Task ID (from taskmaster or plan filename).
    pub id: String,
    /// Task title.
    pub title: String,
    /// Task description/body.
    pub description: Option<String>,
    /// Source of the task.
    pub source: TaskDiscoverySource,
    /// Priority (lower = higher priority).
    pub priority: i32,
    /// Labels/tags.
    pub labels: Vec<String>,
}

/// Source of a discovered task.
#[derive(Debug, Clone)]
pub enum TaskDiscoverySource {
    /// From taskmaster database.
    Taskmaster {
        /// Full issue data.
        issue_id: String,
    },
    /// From Claude plan file.
    Plan {
        /// Path to plan file.
        plan_path: PathBuf,
    },
    /// Explicitly specified.
    Explicit,
}

/// Discovery results.
#[derive(Debug)]
pub struct Discovery {
    /// Discovered tasks.
    tasks: Vec<DiscoveredTask>,
    /// Source used.
    source: TaskSource,
}

impl Discovery {
    /// Discover tasks based on configuration.
    pub fn discover(config: &AutoConfig) -> Result<Self> {
        match &config.task_source {
            TaskSource::Taskmaster { db_path } => Self::from_taskmaster(db_path),
            TaskSource::Plans { claude_dir } => Self::from_plans(claude_dir),
            TaskSource::Explicit { task_ids } => Self::from_explicit(task_ids, config),
            TaskSource::Auto => Self::auto_discover(config),
        }
    }

    /// Discover tasks from taskmaster database.
    fn from_taskmaster(db_path: &PathBuf) -> Result<Self> {
        let repo = SqliteRepository::open(db_path)?;

        // Get ready tasks (open, no blocking dependencies)
        let filter = IssueFilter::default();
        let issues = repo.ready(filter)?;

        let tasks = issues
            .into_iter()
            .map(|issue| DiscoveredTask {
                id: issue.id.clone(),
                title: issue.title.clone(),
                description: if issue.description.is_empty() {
                    None
                } else {
                    Some(issue.description.clone())
                },
                source: TaskDiscoverySource::Taskmaster {
                    issue_id: issue.id,
                },
                priority: issue.priority.as_i32(),
                labels: issue.labels.clone(),
            })
            .collect();

        Ok(Self {
            tasks,
            source: TaskSource::Taskmaster {
                db_path: db_path.clone(),
            },
        })
    }

    /// Discover tasks from Claude plan files.
    fn from_plans(claude_dir: &PathBuf) -> Result<Self> {
        let plans_dir = claude_dir.join("plans");
        let mut tasks = Vec::new();

        if plans_dir.exists() {
            for entry in walkdir::WalkDir::new(&plans_dir)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "md") {
                    if let Some(task) = Self::parse_plan_file(path) {
                        tasks.push(task);
                    }
                }
            }
        }

        Ok(Self {
            tasks,
            source: TaskSource::Plans {
                claude_dir: claude_dir.clone(),
            },
        })
    }

    /// Parse a plan file into a task.
    fn parse_plan_file(path: &std::path::Path) -> Option<DiscoveredTask> {
        let content = std::fs::read_to_string(path).ok()?;
        let filename = path.file_stem()?.to_string_lossy().to_string();

        // Extract title from first heading or filename
        let title = content
            .lines()
            .find(|line| line.starts_with("# "))
            .map(|line| line.trim_start_matches("# ").to_string())
            .unwrap_or_else(|| filename.clone());

        Some(DiscoveredTask {
            id: filename,
            title,
            description: Some(content),
            source: TaskDiscoverySource::Plan {
                plan_path: path.to_path_buf(),
            },
            priority: 0,
            labels: vec![],
        })
    }

    /// Discover from explicit task IDs.
    fn from_explicit(task_ids: &[String], config: &AutoConfig) -> Result<Self> {
        // Try to find these tasks in taskmaster
        let db_path = config.working_directory.join("taskmaster.db");

        if db_path.exists() {
            let repo = SqliteRepository::open(&db_path)?;
            let mut tasks = Vec::new();

            for id in task_ids {
                if let Ok(issue) = repo.get(id) {
                    tasks.push(DiscoveredTask {
                        id: issue.id.clone(),
                        title: issue.title.clone(),
                        description: if issue.description.is_empty() {
                            None
                        } else {
                            Some(issue.description.clone())
                        },
                        source: TaskDiscoverySource::Taskmaster {
                            issue_id: issue.id,
                        },
                        priority: issue.priority.as_i32(),
                        labels: issue.labels.clone(),
                    });
                }
            }

            Ok(Self {
                tasks,
                source: TaskSource::Explicit {
                    task_ids: task_ids.to_vec(),
                },
            })
        } else {
            // Create placeholder tasks
            let tasks = task_ids
                .iter()
                .map(|id| DiscoveredTask {
                    id: id.clone(),
                    title: id.clone(),
                    description: None,
                    source: TaskDiscoverySource::Explicit,
                    priority: 0,
                    labels: vec![],
                })
                .collect();

            Ok(Self {
                tasks,
                source: TaskSource::Explicit {
                    task_ids: task_ids.to_vec(),
                },
            })
        }
    }

    /// Auto-discover: try taskmaster, fall back to plans.
    fn auto_discover(config: &AutoConfig) -> Result<Self> {
        // Try taskmaster.db in working directory
        let db_path = config.working_directory.join("taskmaster.db");
        if db_path.exists() {
            let result = Self::from_taskmaster(&db_path);
            if result.is_ok() && result.as_ref().is_ok_and(|d| !d.tasks.is_empty()) {
                return result;
            }
        }

        // Try ~/.claude/plans/
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let claude_dir = PathBuf::from(home).join(".claude");
        if claude_dir.exists() {
            let result = Self::from_plans(&claude_dir);
            if result.is_ok() && result.as_ref().is_ok_and(|d| !d.tasks.is_empty()) {
                return result;
            }
        }

        // Try .claude/plans/ in working directory
        let local_claude = config.working_directory.join(".claude");
        if local_claude.exists() {
            return Self::from_plans(&local_claude);
        }

        Ok(Self {
            tasks: vec![],
            source: TaskSource::Auto,
        })
    }

    /// Get discovered tasks.
    pub fn tasks(&self) -> Vec<DiscoveredTask> {
        self.tasks.clone()
    }

    /// Get number of tasks.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Check if any tasks were discovered.
    pub fn has_tasks(&self) -> bool {
        !self.tasks.is_empty()
    }

    /// Get the source used for discovery.
    pub fn source(&self) -> &TaskSource {
        &self.source
    }
}

/// Extension trait for Priority to get i32 value.
trait PriorityExt {
    fn as_i32(&self) -> i32;
}

impl PriorityExt for taskmaster::Priority {
    fn as_i32(&self) -> i32 {
        match self {
            taskmaster::Priority::Critical => 0,
            taskmaster::Priority::High => 1,
            taskmaster::Priority::Medium => 2,
            taskmaster::Priority::Low => 3,
            taskmaster::Priority::Backlog => 4,
        }
    }
}
