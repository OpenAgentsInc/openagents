//! Task management CLI commands
//!
//! Implements CLI-001..007 user stories

use crate::{CliError, CliResult, OutputFormat, output::*};
use clap::Subcommand;
use serde::Serialize;
use tasks::{Task, TaskCreate, TaskFilter, TaskRepository, SqliteRepository, TaskStatus, TaskPriority};

/// Task management commands
#[derive(Subcommand, Debug)]
pub enum TasksCommand {
    /// List tasks with optional filtering (CLI-001)
    List {
        /// Filter by status (ready, in_progress, blocked, done)
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by priority (P0, P1, P2)
        #[arg(short, long)]
        priority: Option<String>,

        /// Filter by tag
        #[arg(short, long)]
        tag: Option<String>,

        /// Show only ready tasks
        #[arg(long)]
        ready: bool,

        /// Maximum number of tasks to show
        #[arg(short = 'n', long, default_value = "20")]
        limit: usize,
    },

    /// Add a new task (CLI-002)
    Add {
        /// Task title
        title: String,

        /// Task description
        #[arg(short, long)]
        description: Option<String>,

        /// Priority (P0, P1, P2)
        #[arg(short, long, default_value = "P1")]
        priority: String,

        /// Tags (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,

        /// Parent task ID
        #[arg(long)]
        parent: Option<String>,
    },

    /// Start a task (mark as in_progress) (CLI-003)
    Start {
        /// Task ID
        id: String,
    },

    /// Complete a task (CLI-004)
    Complete {
        /// Task ID
        id: String,

        /// Completion notes
        #[arg(short, long)]
        notes: Option<String>,
    },

    /// Block a task with reason (CLI-005)
    Block {
        /// Task ID
        id: String,

        /// Reason for blocking
        reason: String,
    },

    /// Show task details (CLI-006)
    Show {
        /// Task ID
        id: String,
    },

    /// Delete a task (CLI-007)
    Delete {
        /// Task ID
        id: String,

        /// Force delete without confirmation
        #[arg(short, long)]
        force: bool,
    },
}

/// Task list output for serialization
#[derive(Serialize)]
struct TaskListOutput {
    tasks: Vec<TaskSummary>,
    total: usize,
}

impl std::fmt::Display for TaskListOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use tabled::{Table, settings::Style};

        if self.tasks.is_empty() {
            return writeln!(f, "No tasks found");
        }

        let table = Table::new(&self.tasks)
            .with(Style::rounded())
            .to_string();

        writeln!(f, "{}", table)?;
        writeln!(f, "\nTotal: {} tasks", self.total)
    }
}

/// Task summary for table display
#[derive(Serialize, tabled::Tabled)]
struct TaskSummary {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Pri")]
    priority: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Title")]
    title: String,
}

impl From<&Task> for TaskSummary {
    fn from(task: &Task) -> Self {
        let priority_str = match task.priority {
            TaskPriority::Critical => "P0",
            TaskPriority::High => "P1",
            TaskPriority::Medium => "P2",
            TaskPriority::Low => "P3",
            TaskPriority::Backlog => "P4",
        };
        Self {
            id: truncate(&task.id, 8),
            priority: format_priority(priority_str),
            status: format_status(task.status.as_str()),
            title: truncate(&task.title, 50),
        }
    }
}

/// Task detail output
#[derive(Serialize)]
struct TaskDetailOutput {
    id: String,
    title: String,
    description: String,
    status: String,
    priority: String,
    task_type: String,
    labels: Vec<String>,
    created_at: String,
    updated_at: String,
    assignee: Option<String>,
    close_reason: Option<String>,
}

impl std::fmt::Display for TaskDetailOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", "Task Details".bold())?;
        writeln!(f, "─────────────────────────────────")?;
        writeln!(f, "{:12} {}", "ID:".dimmed(), self.id)?;
        writeln!(f, "{:12} {}", "Title:".dimmed(), self.title)?;
        writeln!(f, "{:12} {}", "Status:".dimmed(), format_status(&self.status))?;
        writeln!(f, "{:12} {}", "Priority:".dimmed(), format_priority(&self.priority))?;
        writeln!(f, "{:12} {}", "Type:".dimmed(), self.task_type)?;

        if !self.description.is_empty() {
            writeln!(f, "{:12} {}", "Description:".dimmed(), self.description)?;
        }

        if !self.labels.is_empty() {
            writeln!(f, "{:12} {}", "Labels:".dimmed(), self.labels.join(", "))?;
        }

        if let Some(ref assignee) = self.assignee {
            writeln!(f, "{:12} {}", "Assignee:".dimmed(), assignee)?;
        }

        if let Some(ref reason) = self.close_reason {
            writeln!(f, "{:12} {}", "Close Reason:".dimmed(), reason)?;
        }

        writeln!(f, "{:12} {}", "Created:".dimmed(), self.created_at)?;
        writeln!(f, "{:12} {}", "Updated:".dimmed(), self.updated_at)?;

        Ok(())
    }
}

impl From<&Task> for TaskDetailOutput {
    fn from(task: &Task) -> Self {
        let priority_str = match task.priority {
            TaskPriority::Critical => "P0",
            TaskPriority::High => "P1",
            TaskPriority::Medium => "P2",
            TaskPriority::Low => "P3",
            TaskPriority::Backlog => "P4",
        };
        Self {
            id: task.id.clone(),
            title: task.title.clone(),
            description: task.description.clone(),
            status: task.status.as_str().to_string(),
            priority: priority_str.to_string(),
            task_type: task.task_type.as_str().to_string(),
            labels: task.labels.clone(),
            created_at: format_timestamp(&task.created_at),
            updated_at: format_timestamp(&task.updated_at),
            assignee: task.assignee.clone(),
            close_reason: task.close_reason.clone(),
        }
    }
}

use colored::Colorize;

/// Execute a tasks command
pub async fn execute(cmd: TasksCommand, workdir: &str, format: OutputFormat) -> CliResult<()> {
    let db_path = format!("{}/tasks.db", workdir);
    let repo = SqliteRepository::open(&db_path)
        .map_err(|e| CliError::Other(format!("Failed to open task database: {}", e)))?;
    repo.init()
        .map_err(|e| CliError::Other(format!("Failed to initialize task database: {}", e)))?;

    match cmd {
        TasksCommand::List { status, priority, tag, ready, limit } => {
            let mut filter = TaskFilter::default();

            if let Some(s) = status {
                filter.status = Some(parse_status(&s)?);
            }

            if let Some(p) = priority {
                filter.priority = Some(parse_priority(&p)?);
            }

            if let Some(t) = tag {
                filter.labels = Some(vec![t]);
            }

            filter.limit = Some(limit);

            // Use ready_tasks if --ready flag is set
            let tasks = if ready {
                repo.ready_tasks(filter)
                    .map_err(|e| CliError::Other(e.to_string()))?
            } else {
                repo.list(filter)
                    .map_err(|e| CliError::Other(e.to_string()))?
            };

            let output = TaskListOutput {
                total: tasks.len(),
                tasks: tasks.iter().map(TaskSummary::from).collect(),
            };

            print_output(&output, format);
        }

        TasksCommand::Add { title, description, priority, tags, parent: _ } => {
            let task = TaskCreate {
                title,
                description,
                priority: parse_priority(&priority)?,
                labels: tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_default(),
                ..Default::default()
            };

            let created = repo.create(task)
                .map_err(|e| CliError::Other(e.to_string()))?;
            print_success(&format!("Created task: {}", created.id));
        }

        TasksCommand::Start { id } => {
            repo.start(&id)
                .map_err(|e| CliError::Other(e.to_string()))?;
            print_success(&format!("Started task: {}", id));
        }

        TasksCommand::Complete { id, notes } => {
            repo.close(&id, notes.as_deref(), vec![])
                .map_err(|e| CliError::Other(e.to_string()))?;
            print_success(&format!("Completed task: {}", id));
        }

        TasksCommand::Block { id, reason } => {
            repo.block(&id, Some(&reason))
                .map_err(|e| CliError::Other(e.to_string()))?;
            print_success(&format!("Blocked task: {} ({})", id, reason));
        }

        TasksCommand::Show { id } => {
            let task = repo.get(&id)
                .map_err(|e| CliError::Other(e.to_string()))?;
            let output = TaskDetailOutput::from(&task);
            print_output(&output, format);
        }

        TasksCommand::Delete { id, force } => {
            if !force {
                print_warning(&format!("This will delete task {}. Use --force to confirm.", id));
                return Ok(());
            }

            repo.delete(&id, Some("Deleted via CLI"))
                .map_err(|e| CliError::Other(e.to_string()))?;
            print_success(&format!("Deleted task: {}", id));
        }
    }

    Ok(())
}

fn parse_status(s: &str) -> CliResult<TaskStatus> {
    match s.to_lowercase().as_str() {
        "open" | "ready" => Ok(TaskStatus::Open),
        "in_progress" | "inprogress" | "progress" => Ok(TaskStatus::InProgress),
        "blocked" => Ok(TaskStatus::Blocked),
        "closed" | "done" | "complete" | "completed" => Ok(TaskStatus::Closed),
        "commit_pending" => Ok(TaskStatus::CommitPending),
        _ => Err(CliError::InvalidArgument(format!("Invalid status: {}", s))),
    }
}

fn parse_priority(s: &str) -> CliResult<TaskPriority> {
    match s.to_uppercase().as_str() {
        "P0" | "0" | "CRITICAL" => Ok(TaskPriority::Critical),
        "P1" | "1" | "HIGH" => Ok(TaskPriority::High),
        "P2" | "2" | "MEDIUM" => Ok(TaskPriority::Medium),
        "P3" | "3" | "LOW" => Ok(TaskPriority::Low),
        "P4" | "4" | "BACKLOG" => Ok(TaskPriority::Backlog),
        _ => Err(CliError::InvalidArgument(format!("Invalid priority: {}", s))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status() {
        assert!(matches!(parse_status("open"), Ok(TaskStatus::Open)));
        assert!(matches!(parse_status("ready"), Ok(TaskStatus::Open)));
        assert!(matches!(parse_status("in_progress"), Ok(TaskStatus::InProgress)));
        assert!(matches!(parse_status("blocked"), Ok(TaskStatus::Blocked)));
        assert!(matches!(parse_status("closed"), Ok(TaskStatus::Closed)));
        assert!(matches!(parse_status("done"), Ok(TaskStatus::Closed)));
        assert!(parse_status("invalid").is_err());
    }

    #[test]
    fn test_parse_priority() {
        assert!(matches!(parse_priority("P0"), Ok(TaskPriority::Critical)));
        assert!(matches!(parse_priority("p1"), Ok(TaskPriority::High)));
        assert!(matches!(parse_priority("2"), Ok(TaskPriority::Medium)));
        assert!(matches!(parse_priority("P3"), Ok(TaskPriority::Low)));
        assert!(matches!(parse_priority("P4"), Ok(TaskPriority::Backlog)));
        assert!(parse_priority("P5").is_err());
    }
}
