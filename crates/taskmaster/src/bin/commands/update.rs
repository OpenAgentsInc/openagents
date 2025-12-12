//! Update command

use clap::Args;
use colored::Colorize;
use taskmaster::{IssueRepository, IssueStatus, IssueType, IssueUpdate, Priority, Result};

#[derive(Args)]
pub struct UpdateArgs {
    /// Issue ID
    id: String,

    /// New title
    #[arg(long)]
    title: Option<String>,

    /// New description
    #[arg(long)]
    description: Option<String>,

    /// New status
    #[arg(long)]
    status: Option<String>,

    /// New priority
    #[arg(long)]
    priority: Option<String>,

    /// New type
    #[arg(long)]
    issue_type: Option<String>,

    /// New assignee
    #[arg(long)]
    assignee: Option<String>,

    /// Clear assignee
    #[arg(long)]
    unassign: bool,

    /// Actor name
    #[arg(long)]
    actor: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

pub fn run(repo: &impl IssueRepository, args: UpdateArgs) -> Result<()> {
    let mut update = IssueUpdate::new();

    if let Some(title) = args.title {
        update = update.title(title);
    }

    if let Some(desc) = args.description {
        update = update.description(desc);
    }

    if let Some(status_str) = args.status {
        let status: IssueStatus = status_str.parse().map_err(|e| {
            taskmaster::TaskmasterError::validation(format!("Invalid status: {}", e))
        })?;
        update = update.status(status);
    }

    if let Some(priority_str) = args.priority {
        let priority: Priority = priority_str.parse().map_err(|e| {
            taskmaster::TaskmasterError::validation(format!("Invalid priority: {}", e))
        })?;
        update = update.priority(priority);
    }

    if let Some(type_str) = args.issue_type {
        let issue_type: IssueType = type_str.parse().map_err(|e| {
            taskmaster::TaskmasterError::validation(format!("Invalid type: {}", e))
        })?;
        update.issue_type = Some(issue_type);
    }

    if args.unassign {
        update = update.assignee(None);
    } else if let Some(assignee) = args.assignee {
        update = update.assignee(Some(assignee));
    }

    let issue = repo.update(&args.id, update, args.actor.as_deref())?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&issue)?);
    } else {
        println!("{} {}", "Updated:".green().bold(), issue.id.cyan());
        println!("{} {}", "Title:".bold(), issue.title);
        println!("{} {}", "Status:".bold(), issue.status);
    }

    Ok(())
}
