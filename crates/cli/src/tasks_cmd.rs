//! Issue management CLI commands
//!
//! Implements CLI-001..007 user stories

use crate::{CliError, CliResult, OutputFormat, output::*};
use clap::Subcommand;
use serde::Serialize;
use taskmaster::{Issue, IssueCreate, IssueFilter, IssueRepository, SqliteRepository, IssueStatus, Priority, LabelFilter};

/// Actor ID for CLI operations
const CLI_ACTOR: &str = "cli";

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

impl From<&Issue> for TaskSummary {
    fn from(issue: &Issue) -> Self {
        let priority_str = match issue.priority {
            Priority::Critical => "P0",
            Priority::High => "P1",
            Priority::Medium => "P2",
            Priority::Low => "P3",
            Priority::Backlog => "P4",
        };
        Self {
            id: truncate(&issue.id, 8),
            priority: format_priority(priority_str),
            status: format_status(issue.status.as_str()),
            title: truncate(&issue.title, 50),
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
    issue_type: String,
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
        writeln!(f, "{:12} {}", "Type:".dimmed(), self.issue_type)?;

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

impl From<&Issue> for TaskDetailOutput {
    fn from(issue: &Issue) -> Self {
        let priority_str = match issue.priority {
            Priority::Critical => "P0",
            Priority::High => "P1",
            Priority::Medium => "P2",
            Priority::Low => "P3",
            Priority::Backlog => "P4",
        };
        Self {
            id: issue.id.clone(),
            title: issue.title.clone(),
            description: issue.description.clone(),
            status: issue.status.as_str().to_string(),
            priority: priority_str.to_string(),
            issue_type: issue.issue_type.as_str().to_string(),
            labels: issue.labels.clone(),
            created_at: format_timestamp(&issue.created_at),
            updated_at: format_timestamp(&issue.updated_at),
            assignee: issue.assignee.clone(),
            close_reason: issue.close_reason.clone(),
        }
    }
}

use colored::Colorize;

/// Execute a tasks command
pub async fn execute(cmd: TasksCommand, workdir: &str, format: OutputFormat) -> CliResult<()> {
    let db_path = format!("{}/issues.db", workdir);
    let repo = SqliteRepository::open(&db_path)
        .map_err(|e| CliError::Other(format!("Failed to open issue database: {}", e)))?;
    repo.init()
        .map_err(|e| CliError::Other(format!("Failed to initialize issue database: {}", e)))?;

    match cmd {
        TasksCommand::List { status, priority, tag, ready, limit } => {
            let mut filter = IssueFilter::default();

            if let Some(s) = status {
                filter.status = Some(vec![parse_status(&s)?]);
            }

            if let Some(p) = priority {
                filter.priority = Some(vec![parse_priority(&p)?]);
            }

            if let Some(t) = tag {
                filter.labels = Some(LabelFilter::any(vec![t]));
            }

            filter.limit = Some(limit);

            // Use ready() if --ready flag is set
            let issues = if ready {
                repo.ready(filter)
                    .map_err(|e| CliError::Other(e.to_string()))?
            } else {
                repo.list(filter)
                    .map_err(|e| CliError::Other(e.to_string()))?
            };

            let output = TaskListOutput {
                total: issues.len(),
                tasks: issues.iter().map(TaskSummary::from).collect(),
            };

            print_output(&output, format);
        }

        TasksCommand::Add { title, description, priority, tags, parent: _ } => {
            let issue = IssueCreate {
                title,
                description,
                priority: parse_priority(&priority)?,
                labels: tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_default(),
                ..Default::default()
            };

            let created = repo.create(issue, "cli")
                .map_err(|e| CliError::Other(e.to_string()))?;
            print_success(&format!("Created issue: {}", created.id));
        }

        TasksCommand::Start { id } => {
            repo.start(&id, Some(CLI_ACTOR))
                .map_err(|e| CliError::Other(e.to_string()))?;
            print_success(&format!("Started issue: {}", id));
        }

        TasksCommand::Complete { id, notes } => {
            repo.close(&id, notes.as_deref(), vec![], Some(CLI_ACTOR))
                .map_err(|e| CliError::Other(e.to_string()))?;
            print_success(&format!("Completed issue: {}", id));
        }

        TasksCommand::Block { id, reason } => {
            repo.block(&id, Some(&reason), Some(CLI_ACTOR))
                .map_err(|e| CliError::Other(e.to_string()))?;
            print_success(&format!("Blocked issue: {} ({})", id, reason));
        }

        TasksCommand::Show { id } => {
            let issue = repo.get(&id)
                .map_err(|e| CliError::Other(e.to_string()))?;
            let output = TaskDetailOutput::from(&issue);
            print_output(&output, format);
        }

        TasksCommand::Delete { id, force } => {
            if !force {
                print_warning(&format!("This will delete issue {}. Use --force to confirm.", id));
                return Ok(());
            }

            repo.tombstone(&id, Some("Deleted via CLI"), Some(CLI_ACTOR))
                .map_err(|e| CliError::Other(e.to_string()))?;
            print_success(&format!("Deleted issue: {}", id));
        }
    }

    Ok(())
}

fn parse_status(s: &str) -> CliResult<IssueStatus> {
    match s.to_lowercase().as_str() {
        "open" | "ready" => Ok(IssueStatus::Open),
        "in_progress" | "inprogress" | "progress" => Ok(IssueStatus::InProgress),
        "blocked" => Ok(IssueStatus::Blocked),
        "closed" | "done" | "complete" | "completed" => Ok(IssueStatus::Closed),
        _ => Err(CliError::InvalidArgument(format!("Invalid status: {}", s))),
    }
}

fn parse_priority(s: &str) -> CliResult<Priority> {
    match s.to_uppercase().as_str() {
        "P0" | "0" | "CRITICAL" => Ok(Priority::Critical),
        "P1" | "1" | "HIGH" => Ok(Priority::High),
        "P2" | "2" | "MEDIUM" => Ok(Priority::Medium),
        "P3" | "3" | "LOW" => Ok(Priority::Low),
        "P4" | "4" | "BACKLOG" => Ok(Priority::Backlog),
        _ => Err(CliError::InvalidArgument(format!("Invalid priority: {}", s))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status() {
        assert!(matches!(parse_status("open"), Ok(IssueStatus::Open)));
        assert!(matches!(parse_status("ready"), Ok(IssueStatus::Open)));
        assert!(matches!(parse_status("in_progress"), Ok(IssueStatus::InProgress)));
        assert!(matches!(parse_status("blocked"), Ok(IssueStatus::Blocked)));
        assert!(matches!(parse_status("closed"), Ok(IssueStatus::Closed)));
        assert!(matches!(parse_status("done"), Ok(IssueStatus::Closed)));
        assert!(parse_status("invalid").is_err());
    }

    #[test]
    fn test_parse_priority() {
        assert!(matches!(parse_priority("P0"), Ok(Priority::Critical)));
        assert!(matches!(parse_priority("p1"), Ok(Priority::High)));
        assert!(matches!(parse_priority("2"), Ok(Priority::Medium)));
        assert!(matches!(parse_priority("P3"), Ok(Priority::Low)));
        assert!(matches!(parse_priority("P4"), Ok(Priority::Backlog)));
        assert!(parse_priority("P5").is_err());
    }
}
